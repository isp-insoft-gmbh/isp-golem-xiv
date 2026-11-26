# Status

Current state for whoever continues; decisions are in [DECISIONS.md](DECISIONS.md), proofs in [EVIDENCE.md](EVIDENCE.md), history in git and [history/](history/).

## Where things stand (2026-11-26)

- **All four slices accepted**; the last full run passed 47 of 47 gates on 2026-11-23 ([EVIDENCE.md](EVIDENCE.md#acceptance)).
- **Live:** router deployed from `trunk` (check `/health`); runner v26 (SHA-256 `4c48bfb9f15a34a28e4cfcd254eb4ccabac769a176ca6f7372de86a1bd0e2987`, Mise 2026.11.14) pinned in ops `trunk` `43d5c51`; all CI and golem jobs run on Blacksmith runners.
- **Toktrak** runs Markdown-only golems on its OpenAI Codex and Anthropic seats; its bug, performance and security golems opened pull requests #164, #165 and #175, all merged.
  On v24 a Claude Code canary on the Anthropic seat passed its full lifecycle; the documentation and refactor golems have not yet run on a current runner.
- **Docs:** README, developer onboarding in [CONTRIBUTING.md](CONTRIBUTING.md), the introductory Confluence page [isp-golem-xiv: Golems für unsere GitHub-Repositories](https://isp-insoft.atlassian.net/wiki/spaces/RD/pages/2279178242) and its child [isp-golem-xiv: Einstieg für Entwickler](https://isp-insoft.atlassian.net/wiki/spaces/RD/pages/2277376047) are current; the current line coverage ratchet holds router 98.4%, runner 88.9%, xtask 46.6%.

## Before 1.0

Oliver decided that 1.0 is the next `v<n>` release after these land, each as its own pull request:

1. Golems follow the target's Mise: merged in #95 and released in v26.
2. Done in v25: detect child memory kills from the kernel counter, with Docker inspection as fallback.
3. `insta` snapshots and `proptest` parser and seat-name checks: merged in #96.
4. Parser tests for four scrubbed App deliveries: merged in #97 ([EVIDENCE.md](EVIDENCE.md#2026-11-26-captured-app-webhook-parser-fixtures)).
5. The [public handbook](https://isp-insoft-gmbh.github.io/isp-golem-xiv/) serves the guide, release notes, and CI report.

v26 is the 1.0 milestone release; Codex/OpenAI and Pi/OpenAI passed live fixture canaries in approved writable disposable workspaces.
Claude Code/Anthropic remains inconclusive: its check was queued with no matching run or reply observed before verified test-repository cleanup ([EVIDENCE.md](EVIDENCE.md#2026-11-26-v26-live-harness-canaries)).

## Open gaps

- Local fix on `fix/recover-unbound-seat` binds future v2 runs before target preflight; not released or deployed.
  The older unbound Anthropic dispatch and the queued v26 Claude request are unchanged; historical repair needs a separate decision ([EVIDENCE.md](EVIDENCE.md#2026-11-26-unbound-anthropic-dispatch)).
- No warning before a seat's sign-in expires: expiry and refresh state are not recorded anywhere the router can read.
- `golem-delivery` tags in ops grow without bound; retention is a later product decision.
- The golem frontmatter field `runner` is accepted but not honoured: every golem runs on the ops label `blacksmith-2vcpu-ubuntu-2404`.
- Organization-wide container caching on Blacksmith is a recorded open risk.
- The runner still accepts request version 1, which the router no longer sends.
