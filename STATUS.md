# Status

- **Current goal (2026-11-18): actual Toktrak execution, user authorized autonomous repair and Toktrak pushes.**
  Discovery alone is not completion; require a successful native run and actual App answer with the requested file-based findings.
  No new login requested or provider-authentication failure established.
  First submission `6978222491` accidentally contained literal `$body`; matching GitHub delivery confirmed HTTP 200/ignored with no admission.
  Corrected comment `6978423619` was read back exactly and admitted as delivery `a7348c80-e34a-11f1-8a08-d5473e717d19`, check `134798067723`, native run `44303282681`.
  That run completed blocked before provider access: `snapshot contains a link or special file`; App reply `6978569880` contains the blocker, not a successful canary answer.
  Exactly one tracked link exists: `.agents/skills` points to `../.claude/skills`.
  Repair is isolated at `C:/Users/okr/AppData/Local/Temp/toktrak-golem-runtime`, branch `fix/golem-skill-snapshot`; original user `mise.lock` remains untouched.
  Replace that alias with all fourteen byte-identical regular skill files; keep `.claude`, Pi/Codex settings and runner bounds unchanged.
  Existing Build validation now checks mirror paths, regular types, bytes and bounded traversal; focused Build tests -69 completed exit 0 and `mise run check` passed after the previously red mirror-root check.
  Full `mise run verify` -70 and clean-tree `mise run ci` -71 completed exit 0 on repair `6296d5c326d7b71329312d75e4f440da9715bf14`; all local application/runtime/coverage gates passed, with Linux/macOS scheduler cases left to native CI jobs.
  Repaired Git tree has no symlinks or submodules; all fourteen mirror files match canonical bytes.
  Toktrak PR #156 is published at that exact head; GitHub CI `44307335930` failed, watcher -72 exit 1.
  Core, production, performance and all three native tracker jobs passed; PIT failed because one `Projection.tokenUsed` return-null mutant timed out, so required `ci` is red and merge is held.
  Failure XML identifies line 350, `NullReturnValsMutator`, status `TIMED_OUT`; application source/tests are unchanged by the mirror repair.
  Earlier baseline PIT had 185 null-return kills/zero timeouts; this run has 184 kills/one timeout, which does not by itself establish a flake or cause.
  Fresh `NULL_RETURNS` analysis -73 completed exit 0 (197 mutations, 185 killed, zero timeouts); it covered the application, not only `Projection.java`, because Mise consumed the first separator.
  Exact line-350/index-84 mutant was `KILLED` by `IdentityServiceTest.given_activeUser_when_usingAndRevokingTrackerToken_then_enforcesLifecycle`, demonstrating variability, not explaining the CI timeout.
  Added synchronous token-used event replay coverage in `ProjectionTest`; focused test -74 completed exit 0, 20/20 passed, without repository-wide checks.
  Correctly separated source-scoped PIT, coverage and full verify -75 completed exit 0: 162 fresh mutations/158 killed/four survivors/zero timeouts, 275 unit and 12 tagged tests passed, existing coverage floors passed, linked production runtime verified.
  Exact line-350/index-84 null-return mutant is now `KILLED` by the new synchronous replay test; mutation operators, timeouts and CI gates remain unchanged.
  New test is separate local commit `3052af556ebe4e7a26bd5bbff9c8d92c35dee076`; clean-tree CI -76 failed publishing the freshly linked Windows production runtime (`AccessDeniedException` on staging → final directory rename), so no push.
  Build restored the previous runtime and retained `.prod.stage-59164-3`; no application assertion failure is reported, and no definitive lock owner/cause is established.
  Owned-handle enumeration timed out; do not kill unrelated Java processes or bypass artifact validation.
  Native clean + unchanged fresh CI -77 completed exit 0, including 275 unit/12 tagged tests, all coverage floors and linked runtime; original checkout/output/lockfile untouched.
  A fresh Git fetch then established `origin/trunk=99d8dcb86b2ddd6a4949cfed2a40473c7938843b`, two commits beyond base `af20a74` (PMD 7.29.0 and dashboard tests); PR's reported base SHA was stale.
  Integrated current target without rewriting published history at local merge `622252d68e8f68d495025bf3272faa76155304ad` (parents `3052af5`, `99d8dcb`), no conflicts.
  Fresh clean CI then source-scoped PIT -78 completed exit 0 on combined `622252d`: 277 unit/12 tagged tests, unchanged coverage floors, 162 fresh mutations/158 killed/four survivors/zero timeouts.
  Pinned clean local head `622252d` was pushed normally from remote `6296d5c`; no forced update.
  Immediate GraphQL PR readback reported old metadata; fresh REST and Git both confirm head `622252d`/target `99d8dcb`, mergeable true but blocked by pending required CI.
  Exact-head GitHub CI `44314269385` completed success on `622252d`; watcher -79 exit 0. All nine jobs passed, full PIT 1875 mutants/1700 killed/zero timeouts. Updated PR description read back exactly.
  Premerge Git fetch found another target advance: `190d984433fedd7a6a76c799e990494de4fb7afc` adds Jackson/Error Prone updates and an action pin; strict protection correctly marks the previously green head behind.
  Integrated without rewriting published history at `ab5731f4d833b978a192099a6c4dc2ab8cfe03fc`, no conflicts; fresh clean CI + source-scoped PIT -80 completed exit 0 (277 unit/12 tagged tests, unchanged coverage floors, 158/162 mutations killed, zero timeouts).
  Pushed exact clean `ab5731f` normally; fresh REST confirms target `190d984` and that head. PR description updated.
  Exact-head GitHub CI `44316591896` completed success on `ab5731f`, watcher -81 exit 0; no old-head success is treated as validation of later target changes.
  Target advanced again to Mise minimum 2026.11.9 plus a Mise-action pin. A shared remote ref moved during inspection; local merge `2595be51da5b3877f4e623ef0b07cd6ae3d8e89b` actually integrates pinned parent `c1d6b98d60df3ed9417a69aa3187002c61d30c8a`, not only earlier `f73a373`. No conflicts or push; record exact parents, not mutable remote aliases.
  Direct host probe on that combined tree exits 1: `mise version 2026.11.9 is required, but you are using 2026.11.2`.
  Native v12 preparation copies Mise 2026.10.21 from ops into the image, whose Dockerfile asserts that exact version; `--prepare-target` runs `mise install` in the target workspace before provider execution.
  Therefore the pin mismatch is a source-grounded next preparation blocker, not a claim that the prior snapshot-blocked run reached this error or failed auth.
  Official task-local Mise 2026.11.9 archive -82 completed exit 0, SHA-256 `29a4b496bf859f2b3c9563c90e695fb504a8d6ce067635730cd6f3f90c9826cd` matches GitHub asset digest and checksum manifest; extracted CLI reports exact 2026.11.9 and accepts the same target config with exit 0. Archive removed; executable is under OS temp, global WinGet install unchanged.
  Native repair is on central branch `fix/native-project-mise` after status checkpoint `df3f7ca`: image and image-test CI pin 2026.11.9, plus credential-free supported/unsupported target-minimum entry tests.
  Initial full central CI -83 completed exit 0 under the unchanged-lock advisory policy, including all four runtime/coordinator suites; no local Docker test was run.
  Independent review tightened the fixture: verify Node resolves under `/target-tools`, use read-only tools for verification, match production's writable disposable workspace.
  Final source/docs CI -85 completed exit 0 under unchanged-lock advisory policy: all four runtime/coordinator suites, native tests, repository checks and Nu parsing passed.
  Docker entrypoint/runtime proof is still unrun locally and remains mandatory in the Linux CI image step before merge.
  Toktrak -84 completed exit 0 on local `2595be5` using verified task-local Mise: full CI/coverage, 277 unit/12 tagged tests and fresh scoped PIT (155/162 killed, seven survivors, zero timeouts). Exact token-used return-null mutant is killed by the new synchronous test; ordering/boundary survivors were inspected, not hidden or expanded into an unrelated campaign.
  Toktrak `2595be5` remained unpublished while native prerequisites were repaired.
  Latest pinned target `e771c4cfa9c9581937790aaf8815587c876a42e0` adds dprint plugins, locked tool refresh and google-java-format 2 layout; overlapping Build/Projection test changes were reviewed as formatting-only.
  Integrated that exact target without conflicts at `71303783f360e112f2565ff72ca3413ee0bf528d`; locked-tool install/owned-file format -93 completed exit 0 without source edits.
  Fresh clean CI + scoped PIT -94 completed exit 0 on exact clean `7130378`, with refreshed tools/formatter, existing coverage floors and 158/162 fresh mutations killed/zero timeouts.
  Exact-head GitHub validation is still required before Toktrak merge.
  Original checkout/lockfile remain untouched; no old-head success substitutes for the latest combined tree.
  Native PR #48 at `1259314876dc09ac112fa8b153814b9443413353` failed exact-head CI `44322126241`; watcher -86 exit 1.
  Repository CI passed, then the new container fixture failed before target preparation: unquoted Nu list commas split `/tmp:rw,nosuid,...` into six values, so Docker attempted image `nosuid:latest` and exited 125.
  Image build and startup/version checks had already returned success; no target-minimum, provider or auth result is established by that failure.
  Quoted the complete tmpfs option; a local probe using the actual argument expression was red at 19 base arguments, then green at the correct 14, with the tmpfs value intact; final Nu parsing passed.
  This is focused argv proof, not Docker runtime proof.
  Correction commit `7669872f57327567b303ab315bc40c559e2e7c58` passed exact-head CI `44325246899`; watcher -87 exit 0, including the real container minimum/preparation/isolated verification cases.
  Refreshed clean mergeability and no reviews/threads; protected squash #48 merged `bf46f79064f18fc7c4b287f2d051989dd05158d6`, whose tree equals reviewed `7669872` (`54a37110e7144750ee05afd4b980f1a577002ece`).
  CI deployment `44327423130` (-89) completed success on exact `bf46f79064f18fc7c4b287f2d051989dd05158d6`; live `/health` returns that full SHA.
  Trunk CI `44327422806` (-88) completed success on exact `bf46f79064f18fc7c4b287f2d051989dd05158d6`, including Agent container.
  Dedicated clean release worktree matched `trunk` and live remote `bf46f79`; reviewed release script pushed annotated `v13` at that exact commit.
  Release `44330868877` (-90) completed success on exact `bf46f79`/tag `v13`.
  Downloaded Linux ELF64 x86_64 asset `763858931`, 5,063,016 bytes, SHA-256 `3c015ae93d66d98c53d080f508da49cadfa7b882974cae24b7013f49324db1d3`; GitHub digest, exact sidecar, SHA256SUMS and ELF architecture agree.
  GitHub release metadata says immutable=false; this is independently checked point-in-time integrity under the existing GitHub/CI trust boundary, not binary attestation.
  Only after verification, committed/pushed atomic live-ops v13/Mise 2026.11.9 pair `99b9bbad1baf7d81f72cda11488651d56ed10eb6` in ops PR #20; exact-head CI `44333434159` passed, refreshed clean eligibility/no reviews or threads, protected squash merged `32fe6ce9280f7bb885b3393e19575f72e4f54c3d`.
  Ops merge tree equals reviewed head; live workflow readback SHA-256 `6d23b8427dc8eff9f303ca85e4e8e3003b55ecae3475a08198e0f90c6c0e8fb0` matches the candidate/public template exactly.
  First readback wrapper rejected GitHub base64 line breaks; joining only encoded lines fixed decoding without changing data or accepting garbage.
  Ops trunk CI `44333668213` completed success on `32fe6ce`; live v13/Mise pair is now verified, not actual provider execution.
  Matching public template on trunk-based `fix/verified-mise-runner-template` may now be published after passed local checks; no old feature ancestry will be replayed.
  Local template CI -91 failed a historical v2/v3 fixture digest because reconstruction retained the new Mise pin; runtime repair/provenance digests were not changed.
  Reconstruction now restores exactly one reviewed 2026.11.9 line to historical 2026.10.21 before the original digest checks; full focused repair-unbound suite passed red→green.
  Full corrected local template CI -92 completed exit 0 under unchanged-lock advisory policy, including all four runtime/coordinator suites and original repair/provenance guards.
  Failed -91 remains recorded as red; no runtime guard or historical digest was relaxed.
  Live ops/template pins remain unchanged; container proof is not actual provider/App-answer proof.
  Toktrak target advanced again after local `2595be5`; defer its final re-integration until native release/pin prerequisites are ready, rather than repeatedly validating stale targets.
  Next: protected native merge/release → verified ops runner/Mise pair → matching public template → current-target Toktrak validation/merge → actual canary.
  No successful working-agent claim yet.
  Branch protection requires strict `ci`, zero approving reviews and linear target history; final delivery must squash without bypass and recheck current target/head/threads.
  Required GitHub CI and fresh repaired-default-SHA actual canary remain outstanding.
  Committed-tree proof independently confirms fourteen matching Git blobs/modes in each skill tree and no tracked links/submodules.
  Earlier clean-tree CI -71 does not cover this new test; required exact-head GitHub CI still must pass before merge.
  Native Codex 0.167.1 source confirms `skills.config` cannot add project roots for `exec`; broader app-server extra-root support is not the same integration.
  Merge remains gated by required GitHub CI; fresh repaired-head archive/provider execution and actual App answer remain unproven.
- **Migration configuration (2026-11-18): ordinary repository golems, deployed discovery verified.**
  Oliver rejected per-repository rollout switches: installed organization App plus `.golems/` is sufficient.
  Branch `feat/installed-org-repositories` removes the sandbox-topic check from repository lookup and scheduled discovery, and removes obsolete admin allowlist reporting; no replacement activation flag.
  Actual red: topic-free installed fixture returned 404 instead of 200 (activity -30).
  Full Worker gate -38 completed exit 0 in 78.80 s; all four suite summaries confirmed.
  Pinned Rust 1.99.1 host/wasm Clippy, 39 router tests and the admin consumer passed.
  Full local CI initially selected Rust 1.100 through the host's Mise `cargo` wrapper in child processes; existing newer Clippy lints failed.
  Rerun -41 used pinned binaries and passed compilation/tests/runtime/bans/licenses/sources, but the broad advisory scan found existing `chrono` unmaintained advisory RUSTSEC-2026-0543 and yanked `serde-saphyr` 2.1.0.
  Dependency inputs are byte-unchanged; -43 completed exit 0 under the existing unchanged-lock PR policy (`GOLEM_CHECK_ADVISORIES=false`), including remaining gates, without suppressions or dependency edits.
  An existing event fixture counted an earlier retained delivery against the next case; captured IDs proved the extra row belonged to the preceding source.
  Its assertions now count exact source IDs, retain 20/18 cardinalities and existing tick limits, and require retirement before the next case; three focused Worker repetitions passed.
  Companion migration is [Toktrak PR #155](https://github.com/isp-insoft-gmbh/toktrak/pull/155), isolated outside this checkout; the user's original lockfile edits are preserved.
  Companion local clean-tree CI completed exit 0; GitHub CI `44272379475` passed on exact head `27b0e99bb8a467b8ac0b07e9b9d102eb1ee3f962`, including core, Linux/macOS/Windows tracker, production, performance and PIT.
  The obsolete companion workflow is disabled with no queued/running legacy jobs; stored credentials were not deleted.
  Router PR #43 merged `d94ecb20f23074a2e5108427bc484b8b91cf68d1` after exact-head CI `44273533940`; refreshed clean eligibility and no review threads verified, merge tree equals reviewed head `8c25cd3c7db799932338a9a0b41f82a8b2b4264a`.
  Trunk CI `44275364375` completed successfully on `d94ecb2`; watcher -46 completed exit 0.
  Deployment `44275360349` completed successfully; watcher -45 exit 0, production `/health` reports full `d94ecb2` SHA.
  Authenticated topic-free discovery returned 200 with nine legacy-invalid definitions before migration.
  Toktrak PR #155 then merged `af20a74fd17367db724aea6833be333062649c7a`; merge tree equals reviewed head, eligibility refreshed clean with no review threads.
  First post-merge live index returned 502; live nine-valid discovery is not yet established.
  Direct same-header GitHub reads found the default-branch full-commit response is 526,823 bytes, above the unchanged 524,288-byte Worker response cap; all nine pinned definition bytes match locally validated files.
  `Github::head` fetches the full commit only for its SHA; next fix uses compact exact Git-ref metadata, preserving limits.
  Companion trunk CI/runtime/performance runs `44278275331`/`44278275172`/`44278275113` completed successfully on `af20a74`; watchers -47/-48/-49 exit 0.
  Compact-head regression reproduced 502 before the fix; strict host/wasm Clippy and 39 router tests passed in -51.
  Full Worker gate -50 failed at the coordinator fixture's obsolete `/commits/trunk` route; that fixture now serves the compact reference, with no assertion or timeout relaxation.
  All four runtime/coordinator suites passed in -52's full log, but its activity exited 1 when the final Nu wrapper tried to use inherited string `LAST_EXIT_CODE` as an integer.
  Typed child exit capture in -53 completed exit 0; full CI log confirms all four runtime/coordinator summaries and remaining gates under `GOLEM_CHECK_ADVISORIES=false`, not a clean advisory scan.
  Compact-head source fix is local commit `a9fa7689aad95954ac86a6bae0b982e3f73fd31b`, not deployed.
  Follow-up inspection found the native runner still rejects targets without the sandbox topic in `runner/src/github.rs::validate_target`; the earlier router-only change was insufficient.
  Native gate patch removes only the topic predicate; identity, membership, check and pinned-definition checks remain.
  New ordinary-repository test was red with `target identity or allowlist mismatch`; topic-free v2 and identity-negative tests now pass (four focused native cases).
  Full native-change CI -54 completed exit 0 under the existing unchanged-lock advisory policy; all four runtime/coordinator summaries and remaining gates confirmed.
  Native tests: 40 library and 66 executable cases passed, with the existing two explicit opt-in cases ignored.
  Native PR #44 passed exact-head GitHub CI `44282831867` on `013366f7d34ced26008ca7f0775f8c57500ead8e`, then merged `d3bdcdb83f6711892729f9bcf87dd74a0c4f83c3`; refreshed clean eligibility, no review threads and matching merge tree verified.
  Trunk CI `44284844881` and Worker deployment `44284844935` completed successfully on exact `d3bdcdb`; watchers -56/-57 exit 0, production `/health` matches that full SHA.
  Annotated tag `v12` was pushed from clean, pinned native trunk; release run `44286965020` completed successfully, watcher -59 exit 0.
  Native Linux x86_64 ELF asset `762992803` (5,063,016 bytes) matches GitHub digest, sidecar and manifest SHA-256 `be664fafba4f6f644f6392488d30df6c8f1f4de4aa435b0a389acba91e538771`.
  Ops PR #18 passed exact-head CI `44288393917` on `2f0437188f2421ceed06c679d9156b83ae06a08b`, then merged `0d1a2d0c2223c2ac5fdff48d06d214b5ddbd765f`; no review threads, matching merge tree and trunk CI `44288691590` success verified.
  Live ops workflow pins v12 and byte-matches the reviewed pin (SHA-256 `36a0c5a2e55e1df5e9b12065bfdfe6a712d12eaddcc02879f9ae53d4d56b5780`); public template is updated in this held branch.
  Compact-head PR #46 passed exact-head CI `44289113469` on `6f033a3f76ad6ee5b46cd11731a5c0e06066f7bf`, then merged `ec831f448ce14f4d8dfcaa1bf1921b11e931e684`; refreshed clean eligibility, no review threads and matching merge tree verified.
  Trunk CI `44290815607` completed successfully on exact `ec831f4`; watcher -61 exit 0.
  Deployment `44290815790` completed successfully on exact `ec831f4`; watcher -62 exit 0.
  Live probe exited 0 after requiring both terminal GitHub successes and exact production `/health` SHA.
  Authenticated Toktrak discovery returned HTTP 200/no-store, nine definitions and zero validation errors without the sandbox topic.
  This establishes deployed discovery recovery after the original 502, not fresh provider execution or full Slice 4 acceptance.
  Compact-head source is applied atop that native correction at private commit `a9996d4`; combined-source CI -58 completed exit 0 under the unchanged-lock advisory policy.
  All four runtime/coordinator summaries and remaining gates passed for the combined source; this is not deployment proof.
  Release from the native merge only after terminal trunk CI success; verify/pin the native artifact before publishing the compact-head Worker change.
  Fresh isolated ops checkout byte-matches the public v11 workflow template; the old bootstrap checkout's unrelated deleted `mise.toml` remains untouched.
  No new agent execution or full Slice 4 acceptance is claimed.
- **Current slice:** 4, in progress; not accepted.
  Slices 1–3 passed deployed acceptance previously (Slice 2: 32/32; Slice 3: 44/44).
- **Current task: admin UI rewrite (deployed and live-verified):** PR #41 merged `b88f5f87095a7c72315558539c8c67d448134fc3` after exact-head CI `44108681183` on `c498383c9e779c2297f6b2b96f999d817a8a9648`; clean merge state and no review threads confirmed.
  Trunk CI `44110999820` and production deployment `44111000121` completed successfully on the merge SHA; watchers -25/-24 each completed exit 0.
  Live HTTP probe exited 0: health matches `b88f5f8`, CSS/JS bytes match reviewed source hashes, CSS MIME/no-store/POST 405, anonymous redirect/API denial and all four member APIs verified.
  Sanitized deployed evidence: `evidence/admin-ui-http-20261117.json`.
  Fresh `/seats` navigation recovered expired OAuth state; authenticated Firefox rendered the rewritten admin page as `bugabinga` on `b88f5f8`.
  Desktop 1440 px and mobile 390 px had matching document widths, actual stylesheet/CSP checks passed, and private screenshots were inspected.
  Native ArrowRight scrolled the focused run table from 0 to 43.17 px without document overflow.
  Live pagination traversed an audit-empty second page: workflow metadata 10→20, missing audits 8→18, two audited rows retained uniquely, repositories 1→2.
  This live step does not prove a new audited row appends; the automated overlapping-page fixture now explicitly asserts new-row identity and existing-link uniqueness.
  Browser record: `evidence/admin-ui-browser-20261117.json`.
  The deployed UI preserves the plain document style with semantic navigation, readable tables and health metrics, concise coverage warnings, visible retry/reload actions and about-data disclosures.
  Static CSS is about 2 KB; no framework, external fonts, animation or polling.
  Admin HTML alone permits same-origin stylesheet loading; API/auth logic and guards are unchanged.
  UI implementation gate `activity-26d024ae-21` completed exit 0/60.21 s with all four suite summaries, including CSS/MIME/no-store/CSP/method checks.
  Follow-up full Worker gate `activity-26d024ae-26` completed exit 0/64.84 s with all four suite summaries; current order-independent append/uniqueness DOM regression also passed separately.
  Current DOM regressions cover mixed-source failures, retry disabling/recovery, membership revoked during pagination, expired pagination, unknowns, counts and deduplication.
  Firefox 160 local HTTP fixture rendered actual Rust HTML/JS/CSS at 1440 px and 390 px with no document overflow; mobile tables scroll within labelled focusable regions instead of squeezing labels into letters.
  Browser retry terminal state verified: error cleared, one undispatched row added, original dispatched row retained once, usage count unchanged; pagination also verified.
  Independent review's hidden-status finding resolved; the earlier local keyboard-redraw limitation was closed by the successful live ArrowRight check.
  Next: publish deployed verification notes and the strengthened append/uniqueness regression; then resume unfinished Slice 4 work.
- **Admin dashboard (first increment deployed):** PR #39 merged `da28fd64545e3a7cdde68b3efc2b4d13c38f6db4` after exact-head CI `44084644483`; trunk CI `44086507038` and deploy `44086507106` succeeded.
  Live health matches `da28fd6`; authenticated runs/seats/repositories/undispatched APIs return 200 with private/no-store responses.
  Runs page 1 contains one verified audit, nine missing audits, zero malformed audits, with 88 workflows total and explicit pagination.
  Member-authenticated views and sanitized durable coordinator audits before retirement are deployed; full Slice 4 remains unaccepted.
  Live red before implementation: tester-authorized `GET /v1/admin/runs` returned 404 on 2026-11-17.
  Final parent full Worker gate `activity-26d024ae-6` passed exit 0 (42.17 s), including the repository-pagination correction and all four unchanged suites; retained log `C:/Users/okr/AppData/Local/Temp/golem-slice4-worker-final-20261117.log`.
  Pinned Rust 1.99.1 host/wasm Clippy, 39 router tests, DOM consumer and repair/digest tests passed.
  Unpinned Clippy selected inherited Rust 1.100 and failed on existing newer lints; explicit `mise exec -- rustup run 1.99.1 cargo clippy …` passed without source changes for those lints.
  Earlier gates failed integrated fixtures; assertions were preserved and fixtures corrected; independent review also found audit-maintenance starvation and retry-budget exhaustion, now covered by red→green coordinator regressions.
  Independent review findings are resolved within inspected scope; Firefox 160 local HTTP fixture populated all five sections and exercised runs/repository pagination.
  Remaining before completion: successful fresh run proof, historical audit backfill, seat-expiry metadata, explicit `mise run accept -- --slice 4`.
  Read-only historical recovery candidate verified for run `43428751065`: request tag `5aa18f1c36ed5752611de04281976b5ba4791324`, bound snapshot `08775555fe6e874b208704a899a1bd196b391aa5`, terminal snapshot `a321c61112bcfaa4e13f9c8ae2326f08a40fb2c5`, target check `131896504858`, matching identities; 387 s duration, reported model `default` means actual model unknown.
  Coverage: one historical join only, not all 88; no backfill writes made.
  Fresh `claudeconv` mention in native-config sandbox issue #7, comment `6948843571`: run `44089143230`, check `134059245652` blocked with native error `container execution failed`; request tag `74a936bba9d36cc37cfcb1b5e00d251a8d18e143` and admin/API v3 audit bindings verified, model null, outcome blocked, duration 367 s.
  This proves blocked-run audit projection, not successful agent execution; stage of container failure remains unknown because existing diagnostics discard stage/exit details.
  Read-only comparison against successful Claude check `131769403103` shows only `.golems/pioncodex.md` and `README.md` changed; harness/toolchain configuration did not change.
  Oliver completed GitHub sign-in in fresh headed Firefox 160; the live page rendered all five populated sections as `bugabinga`, and pagination loaded two repository sections and 20 workflow metadata entries.
  `admin-page` evidence is recorded in `evidence/admin-page-20261117.json`; screenshot inspected privately, no private repository names or credentials in the committed trace.
  Initial browser evidence predates the model correction; subsequent authenticated verification matched live `0c06d46`, API model null and rendered unknown for historical run `43428751065`.
  Full history and refreshing-seat expiry remain explicitly incomplete.
  A network event accidentally exposed the temporary browser-session cookie in private tool output; no provider credential was exposed, network subscription was disabled, cookie expired 2026-11-17 14:41:28 UTC; never copy the value into files or reports.
  Placeholder-model correction merged as PR #40 `0c06d46e5b07db04d28ea49769023a12191d238f` after exact-head CI `44096517493`; trunk CI `44098456881` and deployment `44098456622` passed (activity `activity-26d024ae-13`).
  Live health matches `0c06d46`; authenticated legacy-placeholder normalization is verified live.
  Legacy `model: default` means unknown, not an observed model; red Rust regression confirmed, producer/read projection fixed without rewriting immutable audits.
  Full gate `activity-26d024ae-11` passed exit 0 in 73.53 s; all four suite summaries retained in `C:/Users/okr/AppData/Local/Temp/golem-admin-model-20261117.log`; pinned host/wasm Clippy, four admin tests and repair/digest checks passed.
  Narrow independent review found no new provenance/safety blocker; this correction does not claim universal observed-model provenance for every harness.
  Pre-rewrite fixture: 390 px viewport had document width 567 px; desktop at 1440 px had no document overflow.
  Operator guide drafted at `OPERATIONS.md`; authenticated rendering, placeholder-model correction and the new UI's desktop/mobile behavior are verified; full Slice 4 acceptance remains pending.
- **Slice 3 progress (live unless noted):**
  PR #19 fixed the `app-oauth` probe; secret valid, blocked on Oliver registering callback `https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/auth/github/callback`.
  PR #20 (`27e847f`): help and seat replies live-verified (core sandbox issue 10, comments `6786466094`, `6786465986`); `/seats` and `/admin` sign-in redirect; `seat-page` gate green.
  PR #21 (`5e2222d`): `golems/validate` pull-request check (fixture-tested; live via Slice 3 acceptance).
  PR #22 (`333a816`, CI `42961537241`, deploy `42961537219`): onboarding PR and pinned dashboard on push, seats incl. people who left; upkeep reuses the delivery's head/index so 20-golem pushes keep the event budget (workerd suite green on Linux; Windows resets at the oversized-body test).
  Live probe in kept sandbox `isp-insoft-gmbh/isp-golem-xiv-test-onboardprobe-20261111060243`: onboarding PR #1 with exactly the two files and house rules; after merge, dashboard #2 created and pinned with all four sections listing `hello`; invalid-golem PR got `golems/validate` check `130334409741` = failure, and after merge Disabled listed it.
  Full Slice 3 acceptance cannot run live checks until the static `app-oauth` gate passes.
  PR #24 (`ff5847b`, CI `42967053433`, deploy `42967053554`): seat fallback on a `pi` harness; the agent image with pinned Pi 1.5.1 and the `golem_report` extension built and passed its offline probes in CI.
  Between that deploy and the v6 pin, fallback dispatches would have failed closed at v5 `--validate`; none were observed.
  Release v6 (run `42968967976`, tag `d9df5f1`, native asset `734356865`, SHA-256 `110c251b0476ddffd046b233716e3a4f686fb8c32f820a3049f9b4a3fb7741ad`) verified; ops PR #12 merged `5d435321fe2c75112100bcc5f456c5dec90073c2` (CI `42970183697`), live workflow byte-matches the template (SHA-256 `6e194f2d…`).
  Live fallback: `foreign` golem (`codex`/`github-copilot`) mentioned by anthropic-only `bugabinga` in the probe sandbox, issue 4 comment `6788505691`: check `130357161751` replied/success with `harness: pi`, observed `claude-opus-5-5`, `seat: bugabinga/anthropic` and the `fallback:` line; App reply carries the `foreign` marker; run `42970383464`.
  Open: seat registration page and OAuth callback (needs callback + browser session), seat-replace/refresh evidence (fresh Codex sign-in), fallback runs (runner `pi` harness and non-Anthropic providers), dashboard last outcome and run boxes.
- **Slice 3 acceptance (2026-11-11):** unchanged `--slice 3 --keep` ran after Oliver registered the callback: 41/44 gates ok, all live Slice 1–3 checks green; only evidence `seat-registration`, `seat-refresh`, `seat-replace` missing. Log `C:\Users\okr\AppData\Local\Temp\golem-slice3-uqzSFK\accept.log`.
- **Seat page and Codex runner:** PR #26 merged `4cb24cf` and deployed (seat page live; `APP_CLIENT_SECRET` in the `production` environment); CI `43062181693` built the image with Codex 0.167.1.
  Release v7 (run `43067406314`, native asset `736322171`, SHA-256 `870495496bc268792cbfee4daed8bdc3afcc5b39f90b32062c09627096d3f09d`) verified and pinned by ops PR #13 (`0475bb6`, CI `43068676169`, byte-matched).
  PR #27 (`e045e8c`): router dispatches `openai-codex`/`codex`; PR #28 (`028d796`): seats page requires fresh golem-only sign-ins (bash/zsh and PowerShell steps).
- **Seat evidence (live):** Oliver registered a fresh Codex seat through the page; seat-registration, seat-refresh (Codex refreshes on access-token expiry, so only `exp` was backdated; run `43206088999` refreshed and wrote back, two more runs passed) and seat-replace are recorded in EVIDENCE.md; Slice 3 preflight is fully green.
  The local copy in `~/.secrets/isp-golem-xiv/seat.openai-codex-2` is stale after rotation and should be deleted by Oliver.
- **Copilot removal and harness-only fallback (2026-11-12):** branch `feat/drop-copilot-harness-fallback` on `origin/trunk` `028d796`: commits `b2d90d3` (evidence), `45f728f` (code), `9c171be` (`fix(accept)`).
  Providers are `anthropic`/`openai-codex` only; `choose_seat` keeps the golem's provider and switches to `pi` when its harness does not work there (fixes live red `no-seat`, probe issue 8 comment `6822171957`); contract `fallback` promise reworded with Oliver's authorization; coordinator digest advanced. See DECISIONS 2026-11-12.
  Parent-verified on the committed tree: fmt, clippy native + wasm, router 30, runner 31 + 58 (2 ignored), repair-unbound and deploy tests, all four workerd suites (oversized-body line removed locally only for the Windows reset).
  Merged as PR #29 `7fecb9b` (CI `43211335877`), deployed; release v8 (run `43213854452`, native asset `739341027`, SHA-256 `2d269846b35228c2511358b5c5ef98698bc127b4ead020d8d6b8db00569b6a51`) verified and pinned by ops PR #14 (`b62d509`, CI `43215193615`, byte-matched) within minutes of the deploy.
- **Slice 3 accepted (2026-11-12):** full `--slice 3 --keep` (activity `activity-d4ea3e67-4`) exited 0 with 44/44 gates on deployed `696aafb`, runtime v8; log `C:\Users\okr\AppData\Local\Temp\golem-slice3-0dQAtH\accept.log`.
  First live run of the reworded `fallback` check: check `131212249423` replied with `harness: pi`, `seat: bugabinga/anthropic`, observed `claude-opus-5-5`, `fallback: asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic` (run `43226933720`).
  Kept sandboxes `isp-insoft-gmbh/isp-golem-xiv-test-{core,events,onboarding}-20261112133430`.
  Open, not gated: dashboard last outcome and run boxes; Pi on OpenAI Codex seats; Windows-only oversized-body Worker test reset.
- **Native harness configuration (PR #32, merged `74b09d9`, live):** per Oliver, Claude, Pi and Codex load the target repository's project-local settings, hooks, skills, commands, extensions, MCP servers and context files natively; the runner no longer injects `CLAUDE.md`/`AGENTS.md`.
  CI `43243042077` on `5891911` proved the Pi probes (project extension loads under `--approve`, not under `--no-approve`; `golem_report` conflict exits 1) and the Codex probe (project MCP server starts only with the trust override).
  Claude trust is unproven: CI only shows a trust marker file was written; the probe's outcome is not logged on success.
  Agent decision, not Oliver's: new files under `.claude`, `.codex`, `.pi` or `.agents` that were not in the snapshot are now silently dropped instead of blocking the run, so harness installs and caches work; consequence: a golem asked to add a skill there reports success but its PR lacks those files. Changing or deleting existing config still blocks.
  Release v9 (run `43389057520`, native asset `742815104`, SHA-256 `029be3f35c945b359a9535c4ede7b231baa17706a06c66fc30b3428de3422239`) verified and pinned by ops PR #15 (`a357217`, CI `43390365953`, byte-matched).
  Live proof in `isp-insoft-gmbh/isp-golem-xiv-test-nativeconfig-20261113090534` (issue 1), unique tokens only reachable through project config: Claude (check `131769403103`) used `CLAUDE.md`, the `.claude/skills` skill and the `UserPromptSubmit` hook from `.claude/settings.json`; Pi (check `131772081451`) and Codex (check `131774705838`, Codex seat) used `AGENTS.md` and the `.agents/skills` skill; all replied successfully. This also shows Claude project trust working in practice.
- **Dashboard last result and run boxes:** design and gaps in DECISIONS 2026-11-13; verified locally (router 35, runner 34 + 64, workerd suites) before PR #34.
- **Dashboard shipped and live-proven:** PR #34 merged `d8a6ca1`; release v10 (run `43407846433`, native asset `743214211`, SHA-256 `e36151dcfa2bf40c0a6b63fc02147bd202cdec6abadcfc852c9d5c994bc69fcb`) pinned by ops PR #16 (`c065d02`, CI `43408810302`, byte-matched) about 10 minutes after the router deploy.
  In `isp-insoft-gmbh/isp-golem-xiv-test-nativeconfig-20261113090534` dashboard #4: after a push re-render, ticking `claudeconv`'s box via the API started run `43409045310` (success), the box was cleared, the reply began `@bugabinga`, and the golem's line became `Last: replied on bugabinga/anthropic, claude, model claude-sonnet-5-5, …, [log]`. A click in the GitHub web page is not separately proven (same `issues.edited` event).
- **Pi on OpenAI Codex seats:** PR #35 (runner) merged `e32c6e3`; release v11 (run `43421782938`, native asset `743524944`, SHA-256 `b825e931bdc35523f53e2195218cc4141bec01b56c48214efeb598c7e22c6c71`) pinned by ops PR #17 (`8859012`, CI `43423147818`, byte-matched); router PR #36 merged `acc2045` enables dispatch.
  Live on the real `SEAT_BUGABINGA__OPENAI_CODEX` in `isp-insoft-gmbh/isp-golem-xiv-test-nativeconfig-20261113090534`: `pioncodex` (pi) run `43427764438`, check `131893226478` replied, `harness: pi`, observed `gpt-6.1-sol`; then `codexconv` (codex) run `43428751065`, check `131896504858` replied on the same seat; both logged `golem seat: unchanged` (token not near expiry).
  Not yet observed live: Pi refreshing the seat and the next Codex run accepting the written-back format; expected once the access token nears expiry (~2026-11-22).
- **Windows Worker tests:** PR #38 merged `9ca67f4` after exact-head Linux CI `44066584742`; trunk CI `44068374065` and deploy `44068374290` also passed.
  Oliver completed WSL trust and tool installation; the `x` routing change is merged.
  `mise run test-worker` passed (activity `activity-26d024ae-1`, exit 0, 210.41 s): native Worker build followed by all four unchanged suites in WSL, including the oversized-body 413 assertion in `router/tests/worker.mjs`.
  Fmt, `x` Clippy, its three unit tests and Markdown checks passed.
  WSL errors fail the task; no automatic retry or body draining.
  A router-side drain was tried and reverted (contradicts DECISIONS 2026-11-03).
- **Next:** Slice 4 and the operator guide.
- **Current branch:** `docs/ordinary-migration-live-proof`.
- **UI guidance:** Oliver likes the simple admin style; proposed permanent rules in `DEVELOPMENT.md` await explicit approval and have not been written.
- **Last UI implementation deployment verified (2026-11-17):** PR #41 merge `b88f5f87095a7c72315558539c8c67d448134fc3`; live health and asset hashes match, trunk CI `44110999820` and deploy `44111000121` succeeded.
  Runtime pins native v11 (ops PR #17); live private workflow release URL independently checked.
  Deployed coordinator source is unchanged by PR #41; digest `57d91fc2d7a65c0e7a124bb37c83876ef399408a2f08b3db9f9d2f4545f10462` is pinned in `tools/repair-unbound.mjs`.
- **Released native source (current):** PR #35 commit `e32c6e305505c2963d51c4c0ae9acd10fa9f92bb`, release v11 from successful workflow `43421782938`.
  Native asset `743524944`, SHA-256 `b825e931bdc35523f53e2195218cc4141bec01b56c48214efeb598c7e22c6c71`; GitHub release metadata independently rechecked.
- **Historical v5 artifact verification:** PR #14 commit `49be24620721baed5c0632395209503b1f81ef0b`, release v5 from successful matching release workflow `42051948501`.
  Annotated tag `ac0f538c3980ea33a73e967106b8adc3ad2e4c1a` points to this commit.
  All seven declared assets downloaded and verified against GitHub digests; aggregate/native checksums, archived-vs-standalone binary and license source bytes matched; scratch removed.
  Native asset `713835375`, SHA-256 `a97b865e9a12a921841e7e71b686154229c363b60c2933187ebe590894dbca2f`.
  Release container remains mutable; no independent build attestation or immutable-container claim.
- **Historical v5 private ops:** PR #10 merged as `b0ceede98f7495c86852baa88f4868eb40262aae` after exact-head CI `42054652720` on `a3efde5a108ff57e18027d1ea68fa2c377c74193`.
  Runtime workflow byte-compared with reviewed v5 template; SHA-256 `21f209196050f761bedebb4e27fd0a81b285b0bc14030d19c01061ec43072cc4`.
  Public template/historical-fixture PR #13 merged as `976a4fd900336006a1293b2818c7aa2ebe8c056b`; trunk CI/deployment succeeded and live identity verified.
  `GOLEM_PROTOCOL_VERSION=2` remains enabled.

## Verified recoveries

- **Original orphan:** delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a`, original comment `6620338489`, original check `126334264637`, run `41772599688`, outcome replied.
  Strict observer exited 0: original prepared identity, source/request receipts, bound-before-terminal history, queue/index/cursor retirement, all five ordered native phases.
  This was recovery of the original request, not supersession or fresh publication.
- **Failed scheduled startup:** delivery `f37f6672-d908-4ffa-6488-c011eb2f94c7`, check `126391910880`, failed run `41774622060`.
  After confirmed corrected deployment, guarded helper applied only administrative association; candidate evidence tag `31a8daa0569f0ccc8c6e67587c03d384a41a6916`.
  Normal Coordinator reconciliation produced blocked audit, terminal state and resource retirement.
  Read-only terminal observer exited 0: matching bound snapshot `9b8b149ef2508ad8279b070cd385f7f87089757e`, terminal snapshot `018f1c612059260ae324f6798c4073b0ce448c6c`, receipt pointing to request tag `760eaa30439f66a4313d3c69228216ebbe19d026`, current row absent.
  Provider execution explicitly unproven; this is not schedule-owner evidence.
  No reset, redispatch, manual completion or pending-request deletion was used.

## Outcome proofs and next action

- **Fresh proof process:** `activity-ac784509-3` completed with exit code 0, running `tools/prove-slice2-outcomes.mjs --prove --repo isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e` using released runner `v3`.
  All final bot-owner, timeout/continuation and genuine cron assertions passed.
  Bot-owner: check `126448312133`, immutable delivery `8f3ad966-5b73-5975-cfe3-86c919c853d0`, run `41792488962`, actor `github-actions[bot]`, owner/paying seat `bugabinga/anthropic`, observed model `claude-sonnet-5-5`, no_change with reason and no response comment.
  Timeout run `41792613276` / check `126448315273` published incomplete commit `94b851f`; continuation `41793038096` / check `126450510319` published child `b4f48c7` on same PR #3, preserving exact first line and appending the second.
  Genuine scheduled run `41800937735` / check `126476131244` completed no_change on owner's `bugabinga/anthropic` seat with observed `claude-sonnet-5-5`, null human context, reason and no unsolicited comments.
  Full Slice 2 acceptance executed and failed; slice remains incomplete.
- **New scheduler gap:** live daily cursor had `done=true`, 38 completed repositories, no selected work.
  Existing early return prevents repositories added after convergence from being discovered until next UTC day.
  Deployed correction clears only the done barrier, preserving completed IDs, prepared requests, daily claims and bounds; completed-repository definition and allowlist changes remain day-bound.
  New built-Worker regression was red on old code, green after correction, including restart and same-day late repository admission.
  Raw enqueue-attempt tracking proves rescans do not retry completed occurrences and late occurrence has exactly one attempt/check.
  Independent different-model review found no scoped blocker; latest fresh full CI passed after one earlier failed attempt.
- **Latest full acceptance:** `mise run accept -- --slice 2` exited 1.
  Static/evidence, all Slice 1 scenarios, change-pr and continue-branch passed.
  Forbidden-path audit lacked blocked outcome; serial timed out at 45 minutes; stop never dispatched.
  Event sandbox creation returned GitHub HTTP 500, so event-trigger and no-loop-event had no executed checks.
- **Retained failure, terminal blocked and journal retirement verified:** run `41802954891` ended before admission with static `queue conflict`; delivery `eec9cfc0-d82c-11f1-97dc-9f61ec87b7e3` was administratively bound, then reconciled normally to terminal blocked.
  Read-only observation verified bound snapshot `e3357477f34edb2016df0dd8744d49118053a860`, terminal queue blob `b28ce9ca0060c1906e7a4e8546bd96769ea57d52`, check `126482208907`, and exact private blocked/unconfirmed audit `281795133f8b1067d174205ae402f55640706c5e`.
  Initial observation was terminal only; the corrected observer later exited 0 after a normal reproduction enqueue.
  Current queue blob `78bf8f4407e8585c9f52300040f9db3a860c2d79` has no row; terminal ref points exactly to original request tag `71d4676c50641a33b01fbef512a7d6724b4355ff`.
  Separately verified original bound snapshot and exact private blocked audit remain intact; provider execution remains unproven.
  Retirement happened through enqueue, not polling or manual request deletion.
  Second hello and slow state has not been reverified.
  Do not drop/reset these requests or fabricate successful audits.
  Investigate shared-branch GitHub Contents CAS conflicts versus transient run metadata; exact old denial cause remains unknown.
  Successful logs can be labeled UNKNOWN STEP by gh, so absence of the named-step regex is not absence of lifecycle markers.
- **Confirmed conflict mechanism:** isolated marked private sandbox produced one HTTP 409 per pair in all five concurrent writes to different files on one branch; losing file's blob remained unchanged.
  Probe repository cleanup was verified; this demonstrates the shared-branch conflict mechanism, not the exact historical denial predicate.
- **Local correction verified:** frozen queue SHA/bytes, bounded selective retry/readback, divergent-state fail-closed handling, static diagnostics and run-metadata 404 as transient.
  Correlated completed failures can persist a private blocked/unconfirmed audit when check and repository are both unavailable, then retire normally; existing-repository missing checks still retain ownership.
  Audit tag reuse validates full immutable identity.
  Known v3 operator profile preserves exact provenance, dual legitimate request-environment records and deployment/CAS guards; actual read-only inspection passed for retained failed run with targetUnavailable=true.
  PR #11 exact-head CI `41821200011` passed on `e89adebf014ab945cffe90c6182a83e25112e458`; protected squash merge `65aff0e4a4de1ac64329e0115a31df0d9cffee79` deployed successfully through `41822188562`, trunk CI `41822188764` passed.
  Live health/source digest were verified before guarded association of run `41802954891`; helper returned applied-association with candidate evidence `81674e2077ebe8ba39dcd66e1c3a4c90c227e25e`.
  Subsequent observation proved normal terminal blocked reconciliation and exact private audit, not retirement or successful provider execution.
  Parent fresh full CI exited 0; independent security review closed findings.
  Maintenance guard matrix also joins the normal test/CI command, rather than depending only on explicit local execution.
  All 68 Coordinator scenarios remain under unchanged 60s wrapper through isolated fixture batching, not skipped tests or raised limits.
- **Forbidden-path cause reproduced:** retained sandbox `isp-insoft-gmbh/isp-golem-xiv-test-forbidden-mw28ms64-8bae596d`, source comment `6633850119`, check `126651646306`, run `41852382351`, response comment `6633880139`.
  Released v3 correctly refused the protected edit and left the file unchanged, with no branch or PR; native outcome was nevertheless replied/success.
  This is a reporting defect, not evidence of a protected-write bypass.
  Local candidate adds explicit validated writable disposition; unit tests and CLI feature inspection are not provider compatibility proof.
  Public candidate PR #12 source `03867bac493c361a2ea0eb00815b91e7d672e13c` passed exact-head CI `42013130443`; production remains native v3 pending release/pin.
  Actual Linux/provider attempt `41868477879`, job `126708790091`, completed failure at agent-execution stage, before either typed-result marker.
  Native seat idle and full-window checks passed; independent complete inventory found zero overlapping native runs.
  No compatibility or real-publication proof; temporary provider secret was removed and private test PR #4 closed unmerged.
  Earlier attempt `41861343748` failed before provider execution because installed gh rejects combined --slurp/--jq; corrected inventory query was validated independently.
  Credential-free Linux diagnostic `41870956561` / job `126717544133` completed: both baseline and candidate started successfully, then returned expected authentication failures with no credential and network disabled.
  Candidate initialization included StructuredOutput automatically; no startup flag/tool incompatibility was established.
  This excludes only pre-auth startup failure, not authenticated/provider behavior; original live failure remains unexplained because nonzero execution streams are discarded.
  Private ops PR #5 is diagnostic-only and remains outside the production default branch.
  User approved two additional bounded review calls; diagnostic cleanup finding fixed before execution, with no further production-code changes.
  Hosted authenticated comparison `42005164648`, head `872f849`, passed both idle guards: pages=1, active=0.
  Both greeting records had creation/exit zero, no timeout or truncation, successful terminal/model usage, native parser success; schema returned completed. No native overlap; temporary secret removed.
  This establishes authenticated greeting protocol behavior only, not protected/allowed or publication compatibility.
  Subsequent unchanged provider controls `42006322710` failed the allowed exact-change assertion after protected case passed.
  One isolated diagnostic repeat `42008671537`, head `909e522636a49c92342af8377a9ba11c9885ef32`, verified pinned source `21ef880` and original test SHA-256 `d43655e10efd46ac726c13ec763e43a5dbef73a942d764b76f5efa3171feade1`; every original source byte/assertion was retained around the diagnostic insertion.
  Protected control passed typed blocked, protections and mock-publication checks.
  Allowed diagnostic: disposition=completed, count_ok=true, path_ok=true, bytes_ok=false, mode_ok=true.
  Full single test failed; idle/window checks and independent inventory showed no overlap, temporary-secret removal exited 0.
  No private content was emitted; exact-byte mismatch cause remains unknown. No provider-compatibility or real-publication claim.
- **Deterministic control approved:** replace prose-directed file contents with an exact printf command in the allowed-case instructions.
  Expected bytes, path, count, mode, verification and publication assertions remain unchanged; no production code or judge change.
  Local full CI passed; actual Linux/provider run `42013269480`, private workflow head `e8e7ee5`, job `127197354765`, completed successfully against source `03867bac`.
  Both controls and the complete single integration test passed: protected typed blocked, allowed typed completed, exact bytes/path/count/mode true; observed model claude-sonnet-5-5.
  Hosted guards and independent complete inventory verified no native overlap; temporary provider secret removed and independently absent.
  This is actual provider/container behavior with mock publication, not real GitHub publication or completed acceptance.
- **Real GitHub proof passed:** driver `C:\Users\okr\AppData\Local\Temp\isp-golem-live-controls.mjs` runs on sandbox `isp-insoft-gmbh/isp-golem-xiv-test-controls-mw3gyop7-445293cc`, expecting exact private ops commit `5a4e973` and released v4.
  Protected case verified read-only without redispatch: comment `6656976776`, check `127220936407`, delivery `0de41650-d90d-11f1-81aa-c861ecb10a06`, run `42020289697`, original request tag `ef7781a4b8ba132d05862e85854e7f0b403e1889`.
  Typed refusal, observed model, all five lifecycle phases, terminal binding, protected-file preservation and absence of branch/PR passed.
  Initial observer wrongly required an optional reason in check summary; it now verifies the exact native static refusal and App feedback instead, without changing production code or criteria.
  Continuation `C:\Users\okr\AppData\Local\Temp\isp-golem-continue-v4-controls.mjs` exited 0 with all three verified records; no truncated output.
  Writer comment `6657267593`, check `127227927227`, delivery `3b17ac20-d90f-11f1-8533-379d53454bf6`, run `42022373569`: exact one-file publication, 16-byte oracle, mode, parent, App-authored PR #2/feedback, phases and terminal binding passed; commit `fe47605bcd87421f16a7da788874de2b2ce992ca`.
  Retained sneaky comment `6657302443`, check `127228744030`, delivery `7df8cbf0-d90f-11f1-9f95-bb52fa7b23c6`, run `42022613897`: typed refusal, protected content, absence of branch/PR, phases and terminal binding passed; original source/check identities preserved.
  Actual request branch names independently matched observed branches; all three completed native job windows were non-overlapping.
- **Acceptance failed:** unchanged full Slice 2 activity `activity-090b5926-3` exited 1; 3455 output bytes, untruncated, after all 18 preflight checks passed.
  Raw output remains private at `C:\Users\okr\AppData\Local\Temp\golem-slice2-xq6wcl\accept.log`; sanitized gate/error records available in `C:\Users\okr\AppData\Local\rtk\tee\1793883164_mise_exe_0037ff.log`.
  Forbidden-path, serial, stop, event-trigger and all Slice 1 checks passed.
  `change-pr`: no PR within unchanged 20m; `continue-branch` blocked by that failure; `no-loop-event`: echo did not reply within unchanged 15m.
  Retained core/events sandboxes: `isp-insoft-gmbh/isp-golem-xiv-test-core-20261105130008`, `isp-insoft-gmbh/isp-golem-xiv-test-events-20261105130008`.
  Changer check `127234241422`, delivery `30bed440-d911-11f1-89ac-e59b2587d3ce`, comment `6657535091`, run `42024257200`; validated typed blocked, observed model, all five phases.
  Echo completed check `127249278530`, delivery `719a6dce-3899-05b1-dfc4-444ba7b15f3d`, comment `6658160790`, run `42028717923`; same typed-blocked/phase evidence.
  Echo also has queued sibling check `127249296915` with the same delivery; this proves duplicate check creation, not duplicate execution; preserve identities, no manual terminalization.
  Read-only historical request inspection confirms changer mention suffix preserved, echo comment body omitted despite valid comment identity.
  Source inspection confirms event-comment answers are suppressed by mention-only feedback gating; writable prompt does not explicitly delegate commits to the host.
  Changer commit-delegation explanation remains a hypothesis, not recovered model reasoning; host discarded its structured refusal message.
- **Candidate implementation:** signed event comment bytes preserved within existing dispatch bounds; writable prompt delegates commits/publication to host.
  Event-comment feedback validates API-bound actor/comment/issue/User type without changing `human()` authorization; bot, schedule and absent issue/comment contexts remain silent, owner fallback separately validates actor membership.
  Late lookup failure after initial successful validation attempts blocked check with observed model, no change publication, retained completion boundary; regression is unit-level, not real coordinator proof.
  Independent formatting, Clippy and Rust tests passed: native 63 passed/2 ignored, router 15 passed.
  Ignored provider integration adds unchanged changer-style commit-request case while retaining both previous control oracles; provider/live behavior is unrun.
  Windows full CI failed twice and latest Worker task failed at unchanged oversized-body assertion due Miniflare/Undici ECONNRESET before HTTP status; retain these failures, no guard/assertion/timeout changes.
  Fixed-artifact diagnostic previously passed all four runtime stages without rebuilding and with unchanged artifact/harness hashes; this does not establish full CI or resolve Windows transport.
  Latest explicit `git diff --check` and format check passed after the interrupted batch.
- **Canonical Linux CI verified:** public PR #14 exact head `080713783a75e4218806f8e1671a0eb39ec0a1ad`, run `42039276650`, job `127285477751`, required ci green.
  Every job step succeeded, all four complete Worker-suite markers verified, oversized-body assertion byte-identical to prior trunk, committed diff check passed.
  This is platform-specific validation; Windows transport remains unresolved, no more Windows retry.
- **Actual-provider candidate controls passed:** private test-only PR #9 closed unmerged, branch `probe/context-controls`, exact private head `229f2ca3ac154ffa4a073bce45eab61e00707993`, run `42040986651`, job `127291413828`.
  Public source pinned to `0807137`, unmodified test SHA-256 `b304179a8ae0487ea876213099b0b31c19973392b8c3a358abb28ef9a1bc4c75`.
  Observer `C:\Users\okr\AppData\Local\Temp\isp-golem-watch-context-controls.mjs`, activity `activity-090b5926-4`, exited 0 with full untruncated output.
  verifiedActualProviderCases=true, exactly one selected/passed integration test, protected blocked, allowed/changer completed, model claude-sonnet-5-5.
  All original exact-change/mock-publication assertions passed; both hosted inventories one page/zero active, independent native window and active inventories empty.
  Temporary secret removal exited 0, temporarySecretAbsenceVerified=true; parent independently rechecked absence before closing private PR #9.
  This is actual Linux/provider execution with mock publication; real GitHub behavior remains unproven for this candidate.
- **Merge blocked, no bypass/release:** final docs head `05a2dc9f5a4423cc3201758d24a6b9df2fd07c40` passed exact-head CI `42043789832`, but two unresolved valid review threads prohibited merge.
  `PRRT_kwDOVhkLbc60GFOc`: complete event context could exceed its bound only after durable journaling, stranding event work/index capacity.
  `PRRT_kwDOVhkLbc60GFOh`: missing deleted comments or different-author edits incorrectly blocked event execution during feedback validation.
- **Review follow-up candidate:** complete non-mention event context now rejects before durable work; old invalid unprepared work follows normal source-archive retirement without a check/admission or fabricated native outcome.
  Known comment 404 or valid other-author destination disables feedback without denying authorized event execution; unknown HTTP errors and malformed ID/issue/type remain fail-closed.
  Actor membership validation remains separate from destination eligibility; mention authorization and payer semantics unchanged.
  Added Worker fixtures for zero-side-effect oversized-context rejection and old-work retirement, plus native deletion/edit/owner-fallback regressions.
  Local formatting, Clippy, diff check and Rust tests passed: native 64 passed/2 ignored, router 15 passed; no Windows Worker retry.
  Linux follow-up `17f5c3b` CI `42047850669` failed in new fixture setup: it incorrectly read a retired source ref instead of its done receipt.
  The new pre-journal rejection assertions passed before that setup failure; old-work retirement assertions had not run, no thread resolved.
  Fixture now reads immutable source through the done receipt and waits for previous work retirement before its side-effect snapshot; new exact-head Linux CI still required.
  Final exact-head Linux CI `42049159644` on `d9b557c` passed all required steps and all four complete Worker stages, including old context rejection/recovery and both native regressions.
  Both threads received assertion-level evidence and were resolved before protected merge; outdated flags were not used as evidence.
  Prior Linux/provider proof remains scoped to its recorded old source; unchanged execution/prompt/parser/sandbox/host-publisher files were independently compared, not a whole-v5-binary provider claim.
- **Real v5 proof stopped:** activity `activity-090b5926-5` stopped/exit1; no echo comment submitted and no driver redispatch.
  First changer verified: comment `6662029945`, check `127344375288`, delivery `14a42220-d92e-11f1-90fe-16fb0fd4ae18`, run `42056029957`, actual App PR #9, commit `91719b5c56af5c433ea3d682097033b64797081e`; file/parent/mode/nonce/feedback, all five phases and terminal binding passed.
  Continuation comment `6662081241`, check `127345695362`, delivery `6271bc60-d92e-11f1-945a-0ee7d18bcfc8` remains queued/unbound; request tag `a06bf59d74ba8bb84500d754fc20363a8379832d`, generation `4b241cec-79d5-4ed9-a420-e83427b4a4b7`.
  Last authoritative queue blob `54b419402222d1cc56ba5a1a2a0a31e690bbfc42`: dispatching, dispatched=true, run_id/attempt=null, original resource and bugabinga/anthropic seat retained.
  Candidate matching-title workflow `42056406734`/job `127345869143` failed download with curl22/HTTP504; validation, preparation and native/provider steps all skipped, cleanup success, no native phases.
  Exact continuation-to-run linkage is NOT established: complete 4195-byte job log and full log archive contain zero request envelopes/delivery/check IDs; artifacts count0, REST attempt has no inputs field, inspected GraphQL WorkflowRun fields provide no inputs.
  Current repair helper accepts v2/v3 only; merely adding v5 provenance cannot replace missing full input/run evidence. No association, reset, check terminalization or reservation mutation performed.
  State lease `C:\Users\okr\AppData\Local\Temp\isp-golem-live-v5-b0ceede98f7495c86852baa88f4868eb40262aae.json` retains identities and prevents blind rerun.
- **Stop fence deployed:** PR #16 exact-head CI `42937471264` on `80cf506`, merged `1b72c822c6e00c23fe5cde6770e5a1b969f67e45`; trunk CI `42938150932` and deploy `42938150832` green; live health ∧ coordinator digest verified.
  Independent read-only review (Opus) found no blocking findings; minor budget/retry-window/wording fixes applied before merge; target-gone stopped rows remain a documented gap.
  Relop unavailable: no reviewer pool configured.
  Stop mention `6784348560` on core issue 3 fenced continuation `6271bc60…` durably at 2026-11-11T01:33:12Z (run/attempt null, blob `5681e33…`).
  Observed after grace: row `terminal`, outcome `stopped`, run/attempt null, blob `8e4effa…`; check `127345695362` cancelled with `reason: stopped before runner admission; no agent execution was granted`.
  Retirement observed at the next enqueue: `golem-terminal/6271bc60…` → original request tag `a06bf59…`, row gone from queue.
- **Resumed proof policy:** script `C:\Users\okr\AppData\Local\Temp\isp-golem-live-continue-echo.mjs` resumes from the verified first changer (pre-fence router `15213da`); continuation ∧ echo run on post-fence router `1b72c82`, runtime unchanged v5 without download retries.
  Result: continuation verified (PR #9 → `fff1ac3…`, run `42942341694`) ∧ echo verified (run `42942532889`, single reply, exact context, loop watch 207 s); details in EVIDENCE.
  Driver exit 1 was its own null-branch assumption; echo checks completed read-only.
- **Scopes granted by Oliver;** private runtime PR #11 merged `abf9489eea62f5f7aac43b1185f048a89e05591e` after CI `42946847311`; live runner.yml byte-matches reviewed template (v5 + bounded download retries).
- **Slice 2 acceptance passed:** unchanged `--slice 2 --keep`, activity `activity-d4ea3e67-2`, exit 0, 32/32 gates ok, log `C:\Users\okr\AppData\Local\Temp\golem-slice2-kaTx7M\accept.log` (private, untruncated).
  Deployed version `3413531` (docs-only over fenced `1b72c82`); `accept.nu`/`contract.nuon` identical to trunk.
  Kept sandboxes `isp-insoft-gmbh/isp-golem-xiv-test-core-20261111033454` and `…-events-20261111033454`; per-case check/run IDs in EVIDENCE.
  Still open, not covered by gates: stopped rows with a gone target repository stay retained; Windows-only oversized-body Worker test reset.
- **Next:** Slice 3. Planned human stops first: browser session for seat page, fresh `codex login` into a new folder, App client secret + OAuth callback (`app-oauth` prerequisite).
  Private runtime retry template blocked: gh token lacks `workflow` scope (branch `fix/download-retries` committed locally in ops checkout, push refused); runtime still v5 without retries.
- **Recovery path chosen:** pre-admission stop fence (DECISIONS 2026-11-06).
  Local coordinator suite 70/70 PASS including two new fence scenarios; repair regressions pass with advanced coordinator digest `999dc9687f55265d793dc054db2bf5814f39bbcf6b04a05de467f8fd18c7dad0`.
  Read-only check: stuck row trigger mention, issue 3, App check queued, sole non-terminal row, queue blob still `54b4194…`; a deployed fence plus `@isp-golem-xiv stop` on issue 3 would select it.
  Fence retires only after 15-minute grace with no active same-title runner run; it neither associates run `42056406734` nor proves its inputs.
  Future bounded artifact GET retries and early request-identity evidence can prevent recurrence but cannot retrospectively prove this run's inputs.
  Public template PR #15 final head `4274bf7ec2f2c51a76c2e02b835798328588396c` passed required CI `42062344979`/job `127367177686`, all steps successful.
  Protected merge produced `15213dad31ae5131e69b666eb7f36f7309f64caf`; its trunk CI/deployment are not yet observed, no runtime recovery implied.
  Live continuation/echo and full Slice 2 remain incomplete.
  Public runner template advances to v4; maintenance tests reconstruct exact historical v2/v3 bytes and retain both fixed digest assertions, without broadening repair profiles.
  Only after compatibility passes: released-candidate real GitHub blocked/allowed publication and retained forbidden-task rerun.
  No production-fix claim until these pass; keep private response content out of public evidence.
  Rerun full Slice 2 acceptance with `--keep` for diagnosis; all three real outcome-evidence promises remain valid but do not replace acceptance.
  Do not promote running processes, local tests or failed-startup cleanup into outcome proof.

## Existing evidence and limits

- **Fresh publication:** separate delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e`, check `126340791321`, run `41758683949`, PR #2 in `isp-golem-xiv-test-mw1gkr0c-e4912816`, commit `318d77b2acfe7b0efee5ab037b0c6256d2695c95`.
  Observer verified fresh commit, nonce, attribution, journal lifecycle and native phase ordering.
  Later delivery `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` replied in run `41760278928`; it is not fresh-publication proof.
- **Previous failed outcome sandbox:** `isp-golem-xiv-test-outcomes-mw1fkhw3-c7d0f62e`, retained for failure evidence.
  Partial run `41774486415` blocked on legitimate local PAX path metadata; scheduled run failed before admission; bot fixture was invalid scalar YAML.
  Native fixes were released in `v3`; fixture now uses a trigger sequence.
- **Verification:** final runtime full CI and dedicated maintenance regressions passed; pinned private archive integration explicitly passed with digest/commit verified before extraction, scratch removed.
  Runtime security review closed findings within 4 calls/60 minutes.
  Discovery review used a different effective model after initial reviewer substitution.
- **Open limits:** genuinely ambiguous dispatch retains ownership; large/changing installations may outlast scan capacity; already-completed repository definition changes wait until next UTC day; non-Linux runner labels unsupported; scoped mutation diagnostics unrun within disk budget.
- **Coverage proposal:** isolated temp worktree `C:\Users\okr\AppData\Local\Temp\golem-coverage-ratchet`, branch `build/coverage-ratchet`, not committed or integrated.
  Auxiliary pinned-nightly host coverage measured; canonical Ubuntu ratchet remains unestablished.
- **Ops checkout:** `C:\Users\okr\AppData\Local\Temp\isp-golem-xiv-ops-bootstrap`, branch `fix/pin-v5`; production pin PR #10 merged, test-only PR #9 remains closed unmerged.
- **Secrets:** provisioned under `~/.secrets/isp-golem-xiv/` and encrypted GitHub/Cloudflare secrets; never print or commit.
- **Blocked on Oliver:** nothing for Slice 2.
  Before Slice 3: GitHub browser session and fresh dedicated Codex sign-in are planned human stops.
- **Judge:** `accept.nu`, `contract.nuon` and configured bare loop gate unchanged.
  Bare `mise run accept` defaults to Slice 1; its success does not establish the binding project goal.
  Completion requires explicit `mise run accept -- --slice 4` and operator guide; both remain unfinished.
