# Operating golems

## Enable a repository

Install the organization GitHub App and add `.golems/*.md`.
No repository allowlist, activation flag, local runner, or repository workflow is required.

Pushes and App installation never open starter pull requests.
For optional starter files, sign in at `/admin`, find the repository, and choose **Create starter PR**.
Organization membership and repository write access are required; no subscription seat is needed.
Repeated requests link the existing PR, even when closed; they never reopen or duplicate it.
Review and merge that pull request to add the house rules and `hello` golem.
The App creates and pins **Golems**, the repository dashboard.
Keep repository tool requirements in its own mise configuration; harnesses load repository context, skills, hooks and other project configuration natively.
Repository hooks and MCP servers run inside the agent sandbox and can access its provider sign-in, so review them before enabling a repository.

Add golems as `.golems/<name>.md`, following [SPEC.md](SPEC.md).
Pull requests changing `.golems/` receive the `golems/validate` check.
Invalid definitions appear under **Disabled** and do not run.

## Register a seat

Open [Seats](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats) and sign in with GitHub as an organization member.
Choose Anthropic or OpenAI Codex and follow the page's sign-in instructions.
Use a fresh, golem-only sign-in, never the credential used by your own CLI.
Refreshing sign-ins rotate; sharing one with another CLI can invalidate either copy.

The browser encrypts the sign-in for the private ops repository before uploading it.
The router relays ciphertext and cannot read the credential.
Registering the same provider again replaces that seat; selecting a preferred provider influences fallback when the requested provider is unavailable.
After successful registration, remove the temporary local sign-in folder as the page instructs.

Anthropic seats support Claude and Pi; OpenAI Codex seats support Codex and Pi.
The same stored Codex seat serves both harnesses, with token refresh written back by the runner.
A fallback names the requested and selected seat/harness/model and its reason.
The seat page exposes only your own registered providers.

## Run, observe, stop

Comment with `@isp-golem-xiv <name>` on the first line, followed by your instruction.
A bare `@isp-golem-xiv` lists the repository's golems; `@isp-golem-xiv seat` shows your providers and the registration link.
The dashboard's **Run now** box starts that golem on the member who ticks it; the App clears the box after handling it.

Eyes acknowledge the mention; a rocket indicates dispatch.
The `golem/<name>` check shows outcome, paying seat, harness, model, budget and private ops log link.
The repository dashboard shows the last result per golem, not a historical ledger.
Same-seat and same-golem runs queue rather than overlap.
Changes that pass the golem's verification command are published on its owned branch and pull request.

A run first prepares the agent image: usually it loads a cached image within about a minute, but after a change to the image sources or the pinned Mise, at a UTC month boundary, or once the cache was evicted (the eviction policy of the Blacksmith cache is not verified), the first run builds it, which takes about three minutes.
The ops `agent-image` workflow fills that cache on a miss after any trunk change to `runner.yml` or `agent-image.yml`, daily at 00:05 UTC, and when started by hand before an important run.

Comment `@isp-golem-xiv stop` in the run's thread to request cancellation.
The stop reaches the run when its runner admits it, right after preparation, so a stop during a cached start takes about a minute.
Cancellation acknowledgment alone does not release execution ownership; reconciliation waits for a confirmed terminal workflow.
Do not replay a delivery merely because a reply is slow.

## Read the organization dashboard

The admin dashboard is at [/admin](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/admin).
Organization members sign in with GitHub; every data request rechecks current membership.

- **Runs:** paginated metadata for the last 30 days, with repository, golem, trigger, seat, observed model, outcome, duration and ops log.
- **Usage:** counts and observed time per paying person/provider over loaded verified records, not provider charges or billed Actions minutes.
- **Health:** workflow failures, blocked/incomplete/stopped outcomes, missing audits, and runs from before audits existed.
- **Seats:** registered providers with each seat's latest successful and blocked run among loaded audits, and unmatched secret-name identities for people who left the organization.
- **Repositories:** installed repositories, enabled golems and invalid definitions; unavailable repository metadata does not prevent loading later repositories.

Use the pagination controls before interpreting totals; partial, missing, malformed and pending records are explicitly labeled.
Duration is Actions start to final target check completion, not billing time.
Unknown values are not requested settings or inferred successful results.
Runs created before 2026-11-13 predate audits and are counted apart from missing audits; nothing is backfilled.
Sign-in expiry and refresh state are not recorded, so the dashboard cannot warn before a seat expires; a failing seat shows as blocked runs, and a failed Codex refresh write-back adds a `seat refresh` note to that run's check, and expiry is never inferred from a secret's update time.
Private log links require separate access to `isp-golem-xiv-ops`.
Repositories groups configured, unconfigured and unknown entries; configured repositories can have mixed, disabled, or no definitions.
Open an unconfigured repository to review its proposed starter PR and files; submission can return an existing or closed PR and never reopens it.

### Inspect Queue as an organization owner

Open Operations → Queue and refresh before interpreting a hold.
If Queue is unavailable on an older login, use **Sign in again** to bind the browser session to your GitHub identity.
Ordinary members can still read the other dashboard sections and use repository-authorized onboarding.
Queue groups requests by paying seat, shows reservations first, and keeps pending requests in journal order.
Cross-seat blockers link to their exact requests; seat grouping does not imply global dispatch order.
Waiting age is measured from immutable request creation to the displayed snapshot time.
Candidate requests still need normal dispatch preflight; the view and evidence recheck do not advance the queue.

- **Review pending cancellation:** requires the exact pending request, no dispatch intent or run, and verification of its original queued check.
  Confirmation records a stop request, not an Actions cancellation; normal maintenance completes its check.
- **Recheck release evidence:** requires the exact bound attempt, terminal workflow proof and a verified completed check or durable audit.
  **Verify and release** appears only for eligible evidence and confirms just that reservation, without dispatching a replacement.
- **Unknown run or missing proof:** keep the reservation; inspect private evidence or ask the operations owner to investigate.
  The inline missing fact explains what blocks release; open **Why?** for the next step.
  A matching title alone cannot repair an unbound dispatch, including the historical Anthropic incident.

An expired or changed preview must be checked again; do not repeat an unconfirmed request without reviewing fresh queue state.
Actor, action and request association are in the authoritative `queue.json` commit history even after ordinary retirement.

## Diagnose a refusal or blocked run

Read the refusal's remedy or the check summary first; a `blocked` or `incomplete` check names its reason on a `reason:` line.
Unknown golem → check its spelling and `.golems/` validation.
No seat → register a supported seat, then make a new mention.
Not a member → ask an organization owner to verify membership.
Invalid definition → fix the golem configuration in a reviewed pull request.

For `blocked`, inspect the private ops log as described below.
Common reasons:

- `Mise release asset unavailable`, `Mise archive SHA-256 mismatch` or another Mise selection failure: check that the workspace's hard minimum names an exact official Linux x64 release with its published checksum; the runner does not fall back to an older executable.
- `verification container execution failed` or `target preparation container execution failed`: the repository's own command failed; its output tail in the log shows why.
- `container failed after a process in it was killed at its memory limit`: Docker recorded a memory kill (2 GiB per container); it may or may not have caused the failure.
- `container killed before its time budget`: the container ended with SIGKILL before its time ran out.
- `snapshot file limit exceeded` or `snapshot byte limit exceeded`: a file or the total was over the bounds, 4 MiB per file and 64 MiB in total for the repository snapshot, 64 MiB and 512 MiB for the workspace after the agent or the verification.
- `verification mutated workspace; publication blocked`: the verification command changed files the agent produced.
- `agent reported task blocked`: the golem itself declined; from runner v21 on, the reply quotes its explanation after this line.

Never put target code, credentials, raw provider replies or private logs into the public code repository.
A protected-path refusal must be fixed in the task or repository configuration, not by weakening runner guards.
An `incomplete` run can publish only verified work; a later mention continues its owned branch.

If a run remains reserved after startup or transport failure, preserve its request/run identities and investigate coordinator evidence before taking action.
Do not reset queue state, manually mark terminal, or blindly redispatch.
Use the read-only defaults of `tools/repair-unbound.mjs`; any guarded repair needs matching provenance and deployed-source checks.
For a broken provider sign-in, register a fresh replacement through Seats rather than copying an old local credential.

## Read a run log

The check's log link opens the private ops run; read it there or with `gh run view <id> --repo isp-insoft-gmbh/isp-golem-xiv-ops --log`, which prefixes each line with job, step and timestamp.
Lines starting with the word `golem` come from the runner; output tails follow unprefixed between `::stop-commands::` markers, and the image build prints Docker's own output.
Read-only runs print the snapshot, Mise selection and agent phases and end with `golem completed: replied`.

- `golem image:` in the preparation step: loaded from cache, or no usable cache, building, then built, cached; a build also runs the image's own self-tests.
- `golem lifecycle:` admitted, containers_destroyed, publication_credentials_minted, publication_complete, reservation_released, in that order; a missing step shows where the run stopped.
  `admitted` means the run owns the queue reservation, not that target validation passed or an agent started.
- `golem startup:` target installation token or validation failure; `golem GitHub:` reports the HTTP method and rejection status without the URL or body.
- `golem mise: using verified <version>` appears only when using a checked per-run upgrade.
- `golem phase:` precedes each step reached: snapshot, Mise selection, target preparation, agent, agent result, agent workspace scan, verification, verified workspace scan (only after a passing verification), publication (only when there are changes); an error after a phase line belongs to that phase.
- `golem dashboard:` whether the repository dashboard was updated, before publication_complete.
- `golem container:` each container's stage, exit code, duration and output sizes, and whether a memory kill was recorded.
- `golem target preparation` and `golem verification` print the last lines (at most 200 or 16 KiB) of each output stream on every run, or that the stream was empty; a tail containing a registered credential is withheld entirely, and agent output is never printed.
- `golem seat:` for OpenAI Codex seats, also under Pi: unchanged, refreshed, or write-back failed; a failed write-back also appears in the check summary, and the next run may need a fresh sign-in; Anthropic seats print no seat line.
- `golem runner failed:` the run's final error.

## Release and verify

Changes reach `trunk` only through a pull request with required `ci` green on the exact head.
CI performs deployments; operators do not deploy locally.
The private ops repository's plan offers no branch protection, so merge its pull requests only after `ci` passed on the exact head and review threads are resolved.

A runner change reaches golems in this order:

1. Merge it, then run `mise run release -- '<summary>'` on a clean `trunk` equal to `origin/trunk` whose head passed `ci`; CI builds and publishes the next `v<n>`.
2. Check the release with `mise run verify-release -- v<n> <full source sha>`: it ties the tag to that source and its release run, compares the asset's SHA-256 with GitHub's digest, the `.sha256` sidecar and `SHA256SUMS`, requires the embedded image to assert the Mise version of the source's `agent.Dockerfile`, and records the result under `evidence/`.
3. In the ops repository, pin `RUNNER_RELEASE_URL` and `RUNNER_SHA256` in `.github/workflows/runner.yml`, in the same commit as its Mise `version` when the release's image asserts a new Mise, since the image copies the host's Mise; merging it starts `agent-image`, which logs `golem image: built, cached` when the image key changed and otherwise finds the cached image and skips the build.
4. Copy the live workflows `runner.yml`, `agent-image.yml` and `ci.yml`, plus `.github/actionlint.yaml` and `mise.toml`, byte for byte into `ops/` here, adjusting `tools/repair-unbound-test.mjs` so historical workflows still reconstruct.
5. The next golem run must log `golem image: loaded from cache`.

After changes to promised behaviour, run `mise run accept -- --slice 4`; it spends about a dozen seat runs on fresh test repositories and deletes them unless `--keep` is given.
Windows Worker suites run unchanged through WSL; setup and the explicit native diagnostic override are described in [DEVELOPMENT.md](DEVELOPMENT.md).
