# Development

Goals for how `isp-golem-xiv` is built.
They describe the target, not today's state; the agent moves the repository toward them and records gaps in `DECISIONS.md`.
Where a goal and a deadline conflict, the goal wins only if the gap would be hard to close later.

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
- Code that handles external input, above all the router, denies `unwrap_used`, `expect_used`, `indexing_slicing`, and `panic` outside tests.
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
| coverage | a diagnostic, not a target | a ratchet: line and branch coverage per crate, kept in a committed baseline file updated by the pull request that changes it, may only drop with a reason in the pull request, e.g. removed code or I/O glue covered by `accept.nu` |
| acceptance | the deployed system | `accept.nu` against `contract.nuon` |

- Tools: the standard test harness, `insta` for snapshots, `proptest` for properties, `cargo-mutants` for mutation, `cargo-llvm-cov` for coverage.
- Test names state the situation and the expectation.
- Parsers of GitHub payloads are tested with real deliveries captured from the App, not hand-written ones.
- A failing test is diagnosed before it is changed; tests are never weakened to get green.
- Fast tests run on every change.

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
- Every routine action is a task: `ci`, `test`, `fmt`, `accept`, `dist`, `release`, `gh-settings`.
- `mise run ci` locally and in CI is the same gate.
- Orchestration is one-way: mise calls the tools, the tools never call mise.

## Tooling

| Tool | Language | Tasks |
|---|---|---|
| xtask crate `x/` | Rust, same lints and tests as the product | `ci`, `test`, `fmt`, `dist` |
| `accept.nu` | Nushell | `accept` |
| `tools/release.nu`, `tools/gh-settings.nu` | Nushell | `release`, `gh-settings` |

- Build logic is Rust so it is tested and linted like the product.
- The judge and the repository administration stay Nushell: they share no code, helpers, or bugs with the product they check or configure.
- mise invokes the xtask with `cargo run -p x -- <task>` and the Nushell scripts with `nu --no-config-file <script>`.
- Today `ci` still runs `tools/ci.nu`; it moves into the xtask when the first Rust code lands, recorded in `DECISIONS.md`.

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
- `SPEC.md` says what it does, `PLAN.md` how it gets built, `DEVELOPMENT.md` how it is developed, `DECISIONS.md` why things are the way they are.
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
- Results are reported where people look: check runs for status, annotations for actionable problems, the job summary for a short human report.
- Artifacts only when data must be downloaded, with a short retention.
- Untrusted pull request code never sees secrets; `pull_request_target` is not used.
- Deployment secrets live in the `production` environment, reachable only from `trunk` and release tags.
- Repository Actions settings, including required SHA pinning and read-only default tokens, are part of `.github/settings.nuon`.
- Nothing from a target repository ever appears in the code repository's CI; end-to-end scenarios run through `accept.nu` outside it.

## Admin dashboard

It is a product feature, specified in [SPEC.md](SPEC.md#admin-dashboard).
