# Status

- **Current slice:** 2, incomplete.
  Slice 1 passed three consecutive deployed acceptance runs; later regression passed with released runner `v2`.
- **Current branch:** `fix/typed-policy-refusal`; PR #11 remains deployed, full Slice 2 acceptance still failed.
- **Deployed Worker:** PR #11 commit `65aff0e4a4de1ac64329e0115a31df0d9cffee79`; exact-head CI `41821200011` on `e89adeb`, trunk CI `41822188764` and deployment `41822188562` succeeded.
  Live health and reviewed Coordinator digest `c15e772b97c52c70f63e88b62fd58c0859c370a1dc70c395c19fb18893b69891` verified before maintenance association.
  Prior PR #10 discovery correction remains included.
- **Released native source:** PR #9 commit `00d62520f873ec404d66f409d51db18933f82ae8`.
  Exact-head CI on `644c930` (`41787665840`), trunk CI `41788243519` and deployment `41788243140` succeeded.
  Release `v3` succeeded (`41788750529`) from this commit.
- **Private ops:** PR #3 merged as `8aca09966fff94e36bb757c4566d08aaeae5530d` after exact-head CI on `87e5bff` (`41791328946`).
  Runtime workflow byte-compared with reviewed local `ops/runner.yml`, now pins released `v3`.
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
- **Next step:** protected merge/release of reviewed candidate, pin released runner through private ops PR, then real GitHub blocked/allowed publication and retained forbidden-task rerun.
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
- **Ops checkout:** `C:\Users\okr\AppData\Local\Temp\isp-golem-xiv-ops-bootstrap`, currently `probe/writable-controls`; test-only PR #7 must close unmerged after proof, then prepare released-runner pin from current ops trunk.
- **Secrets:** provisioned under `~/.secrets/isp-golem-xiv/` and encrypted GitHub/Cloudflare secrets; never print or commit.
- **Blocked on Oliver:** nothing for Slice 2.
  Before Slice 3: GitHub browser session and fresh dedicated Codex sign-in are planned human stops.
- **Judge:** `accept.nu`, `contract.nuon` and configured bare loop gate unchanged.
  Bare `mise run accept` defaults to Slice 1; its success does not establish the binding project goal.
  Completion requires explicit `mise run accept -- --slice 4` and operator guide; both remain unfinished.
