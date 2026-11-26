#![forbid(unsafe_code)]

mod github;
mod sandbox;
mod snapshot;
mod target_mise;
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
    let arguments: Vec<String> = env::args().skip(1).take(4).collect();
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--prepare", mise] => return sandbox::prepare(Path::new(mise), None).await,
        ["--prepare", mise, cache] => {
            return sandbox::prepare(Path::new(mise), Some(Path::new(cache))).await;
        }
        // The workflow's image cache key: what `prepare` verifies on a loaded image, plus the
        // UTC month, so unpinned base packages are rebuilt at least monthly.
        ["--image-key", mise] => {
            let output = environment("GITHUB_OUTPUT", 4096)?;
            let mut file = OpenOptions::new()
                .append(true)
                .open(output)
                .map_err(|_| "cannot write workflow image key")?;
            let digest = sandbox::image_source_digest(Path::new(mise))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "system clock before 1970")?
                .as_secs();
            let minute = router::utc_minute(now);
            let month = minute.get(..7).ok_or("cannot format image key month")?;
            return writeln!(file, "key=golem-agent-image-{digest}-{month}")
                .map_err(|_| "cannot write workflow image key");
        }
        _ => {}
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
    if request.version == 2 {
        return RunContext {
            github: &github,
            request: &request,
            private_key: &private_key,
            run_id: &run_id,
            attempt: &attempt,
        }
        .run_writable(seat, &mut guard)
        .await;
    }
    let token = initial_target(&github, &request, &private_key, &mut guard).await?;
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
                    .check_reason(
                        request,
                        token,
                        "blocked",
                        &output.model,
                        run_id,
                        Some(error),
                        guard,
                    )
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
                .check_reason(request, token, outcome, model, run_id, Some(error), guard)
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
    println!("golem phase: snapshot");
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
    println!("golem phase: Mise selection");
    let seconds = match target_mise::prepare(&workspace, request.timeout_seconds).await {
        Ok(seconds) => seconds,
        Err(error) => return Ok(Err(error)),
    };
    println!("golem phase: agent");
    let execution = sandbox::run(
        &workspace,
        name,
        seat,
        request.model.as_deref(),
        &prompt,
        seconds,
    )
    .await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let _retained = temporary.keep();
            return Err(error);
        }
    };
    temporary
        .close()
        .map_err(|_| "cannot remove private runner workspace; publication blocked")?;
    Ok(match execution {
        Ok((stdout, stderr)) => agent_result(&stdout, &stderr, guard),
        Err(error) => Err(error),
    })
}

/// Why a parsed writable result must not publish; only a typed blocked result carries the
/// golem's own explanation for the human.
fn refusal(output: &AgentOutput, timed_out: bool) -> Option<(&'static str, Option<&str>)> {
    match output.disposition {
        Some(golem_runner::AgentDisposition::Blocked) => {
            Some(("agent reported task blocked", Some(output.text.as_str())))
        }
        Some(golem_runner::AgentDisposition::Completed) => None,
        _ if timed_out => None,
        _ => Some(("missing writable disposition", None)),
    }
}

/// The workspace the agent left, after checking every change and its publication candidate.
fn agent_workspace(
    workspace: &Path,
    before: &golem_runner::changes::Files,
    guard: &OutputGuard,
) -> Result<golem_runner::changes::Files> {
    let after_agent = golem_runner::changes::scan_workspace(workspace, before)?;
    golem_runner::changes::validate_workspace(before, &after_agent, guard)?;
    golem_runner::changes::diff(
        before,
        &golem_runner::changes::publishable(before, &after_agent)?,
        guard,
    )?;
    Ok(after_agent)
}

fn verified_changes(
    workspace: &Path,
    before: &golem_runner::changes::Files,
    after_agent: &golem_runner::changes::Files,
    guard: &OutputGuard,
) -> Result<Vec<golem_runner::changes::Change>> {
    let published = golem_runner::changes::publishable(before, after_agent)?;
    let after_verify = golem_runner::changes::scan_workspace(workspace, before)?;
    if published
        .iter()
        .any(|(path, file)| after_verify.get(path) != Some(file))
    {
        return Err("verification mutated workspace; publication blocked");
    }
    // Ignored and verifier-generated outputs are disposable, never part of the frozen
    // publication. Still scan every output and reject forbidden paths, links, floods and
    // credentials.
    golem_runner::changes::validate_workspace(after_agent, &after_verify, guard)?;
    golem_runner::changes::diff(before, &published, guard)
}

/// What a Codex or Pi run on a Codex seat left in its private `auth.json`, in that harness's
/// format, read after confirmed destruction.
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

async fn initial_target(
    github: &github::Github,
    request: &DispatchV1,
    private_key: &str,
    guard: &mut OutputGuard,
) -> Result<String> {
    let token = github
        .token(private_key, request, false, guard)
        .await
        .inspect_err(|_| eprintln!("golem startup: target installation token failed"))?;
    github
        .validate_target(request, &token)
        .await
        .inspect_err(|_| eprintln!("golem startup: target validation failed"))?;
    Ok(token)
}

async fn admit_target(
    bind: impl Future<Output = Result<()>>,
    target: impl Future<Output = Result<String>>,
) -> Result<String> {
    // Record exact run ownership before fallible target I/O. A target failure then
    // leaves a bound run for terminal reconciliation, not an ambiguous dispatch.
    bind.await?;
    println!("golem lifecycle: admitted");
    target.await
}

impl RunContext<'_> {
    async fn run_writable(&self, seat: Result<Seat>, guard: &mut OutputGuard) -> Result<()> {
        let token = admit_target(
            github::queue(
                self.request,
                self.private_key,
                self.run_id,
                self.attempt,
                "bind",
            ),
            initial_target(self.github, self.request, self.private_key, guard),
        )
        .await?;
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
        // Pi keeps the seat in its own format; the stored seat is always Codex's.
        let pi = self.request.harness == "pi";
        if let Some(Ok(text)) = &readback {
            if pi {
                codex::guard_pi_text(text, guard);
            } else {
                codex::guard_text(text, guard);
            }
        }
        let decision = readback.map_or(Refresh::Unchanged, |after| {
            if pi {
                codex::pi_refresh(original, after, std::time::SystemTime::now())
            } else {
                codex::refresh(original, after)
            }
        });
        let failure = match decision {
            Refresh::Unchanged => {
                println!("golem seat: unchanged");
                return;
            }
            Refresh::Invalid(error) => error,
            Refresh::Refreshed(refreshed) => {
                // A converted Pi sign-in is new text; guard it like the read-back.
                refreshed.guard(guard);
                match self
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
                }
            }
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
        // A Codex seat reaches Pi as a converted copy in Pi's own format.
        let pi = self.request.harness == "pi";
        println!("golem phase: snapshot");
        let prepared = self
            .prepare_writable(token, guard)
            .await
            .and_then(|prepared| {
                let home = match seat {
                    Seat::Codex(auth) if pi => Some(codex::private_home(&auth.pi_text()?)?),
                    Seat::Codex(auth) => Some(codex::private_home(auth.text())?),
                    Seat::Token(_) => None,
                };
                Ok((prepared, home))
            });
        let ((temporary, workspace, base_sha, prompt, before), home) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Ok((Err(error), None)),
        };
        let credential = match (seat, &home) {
            (Seat::Codex(_), Some(home)) if pi => sandbox::Credential::PiHome(home.path()),
            (Seat::Codex(_), Some(home)) => sandbox::Credential::CodexHome(home.path()),
            (Seat::Token(token), _) => sandbox::Credential::Token(token),
            (Seat::Codex(_), None) => return Ok((Err("missing private seat home"), None)),
        };
        let operation = async {
            println!("golem phase: Mise selection");
            let seconds = match target_mise::prepare(&workspace, self.request.timeout_seconds).await
            {
                Ok(seconds) => seconds,
                Err(error) => return Ok(Err(error)),
            };
            println!("golem phase: target preparation");
            if let Err(error) =
                sandbox::prepare_target(&workspace, &format!("{name}-prepare"), guard).await?
            {
                return Ok(Err(error));
            }
            println!("golem phase: agent");
            let execution = match sandbox::run_writable(
                &workspace,
                name,
                credential,
                &self.request.harness,
                self.request.model.as_deref(),
                &prompt,
                seconds,
            )
            .await?
            {
                Ok(execution) => execution,
                Err(error) => return Ok(Err(error)),
            };
            println!("golem phase: agent result");
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
            let changes = self.verified(&workspace, &before, name, guard).await?;
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

    /// The agent's checked changes after the target's own verification left them unchanged.
    /// Outer error: verifier destruction unconfirmed.
    async fn verified(
        &self,
        workspace: &Path,
        before: &golem_runner::changes::Files,
        name: &str,
        guard: &OutputGuard,
    ) -> Result<Result<Vec<golem_runner::changes::Change>>> {
        println!("golem phase: agent workspace scan");
        let after_agent = match agent_workspace(workspace, before, guard) {
            Ok(after_agent) => after_agent,
            Err(error) => return Ok(Err(error)),
        };
        println!("golem phase: verification");
        let verified = sandbox::verify(
            workspace,
            &format!("{name}-verify"),
            self.request.verify.as_deref(),
            guard,
        )
        .await?;
        Ok(verified.and_then(|()| {
            println!("golem phase: verified workspace scan");
            verified_changes(workspace, before, &after_agent, guard)
        }))
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
            Err(error) => {
                return self
                    .report_blocked(token, error, None, "unobserved", guard)
                    .await;
            }
        };
        let model = result.output.model.as_str();
        let changes = match result.changes {
            Ok(changes) => changes,
            Err(error) => return self.report_blocked(token, error, None, model, guard).await,
        };
        if let Some((reason, explanation)) = refusal(&result.output, result.timed_out) {
            return self
                .report_blocked(token, reason, explanation, model, guard)
                .await;
        }
        if result.timed_out {
            result
                .output
                .text
                .push_str("\n\nRun incomplete: agent wall-clock budget exhausted.");
        }
        let feedback = match self.github.feedback_eligible(self.request, token).await {
            Ok(feedback) => feedback,
            Err(error) => return self.report_blocked(token, error, None, model, guard).await,
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
            println!("golem phase: publication");
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
                    let _reported = self
                        .report_blocked(token, error, None, model, guard)
                        .await?;
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

    /// `explanation` is the golem's own reason, already guarded like any reply text.
    async fn report_blocked(
        &self,
        token: &str,
        error: &'static str,
        explanation: Option<&str>,
        model: &str,
        guard: &OutputGuard,
    ) -> Result<Result<()>> {
        let feedback = self.github.feedback_eligible(self.request, token).await;
        let label = self.github.mark_blocked(self.request, token).await;
        let reply = match feedback {
            Ok(true) => {
                let mut text =
                    format!("Run blocked: {error}. Inspect the private Actions run for status.");
                if let Some(explanation) = explanation {
                    text.push_str("\n\n");
                    text.push_str(explanation);
                }
                self.github.reply(self.request, token, &text, guard).await
            }
            Ok(false) => Ok(()),
            Err(error) => Err(error),
        };
        let check = self
            .github
            .check_reason(
                self.request,
                token,
                "blocked",
                model,
                self.run_id,
                Some(error),
                guard,
            )
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
