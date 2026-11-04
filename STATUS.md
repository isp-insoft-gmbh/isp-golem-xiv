# Status

- **Current slice:** 2, incomplete.
  Slice 1 passed three consecutive deployed acceptance runs.
- **Current branch:** `fix/queue-bootstrap`, local production fix pending parent review/publication.
- **Deployed code:** `0a9555f6c510085d251a6acf891e6740a9cabad4`, PR #7 merged after exact-head `ci` succeeded on `0bf104d` (`41752524809`).
  Trunk CI `41753576752`, deployment `41753576792`, release `v2` (`41754384511`) succeeded.
  Private ops PR #2 merged after green CI; `runner.yml` at `bf750cb` was byte-compared with reviewed `v2` pin.
- **Rollout state:** `GOLEM_PROTOCOL_VERSION` removed after canary diagnosis; legacy routing restored.
  Full deployed Slice 1 regression passed with released runner `v2` (private run `41759292169`).
- **Real fresh publication proven:** delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e`, comment `6620598667`, check `126340791321`, private run `41758683949`, PR #2 in `isp-golem-xiv-test-mw1gkr0c-e4912816`, commit `318d77b2acfe7b0efee5ab037b0c6256d2695c95`.
  Read-only observer verified immutable request/comment/check identity, `changed` outcome, nonce content, fresh commit during that run, bound-before-terminal history, matching retirement receipt, and ordered native destruction/publication/release markers.
  Later delivery `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` completed as `replied` in run `41760278928`; it is not fresh-publication proof.
- **Unresolved orphan:** original delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a`, comment `6620338489`, check `126334264637` remains queued.
  Immutable event source exists, but no request tag, terminal receipt, current queue row or run ID exists.
  This request was not intentionally superseded.
  Source tag timestamp `2026-11-04T01:49:30Z` precedes queue-branch initialization commit `15a5b62` at `01:50:11Z`.
  V2 mention persistence lacks the coordinator bootstrap precondition used by event/schedule paths.
- **Queue-bootstrap milestone:** local implementation establishes the journal branch before consuming a v2 delivery claim, discovers at most one orphan per tick from at most 512 pending source refs, and retains original prepared check/request identity.
  Completion refs retain the original annotated source SHA before index/cursor retirement; pending refs are deleted only after receipt verification.
  Strict GitHub missing-branch Contents semantics reproduced the old cold-mention failure; rebuilt real edge + actual SQLite Coordinator tests passed cold bootstrap, orphan restart, legacy completed-source dedup, retirement interruption recovery, and existing regressions.
  Event-index head rotation passed a dedicated real edge + Coordinator regression: persist failing head plus healthy peer, tick, restart, admit peer exactly once while retaining the failing source/cursor.
  Final fresh `mise run test-worker` rebuilt the Worker and passed all runtime/Coordinator suites, including the unchanged recovery +36 and scheduled-call bounds.
  Explicit 512/513-source and full event-index capacity stress matrices remain unverified.
- **Local publication checks:** fresh full `mise run ci` exited 0 after removing duplicated mention reaction/member/stop handling rather than suppressing the overlong-function lint.
  Different-model scoped review found no concrete correctness/security/regression blocker.
- **Next step:** publish through exact-head green CI and deployment, then re-enable canary and verify the original live orphan completes.
  Local mock-GitHub success does not close that live proof.
  Afterwards run unattended/incomplete evidence and full Slice 2 acceptance.
- **Proof helper correction:** GitHub CheckRun has no `created_at`; default list filtering also hides older same-name checks.
  Observer now uses explicit check ID plus immutable request/comment binding, checks fresh publication causally, and rejects the queued orphan and later reply as negative controls.
  `tools/prove-slice2-pilot.mjs --observe <comment URL> --check <check ID>` passed for the changed invocation; both negative controls exited 1.
- **Local verification:** final rebased Slice 2 full `mise run ci` passed; real Rust edge + real SQLite lifecycle tests use mocked GitHub and simulated callbacks.
  Latest native Linux WSL tests/Clippy include label handling and numeric callback wire types.
  These are separate from the real canary proof above.
- **Evidence still missing:** real schedule-owner, bot-owner, incomplete/continuation, full Slice 2 acceptance.
  `tools/prove-slice2-outcomes.mjs --setup` created `isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1fkhw3-c7d0f62e`; no proof dispatches or mentions started.
- **Open gaps:** unbound ambiguous dispatch retains resources indefinitely; large/changing installations can outlast daily scan capacity; non-Linux runner labels unsupported; scoped Slice 2 mutation diagnostics unrun within disk budget.
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
