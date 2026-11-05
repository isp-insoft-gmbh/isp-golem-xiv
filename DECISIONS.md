# Decisions

Why things are the way they are, newest first.
Each entry: date, decision, reason, and evidence where it exists.

## 2026-11-05: Startup download failure without run-linked inputs

- V5 first retained changer passed actual publication/lifecycle audit; continuation remains unbound.
  Matching-title workflow download failed HTTP504 before credential selection or native execution, but title/timing/uniqueness do not prove the complete dispatched request.
  Inspected complete job/run logs have zero request envelopes, no artifacts or API inputs were found; archive scratch removed.
  Original request/check/generation and reservation remain unchanged, echo unexecuted, no retry/association/manual terminalization.
- Future prevention should bound only artifact GET retries and record authenticated full-input identity before download, without retrying an agent or weakening checksum/TLS checks.
  Such records cannot be backfilled for this workflow.
- Conditional design investigation found a possible distinct pre-admission cancellation path: persisted bind identity precedes every v2 grant, while a durable stop fence denies late grants.
  This is not implemented or approved and does not authorize current-state mutation.
  Prerequisites include monotonic no-prior-bind provenance, null run/attempt, exact CAS/readback fence, eligibility recheck after races, durable original-tag receipt, truthful no-admission audit and retention on unknown/bound states.
  Existing destruction/completion and legacy-active barriers remain; it would not prove cancellation of an unidentified running workflow or satisfy that acceptance case.

## 2026-11-05: Event context, delegated commits, separate feedback authority

- Unchanged Slice 2 acceptance failed changer and echo while protected-path, serial, stop and issue-opened no-change passed.
  Historical request inspection confirmed changer instructions preserved but echo's triggering comment body absent.
  Both runs reported validated typed blocked with observed model and all lifecycle phases; no startup/schema/publication failure is inferred from that terminal path.
- Candidate event preparation copies signed comment text into existing bounded instructions without truncation; event task text remains untrusted, not authority to bypass runner restrictions.
  Native feedback eligibility stays separate from `human()` authorization and payer rules.
  Human event-comment feedback requires API-bound comment ID, actor, issue and User type; owner fallback validates actor membership independently, bots and absent issue/comment context remain silent.
- Writable prompt explicitly delegates commits/push/PR creation to trusted host after verification.
  This addresses a plausible changer conflict; its model's actual refusal reasoning was not retained, so behavioral resolution requires the unchanged task on the candidate.
- A comment lookup may fail after initial successful validation.
  Candidate attempts a blocked check retaining the observed model, prohibits change publication and does not release ownership while feedback validation remains uncertain.
  Regression exercises initial success followed by lookup failures through the destruction-guard callback; it is a unit boundary, not real Docker/coordinator proof.
- Duplicate queued echo check with the same delivery is a separate unresolved creation/journal gap.
  It does not establish duplicate execution; preserve identities, no deletion/reset/manual terminalization.
- Local full CI twice reset the Windows Miniflare connection at unchanged oversized-body fixture before event context tests.
  The same built modules and all four unmodified runtime harness stages subsequently passed once without rebuilding; hashes remained equal.
  This is diagnostic timing evidence, not full-CI success or proof of exclusive Windows scope; no bounds, assertions, test selection or timeouts changed.

## 2026-11-04: Protected-path refusal needs an explicit disposition

- Retained reproduction on native v3 produced replied/success for a correctly refused protected-path request.
  Check `126651646306`, run `41852382351`, source comment `6633850119`; original protected file unchanged, no owned branch or PR.
  This establishes outcome misclassification, not a filesystem guard bypass.
  Private response content is not copied into public evidence or test fixtures.
- Writable results now request a strict completed/blocked disposition and bounded message from the pinned harness.
  Native parsing validates the structure; ordinary prose, missing structure and malformed terminal streams never infer a successful task.
  Typed blocked results cannot publish changes, and known observed model attribution survives post-agent validation failures.
  Read-only behavior and bounded timeout partial-publication semantics remain separate.
- Synthetic regressions and pinned CLI feature inspection establish handling and feature presence only.
  Actual Linux/provider controls must show both protected refusal and allowed work before production rollout; fail-closed behavior alone could hide incompatibility.
  A dedicated opt-in test uses actual containers/provider with generic fixtures, then asserts mock publication payloads.
  Real GitHub publication, retained-scenario rerun and unchanged full acceptance remain separate required proofs.
- The protected case later passed against the actual provider, while allowed work reported completed with correct count/path/mode but wrong exact bytes.
  No private contents were disclosed; the exact content mismatch remains unknown.
  Oliver approved a deterministic printf command as the control's artifact producer rather than prose-directed punctuation.
  Only control instructions change; byte assertions, verification, protection and publication criteria remain intact, with no judge or production-code change.
- Review found duplicate timeout terminal records, lost model attribution after validation failure, and incomplete positive-control payload assertions; candidate corrections address each.
  Fresh full CI passed after one unchanged retry of a Windows Miniflare ECONNRESET failure.

## 2026-11-04: Full acceptance failure and shared-branch journal fencing

- The real v3 outcome proof process exited 0, including bot-owner, timeout publication/continuation and actual Cloudflare scheduled owner execution.
  Full unchanged Slice 2 acceptance then exited 1: all static/evidence and Slice 1 checks, changed PR and continuation passed; forbidden audit lacked blocked outcome, serial timed out, stop never dispatched, event setup returned GitHub HTTP 500 and its checks were unexecuted.
- Failed native v3 run `41802954891` reported static `queue conflict`, leaving request `eec9cfc0-d82c-11f1-97dc-9f61ec87b7e3` unbound dispatching and two other requests pending.
  The judge's cleanup made the target unavailable; do not discard requests or infer the forbidden task's actual outcome from a missing check.
- An isolated marked private repository reproduced five concurrent Contents PUT pairs to different files on one branch.
  Every pair produced one HTTP 409 reference conflict while the losing file blob remained unchanged; owned repository deletion/readback confirmed cleanup.
  This establishes the API conflict mechanism affecting queue and cursor writers sharing `golem-state`, not the historical rejection's exact predicate.
- Queue saves now freeze expected blob SHA and intended bytes, confirm exact committed readback, retry only unchanged-preimage branch conflicts within bounds, and reject divergent or malformed state without merging or granting execution.
  Missing run metadata is transient; ownership mismatches remain denied.
  No blind dispatch or agent-execution retry.
- A correlated completed failed run with both check and target unavailable can persist an immutable private blocked/unconfirmed audit before terminalization; retirement preserves its discoverable audit ref.
  Existing target with missing check, ambiguous API failures, wrong identity/attempt or failed audit persistence retain ownership.
  Audit reuse validates tag name, SHA, subject type/SHA and full message.
- Known v3 maintenance association adds exact release routing, asset, source tag, build and timestamp checks under the declared trusted GitHub/CI boundary.
  Both legitimate request environment records in the pinned workflow are required and deep-equal to the immutable request; extra records fail closed.
  Read-only inspection passed; apply still requires corrected live Coordinator source and unchanged pre-CAS deployment proof.
- Full CI initially hit the unchanged 60-second Coordinator suite wrapper after added cases.
  Four independent fixture batches preserve isolated SQLite stores, real restarts and all assertions; parent fresh full CI then exited 0, with no wrapper, limit or selection changes.
  Different-model security review closed findings within three calls of its finite budget.
- Forbidden behavior still needs a retained live reproduction after seat recovery; no prompt safety guard, promise or judge check has been weakened.

## 2026-11-04: Released runtime repair and continued repository discovery

- PR #9 passed exact-head CI on `644c930` (`41787665840`), merged as `00d62520`, passed trunk CI `41788243519` and deployment `41788243140`, and published `v3` (`41788750529`).
  Private ops PR #3 passed CI on exact head `87e5bff` (`41791328946`), merged as `8aca099`, then runtime workflow byte-matched the reviewed `v3` pin.
- Guarded operator association executed only after corrected deployment, producing candidate evidence tag `31a8daa0569f0ccc8c6e67587c03d384a41a6916`.
  Normal reconciliation produced blocked audit and terminal retirement for failed scheduled request `f37f6672-d908-4ffa-6488-c011eb2f94c7`, check `126391910880`, run `41774622060`.
  Read-only observer exited 0 against matching bound/terminal snapshots, immutable request and receipt, with current row absent.
  Provider execution is unproven; never count this cleanup as schedule-owner evidence.
- Fresh valid outcome sandbox `isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e` is running real bot, timeout, continuation and cron proofs with released `v3`; no terminal proof result yet.
- Live scheduler cursor had completed 38 repositories and set done for the UTC day.
  Its early return prevented newly added repositories from discovery until tomorrow, so resume bounded metadata sweeps while preserving completed IDs and daily claims.
  Do not rescan definitions of completed repositories or reset prepared work; that separate limitation remains explicit.
- New built-Worker regression was red before the correction and green afterward, with persisted restart, same-day late repository admission and preserved request bounds.
  Raw enqueue attempts are recorded before rejection/deduplication; old occurrence counts must not increase during rescans, while the late occurrence requires exactly one attempt and one check.
  This preserves legitimate initial 429 retries rather than pretending every occurrence always admitted on its first attempt.
- A substituted different-model reviewer found no scoped blocker after the raw-attempt assertion fix.
  Historical maintenance fixture reconstructs exact v2 workflow bytes from the current v3 URL and still checks the full pinned v2 digest; production historical validation is unchanged.
  Latest fresh full CI passed; no judge selection, limit, promise or gate was changed.

## 2026-11-04: Local PAX paths and failed startup recovery

- Original orphan recovery was independently observed after deployment: original check `126334264637`, run `41772599688`, exact original prepared identity, receipts, bound-before-terminal history, retirement and ordered native phases.
  Its outcome is replied, not fresh publication; see `EVIDENCE.md`.
- Real outcomes proof process stopped with no accepted outcome evidence.
  Partial run `41774486415` blocked before provider execution because the actual codeload archive contains a local PAX `path` record.
  Scheduled run `41774622060` failed before admission with the old generic queue denial; its HTTP cause remains unidentified and its reservation is preserved.
  Bot fixture used scalar triggers although frontmatter requires a sequence; deployed index confirmed it disabled, so correct fixture syntax rather than bot filtering.
- The immutable private archive at commit `6707a8bbef9526cbdc467a287c1089454e164f7a`, compressed SHA-256 `2d4de7a25756c7703182a7c14f44b557a8043e9c525aeb0c41f9d0f46ba17461`, reproduced the old exact extraction error.
  Local PAX support permits one bounded UTF-8 path record for the next real entry, no other overrides; existing path, link, root, size and collision guards remain.
  Review found inherited Win32 alias/device hazards; raw component checks now reject them before normalization or writes for ordinary and effective PAX paths.
  Parent executed the ignored private integration test explicitly: tracked digest and embedded commit checked before extraction, independent tar comparison passed, scratch archive removed.
- Native control requests now use bounded same-run retries for transport, 429 and 5xx only, with fresh signed timestamps; authorization, ownership and malformed grants remain fail-closed.
  This retries control communication, never agent execution, and does not retroactively identify the old denial's cause.
- Undispatched pending/reserved requests now preflight target identity; 404 records terminal blocked reason `target unavailable at dispatch preflight` privately.
  Do not claim deletion: GitHub also hides authorization failures as 404.
  Dispatched/bound work, 403, 5xx and identity mismatch retain their claims.
- A narrow operator helper can associate one proven completed failed workflow with its exact existing unbound reservation, never reset, redispatch or manually mark it completed.
  It requires exact immutable request, generation, workflow bytes, full trusted-step request/log evidence, unique run, known historical runner asset identity, candidate evidence and queue CAS; normal coordinator reconciliation then owns blocked auditing and release.
  Apply additionally requires the reviewed Coordinator source at the exact live Worker health version, before evidence writes and again immediately before CAS.
  Version drift, source drift and health failure on the second check all prove zero queue PUTs, retained dispatching state and no applied evidence.
  Actual read-only inspection passed; no remote apply has occurred.
- Release `v2` is not an immutable release container.
  Historical binary identity is pinned by asset `707806211`, its exact GitHub digest, unchanged creation/update times preceding the failed run, uploader, annotated source tag and successful release-build metadata.
  This uses trusted GitHub metadata and CI, not an independent cryptographic build attestation.
- Security review completed within four calls and the 60-minute bound, closing path aliases, deployment gating, historical asset identity, default branch, tracked archive identifiers and lifecycle detection findings.
  Fresh final Node repair suite and full `mise run ci` exited 0.
  Earlier local simulator resets and a regex-stem typo were not hidden with gate changes, ignores or raised limits.
- These corrections remain local until protected PR CI, deployment and a new released runner succeed.
  Schedule-owner, bot-owner, incomplete/continuation and full Slice 2 acceptance are still unverified.

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
  A second scoped review covered both operational proof helpers and the already-applied ops `v2` pin without finding a publication blocker.
  Outcomes polling tolerates a missing immutable request tag only while its check is non-completed; completed checks remain strict.
  Current-day schedule freshness uses the actual Actions run's `created_at`, never a nonexistent CheckRun field.
  The pilot observer additionally checks golem/harness, workflow path and exact journal tag/resource/seat; the real changed observation passed again.
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
