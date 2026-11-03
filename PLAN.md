# isp-golem-xiv plan

Implements [SPEC.md](SPEC.md) in four vertical slices.
Each slice ends with working, deployed behavior that a person can observe on GitHub.
The agent works through all slices without pausing for approval.
It stops only at the [planned stops](#planned-stops), when blocked by something it cannot obtain itself, or when the spec is wrong or contradicted by evidence.
The goal is reached when `mise run accept -- --slice 4` passes and the operator guide is written.

## Ground rules

- **The spec is the contract.** Where it is silent, the agent decides, records the decision in `DECISIONS.md`, and continues.
- **Sandbox first.** All end-to-end testing happens in test repositories in `isp-insoft-gmbh`; real repositories are never touched.
  The agent may create as many test repositories as scenarios need and deletes them when they are no longer needed; the App installation covers every repository, so new ones are installed automatically.
  Their naming and marking are defined in `contract.nuon` under `test_repos`.
  The production App is used directly, so until Oliver lifts the allowlist the router acts only on repositories carrying that topic; events from every other repository are ignored.
- **Deployed means verified.** A slice is done only when `mise run accept -- --slice <n>` passes against the deployed system; see [Acceptance contract](#acceptance-contract).
- **Every slice leaves the system working.** No slice breaks the behavior of a previous one; earlier scenarios are rerun.
- **Tests are part of the slice.** Fast tests cover rules and decisions and run in the code repository's CI; end-to-end scenarios run through `accept.nu` from the agent's machine, because they need seats and private test repositories that public CI must never see.
- **CI deploys.** Releases and deployments happen only in CI; the agent never deploys from its own machine.
- **Progress survives the session.** `STATUS.md` says where the work stands; the agent reads it first and updates it at every milestone, so a fresh session continues instead of starting over.
- **Two repositories.** Code, tests, and releases live in the public code repository; the runner workflow, seats, and every golem run live in the private ops repository. Nothing from a target repository ever reaches the code repository.
- **Pull requests only.** Changes reach the code repository's `trunk` through pull requests with a green `ci` check; the agent merges its own pull requests once they are green. The rules are in `.github/settings.nuon`, applied with `mise run gh-settings`.
- **Releases by tag.** `mise run release -- "<message>"` pushes the next annotated tag `v<n>`; CI validates, builds `dist/` with `mise run dist`, and publishes a GitHub release. Tags are immutable, so the first tag is pushed only after `mise run dist` exists. The ops repository runs a pinned released runner, never an unreleased one.
- **Nothing is half-built ahead of need.** A slice builds only what its scenario requires.

## Development infrastructure

[DEVELOPMENT.md](DEVELOPMENT.md) is the target; it is built up with the slices, not ahead of them.

| When | What |
|---|---|
| Slice 1 | `fmt`, Clippy groups and denied lints, `cargo-deny`, `cargo-machete`, `actionlint`, `zizmor`, `typos`, `rumdl`, `dprint` in `mise run ci`; the xtask; first unit and snapshot tests; the first release |
| from the first code | mutation testing during development, within a measured budget |
| after Slice 1 | the coverage ratchet |
| Slice 3 | property tests for seat names and seat selection |
| after Slice 3 | mdBook site on GitHub Pages with changelog page and CI reports |

## Slice 1: a golem answers a mention

The thinnest path through every part: App, router, runner, seat, harness, and back to GitHub.

**Scenario.**
An organization member comments `@isp-golem-xiv hello` on an issue in a test repository.
👀 appears on the comment, then 🚀 and a check run with seat, model, budget, and log link.
Within minutes `isp-golem-xiv` replies with what the `hello` golem sees.
The check run ends as `replied`.

**In scope.**

- Router and runner in the code repository, with the `ci` and `dist` tasks and a first release.
- Ops repository with the generic runner workflow running that release.
- Router deployed on Cloudflare Workers Free under the worker name and URL in `contract.nuon`, and connected to the App's webhook through `PATCH /app/hook/config` with an App JWT.
- Mention parsing, membership check, golem lookup from `.golems/`.
- One Anthropic seat provisioned directly by the operator.
- Runner that checks out the target, runs the `claude` harness, and posts the reply.
- Refusal replies for non-members, unknown golems, and missing seats.
- Loop protection: events caused by `isp-golem-xiv` never trigger golems.

**Done when.**

- `mise run accept -- --slice 1` passes three times in a row on the deployed system.
- The agent's own tests show that a person without a seat and a signed delivery of a non-member mention are refused as the spec describes.

## Slice 2: a golem changes code

Golems produce pull requests and run without a person present.

**Scenario.**
A test repository has a golem with a `verify` command and a `branch`.
A member mentions it with an instruction; it opens a pull request whose commits pass `verify`, with a footer naming the seat and model.
A second mention continues the same branch instead of starting over.
A scheduled golem runs on its day on its `owner`'s seat.
An event caused by a bot runs a triggered golem on its `owner`'s seat.
`@isp-golem-xiv stop` ends a running golem with the outcome `stopped`.

**In scope.**

- Toolchain preparation from the target's `mise` description.
- Outcomes `changed`, `no_change`, `incomplete`, `blocked`, and `stopped`, each visible where the spec says.
- Branch ownership and continuation; `verify` before publishing.
- Event triggers, daily schedule, and the `owner` rule for runs without a person.
- No overlapping runs of the same golem in the same repository, and none on the same seat.
- Guards: golems never change `.golems/`, workflows, or harness configuration of the target.
- `timeout` and `stop`.

**Done when.**

- Each outcome has been produced at least once in a test repository and shows up as the spec describes.
- A golem that edits a forbidden path is blocked and nothing is published.
- Two simultaneous mentions of the same golem run one after the other.
- The scheduled and the bot-triggered scenario run on the owner's seat.
- All Slice 1 scenarios still pass.

## Slice 3: teams onboard and serve themselves

People enable golems, register seats, and find their way without asking anyone.

**Scenario.**
A fresh test repository without `.golems/` is allowlisted.
`isp-golem-xiv` opens **Enable golems**; merging it creates and pins the **Golems** dashboard.
A member without a seat mentions a golem, follows the link in the refusal, registers a seat, and mentions again successfully.
A member whose only seat is at a different provider than the golem prefers gets a run that says which seat and model it used and why.
A pull request with an invalid golem fails the validation check; after merge the dashboard lists it as disabled.

**In scope.**

- Onboarding pull request with the minimal `_golems.md` and `hello.md`.
- Dashboard issue with all sections from the spec, including run boxes, kept current after every run and every change to `.golems/`.
- `@isp-golem-xiv` help and `@isp-golem-xiv seat`, scoped to where they are asked.
- Seat registration page with encryption in the browser; the router never sees a credential.
- Providers beyond Anthropic with refreshing sign-ins; one seat shared by all compatible harnesses.
- Seat selection and fallback with full transparency, preferred seat per person.
- Validation check for pull requests that change `.golems/`.
- Detecting seats of people who left the organization.

**Done when.**

- The onboarding, registration, fallback, and validation scenarios each pass on the deployed system.
- A refreshing seat survives at least three consecutive runs without registering again.
- Registering again replaces the previous sign-in.
- All Slice 1 and Slice 2 scenarios still pass.

## Slice 4: the organization sees all runs

The admin dashboard from the spec.

**Scenario.**
An organization member opens the admin dashboard, signs in with GitHub, and finds the run that just answered in a test repository, with its seat, model, outcome, duration, and log link.

**In scope.**

- Sign-in for organization members, reusing the seat page's GitHub sign-in.
- Runs, usage, health, seats, and repositories, derived from GitHub and cached only where Workers Free limits require it.
- One audit record per run in the ops repository, in a form the implementation chooses.

**Done when.**

- `mise run accept -- --slice 4` passes, including the evidence for the rendered page.
- All earlier scenarios still pass.

## Acceptance contract

`contract.nuon` holds every promise the deployed system must keep, with an id, its slice, and a guide that tells the agent what to do when it fails.
It is the single source of truth for names, markers, limits, and required App settings; this plan and the spec do not repeat them.
`accept.nu` checks those promises: first static checks of GitHub, the App, both repositories, and the router, then live scenarios in a fresh test repository.
Every failure prints the promise, what was observed, and the next step; treat that output as instructions.

Promises come in three kinds:

- **prerequisite**: provided by Oliver, such as App settings and tokens; when one fails, the agent reports it as blocked.
- **check**: verified live by `accept.nu` against the deployed system.
- **evidence**: behavior `accept.nu` cannot drive, such as a second person, a browser sign-in, or a schedule; the agent proves it with its own tests or a recorded run and lists it in `EVIDENCE.md` as `- <id>: <link or test path> <one line>`.

Promises for every slice are written.
A slice whose promises are missing, or a promise without a check, fails instead of passing silently.

Rules for the judge:

- **Fix, never weaken.** The agent may fix bugs in `accept.nu`, such as a wrong API path, parsing, or flakiness, but never relax a promise.
- **The contract stays.** The agent never adds, removes, or rewords promises; if it believes a promise is wrong, it records a proposal in `DECISIONS.md`, keeps working within the current promise, and reports it at the end.
- **One commit per change.** Each change to `accept.nu` or `contract.nuon` is its own commit `fix(accept): <what>` with the reason and the failing output as evidence, and an entry in `DECISIONS.md`.
- **Red before green.** Every promise of kind check must be seen failing against a system that lacks the behavior before it is seen passing; both outputs go into `DECISIONS.md`. Prerequisites are exempt.
- **Report every change.** The final report lists all changes to the judge, grouped as removed check, raised limit, removed promise, and other.

The agent commits under its own author name `isp-golem-xiv agent`, so `git log --format=%an -- accept.nu contract.nuon` separates its changes from Oliver's.

Usage: `mise run accept -- --slice <n>`; `--preflight` runs only the static checks and the evidence lookup and creates nothing; `--keep` keeps the test repositories.
A full run of `--slice 4` is long, its duration not yet measured, and creates several test repositories; `accept.nu` deletes them afterwards and sweeps leftovers older than a day at start.

## After the slices

The agent writes a short operator guide: how to add a repository, how people register seats, how to read the dashboard, what to do when a run is blocked.
Lifting the allowlist, and with it acting on real repositories, remains a human decision.

## Prerequisites

Secrets live in `~/.secrets/isp-golem-xiv/`, one value per file, described in its `README.md`.
The agent copies them into GitHub and Cloudflare secrets without ever printing them.
Deployment secrets of the code repository go into its `production` environment, which only `trunk` and release tags can use; the deploying job declares `environment: production`, never repository secrets.

- The public code repository `isp-golem-xiv`, created by Oliver with `.github/settings.nuon` applied.
- Oliver's gh sign-in as organization owner, which lets the agent create and delete private repositories, including the ops repository `isp-golem-xiv-ops`.
- The App `isp-golem-xiv`, installed on all repositories, with the events and permissions listed in `contract.nuon`, its private key, its client secret, and its callback URL; its webhook is active.
- A Cloudflare API token for Workers and KV, and the `workers.dev` subdomain named in `contract.nuon`; the Cloudflare MCP for docs and inspection.
- Oliver's account for member scenarios.
  Refusals for non-members are verified with signed webhook deliveries sent to the deployed router, not with a second account.
- Subscription sign-ins for at least two providers, dedicated to testing.

## Planned stops

The agent asks Oliver at these points and otherwise continues; while waiting, it works on whatever does not depend on the answer.

- **Before Slice 3: browser.** A browser sign-in to GitHub for the registration page scenario, either a session cookie or a dedicated Firefox profile; the agent never handles passwords.
- **Before Slice 3: fresh Codex sign-in.** Refreshing sign-ins rotate, so the copy in `~/.secrets/isp-golem-xiv/` is stale after its first use; Oliver signs in again with `codex login` into a fresh folder.
- **Any time: a prerequisite fails.** The guide of the failing promise says what to ask for.
