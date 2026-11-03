use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WASM_TARGET: &str = "wasm32-unknown-unknown";
const COMMAND_TIMEOUT: Duration = Duration::from_mins(30);

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("x: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("x must be a workspace member")?;
    env::set_current_dir(root)?;
    let arguments: Vec<String> = env::args().skip(1).take(3).collect();
    let [task] = arguments.as_slice() else {
        return Err("usage: cargo run -p x -- <ci|test|test-worker|fmt|dist|worker>".into());
    };
    match task.as_str() {
        "ci" => ci(),
        "test" => {
            tests()?;
            runtime_tests()
        }
        "test-worker" => runtime_tests(),
        "fmt" => {
            run("cargo", &["fmt", "--all"])?;
            run("dprint", &["fmt"])
        }
        "dist" => distribution(),
        "worker" => worker(),
        _ => Err(format!("unknown task: {task}").into()),
    }
}

fn run(program: &str, arguments: &[&str]) -> Result<()> {
    run_at(program, arguments, Path::new("."))
}

fn run_at(program: &str, arguments: &[&str], directory: &Path) -> Result<()> {
    eprintln!("x: {program} {}", arguments.join(" "));
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    while started.elapsed() < COMMAND_TIMEOUT {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("{program} exited with {status}").into())
            };
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.kill()?;
    let _status = child.wait()?;
    Err(format!("{program} exceeded {} seconds", COMMAND_TIMEOUT.as_secs()).into())
}

fn tests() -> Result<()> {
    // Worker glue is wasm-only; the router's deterministic logic is tested on host.
    run("cargo", &["test", "--locked", "--workspace"])?;
    run("nu", &["--no-config-file", "tools/deploy-test.nu"])?;
    run("node", &["tools/deploy-http-test.mjs"])
}

fn runtime_tests() -> Result<()> {
    run("node", &["tools/router-runtime-tests.mjs"])
}

fn ci() -> Result<()> {
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run("dprint", &["check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    tests()?;
    run(
        "cargo",
        &[
            "clippy",
            "--locked",
            "-p",
            "router",
            "--target",
            WASM_TARGET,
            "--",
            "-D",
            "warnings",
        ],
    )?;
    // Exercise release packaging on pull requests, not first after tagging.
    distribution()?;
    runtime_tests()?;
    // Advisories gate lockfile changes, while scheduled CI reports new advisories.
    run(
        "cargo",
        &["deny", "--locked", "check", "bans", "licenses", "sources"],
    )?;
    if env::var("GOLEM_CHECK_ADVISORIES").as_deref() != Ok("false") {
        run("cargo", &["deny", "--locked", "check", "advisories"])?;
    }
    // Invoke directly: the Windows build treats Cargo's injected subcommand as a path.
    run("cargo-machete", &["--with-metadata", "--skip-target-dir"])?;
    run("actionlint", &[])?;
    run(
        "zizmor",
        &["--offline", "--min-severity", "low", ".github/workflows"],
    )?;
    run("typos", &[])?;
    run("rumdl", &["check", "."])?;
    // Preserve the existing judge's independent parser and data checks unchanged.
    run("nu", &["--no-config-file", "tools/ci.nu"])?;
    run(
        "nu",
        &[
            "--no-config-file",
            "-c",
            "for script in (glob tools/*.nu) { if not (nu-check $script) { error make {msg: $\"($script) does not parse\"} } }",
        ],
    )
}

fn worker() -> Result<()> {
    // Absolute paths keep worker-build output independent of invocation directory.
    let output = env::current_dir()?.join("router/build");
    let output = output.to_str().ok_or("worker output path is not UTF-8")?;
    run_at(
        "worker-build",
        &["--release", "--out-dir", output, "--", "--locked"],
        Path::new("router"),
    )?;
    validate_worker(Path::new(output))
}

fn validate_worker(directory: &Path) -> Result<()> {
    let javascript = directory.join("index.js");
    if fs::metadata(&javascript)?.len() == 0 {
        return Err("worker JavaScript is empty".into());
    }
    let mut header = [0; 8];
    fs::File::open(directory.join("index_bg.wasm"))?.read_exact(&mut header)?;
    if header != [0, 97, 115, 109, 1, 0, 0, 0] {
        return Err("worker output is not a WebAssembly v1 module".into());
    }
    Ok(())
}

fn host_from_version(version: &str) -> Result<&str> {
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc did not report a host triple")?;
    if host.is_empty()
        || host.len() > 100
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("invalid rustc host triple".into());
    }
    Ok(host)
}

fn distribution() -> Result<()> {
    let rustc = Command::new("rustc")
        .arg("--version")
        .arg("--verbose")
        .output()?;
    if !rustc.status.success() {
        return Err("rustc --version failed".into());
    }
    let version = String::from_utf8(rustc.stdout)?;
    let host = host_from_version(&version)?;
    run(
        "cargo",
        &[
            "build",
            "--release",
            "--locked",
            "--target",
            host,
            "--target-dir",
            "target",
            "--manifest-path",
            "runner/Cargo.toml",
            "--bin",
            "golem-runner",
        ],
    )?;
    worker()?;
    let output = PathBuf::from("dist");
    fs::create_dir_all(&output)?;
    let binary = if cfg!(windows) {
        "golem-runner.exe"
    } else {
        "golem-runner"
    };
    // Explicit allowlist: never archive a checkout, build tree, or runtime logs.
    fs::copy(
        Path::new("target").join(host).join("release").join(binary),
        output.join(binary),
    )?;
    fs::copy("LICENSE", output.join("LICENSE"))?;
    fs::copy(
        "tools/licenses/CDLA-Permissive-2.0.txt",
        output.join("CDLA-Permissive-2.0.txt"),
    )?;
    let raw_binary = format!(
        "golem-runner-{}-{}{}",
        env::consts::OS,
        env::consts::ARCH,
        env::consts::EXE_SUFFIX
    );
    fs::copy(output.join(binary), output.join(&raw_binary))?;
    let runner_archive = format!("golem-runner-{host}.tar.gz");
    run_at(
        "tar",
        &[
            "-czf",
            &runner_archive,
            binary,
            "LICENSE",
            "CDLA-Permissive-2.0.txt",
        ],
        &output,
    )?;
    verify_archive(
        &output.join(&runner_archive),
        &[binary, "LICENSE", "CDLA-Permissive-2.0.txt"],
    )?;
    let worker_output = output.join("worker");
    fs::create_dir_all(&worker_output)?;
    for filename in ["index.js", "index_bg.wasm"] {
        fs::copy(
            Path::new("router/build").join(filename),
            worker_output.join(filename),
        )?;
    }
    run_at(
        "tar",
        &[
            "-czf",
            "golem-worker.tar.gz",
            "-C",
            "worker",
            "index.js",
            "index_bg.wasm",
        ],
        &output,
    )?;
    verify_archive(
        &output.join("golem-worker.tar.gz"),
        &["index.js", "index_bg.wasm"],
    )?;
    let checksum_command = format!(
        "let raw = \"dist/{raw_binary}\"; let raw_sum = $\"(open --raw $raw | hash sha256)  ($raw | path basename)\"; $raw_sum + (char nl) | save --force \"dist/{raw_binary}.sha256\"; let files = [\"dist/{runner_archive}\" \"dist/golem-worker.tar.gz\" $raw]; ($files | each {{|file| $\"(open --raw $file | hash sha256)  ($file | path basename)\" }} | str join (char nl)) + (char nl) | save --force dist/SHA256SUMS"
    );
    run("nu", &["--no-config-file", "-c", &checksum_command])?;
    Ok(())
}

fn verify_archive(archive: &Path, expected: &[&str]) -> Result<()> {
    let listing = Command::new("tar").arg("-tzf").arg(archive).output()?;
    if !listing.status.success() {
        return Err(format!("cannot list archive {}", archive.display()).into());
    }
    let names = String::from_utf8(listing.stdout)?;
    let actual: Vec<&str> = names.lines().collect();
    if actual != expected {
        return Err(format!("unexpected members in archive {}", archive.display()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{host_from_version, run_at, validate_worker, verify_archive};
    use std::fs;

    #[test]
    fn host_triple_rejects_missing_or_path_like_values() {
        assert_eq!(
            host_from_version("rustc 1.99.1\nhost: x86_64-unknown-linux-gnu\n").ok(),
            Some("x86_64-unknown-linux-gnu")
        );
        for input in ["", "host: ", "host: ../../secret", "host: x;command"] {
            assert!(host_from_version(input).is_err(), "{input}");
        }
        let limit = "a".repeat(100);
        assert_eq!(
            host_from_version(&format!("host: {limit}")).ok(),
            Some(limit.as_str())
        );
        assert!(host_from_version(&format!("host: {}", "a".repeat(101))).is_err());
    }

    #[test]
    fn archive_consumer_accepts_exact_members_and_rejects_extra_files() -> super::Result<()> {
        let directory =
            std::env::temp_dir().join(format!("golem-x-archive-{}", std::process::id()));
        fs::create_dir(&directory)?;
        let result = (|| {
            fs::write(directory.join("golem-runner"), "test executable bytes")?;
            fs::write(directory.join("LICENSE"), "MIT")?;
            fs::write(directory.join("private-log"), "must never be packaged")?;
            run_at(
                "tar",
                &["-czf", "runner.tar.gz", "golem-runner", "LICENSE"],
                &directory,
            )?;
            let archive = directory.join("runner.tar.gz");
            assert!(verify_archive(&archive, &["golem-runner", "LICENSE"]).is_ok());
            assert!(verify_archive(&archive, &["LICENSE"]).is_err());
            run_at(
                "tar",
                &[
                    "-czf",
                    "runner.tar.gz",
                    "golem-runner",
                    "LICENSE",
                    "private-log",
                ],
                &directory,
            )?;
            assert!(verify_archive(&archive, &["golem-runner", "LICENSE"]).is_err());
            Ok(())
        })();
        fs::remove_dir_all(directory)?;
        result
    }

    #[test]
    fn worker_artifact_requires_javascript_and_real_wasm_header() -> std::io::Result<()> {
        let directory = std::env::temp_dir().join(format!("golem-x-worker-{}", std::process::id()));
        fs::create_dir(&directory)?;
        let result = (|| {
            assert!(validate_worker(&directory).is_err());
            fs::write(directory.join("index.js"), "export default {};")?;
            fs::write(directory.join("index_bg.wasm"), b"not wasm")?;
            assert!(validate_worker(&directory).is_err());
            fs::write(
                directory.join("index_bg.wasm"),
                [0, 97, 115, 109, 1, 0, 0, 0],
            )?;
            assert!(validate_worker(&directory).is_ok());
            fs::write(directory.join("index.js"), "")?;
            assert!(validate_worker(&directory).is_err());
            Ok(())
        })();
        fs::remove_dir_all(directory)?;
        result
    }
}
