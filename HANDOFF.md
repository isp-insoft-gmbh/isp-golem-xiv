# Router / runner protocol

## Released version 1

Workflow inputs: `repo`, `golem`, `request` (JSON).
Run-name: `${{ inputs.repo }} ${{ inputs.golem }}`.
Ops default branch: `trunk`.
Released runner `v1` accepts protocol version 1 and supports authenticated replies.

Version 1 request fields: `version`, `delivery_id` (UUID), `repository_id`, `repository`, `head_sha`, `golem`, `issue_number`, `comment_id`, `actor_id`, `actor_login`, `seat_login`, `provider`, `harness`, `model` (nullable), `timeout_seconds`, `instructions`, `check_run_id`.
Runner reads `GOLEM_REQUEST`, `GOLEM_REPO`, `GOLEM_NAME`, `GOLEM_APP_PRIVATE_KEY`, `GOLEM_SEAT`, `GOLEM_SEAT_SECRET_NAME`, `GITHUB_RUN_ID`, `GITHUB_RUN_ATTEMPT`.
Agent never receives App key, GitHub credentials, Docker socket, host home or workflow files.
Confirmed container destruction precedes fresh scoped publication credentials.

## Slice 2 version 2

Keep version 1 working throughout deployment/release transition.
Version 2 preserves the common fields and adds:

- `trigger`: `mention` | `event` | `schedule`.
- `branch`: validated owned branch, default `golems/<name>`.
- `verify`: nullable command.
- `owner`: nullable GitHub login from definition.
- `queue_generation`: opaque UUID assigned by coordinator before dispatch.

For version 2, `issue_number`, `comment_id`, `actor_id`, `actor_login` may be null when no corresponding context exists.
Mention requests require real actor/comment identity, organization membership and the actor's seat.
Event/schedule requests may pay from definition owner's seat; scheduled requests require owner.
Actual branch snapshot is selected after queue admission, continuing existing owned branch when present.
Runner independently validates target topic, configuration at `head_sha`, branch/verify/owner mapping and context before execution.

## Rollout compatibility

Private ops variable `GOLEM_PROTOCOL_VERSION=2` activates Slice 2 routing and scheduling.
Absent variable preserves released version 1 mention behavior and ignores new event/schedule triggers.
Read this variable through the App's metadata API, never subscription values.
Publish the backward-compatible new runner and pin ops to it before setting this variable.
Coordinator must conservatively wait for any older, unknown in-progress/queued runner workflows before granting execution, so activation cannot overlap a legacy run on the same resources.

## Queue and coordination

A single SQLite Durable Object `Coordinator` per organization arbitrates BOTH seat and repository/golem resources together.
No Actions job is created merely to wait.
No default Actions concurrency groups.
Authoritative queue is a bounded, versioned JSON index on private ops branch `golem-state`; update through Contents API with previous blob SHA (conflict-safe).
Full immutable request bodies live in private annotated tags `golem-request/<request-id>`; index keeps small ownership/state records.
SQLite is a rebuildable projection, never the only grant record.
Persist reservation and generation before dispatch; ambiguous dispatch retains reservation, never blind retry.
FIFO means durable admission order among requests sharing either resource; unrelated resources can advance.
Bound capacity explicitly; reject admission before acknowledgment if full, never evict accepted pending work.
Retire terminal records from current index only after durable terminal transition; Git history and request tags retain audit provenance.

Internal public route: `POST /internal/queue`.
JSON body: `{action, request_id, generation, run_id, attempt, timestamp_ms, ...}`.
`run_id`, `attempt`, `timestamp_ms` are JSON numbers bounded to JavaScript safe integers; attempt must be 1.
Header `X-Golem-Signature-256`: `sha256=<hex HMAC-SHA256>` over exact body bytes, key = trimmed raw App PEM string encoded UTF-8.
Normalize CRLF to LF before trimming the PEM in BOTH router and native client.
Validate timestamp within 60 seconds.
Only trusted native supervisor holds this key; no coordinator auth enters agent/verify containers.
Rust edge validates signed internal requests before forwarding to the DO.
DO binding itself is not an unauthenticated public endpoint.

Actions:

- `bind`: require matching request/generation; durably bind one real run ID/attempt; return `{state:"granted"}` only for owner.
- `complete`: require matching request/generation/run/attempt, confirmed container destruction and completed publication; durably release both resources, advance next eligible request.
- `stop`: router-owned operation; mark stop request, cancel bound GitHub run, retain resources until completion is established.

Runner binds before agent access and never retries execution on denied/uncertain grant.
Native client sends signed `complete` only after agent and verifier destruction, and publication.
A missing heartbeat, timer, cancellation acknowledgment, 404 or generation number is not proof execution ended.
DO alarms reconcile known GitHub terminal workflow states, repair stopped checks, and advance pending requests.
Late completion from old owner cannot release successor.
Periodic cron wakes reconciliation and real daily scheduling.

## Module ownership during implementation

- `runner/`: native v1/v2 validation, branch/verify/change publication, incomplete result, toolchain prep, queue client, container lifecycle.
- `router/src/lib.rs`, `router/src/edge.rs`, router policy tests: v2 mentions/events/scheduling/stop, owner selection, queue handoff/internal auth, worker scheduled entrypoint.
- `router/src/coordinator.js`, coordinator runtime tests: GitHub-backed FIFO journal and SQLite coordinator.
- Parent: workspace deps/lock integration, Worker packaging/export/bindings/migration/cron deployment, CI integration, evidence/status/decisions, GitHub publication.

Coordinator JS exports class `Coordinator` from `router/src/coordinator.js` and imports `appJwt` from `./runtime.js`.
It exposes `fetch` actions `enqueue` (body `{request}`), `bind`, `complete`, `stop` (body `{repository, issue_number}`), `tick`.
Rust policy calls the DO binding directly for enqueue/stop/tick; signed external native callbacks use `/internal/queue`.
Enqueue returns `{state:"dispatched"|"pending", request_id}`; coordinator owns dispatch and rocket reaction.
Worker bindings remain `APP_PRIVATE_KEY`, `WEBHOOK_SECRET`, `VERSION`, plus SQLite DO `COORDINATOR`.
