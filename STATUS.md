# Status

Current state for whoever continues; history is in git, [DECISIONS.md](DECISIONS.md) and [EVIDENCE.md](EVIDENCE.md).

## Where things stand (2026-11-23)

- **All slices accepted.** Slice 2 32/32, Slice 3 44/44, Slice 4 47/47 on deployed `87d2024` with runner v20 ([EVIDENCE.md](EVIDENCE.md), 2026-11-23).
  The operator guide ([OPERATIONS.md](OPERATIONS.md)) was brought up to date afterwards.
- **Live:** router deployed from `trunk` (check `/health`); runner v21 (SHA-256 `4a6110d9926d6a500e7c6534a927fc9c73bc034b1676fb9135b043e47b7ce58e`) pinned in ops `trunk` `67a9d60`; agent image cached under its source digest and checked daily by the ops `agent-image` workflow.
- **Toktrak** runs Markdown-only golems on the OpenAI Codex seat through Pi; its bug, performance and maintenance golems opened pull requests #164, #165 and #170, which await human review.

## Blocked

- **Actions budget exhausted (2026-11-23 19:36Z).** Ops CI run 45250684632 was refused with "The job was not started because an Actions budget is preventing further use."
  Every golem runs in the private ops repository, so no golem can start until an organization owner raises the Actions budget; public-repository CI is unaffected.
  Ops pull request #39 (removing the diagnostic probes) waits for green CI on its head.

## Ordering constraints

- **Toktrak #166 must wait.** It raises Toktrak's Mise `min_version` to 2026.11.12, while the agent image asserts, and ops pins, Mise 2026.11.10, which refuses that configuration before any install.
  First advance the image assertion, CI tool version and supported-minimum fixture here, release and verify a runner, pin it in ops and update `ops/`; only then merge #166 (see DECISIONS.md, 2026-11-19 Mise entries).
  Auto-merge is off and the pull request carries a comment saying so.

## Open gaps

- No warning before a seat's sign-in expires: expiry and refresh state are not recorded anywhere the router can read.
- One live Pi-on-Codex refresh write-back was observed (run 45088585900); the next run accepting the rewritten sign-in, and a failed write-back, are not yet observed.
- `golem-delivery` tags in ops grow without bound (about 1,750); retention is a later product decision.
- After the next golem request, the ops queue journal should be empty: one terminal row of a deleted acceptance repository awaits retirement, which happens on enqueue; terminal rows hold no seat.

## Housekeeping done before release

- All 35 acceptance test repositories are deleted; GitHub keeps deleted organization repositories restorable for 90 days.
- Ops: 32 merged branches deleted, four orphaned diagnostic workflows disabled, old Mise caches removed; the five `probe/*` branches of closed, unmerged pull requests #4 to #9 remain; deleting them would be reversible from those pull requests.
- The App's self-onboarding pull requests here (#45) and in ops (#19) were closed: neither repository is a golem target.
