# Evidence

Proofs for promises of kind `evidence` in [contract.nuon](contract.nuon), which `accept.nu` cannot drive itself.
One line per promise: `- <id>: <link to a run, comment, or screenshot, or path of a test> <what it shows>`.

- schedule-owner: [genuine scheduled run 41800937735](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41800937735) `router/tests/worker.mjs` covers daily occurrence identity, owner selection, restart and duplicate prevention; real Cloudflare cron discovered the fresh sandbox, completed its only scheduled check `126476131244` for delivery `479fffd1-0986-c3b9-1fff-69f0544c6caa` as no_change on `bugabinga/anthropic` with observed `claude-sonnet-5-5`, null human context, reason and no unsolicited comments; full real outcome-proof process exited 0.
- incomplete: [timeout run 41792613276](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41792613276) exhausted its 120s budget, completed check `126448315273` as neutral/incomplete and published verified `FIRST` content in commit `94b851f29f982fb194d8ba844007b85f7834dfe8`; [continuation 41793038096](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41793038096) completed check `126450510319` as changed, preserved that exact first line and appended `SECOND` in child commit `b4f48c72a8ddbaa64c200205bf29682c6bd4f02a` on the same [PR #3](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e/pull/3), with owner seat/model footers and matching immutable request/terminal bindings verified by `tools/prove-slice2-outcomes.mjs`.
- bot-owner: [real owner-seat run 41792488962](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41792488962) `router/tests/worker.mjs` covers the owner-seat decision; actual GitHub Actions bot created [issue #1](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e/issues/1), immutable delivery `8f3ad966-5b73-5975-cfe3-86c919c853d0` binds check `126448312133` to actor `github-actions[bot]` and owner `bugabinga`, completed no_change on `bugabinga/anthropic` using Claude `claude-sonnet-5-5`, with a reason and no response comment.
- refused-no-seat: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613042205) `tools/prove-refusals.mjs --repo isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b` cloned an actual App delivery, signed a simulation for member `akonczalla` without a seat, observed the App's `no-seat` reply and registration link, with zero matching runner runs.
- refused-not-member: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613046125) The same harness signed the recorded delivery as verified non-member `octocat`, observed the App's `not-member` reply, with zero matching runner runs.

Slice 1 full deployed acceptance passed three consecutive times with released runner `v1`:

1. [run 41733197463](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41733197463)
2. [run 41733919165](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41733919165)
3. [run 41734601072](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41734601072)

Each acceptance invocation independently checked repository-only sentinel content, requested exact text, completed attribution audit and absence of App loops.
Acceptance-created scenario repositories were cleaned by the unchanged judge.

Both signed simulations reached deployed Worker `8b13b0f1a33b10d9b26738002e6d697785a228d0`.
These prove router decisions using recorded deliveries, not actions performed by those account holders.
The private evidence repository is retained while these links are needed; the harness remains the reproducible test path if routine stale-repository cleanup later removes it.

## Slice 2 canary, not completed acceptance

Real fresh publication: [comment 6620598667](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw1gkr0c-e4912816/issues/1#issuecomment-6620598667), check `126340791321`, delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e`, [private run 41758683949](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41758683949), [PR #2](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw1gkr0c-e4912816/pull/2), [new commit 318d77b](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw1gkr0c-e4912816/commit/318d77b2acfe7b0efee5ab037b0c6256d2695c95).
The read-only observer bound immutable request to that comment/check, verified `changed` attribution and exact nonce at the fresh commit, matched bound-before-terminal GitHub journal history and retirement receipt, and checked ordered trusted native markers: admitted → containers_destroyed → publication_credentials_minted → publication_complete → reservation_released.
Command: `mise exec -- node tools/prove-slice2-pilot.mjs --observe https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw1gkr0c-e4912816/issues/1#issuecomment-6620598667 --check 126340791321`.

Distinct deliveries remain distinct:

| Delivery | Source comment | Check | Run | Observed result |
|---|---|---|---|---|
| `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a` | `6620338489` | `126334264637` | `41772599688` | recovered after correction; replied; original identity and terminal retirement verified |
| `7d8caf40-d7f4-11f1-9909-46c276fd737e` | `6620598667` | `126340791321` | `41758683949` | changed; matching terminal receipt; fresh publication above |
| `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` | `6620803848` | `126346007814` | `41760278928` | replied; current terminal row; existing content, not fresh publication |

Observer negative controls exited 1 for the orphan's missing immutable request tag and the later reply's lack of a changed outcome.
Successful handling of the later deliveries did not complete or supersede the original request.
The orphan's source tag at `01:49:30Z` predates queue-branch initialization at `01:50:11Z` on 2026-11-04; recovery required a product correction, not a verifier relaxation.

After pinning released runner `v2` with protocol-2 routing disabled, full deployed Slice 1 regression passed ([run 41759292169](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41759292169)).
All three real Slice 2 outcome-evidence promises are recorded; full Slice 2 acceptance remains pending.

## Original orphan recovery after corrected deployment

PR #8 passed exact-head CI `41770161589` on `1ec73dd868be7e987f2ad3c04167ed397aecc778`; merged commit `a1efb737da97f86fd72615b4a546d90a1b810b55` passed trunk CI `41770760497` and deployment `41770760575` before canary reactivation.
Original [comment 6620338489](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw1gkr0c-e4912816/issues/1#issuecomment-6620338489), original check `126334264637`, delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a` completed as `replied` in [run 41772599688](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41772599688).

A strict read-only observer exited 0 after checking:

- Original annotated source `27e3076d414068015ed26ba7856fc6603cbafb93` retained through `golem-event-done/f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a`.
- Exact original prepared request preserved after excluding only newly assigned `queue_generation`; canonical SHA-256 `75ac1186e6b700a2333fbbc9780dfa7f955e746cbc53b6755d691fca4e57bc18`.
- Matching immutable request tag `a80d0b7ed868af6c3ce20e9f248c2c277beb2190`, also retained by `golem-terminal/f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a`.
- Matching resource, seat, generation, original check/comment and actual run in [bound snapshot](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/blob/38b027b908b6c9d245fea905105ea175f321e620/queue.json) before [terminal snapshot](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/blob/fde31b68aff091c98ee684766e89f5d40c2d6182/queue.json).
- Current queue row, event index entry, event cursor and pending source ref removed; source remains reachable through completion receipt.
- Ordered trusted native markers: admitted → containers_destroyed → publication_credentials_minted → publication_complete → reservation_released.
- Original nonce still exact in owned PR #2.

The first watch timed out waiting for terminal compaction: completed terminal rows compact on subsequent admission, not idle ticks.
Starting the planned bot-owner proof triggered that normal compaction; the unchanged second watch succeeded.
This proves the original orphan was recovered and retired; it does not turn its reply into fresh publication or complete Slice 2 acceptance.

## Failed-startup reservation reconciliation

After PR #9 deployed `00d62520f873ec404d66f409d51db18933f82ae8`, guarded operator association linked scheduled delivery `f37f6672-d908-4ffa-6488-c011eb2f94c7` to its proven completed failed [run 41774622060](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41774622060).
Candidate evidence tag `31a8daa0569f0ccc8c6e67587c03d384a41a6916` records the immutable request/log/workflow checks and deployment guard.
The helper did not reset, redispatch or mark completion; normal Coordinator reconciliation completed original check `126391910880` as blocked.

Read-only observer exited 0, checking exact request tag `760eaa30439f66a4313d3c69228216ebbe19d026`, generation and run identity in [bound snapshot](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/blob/9b8b149ef2508ad8279b070cd385f7f87089757e/queue.json) before [terminal snapshot](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/blob/018f1c612059260ae324f6798c4073b0ce448c6c/queue.json), matching terminal receipt and current queue-row absence.
This proves failed-startup cleanup and resource retirement, not provider execution or successful scheduled owner-seat behavior.
Fresh `v3` bot-owner, timeout/continuation and genuine scheduled execution are proven in `isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e`; operational proof process exited 0 after all final assertions.

## Writable protocol pre-rollout validation

[Private Linux/provider controls 42013269480](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/42013269480), workflow head `e8e7ee5cab7a14032a0175db31180a5726cef9cb`, job `127197354765`, completed successfully against candidate source `03867bac493c361a2ea0eb00815b91e7d672e13c`.
Pinned test-source SHA-256 `9d64df8e097cf03b9865a6dcae9211ab12cd87c9a66a8c3e5a0a5056b9915229` was verified before adding metadata-only diagnostics; original source and assertions remained unchanged around that insertion.
Exactly one ignored integration test ran and passed both sequential actual-provider cases using the Linux container and observed `claude-sonnet-5-5`.
Protected request returned typed blocked, left protected content unchanged, and passed blocked-feedback/no-publication mock assertions.
Allowed request ran an exact printf command, returned typed completed, produced the exact expected non-executable file, and passed blob/tree/commit/ref/PR mock payload assertions.
Both hosted inventory guards reported pages=1 and active=0; independent complete inventory found no native run in the probe window and no active native run afterward.
Temporary provider-secret removal exited 0, independently verified absent afterward.
Private response content and file contents are not included here.
This proves the tested protocol/container/provider combination with mock publication, not real GitHub publication, deployed refusal classification, or full Slice 2 acceptance.
Earlier prose-directed controls failed exact-byte comparison; their actual differing bytes remain undisclosed and unknown.

## Released v4 artifact verification

[Release workflow 42016735005](https://github.com/isp-insoft-gmbh/isp-golem-xiv/actions/runs/42016735005) completed successfully for annotated v4 tag `9181164f3245f8f9e356e8cc9ab78e4513de255c`, commit `d8af87cffa10081e3f120c60ef49e32dce6372f8`.
All seven workflow-declared assets were downloaded into OS temp space and matched GitHub-reported SHA-256 digests and sizes.
The three aggregate checksum entries and standalone native checksum sidecar matched; native tar member equaled standalone binary, and both license assets matched committed source bytes.
Native asset ID `713144087`, SHA-256 `a8ae74bcbe33f0b3b95341c9231a0b8d88d915d685bbcbd77d0e7183997a7b92`; release ID `435105666`.
Verification scratch was removed.
Separately, trunk CI `42015764396` and deployment `42015764736` succeeded, and live health/source digest verified commit `d8af87c` with unchanged reviewed Coordinator digest.
GitHub reports immutable=false for the release container; this is trusted metadata/checksum verification, not independent build attestation or an immutable-container guarantee.
Real GitHub native-v4 publication was subsequently verified below; full Slice 2 acceptance remains incomplete.

## Native v4 real GitHub protected/allowed controls

Private runtime PR #8 merged as `5a4e973a3eb51c5455dc8393e6d59d5f659e51db` after exact-head CI `42019253787` on `a87b1e96c0253d22d052fb7eaafc65084cf455c5`.
Its released-v4 workflow bytes matched the reviewed template, SHA-256 `ccfe3bf8efe095418b3b0f03d0ce465c6a50344941d64d88916cc955dc7e3035`.
The continuation exited 0 with untruncated verified records for all three cases, using real native Linux/provider execution and real GitHub publication rather than a publication mock.
Protected/allowed sandbox: `isp-insoft-gmbh/isp-golem-xiv-test-controls-mw3gyop7-445293cc`.

- Protected: comment `6656976776`, check `127220936407`, delivery `0de41650-d90d-11f1-81aa-c861ecb10a06`, run `42020289697`; request tag `ef7781a4b8ba132d05862e85854e7f0b403e1889`.
- Allowed: comment `6657267593`, check `127227927227`, delivery `3b17ac20-d90f-11f1-8533-379d53454bf6`, run `42022373569`; request tag `000c15952a59662d43aaab6ec845434c73cad808`.
- Retained sneaky: unchanged source in `isp-insoft-gmbh/isp-golem-xiv-test-forbidden-mw28ms64-8bae596d`, comment `6657302443`, check `127228744030`, delivery `7df8cbf0-d90f-11f1-9f95-bb52fa7b23c6`, run `42022613897`; request tag `d211b81c234972f6f9b6f40fc23b3c0f98ed8b0c`.

Both protected cases had validated typed refusal, observed model, preserved protected/default content, no task branch or PR, App feedback, five ordered lifecycle phases and exact terminal request binding.
Allowed publication had exactly one added expected file, byte-exact 16-byte oracle, non-executable mode 100644, expected parent, protected content unchanged, App-authored PR #2 and matching feedback; commit `fe47605bcd87421f16a7da788874de2b2ce992ca`.
Allowed lifecycle phases, model/seat attribution and terminal request binding also passed.
All three actual request branch names matched the branches audited; their completed native job windows did not overlap.
Initial protected observer incorrectly expected an optional reason in check summary; read-only observation of its original request instead verified the exact native static refusal and App feedback, without redispatch or production changes.
Separately, public template PR #13 trunk CI `42020372496` and deployment `42020372451` succeeded; live health verified `976a4fd900336006a1293b2818c7aa2ebe8c056b` with unchanged reviewed Coordinator digest.
This proves these three cases, not full Slice 2 acceptance.

## Subsequent unchanged Slice 2 acceptance failure

Full `mise run accept -- --slice 2 --keep` exited 1 with 3455 untruncated output bytes after all 18 preflight checks passed.
All Slice 1 checks, forbidden-path, serial, stop and event-trigger passed.
Change-pr had no PR within its unchanged 20m bound; continuation was consequently unexecuted, and echo had no matching reply within unchanged 15m.
Retained sandboxes: `isp-insoft-gmbh/isp-golem-xiv-test-core-20261105130008`, `isp-insoft-gmbh/isp-golem-xiv-test-events-20261105130008`.
Changer check `127234241422`, delivery `30bed440-d911-11f1-89ac-e59b2587d3ce`, comment `6657535091`, run `42024257200`, request tag `16c04bc8a2fa379628b2c8b493336cfa27446288`: validated typed blocked with observed model and all five lifecycle phases.
Echo check `127249278530`, delivery `719a6dce-3899-05b1-dfc4-444ba7b15f3d`, comment `6658160790`, run `42028717923`, request tag `7b02e533b088c1a27366f5df72d476cb836946bc`: same typed-blocked/lifecycle evidence.
Read-only inspection of those exact request tags confirmed changer retained its mention suffix, while echo omitted the triggering comment body despite correct comment/actor identities.
A queued echo check `127249296915` shares the delivery ID; duplicate check creation is established, duplicate execution is not.
No failed task was redispatched, reset or manually terminalized during diagnosis.

## Context/delegation candidate Linux and provider controls

Public PR #14 candidate `080713783a75e4218806f8e1671a0eb39ec0a1ad` passed full canonical Linux CI `42039276650`, job `127285477751`, including all required steps and all four complete Worker-suite stages.
The oversized-body assertion remained byte-identical to previous trunk; committed diff check passed.
Retained Windows Miniflare/Undici resets are not resolved by this Linux pass.
Private test-only PR #9, exact head `229f2ca3ac154ffa4a073bce45eab61e00707993`, provider workflow `42040986651`/job `127291413828`, completed successfully.
The job checked public source `0807137` and unchanged `runner/src/tests.rs` SHA-256 `b304179a8ae0487ea876213099b0b31c19973392b8c3a358abb28ef9a1bc4c75` before credential exposure.
Exactly one selected actual Linux/provider integration test passed: protected typed blocked, original deterministic allowed typed completed, unchanged changer-style commit request typed completed; observed model claude-sonnet-5-5.
Original exact file/count/bytes/mode and mock Git-object/PR publication assertions remained enforced; changer also required its exact nonce file and non-executable mode.
Both hosted native inventories reported one page and zero active; independent inventory found no native run in the probe window and none active afterwards.
Observer exited 0 with verifiedActualProviderCases=true and complete output, then secret removal exit0/temporarySecretAbsenceVerified=true; parent independently rechecked absence.
Private PR #9 closed unmerged; production runtime remained v4.
This proves actual-provider cases with mock publication, not real GitHub candidate publication, echo feedback or full Slice 2 acceptance.

## Follow-up Linux regressions and released v5

Final PR #14 head `d9b557c8b08a55f10435a757226eb38440156581` passed canonical Linux CI `42049159644`, job `127320248268`, including every required step and all four complete Worker-suite stages.
Compiled-Worker assertions verified full-context rejection before API/ref/journal/check side effects and normal retirement of older invalid unprepared work with original-source done receipt, freed index capacity, no check/admission/native outcome.
Native regressions verified deleted/other-author-edited destinations preserve authorized event paths without feedback for member/owner payer cases; unknown failures retain fail-closed blocked reporting and observed-model/completion guards.
Both review threads received these exact assertion/CI mappings and were resolved before protected merge, rather than resolved merely as outdated.
PR #14 merged as `49be24620721baed5c0632395209503b1f81ef0b`; trunk CI `42050838758` and deployment `42050838841` succeeded, and live health independently verified this version and unchanged Coordinator digest.
Matching v5 release workflow `42051948501` succeeded; annotated tag `ac0f538c3980ea33a73e967106b8adc3ad2e4c1a` resolves to that commit.
All seven declared assets matched downloaded sizes/GitHub SHA-256 digests, aggregate/native checksums, archived-vs-standalone native bytes and committed license bytes; verification scratch removed.
Native asset `713835375`, SHA-256 `a97b865e9a12a921841e7e71b686154229c363b60c2933187ebe590894dbca2f`; release ID `435369758`.
Release container remains mutable; no independent attestation or immutable-container guarantee.
Private runtime pin PR #10 merged as `b0ceede98f7495c86852baa88f4868eb40262aae` after exact-head CI `42054652720` on `a3efde5a108ff57e18027d1ea68fa2c377c74193`.
Live workflow bytes matched the reviewed v5 template, SHA-256 `21f209196050f761bedebb4e27fd0a81b285b0bc14030d19c01061ec43072cc4`.
At release verification, whole-v5 live behavior remained unproven; subsequent case evidence follows below.

## V5 retained changer success and blocked continuation

On original core repository/definitions, new changer comment `6662029945`, check `127344375288`, delivery `14a42220-d92e-11f1-90fe-16fb0fd4ae18`, run `42056029957` passed the real audit.
Actual App PR #9/commit `91719b5c56af5c433ea3d682097033b64797081e` had only expected golem.txt changes, nonce, non-executable mode and original parent, protected/default content unchanged, attributed feedback, all five ordered phases and exact terminal request binding.
Original failed requests were not overwritten or redispatched.
New continuation comment `6662081241`, check `127345695362`, delivery `6271bc60-d92e-11f1-945a-0ee7d18bcfc8`, request tag `a06bf59d74ba8bb84500d754fc20363a8379832d` remains queued/unbound.
A matching-title workflow `42056406734`/job `127345869143` failed artifact download HTTP504 before validation/preparation/provider access; checksum and native steps never ran.
That metadata does not prove the exact request inputs for that workflow.
Complete job log and downloaded full run-log archive had no request envelope or original delivery/check identities, no artifacts were retained, and inspected REST/GraphQL metadata supplied no inputs.
Diagnostic archive scratch removed; no missing envelope was treated as equal, no run association or manual terminalization performed.
Observer activity was stopped/exit1 with identities preserved; echo was not submitted, so neither continuation nor echo nor whole suite passed.
Current authoritative continuation row retains its resource and seat with null run/attempt; recovery requires additional authoritative evidence or a separately proved cancellation protocol, not a blind rerun.

## Stop fence live recovery and resumed continuation/echo

Stop-fence coordinator PR #16 merged as `1b72c822c6e00c23fe5cde6770e5a1b969f67e45` after exact-head Linux CI `42937471264`; trunk CI `42938150932` and deploy `42938150832` succeeded and live health verified that version with coordinator digest `999dc9687f55265d793dc054db2bf5814f39bbcf6b04a05de467f8fd18c7dad0`.
Stop mention `6784348560` on core issue 3 fenced continuation delivery `6271bc60-d92e-11f1-945a-0ee7d18bcfc8` at 2026-11-11T01:33:12Z with null run/attempt.
After the 15-minute grace with no active same-title runner run, the row became terminal/stopped with null run/attempt, and check `127345695362` completed cancelled with `reason: stopped before runner admission; no agent execution was granted`.
The next enqueue retired the row: `golem-terminal/6271bc60…` points to the original request tag `a06bf59d74ba8bb84500d754fc20363a8379832d` and the row left `queue.json`.
No workflow run was associated, cancelled, or redispatched.

Router versions differ across this proof: the first changer case (run `42056029957`) ran on router `15213da`, while the continuation and echo below ran on post-fence router `1b72c82`; the private runtime was `b0ceede` (native v5, no download retries) for all three.

- Continuation: comment `6784687086`, check `130266801688`, delivery `de7efaf0-dd72-11f1-9219-ed3f0d9511ce`, run `42942341694`; same App PR #9 advanced to commit `fff1ac3e54c92c6715b726f968ee44791d9672a0` with parent `91719b5…`, golem.txt alone modified, mode 100644, original bytes preserved with both nonces, feedback, five ordered phases and terminal binding passed.
- Echo: non-mention comment `6784714603` on events issue 1, check `130267437627` replied/success, delivery `913f1d37-ac72-6365-199e-87d6a211f24c`, run `42942532889`; audit (model, seat, phases, terminal row run/attempt) passed and the signed request instructions equal the exact triggering body under the event prefix.
  After 207 seconds (≥ unchanged 3-minute loop watch) there was exactly one echo runner run and one App reply `6784735711`, marked and quoting the trigger, with no `golems/echo` branch or PR.
  The driver itself exited 1 on its own wrong assumption that event requests carry a null branch (router assigns `golems/echo`); remaining checks were completed read-only without a new comment.

This proves the previously failing changer, continuation and echo behaviours live on v5; unchanged full Slice 2 acceptance has not yet been rerun.
