# Status

Current state for whoever continues; history is in git, [DECISIONS.md](DECISIONS.md) and [EVIDENCE.md](EVIDENCE.md).

## Where things stand (2026-11-24)

- **All slices accepted.** Slice 2 32/32, Slice 3 44/44, Slice 4 47/47 on deployed `87d2024` with runner v20 ([EVIDENCE.md](EVIDENCE.md), 2026-11-23).
  The operator guide ([OPERATIONS.md](OPERATIONS.md)) was brought up to date afterwards.
- **Live:** router deployed from `trunk` (check `/health`); runner v24 (SHA-256 `52cb0ed74bd39a1003621256278f999bd288b8a07dcb1ac1e9dcfc57dcc09116`, Mise 2026.11.14) pinned in ops `trunk` `0caa920e`; agent image cached under its source digest and checked daily by the ops `agent-image` workflow.
- **Runners:** all CI and golem runs use Blacksmith runners (Ubuntu 24.04 labels); releases keep upstream `actions/checkout`, since Blacksmith's checkout replaces annotated tags (v22 was burned by it).
- **Queue and event records** are rooted in blobs, so runner pins no longer strand rows; every commit-rooted row admitted earlier has retired.
- **Toktrak** runs Markdown-only golems on its OpenAI Codex and Anthropic seats; its bug, performance and security golems opened pull requests #164, #165 and #175, all merged.
  On v24 a Claude Code canary on the Anthropic seat passed its full lifecycle (EVIDENCE.md, 2026-11-24); the documentation and refactor golems have not yet run on a current runner.
  Renovate raises Toktrak's Mise minimum almost daily; until golems follow the target's Mise (decided, not built), each bump past 2026.11.14 breaks its golems again.

## In progress: development goals and documentation

Oliver re-evaluated the goals in [DEVELOPMENT.md](DEVELOPMENT.md) (DECISIONS.md, 2026-11-24); each open goal lands as its own pull request:

1. Done: README rewritten, golem examples in the documentation parsed by the product's parser, stale goals corrected.
2. Done: the introductory Confluence page [isp-golem-xiv: Golems für unsere GitHub-Repositories](https://isp-insoft.atlassian.net/wiki/spaces/RD/pages/2279178242) in R&D › Agentic Coding, in German.
3. Done: line coverage ratchet per crate with `cargo-llvm-cov` and `coverage.toml`; two Blacksmith `ci` runs of the same commit measured exactly the baseline.
4. `insta` snapshots for replies, check summaries and the dashboard; `proptest` for parsers and seat names.
5. Payload parsers tested with real webhook deliveries captured from the App, scrubbed.
6. The mdBook site on GitHub Pages: user documentation, release notes, CI reports.

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
