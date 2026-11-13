#![forbid(unsafe_code)]

mod github;
mod sandbox;
mod snapshot;
#[cfg(test)]
mod tests;

use golem_runner::{
    AgentOutput, DispatchV1, OutputGuard, Result, agent_result,
    codex::{self, CodexAuth, Refresh},
    seat::Seat,
};
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

fn selected_seat(request: &DispatchV1) -> Result<Seat> {
    let seat = Seat::parse(
        &request.provider,
        &environment("GOLEM_SEAT", codex::AUTH_BYTES_MAX)?,
    )?;
    if environment("GOLEM_SEAT_SECRET_NAME", 100)? != request.seat_secret_name() {
        return Err("selected seat mapping mismatch");
    }
    Ok(seat)
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
        seat.guard(&mut guard);
    }
    let github = github::Github::new()?;
    let token = github
        .token(&private_key, &request, false, &mut guard)
        .await?;
    github.validate_target(&request, &token).await?;
    if request.version == 2 {
        return RunContext {
            github: &github,
            request: &request,
            private_key: &private_key,
            run_id: &run_id,
            attempt: &attempt,
        }
        .run_writable(token, seat, &mut guard)
        .await;
    }
    let model = request.model.as_deref().unwrap_or("default");
    let started = github
        .check(&request, &token, "running", model, &run_id, &guard)
        .await;
    let name = format!("golem-{run_id}-{attempt}");
    let result = match started.and(seat) {
        Err(error) => Err(error),
        Ok(Seat::Codex(_)) => Err("read-only runs need a setup token seat"),
        Ok(Seat::Token(seat)) if env::var("GOLEM_PREPARATION").as_deref() == Ok("success") => {
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

fn verified_changes(
    workspace: &Path,
    before: &golem_runner::changes::Files,
    after_agent: &golem_runner::changes::Files,
    guard: &OutputGuard,
) -> Result<Vec<golem_runner::changes::Change>> {
    let after_verify = golem_runner::changes::scan_after(workspace, before)?;
    if after_agent
        .iter()
        .any(|(path, file)| after_verify.get(path) != Some(file))
    {
        return Err("verification mutated workspace; publication blocked");
    }
    // Generated regular outputs are disposable, never part of the frozen publication.
    // Still scan every output and reject forbidden paths, links, floods and credentials.
    golem_runner::changes::diff(after_agent, &after_verify, guard)?;
    golem_runner::changes::diff(before, after_agent, guard)
}

/// What a Codex run left in its private `auth.json`, read after confirmed destruction.
type SeatReadback = Option<Result<String>>;

/// Private parent directory, workspace, base commit, prompt and baseline files.
type Prepared = (
    tempfile::TempDir,
    std::path::PathBuf,
    String,
    String,
    golem_runner::changes::Files,
);

struct WritableResult {
    base_sha: String,
    changes: Result<Vec<golem_runner::changes::Change>>,
    output: AgentOutput,
    timed_out: bool,
}

// The sole transition into privileged publication consumes confirmed destruction.
async fn after_destruction<T>(
    execution: impl std::future::Future<Output = Result<T>>,
    publish: impl AsyncFnOnce(T) -> Result<()>,
) -> Result<()> {
    let result = execution.await?;
    publish(result).await
}

struct RunContext<'a> {
    github: &'a github::Github,
    request: &'a DispatchV1,
    private_key: &'a str,
    run_id: &'a str,
    attempt: &'a str,
}

impl RunContext<'_> {
    async fn run_writable(
        &self,
        token: String,
        seat: Result<Seat>,
        guard: &mut OutputGuard,
    ) -> Result<()> {
        github::queue(
            self.request,
            self.private_key,
            self.run_id,
            self.attempt,
            "bind",
        )
        .await?;
        println!("golem lifecycle: admitted");
        let name = format!("golem-{}-{}", self.run_id, self.attempt);
        let started = self
            .github
            .check(
                self.request,
                &token,
                "running",
                self.request.model.as_deref().unwrap_or("default"),
                self.run_id,
                guard,
            )
            .await;
        let execution = async {
            let result = match started.and(seat.as_ref().map_err(|error| *error)) {
                Err(error) => Ok((Err(error), None)),
                Ok(seat) if env::var("GOLEM_PREPARATION").as_deref() == Ok("success") => {
                    self.execute_writable(&token, seat, &name, guard).await
                }
                Ok(_) => Ok((Err("trusted harness preparation failed"), None)),
            };
            drop(token);
            result
        };
        // Release the immutable guard borrow before minting adds new credentials.
        let result = execution.await;
        after_destruction(std::future::ready(result), async |(result, readback)| {
            println!("golem lifecycle: containers_destroyed");
            // Before any target publication credential: a refreshed sign-in must survive
            // whatever happens to publication, and its values must be guarded first.
            if let Ok(Seat::Codex(original)) = &seat {
                self.write_back_seat(original, readback, guard).await;
            }
            let token = self
                .github
                .token(self.private_key, self.request, true, guard)
                .await?;
            println!("golem lifecycle: publication_credentials_minted");
            let outcome = self.publish_writable(&token, result, guard).await;
            // Only after a final check; never alters the outcome or queue completion.
            if let Some((final_outcome, model)) = self.request.published.take() {
                let status = self
                    .github
                    .update_dashboard(
                        self.request,
                        &token,
                        (&final_outcome, &model),
                        self.run_id,
                        guard,
                    )
                    .await;
                println!("golem dashboard: {status}");
            }
            // Feedback/check failures retain the queue claim for coordinator repair.
            let terminal = outcome?;
            println!("golem lifecycle: publication_complete");
            self.complete(true, true).await?;
            println!("golem lifecycle: reservation_released");
            terminal
        })
        .await
    }

    /// Never fails the run: a failed write-back becomes a check summary note instead.
    async fn write_back_seat(
        &self,
        original: &CodexAuth,
        readback: SeatReadback,
        guard: &mut OutputGuard,
    ) {
        if let Some(Ok(text)) = &readback {
            codex::guard_text(text, guard);
        }
        let failure =
            match readback.map_or(Refresh::Unchanged, |after| codex::refresh(original, after)) {
                Refresh::Unchanged => {
                    println!("golem seat: unchanged");
                    return;
                }
                Refresh::Invalid(error) => error,
                Refresh::Refreshed(refreshed) => match self
                    .github
                    .write_seat(
                        self.private_key,
                        &self.request.seat_secret_name(),
                        refreshed.text(),
                        guard,
                    )
                    .await
                {
                    Ok(()) => {
                        println!("golem seat: refreshed");
                        return;
                    }
                    Err(error) => error,
                },
            };
        println!("golem seat: write-back failed");
        self.request.seat_note.set(Some(failure));
    }

    async fn complete(&self, containers_destroyed: bool, publication_complete: bool) -> Result<()> {
        if !containers_destroyed || !publication_complete {
            return Err("queue completion requires destruction and publication proofs");
        }
        github::queue(
            self.request,
            self.private_key,
            self.run_id,
            self.attempt,
            "complete",
        )
        .await
    }

    /// Private workspace, snapshot, prompt and baseline scan; no container yet.
    async fn prepare_writable(&self, token: &str, guard: &OutputGuard) -> Result<Prepared> {
        let base_sha = self.github.branch_snapshot(self.request, token).await?;
        let temporary =
            tempfile::tempdir().map_err(|_| "cannot create private runner workspace")?;
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir(&workspace).map_err(|_| "cannot create snapshot workspace")?;
        snapshot::extract(
            &self
                .github
                .snapshot_at(self.request, token, &base_sha)
                .await?,
            &workspace,
        )?;
        let configuration = temporary.path().join("configuration");
        let configuration = if base_sha == self.request.head_sha {
            workspace.clone()
        } else {
            std::fs::create_dir(&configuration)
                .map_err(|_| "cannot create configuration workspace")?;
            snapshot::extract(
                &self.github.snapshot(self.request, token).await?,
                &configuration,
            )?;
            configuration
        };
        let prompt =
            snapshot::writable_prompt(&workspace, &configuration, self.request, &base_sha)?;
        guard.check(&prompt)?;
        let before = golem_runner::changes::scan(&workspace)?;
        Ok((temporary, workspace, base_sha, prompt, before))
    }

    /// Outer error: destruction unconfirmed. The seat home is removed on every path.
    async fn execute_writable(
        &self,
        token: &str,
        seat: &Seat,
        name: &str,
        guard: &OutputGuard,
    ) -> Result<(Result<WritableResult>, SeatReadback)> {
        let prepared = self
            .prepare_writable(token, guard)
            .await
            .and_then(|prepared| {
                let home = match seat {
                    Seat::Codex(auth) => Some(codex::private_home(auth)?),
                    Seat::Token(_) => None,
                };
                Ok((prepared, home))
            });
        let ((temporary, workspace, base_sha, prompt, before), home) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Ok((Err(error), None)),
        };
        let credential = match (seat, &home) {
            (Seat::Codex(_), Some(home)) => sandbox::Credential::CodexHome(home.path()),
            (Seat::Token(token), _) => sandbox::Credential::Token(token),
            (Seat::Codex(_), None) => return Ok((Err("missing private seat home"), None)),
        };
        let operation = async {
            if let Err(error) =
                sandbox::prepare_target(&workspace, &format!("{name}-prepare")).await?
            {
                return Ok(Err(error));
            }
            let execution = match sandbox::run_writable(
                &workspace,
                name,
                credential,
                &self.request.harness,
                self.request.model.as_deref(),
                &prompt,
                self.request.timeout_seconds,
            )
            .await?
            {
                Ok(execution) => execution,
                Err(error) => return Ok(Err(error)),
            };
            let output = match golem_runner::writable_result(
                &self.request.harness,
                self.request.model.as_deref(),
                execution.timed_out,
                &execution.stdout,
                &execution.stderr,
                guard,
            ) {
                Ok(output) => output,
                Err(error) => return Ok(Err(error)),
            };
            let after_agent =
                golem_runner::changes::scan_after(&workspace, &before).and_then(|after_agent| {
                    golem_runner::changes::diff(&before, &after_agent, guard).map(|_| after_agent)
                });
            let changes = match after_agent {
                Err(error) => Err(error),
                Ok(after_agent) => {
                    let verified = sandbox::verify(
                        &workspace,
                        &format!("{name}-verify"),
                        self.request.verify.as_deref(),
                    )
                    .await?;
                    let changes = verified_changes(&workspace, &before, &after_agent, guard);
                    verified.and(changes)
                }
            };
            Ok(Ok(WritableResult {
                base_sha,
                changes,
                output,
                timed_out: execution.timed_out,
            }))
        }
        .await;
        // A failed destruction must not remove a possibly still-mounted workspace, but a
        // credential never outlives the run: dropping the seat home removes it either way.
        let result = match operation {
            Ok(result) => result,
            Err(error) => {
                let _retained = temporary.keep();
                drop(home);
                return Err(error);
            }
        };
        let readback = home.map(|home| {
            let after = codex::read_auth(home.path());
            if home.close().is_err() {
                println!("golem seat: private home cleanup failed");
            }
            after
        });
        temporary
            .close()
            .map_err(|_| "cannot remove private runner workspace; publication blocked")?;
        Ok((result, readback))
    }

    // Outer error: feedback/check incomplete. Inner error: safely reported blocked execution.
    async fn publish_writable(
        &self,
        token: &str,
        result: Result<WritableResult>,
        guard: &OutputGuard,
    ) -> Result<Result<()>> {
        let mut result = match result.and_then(|result| {
            guard.check(&result.output.text)?;
            guard.check(&result.output.model)?;
            // publish_changes rescans the complete batch against the newly minted token.
            Ok(result)
        }) {
            Ok(result) => result,
            Err(error) => return self.report_blocked(token, error, "unobserved", guard).await,
        };
        let model = result.output.model.as_str();
        let changes = match result.changes {
            Ok(changes) => changes,
            Err(error) => return self.report_blocked(token, error, model, guard).await,
        };
        let blocked = if result.output.disposition == Some(golem_runner::AgentDisposition::Blocked)
        {
            Some("agent reported task blocked")
        } else if !result.timed_out
            && result.output.disposition != Some(golem_runner::AgentDisposition::Completed)
        {
            Some("missing writable disposition")
        } else {
            None
        };
        if let Some(reason) = blocked {
            return self.report_blocked(token, reason, model, guard).await;
        }
        if result.timed_out {
            result
                .output
                .text
                .push_str("\n\nRun incomplete: agent wall-clock budget exhausted.");
        }
        let feedback = match self.github.feedback_eligible(self.request, token).await {
            Ok(feedback) => feedback,
            Err(error) => return self.report_blocked(token, error, model, guard).await,
        };
        let mut text = result.output.text.clone();
        let outcome = if result.timed_out {
            "incomplete"
        } else if !changes.is_empty() {
            "changed"
        } else if feedback {
            "replied"
        } else {
            "no_change"
        };
        if !changes.is_empty() {
            match self
                .github
                .publish_changes(
                    self.request,
                    token,
                    &result.base_sha,
                    &changes,
                    &result.output,
                    guard,
                )
                .await
            {
                Ok(url) => {
                    text.push_str("\n\nPull request: ");
                    text.push_str(&url);
                }
                Err(error) => {
                    let _reported = self.report_blocked(token, error, model, guard).await?;
                    // Publication may have partially succeeded; retain ownership for repair.
                    return Err(error);
                }
            }
        }
        let reply = if feedback {
            self.github.reply(self.request, token, &text, guard).await
        } else {
            Ok(())
        };
        let check = self
            .github
            .check_reason(
                self.request,
                token,
                outcome,
                model,
                self.run_id,
                (outcome == "no_change").then_some(text.as_str()),
                guard,
            )
            .await;
        reply?;
        check?;
        Ok(Ok(()))
    }

    async fn report_blocked(
        &self,
        token: &str,
        error: &'static str,
        model: &str,
        guard: &OutputGuard,
    ) -> Result<Result<()>> {
        let feedback = self.github.feedback_eligible(self.request, token).await;
        let label = self.github.mark_blocked(self.request, token).await;
        let reply = match feedback {
            Ok(true) => {
                self.github
                    .reply(
                        self.request,
                        token,
                        &format!(
                            "Run blocked: {error}. Inspect the private Actions run for status."
                        ),
                        guard,
                    )
                    .await
            }
            Ok(false) => Ok(()),
            Err(error) => Err(error),
        };
        let check = self
            .github
            .check(self.request, token, "blocked", model, self.run_id, guard)
            .await;
        label?;
        reply?;
        check?;
        Ok(Err(error))
    }
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
