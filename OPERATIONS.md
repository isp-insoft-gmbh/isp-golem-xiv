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

Comment `@isp-golem-xiv stop` in the run's thread to request cancellation.
Cancellation acknowledgment alone does not release execution ownership; reconciliation waits for a confirmed terminal workflow.
Do not replay a delivery merely because a reply is slow.

## Read the organization dashboard

The first Slice 4 admin dashboard increment is deployed at [/admin](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/admin); full acceptance remains pending.
Organization members sign in with GitHub; every data request rechecks current membership.

- **Runs:** paginated metadata for the last 30 days, with repository, golem, trigger, seat, observed model, outcome, duration and ops log.
- **Usage:** counts and observed time per paying person/provider over loaded verified records, not provider charges or billed Actions minutes.
- **Health:** workflow failures, blocked/incomplete/stopped outcomes and coverage gaps.
- **Seats:** registered providers and unmatched secret-name identities for people who left the organization.
- **Repositories:** installed repositories, enabled golems and invalid definitions; unavailable repository metadata does not prevent loading later repositories.

Use the pagination controls before interpreting totals; partial, missing, malformed and pending records are explicitly labeled.
Duration is Actions start to final target check completion, not billing time.
Unknown values are not requested settings or inferred successful results.
Historical audit backfill and refreshing-seat expiry metadata remain unfinished; expiry is not inferred from a secret's update time.
Private log links require separate access to `isp-golem-xiv-ops`.

## Diagnose a refusal or blocked run

Read the refusal's remedy or the check summary first.
Unknown golem → check its spelling and `.golems/` validation.
No seat → register a supported seat, then make a new mention.
Not a member → ask an organization owner to verify membership.
Invalid definition → fix the golem configuration in a reviewed pull request.

For `blocked`, inspect the private ops log and the verification failure or protected-path reason.
Never put target code, credentials, raw provider replies or private logs into the public code repository.
A protected-path refusal must be fixed in the task or repository configuration, not by weakening runner guards.
An `incomplete` run can publish only verified work; a later mention continues its owned branch.

If a run remains reserved after startup or transport failure, preserve its request/run identities and investigate coordinator evidence before taking action.
Do not reset queue state, manually mark terminal, or blindly redispatch.
Use the read-only defaults of `tools/repair-unbound.mjs`; any guarded repair needs matching provenance and deployed-source checks.
For a broken provider sign-in, register a fresh replacement through Seats rather than copying an old local credential.

## Release and verify

Changes reach `trunk` only through a pull request with required `ci` green on the exact head.
CI performs deployments; operators do not deploy locally.
Release runners with the repository release task, verify release assets, then pin the private ops workflow before a router change depends on new runner behavior.
The public `ops/runner.yml` template and the live private workflow must match.

Full project completion requires explicit `mise run accept -- --slice 4` plus recorded browser evidence, not bare `mise run accept`.
Windows Worker suites run unchanged through WSL; setup and the explicit native diagnostic override are described in [DEVELOPMENT.md](DEVELOPMENT.md).
