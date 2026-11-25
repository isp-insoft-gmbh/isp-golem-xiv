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
const COVERAGE_BASELINE: &str = "coverage.toml";
const COVERAGE_CRATES: [&str; 3] = ["router", "runner", "x"];
const COVERAGE_REPORT_BYTES_MAX: u64 = 16_000_000;

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
        return Err(
            "usage: cargo run -p x -- <ci|test|test-worker|coverage|fmt|dist|worker>".into(),
        );
    };
    match task.as_str() {
        "ci" => ci(),
        "coverage" => coverage(),
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
    script_tests()
}

fn script_tests() -> Result<()> {
    run("node", &["tools/repair-unbound-test.mjs"])?;
    run("node", &["tools/verify-release-test.mjs"])?;
    run("nu", &["--no-config-file", "tools/deploy-test.nu"])?;
    run("node", &["tools/deploy-http-test.mjs"])
}

/// On Windows, Miniflare resets oversized uploads instead of returning the router's 413
/// (decision log, 2026-11-03), so the unchanged suite requires WSL Linux like CI.
/// Set `GOLEM_WORKER_TESTS_NATIVE` to force a native diagnostic run.
fn runtime_tests() -> Result<()> {
    if cfg!(windows) && env::var_os("GOLEM_WORKER_TESTS_NATIVE").is_none() {
        let root = env::current_dir()?;
        let root = root.to_str().ok_or("repository path is not UTF-8")?;
        return run(
            "wsl.exe",
            &[
                "--cd",
                root,
                "--exec",
                "mise",
                "exec",
                "--",
                "node",
                "tools/router-runtime-tests.mjs",
            ],
        );
    }
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
    coverage()?;
    script_tests()?;
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

/// Runs the Rust tests once, instrumented, and holds each crate's line coverage to the
/// committed baseline. Only Linux enforces it: other hosts skip `cfg(unix)` code and tests,
/// so their numbers are lower than the CI measurement the baseline records.
fn coverage() -> Result<()> {
    // Invoke directly: the Windows build treats Cargo's injected subcommand as a path.
    run(
        "cargo-llvm-cov",
        &["llvm-cov", "--locked", "--workspace", "--no-report"],
    )?;
    let baseline_text = fs::read_to_string(COVERAGE_BASELINE)?;
    let baseline = coverage_baseline(&baseline_text)?;
    let root = env::current_dir()?;
    let root = root.to_str().ok_or("repository path is not UTF-8")?;
    let reports = Path::new("target/llvm-cov-summary");
    fs::create_dir_all(reports)?;
    let mut below = Vec::new();
    for (krate, minimum) in baseline {
        let report = reports.join(format!("{krate}.json"));
        let report_path = report.to_str().ok_or("report path is not UTF-8")?;
        run(
            "cargo-llvm-cov",
            &[
                "llvm-cov",
                "report",
                "--json",
                "--summary-only",
                "--output-path",
                report_path,
                "--ignore-filename-regex",
                &coverage_ignore(root, krate)?,
            ],
        )?;
        if fs::metadata(&report)?.len() > COVERAGE_REPORT_BYTES_MAX {
            return Err(format!("{krate} coverage report is too large").into());
        }
        let measured = line_coverage_tenths(&fs::read_to_string(&report)?)?;
        eprintln!(
            "x: {krate} line coverage {} (baseline {})",
            percent(measured),
            percent(minimum)
        );
        if measured < minimum {
            below.push(format!(
                "{krate} {} < {}",
                percent(measured),
                percent(minimum)
            ));
        }
    }
    if below.is_empty() {
        return Ok(());
    }
    let message = format!(
        "line coverage fell below {COVERAGE_BASELINE}: {}; add tests, or lower the baseline and say why in the pull request",
        below.join(", ")
    );
    if cfg!(target_os = "linux") {
        return Err(message.into());
    }
    eprintln!("x: not enforced on this host, Linux CI is: {message}");
    Ok(())
}

fn percent(tenths: u32) -> String {
    format!("{}.{}%", tenths / 10, tenths % 10)
}

/// Parses `<crate> = <percent with one decimal>` lines; every measured crate appears once,
/// in `COVERAGE_CRATES` order.
fn coverage_baseline(text: &str) -> Result<Vec<(&str, u32)>> {
    let mut entries = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (krate, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid coverage baseline line: {line}"))?;
        let (whole, tenth) = value
            .trim()
            .split_once('.')
            .ok_or_else(|| format!("coverage needs one decimal: {line}"))?;
        let whole: u32 = whole.parse()?;
        let tenth: u32 = if tenth.len() == 1 {
            tenth.parse()?
        } else {
            return Err(format!("coverage needs one decimal: {line}").into());
        };
        let tenths = whole
            .checked_mul(10)
            .and_then(|tenths| tenths.checked_add(tenth))
            .ok_or_else(|| format!("coverage above 100%: {line}"))?;
        if tenths > 1000 {
            return Err(format!("coverage above 100%: {line}").into());
        }
        entries.push((krate.trim(), tenths));
    }
    let crates: Vec<&str> = entries.iter().map(|(krate, _)| *krate).collect();
    if crates != COVERAGE_CRATES {
        return Err(format!("{COVERAGE_BASELINE} must list {COVERAGE_CRATES:?} in order").into());
    }
    Ok(entries)
}

/// Files outside the crate and test-only files, anchored at the repository root so that
/// directory names above it, such as a CI runner's home `/home/runner`, never match.
fn coverage_ignore(root: &str, krate: &str) -> Result<String> {
    if !COVERAGE_CRATES.contains(&krate) {
        return Err(format!("unknown coverage crate: {krate}").into());
    }
    let others: Vec<&str> = COVERAGE_CRATES
        .iter()
        .copied()
        .filter(|other| *other != krate)
        .collect();
    let mut escaped = String::with_capacity(root.len() * 2);
    for character in root.chars() {
        if ".[]()*+?{}|^$\\".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    Ok(format!(
        "^{escaped}[\\/](({})[\\/]|[^\\/]+[\\/]tests[\\/]|.*[\\/](tests|[a-z_]+_tests)\\.rs$)",
        others.join("|")
    ))
}

/// Line coverage in tenths of a percent, rounded down, from the totals of an
/// `llvm-cov export` summary.
fn line_coverage_tenths(report: &str) -> Result<u32> {
    let totals = report
        .rfind("\"totals\"")
        .and_then(|start| report.get(start..))
        .ok_or("coverage report has no totals")?;
    let lines = totals
        .find("\"lines\":{")
        .and_then(|start| totals.get(start..))
        .ok_or("coverage totals have no lines")?;
    let lines = lines
        .find('}')
        .and_then(|end| lines.get(..end))
        .ok_or("coverage line totals are unterminated")?;
    let number = |key: &str| -> Result<u64> {
        let digits: String = lines
            .split_once(key)
            .ok_or_else(|| format!("coverage line totals have no {key}"))?
            .1
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        Ok(digits.parse()?)
    };
    let count = number("\"count\":")?;
    let covered = number("\"covered\":")?;
    if count == 0 || covered > count {
        return Err("coverage line totals are inconsistent".into());
    }
    Ok(u32::try_from(covered * 1000 / count)?)
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
    // Keep worker-build's compiled Rust default and add the named DO export.
    let directory = Path::new(output);
    for filename in ["coordinator.js", "runtime.js"] {
        fs::copy(
            Path::new("router/src").join(filename),
            directory.join(filename),
        )?;
    }
    let entry = directory.join("index.js");
    let mut javascript = fs::read_to_string(&entry)?;
    javascript.push_str("\nexport { Coordinator } from \"./coordinator.js\";\n");
    fs::write(entry, javascript)?;
    validate_worker(directory)
}

fn validate_worker(directory: &Path) -> Result<()> {
    for filename in ["index.js", "coordinator.js", "runtime.js"] {
        let size = fs::metadata(directory.join(filename))?.len();
        if size == 0 || size > 5_000_000 {
            return Err(format!("worker module {filename} size is invalid").into());
        }
    }
    if fs::metadata(directory.join("index_bg.wasm"))?.len() > 20_000_000 {
        return Err("worker WebAssembly size is invalid".into());
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
    for filename in ["index.js", "index_bg.wasm", "coordinator.js", "runtime.js"] {
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
            "coordinator.js",
            "runtime.js",
        ],
        &output,
    )?;
    verify_archive(
        &output.join("golem-worker.tar.gz"),
        &["index.js", "index_bg.wasm", "coordinator.js", "runtime.js"],
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
    use super::{
        coverage_baseline, coverage_ignore, host_from_version, line_coverage_tenths, percent,
        run_at, validate_worker, verify_archive,
    };
    use std::fs;

    #[test]
    fn coverage_baseline_requires_every_crate_in_order_with_one_decimal() {
        let valid = "# comment\n\nrouter = 81.2\nrunner = 0.0\nx = 100.0\n";
        assert_eq!(
            coverage_baseline(valid).ok(),
            Some(vec![("router", 812), ("runner", 0), ("x", 1000)])
        );
        for invalid in [
            "router = 81.2\nrunner = 70.0\n",
            "runner = 70.0\nrouter = 81.2\nx = 1.0\n",
            "router = 81\nrunner = 70.0\nx = 1.0\n",
            "router = 81.25\nrunner = 70.0\nx = 1.0\n",
            "router = 100.1\nrunner = 70.0\nx = 1.0\n",
            "router 81.2\nrunner = 70.0\nx = 1.0\n",
            "router = -1.0\nrunner = 70.0\nx = 1.0\n",
            "router = 4294967295.0\nrunner = 70.0\nx = 1.0\n",
        ] {
            assert!(coverage_baseline(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn line_coverage_reads_the_totals_not_a_file_summary_and_rounds_down() {
        let report = r#"{"data":[{"files":[{"summary":{"lines":{"count":10,"covered":10,"percent":100}}}],"totals":{"branches":{"count":0,"covered":0},"lines":{"count":3,"covered":2,"percent":66.6},"regions":{"count":1,"covered":1}}}]}"#;
        assert_eq!(line_coverage_tenths(report).ok(), Some(666));
        assert_eq!(percent(666), "66.6%");
        for invalid in [
            r#"{"data":[]}"#,
            r#"{"totals":{"regions":{"count":1}}}"#,
            r#"{"totals":{"lines":{"count":0,"covered":0}}}"#,
            r#"{"totals":{"lines":{"count":1,"covered":2}}}"#,
        ] {
            assert!(line_coverage_tenths(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn coverage_ignore_is_anchored_at_the_root_and_escapes_it() {
        assert_eq!(
            coverage_ignore("/home/runner/work/a.b", "router")
                .ok()
                .as_deref(),
            Some(
                r"^/home/runner/work/a\.b[\/]((runner|x)[\/]|[^\/]+[\/]tests[\/]|.*[\/](tests|[a-z_]+_tests)\.rs$)"
            )
        );
        assert!(
            coverage_ignore(r"C:\w", "x")
                .is_ok_and(|regex| regex.starts_with(r"^C:\\w[\/]((router|runner)[\/]"))
        );
        assert!(coverage_ignore("/w", "other").is_err());
    }

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
            fs::write(
                directory.join("coordinator.js"),
                "export class Coordinator {}",
            )?;
            fs::write(directory.join("runtime.js"), "export function appJwt() {}")?;
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
