# Development

Goals for how `isp-golem-xiv` is built.
They describe the target, not today's state; the agent moves the repository toward them and records gaps in `DECISIONS.md`.
Where a goal and a deadline conflict, the goal wins only if the gap would be hard to close later.

## Working rules

- **Two repositories.** Code, tests and releases live in this public repository; the runner workflow, seats and every golem run live in the private ops repository `isp-golem-xiv-ops`. Nothing from a target repository ever reaches this repository.
- **Pull requests only.** Changes reach `trunk` through pull requests with a green `ci` check on the exact head; the rules are in `.github/settings.nuon`, applied with `mise run gh-settings`. The ops repository has no branch protection, so the same gate is applied by hand there.
- **CI deploys.** The router is deployed and releases are published only by CI; nobody deploys from a workstation.
- **Released runners only.** Ops pins a released runner by URL and SHA-256, never an unreleased build; [OPERATIONS.md](OPERATIONS.md#release-and-verify) has the order of a runner change.
- **Test repositories for end-to-end work.** `accept.nu` creates and deletes test repositories named as `contract.nuon` defines under `test_repos`; a run that spends a seat on a real repository, such as a canary, needs Oliver's approval and pass criteria written before it starts.
- **Secrets stay secret.** They live in `~/.secrets/isp-golem-xiv/`, described in its `README.md`, and are copied into GitHub or Cloudflare secrets without being printed; deployment secrets sit in the `production` environment.

## Code

The style is [TigerStyle](https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md), applied to Rust.

- **Safety first, then performance, then developer experience.**
- **Assert everything internal.** Arguments, return values, invariants, and forbidden states are asserted; external input is validated and returns errors instead.
- **Bound everything.** Every loop, queue, retry, timeout, body, file, and batch has an explicit limit.
- **No recursion, no unbounded work.** Control flow is simple and visible.
- **Push control flow up, data transformation down.** Nondeterministic I/O sits at the edges, around deterministic logic.
- **Small interfaces with defined failure behavior.**
- **Precise names.** Nouns and verbs without abbreviations; qualifiers as suffixes, e.g. `timeout_ms_max`.
- **No dependency without a reason.** Standard library first; every crate is a decision recorded in `DECISIONS.md`.
- **Documentation comments only where they add something** the signature does not say; comments explain why, not what.
- **No `unsafe`.**

## Linting

- Rust edition 2024, toolchain pinned to an exact version in `rust-toolchain.toml` with `clippy`, `rustfmt`, and the `wasm32-unknown-unknown` target; rustup reads it, mise does not manage Rust.
  A toolchain bump is its own pull request, so new lints never break `trunk` unannounced.
- `rustfmt` with defaults.
- Clippy groups `all`, `pedantic`, `suspicious`, `complexity`, `perf`, and `style` as warnings; CI turns every warning into an error.
- `unsafe_code` is forbidden; `let_underscore_drop` and `irrefutable_let_patterns` are denied.
- Every crate denies `unwrap_used`, `expect_used`, `indexing_slicing`, and `panic` outside tests, because the router, the runner and the xtask all handle external input.
- A lint is only silenced at the narrowest scope, with a reason in a comment.
- Beyond Rust, one tool per class of bug, all installed by mise, all part of `mise run ci`, all blocking:

| Tool | Catches |
|---|---|
| `cargo-deny` | unwanted licenses, duplicate or banned crates, unknown sources; advisories block only pull requests that change `Cargo.lock`, a scheduled run reports new ones |
| `cargo-machete` | dependencies that are not used |
| `actionlint` | broken workflow files |
| `zizmor` | insecure workflows: script injection, unpinned actions, broad permissions |
| `typos` | typos in code, identifiers, and documentation |
| `rumdl` | Markdown, configured in `.rumdl.toml` |
| `dprint` | TOML formatting |
| `nu-check` | Nushell scripts that do not parse |

## Router on Cloudflare Workers

The router runs as WebAssembly, where some of the standard library fails at runtime and a panic loses the request.

- Logic lives in a crate that compiles and is tested on the host; the worker crate only translates between Workers and that logic.
- CI runs Clippy for `wasm32-unknown-unknown` and builds the worker on every pull request.
- Time comes from the Workers runtime at the edge and is passed in; `std::time::Instant` and `SystemTime` are never used in wasm code.
- External input never panics; it becomes an error answer.
- Webhook deliveries are deduplicated by `X-GitHub-Delivery`, because a redelivered event would otherwise start and charge a second run.
- Slow work never delays the webhook answer.

## Tests

Tests protect behavior, not construction history.

| Kind | Used for | Goal |
|---|---|---|
| unit | rules, decisions, parsing, limits | every branch that could regress unnoticed |
| snapshot | stable text a human reviews: replies, check run summaries, dashboard Markdown | every changed snapshot is reviewed and traced to code |
| property | parsers, the seat-name mapping, fallback selection, anything with an input space | invariants instead of hand-picked examples |
| mutation | the code being written, while it is written | stronger tests; never a gate |
| coverage | a diagnostic, not a target | a ratchet: line coverage per crate (the pinned stable toolchain rejects branch coverage), kept in `coverage.toml` as Linux CI measures it and updated by the pull request that changes it, may only drop with a reason in the pull request, e.g. removed code or I/O glue covered by `accept.nu` |
| acceptance | the deployed system | `accept.nu` against `contract.nuon` |

- Tools: the standard test harness, `insta` for snapshots, `proptest` for properties, `cargo-mutants` for mutation, `cargo-llvm-cov` for coverage.
- Test names state the situation and the expectation.
- Parsers of GitHub payloads are tested with real deliveries captured from the App, not hand-written ones.
- A failing test is diagnosed before it is changed; tests are never weakened to get green.
- Fast tests run on every change.

On Windows, `mise run test-worker` builds the Worker natively, then runs the unchanged workerd suites in the default WSL distribution.
That distribution needs `mise` on its executable search path, this repository trusted, and its mise tools installed.
WSL failures fail the task; there is no automatic native retry.
Set `GOLEM_WORKER_TESTS_NATIVE` for a native diagnostic run, where oversized uploads can reset instead of returning 413.
Linux CI runs the same suites natively and remains the merge gate.

### Mutation testing

Mutation testing improves tests during development; it is never a gate and has no target score.
Principles from pitest ([Don't let your code dry](https://blog.pitest.org/dont-let-your-code-dry/)) apply to `cargo-mutants`, with one Rust constraint: every mutant is a rebuild, so compute and disk grow fast.

- **Mutate while the code is wet.** Run it on what is being changed, during the change, and act on the result right away.
- **Smallest scope that answers the question.** Only the files or functions just touched, never the whole workspace by default.
- **Cheapest useful run.** Only the tests that cover the mutated code, tight timeouts, incremental runs that skip mutants already caught, bounded parallelism, builds reused instead of copied where the tool allows.
- **Observe resources.** Every run records wall time, CPU time, and peak disk use; the agent keeps runs within a budget it measures and writes down, and cleans up build output afterwards.
- **A survivor is a question, not a failure.** Answer it with a better test, simpler code, or a short reason to leave it.
- **CI stays optional.** At most an informational run on changed lines of a pull request, within a hard time budget, never blocking a merge.

## Mise

Mise is the single entry point for people, the agent, and CI.

- Every tool and version comes from `mise.toml`; nobody installs tools by hand.
- Every routine action is a task: `ci`, `test`, `test-worker`, `test-agent`, `coverage`, `fmt`, `worker`, `dist`, `deploy`, `accept`, `release`, `gh-settings`.
- `mise run ci` locally and in CI is the same gate.
- Orchestration is one-way: mise calls the tools, the tools never call mise tasks.
  The Windows xtask uses WSL `mise exec` only to initialize the Linux environment for the unchanged Worker suites.

## Tooling

| Tool | Language | Tasks |
|---|---|---|
| xtask crate `x/` | Rust, same lints and tests as the product | `ci`, `test`, `test-worker`, `coverage`, `fmt`, `worker`, `dist` |
| `accept.nu` | Nushell | `accept` |
| `tools/release.nu`, `tools/gh-settings.nu`, `tools/deploy.nu`, `tools/agent-container-test.nu` | Nushell | `release`, `gh-settings`, `deploy`, `test-agent` |

- Build logic is Rust so it is tested and linted like the product.
- The judge and the repository administration stay Nushell: they share no code, helpers, or bugs with the product they check or configure.
- mise invokes the xtask with `cargo run -p x -- <task>` and the Nushell scripts with `nu --no-config-file <script>`.
- The xtask's `ci` calls `tools/ci.nu` only to parse-check the Nushell scripts and NUON data, which needs Nushell itself.

## Commits and pull requests

- A commit subject is short, concrete, and imperative, without a final period, e.g. `fix(router): ignore events sent by the App`.
- One coherent change per commit; a body only when the subject cannot carry the reason.
- A pull request explains context, the problem, why this change, and the outcome, in prose; it does not restate passing checks.
- Tone is plain, direct, and technical, without marketing or emoji.
- No AI attribution or trailers.
- Pull requests are squashed or rebased; `trunk` stays linear.

## Changelog and releases

- A release is one intent: `mise run release -- "<message>"` pushes the next annotated tag `v<n>`, counting up from `v0`, with that message.
- CI validates the tagged commit, builds `dist/`, and publishes a GitHub release.
- Release notes are generated from the tag message, merged pull requests, and commits not covered by them.
- There is no `CHANGELOG.md` in the repository; release notes live in GitHub releases, and the Pages site builds its changelog page from them.
- Released artifacts are immutable; the ops repository pins a version.

## Documentation

- `README.md` says what it is and how to start in under a minute.
- `CONTRIBUTING.md` gets a new developer from a clone to a merged change.
- `SPEC.md` says what it does, `DEVELOPMENT.md` how it is developed, `OPERATIONS.md` how it is run, `DECISIONS.md` which decisions are in force, `EVIDENCE.md` what proves the evidence promises, and `STATUS.md` where things stand.
- `history/` keeps the dated decision and evidence logs; new decisions get a dated entry under "Log" in `DECISIONS.md` and a topic entry when they change a rule.
- User documentation is written for the people who meet golems on GitHub: how to enable, write, run, and pay for golems.
- Examples in documentation are tested: Rust examples as doc tests, golem examples by the product's own golem parser.

## GitHub Pages

The code repository publishes an [mdBook](https://rust-lang.github.io/mdBook/) site from `trunk`, built with mise:

- user documentation,
- the latest release and its notes,
- reports from CI that are worth reading without downloading anything, such as coverage and mutation results.

## CI and reports

- Each workflow gets only the permissions it needs.
- Every external action is pinned to a full commit SHA, with its tag in a comment.
- Every job has a measured `timeout-minutes`.
- Superseded pull request runs are cancelled; publication and deployment runs are not.
- Results are reported where people look: check runs for status, the failing step's log for the cause, and the Pages site for reports worth reading.
- Artifacts only when data must be downloaded, with a short retention.
- Untrusted pull request code never sees secrets; `pull_request_target` is not used.
- Deployment secrets live in the `production` environment, reachable only from `trunk` and release tags.
- Repository Actions settings, including required SHA pinning and read-only default tokens, are part of `.github/settings.nuon`.
- Nothing from a target repository ever appears in the code repository's CI; end-to-end scenarios run through `accept.nu` outside it.

## Acceptance contract

`contract.nuon` holds every promise the deployed system keeps, with an id, its slice and a guide that says what to do when it fails; it is the single source of truth for names, markers, limits and required App settings.
`accept.nu` checks those promises: static checks of GitHub, the App, both repositories and the router first, then live scenarios in fresh test repositories.
Every failure prints the promise, what was observed and the next step.

Promises come in three kinds:

- **prerequisite**: provided by Oliver, such as App settings and tokens; a failing one is reported as blocked.
- **check**: verified live by `accept.nu` against the deployed system.
- **evidence**: behavior `accept.nu` cannot drive, such as a second person, a browser sign-in or a schedule; it is proven by a test or a recorded run listed in `EVIDENCE.md` as `- <id>: <link or test path> <one line>`.

Rules for the judge:

- **Fix, never weaken.** Bugs in `accept.nu`, such as a wrong API path, parsing or flakiness, are fixed; promises are never relaxed.
- **The contract changes only with Oliver.** Promises are added, removed or reworded only with his approval; a promise believed wrong becomes a proposal in `DECISIONS.md`.
- **One commit per change.** Each change to `accept.nu` or `contract.nuon` is its own commit `fix(accept): <what>` with the reason and the failing output, and a `DECISIONS.md` entry.
- **Red before green.** A new check is seen failing against a system without the behavior before it is seen passing.

Usage: `mise run accept -- --slice <n>`; `--preflight` runs only the static checks and the evidence lookup and creates nothing; `--keep` keeps the test repositories.
A full `--slice 4` run spends about a dozen seat runs and deletes its test repositories afterwards; it sweeps leftovers older than a day at start.

## Internals

The router dispatches one `runner.yml` workflow run per golem run in the ops repository, with inputs `repo`, `golem` and `request` (JSON) and the run name `<repo> <golem>`.
The router sends request version 2; the runner still accepts version 1, which carries no trigger, branch, verify or owner.
Version 2 adds `trigger` (`mention`, `dashboard`, `event`, `schedule`), the owned `branch`, a nullable `verify`, a nullable `owner` and the coordinator's opaque `queue_generation`; comment and actor fields are null when no such context exists.
The runner validates the request against the golem definition at `head_sha` and recomputes the seat choice before any container starts.

One SQLite Durable Object `Coordinator` (`router/src/coordinator.js`) arbitrates seat and repository/golem resources together, so queued runs use no Actions runner.
The authoritative queue is a bounded JSON journal `queue.json` on the ops branch `golem-state`, written with compare-and-swap on its blob SHA; immutable request bodies live in annotated `golem-request/<id>` tags of a blob, and SQLite is a rebuildable projection.
The Durable Object answers `enqueue`, `bind`, `complete`, `stop` and `tick`; the native runner reaches `bind` and `complete` through `POST /internal/queue`, signed with `X-Golem-Signature-256` (HMAC-SHA256 over the exact body, keyed with the trimmed App PEM) and a timestamp within 60 seconds.
The runner binds before any agent access and sends `complete` only after agent and verifier containers are destroyed and publication is done; reconciliation then writes the run's `golem-admin/<id>` audit and retires the row with a `golem-terminal/<id>` receipt.

Worker bindings are `APP_PRIVATE_KEY`, `WEBHOOK_SECRET`, `VERSION` and the Durable Object `COORDINATOR`.

## Admin dashboard

It is a product feature, specified in [SPEC.md](SPEC.md#admin-dashboard).
