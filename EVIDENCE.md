# Evidence

Proofs for the evidence promises in [contract.nuon](contract.nuon) and for the system as it runs now.
Dated proofs up to 2026-11-24 are in [history/evidence-log.md](history/evidence-log.md); a new proof gets a dated entry under "Log" at the end of this file.

## Evidence promises

`accept.nu` reads these lines: one per evidence promise, each with a recorded run or a test.

- schedule-owner: [genuine scheduled run 41800937735](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41800937735) `router/tests/worker.mjs` covers daily occurrence identity, owner selection, restart and duplicate prevention; real Cloudflare cron discovered the fresh sandbox, completed its only scheduled check `126476131244` for delivery `479fffd1-0986-c3b9-1fff-69f0544c6caa` as no_change on `bugabinga/anthropic` with observed `claude-sonnet-5-5`, null human context, reason and no unsolicited comments; full real outcome-proof process exited 0.
- incomplete: [timeout run 41792613276](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41792613276) exhausted its 120s budget, completed check `126448315273` as neutral/incomplete and published verified `FIRST` content in commit `94b851f29f982fb194d8ba844007b85f7834dfe8`; [continuation 41793038096](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41793038096) completed check `126450510319` as changed, preserved that exact first line and appended `SECOND` in child commit `b4f48c72a8ddbaa64c200205bf29682c6bd4f02a` on the same [PR #3](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e/pull/3), with owner seat/model footers and matching immutable request/terminal bindings verified by the Slice 2 proof tool `tools/prove-slice2-outcomes.mjs` (removed on 2026-11-24, kept in git history).
- bot-owner: [real owner-seat run 41792488962](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41792488962) `router/tests/worker.mjs` covers the owner-seat decision; actual GitHub Actions bot created [issue #1](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e/issues/1), immutable delivery `8f3ad966-5b73-5975-cfe3-86c919c853d0` binds check `126448312133` to actor `github-actions[bot]` and owner `bugabinga`, completed no_change on `bugabinga/anthropic` using Claude `claude-sonnet-5-5`, with a reason and no response comment.
- refused-no-seat: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613042205) the refusal proof tool `tools/prove-refusals.mjs` (removed on 2026-11-24, kept in git history) cloned an actual App delivery, signed a simulation for member `akonczalla` without a seat, observed the App's `no-seat` reply and registration link, with zero matching runner runs.
- seat-registration: [first run on the registered seat 43202580042](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43202580042) Oliver signed in on the deployed seats page in a separate Firefox profile and registered a fresh golem-only Codex sign-in; the browser sealed it and the router relayed only ciphertext, creating `SEAT_BUGABINGA__OPENAI_CODEX` at 2026-11-12T10:39:09Z (secret metadata only); the next mention, comment `6821378271`, ran on it: check `131129389837` replied, `seat: bugabinga/openai-codex`, `harness: codex`. `router/tests/worker.mjs` covers callback, session, CSRF and exact ciphertext relay, and a libsodium cross-check covers the browser encryption.
- seat-refresh: [refreshing run 43206088999](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43206088999) Codex 0.167.1 refreshes when the access token is about to expire, not by `last_refresh` age, so the seeded seat had only its access-token `exp` claim set an hour in the past (and `last_refresh` 30 days old) and was stored via the API at 11:03:14Z. Run 1 (check `131140812786`, replied) logged `golem seat: refreshed`, and the stored seat's `updated_at` moved to 11:08:56Z. The next two runs, [43206718988](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43206718988) (check `131143483988`) and [43207333628](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43207333628) (check `131145603414`), replied on the refreshed seat with `golem seat: unchanged`.
- seat-replace: [first run on the replaced seat 43206088999](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/43206088999) `router/tests/worker.mjs` registers the same provider twice through the page and asserts the second write replaces the same secret name with the new ciphertext (`replaced: true`); live, `SEAT_BUGABINGA__OPENAI_CODEX` created at 10:39:09Z was replaced at 11:03:14Z (metadata `updated_at` > `created_at`), and every later run used the replacement.
- seat-left-org: `router/tests/policy.rs` (`seat_holders_map_secret_names_to_members_and_report_people_who_left`, `dashboard_lists_sections_golems_disabled_and_seats_privately_only`) and the onboarding scenario in `router/tests/worker.mjs` render the private dashboard's Seats section from ops secret names matched to current members, listing an unmatched `SEAT_GONE_PERSON__OPENAI_CODEX` under "Seats of people who left the organization" and never showing non-seat secret names or values; public repositories list no seats.
- admin-page: [recorded live Firefox session](https://github.com/isp-insoft-gmbh/isp-golem-xiv/blob/0851629c8716f9cd32ecb117d67b8348c0722306/evidence/admin-page-20261117.json) Oliver completed GitHub sign-in as `bugabinga`; the deployed page populated Runs, Usage, Health, Seats and Repositories, and live pagination loaded a second repository and another runs page; screenshot inspected privately, with historical gaps and unknown expiry explicitly visible; this proves rendering, not complete Slice 4 acceptance.
- refused-not-member: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613046125) The same harness signed the recorded delivery as verified non-member `octocat`, observed the App's `not-member` reply, with zero matching runner runs.

## Acceptance

The live `mise run accept -- --slice 4 --keep` on deployed `87d2024` (trunk CI 45172765960, deploy 45172766042, `/health` exact) exited 0 with all 47 gates ok, covering every Slice 1 to 4 promise in `contract.nuon`; it ran 11 golem runs on ops `4078fa5` with runner v20 on 2026-11-23.

## Current runner

Runner v24 is pinned in ops `0caa920e` with Mise 2026.11.14; its [integrity record](evidence/native-v24-integrity-20261124.json) checks the asset against GitHub's digest, the sidecar and `SHA256SUMS` and finds the embedded image assertion for Mise 2026.11.14 once.
On it, a Claude Code canary on the Anthropic seat (ops run 45399130383) passed preparation, agent, verification and all five lifecycle markers, and replied with its nonce; on v23, the scheduled `security` golem (ops run 45297866550) published Toktrak pull request #175, and a Pi-on-Codex canary (ops run 45358172251) replied with its nonce.
The dated detail of these runs is in the log.

## Coverage

Two `ci` runs of commit `2ff1fa3` on different Blacksmith machines (run 45377744635, attempts 1 and 2) measured router 98.4%, runner 88.9% and xtask 46.7% line coverage, exactly `coverage.toml`.

## Log

New dated proofs go here, newest first.

### 2026-11-26: Local mdBook preview

Mise built the book on Windows with mdBook 0.5.4, the published v25 release notes, and Linux line coverage from [trunk CI run 45789314401, attempt 1](https://github.com/isp-insoft-gmbh/isp-golem-xiv/actions/runs/45789314401/attempts/1): router 98.4%, runner 88.9%, xtask 46.6%.
An offline URL-resolution audit used the `/isp-golem-xiv/` prefix and found no broken local references across 11 output HTML pages, including 51 HTML fragment targets.
The external check returned HTTP 200 for 19 GitHub URLs and GitHub sign-in redirects for Seats and the organization dashboard; authenticated flows were not exercised.
The reports chapter was inspected in headless Chrome.
The focused router documentation test passed both cases in WSL and requires a parseable example from the book.
The Pages workflow passed actionlint and an offline zizmor audit; it was not run.

### 2026-11-26: Captured App webhook parser fixtures

Captured four original deliveries on 2026-11-26 UTC through GitHub's App webhook delivery list and detail GET endpoints, authenticated as the existing App.
No acceptance repository was found in the bounded metadata search; the four detail reads were verified as belonging to public organization repositories.
The capture verified event and action against the listing and compared the parser-relevant projections before and after scrubbing in memory:

| Delivery | Preserved and tested |
| --- | --- |
| `push` | No action field or thread object; the decoded event matches the bare trigger only. |
| `pull_request.opened` | PR number at `pull_request.number`, no `issue` object and exact action matching. |
| `issue_comment.created` | Bot sender and matching comment author, issue number and comment ID, non-command body decoded as event context. |
| `issues.edited` | Bot dashboard edit with string bodies at `issue.body` and `changes.body.from`; the parser extracts current and prior values separately. |

Only allowlisted payload fields were written to [fixtures/webhooks](router/tests/fixtures/webhooks); review confirmed that IDs, logins, repository names and free text are synthetic.
Raw responses stayed in memory and were not sent to the model.
No headers, URLs, source IDs or raw bodies were retained.
Dashboard text distinguishes the two fields with synthetic placeholders; this does not claim a real checkbox transition.
These samples do not cover a human mention or `pull_request_review`; the existing synthetic tests still cover positive mention and dashboard routing.
On pinned Rust 1.99.1, local workspace tests passed (190 passed, two existing ignores), including all four capture-backed cases; router Clippy passed with warnings denied.
No replay, external write, seat run or deployment verification was performed.

### 2026-11-25: Semantic snapshots and bounded properties

Reviewed 12 `insta` baselines in `router/tests/snapshots/` and `runner/src/snapshots/` against the reply markers, remedies, check attribution and repository dashboard sections in `contract.nuon` and `SPEC.md`.
The policy tests still check public dashboard privacy separately; the runner tests still check that a fallback matches the pinned golem definition.
`router/tests/properties.rs` exercises five properties with 96 cases each: mention authority and exact context, UTF-8 byte limits, frontmatter duration and duplicate keys, seat discovery and alias rejection, and the GitHub login length boundary.
With Rust 1.99.1 and `INSTA_UPDATE=no`, Windows workspace tests exited 0 with 186 passed and two existing ignores; a second run verified all 12 snapshot hashes unchanged.
Linux coverage met the unchanged ratchet: router 98.4%, runner 88.9%, xtask 46.6%.
Pinned Linux CI passed formatting, host and wasm Clippy, coverage, script tests, local packaging, workerd and dependency checks, then stopped at `actionlint` because the temporary source copy lacked Git metadata.
`actionlint` and the remaining repository checks passed separately in the working checkout.
This is local evidence, not a captured App delivery, GitHub rendering check or live run.

### 2026-11-25: Target Mise minimum, local runner proof

`runner/src/target_mise/tests.rs` uses a deterministic newer release fixture to check version selection, fixed asset names, checksum-before-extraction, size and deadline limits, and failure without staging.
The pinned official Mise 2026.11.14 Linux x64 archive matched its published checksum, and `mise config ls --json` ignored both the bare `.toml` fragment and an uppercase `.TOML` fragment in an isolated workspace.
On Linux Docker Engine 29.10.0 with crun, `verified_target_mise_is_available_to_every_run_container` used the unchanged sandbox create flags and a seat-free image with the production entrypoint.
Preparation and verification exited 0, and `mise --version` printed `fixture-mise` inside verification even with another `mise` in the target tool path.
The test also checks the read-only executable mount in read-only agent, preparation and writable agent/verifier container commands.
The existing agent-container task now runs this check against the full agent image in CI; the local proof used the pinned Node base and production entrypoint, not a full image build or a deployed run.
With Rust 1.99.1 on Linux, Clippy passed with `-D warnings`, the runner suite passed 142 tests with three existing ignores, and LLVM reported 88.95% runner line coverage against the 88.9% floor.

### 2026-11-25: Detect child memory kills

[Blacksmith probe 45610209551](https://github.com/isp-insoft-gmbh/isp-golem-xiv/actions/runs/45610209551) observed 160 child kills at the memory limit.
The kernel counter recorded all 160; Docker inspection and events missed 30.
[Real-Docker CI](https://github.com/isp-insoft-gmbh/isp-golem-xiv/actions/runs/45616492400) reported ten of ten child kills with the patched image, including one Docker missed.
[Release integrity](evidence/native-v25-integrity-20261125.json) ties v25 to `b817366`; [ops PR #43](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/pull/43) pinned its verified digest.
