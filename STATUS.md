# Status

- **Current slice:** 1, deployed; all preflight promises green, authenticated runner acceptance not yet run.
- **Branch:** `docs/slice-1-evidence` tracks milestones and forthcoming live proofs.
- **Next step:** review/test/publish the codeload PAX fix through a green PR; release next immutable tag `v1`, update ops pin, rerun full Slice 1 acceptance until three consecutive passes.
- **First full acceptance:** failed `reply` and `audit`; `no-loop` skipped because reply failed.
  Index, eyes, dispatch, unknown-golem refusal passed.
  Private ops run `41702041018` failed before agent execution: `snapshot contains a link or special file`.
  Root cause: actual GitHub codeload archives begin with a 52-byte PAX global commit-comment header; extractor rejected it.
  Local fix allows only that bounded metadata record and preserves hostile archive rejection; regression red→green, actual public code archive extraction green.
  Authenticated Claude execution remains unproven.
- **Trunk:** `ci` (`41697718686`) and deploy (`41697718711`) succeeded on `8b13b0f`.
- **Release:** immutable `v0` published successfully (`41700313107`); ops still pins `v0` until corrected runner release is published.
- **Last acceptance:** `mise run accept -- --preflight --slice 1` passed every static/evidence check.
- **Merged:** PR #4 through green required `ci` on exact head `b18fa24`; rebase merge preserves agent authors on trunk `8b13b0f`.
- **Blocked on Oliver:** nothing for Slice 1.
- **Implemented:** Rust Worker router, isolated Claude runner, private ops workflow, Rust xtask, pinned lint/build tools, CI-only deployment/release.
- **Verified locally:** fresh full `mise run ci` passed after mention/rerun hardening and again after container-test CI wiring (27 Rust tests, workerd suites, packaging, linters).
- **Container verification:** Docker unavailable locally; Linux run `41693321024` on `69a0b0c` passed `mise run ci` but FAILED `Agent container` during image build with suppressed diagnostics.
  Run `41695226429` on `14a8728` also failed image build; diagnostics locate unsupported help-text assumption for `--max-turns`.
  Exact pinned Linux binary parser probes passed offline in isolated WSL; Dockerfile now repeats positive/negative parser probes instead of help grep.
  Fresh local `mise run ci` passes on this correction; independent different-model review found no defect in parser probes or trusted build diagnostics.
  Linux CI `41696918035` on `b18fa24` PASSED, including actual pinned Bookworm image build, credential-free parser probes, version, timeout=0 rejection, forced timeout=1 kill (137), cleanup.
  This proves image/watchdog behavior, not authenticated Claude execution or live refusals.
- **Local transport gap:** subsequent full CI runs intermittently fail oversized-body workerd requests with ECONNRESET.
  Isolated 20-trial/condition experiment reproduced failure in trivial early-413 Worker, both dispatchFetch and direct Miniflare HTTP, proving simulator transport behavior independent of Rust router; no gate/assertion changed.
- **Runner fix:** empty LF/CRLF frontmatter failed a new regression case before correction and passed afterwards.
- **Live evidence:** signed, actual-delivery-derived member-without-seat and non-member simulations passed against deployed Worker; independent API readback confirmed markers, registration link, zero matching runner runs; URLs in `EVIDENCE.md`.
  Proof helper's delivery-detail 404 was caused by JS precision loss in int64 IDs; corrected and resumed the same test repo.
- **Independent review:** gpt-5.6-terra found no demonstrated auth/secret exploit; live/container proof still pending.
- **Provisioned:** private `isp-insoft-gmbh/isp-golem-xiv-ops` initialized on `trunk`; tester Anthropic seat and App key stored as encrypted secrets; code `production` environment has Cloudflare token/account, App key, matching generated webhook secret.
- **Webhook:** configured active production URL; local secret retained at `~/.secrets/isp-golem-xiv/webhook.secret`, never printed.
- **Ops synchronized:** remote `runner.yml` at `3381660` byte-compared with reviewed local source; rerun guard present.
- **Retained evidence repository:** `isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b` (private, marked); contains the two refusal proofs.
- **Temporary checkout:** OS temp `isp-golem-xiv-ops-bootstrap`, remove after ops synchronization.
- **Acceptance:** initial Slice 1 preflight prerequisites green; product static checks red, live checks skipped.
- **Known limitation:** delivery claims provide at-most-once dispatch attempts; ambiguous failures retain claims, require reconciliation, never blind replay.
- **Planned Slice 3 stops:** GitHub browser session and fresh dedicated Codex sign-in.
