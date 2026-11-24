# Status

Current state for whoever continues; history is in git, [DECISIONS.md](DECISIONS.md) and [EVIDENCE.md](EVIDENCE.md).

## Where things stand (2026-11-24)

- **All slices accepted.** Slice 2 32/32, Slice 3 44/44, Slice 4 47/47 on deployed `87d2024` with runner v20 ([EVIDENCE.md](EVIDENCE.md), 2026-11-23).
  The operator guide ([OPERATIONS.md](OPERATIONS.md)) was brought up to date afterwards.
- **Live:** router deployed from `trunk` (check `/health`); runner v23 (SHA-256 `7f9ac78f7bae2b9446b826f339264ba973f56c92bc8fab5e59039c4255cb6ce1`, Mise 2026.11.12) pinned in ops `trunk` `e4ae87d`; agent image cached under its source digest and checked daily by the ops `agent-image` workflow.
- **Runners:** all CI and golem runs use Blacksmith runners (Ubuntu 24.04 labels); releases keep upstream `actions/checkout`, since Blacksmith's checkout replaces annotated tags (v22 was burned by it).
- **Queue and event records** are rooted in blobs, so runner pins no longer strand rows; every commit-rooted row admitted earlier has retired.
- **Toktrak** runs Markdown-only golems on the OpenAI Codex seat through Pi; after its Mise 2026.11.12 bump (#166), a canary on v23 passed target preparation and its full lifecycle (EVIDENCE.md, 2026-11-24).
  Its bug, performance and maintenance golems opened pull requests #164, #165 and #170, which await human review.

## Ordering constraints

- **A target's Mise bump blocks its golems** until the agent image asserts, and ops pins, that Mise: advance the image assertion, CI Mise and supported-minimum fixture here, release and verify a runner, pin URL, SHA-256 and Mise in one ops commit, then update `ops/`.

## Open gaps

- No warning before a seat's sign-in expires: expiry and refresh state are not recorded anywhere the router can read.
- One live Pi-on-Codex refresh write-back was observed (run 45088585900); the next run accepting the rewritten sign-in, and a failed write-back, are not yet observed.
- `golem-delivery` tags in ops grow without bound (about 1,750); retention is a later product decision.
- The golem frontmatter field `runner` is accepted but not honoured: every golem runs on the ops label `blacksmith-2vcpu-ubuntu-2404`.
- The ops `agent-image` workflow's comment still describes GitHub's seven-day cache eviction; Blacksmith's cache policy is not verified.
- Organization-wide container caching on Blacksmith is a recorded open risk; the agent image avoids its sticky disks and Docker builder (DECISIONS.md).

## Housekeeping done before release

- All 35 acceptance test repositories are deleted; GitHub keeps deleted organization repositories restorable for 90 days.
- Ops: 32 merged branches deleted, four orphaned diagnostic workflows disabled, old Mise caches removed; the five `probe/*` branches of closed, unmerged pull requests #4 to #9 remain; deleting them would be reversible from those pull requests.
- The App's self-onboarding pull requests here (#45) and in ops (#19) were closed: neither repository is a golem target.
