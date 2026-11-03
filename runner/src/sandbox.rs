use golem_runner::{OUTPUT_BYTES_MAX, Result};
use std::{ffi::OsStr, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const IMAGE: &str = "isp-golem-xiv-agent:v0";
const FORBIDDEN_READ_ROOTS: [&str; 6] = ["proc", "sys", "dev", "home", "tmp", "run"];

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

async fn read_bounded<R: AsyncRead + Unpin>(reader: R) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((OUTPUT_BYTES_MAX + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "cannot read process output")?;
    if bytes.len() > OUTPUT_BYTES_MAX {
        return Err("process output exceeds limit");
    }
    Ok(bytes)
}

async fn execute(
    mut command: Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<(Vec<u8>, Vec<u8>)> {
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
    let operation = async {
        let send = async {
            if let Some(mut stdin) = stdin {
                if let Some(input) = input {
                    stdin
                        .write_all(input)
                        .await
                        .map_err(|_| "cannot send agent prompt")?;
                }
                stdin
                    .shutdown()
                    .await
                    .map_err(|_| "cannot finish agent prompt")?;
            }
            Ok::<(), &'static str>(())
        };
        let (stdout, stderr, (), status) =
            tokio::try_join!(read_bounded(stdout), read_bounded(stderr), send, async {
                child.wait().await.map_err(|_| "cannot wait for Docker")
            })?;
        if !status.success() {
            return Err("Docker command failed");
        }
        Ok((stdout, stderr))
    };
    let result = tokio::time::timeout(timeout, operation)
        .await
        .map_err(|_| "agent wall-clock budget exhausted")?;
    // kill_on_drop also covers output floods and interrupted writes.
    result
}

pub async fn prepare(mise: &Path) -> Result<()> {
    let context = tempfile::tempdir().map_err(|_| "cannot create image build context")?;
    std::fs::copy(mise, context.path().join("mise"))
        .map_err(|_| "cannot stage pinned mise executable")?;
    std::fs::write(
        context.path().join("Dockerfile"),
        include_str!("../agent.Dockerfile"),
    )
    .map_err(|_| "cannot stage trusted Dockerfile")?;
    std::fs::write(
        context.path().join("agent.mise.toml"),
        include_str!("../agent.mise.toml"),
    )
    .map_err(|_| "cannot stage trusted tool pins")?;
    let mut command = docker();
    command
        .args(["build", "--pull", "--tag", IMAGE])
        .arg(context.path());
    execute(command, None, Duration::from_secs(600)).await?;
    Ok(())
}

/// Always attempt removal, even if create/start failed ambiguously.
/// A successful `rm --force` acknowledgment is required before publication.
pub async fn run(
    workspace: &Path,
    name: &str,
    seat: &str,
    model: Option<&str>,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<Result<(Vec<u8>, Vec<u8>)>> {
    let settings = claude_settings();
    let mut create = docker();
    create.args([
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
        "--user=1000:1000",
        "--tmpfs",
        "/tmp:rw,nosuid,nodev,noexec,size=64m,mode=1777",
        "--tmpfs",
        "/home/agent:rw,nosuid,nodev,noexec,size=128m,uid=1000,gid=1000,mode=700",
        "--mount",
    ]);
    let workspace = workspace.to_str().ok_or("workspace path is not UTF-8")?;
    if workspace.contains([',', '\n', '\r']) {
        return Err("invalid workspace mount path");
    }
    create.arg(format!(
        "type=bind,source={workspace},target=/workspace,readonly,bind-propagation=rprivate"
    ));
    // Pass only the selected subscription; never inherit host/App/GitHub environment.
    create
        .args([
            "--env",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "--env",
            "GOLEM_TIMEOUT_SECONDS",
            IMAGE,
            "--safe-mode",
            "--print",
            "--output-format",
            "json",
            "--no-session-persistence",
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--disable-slash-commands",
            "--tools",
            "Read,Glob,Grep",
            "--allowedTools",
            "Read(//workspace/**)",
            "Glob(//workspace/**)",
            "Grep(//workspace/**)",
            "--settings",
            &settings,
            "--max-turns",
            "32",
        ])
        .env("CLAUDE_CODE_OAUTH_TOKEN", seat)
        // Image entrypoint uses this trusted watchdog, independent of agent cooperation.
        .env("GOLEM_TIMEOUT_SECONDS", timeout_seconds.to_string());
    if let Some(model) = model {
        create.args(["--model", model]);
    }
    let created = execute(create, None, Duration::from_secs(60)).await;
    let result = if created.is_ok() {
        let mut start = docker();
        start.args(["start", "--attach", "--interactive", name]);
        execute(
            start,
            Some(prompt.as_bytes()),
            Duration::from_secs(timeout_seconds),
        )
        .await
    } else {
        Err("container creation failed")
    };
    let mut remove = docker();
    remove.args([OsStr::new("rm"), OsStr::new("--force"), OsStr::new(name)]);
    // A timeout or failure here is fatal: no publication, including failure summaries.
    let (acknowledgment, _) = execute(remove, None, Duration::from_secs(30))
        .await
        .map_err(|_| "container destruction unconfirmed; publication prohibited")?;
    if std::str::from_utf8(&acknowledgment).map(str::trim) != Ok(name) {
        return Err("container destruction acknowledgment mismatch; publication prohibited");
    }
    Ok(result)
}

#[cfg(test)]
// Test assertions deliberately panic on malformed fixtures.
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
    fn image_entrypoint_uses_a_trusted_timeout_before_claude() {
        // This protects the budget even when Claude ignores cancellation or hangs.
        let dockerfile = include_str!("../agent.Dockerfile");
        assert!(dockerfile.contains("ENTRYPOINT [\"/usr/local/bin/golem-timeout\"]"));
        assert!(dockerfile.contains("exec /usr/bin/timeout --signal=KILL"));
    }

    #[tokio::test]
    async fn process_output_limit_rejects_flood_without_retaining_unbounded_bytes() {
        assert_eq!(read_bounded(&b"normal"[..]).await.unwrap(), b"normal");
        let flood = vec![b'x'; OUTPUT_BYTES_MAX + 1];
        assert!(read_bounded(flood.as_slice()).await.is_err());
    }
}
