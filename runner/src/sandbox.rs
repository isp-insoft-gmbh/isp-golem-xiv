use base64::Engine as _;
use golem_runner::{OUTPUT_BYTES_MAX, OutputGuard, Result};
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
/// The in-image watchdog starts after the host deadline and kills with exit 137, so the host
/// always times out first: an exit 137 before then is another kill, never the time budget.
const KILLED: &str = "container killed before its time budget";
/// Docker's record that some process was killed at the memory limit. It is sticky and
/// container-wide, so it is an observation beside the failure, not necessarily its cause.
const MEMORY_LIMIT: &str = "container failed after a process in it was killed at its memory limit";
/// Per stream, what target preparation and verification leave in the private run log.
const TAIL_LINES: usize = 200;
const TAIL_BYTES: usize = 16 * 1024;
/// Private per-run `CODEX_HOME`, outside /workspace; holds only the seat sign-in at start.
const CODEX_HOME: &str = "/golem-codex";
/// Private per-run Pi agent directory for a Codex seat, outside /workspace; holds only Pi's
/// converted `auth.json` at start. `container.cjs` points `PI_CODING_AGENT_DIR` here.
const PI_HOME: &str = "/golem-pi";
/// Strict output schema baked into the image; `OpenAI` strict mode supports no length bounds,
/// so the runner enforces those itself.
const CODEX_SCHEMA: &str = "/opt/golem/codex-result-schema.json";

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
        "permissions": {
            "allow": ["Read(//workspace/**)", "Glob(//workspace/**)", "Grep(//workspace/**)"],
            "deny": deny
        }
    })
    .to_string()
}

/// Golem policy on top of the repository's own settings: deny rules from every source
/// combine, so project denies still apply; project hooks, skills and MCP stay untouched.
fn writable_settings() -> String {
    serde_json::json!({
        "permissions": {
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
        ("pi-report.js", include_str!("../pi-report.js")),
        (
            "codex-result-schema.json",
            include_str!("../codex-result-schema.json"),
        ),
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
            // Thread-heavy target test suites (JUnit) need more than 128; still bounds fork bombs.
            "--pids-limit=1024",
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

/// The last lines of one output stream, at most [`TAIL_LINES`] and [`TAIL_BYTES`], with
/// control characters replaced so target output cannot rewrite the log display.
fn tail(bytes: &[u8]) -> (&[u8], String) {
    let start = bytes.len().saturating_sub(TAIL_BYTES);
    let raw = bytes.get(start..).unwrap_or_default();
    let text = String::from_utf8_lossy(raw);
    // A cut through the middle of a line keeps only whole lines, unless no other line exists.
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && lines.len() > 1 {
        lines.remove(0);
    }
    let kept = lines
        .get(lines.len().saturating_sub(TAIL_LINES)..)
        .unwrap_or_default();
    let text = kept
        .iter()
        .map(|line| {
            line.chars()
                .map(|c| {
                    if c.is_control() && c != '\t' {
                        '\u{fffd}'
                    } else {
                        c
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (raw, text)
}

/// A stream's tail for the private run log, or why it is withheld. `token` stops Actions
/// workflow commands for the tail, so target output cannot annotate or alter the run.
fn output_report(
    stage: &str,
    stream: &str,
    bytes: &[u8],
    guard: &OutputGuard,
    token: &str,
) -> String {
    let (raw, text) = tail(bytes);
    if text.is_empty() {
        return format!("golem {stage} {stream}: empty");
    }
    if golem_runner::changes::check_bytes(raw, guard).is_err() || guard.check(&text).is_err() {
        return format!("golem {stage} {stream} tail withheld: matches a guarded credential");
    }
    format!(
        "golem {stage} {stream} tail ({} lines):\n::stop-commands::{token}\n{text}\n::{token}::",
        text.lines().count()
    )
}

/// Whether Docker recorded an out-of-memory kill anywhere in the container. Observed to include
/// child processes on hosted Ubuntu CI (cgroup v2, Docker 29.4.2, run 44840098741), which
/// `tools/agent-container-test.nu` keeps checking. Unknown means no.
async fn memory_limited(name: &str) -> bool {
    let mut inspect = docker();
    inspect.args(["inspect", "--format", "{{.State.OOMKilled}}", name]);
    execute(inspect, None, Duration::from_secs(30))
        .await
        .is_ok_and(|(stdout, _)| stdout.trim_ascii() == b"true")
}

fn exit_label(status: Option<ExitStatus>) -> String {
    match status {
        None => "none (host deadline)".to_owned(),
        Some(status) => status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string()),
    }
}

/// What a container leaves in the private run log: its stage, exit, duration and output
/// sizes always; its output only with `output_log`, never for agents.
struct Report<'a> {
    stage: &'static str,
    execution_failure: &'static str,
    output_log: Option<&'a OutputGuard>,
}

/// Outer errors prohibit ALL publication: destruction was not confirmed.
/// Validation failures before any Docker operation are confirmed-safe inner errors.
async fn lifecycle(
    mut create: Command,
    name: &str,
    input: Option<&[u8]>,
    seconds: u64,
    report: Report<'_>,
) -> Result<Result<Execution>> {
    // Include create/start/attach in one host wall-clock budget; the image has
    // its own watchdog, independent of Docker client/host cancellation.
    let started = tokio::time::Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let operation = async {
        create.stdout(Stdio::piped());
        execute(create, None, Duration::from_secs(seconds.min(60))).await?;
        let mut start = docker();
        start.args(["start", "--attach", "--interactive", name]);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let (mut execution, status) = capture(start, input, remaining).await?;
        let memory_limit =
            status.is_some_and(|status| !status.success()) && memory_limited(name).await;
        println!(
            "golem container: {} exit {}{} after {}s, stdout {} bytes, stderr {} bytes",
            report.stage,
            exit_label(status),
            if memory_limit {
                ", memory limit hit"
            } else {
                ""
            },
            started.elapsed().as_secs(),
            execution.stdout.len(),
            execution.stderr.len()
        );
        if let Some(guard) = report.output_log {
            let mut token = [0; 18];
            dryoc::rng::copy_randombytes(&mut token);
            let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
            for (stream, bytes) in [("stdout", &execution.stdout), ("stderr", &execution.stderr)] {
                println!(
                    "{}",
                    output_report(report.stage, stream, bytes, guard, &token)
                );
            }
        }
        // Exit 124 keeps its timeout handling even after a memory kill.
        if status.is_some_and(|status| status.code() == Some(124)) {
            execution.timed_out = true;
        } else if memory_limit {
            return Err(MEMORY_LIMIT);
        } else if status.is_some_and(|status| status.code() == Some(137)) {
            return Err(KILLED);
        }
        if !execution.timed_out && !status.is_some_and(|status| status.success()) {
            return Err(report.execution_failure);
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

/// The seat as the harness receives it.
#[derive(Clone, Copy)]
pub enum Credential<'a> {
    /// A setup token, passed only as the harness's own environment variable.
    Token(&'a str),
    /// A private directory with the Codex sign-in, mounted writable so Codex can refresh it.
    CodexHome(&'a Path),
    /// A private Pi agent directory with the Codex sign-in in Pi's format, mounted writable so
    /// Pi can refresh it and keep its lock directory beside it.
    PiHome(&'a Path),
}

struct Agent<'a> {
    credential: Credential<'a>,
    harness: &'a str,
    model: Option<&'a str>,
    writable: bool,
}

async fn agent(
    workspace: &Path,
    name: &str,
    settings: &Agent<'_>,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<Result<Execution>> {
    let writable = settings.writable;
    // Pi and Codex run writable golems only; the read-only v1 protocol stays on claude.
    if !matches!(
        (settings.harness, writable, settings.credential),
        ("claude", _, Credential::Token(_))
            | ("pi", true, Credential::Token(_) | Credential::PiHome(_))
            | ("codex", true, Credential::CodexHome(_))
    ) {
        return Ok(Err("unsupported harness"));
    }
    let mut create =
        match create_container(workspace, name, writable, writable, false, timeout_seconds) {
            Ok(create) => create,
            Err(error) => return Ok(Err(error)),
        };
    let configured = match (settings.harness, settings.credential) {
        ("pi", credential) => configure_pi(&mut create, credential, settings.model),
        (_, Credential::CodexHome(home)) => configure_codex(&mut create, home, settings.model),
        (_, Credential::Token(seat)) => {
            configure_agent(&mut create, seat, settings.model, writable);
            Ok(())
        }
        (_, Credential::PiHome(_)) => Err("unsupported harness"),
    };
    if let Err(error) = configured {
        return Ok(Err(error));
    }
    lifecycle(
        create,
        name,
        Some(prompt.as_bytes()),
        timeout_seconds,
        Report {
            stage: "agent",
            execution_failure: "harness container execution failed",
            output_log: None,
        },
    )
    .await
}

/// JSON events on stdout, the prompt on stdin, no session. The repository's own Pi setup
/// loads natively: `--approve` trusts the project for this process (`.pi/` settings,
/// extensions, skills, prompts, MCP, packages); context files load anyway. The golem-owned
/// report extension is loaded first, so a project tool with its name is a startup conflict
/// and Pi exits 1 before any event. Themes are interface-only and stay off.
/// An Anthropic seat is only `ANTHROPIC_OAUTH_TOKEN`; a Codex seat is only Pi's `auth.json`
/// in the private agent directory, never an environment value. `--provider` without
/// `--model` is a Pi startup error, so without a requested model Pi picks its default for
/// the only provider it has a sign-in for.
fn configure_pi(
    create: &mut Command,
    credential: Credential<'_>,
    model: Option<&str>,
) -> Result<()> {
    let provider = match credential {
        Credential::Token(seat) => {
            create
                .args(["--env", "ANTHROPIC_OAUTH_TOKEN"])
                .env("ANTHROPIC_OAUTH_TOKEN", seat);
            "anthropic"
        }
        Credential::PiHome(home) => {
            mount(create, home, PI_HOME, false)?;
            golem_runner::codex::PI_PROVIDER
        }
        Credential::CodexHome(_) => return Err("unsupported harness"),
    };
    create
        .args([
            "--env",
            "GOLEM_WRITABLE",
            "--env",
            "GOLEM_HARNESS",
            "--env",
            "GOLEM_PROVIDER",
        ])
        .env("GOLEM_WRITABLE", "1")
        .env("GOLEM_HARNESS", "pi")
        .env("GOLEM_PROVIDER", provider)
        .args([
            IMAGE,
            "--mode",
            "json",
            "--no-session",
            "--session-dir",
            "/tmp/golem-pi-sessions",
            "--approve",
            "--extension",
            "/opt/golem/pi-report.js",
            "--no-themes",
        ]);
    if let Some(model) = model {
        create.args(["--provider", provider, "--model", model]);
    }
    Ok(())
}

/// JSONL events on stdout, the prompt on stdin, the container as the only sandbox, no user
/// configuration (`CODEX_HOME` holds only the sign-in), sessions, history, web search or
/// telemetry. The repository's own `.codex/` layer, rules, hooks and AGENTS.md load
/// natively: `/workspace` is trusted for this run and hook review is bypassed, since
/// nobody can review hooks unattended. No seat value in argv or environment.
fn configure_codex(create: &mut Command, home: &Path, model: Option<&str>) -> Result<()> {
    mount(create, home, CODEX_HOME, false)?;
    create
        .args(["--env", "GOLEM_WRITABLE", "--env", "GOLEM_HARNESS"])
        .env("GOLEM_WRITABLE", "1")
        .env("GOLEM_HARNESS", "codex")
        .args([
            IMAGE,
            "exec",
            "--json",
            "--output-schema",
            CODEX_SCHEMA,
            "--skip-git-repo-check",
            "--dangerously-bypass-approvals-and-sandbox",
            "--ephemeral",
            "--ignore-user-config",
            "--dangerously-bypass-hook-trust",
            "--color",
            "never",
        ]);
    for setting in CODEX_SETTINGS {
        create.args(["--config", setting]);
    }
    if let Some(model) = model {
        create.args(["--model", model]);
    }
    create.arg("-");
    Ok(())
}

/// Checked against codex-rs 0.167.1 `config_toml.rs`; unknown keys would be ignored silently.
/// `--config` layers sit above project layers, so these few keys win over `.codex/config.toml`.
/// Trust is an inline table on purpose: `overrides.rs` splits dotted keys on every `.` without
/// honoring quotes, so `projects."/workspace".trust_level` would name a key with literal quotes
/// and never match. The loader merges `--config` before resolving trust (`loader/mod.rs`).
const CODEX_SETTINGS: [&str; 6] = [
    "projects={\"/workspace\"={trust_level=\"trusted\"}}",
    "web_search=\"disabled\"",
    "check_for_update_on_startup=false",
    "analytics.enabled=false",
    "feedback.enabled=false",
    "history.persistence=\"none\"",
];

fn configure_agent(create: &mut Command, seat: &str, model: Option<&str>, writable: bool) {
    let settings = if writable {
        writable_settings()
    } else {
        claude_settings()
    };
    create
        .args([
            "--env",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "--env",
            "GOLEM_WRITABLE",
            "--env",
            "GOLEM_HARNESS",
        ])
        .env("CLAUDE_CODE_OAUTH_TOKEN", seat)
        .env("GOLEM_WRITABLE", if writable { "1" } else { "0" })
        .env("GOLEM_HARNESS", "claude")
        .args([
            IMAGE,
            "--print",
            "--output-format",
            if writable { "stream-json" } else { "json" },
        ]);
    if writable {
        create.args([
            "--verbose",
            "--json-schema",
            golem_runner::WRITABLE_RESULT_SCHEMA,
        ]);
    }
    // Project and local settings, hooks, skills, commands, agents and `.mcp.json` load
    // natively; the user source is a fresh, empty directory anyway.
    create.args([
        "--no-session-persistence",
        "--setting-sources",
        "project,local",
    ]);
    if writable {
        // Nobody can answer a prompt: every tool, including project skills and MCP, runs
        // inside the container, limited only by deny rules.
        create.args(["--permission-mode", "bypassPermissions"]);
    } else {
        // The read-only reply protocol's own tools inspect files only; project hooks and
        // MCP servers still run natively, against a read-only workspace mount.
        create.args([
            "--tools",
            "Read,Glob,Grep",
            "--allowedTools",
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
    let settings = Agent {
        credential: Credential::Token(seat),
        harness: "claude",
        model,
        writable: false,
    };
    Ok(agent(workspace, name, &settings, prompt, timeout_seconds)
        .await?
        .and_then(|execution| {
            if execution.timed_out {
                Err("agent wall-clock budget exhausted")
            } else {
                Ok((execution.stdout, execution.stderr))
            }
        }))
}

/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation/execution failed safely. Timeout instead returns
/// bounded partial streams with `timed_out`, after confirmed destruction.
pub async fn run_writable(
    workspace: &Path,
    name: &str,
    credential: Credential<'_>,
    harness: &str,
    model: Option<&str>,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<Result<Execution>> {
    let settings = Agent {
        credential,
        harness,
        model,
        writable: true,
    };
    agent(workspace, name, &settings, prompt, timeout_seconds).await
}

/// The sibling `<workspace>.golem-tools` must live in the supervisor's private
/// temporary parent. Caller removes that parent only after destruction is confirmed.
/// Activates installed tool paths and Rust's selected toolchain only, never
/// arbitrary repository environment/hooks. Cargo caches use fresh container HOME.
///
/// # Errors
/// Outer error means destruction is unconfirmed; prohibit publication.
/// Inner error means validation, installation, or the ten-minute budget failed.
pub async fn prepare_target(
    workspace: &Path,
    name: &str,
    guard: &OutputGuard,
) -> Result<Result<()>> {
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
    Ok(lifecycle(
        create,
        name,
        None,
        TARGET_TIMEOUT_SECONDS,
        Report {
            stage: "target preparation",
            execution_failure: "target preparation container execution failed",
            output_log: Some(guard),
        },
    )
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
pub async fn verify(
    workspace: &Path,
    name: &str,
    command: Option<&str>,
    guard: &OutputGuard,
) -> Result<Result<()>> {
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
        Report {
            stage: "verification",
            execution_failure: "verification container execution failed",
            output_log: Some(guard),
        },
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
            assert_eq!(args.iter().any(|arg| *arg == "--json-schema"), writable);
            if writable {
                assert!(args.windows(2).any(|pair| pair[0] == "--json-schema"
                    && pair[1] == golem_runner::WRITABLE_RESULT_SCHEMA));
            }
        }
    }

    fn harness_args(command: &Command) -> Vec<String> {
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let image = args.iter().position(|arg| arg == IMAGE).unwrap();
        args[image + 1..].to_vec()
    }

    // Flags that kept repository harness configuration from loading; none may return.
    const DISABLING_FLAGS: [&str; 16] = [
        "--safe-mode",
        "--strict-mcp-config",
        "--mcp-config",
        "--disable-slash-commands",
        "--bare",
        "--offline",
        "--no-approve",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
        "--no-mcp",
        "--append-system-prompt",
        "--ignore-rules",
        "project_doc_max_bytes=0",
        "--disable-hooks",
    ];

    #[test]
    fn claude_loads_project_configuration_and_keeps_the_result_channel() {
        for writable in [true, false] {
            let mut command = docker();
            configure_agent(
                &mut command,
                "sentinel",
                Some("claude-sonnet-5-5"),
                writable,
            );
            let harness = harness_args(&command);
            let pair = |flag: &str, value: &str| {
                harness
                    .windows(2)
                    .any(|pair| pair[0] == flag && pair[1] == value)
            };
            for flag in DISABLING_FLAGS {
                assert!(!harness.iter().any(|arg| arg == flag), "{flag}");
            }
            assert!(pair("--setting-sources", "project,local"));
            assert!(pair("--max-turns", "32"));
            assert!(pair("--model", "claude-sonnet-5-5"));
            for flag in ["--print", "--no-session-persistence", "--settings"] {
                assert!(harness.iter().any(|arg| arg == flag), "{flag}");
            }
            let settings =
                harness[harness.iter().position(|arg| arg == "--settings").unwrap() + 1].clone();
            assert!(!settings.contains("Hooks"), "project hooks stay enabled");
            assert_eq!(
                pair("--permission-mode", "bypassPermissions"),
                writable,
                "unattended writable runs never wait for a prompt"
            );
            assert_eq!(
                harness.iter().any(|arg| arg == "--tools"),
                !writable,
                "writable runs keep every tool, including project skills and MCP"
            );
            assert!(
                !harness.iter().any(|arg| arg == "sentinel"),
                "seat never in argv"
            );
        }
    }

    #[test]
    fn pi_runs_json_events_with_project_trust_the_report_extension_and_only_its_seat() {
        let home = std::env::temp_dir().join("golem-pi-fixture");
        for (credential, provider, model) in [
            (Credential::Token("sentinel"), "anthropic", None),
            (
                Credential::Token("sentinel"),
                "anthropic",
                Some("claude-sonnet-5-5"),
            ),
            (Credential::PiHome(&home), "openai-codex", None),
            (
                Credential::PiHome(&home),
                "openai-codex",
                Some("gpt-6.1-sol"),
            ),
        ] {
            let mut command = docker();
            configure_pi(&mut command, credential, model).unwrap();
            let command = command.as_std();
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let image = args.iter().position(|arg| arg == IMAGE).unwrap();
            let mounts: Vec<_> = args[..image]
                .windows(2)
                .filter(|pair| pair[0] == "--mount")
                .map(|pair| pair[1].clone())
                .collect();
            let seated = matches!(credential, Credential::PiHome(_));
            if seated {
                assert_eq!(
                    mounts,
                    [format!(
                        "type=bind,source={},target={PI_HOME},bind-propagation=rprivate",
                        home.display()
                    )],
                    "writable, outside /workspace"
                );
            } else {
                assert_eq!(mounts, [] as [String; 0]);
            }
            let harness = &args[image + 1..];
            let pair = |flag: &str, value: &str| {
                harness
                    .windows(2)
                    .any(|pair| pair[0] == flag && pair[1] == value)
            };
            assert!(pair("--mode", "json"));
            assert!(pair("--extension", "/opt/golem/pi-report.js"));
            assert!(pair("--session-dir", "/tmp/golem-pi-sessions"));
            for flag in ["--no-session", "--approve", "--no-themes"] {
                assert!(harness.iter().any(|arg| arg == flag), "{flag}");
            }
            for flag in DISABLING_FLAGS.iter().chain(&["--tools"]) {
                assert!(!harness.iter().any(|arg| arg == flag), "{flag}");
            }
            assert_eq!(
                harness.iter().filter(|arg| *arg == "--extension").count(),
                1
            );
            assert_eq!(
                model.is_some(),
                pair("--provider", provider) && pair("--model", model.unwrap_or_default())
            );
            assert_eq!(
                harness.iter().any(|arg| arg == "--provider"),
                model.is_some(),
                "Pi rejects --provider without --model"
            );
            assert!(
                !args.iter().any(|arg| arg == "sentinel"),
                "seat never in argv"
            );
            let env: Vec<_> = command
                .get_envs()
                .filter_map(|(key, value)| Some((key.to_str()?, value?.to_str()?)))
                .collect();
            assert_eq!(
                env.contains(&("ANTHROPIC_OAUTH_TOKEN", "sentinel")),
                !seated
            );
            assert_eq!(
                args.iter().any(|arg| arg == "ANTHROPIC_OAUTH_TOKEN"),
                !seated,
                "a Codex seat never forwards an Anthropic token"
            );
            assert!(env.contains(&("GOLEM_PROVIDER", provider)));
            assert!(env.contains(&("GOLEM_HARNESS", "pi")));
            assert!(env.contains(&("GOLEM_WRITABLE", "1")));
            assert!(!env.iter().any(|(key, _)| *key == "CLAUDE_CODE_OAUTH_TOKEN"));
            if seated {
                assert!(env.iter().all(|(key, _)| !key.contains("TOKEN")));
            }
        }
        let mut command = docker();
        assert!(
            configure_pi(&mut command, Credential::CodexHome(&home), None).is_err(),
            "Pi never reads Codex's own format"
        );
    }

    #[tokio::test]
    async fn pi_is_writable_only_and_unknown_harnesses_never_start_a_container() {
        let home = Path::new("/unused-home");
        for (harness, writable, credential) in [
            ("pi", false, Credential::Token("sentinel")),
            ("codex", true, Credential::Token("sentinel")),
            ("codex", false, Credential::CodexHome(home)),
            ("claude", true, Credential::CodexHome(home)),
            ("pi", true, Credential::CodexHome(home)),
            ("pi", false, Credential::PiHome(home)),
            ("codex", true, Credential::PiHome(home)),
            ("claude", true, Credential::PiHome(home)),
            ("claude", false, Credential::PiHome(home)),
            ("", true, Credential::Token("sentinel")),
        ] {
            let settings = Agent {
                credential,
                harness,
                model: None,
                writable,
            };
            let result = agent(Path::new("/unused"), "golem-test", &settings, "prompt", 60).await;
            assert!(
                matches!(result, Ok(Err("unsupported harness"))),
                "{harness}"
            );
        }
    }

    #[test]
    fn harness_entrypoint_is_absolute_even_with_target_tool_paths() {
        let entrypoint = include_str!("../container.cjs");
        assert!(entrypoint.contains("harnessEntry('/opt/golem/claude-entry'"));
        assert!(entrypoint.contains("harnessEntry('/opt/golem/pi-entry'"));
        assert!(entrypoint.contains("javascript ? '/usr/local/bin/node' : entry"));
        let dockerfile = include_str!("../agent.Dockerfile");
        assert!(dockerfile.contains("COPY pi-report.js /opt/golem/pi-report.js"));
        assert!(dockerfile.contains("/opt/golem/pi-entry"));
        assert!(entrypoint.contains("harnessEntry('/opt/golem/codex-entry'"));
        assert!(entrypoint.contains(&format!("CODEX_HOME: '{CODEX_HOME}'")));
        assert!(entrypoint.contains(&format!("'{PI_HOME}'")));
        assert!(dockerfile.contains(&format!("{CODEX_HOME} {PI_HOME} ")));
        assert!(dockerfile.contains(&format!("{PI_HOME}/auth.json")));
        assert!(dockerfile.contains(&format!("COPY codex-result-schema.json {CODEX_SCHEMA}")));
        assert!(dockerfile.contains("/opt/golem/codex-entry"));
        assert!(
            include_str!("../agent.mise.toml").contains("\"npm:@openai/codex\" = \"0.167.1\""),
            "codex pinned exactly"
        );
    }

    #[test]
    fn codex_runs_json_exec_with_its_private_home_and_no_seat_in_argv_or_environment() {
        for model in [None, Some("gpt-5.5")] {
            let mut command = docker();
            let home = std::env::temp_dir().join("golem-seat-fixture");
            configure_codex(&mut command, &home, model).unwrap();
            let command = command.as_std();
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let image = args.iter().position(|arg| arg == IMAGE).unwrap();
            let mounts: Vec<_> = args[..image]
                .windows(2)
                .filter(|pair| pair[0] == "--mount")
                .map(|pair| pair[1].clone())
                .collect();
            assert_eq!(
                mounts,
                [format!(
                    "type=bind,source={},target={CODEX_HOME},bind-propagation=rprivate",
                    home.display()
                )],
                "writable, outside /workspace"
            );
            let harness = &args[image + 1..];
            assert_eq!(harness[0], "exec");
            assert_eq!(harness.last().unwrap(), "-", "prompt on stdin");
            let pair = |flag: &str, value: &str| {
                harness
                    .windows(2)
                    .any(|pair| pair[0] == flag && pair[1] == value)
            };
            assert!(pair("--output-schema", CODEX_SCHEMA));
            assert!(pair("--color", "never"));
            for flag in [
                "--json",
                "--skip-git-repo-check",
                "--dangerously-bypass-approvals-and-sandbox",
                "--ephemeral",
                "--ignore-user-config",
                "--dangerously-bypass-hook-trust",
            ] {
                assert!(harness.iter().any(|arg| arg == flag), "{flag}");
            }
            for flag in DISABLING_FLAGS {
                assert!(!harness.iter().any(|arg| arg == flag), "{flag}");
            }
            for setting in CODEX_SETTINGS {
                assert!(pair("--config", setting), "{setting}");
            }
            // An inline table: a quoted dotted key would keep its quotes and never match.
            assert!(pair(
                "--config",
                r#"projects={"/workspace"={trust_level="trusted"}}"#
            ));
            assert!(
                !harness
                    .iter()
                    .any(|arg| arg.starts_with("projects.") || arg.contains("project_doc")),
                "project documentation and trust are not overridden away"
            );
            assert_eq!(model.is_some(), pair("--model", model.unwrap_or_default()));
            assert!(!harness.iter().any(|arg| arg == "--model") || model.is_some());
            let env: Vec<_> = command
                .get_envs()
                .filter_map(|(key, value)| Some((key.to_str()?, value?.to_str()?)))
                .collect();
            assert!(env.contains(&("GOLEM_HARNESS", "codex")));
            assert!(env.contains(&("GOLEM_WRITABLE", "1")));
            assert!(
                env.iter()
                    .all(|(key, _)| !key.contains("TOKEN") && !key.contains("CODEX"))
            );
        }
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../codex-result-schema.json")).unwrap();
        assert_eq!(
            schema["required"],
            serde_json::json!(["disposition", "message"])
        );
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["disposition"]["enum"],
            serde_json::json!(["completed", "blocked"])
        );
        assert_eq!(schema["properties"].as_object().unwrap().len(), 2);
        assert!(
            !include_str!("../codex-result-schema.json").contains("Length"),
            "strict mode"
        );
    }

    #[test]
    fn image_entrypoint_uses_a_trusted_timeout_before_claude() {
        let dockerfile = include_str!("../agent.Dockerfile");
        assert!(dockerfile.contains("ENTRYPOINT [\"/usr/local/bin/golem-timeout\"]"));
        assert!(dockerfile.contains("exec /usr/bin/timeout --signal=KILL"));
    }

    #[test]
    fn golem_settings_only_deny_and_never_disable_project_hooks() {
        for settings in [writable_settings(), claude_settings()] {
            let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
            assert_eq!(settings.as_object().unwrap().len(), 1, "{settings}");
            let deny = settings["permissions"]["deny"].as_array().unwrap();
            for tool in ["WebFetch", "WebSearch"] {
                assert!(deny.iter().any(|rule| rule == tool));
            }
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
            (
                "start-failure",
                "echo id",
                "printf secret-output; printf secret-diagnostic >&2; exit 1",
                "echo \"$3\"",
            ),
            ("start-remove-failure", "echo id", "exit 1", "exit 1"),
            (
                "start-remove-mismatch",
                "echo id",
                "exit 1",
                "echo wrong-name",
            ),
            (
                "killed",
                "echo id",
                "printf partial; printf diagnostic >&2; exit 137",
                "echo \"$3\"",
            ),
            ("memory-limit", "echo id", "exit 1", "echo \"$3\""),
            (
                "timeout-after-memory-limit",
                "echo id",
                "printf partial; exit 124",
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
            // Only these scenarios have Docker record an out-of-memory kill.
            let inspect = if scenario.contains("memory-limit") {
                "echo true"
            } else {
                "echo false"
            };
            std::fs::write(&executable, format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\ncase \"$1\" in\ncreate) {create};;\nstart) {start};;\ninspect) {inspect};;\nrm) {remove};;\n*) exit 98;;\nesac\n", log.display())).unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            for phase in ["lifecycle", "prepare", "v1", "writable", "verify"] {
                // Only the direct lifecycle probe uses a one-second host deadline.
                if phase != "lifecycle" && matches!(scenario, "timeout" | "create-timeout") {
                    continue;
                }
                std::fs::write(&log, "").unwrap();
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
                    .env("GOLEM_TEST_PHASE", phase)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{phase}/{scenario}: {}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                let calls = std::fs::read_to_string(&log).unwrap();
                // Only a container that exited unsuccessfully is asked about memory kills.
                let failed = matches!(
                    scenario,
                    "start-failure"
                        | "start-remove-failure"
                        | "start-remove-mismatch"
                        | "killed"
                        | "memory-limit"
                        | "timeout-after-memory-limit"
                );
                assert_eq!(
                    calls,
                    if scenario.starts_with("create-") {
                        "create\nrm\n"
                    } else if failed {
                        "create\nstart\ninspect\nrm\n"
                    } else {
                        "create\nstart\nrm\n"
                    },
                    "{phase}/{scenario}"
                );
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "invoked by lifecycle_requires_removal_acknowledgment_after_every_attempt"]
    async fn lifecycle_probe() {
        let scenario = std::env::var("GOLEM_TEST_SCENARIO").unwrap();
        let phase = std::env::var("GOLEM_TEST_PHASE").unwrap();
        if phase != "lifecycle" {
            production_lifecycle_probe(&phase, &scenario).await;
            return;
        }
        let mut create = docker();
        create.arg("create");
        let result = lifecycle(
            create,
            "golem-test",
            None,
            1,
            Report {
                stage: "agent",
                execution_failure: "harness container execution failed",
                output_log: None,
            },
        )
        .await;
        match scenario.as_str() {
            "remove-failure" | "start-remove-failure" => assert!(matches!(
                result,
                Err("container destruction unconfirmed; publication prohibited")
            )),
            "remove-mismatch" | "start-remove-mismatch" => assert!(matches!(
                result,
                Err("container destruction acknowledgment mismatch; publication prohibited")
            )),
            "create-failure" => assert!(matches!(result, Ok(Err("Docker command failed")))),
            "start-failure" => {
                assert!(matches!(
                    result,
                    Ok(Err("harness container execution failed"))
                ));
            }
            "killed" => assert!(matches!(result, Ok(Err(KILLED)))),
            "memory-limit" => assert!(matches!(result, Ok(Err(MEMORY_LIMIT)))),
            "timeout-after-memory-limit" => {
                let execution = result.unwrap().unwrap();
                assert!(execution.timed_out);
                assert_eq!(execution.stdout, b"partial");
            }
            "create-timeout" => assert!(matches!(
                result,
                Ok(Err("container wall-clock budget exhausted"))
            )),
            "timeout" => {
                let execution = result.unwrap().unwrap();
                assert!(execution.timed_out);
                assert_eq!(execution.stdout, b"partial");
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

    #[cfg(unix)]
    async fn production_lifecycle_probe(phase: &str, scenario: &str) {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        if matches!(phase, "writable" | "verify") {
            std::fs::create_dir(toolchain_path(&workspace).unwrap()).unwrap();
        }
        let result = match phase {
            "prepare" => prepare_target(&workspace, "golem-test", &OutputGuard::default()).await,
            "v1" => run(&workspace, "golem-test", "secret-seat", None, "", 60)
                .await
                .map(|result| result.map(|_| ())),
            "writable" => run_writable(
                &workspace,
                "golem-test",
                Credential::Token("secret-seat"),
                "claude",
                None,
                "",
                60,
            )
            .await
            .map(|result| {
                result.map(|execution| {
                    assert_eq!(
                        execution.timed_out,
                        scenario == "timeout-after-memory-limit"
                    );
                })
            }),
            "verify" => verify(&workspace, "golem-test", Some(""), &OutputGuard::default()).await,
            _ => unreachable!(),
        };
        let expected = match scenario {
            "remove-failure" | "start-remove-failure" => {
                Err("container destruction unconfirmed; publication prohibited")
            }
            "remove-mismatch" | "start-remove-mismatch" => {
                Err("container destruction acknowledgment mismatch; publication prohibited")
            }
            "create-failure" => Ok(Err("Docker command failed")),
            "start-failure" => Ok(Err(match phase {
                "prepare" => "target preparation container execution failed",
                "v1" | "writable" => "harness container execution failed",
                "verify" => "verification container execution failed",
                _ => unreachable!(),
            })),
            // Never partial agent work or a budget timeout.
            "killed" => Ok(Err(KILLED)),
            "memory-limit" => Ok(Err(MEMORY_LIMIT)),
            "timeout-after-memory-limit" => match phase {
                "prepare" => Ok(Err("target preparation timed out")),
                "v1" => Ok(Err("agent wall-clock budget exhausted")),
                "writable" => Ok(Ok(())),
                "verify" => Ok(Err("verification timed out")),
                _ => unreachable!(),
            },
            "success" => Ok(Ok(())),
            _ => unreachable!(),
        };
        assert_eq!(result, expected, "{phase}/{scenario}");
    }

    #[tokio::test]
    async fn invalid_timeout_is_safe_inner_error_without_container_creation() {
        let result = run_writable(
            Path::new("/unused"),
            "golem-test",
            Credential::Token("sentinel"),
            "claude",
            None,
            "prompt",
            0,
        )
        .await;
        assert!(matches!(result, Ok(Err("invalid sandbox timeout"))));
    }

    #[test]
    fn output_report_bounds_sanitizes_and_fences_target_output() {
        let guard = OutputGuard::default();
        let bytes: Vec<u8> = (0..300)
            .flat_map(|line| format!("line {line}\n").into_bytes())
            .chain(b"::error::spoof\x1b[2Jend\r\n".iter().copied())
            .collect();
        let report = output_report("verification", "stdout", &bytes, &guard, "tok");
        let lines: Vec<_> = report.lines().collect();
        assert_eq!(lines[0], "golem verification stdout tail (200 lines):");
        assert_eq!(lines[1], "::stop-commands::tok");
        assert_eq!(lines[2], "line 101");
        assert_eq!(lines[201], "::error::spoof\u{fffd}[2Jend");
        assert_eq!(lines[202], "::tok::");
        assert_eq!(lines.len(), 203);
        assert_eq!(
            output_report("verification", "stderr", b"", &guard, "tok"),
            "golem verification stderr: empty"
        );
    }

    #[test]
    fn output_report_keeps_only_whole_lines_within_byte_bound() {
        let mut bytes = vec![b'x'; TAIL_BYTES];
        bytes.extend_from_slice(b"\nlast\n");
        let report = output_report(
            "verification",
            "stdout",
            &bytes,
            &OutputGuard::default(),
            "t",
        );
        assert_eq!(
            report,
            "golem verification stdout tail (1 lines):\n::stop-commands::t\nlast\n::t::"
        );
    }

    #[test]
    fn output_report_keeps_a_cut_line_when_no_whole_line_exists() {
        let bytes = vec![b'x'; TAIL_BYTES + 100];
        let report = output_report(
            "verification",
            "stdout",
            &bytes,
            &OutputGuard::default(),
            "t",
        );
        assert_eq!(
            report,
            format!(
                "golem verification stdout tail (1 lines):\n::stop-commands::t\n{}\n::t::",
                "x".repeat(TAIL_BYTES)
            )
        );
    }

    #[test]
    fn output_report_withholds_guarded_values_in_any_encoding() {
        let mut guard = OutputGuard::default();
        guard.add("sentinel-credential-value");
        let utf16: Vec<u8> = "sentinel-credential-value"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        for bytes in [b"ok\nsentinel-credential-value\n".to_vec(), utf16] {
            let report = output_report("target preparation", "stderr", &bytes, &guard, "t");
            assert_eq!(
                report,
                "golem target preparation stderr tail withheld: matches a guarded credential"
            );
        }
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
