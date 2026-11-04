# Decisions

Why things are the way they are, newest first.
Each entry: date, decision, reason, and evidence where it exists.

## 2026-11-04: Cold-start bootstrap and orphan recovery correction

- A controlled Contents API write naming a nonexistent private-ops branch returned HTTP 404 (`Branch ... not found`), without creating a file.
  The actual canary's prepared source predates `golem-state` initialization; the old mention path attempted continuation writes without establishing that branch.
- V2 processing now verifies the state branch before consuming its delivery claim and invokes the coordinator's atomic bootstrap when absent.
  Legacy routing returns before this new precondition.
  Strict mock GitHub Contents semantics reproduced the old cold-start failure before the correction.
- Cron discovers at most one orphan source per tick from at most 512 pending source refs, preserving its exact prepared request and check.
  Event-index rotation prevents one failing indexed head from indefinitely starving healthy peers.
  Completion receipts retain original annotated-source identity before removing index/cursor/pending-ref state; the immutable source remains reachable through that receipt.
  Existing completed sources are safely re-admitted through coordinator idempotence, not dispatched again.
- Real built edge + actual SQLite tests cover first bootstrap, orphan restart with no recreated checks, legacy completed-source dedup, five interrupted retirement boundaries and blocked-head fairness across restart.
  Existing multi-event +36 recovery and scheduled-call bounds remain asserted.
  Full 512/513-source and full-index-capacity matrices remain unverified.
- Fresh full CI initially caught an overlong `process` function.
  Removed duplicated reaction/membership/stop handling already implemented by `process_mention`, passing repository metadata so authorized normal requests look up head only after stop handling.
  No lint suppression or gate relaxation.
- Subsequent fresh full `mise run ci` exited 0; focused different-model review found no scoped correctness/security/regression blocker.
  Original live orphan remains unresolved until this correction is deployed and independently observed serving it.

## 2026-11-04: Live canary identities and cold-start admission gap

- Guarded Slice 2 PR #7 merged only after exact-head CI passed on `0bf104d`; trunk `0a9555f` passed CI, deployed, and published `v2`.
  Private ops PR #2 merged with green CI after release publication; its runtime workflow was byte-compared with the `v2` pin.
- First observer incorrectly filtered on nonexistent CheckRun `created_at`; GitHub's default latest-name filter also hides older same-name checks.
  The corrected read-only observer binds an explicit check to immutable request/comment identity, uses actual workflow timestamps for bounded lifecycle history and fresh-commit causality, and preserves all changed-outcome, content, audit, queue and phase assertions.
- Delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e` is the fresh-publication proof: check `126340791321`, run `41758683949`, new commit `318d77b` during that invocation, matching immutable request/terminal receipt and bound-before-terminal history.
  Trusted native logs place confirmed destruction before publication credentials, publication completion before reservation release.
- Later delivery `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` completed as replied against existing content; its successful run is not evidence of a fresh commit.
  Original delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a` remains queued with an immutable prepared source but no request tag, receipt, queue row or run.
  All three were independently correlated through their source comments/checks; later success does not supersede the original request.
- The source of the unresolved request was written at `01:49:30Z`, before `golem-state` initialization at `01:50:11Z`.
  Mention persistence does not establish the coordinator journal before writing its continuation, unlike the event/schedule paths.
  This cold-start path and the missing orphan-source discovery require a product correction; do not mark or discard the original check to obtain a pass.
- Canary routing was disabled again; full deployed Slice 1 regression passed with released runner `v2` (`41759292169`).
  The attributable tail observed a later request only; deletion was verified independently and no tail result is treated as provider/publication proof.
- Real schedule-owner, bot-owner, incomplete/continuation and full Slice 2 acceptance remain unproven.
  Auxiliary coverage work is an isolated measured proposal, not a CI ratchet yet.

## 2026-11-04: Guarded Slice 2 implementation, live proof pending

- Version 2 routing is opt-in via private ops variable `GOLEM_PROTOCOL_VERSION=2`.
  Absence preserves the released mention path; enable only after publishing and pinning the backward-compatible runner.
- One SQLite Durable Object arbitrates seat and repository/golem resources together; queued requests do not consume Actions runners.
  Private ops `golem-state:queue.json` and immutable request tags are authoritative; SQLite is a rebuildable projection.
  Accepted pending records are never evicted; capacity is explicitly 32 current records.
  Dispatch ambiguity retains its reservation instead of risking a second charge.
- Signed native callbacks use exact-body HMAC-SHA256 with normalized App PEM, freshness checking, generation and numeric safe-integer run/attempt identity.
  Parent integration found string/number mismatch between native producer and JS consumer; wire producer corrected and regression added.
  Only trusted supervisor may attest all containers destroyed and publication complete before completion releases resources.
- Writable execution, target mise preparation and verification remain in isolated containers without App/GitHub credentials.
  Native publisher checks bounded filesystem changes and forbidden paths, continues the owned branch/PR, publishes through scoped Git Data APIs, and keeps attribution observable.
  Host runner reuses the pure router parser through an explicitly versioned path dependency; no second frontmatter parser.
- Independent different-model review found three concrete defects: scheduled claims could strand rejected admissions, stop cancelled multiple/unattended runs, and terminal-run reconciliation could release resources without final audit.
  Corrections retain exact prepared schedule/check across restart, stop only the latest active mention (otherwise latest pending mention), and repair missing terminal audit before release.
  Native audit producer payloads were exercised against actual SQLite reconciliation with mocked GitHub; valid audit outcomes are preserved.
- V2 mention admissions now reuse durable prepared continuations after rejection, with person seat and mention trigger unchanged.
  Unindexed sources before successful index admission remain a separate open recovery gap, not a completed behavior.
- Consolidated rebuilt Worker tests initially passed, then exposed a new fixture-state error after adding mention recovery: its extra captured admission contaminated a later hard-coded queue count.
  Isolated the new case after existing scenarios, kept original assertions, waited for the actual rejected admission rather than merely a preparatory write, and asserted no premature rocket.
  The actual-edge/actual-Coordinator mock GitHub adapter then rejected newly required continuation endpoints; added bounded Contents CAS semantics, not bypassed production persistence.
- Final rebuilt Worker suite passes: released-v1 path, v2 owner/event/schedule continuation, actual Rust edge → actual SQLite admission/dispatch/signed bind/completion/rebuild/resource release, plus 25 coordinator scenarios.
  Subsequent fresh full `mise run ci` exited 0 on the corrected shared tree.
  These use mocked GitHub and simulated native callbacks; authenticated writable execution/publication and deployed v1 regression remain unproven.
- Latest native WSL Linux tests/strict Clippy include label handling and numeric queue-wire correction; image probes establish executable/parser behavior only.
  Scoped Slice 2 mutation testing remains unrun.
  Branch coverage ratchet is not fabricated: exact stable 1.99.1 rejects the required experimental branch-coverage option; toolchain/coverage decision remains open.
- Other open gaps: event-index admission capacity/CAS can leave undiscovered source tags, unbound ambiguous dispatch can remain reserved indefinitely, large/changing installations can exceed daily scan throughput, non-Linux hosted labels are not implemented.
  No claim of universal credential-exfiltration prevention: scanning blocks handled/recognized representations, not arbitrary covert encodings.
- Judge and configured acceptance commands remain unchanged.

## 2026-11-03: First authenticated-path acceptance fails before agent startup

- After successful release `v0` (`41700313107`), full `mise run accept -- --slice 1` passed all static promises plus index, eyes, dispatch, unknown-golem refusal.
  It failed reply (`no reply containing the nonce within 15min`) and audit (`summary lacks lines matching: outcome: replied`); no-loop was skipped, not observed failing.
- Private runner job `41702041018` failed before provider execution: `snapshot contains a link or special file`.
  This is an archive handling defect, not evidence of invalid subscription credentials.
- Actual codeload archive of the public code repository starts with `pax_global_header`, type `g`, containing `52 comment=<40-hex-commit>\n`.
  `tar` exposes this global header; the old extractor rejected it.
- Runner now inspects raw entries and permits only that first, exact-sized commit-comment record.
  Path/link/size overrides, malformed or oversized metadata, other extensions, links and devices remain rejected.
  Expanded-size accounting includes metadata, headers, padding and trailing bytes without retaining trailing data.
  Permanent representative regression was red with the deployed error, then green; extraction of the actual downloaded public code archive also passed.
  Temporary archive and probe artifacts were cleaned; no target source was placed in the public code repository.
- Actual immutable public archive differential proof (not the synthetic regression fixture): downloaded `https://codeload.github.com/isp-insoft-gmbh/isp-golem-xiv/tar.gz/8b13b0f1a33b10d9b26738002e6d697785a228d0`, 108774 compressed bytes, SHA-256 `2db21cc3ad4459c8ff563725aeda8b963547fe1b3b5a3e17eaff306024075369`.
  An OS-temp Rust driver included old source obtained by `git show 8b13b0f:runner/src/snapshot.rs` and current source as separate modules, linked existing locked dependency artifacts, and ran both on identical downloaded bytes.
  Old source SHA-256 `d0e0bdba034bd61351489c66d76ee8879eba6bc54a5428c72cb069fd7decf601` returned exactly `snapshot contains a link or special file`, extracting zero entries.
  Current source SHA-256 `6663793378db19160a3489767ee970d20ba9f817915d5b2adeabf13cad678b3c` extracted 54 files and 12 directories, 364219 payload bytes, with no metadata file.
  Extracted README.md (310 bytes, Git blob `1c7c7483c107e9f0cc97a746b3c17074140b7bf3`) and LICENSE (1072 bytes, Git blob `55ec06c8f9e9c69ea71755e61081340d5460103a`) exactly matched independent Contents API bytes at the same immutable commit; API blob hashes were recomputed.
  Identical assertions passed twice; temporary downloads, old/new source copies, driver, executable and extracted trees were removed.
- This still does not prove authenticated agent execution; a new released runner and three consecutive full acceptance passes remain required.

## 2026-11-03: Deployed Slice 1 preflight

- PR #4 merged only after required Linux `ci` succeeded on exact intended head `b18fa24` (run `41696918035`), including actual pinned image build and timeout entrypoint tests.
  Rebase merge preserved agent authors; trunk `8b13b0f` subsequently passed `ci` (`41697718686`) and deployed through CI (`41697718711`).
- All Slice 1 static checks now pass, including health/version, bad signature rejection, index auth.
  Earlier red observations for these static promises are retained above/below; live acceptance scenarios are still pending.
- `tools/prove-refusals.mjs` captured a real App delivery and sent signed actor simulations to the deployed Worker.
  Both no-seat and non-member refusals were independently read back; no matching runner workflows started.
  These are simulations, not actions performed by the named account holders; links and reproducible path are in `EVIDENCE.md`.
- Proof helper initially failed delivery-detail lookup because JSON numeric delivery IDs exceeded JS safe integer precision.
  Preserving their exact decimal strings corrected the URL; resumed the same repository rather than generating more test repositories.
- Release `v0` was tagged only after trunk CI passed and real distribution packaging had passed in PR CI; publishing remains CI-only.

## 2026-11-03: Container compatibility diagnosis

- Linux CI `41693321024` (`69a0b0c`) and `41695226429` (`14a8728`) passed `mise run ci` but failed the separate agent-container step; neither constitutes a green required `ci` check.
- Trusted preparation now exposes credential-free image build diagnostics; agent execution diagnostics remain suppressed and bounded.
  Second run identifies the final `--max-turns` help-text grep as the failing build operation.
- Exact Claude 2.1.317 Linux ELF (SHA-256 `eb224419e344575faa4a1f39b290b649425a10cd5ee2ef1d003480cd9ae57eb0`) was installed through mise into isolated WSL temp space.
  In a fresh HOME with cleared environment and network namespace disabled: unknown option rejected; invalid `--max-turns` value rejected with numeric parser error; valid value reached the not-logged-in error.
  Binary option registration also contains `--max-turns <turns>`; ordinary help omits it.
- Dockerfile replaces the false help-text assumption with the same positive/negative parser probes under `RUN --network=none`, fresh HOME, cleared env, hard timeout.
  This proves parser compatibility only, not authenticated turn-limit enforcement.
  Existing image-build/watchdog test and acceptance gates are unchanged.
  WSL probe block passed twice; temporary 638 MiB install cleaned.
- Intermittent local oversized-request ECONNRESET reproduced with a trivial early-413 Worker, both Miniflare dispatchFetch and direct HTTP, independent of Rust code.
  Fixed 20-trial conditions included 262186-byte and 2097194-byte JSON and chunked bodies; early-response variants failed transport requests, while all following requests and valid-body controls returned 200.
  Bounded reading eliminated resets only for the near-limit body, not larger/chunked uploads; no arbitrary draining, retries, assertion removal, or dependency change made to manufacture a pass.
  Linux full CI has passed this unchanged test on both completed runs; Windows transport nondeterminism remains recorded.

## 2026-11-03: Slice 1 implementation

- Rust workspace separates host-tested router policy, wasm edge I/O, trusted native runner, stdlib-only xtask.
- Router reads the index fresh rather than retaining a cache; bounded to 20 golems of 64 KiB each.
- Atomic create-only GitHub refs in private ops claim delivery IDs, with payload fingerprints; no eventually consistent KV lock.
  This prevents duplicate dispatch attempts, not transactional delivery guarantees across GitHub APIs.
  Failed or ambiguous deliveries retain claims; absent rocket does not establish dispatch failure.
  Reconciliation requires checking delivery, check external ID, and ops runs before any operator action; workflow reruns are rejected before credential access.
- Runner snapshots the exact dispatch SHA, runs Claude in an unprivileged read-only container with only read tools, subscription token, target snapshot, bounded resources, independent timeout entrypoint.
  No GitHub token, App key, host home, Docker socket, or workflow control files reach the container.
  Confirmed container destruction precedes fresh scoped publication credentials.
  Output scanning is defense in depth, not a claim of arbitrary covert-channel prevention.
- Direct Workers REST multipart upload supports Rust SDK build artifacts without a second CLI build format; production deployment happens only in CI after validation.
- Dependencies: serde/serde_json for bounded protocol I/O; serde-saphyr for strict bounded YAML; hmac/sha2 for constant-time webhook signatures and fingerprints; worker/wasm-bindgen/futures-util/base64 for Workers runtime APIs, native crypto bridge, cancellation, contents decoding.
  Runner uses reqwest/tokio for bounded async HTTP/process I/O, jsonwebtoken for native App JWTs, flate2/tar for validated snapshots, tempfile for automatically cleaned workspaces, base64 for secret scanning.
- Global duplicate-dependency denial remains; exact-version exceptions in `deny.toml` cover incompatible upstream base64/getrandom/syn/windows-sys generations.
  CDLA-Permissive-2.0 allowed only for webpki-roots 1.0.11; its license ships in releases.
- Local `mise run ci` passed after multiline/rerun hardening and again after container-test wiring: 27 Rust tests, wasm Clippy/build, built Worker workerd tests, packaging, deploy transport tests, all linters, original judge parser checks.
  Docker is unavailable locally; Linux CI must build the actual pinned agent image and exercise its timeout entrypoint before integration is accepted.
- Scoped mutation evidence: router 3 selected mention mutants caught (~49s); xtask 12/12 selected mutants caught after boundary-test improvements (38.49s wall, 0.31s driver CPU, sampled peak scratch 17.6 MB).
  Compiler/test CPU and router peak disk were not captured; no complete resource-accounting claim.
- Independent review by gpt-5.6-terra found no demonstrated auth/secret exploit; live deployment and Docker execution remain verification gaps.
- Initial preflight red observations: ops repo absent, router health 404; workflow, seat, signature/auth checks blocked by these; evidence absent.
  Initial full gate also skipped live checks because static setup failed.
  These skipped checks are not claimed as live red observations; retain actual later acceptance failures before marking their behavior proven.
- Judge changes: none (`accept.nu`, `contract.nuon`, `tools/ci.nu` unchanged).

## 2026-11-03: Hardening before handoff

Oliver's decisions after four independent reviews of the judge, the spec and plan, the development workflow, and the agent's autonomy.

- **Promises for all slices are written before handoff.** Otherwise `--slice 3` passed on Slice 1 checks alone, and the agent would have written promises it then checks itself.
- **The App installation covers all repositories.** Adding repositories to a selected installation needs a token gh does not have; the topic allowlist already limits what the router acts on.
- **The admin dashboard is Slice 4 with its own promises.** It stays a goal but no longer competes with the core.
- **Anthropic seats may run under `pi`.** People remain responsible for their providers' terms.
- **Router worker name and URL are fixed in `contract.nuon`.** The App's OAuth callback URL is set by hand and must not change when the agent deploys. The account's `workers.dev` subdomain `isp-insoft-gmbh` was created for this through the Cloudflare API; it can be renamed in the Cloudflare dashboard.
- **Promise kinds prerequisite, check, evidence.** Prerequisites are exempt from red before green; behavior `accept.nu` cannot drive is proven in `EVIDENCE.md`.
- **End-to-end scenarios stay out of public CI.** They need seats and private test repositories.
- **Rust is pinned by `rust-toolchain.toml`, not mise.** mise's `RUSTUP_TOOLCHAIN` would override the pin; Clippy's `cargo` group is dropped because its findings are mostly in dependencies.
- **`stopped` is an outcome**, and `no_change` belongs to runs without a person to answer.
