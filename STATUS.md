# Status

Current state for whoever continues; decisions are in [DECISIONS.md](DECISIONS.md), proofs in [EVIDENCE.md](EVIDENCE.md), history in git and [history/](history/).

## Where things stand (2026-11-24)

- **All four slices accepted**; the last full run passed 47 of 47 gates on 2026-11-23 ([EVIDENCE.md](EVIDENCE.md#acceptance)).
- **Live:** router deployed from `trunk` (check `/health`); runner v24 (SHA-256 `52cb0ed74bd39a1003621256278f999bd288b8a07dcb1ac1e9dcfc57dcc09116`, Mise 2026.11.14) pinned in ops `trunk` `0caa920e`; all CI and golem jobs run on Blacksmith runners.
- **Toktrak** runs Markdown-only golems on its OpenAI Codex and Anthropic seats; its bug, performance and security golems opened pull requests #164, #165 and #175, all merged.
  On v24 a Claude Code canary on the Anthropic seat passed its full lifecycle; the documentation and refactor golems have not yet run on a current runner.
- **Docs:** README, developer onboarding in [CONTRIBUTING.md](CONTRIBUTING.md), the introductory Confluence page [isp-golem-xiv: Golems für unsere GitHub-Repositories](https://isp-insoft.atlassian.net/wiki/spaces/RD/pages/2279178242) and its child [isp-golem-xiv: Einstieg für Entwickler](https://isp-insoft.atlassian.net/wiki/spaces/RD/pages/2277376047) are current; the line coverage ratchet holds router 98.4%, runner 88.9%, xtask 46.7%.

## Before 1.0

Oliver decided that 1.0 is the next `v<n>` release after these land, each as its own pull request:

1. Golems follow the target's Mise: the runner downloads and checks the Mise release a target's `min_version` requires. Until then every Renovate bump of Toktrak's Mise past 2026.11.14 breaks its golems.
2. Detect child memory kills from the kernel counter, with Docker inspection as fallback; v24 can miss the kill when the parent survives.
3. `insta` snapshots for replies, check summaries and the dashboard; `proptest` for parsers and seat names.
4. Payload parsers tested with real webhook deliveries captured from the App, scrubbed.
5. The mdBook site on GitHub Pages: user documentation, release notes, CI reports.

Then: release, integrity record, ops pin, `ops/` template, and a canary per harness.

## Open gaps

- No warning before a seat's sign-in expires: expiry and refresh state are not recorded anywhere the router can read.
- `golem-delivery` tags in ops grow without bound; retention is a later product decision.
- The golem frontmatter field `runner` is accepted but not honoured: every golem runs on the ops label `blacksmith-2vcpu-ubuntu-2404`.
- Organization-wide container caching on Blacksmith is a recorded open risk.
- The runner still accepts request version 1, which the router no longer sends.
