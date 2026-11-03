#![forbid(unsafe_code)]

mod github;
mod sandbox;
mod snapshot;
#[cfg(test)]
mod tests;

use golem_runner::{AgentOutput, DispatchV1, OutputGuard, Result, agent_result};
use std::{env, fs::OpenOptions, io::Write, path::Path, process::ExitCode};

fn environment(name: &str, maximum: usize) -> Result<String> {
    let value = env::var(name).map_err(|_| "required runner environment is missing")?;
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err("invalid runner environment");
    }
    Ok(value)
}

fn dispatch() -> Result<DispatchV1> {
    DispatchV1::parse(
        &environment("GOLEM_REQUEST", golem_runner::REQUEST_BYTES_MAX)?,
        &environment("GOLEM_REPO", 160)?,
        &environment("GOLEM_NAME", 64)?,
    )
}

fn numeric_environment(name: &str) -> Result<String> {
    let value = environment(name, 20)?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) || value.bytes().all(|byte| byte == b'0') {
        return Err("invalid Actions run identity");
    }
    Ok(value)
}

fn selected_seat(request: &DispatchV1) -> Result<String> {
    let seat = environment("GOLEM_SEAT", 4096)?;
    let seat = seat.trim();
    if !seat.starts_with("sk-ant-oat01-")
        || seat.len() < 32
        || seat.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err("selected seat is not a Claude subscription setup token");
    }
    if environment("GOLEM_SEAT_SECRET_NAME", 100)? != request.seat_secret_name() {
        return Err("selected seat mapping mismatch");
    }
    Ok(seat.to_owned())
}

async fn run() -> Result<()> {
    let arguments: Vec<String> = env::args().skip(1).take(3).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--prepare")
        && arguments.len() == 2
    {
        return sandbox::prepare(Path::new(
            arguments.get(1).ok_or("mise executable missing")?,
        ))
        .await;
    }
    let request = dispatch()?;
    if arguments.as_slice() == ["--validate"] {
        let output = environment("GITHUB_OUTPUT", 4096)?;
        let mut file = OpenOptions::new()
            .append(true)
            .open(output)
            .map_err(|_| "cannot write workflow validation outputs")?;
        writeln!(file, "seat_secret={}", request.seat_secret_name())
            .map_err(|_| "cannot write workflow validation outputs")?;
        return Ok(());
    }
    if !arguments.is_empty() {
        return Err("unsupported runner arguments");
    }
    let run_id = numeric_environment("GITHUB_RUN_ID")?;
    let attempt = numeric_environment("GITHUB_RUN_ATTEMPT")?;
    golem_runner::initial_workflow_attempt(&attempt)?;
    let private_key = environment("GOLEM_APP_PRIVATE_KEY", 32_000)?;
    let seat = selected_seat(&request);
    let mut guard = OutputGuard::default();
    guard.add(&private_key);
    if let Ok(seat) = &seat {
        guard.add(seat);
    }
    let github = github::Github::new()?;
    let token = github
        .token(&private_key, &request, false, &mut guard)
        .await?;
    github.validate_target(&request, &token).await?;
    let model = request.model.as_deref().unwrap_or("default");
    let started = github
        .check(&request, &token, "running", model, &run_id, &guard)
        .await;
    let name = format!("golem-{run_id}-{attempt}");
    let result = match started.and(seat) {
        Err(error) => Err(error),
        Ok(seat) if env::var("GOLEM_PREPARATION").as_deref() == Ok("success") => {
            execute_agent(&github, &request, &token, &seat, &name, &guard).await?
        }
        Ok(_) => Err("trusted harness preparation failed"),
    };
    drop(token);
    let token = github
        .token(&private_key, &request, true, &mut guard)
        .await?;
    publish_result(&github, &request, &token, &run_id, result, &guard).await
}

async fn publish_result(
    github: &github::Github,
    request: &DispatchV1,
    token: &str,
    run_id: &str,
    result: Result<AgentOutput>,
    guard: &OutputGuard,
) -> Result<()> {
    // Include newly minted credentials in the final scan as well.
    let result = result.and_then(|output| {
        guard.check(&output.text)?;
        guard.check(&output.model)?;
        Ok(output)
    });
    match result {
        Ok(output) => {
            if let Err(error) = github.reply(request, token, &output.text, guard).await {
                github
                    .check(request, token, "blocked", &output.model, run_id, guard)
                    .await?;
                return Err(error);
            }
            github
                .check(request, token, "replied", &output.model, run_id, guard)
                .await?;
            println!("golem completed: replied");
            Ok(())
        }
        Err(error) => {
            let outcome = if error == "agent wall-clock budget exhausted" {
                "incomplete"
            } else {
                "blocked"
            };
            // Attempt both independently: unavailable checks must not suppress requester feedback.
            let model = request.model.as_deref().unwrap_or("default");
            let check = github
                .check(request, token, outcome, model, run_id, guard)
                .await;
            // Only static, trusted errors; raw agent output and diagnostics are never logged.
            let reply = github
                .reply(
                    request,
                    token,
                    &format!("Run {outcome}: {error}. Inspect the private Actions run for status."),
                    guard,
                )
                .await;
            check?;
            reply?;
            Err(error)
        }
    }
}

/// Outer errors prohibit publication because container destruction is unconfirmed.
/// Pre-sandbox failures are safe to report after target identity has been established.
async fn execute_agent(
    github: &github::Github,
    request: &DispatchV1,
    token: &str,
    seat: &str,
    name: &str,
    guard: &OutputGuard,
) -> Result<Result<AgentOutput>> {
    let preparation = async {
        let archive = github.snapshot(request, token).await?;
        let temporary =
            tempfile::tempdir().map_err(|_| "cannot create private runner workspace")?;
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir(&workspace).map_err(|_| "cannot create snapshot workspace")?;
        snapshot::extract(&archive, &workspace)?;
        let prompt = snapshot::prompt(&workspace, request)?;
        guard.check(&prompt)?;
        Ok::<_, &'static str>((temporary, workspace, prompt))
    }
    .await;
    let (temporary, workspace, prompt) = match preparation {
        Ok(prepared) => prepared,
        Err(error) => return Ok(Err(error)),
    };
    let execution = sandbox::run(
        &workspace,
        name,
        seat,
        request.model.as_deref(),
        &prompt,
        request.timeout_seconds,
    )
    .await?;
    temporary
        .close()
        .map_err(|_| "cannot remove private runner workspace; publication blocked")?;
    Ok(match execution {
        Ok((stdout, stderr)) => agent_result(&stdout, &stderr, guard),
        Err(error) => Err(error),
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("golem runner failed: {error}");
            ExitCode::FAILURE
        }
    }
}
