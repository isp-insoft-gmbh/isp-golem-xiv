# Status

- **Current slice:** 2, incomplete.
  Slice 1 passed three consecutive deployed acceptance runs; later regression passed with released runner `v2`.
- **Current branch:** `docs/slice-2-remaining-proof`, recording verified outcome evidence.
- **Deployed Worker:** PR #10 commit `22e8a24a85205cd742fcafad7bdb83073ec7f753`; exact-head CI `41792925145` on `c8106bc`, trunk CI `41793629432` and deployment `41793629564` succeeded.
  Live cursor observation confirms scanning resumed (`done=false`, page 2) without resetting 38 completed IDs.
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

- **Fresh proof process:** `activity-ac784509-3`, running `tools/prove-slice2-outcomes.mjs --prove --repo isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e` using released runner `v3`.
  Last observation: running, bot-owner and timeout/continuation assertions passed; waiting for genuine cron.
  Bot-owner: check `126448312133`, immutable delivery `8f3ad966-5b73-5975-cfe3-86c919c853d0`, run `41792488962`, actor `github-actions[bot]`, owner/paying seat `bugabinga/anthropic`, observed model `claude-sonnet-5-5`, no_change with reason and no response comment.
  Timeout run `41792613276` / check `126448315273` published incomplete commit `94b851f`; continuation `41793038096` / check `126450510319` published child `b4f48c7` on same PR #3, preserving exact first line and appending the second.
  Scheduled execution, overall process result and full Slice 2 acceptance remain unverified.
- **New scheduler gap:** live daily cursor had `done=true`, 38 completed repositories, no selected work.
  Existing early return prevents repositories added after convergence from being discovered until next UTC day.
  Deployed correction clears only the done barrier, preserving completed IDs, prepared requests, daily claims and bounds; completed-repository definition and allowlist changes remain day-bound.
  New built-Worker regression was red on old code, green after correction, including restart and same-day late repository admission.
  Raw enqueue-attempt tracking proves rescans do not retry completed occurrences and late occurrence has exactly one attempt/check.
  Independent different-model review found no scoped blocker; latest fresh full CI passed after one earlier failed attempt.
- **Next step:** observe genuine cron and the proof process's terminal result.
  Fresh native bot-owner and incomplete/continuation proofs passed; only schedule-owner evidence remains before full Slice 2 acceptance.
  Collect its terminal result plus actual bot-owner, timeout publication/continuation and genuine cron evidence, then run full `mise run accept -- --slice 2`.
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
- **Ops checkout:** `C:\Users\okr\AppData\Local\Temp\isp-golem-xiv-ops-bootstrap`, currently `fix/pin-v3`; synchronize/clean after pin milestone.
- **Secrets:** provisioned under `~/.secrets/isp-golem-xiv/` and encrypted GitHub/Cloudflare secrets; never print or commit.
- **Blocked on Oliver:** nothing for Slice 2.
  Before Slice 3: GitHub browser session and fresh dedicated Codex sign-in are planned human stops.
- **Judge:** `accept.nu`, `contract.nuon` and configured loop gate unchanged.
  Overall Slice 4 goal and operator guide remain unfinished.
