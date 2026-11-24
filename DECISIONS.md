# Decisions

Decisions in force, by topic; each names the log entry it comes from.
The dated history up to 2026-11-24 is in [history/decisions-log.md](history/decisions-log.md); new decisions get a dated entry under "Log" at the end of this file.

## Architecture and trust boundaries

### The App is installed on all organization repositories

Oliver decided that the App installation covers all repositories, because adding repositories to a selected installation needs a token gh does not have.

Log: 2026-11-03 "Hardening before handoff"

### No per-repository activation

Oliver rejected per-repository activation and removed the sandbox-topic rollout restriction; SPEC states the resulting rule.
Neither repository lookup, scheduled discovery nor the runner's target validation inspects topics; identity, membership, human and check binding and pinned definition validation remain.
Acceptance-test repository names and topics are cleanup markers, not dispatch controls.

Log: 2026-11-18 "Installed organization repository plus Markdown is sufficient"; 2026-11-18 "Finish ordinary-repository support at both execution boundaries"

### Router worker name and URL are fixed in the contract

They are fixed in `contract.nuon`, because the App's OAuth callback URL is set by hand and must not change when the agent deploys.
The `workers.dev` subdomain `isp-insoft-gmbh` was created for this through the Cloudflare API.

Log: 2026-11-03 "Hardening before handoff"

### One Rust workspace, one golem parser

The workspace separates host-tested router policy, wasm edge I/O, the trusted native runner and a stdlib-only xtask.
The runner reuses the router's pure parser through a versioned path dependency; there is no second frontmatter parser.

Log: 2026-11-03 "Slice 1 implementation"; 2026-11-04 "Guarded Slice 2 implementation, live proof pending"

### Agent containers never hold GitHub credentials

Preparation, agent and verification containers get no App or GitHub credentials, host home, Docker socket or workflow control files.
Confirmed container destruction precedes fresh, scoped publication credentials, and only the trusted supervisor may attest destruction and publication before completion releases resources.

Log: 2026-11-03 "Slice 1 implementation"; 2026-11-04 "Guarded Slice 2 implementation, live proof pending"

### Output scanning is defense in depth

Scanning blocks handled and recognized credential representations, not arbitrary covert encodings, and it blocks publishing a seat, not exfiltrating it over the network.

Log: 2026-11-04 "Guarded Slice 2 implementation, live proof pending"; 2026-11-12 "Harnesses run the repository's own configuration"

## Router, queue and coordinator

### The router reads the golem index fresh

The index is read fresh, never cached, bounded to 20 golems of 64 KiB each.

Log: 2026-11-03 "Slice 1 implementation"

### Delivery claims are create-only refs in ops

Atomic create-only refs in the ops repository claim delivery IDs with payload fingerprints; there is no eventually consistent KV lock.
This prevents duplicate dispatch attempts, not transactional delivery; failed or ambiguous deliveries keep their claims, and a missing rocket does not prove dispatch failed.
Workflow reruns are rejected before credential access.

Log: 2026-11-03 "Slice 1 implementation"

### One coordinator arbitrates seats and golems

One SQLite Durable Object arbitrates seat and repository/golem resources together; queued requests use no Actions runners.
The ops `golem-state` journal and immutable request tags are authoritative, SQLite is a rebuildable projection.
Capacity is 32 current records, accepted pending records are never evicted, and dispatch ambiguity keeps its reservation rather than risk a second charge.

Log: 2026-11-04 "Guarded Slice 2 implementation, live proof pending"

### Runner callbacks are signed; only communication is retried

Callbacks use exact-body HMAC-SHA256 keyed with the normalized App PEM, freshness checks, a generation and numeric safe-integer run and attempt identity.
Control requests retry in the same run only on transport errors, 429 and 5xx, with fresh signed timestamps; authorization, ownership and malformed grants fail closed, and agent execution is never retried.

Log: 2026-11-04 "Guarded Slice 2 implementation, live proof pending"; 2026-11-04 "Local PAX paths and failed startup recovery"

### Journal writes are compare-and-swap with readback

Concurrent Contents writes to one branch conflict with HTTP 409, so queue saves freeze the expected blob SHA and bytes, confirm exact readback, retry only unchanged-preimage conflicts within bounds, and reject divergent state without merging or granting.
Nothing blindly redispatches or retries an agent.
Reconciliation ticks skip journal writes that change nothing, since each write sets off push webhooks.

Log: 2026-11-04 "Full acceptance failure and shared-branch journal fencing"; 2026-11-23 "Failed retirements no longer refuse admission; failures are logged"

### Queue and event records are rooted in blobs

The App gets 403 creating refs whose chain ends at a commit with workflow files other than trunk's, so commit-rooted records became unretirable after every ops workflow change, including runner pins.
Oliver chose blob-rooted records over granting the App `workflows: write`, which would let it rewrite workflows in every organization repository.
Request and event source tags are annotated tags of a blob holding their message; a retirement receipt is a blob holding the request tag's id and a newline, and receipts pointing at the request tag stay valid.
Every commit-rooted row admitted before this change had retired by 2026-11-24, so runner pins no longer strand queue rows.

Log: 2026-11-23 "Queue and event records are rooted in blobs"

### Failed retirements defer; they never refuse admission

A failed audit or receipt only defers that row's retirement and the new request is admitted, because the router never retries an admission.
A receipt naming another request tag leaves the row terminal and logged, holding one of the 32 queue slots but no seat.
Journal write conflicts still refuse an admission, since the admission itself must be saved.

Log: 2026-11-23 "Failed retirements no longer refuse admission; failures are logged"

### Cold start bootstraps the state branch

Version 2 processing verifies `golem-state` before consuming the delivery claim and calls the coordinator's atomic bootstrap when it is absent, because a Contents write to a missing branch returns 404.

Log: 2026-11-04 "Cold-start bootstrap and orphan recovery correction"

### Orphan sources are recovered by cron

Cron discovers at most one orphan source per tick from at most 512 pending source refs, keeping its prepared request and check; event-index rotation keeps one failing head from starving peers.
Completion receipts keep the original source identity before index, cursor and pending-ref state is removed; completed sources are re-admitted idempotently, never dispatched again.
Mention admissions reuse durable prepared continuations after rejection.

Log: 2026-11-04 "Cold-start bootstrap and orphan recovery correction"; 2026-11-04 "Guarded Slice 2 implementation, live proof pending"

### Scheduled discovery resumes for repositories added the same day

After a completed daily sweep, the scheduler still sweeps metadata of newly added repositories, keeping completed IDs and daily claims; completed repositories are not rescanned that day.

Log: 2026-11-04 "Released runtime repair and continued repository discovery"

### Undispatched requests preflight their target

Before dispatch, a 404 on the target records the terminal blocked reason `target unavailable at dispatch preflight`; deletion is never claimed, since GitHub also answers 404 for missing authorization.
Dispatched or bound work, 403, 5xx and identity mismatch keep their claims.

Log: 2026-11-04 "Local PAX paths and failed startup recovery"

### Stop reaches only the latest run of its thread

A stop mention stops the latest active mention run, otherwise the latest pending one, never several or unattended runs; in the dashboard issue it also stops the latest dashboard run.

Log: 2026-11-04 "Guarded Slice 2 implementation, live proof pending"; 2026-11-13 "Dashboard last result and run boxes"

### Stop fence for dispatched rows that never bound

A stopped dispatched row with null run and attempt retires 15 minutes after the durable stop, if no active runner run has the request's display title; terminal rows reject late binds with 409.
Its check says `reason: stopped before runner admission; no agent execution was granted`, inventing no run id, model or audit.
Unstopped unbound dispatches are never retired this way, and the fence is skipped once an operation has used 36 of its 45 API calls.

Log: 2026-11-06 "Stop fence for dispatched rows that never bound"

### Terminal runs are audited before release

Reconciliation repairs a missing terminal audit before releasing resources.
A failed run whose check and target are both unavailable gets an immutable private blocked/unconfirmed audit before terminalization; ambiguous failures, wrong identity or failed audit persistence keep ownership.
Audit reuse validates tag name, SHA, subject and full message.

Log: 2026-11-04 "Guarded Slice 2 implementation, live proof pending"; 2026-11-04 "Full acceptance failure and shared-branch journal fencing"

### Audit maintenance leaves dispatch headroom

Audit maintenance leaves call budget for dispatch, and dispatch-intent publication checks retry headroom after reservation.

Log: 2026-11-17 "Organization dashboard and durable sanitized audits"

### Unbound reservations are repaired only by the guarded helper

`tools/repair-unbound.mjs` may only associate one proven completed failed workflow with its exact unbound `dispatching` reservation; it never resets, redispatches or completes work, and reconciliation then audits and releases.
`--apply` requires the reviewed coordinator source at the live Worker health version, checked before evidence writes and before CAS.
The reviewed digest advances with every coordinator change, and the helper refuses `--apply` until that coordinator is deployed.

Log: 2026-11-04 "Local PAX paths and failed startup recovery"; 2026-11-23 "Queue and event records are rooted in blobs"

### Head discovery uses the compact Git-ref endpoint

The full commit response can exceed the unchanged 524,288-byte cap, so head discovery reads the Git-ref endpoint, encodes branch segments and requires a commit object with a valid SHA.

Log: 2026-11-18 "Finish ordinary-repository support at both execution boundaries"

### JSON endpoints accept the media type, not one spelling

JSON endpoints match the case-insensitive essence `application/json`, so `charset=utf-8` passes while `text/plain` and lookalikes fail; limits, membership, permissions and CSRF are unchanged.

Log: 2026-11-18 "Accept the JSON media type, not one header spelling"

## Triggers, commands and repository surfaces

### Harness and provider are preferences, not validation errors

Incompatible harness and provider pairs stay valid because SPEC resolves them at run time; unknown values stay invalid.
`help` and `seat` never dispatch, and `seat` shows secret names, never values.

Log: 2026-11-11 "Help and seat commands; harness/provider are preferences"

### Pull requests touching `.golems/` get a validation check

Opened, synchronized and reopened pull requests touching `.golems/`, or filling a whole files page, get `golems/validate` from the router's index parser.
Validation runs before trigger matching and never drops matching event golems; App-authored events stay ignored.

Log: 2026-11-11 "Pull-request validation check"

### Event comment text is untrusted; feedback has its own authority

Signed event comment text is copied into the bounded instructions without truncation and stays untrusted.
Feedback eligibility is separate from authorization and payer rules: it needs an API-bound comment ID, actor, issue and `User` type, owner fallback checks membership independently, and bots stay silent.
A comment lookup failing after validation yields a blocked check that keeps the observed model, publishes nothing and keeps ownership.

Log: 2026-11-05 "Event context, delegated commits, separate feedback authority"

### Commits are delegated to the host

The writable prompt says commits, pushes and pull requests are made by the trusted host after verification.

Log: 2026-11-05 "Event context, delegated commits, separate feedback authority"

### Starter pull requests only on explicit request

Oliver rejected automatic starter PRs; SPEC and OPERATIONS describe **Create starter PR**.
It needs fresh membership and repository write or admin; maintain maps to write, custom role names never grant access, cookie requests need session CSRF and exact origin, and bearer requests never fall back to a cookie.
The once-per-repository claim `golem-onboarding/<repository id>` is reused; unrelated branches are never overwritten, incomplete claims never resumed automatically, and a 422 is followed by exact claim verification.

Log: 2026-11-18 "Starter PRs require an explicit UI request"; 2026-11-11 "Onboarding and dashboard on push"

### The dashboard issue is kept current on push

The **Golems** issue is found by App author, title and hidden marker, created and pinned once, and patched only when its rendered body changes.
Seats are listed only in private repositories, from ops secret names; values are never read.
Upkeep failures are logged and never drop event golems.

Log: 2026-11-11 "Onboarding and dashboard on push"

### Each golem's last result lives in a validated marker

A hidden marker holds `outcome`, `seat=<login>/<provider>`, `harness`, `model`, `at` and the ops run id, validated on render and parse; the visible `Last:` line derives from it.
Invalid or hand-edited markers render as "never run", re-renders keep valid ones, and purposes escape `<` so they cannot open an HTML comment.

Log: 2026-11-13 "Dashboard last result and run boxes"

### The runner updates the dashboard after a run

After the final check and container destruction, the runner re-reads the dashboard and patches only its golem's lines with the publication token.
This is best effort, logs `golem dashboard: updated|not found|failed`, never changes the outcome, and the last writer wins.

Log: 2026-11-13 "Dashboard last result and run boxes"

### Dashboard run boxes are commands

A `User` edit of the App's dashboard ticking a box is a dashboard command, not an `issues.edited` event; bodies are parsed only for ticks, at most 64 KiB each, and never journaled.
At most three boxes per edit start runs, mention refusal rules apply, and the ticking person always pays.
Ticks become durable event work keyed by `occurrence_id(dashboard-v2/<delivery>/<golem>)` behind the delivery claim, then boxes are cleared best effort.
The runner treats `dashboard` like `mention` but verifies the dashboard issue instead of a comment.

Log: 2026-11-13 "Dashboard last result and run boxes"

### A blocked golem's reply quotes its reason

Oliver chose to quote the golem's message after the static blocked line, guarded like any reply.
Only typed blocked results carry it; runner blocks and missing or malformed dispositions keep the static reason.

Log: 2026-11-23 "A blocked golem's reply says why"

## Runner, sandbox and limits

### Snapshot archives accept only two PAX records

Extraction allows a first exact-sized `pax_global_header` with the commit comment and one bounded UTF-8 local PAX `path` record for the next entry; other overrides and devices are rejected.
Raw path components are checked for Win32 alias and device names before normalization or writes.

Log: 2026-11-03 "First authenticated-path acceptance fails before agent startup"; 2026-11-04 "Local PAX paths and failed startup recovery"

### Each scan bound has one definition

`changes::SOURCE_LIMITS` (4 MiB per file, 64 MiB total) bounds archives, baseline scans and published changes.
`changes::WORKSPACE_LIMITS` (64 MiB, 512 MiB), approved by Oliver, bounds the workspace after agent or verifier.
Other guards are unchanged.

Log: 2026-11-20 "One definition per scan bound; ignored outputs never publish"; 2026-11-19 "Larger disposable verifier outputs, unchanged publication bounds"

### Ignored outputs never publish

New files ignored by the snapshot's own `.gitignore` files are not published; baseline bytes decide, and snapshot files and their deletion always remain candidates.
Every change is still checked for forbidden paths, links and credentials; only published bytes are frozen against verifier mutation.
Matching uses the `ignore` crate, skipping a leading UTF-8 BOM like Git.

Log: 2026-11-20 "One definition per scan bound; ignored outputs never publish"

### Workspace links are recorded, never followed

Scans read link targets and never descend through links; new links in ignored paths are disposable.
Oliver chose to publish a changed link only if its target is relative, without backslash, drive or control characters, with `..` only as a prefix, inside the repository, not into a protected path, and at most 100 bytes, since longer targets need a PAX `linkpath` that extraction rejects.
No link may stand at a protected path itself, including `.git`, because it would move guarded content to an unguarded path.
Extraction creates links after every file and rejects a link below another, so it never writes through one.

Log: 2026-11-20 "Workspace links are recorded, never followed, and published only when safe"

### Protected paths stay read-only to golems

Golems never change `.golems/`, workflows or `.claude`, `.codex`, `.pi`, `.agents`, `.mcp.json`, `.claude.json`, `CLAUDE.md`, `AGENTS.md`.
New paths under the four harness directories are harness state, skipped by the post-agent scan; snapshot files there are still compared.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Writable results carry a typed disposition

Writable runs require a completed or blocked disposition with a bounded message; prose or malformed streams never count as success.
Typed blocked results cannot publish, and the observed model survives validation failures.

Log: 2026-11-04 "Protected-path refusal needs an explicit disposition"

### Golem containers allow 1024 processes and threads

Oliver raised `--pids-limit` from 128 to 1024 for all golem containers, because threads count toward it; memory (2 GiB) and all other controls are unchanged.

Log: 2026-11-20 "Golem containers allow 1024 processes and threads"

### The agent image is cached by content key

The image label is a SHA-256 over staged sources and the pinned Mise; `--image-key` adds the UTC month, so base packages rebuild at least monthly.
`--prepare` loads a cached image only on a matching label, else builds with self-tests and saves.
The label is self-asserted, so integrity rests on cache writers: saving happens only right after preparation, before any seat, on the ops default branch.

Log: 2026-11-21 "Golems start faster so stop stays prompt"

### Codex self-tests end when decided

Oliver chose that the trusted Codex probe ends once its MCP marker and `thread.started` appear (at most 120 s) and the untrusted probe waits 30 s.

Log: 2026-11-21 "Golems start faster so stop stays prompt"

### Image probes test parsers, not help text

The Claude `--max-turns` option is checked by positive and negative parser probes without network, because help omits it; this proves parsing only, not turn-limit enforcement.

Log: 2026-11-03 "Container compatibility diagnosis"

### Agent image carries Python and uv

Debian `python3` and pinned `uv` 0.13.7 are on the PATH for `uvx` MCP servers and skills; uv's directories point at `/tmp` because the home is `noexec`.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Runner downloads retry only transient failures

Artifact GETs retry 408, 429, 5xx and timeouts for at most 180 s per file in 60 s attempts, within a ten-minute step; refused, reset and DNS failures are not retried.

Log: 2026-11-06 "Stop fence for dispatched rows that never bound"

## Harnesses, seats and providers

### Providers are `anthropic` and `openai-codex`

Oliver removed `github-copilot` because nobody uses it; a golem naming it is invalid like any unknown provider.

Log: 2026-11-12 "No Copilot provider; harness-only fallback"

### Seat choice is one function, recomputed by the runner

`choose_seat` implements SPEC's order for help and dispatch; without a preferred seat it takes the first registered seat in contract provider order, and cost never moves to another payer.
Requests carry provider, harness, model and an optional bounded `fallback` text that every check summary, reply and pull request repeats; requests without fallback stay byte-identical.
The runner recomputes the choice from the pinned definition and the paying seat and rejects any mismatch.
Oliver chose harness-only fallback: a seat at the golem's provider whose harness does not work there keeps that seat and runs `pi` with its default model.

Log: 2026-11-11 "Help and seat commands; harness/provider are preferences"; 2026-11-11 "Seat fallback runs on a `pi` harness"; 2026-11-12 "No Copilot provider; harness-only fallback"

### Harnesses run the repository's own configuration

Oliver decided that harnesses load the target's project settings, hooks, skills, commands, agents, extensions, rules, MCP servers and context files natively; the runner reimplements none of them.
The prompt keeps only golem-owned content (capabilities and boundaries, house rules, task, trigger context, instructions) and no longer injects `CLAUDE.md` or `AGENTS.md`.
Each harness emits exactly one typed result; unknown record types are ignored and malformed lines fail closed.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Repository harness code can read the seat

Repository hooks, extensions and MCP servers run as children of the harness and inherit its seat, so anyone who can push harness configuration can read the seat of whoever runs a golem there.
Trusting project configuration means trusting the repository's committed harness code like its build scripts.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Claude runs with project settings and bypassed permissions

`claude -p` runs with `--setting-sources project,local` and a fresh `$CLAUDE_CONFIG_DIR/.claude.json` that marks `/workspace` trusted, so trust-gated project items apply.
Writable runs use `--permission-mode bypassPermissions`; our `--settings` only adds `deny` for `WebFetch` and `WebSearch`, and deny rules from every source still bind in bypass mode.
The read-only v1 protocol keeps only `Read`, `Glob` and `Grep`.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Pi runs with `--approve`

Pi 1.5.1 runs with `--approve` instead of `--no-approve`, because `--no-approve` declines project trust and Pi never asks before tool calls.
The `golem_report` instruction travels in the prompt, because `--append-system-prompt` would hide a project `.pi/APPEND_SYSTEM.md`.
`/opt/golem/pi-report.js` loads via `--extension`, so a project extension registering `golem_report`, or any project extension failing to load, fails the run closed.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Pi's result is one `golem_report` call

The typed result is exactly one successful `golem_report` call made last; missing, duplicate or malformed reports and unsettled or aborted runs fail closed, because Pi's JSON mode exits 0 even on provider errors.
An Anthropic seat's token reaches Pi as `ANTHROPIC_OAUTH_TOKEN`.

Log: 2026-11-11 "Seat fallback runs on a `pi` harness"; 2026-11-13 "Pi on OpenAI Codex seats (runner only)"

### Codex runs with trusted project and hooks

`codex exec` runs with `--dangerously-bypass-hook-trust` and `--config 'projects={"/workspace"={trust_level="trusted"}}'`; the inline table is deliberate because dotted override keys split on every `.`.
`--ignore-user-config` stays, and our `--config` keys (web search off, no history, analytics, feedback or update check) win over `.codex/config.toml` for those keys only.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Codex seats write back refreshes

Codex gets a private per-run `CODEX_HOME` holding only the seat's `auth.json`, mounted outside `/workspace`.
After confirmed destruction and before publication credentials, a refreshed `auth.json` (same account, newer `last_refresh`, valid tokens) is sealed with `dryoc` and written back with an ops-only `secrets: write` token; failures are noted in summaries, never fail the run.

Log: 2026-11-11 "Seat registration page and refreshing Codex seats"

### Pi runs on Codex seats from a converted sign-in

The seat stays in Codex format; Pi gets a converted copy whose `expires` is the access token's JWT `exp`, in a private writable per-run `/golem-pi` named by `PI_CODING_AGENT_DIR`, and no Anthropic token is forwarded.
A seat without a decodable `exp` or a `chatgpt_account_id` claim is refused for Pi before any container, since a guessed expiry would rotate the seat every run; it still runs `codex`.
`--provider openai-codex --model <m>` is passed only for a requested model.
After destruction, changed tokens are written back into Codex's file from the original seat's fields with a runner-stamped `last_refresh`, after an account check on the `chatgpt_account_id` claims.

Log: 2026-11-13 "Pi on OpenAI Codex seats (runner only)"

### Seat registration seals credentials in the browser

Unauthenticated `/seats` and `/admin` redirect to App user authorization with a fresh state in a ten-minute `HttpOnly; Secure; SameSite=Lax` cookie scoped to `/auth/github`; the callback checks it in constant time, requires active membership, and sets a one-hour `golem_session` cookie MAC'd with a key derived from the client secret.
Writes need an exact same-origin `Origin`, the session, an `X-Golem-Csrf` HMAC and rechecked membership.
The browser seals the credential with a libsodium-compatible sealed box against the ops Actions public key, the router only relays `encrypted_value` to `SEAT_<LOGIN>__<PROVIDER>`, and the preferred-seat variable accepts only a provider the person has.
Strict CSP, no inline script; vendored TweetNaCl.js 1.0.3 and blakejs 1.2.1 are byte-identical to their npm tarballs.

Log: 2026-11-11 "Seat registration page and refreshing Codex seats"; 2026-11-11 "Help and seat commands; harness/provider are preferences"

### Seat health comes from audits only

Nothing the router can read holds a seat's expiry or refresh result, so, asked again with these facts, Oliver chose audit-only signals.
The seats table shows each seat's latest successful and latest blocked run among loaded audits, matched in the secret's form (login uppercased, `-` as `_`), because departed members are known only by that form.

Log: 2026-11-23 "Seat health from audits; runs before audits counted apart"

## Diagnostics and logging

### Preparation and verification output tails reach the private log

Oliver chose to print a bounded tail of target preparation and verification output to the private Actions log on every run, passing or failing; agent output is still never logged.
Each tail is at most 200 lines and 16 KiB per stream, withheld entirely if it contains a guarded credential as UTF-8 or UTF-16, has control characters replaced, and is fenced by a random `stop-commands` token so target output cannot issue workflow commands.
Every container stage also logs exit code, duration and output sizes, and a phase line precedes each runner step.

Log: 2026-11-20 "Container failures are diagnosable from the private run log"

### Nonzero container exits name their caller

A nonzero start or attach outcome reports only a fixed static caller message: preparation, harness or verification.
It identifies the caller, not that the harness launched, the provider failed or a target command ran.

Log: 2026-11-19 "Distinguish nonzero container caller phases without exposing streams"

### Kills and memory limits are reported as observations

Oliver chose that exit 137 before the host deadline blocks as a kill instead of a timeout with partial publication; exit 124 keeps its timeout handling, as changing it was not decided.
After any unsuccessful exit the runner asks Docker and, if a memory kill was recorded, blocks with "container failed after a process in it was killed at its memory limit".
The record is sticky and container-wide, so the message states an observation, not a cause.
Docker sometimes leaves no record when a child is killed and the container survives, so Oliver decided that the runner also watches Docker's out-of-memory events during the run and the container test asserts that detection; until it ships, the note can be missing.

Log: 2026-11-20 "Container failures are diagnosable from the private run log"

### Router and coordinator log fixed stages only

Router and coordinator log fixed failure stages and statuses, never request data.
Worker observability keeps these logs with invocation logs off, so request URLs such as the OAuth callback's code are not recorded, and query strings are redacted.

Log: 2026-11-23 "Failed retirements no longer refuse admission; failures are logged"

## Admin dashboard and audits

### Each run gets an immutable sanitized audit

Before retirement the coordinator publishes an immutable v3 annotated `golem-admin/<request-id>` tag targeting the request tag, plus a `golem-run/<run-id>` alias for dispatched work.
The closed schema holds request and run identity, repository, golem, trigger, seat, observed model, outcome, duration, completion time and the fixed ops log URL; instructions and credentials are never projected, and unknown observations stay null.
Duration is Actions `run_started_at` to final target check completion, not billing time; retries validate existing records and never overwrite conflicts.

Log: 2026-11-17 "Organization dashboard and durable sanitized audits"

### An unobserved model is null

`default` is not an observed model: new audits record null, existing v3 placeholders normalize to null at read time, and retirement accepts that historical unknown without rewriting the audit.
Requested model names do not establish a provider observation.

Log: 2026-11-17 "Live admin projection and unknown model correction"

### Admin data requires current membership

Every admin data request rechecks current organization membership; bearer identity comes from GitHub `/user`, never from caller claims, and cookie sign-in returns to the fixed `/admin` destination.
Responses are private and no-store, text is escaped, and log URLs stay fixed to ops.

Log: 2026-11-17 "Organization dashboard and durable sanitized audits"

### Totals describe loaded records, with gaps labeled

Pagination is bounded per request with explicit missing, malformed, pending and partial-page labels; a repository metadata failure shows as unavailable, never as a disabled repository.
Usage and health describe loaded verified records, not whole-window totals.
Runs created before 2026-11-13T12:51:43Z, when the first sanitized audit was written, count as predating audits rather than missing; nothing is backfilled or fabricated.

Log: 2026-11-17 "Organization dashboard and durable sanitized audits"; 2026-11-23 "Seat health from audits; runs before audits counted apart"

### The admin UI is a plain semantic document

Oliver asked for an understandable, fast, actionable rewrite without elaborate CSS: native semantic elements, no framework, remote font, animation or polling.
A roughly 2 KB same-origin stylesheet is allowed by `style-src 'self'` on admin HTML only; other CSP restrictions are unchanged.
Gaps, failures, status and controls stay visible outside disclosures.

Log: 2026-11-17 "Rewrite the admin UI as a plain, actionable document"

## CI, release and ops

### CI and golems run on Blacksmith runners

The organization's Actions budget stopped private-repository jobs, so Oliver chose Blacksmith runners and trusts Blacksmith with what those jobs hold, including the App private key, seat sign-ins and private logs.
All jobs use `blacksmith-*-ubuntu-2404`, not 26.04, because the runner links the build host's glibc and must be built on an Ubuntu no newer than the one golems run on.
Third-party actions stay pinned to full commit SHAs.

Log: 2026-11-23 "CI and golems run on Blacksmith runners"

### Agent image cache writes only before any seat

Blacksmith serves `actions/cache` from a branch-scoped cache, and only the save step right after preparation, before any seat, writes the agent image.
Sticky disks and Blacksmith's Docker layer builder commit state after the agent ran, so golem jobs do not use them for the image.

Log: 2026-11-23 "CI and golems run on Blacksmith runners"

### Releases use upstream `actions/checkout`; v22 is burned

`useblacksmith/checkout` replaces an annotated tag with a lightweight ref, so the notes would be the commit message; `release.yml` uses upstream `actions/checkout`, while CI and deploy keep `useblacksmith/checkout` for branches.
Release tags are immutable and the workflow runs from the tagged commit, so v22 stays unpublished and the next release after it is v23.

Log: 2026-11-23 "Releases check out with upstream `actions/checkout`; v22 is burned"

### Release bytes are verified, never rewritten

A release tag is never rewritten and its assets never replaced; hosting is mutable, so integrity is point-in-time and never described as build attestation or immutable hosting.
The ops runner workflow checks a fixed reviewed SHA-256 before credential selection, in addition to the sidecar.

Log: 2026-11-19 "Keep the native Mise pin compatible with the current target"; 2026-11-19 "Consume reviewed native bytes despite mutable release hosting"

### Runners ship before routers that need them

A Worker that emits new request fields or choices deploys only after the runner release accepting them is pinned in ops, because the router has no runner-version gate and an older runner rejects the dispatch after it started.

Log: 2026-11-11 "Seat fallback runs on a `pi` harness"; 2026-11-12 "No Copilot provider; harness-only fallback"; 2026-11-13 "Dashboard last result and run boxes"

### Golems follow the target's Mise minimum

Renovate raises Toktrak's Mise `min_version` almost daily, and each bump broke its golems until a release and an ops pin caught up, so Oliver decided that the runner reads the target's `min_version` and, when it is newer than the image's Mise, downloads that Mise release, checks its published SHA-256 and uses it in every container of the run.
Until that runner change ships, golems' Mise moves only in this order: the image's version assertion, CI's container-test Mise and the supported-minimum fixture, a verified release, the ops runner pin with its Mise version in the same commit, and the `ops/` template; it is 2026.11.14 since runner v24.
A target's minimum is never met by a config bypass or a downgraded target requirement; deploy and release lint pins that do not prepare target images stay unchanged.

Log: 2026-11-24 "Golems follow the target's Mise; 2026.11.14 meanwhile"; 2026-11-23 "Golems carry Mise 2026.11.12"; 2026-11-18 "Preserve native skills and honor the target's Mise minimum"

### Workers deploy by direct REST upload

Deployment uses the Workers REST multipart upload, so the Rust SDK build artifacts need no second CLI build format; production deployment happens only in CI after validation.

Log: 2026-11-03 "Slice 1 implementation"

### Acceptance promises are written before the work

Promises for all slices are written before handoff, so the agent never writes promises it then checks.
Promise kinds are prerequisite, check and evidence: prerequisites are exempt from red before green, and behavior `accept.nu` cannot drive is proven in `EVIDENCE.md`.
Judge changes need Oliver's authorization and land as their own commits with red evidence; he authorized the harness-only `fallback` scenario and the no-auto, explicit-request onboarding scenario.

Log: 2026-11-03 "Hardening before handoff"; 2026-11-12 "No Copilot provider; harness-only fallback"; 2026-11-18 "Starter PRs require an explicit UI request"

## Tooling and tests

### Line coverage ratchet per crate

`mise run ci` runs the Rust tests once under `cargo-llvm-cov` 0.9.2 and fails when a crate falls below `coverage.toml`; the `llvm-tools` component joins the pinned 1.99.1 toolchain.
Each crate's filter is anchored at the repository root, because an unanchored `runner/` also matches `/home/runner`.
Only Linux enforces it, since Windows skips `cfg(unix)` code; the baseline router 98.4%, runner 88.9%, xtask 46.7% was reproduced on two Blacksmith machines.
The baseline has no margin and also gates deploy, release and the weekly run; a flaky line is fixed in its test.

Log: 2026-11-24 "Line coverage ratchet"

### Development goals realised as Oliver decided

Branch coverage is dropped from the goals because the pinned stable toolchain rejects its experimental option.
The mdBook Pages site, `insta` for human-reviewed text, `proptest` for parsers and seat names, and parser tests on captured, scrubbed App deliveries are goals Oliver wants realised before 1.0.
Golem examples in the documentation are parsed by `parse_golem` in `router/tests/docs.rs`.

Log: 2026-11-24 "Development goals re-evaluated, then realised"

### Rust is pinned by `rust-toolchain.toml`

Mise does not manage Rust because its `RUSTUP_TOOLCHAIN` would override the pin, and Clippy's `cargo` group is dropped because its findings are mostly in dependencies.

Log: 2026-11-03 "Hardening before handoff"

### Windows Worker suites run in WSL

Oliver chose WSL over skipping oversized-body assertions or draining unauthenticated bodies; DEVELOPMENT.md describes the setup.
Windows Miniflare resets on oversized requests are transport nondeterminism and are not fixed by draining, retries, removed assertions or dependency changes.

Log: 2026-11-17 "Run Windows Worker suites in WSL"; 2026-11-03 "Container compatibility diagnosis"

### Dependencies and their reasons

Every crate's reason is in its log entry; notably serde-saphyr for strict bounded YAML, `dryoc` 2.0.0 instead of `crypto_box` to avoid duplicate crates, and `ignore` for gitignore matching instead of a hand-written subset.
Duplicate-dependency denial stays with exact-version exceptions in `deny.toml`; CDLA-Permissive-2.0 is allowed only for webpki-roots 1.0.11, whose license ships in releases.
Oliver approved ignoring RUSTSEC-2026-0543 (`chrono`, unmaintained, reached only through the wasm `worker` crate).

Log: 2026-11-03 "Slice 1 implementation"; 2026-11-11 "Seat registration page and refreshing Codex seats"; 2026-11-20 "One definition per scan bound; ignored outputs never publish"

### Positive control uses a deterministic artifact

Oliver approved a deterministic `printf` command as the positive control's artifact producer instead of prose-directed punctuation; byte assertions, verification, protection and publication criteria stay intact.

Log: 2026-11-04 "Protected-path refusal needs an explicit disposition"

### Proof helpers bind to real identities

Observers bind an explicit check to immutable request and comment identity and use actual workflow timestamps, because CheckRuns have no `created_at` and GitHub's default filter hides older same-name checks.
Delivery IDs are kept as exact decimal strings, since they exceed JavaScript's safe integer range.

Log: 2026-11-04 "Live canary identities and cold-start admission gap"; 2026-11-03 "Deployed Slice 1 preflight"

## Open gaps and proposals

### Blacksmith container caching is an open risk

Blacksmith's Docker container caching keeps an organization-wide image store and merges pulled images back after each job, including golem jobs after the agent ran.
A digest-pinned base image does not protect a tampered local store, so this stays open until caching is turned off for the organization or the risk is accepted.

Log: 2026-11-23 "CI and golems run on Blacksmith runners"

### No warning before a sign-in expires

Warning before expiry would need the runner to record expiry or refresh results; expiry is never inferred from a secret's update time.

Log: 2026-11-23 "Seat health from audits; runs before audits counted apart"; 2026-11-17 "Organization dashboard and durable sanitized audits"

### Seats are readable inside the sandbox

The seat is readable by the agent and by repository hooks, extensions and MCP servers, and `/golem-pi` and `/golem-codex` are host bind mounts without a size bound.
Pi decodes token payloads with `atob`, which rejects base64url, so some tokens could fail inside Pi (not observed); Pi has no turn limit beyond the timeout.

Log: 2026-11-13 "Pi on OpenAI Codex seats (runner only)"; 2026-11-11 "Seat fallback runs on a `pi` harness"

### Session cookie lacks the `__Host-` prefix

`golem_session` is not a `__Host-` cookie, so a sibling `workers.dev` subdomain could plant one; its MAC still prevents forgery.

Log: 2026-11-11 "Seat registration page and refreshing Codex seats"

### Harness configuration gaps

Claude plugins install into the `noexec` home and may not run; read-only runs have a `noexec` `/tmp`, so `npx` and `uvx` MCP servers fail there.
Whether Codex needs `[features].hooks = true` for project hooks is unverified, and hook and MCP output counts against the 1 MiB output bound.

Log: 2026-11-12 "Harnesses run the repository's own configuration"

### Dashboard gaps

Concurrent first pushes could create two dashboards; coordinator-only outcomes and the disabled v1 protocol do not update it, and coordinator-written stopped or blocked summaries lack the fallback line.
Whether a web-interface tick delivers `changes.body.from`, and whether nested task-list boxes stay clickable, needs a live tick.

Log: 2026-11-11 "Onboarding and dashboard on push"; 2026-11-13 "Dashboard last result and run boxes"; 2026-11-11 "Seat fallback runs on a `pi` harness"

### Queue and discovery gaps

A stopped row whose target repository is gone keeps failing its check update and holds its seat.
A duplicate queued check for one delivery is an unresolved creation or journal gap; preserve identities and never delete, reset or terminalize by hand.
Large or changing installations can exceed daily scan throughput, and the full 512/513-source and full-index-capacity matrices are unverified.

Log: 2026-11-06 "Stop fence for dispatched rows that never bound"; 2026-11-05 "Event context, delegated commits, separate feedback authority"; 2026-11-04 "Guarded Slice 2 implementation, live proof pending"; 2026-11-04 "Cold-start bootstrap and orphan recovery correction"

### Proposal: record full input identity before download

Recording authenticated full-input identity before the runner download would link a startup failure to its request; it cannot be backfilled and is not implemented.

Log: 2026-11-05 "Startup download failure without run-linked inputs"

### Unmeasured workspace scan memory

Peak memory with agent and verifier workspaces both held, each up to 512 MiB, is unmeasured.

Log: 2026-11-20 "One definition per scan bound; ignored outputs never publish"

## Log

New dated entries go here, newest first.

### 2026-11-24: Ops leftovers deleted; two proof runs lost with them

As Oliver chose for the 1.0 cleanup, the five `probe/*` branches of the closed ops pull requests and the four orphaned, disabled diagnostic workflows were deleted; a workflow without its file disappears only with its runs, so its nine runs were deleted too.
The deletion was bounded by workflow state, not by what cited the runs, and two of the runs were cited as proof in the evidence log: run 42013269480 ("Writable protocol pre-rollout validation") and run 42040986651 ("Context/delegation candidate Linux and provider controls").
Their logs are irreversibly gone; the entries are annotated, and the commits they ran, `e8e7ee5` and `229f2ca`, remain reachable in the ops repository.
The branch refs stay recoverable from the pull requests' `refs/pull/<n>/head`; the runs do not.
Before deleting runs or other records, search the repository for their identifiers first.

### 2026-11-24: Memory kills detected from events, not only the sticky flag

The container test's survived shape saw the hog exit 137 while Docker recorded `OOMKilled false` (CI runs 45270797820 and 45400895299, two of about ten), which settled the open question of PR #76: Docker's record can miss a memory kill of a child when the container survives.
Oliver chose that the runner also watches Docker's per-container out-of-memory events during the run and that the test asserts the runner's detection; a test that accepted the missing record or retried until it appeared would hide the gap.
This runner change lands before 1.0; until then trunk `ci` can fail on that shape.

### 2026-11-24: Cleanup before 1.0

Oliver asked to remove the cruft before 1.0 and chose what goes; git history keeps all of it.

- `PLAN.md` is retired because all four slices are accepted; its working rules and acceptance-contract rules moved to DEVELOPMENT.md, and AGENTS.md now describes maintenance instead of the slice loop.
- `accept.nu` and `contract.nuon` changed only their comment pointing to those rules (commit `fix(accept): point to the acceptance rules in DEVELOPMENT.md`); no promise, check or limit changed.
- `HANDOFF.md`, the implementation-time protocol notes, is replaced by the current internals in DEVELOPMENT.md; the dropped parts described the version 1 rollout and module ownership during Slice 2.
- The one-time Slice 2 proof tools `tools/prove-refusals.mjs`, `tools/prove-slice2-outcomes.mjs` and `tools/prove-slice2-pilot.mjs` are removed; the evidence lines that cite them keep their run links.
- The dated logs moved verbatim to `history/`; this file lists the decisions in force by topic, and EVIDENCE.md keeps the evidence promises and current proofs.
