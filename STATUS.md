# Status

- **Current slice:** 2, incomplete.
  Slice 1 passed three consecutive deployed acceptance runs.
- **Current branch:** preparing `fix/native-startup-recovery`, with original-recovery evidence and locally verified runtime corrections.
- **Latest protected merge:** PR #8 → `a1efb737da97f86fd72615b4a546d90a1b810b55`, after exact-head `ci` passed on `1ec73dd868be7e987f2ad3c04167ed397aecc778` (`41770161589`).
  Merged-head CI `41770760497` and deployment `41770760575` both succeeded on that exact commit.
- **Previous deployed code:** `0a9555f6c510085d251a6acf891e6740a9cabad4`, PR #7 merged after exact-head `ci` succeeded on `0bf104d` (`41752524809`).
  Trunk CI `41753576752`, deployment `41753576792`, release `v2` (`41754384511`) succeeded.
  Private ops PR #2 merged after green CI; `runner.yml` at `bf750cb` was byte-compared with reviewed `v2` pin.
- **Rollout state:** `GOLEM_PROTOCOL_VERSION=2` re-enabled only after successful corrected deployment and byte-comparison of the private ops `v2` pin.
  Full deployed Slice 1 regression passed with released runner `v2` (private run `41759292169`).
- **Real fresh publication proven:** delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e`, comment `6620598667`, check `126340791321`, private run `41758683949`, PR #2 in `isp-golem-xiv-test-mw1gkr0c-e4912816`, commit `318d77b2acfe7b0efee5ab037b0c6256d2695c95`.
  Read-only observer verified immutable request/comment/check identity, `changed` outcome, nonce content, fresh commit during that run, bound-before-terminal history, matching retirement receipt, and ordered native destruction/publication/release markers.
  Later delivery `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` completed as `replied` in run `41760278928`; it is not fresh-publication proof.
- **Original orphan recovered and verified:** delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a`, original comment `6620338489`, original check `126334264637`, run `41772599688`, outcome `replied`.
  Strict read-only `--watch` exited 0, comparing the exact original prepared request against its new immutable request, matching bound-before-terminal history and both receipts, confirming queue/index/cursor/pending-ref retirement and all five trusted native phases.
  Original source SHA `27e3076d414068015ed26ba7856fc6603cbafb93`; request tag `a80d0b7ed868af6c3ce20e9f248c2c277beb2190`.
  This request was recovered, not superseded; no fresh-publication claim for this reply.
  First watch timed out awaiting terminal compaction, which runs on next admission rather than idle ticks.
  Starting the planned bot-owner proof triggered normal compaction; the unchanged second watch succeeded.
  The old source at `01:49:30Z` preceded queue initialization at `01:50:11Z`; correction establishes bootstrap before claim consumption.
- **Queue-bootstrap milestone:** local implementation establishes the journal branch before consuming a v2 delivery claim, discovers at most one orphan per tick from at most 512 pending source refs, and retains original prepared check/request identity.
  Completion refs retain the original annotated source SHA before index/cursor retirement; pending refs are deleted only after receipt verification.
  Strict GitHub missing-branch Contents semantics reproduced the old cold-mention failure; rebuilt real edge + actual SQLite Coordinator tests passed cold bootstrap, orphan restart, legacy completed-source dedup, retirement interruption recovery, and existing regressions.
  Event-index head rotation passed a dedicated real edge + Coordinator regression: persist failing head plus healthy peer, tick, restart, admit peer exactly once while retaining the failing source/cursor.
  Final fresh `mise run test-worker` rebuilt the Worker and passed all runtime/Coordinator suites, including the unchanged recovery +36 and scheduled-call bounds.
  Explicit 512/513-source and full event-index capacity stress matrices remain unverified.
- **Local publication checks:** fresh full `mise run ci` exited 0 after removing duplicated mention reaction/member/stop handling rather than suppressing the overlong-function lint.
  Different-model scoped review found no concrete correctness/security/regression blocker.
- **Next step:** publish runtime corrections through exact-head green PR CI and deployment, release runner `v3`, synchronize reviewed private ops pin through green CI.
  Only after new Worker deployment may `tools/repair-unbound.mjs --apply` associate the proven failed scheduled workflow; normal reconciliation must produce its blocked audit and terminal retirement.
  Helper read-only inspection passed; no apply or live recovery of this scheduled reservation is claimed.
  Rerun genuine outcome proofs in a fresh valid sandbox using the corrected released runner, then full Slice 2 acceptance.
  Operational proof process `activity-ac784509-2` was stopped and confirmed exited 1; no outcome promise passed.
  Partial run `41774486415` bound/released but blocked before provider execution: actual snapshot has local PAX `path` metadata rejected by the extractor.
  Scheduled run `41774622060` failed before admission with `queue request denied`; its unbound `dispatching` reservation remains retained, blocking the seat.
  A pending core acceptance request is also retained; never discard it to manufacture a pass.
  Bot fixture incorrectly used scalar `triggers: issues.opened` although the parser requires a list; correct the fixture rather than changing bot filtering.
  Baseline was executed post-merge, before reactivation while deployment was underway, not before deployment.
- **Proof helper correction:** GitHub CheckRun has no `created_at`; default list filtering also hides older same-name checks.
  Observer now uses explicit check ID plus immutable request/comment binding, checks fresh publication causally, and rejects the queued orphan and later reply as negative controls.
  `tools/prove-slice2-pilot.mjs --observe <comment URL> --check <check ID>` passed for the changed invocation; both negative controls exited 1.
- **Local verification:** final rebased Slice 2 full `mise run ci` passed; real Rust edge + real SQLite lifecycle tests use mocked GitHub and simulated callbacks.
  Latest native Linux WSL tests/Clippy include label handling and numeric callback wire types.
  These are separate from the real canary proof above.
- **Evidence still missing:** real schedule-owner, bot-owner, incomplete/continuation, full Slice 2 acceptance.
  Original outcomes sandbox is retained for failed-run evidence; new runtime corrections and a valid bot fixture are required before rerunning real proofs.
- **Local runtime correction checks:** strict local PAX paths, raw Win32 component guards, bounded idempotent control retries, unavailable-target preflight and guarded operator association passed final fresh full CI and dedicated repair tests.
  Parent explicitly reran the pinned private archive integration: 1 passed, digest/commit verified before extraction; no private scratch retained.
  Different-model security review closed all findings within 4 calls/60 minutes; changes are not yet deployed or released.
- **Open gaps:** genuinely ambiguous dispatch still retains resources; large/changing installations can outlast daily scan capacity; non-Linux runner labels unsupported; scoped Slice 2 mutation diagnostics unrun within disk budget.
- **Coverage proposal:** isolated temp worktree `C:\Users\okr\AppData\Local\Temp\golem-coverage-ratchet`, branch `build/coverage-ratchet`, measured WSL host line/branch counts using auxiliary pinned nightly.
  Not committed, not enabled in CI, not an established canonical Ubuntu ratchet.
  Parent workspace unchanged; proposal resources/side effects need review before publication.
- **Retained private test repositories:** refusal proofs `isp-golem-xiv-test-mw126nzr-2665e70b`; pilot `isp-golem-xiv-test-mw1gkr0c-e4912816`; outcomes setup above.
- **Temporary ops checkout:** `C:\Users\okr\AppData\Local\Temp\isp-golem-xiv-ops-bootstrap`, retain while release pins change.
  Debug tail deletion was independently verified; scratch diagnostic scripts can be removed once their metadata evidence is recorded.
- **Secrets:** provisioned in code `production` environment and private ops; local files under `~/.secrets/isp-golem-xiv/`, never print or commit.
- **Blocked on Oliver:** nothing for Slice 2.
- **Planned Slice 3 stops:** GitHub browser session and fresh dedicated Codex sign-in.
- **Judge:** `accept.nu`, `contract.nuon`, configured loop gate unchanged.
  Overall Slice 4 goal and operator guide remain unfinished.
