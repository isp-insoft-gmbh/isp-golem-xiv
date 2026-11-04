# Evidence

Proofs for promises of kind `evidence` in [contract.nuon](contract.nuon), which `accept.nu` cannot drive itself.
One line per promise: `- <id>: <link to a run, comment, or screenshot, or path of a test> <what it shows>`.

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
Real schedule-owner, bot-owner, incomplete/continuation proofs and full Slice 2 acceptance are still absent.

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
