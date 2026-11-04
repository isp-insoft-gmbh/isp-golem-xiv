use golem_runner::{OUTPUT_BYTES_MAX, Result};
use std::{
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const IMAGE: &str = "isp-golem-xiv-agent:v0";
const FORBIDDEN_READ_ROOTS: [&str; 6] = ["proc", "sys", "dev", "home", "tmp", "run"];
const TARGET_TIMEOUT_SECONDS: u64 = 600;

pub struct Execution {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

fn claude_settings() -> String {
    let mut deny = vec![
        "Bash".to_owned(),
        "WebFetch".to_owned(),
        "WebSearch".to_owned(),
    ];
    for root in FORBIDDEN_READ_ROOTS {
        for tool in ["Read", "Glob", "Grep"] {
            deny.push(format!("{tool}(//{root}/**)"));
        }
    }
    serde_json::json!({
        "disableAllHooks": true,
        "permissions": {
            "allow": ["Read(//workspace/**)", "Glob(//workspace/**)", "Grep(//workspace/**)"],
            "deny": deny
        }
    })
    .to_string()
}

fn writable_settings() -> String {
    serde_json::json!({
        "disableAllHooks": true,
        "permissions": {
            "allow": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
            "deny": ["WebFetch", "WebSearch"]
        }
    })
    .to_string()
}

fn docker() -> Command {
    let mut command = Command::new("docker");
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

async fn collect_bounded<R: AsyncRead + Unpin>(mut reader: R, bytes: &mut Vec<u8>) -> Result<()> {
    let mut chunk = [0; 4096];
    loop {
        let count = reader
            .read(&mut chunk)
            .await
            .map_err(|_| "cannot read process output")?;
        if count == 0 {
            return Ok(());
        }
        let remaining = OUTPUT_BYTES_MAX.saturating_sub(bytes.len());
        let retained = chunk
            .get(..count.min(remaining))
            .ok_or("invalid output length")?;
        bytes.extend_from_slice(retained);
        if count > remaining {
            return Err("process output exceeds limit");
        }
    }
}

async fn capture(
    mut command: Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<(Execution, Option<ExitStatus>)> {
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|_| "cannot start Docker")?;
    let stdout = child.stdout.take().ok_or("process output unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("process diagnostics unavailable")?;
    let stdin = child.stdin.take();
    // Buffers outlive the cancelled future, retaining bounded partial output.
    let mut execution = Execution {
        stdout: Vec::new(),
        stderr: Vec::new(),
        timed_out: false,
    };
    let operation = async {
        let send = async {
            if let Some(mut stdin) = stdin {
                if let Some(input) = input {
                    stdin
                        .write_all(input)
                        .await
                        .map_err(|_| "cannot send container input")?;
                }
                stdin
                    .shutdown()
                    .await
                    .map_err(|_| "cannot finish container input")?;
            }
            Ok::<(), &'static str>(())
        };
        let ((), (), (), status) = tokio::try_join!(
            collect_bounded(stdout, &mut execution.stdout),
            collect_bounded(stderr, &mut execution.stderr),
            send,
            async { child.wait().await.map_err(|_| "cannot wait for Docker") }
        )?;
        Ok::<_, &'static str>(status)
    };
    if let Ok(status) = tokio::time::timeout(timeout, operation).await {
        Ok((execution, Some(status?)))
    } else {
        execution.timed_out = true;
        Ok((execution, None))
    }
}

async fn execute(
    command: Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let (execution, status) = capture(command, input, timeout).await?;
    if execution.timed_out {
        return Err("container wall-clock budget exhausted");
    }
    if !status.is_some_and(|status| status.success()) {
        return Err("Docker command failed");
    }
    Ok((execution.stdout, execution.stderr))
}

/// # Errors
/// Image staging, build failure, or the ten-minute build deadline.
pub async fn prepare(mise: &Path) -> Result<()> {
    let context = tempfile::tempdir().map_err(|_| "cannot create image build context")?;
    std::fs::copy(mise, context.path().join("mise"))
        .map_err(|_| "cannot stage pinned mise executable")?;
    for (name, contents) in [
        ("Dockerfile", include_str!("../agent.Dockerfile")),
        ("agent.mise.toml", include_str!("../agent.mise.toml")),
        ("container.cjs", include_str!("../container.cjs")),
    ] {
        std::fs::write(context.path().join(name), contents)
            .map_err(|_| "cannot stage trusted image source")?;
    }
    let mut command = docker();
    command
        .args(["build", "--pull", "--tag", IMAGE])
        .arg(context.path());
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let mut child = command.spawn().map_err(|_| "cannot start image build")?;
    let status = tokio::time::timeout(Duration::from_secs(600), child.wait())
        .await
        .map_err(|_| "image build exceeded time limit")?
        .map_err(|_| "cannot wait for image build")?;
    if !status.success() {
        return Err("trusted image build failed");
    }
    Ok(())
}

fn toolchain_path(workspace: &Path) -> Result<PathBuf> {
    let mut name = workspace
        .file_name()
        .ok_or("workspace has no directory name")?
        .to_os_string();
    name.push(".golem-tools");
    Ok(workspace.with_file_name(name))
}

#[cfg(unix)]
fn workspace_identity(workspace: &Path) -> Result<(u32, u32)> {
    use std::os::unix::fs::MetadataExt;
    let metadata =
        std::fs::symlink_metadata(workspace).map_err(|_| "cannot inspect workspace owner")?;
    if !metadata.is_dir() {
        return Err("workspace must be a real directory");
    }
    Ok((metadata.uid(), metadata.gid()))
}

#[cfg(not(unix))]
fn workspace_identity(_workspace: &Path) -> Result<(u32, u32)> {
    Err("writable sandbox requires Unix workspace ownership")
}

fn mount(command: &mut Command, source: &Path, target: &str, readonly: bool) -> Result<()> {
    let source = source.to_str().ok_or("mount path is not UTF-8")?;
    if !Path::new(source).is_absolute() || source.contains([',', '\n', '\r', '\0']) {
        return Err("invalid workspace mount path");
    }
    let readonly = if readonly { ",readonly" } else { "" };
    command.arg("--mount").arg(format!(
        "type=bind,source={source},target={target}{readonly},bind-propagation=rprivate"
    ));
    Ok(())
}

fn create_container(
    workspace: &Path,
    name: &str,
    writable: bool,
    target: bool,
    preparing: bool,
    seconds: u64,
) -> Result<Command> {
    if name.is_empty()
        || name.len() > 128
        || !name.starts_with("golem-")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid sandbox container name");
    }
    if !(1..=18_000).contains(&seconds) {
        return Err("invalid sandbox timeout");
    }
    let (uid, gid) = if target || writable {
        workspace_identity(workspace)?
    } else {
        #[cfg(unix)]
        {
            workspace_identity(workspace)?
        }
        #[cfg(not(unix))]
        {
            (1000, 1000)
        }
    };
    let mut create = docker();
    create
        .args([
            "create",
            "--name",
            name,
            "--interactive",
            "--init",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit=128",
            "--memory=2g",
            "--memory-swap=2g",
            "--cpus=2",
            "--ulimit",
            "nofile=1024:1024",
            "--log-driver=none",
            "--user",
        ])
        .arg(format!("{uid}:{gid}"))
        .arg("--tmpfs")
        .arg(if target {
            "/tmp:rw,nosuid,nodev,exec,size=1g,mode=1777"
        } else {
            "/tmp:rw,nosuid,nodev,noexec,size=256m,mode=1777"
        })
        .arg("--tmpfs")
        .arg(format!(
            "/home/agent:rw,nosuid,nodev,noexec,size=512m,uid={uid},gid={gid},mode=700"
        ));
    mount(&mut create, workspace, "/workspace", !writable)?;
    if target {
        let tools = toolchain_path(workspace)?;
        if !std::fs::symlink_metadata(&tools).is_ok_and(|metadata| metadata.is_dir()) {
            return Err("target toolchain not prepared");
        }
        mount(&mut create, &tools, "/target-tools", !preparing)?;
    }
    create
        .args(["--env", "GOLEM_TIMEOUT_SECONDS"])
        .env("GOLEM_TIMEOUT_SECONDS", seconds.to_string());
    Ok(create)
}

/// Outer errors prohibit ALL publication: destruction was not confirmed.
/// Validation failures before any Docker operation are confirmed-safe inner errors.
async fn lifecycle(
    mut create: Command,
    name: &str,
    input: Option<&[u8]>,
    seconds: u64,
) -> Result<Result<Execution>> {
    // Include create/start/attach in one host wall-clock budget; the image has
    // its own watchdog, independent of Docker client/host cancellation.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let operation = async {
        create.stdout(Stdio::piped());
        execute(create, None, Duration::from_secs(seconds.min(60))).await?;
        let mut start = docker();
        start.args(["start", "--attach", "--interactive", name]);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let (mut execution, status) = capture(start, input, remaining).await?;
        if status.is_some_and(|status| matches!(status.code(), Some(124 | 137))) {
            execution.timed_out = true;
        }
        if !execution.timed_out && !status.is_some_and(|status| status.success()) {
            return Err("container execution failed");
        }
        Ok(execution)
    };
    let result = operation.await;
    let mut remove = docker();
    remove.args(["rm", "--force", name]);
    let (acknowledgment, _) = execute(remove, None, Duration::from_secs(30))
        .await
        .map_err(|_| "container destruction unconfirmed; publication prohibited")?;
    if std::str::from_utf8(&acknowledgment).map(str::trim) != Ok(name) {
        return Err("container destruction acknowledgment mismatch; publication prohibited");
    }
    Ok(result)
}

async fn agent(
    workspace: &Path,
    name: &str,
    seat: &str,
    model: Option<&str>,
    prompt: &str,
    timeout_seconds: u64,
    writable: bool,
) -> Result<Result<Execution>> {
    let mut create =
        match create_container(workspace, name, writable, writable, false, timeout_seconds) {
            Ok(create) => create,
            Err(error) => return Ok(Err(error)),
        };
    configure_agent(&mut create, seat, model, writable);
    lifecycle(create, name, Some(prompt.as_bytes()), timeout_seconds).await
}

fn configure_agent(create: &mut Command, seat: &str, model: Option<&str>, writable: bool) {
    let settings = if writable {
        writable_settings()
    } else {
        claude_settings()
    };
    let tools = if writable {
        "Read,Write,Edit,Bash,Glob,Grep"
    } else {
        "Read,Glob,Grep"
    };
    create
        .args([
            "--env",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "--env",
            "GOLEM_WRITABLE",
        ])
        .env("CLAUDE_CODE_OAUTH_TOKEN", seat)
        .env("GOLEM_WRITABLE", if writable { "1" } else { "0" })
        .args([
            IMAGE,
            "--safe-mode",
            "--print",
            "--output-format",
            if writable { "stream-json" } else { "json" },
        ]);
    if writable {
        create.arg("--verbose");
    }
    create.args([
        "--no-session-persistence",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        "{\"mcpServers\":{}}",
        "--disable-slash-commands",
        "--tools",
        tools,
        "--allowedTools",
    ]);
    if writable {
        create.args(["Read", "Write", "Edit", "Bash", "Glob", "Grep"]);
    } else {
        create.args([
            "Read(//workspace/**)",
            "Glob(//workspace/**)",
            "Grep(//workspace/**)",
        ]);
    }
    create.args(["--settings", &settings, "--max-turns", "32"]);
    if let Some(model) = model {
        create.args(["--model", model]);
    }
}

/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation/execution failed safely, including timeout.
pub async fn run(
    workspace: &Path,
    name: &str,
    seat: &str,
    model: Option<&str>,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<Result<(Vec<u8>, Vec<u8>)>> {
    Ok(
        agent(workspace, name, seat, model, prompt, timeout_seconds, false)
            .await?
            .and_then(|execution| {
                if execution.timed_out {
                    Err("agent wall-clock budget exhausted")
                } else {
                    Ok((execution.stdout, execution.stderr))
                }
            }),
    )
}

/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation/execution failed safely. Timeout instead returns
/// bounded partial streams with `timed_out`, after confirmed destruction.
pub async fn run_writable(
    workspace: &Path,
    name: &str,
    seat: &str,
    model: Option<&str>,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<Result<Execution>> {
    agent(workspace, name, seat, model, prompt, timeout_seconds, true).await
}

/// The sibling `<workspace>.golem-tools` must live in the supervisor's private
/// temporary parent. Caller removes that parent only after destruction is confirmed.
/// Activates installed tool paths and Rust's selected toolchain only, never
/// arbitrary repository environment/hooks. Cargo caches use fresh container HOME.
///
/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation, installation, or the ten-minute budget failed.
pub async fn prepare_target(workspace: &Path, name: &str) -> Result<Result<()>> {
    let tools = match toolchain_path(workspace) {
        Ok(tools) => tools,
        Err(error) => return Ok(Err(error)),
    };
    #[cfg(unix)]
    let (uid, gid) = match workspace_identity(workspace) {
        Ok(identity) => identity,
        Err(error) => return Ok(Err(error)),
    };
    #[cfg(not(unix))]
    if let Err(error) = workspace_identity(workspace) {
        return Ok(Err(error));
    }
    if std::fs::create_dir(&tools).is_err() {
        return Ok(Err("cannot create isolated target toolchain"));
    }
    #[cfg(unix)]
    if std::os::unix::fs::chown(&tools, Some(uid), Some(gid)).is_err() {
        return Ok(Err("cannot set target toolchain owner"));
    }
    let mut create =
        match create_container(workspace, name, true, true, true, TARGET_TIMEOUT_SECONDS) {
            Ok(create) => create,
            Err(error) => return Ok(Err(error)),
        };
    create.args([IMAGE, "--prepare-target"]);
    Ok(lifecycle(create, name, None, TARGET_TIMEOUT_SECONDS)
        .await?
        .and_then(|execution| {
            if execution.timed_out {
                Err("target preparation timed out")
            } else {
                Ok(())
            }
        }))
}

/// Runs even an absent command in a fresh credential-free container.
///
/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation, command execution, or the ten-minute budget failed.
pub async fn verify(workspace: &Path, name: &str, command: Option<&str>) -> Result<Result<()>> {
    let command = command.unwrap_or(":");
    if command.len() > OUTPUT_BYTES_MAX || command.contains('\0') {
        return Ok(Err("invalid verification command"));
    }
    let mut create =
        match create_container(workspace, name, true, true, false, TARGET_TIMEOUT_SECONDS) {
            Ok(create) => create,
            Err(error) => return Ok(Err(error)),
        };
    create.args([IMAGE, "--verify-target"]);
    Ok(lifecycle(
        create,
        name,
        Some(command.as_bytes()),
        TARGET_TIMEOUT_SECONDS,
    )
    .await?
    .and_then(|execution| {
        if execution.timed_out {
            Err("verification timed out")
        } else {
            Ok(())
        }
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn settings_deny_every_read_tool_outside_workspace() {
        let settings: serde_json::Value = serde_json::from_str(&claude_settings()).unwrap();
        let deny = settings["permissions"]["deny"].as_array().unwrap();
        for root in FORBIDDEN_READ_ROOTS {
            for tool in ["Read", "Glob", "Grep"] {
                assert!(
                    deny.iter()
                        .any(|rule| rule == &format!("{tool}(//{root}/**)")),
                    "{tool} must deny /{root}"
                );
            }
        }
    }

    #[test]
    fn agent_output_format_matches_execution_mode() {
        for (writable, format) in [(false, "json"), (true, "stream-json")] {
            let mut command = docker();
            configure_agent(&mut command, "sentinel", None, writable);
            let args: Vec<_> = command.as_std().get_args().collect();
            assert!(
                args.windows(2)
                    .any(|pair| pair[0] == "--output-format" && pair[1] == format)
            );
            assert_eq!(args.iter().any(|arg| *arg == "--verbose"), writable);
        }
    }

    #[test]
    fn harness_entrypoint_is_absolute_even_with_target_tool_paths() {
        let entrypoint = include_str!("../container.cjs");
        assert!(entrypoint.contains("const entry = '/opt/golem/claude-entry'"));
        assert!(entrypoint.contains("javascript ? '/usr/local/bin/node' : entry"));
    }

    #[test]
    fn image_entrypoint_uses_a_trusted_timeout_before_claude() {
        let dockerfile = include_str!("../agent.Dockerfile");
        assert!(dockerfile.contains("ENTRYPOINT [\"/usr/local/bin/golem-timeout\"]"));
        assert!(dockerfile.contains("exec /usr/bin/timeout --signal=KILL"));
    }

    #[test]
    fn writable_permissions_include_all_requested_tools_without_repository_settings() {
        let settings: serde_json::Value = serde_json::from_str(&writable_settings()).unwrap();
        assert_eq!(settings["disableAllHooks"], true);
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        for tool in ["Read", "Write", "Edit", "Bash", "Glob", "Grep"] {
            assert!(allow.iter().any(|rule| rule == tool));
        }
    }

    #[cfg(unix)]
    #[test]
    fn container_uses_workspace_owner_and_only_explicit_mounts_without_credentials() {
        use std::os::unix::fs::MetadataExt;
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let tools = toolchain_path(&workspace).unwrap();
        std::fs::create_dir(&tools).unwrap();
        let command = create_container(&workspace, "golem-test", true, true, false, 60).unwrap();
        let command = command.as_std();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let metadata = std::fs::metadata(&workspace).unwrap();
        assert!(args.contains(&format!("{}:{}", metadata.uid(), metadata.gid())));
        assert!(
            args.iter()
                .any(|arg| arg == "/tmp:rw,nosuid,nodev,exec,size=1g,mode=1777")
        );
        let mounts: Vec<_> = args
            .windows(2)
            .filter(|pair| pair[0] == "--mount")
            .map(|pair| pair[1].as_str())
            .collect();
        assert_eq!(mounts.len(), 2);
        assert!(
            mounts
                .iter()
                .any(|mount| mount.contains("target=/workspace,") && !mount.contains("readonly"))
        );
        assert!(
            mounts
                .iter()
                .any(|mount| mount.contains("target=/target-tools,readonly,"))
        );
        let env: Vec<_> = command
            .get_envs()
            .filter_map(|(key, value)| value.map(|_| key.to_string_lossy().into_owned()))
            .collect();
        assert!(env.iter().all(|key| matches!(
            key.as_str(),
            "PATH" | "SystemRoot" | "GOLEM_TIMEOUT_SECONDS"
        )));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn host_timeout_retains_both_partial_streams_and_success_captures_exit() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf partial; printf diagnostic >&2; sleep 3"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let (execution, status) = capture(command, None, Duration::from_millis(500))
            .await
            .unwrap();
        assert!(execution.timed_out);
        assert!(status.is_none());
        assert_eq!(execution.stdout, b"partial");
        assert_eq!(execution.stderr, b"diagnostic");
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf result"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let (execution, status) = capture(command, None, Duration::from_secs(2))
            .await
            .unwrap();
        assert!(!execution.timed_out);
        assert!(status.unwrap().success());
        assert_eq!(execution.stdout, b"result");
    }

    // A subprocess gives each fake Docker an isolated PATH without mutating
    // process-global environment (which would require unsafe in edition 2024).
    #[cfg(unix)]
    #[test]
    fn lifecycle_requires_removal_acknowledgment_after_every_attempt() {
        use std::os::unix::fs::PermissionsExt;
        for (scenario, create, start, remove) in [
            (
                "success",
                "echo id",
                "printf reply; printf diagnostic >&2",
                "echo \"$3\"",
            ),
            ("create-failure", "exit 1", "exit 99", "echo \"$3\""),
            ("start-failure", "echo id", "exit 1", "echo \"$3\""),
            (
                "image-timeout",
                "echo id",
                "printf partial; printf diagnostic >&2; exit 137",
                "echo \"$3\"",
            ),
            ("remove-failure", "echo id", "printf reply", "exit 1"),
            (
                "remove-mismatch",
                "echo id",
                "printf reply",
                "echo wrong-name",
            ),
            (
                "timeout",
                "echo id",
                "printf partial; exec sleep 3",
                "echo \"$3\"",
            ),
            ("create-timeout", "exec sleep 3", "exit 99", "echo \"$3\""),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let log = directory.path().join("calls");
            let executable = directory.path().join("docker");
            std::fs::write(&executable, format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\ncase \"$1\" in\ncreate) {create};;\nstart) {start};;\nrm) {remove};;\n*) exit 98;;\nesac\n", log.display())).unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "sandbox::tests::lifecycle_probe",
                    "--ignored",
                    "--nocapture",
                ])
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", directory.path().display()),
                )
                .env("GOLEM_TEST_SCENARIO", scenario)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{scenario}: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            let calls = std::fs::read_to_string(log).unwrap();
            assert_eq!(
                calls,
                if scenario.starts_with("create-") {
                    "create\nrm\n"
                } else {
                    "create\nstart\nrm\n"
                }
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "invoked by lifecycle_requires_removal_acknowledgment_after_every_attempt"]
    async fn lifecycle_probe() {
        let scenario = std::env::var("GOLEM_TEST_SCENARIO").unwrap();
        let mut create = docker();
        create.arg("create");
        let result = lifecycle(create, "golem-test", None, 1).await;
        match scenario.as_str() {
            "remove-failure" | "remove-mismatch" => assert!(result.is_err()),
            "create-failure" | "start-failure" | "create-timeout" => {
                assert!(result.unwrap().is_err());
            }
            "timeout" | "image-timeout" => {
                let execution = result.unwrap().unwrap();
                assert!(execution.timed_out);
                assert_eq!(execution.stdout, b"partial");
                if scenario == "image-timeout" {
                    assert_eq!(execution.stderr, b"diagnostic");
                }
            }
            "success" => {
                let execution = result.unwrap().unwrap();
                assert!(!execution.timed_out);
                assert_eq!(execution.stdout, b"reply");
                assert_eq!(execution.stderr, b"diagnostic");
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn invalid_timeout_is_safe_inner_error_without_container_creation() {
        let result = run_writable(
            Path::new("/unused"),
            "golem-test",
            "sentinel",
            None,
            "prompt",
            0,
        )
        .await;
        assert!(matches!(result, Ok(Err("invalid sandbox timeout"))));
    }

    #[tokio::test]
    async fn process_output_limit_rejects_flood_without_retaining_unbounded_bytes() {
        let mut bytes = Vec::new();
        collect_bounded(&b"normal"[..], &mut bytes).await.unwrap();
        assert_eq!(bytes, b"normal");
        bytes.clear();
        let flood = vec![b'x'; OUTPUT_BYTES_MAX + 1];
        assert!(collect_bounded(flood.as_slice(), &mut bytes).await.is_err());
        assert_eq!(bytes.len(), OUTPUT_BYTES_MAX);
    }
}
