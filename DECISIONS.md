# Decisions

Why things are the way they are, newest first.
Each entry: date, decision, reason, and evidence where it exists.

## 2026-11-03: Slice 1 implementation

- Rust workspace separates host-tested router policy, wasm edge I/O, trusted native runner, stdlib-only xtask.
- Router reads the index fresh rather than retaining a cache; bounded to 20 golems of 64 KiB each.
- Atomic create-only GitHub refs in private ops claim delivery IDs, with payload fingerprints; no eventually consistent KV lock.
  This prevents duplicate dispatch attempts, not transactional delivery guarantees across GitHub APIs.
  Failed or ambiguous deliveries retain claims; absent rocket does not establish dispatch failure.
  Reconciliation requires checking delivery, check external ID, and ops runs before any operator action; workflow reruns are rejected before credential access.
- Runner snapshots the exact dispatch SHA, runs Claude in an unprivileged read-only container with only read tools, subscription token, target snapshot, bounded resources, independent timeout entrypoint.
  No GitHub token, App key, host home, Docker socket, or workflow control files reach the container.
  Confirmed container destruction precedes fresh scoped publication credentials.
  Output scanning is defense in depth, not a claim of arbitrary covert-channel prevention.
- Direct Workers REST multipart upload supports Rust SDK build artifacts without a second CLI build format; production deployment happens only in CI after validation.
- Dependencies: serde/serde_json for bounded protocol I/O; serde-saphyr for strict bounded YAML; hmac/sha2 for constant-time webhook signatures and fingerprints; worker/wasm-bindgen/futures-util/base64 for Workers runtime APIs, native crypto bridge, cancellation, contents decoding.
  Runner uses reqwest/tokio for bounded async HTTP/process I/O, jsonwebtoken for native App JWTs, flate2/tar for validated snapshots, tempfile for automatically cleaned workspaces, base64 for secret scanning.
- Global duplicate-dependency denial remains; exact-version exceptions in `deny.toml` cover incompatible upstream base64/getrandom/syn/windows-sys generations.
  CDLA-Permissive-2.0 allowed only for webpki-roots 1.0.11; its license ships in releases.
- Local `mise run ci` passed after multiline/rerun hardening and again after container-test wiring: 27 Rust tests, wasm Clippy/build, built Worker workerd tests, packaging, deploy transport tests, all linters, original judge parser checks.
  Docker is unavailable locally; Linux CI must build the actual pinned agent image and exercise its timeout entrypoint before integration is accepted.
- Scoped mutation evidence: router 3 selected mention mutants caught (~49s); xtask 12/12 selected mutants caught after boundary-test improvements (38.49s wall, 0.31s driver CPU, sampled peak scratch 17.6 MB).
  Compiler/test CPU and router peak disk were not captured; no complete resource-accounting claim.
- Independent review by gpt-5.6-terra found no demonstrated auth/secret exploit; live deployment and Docker execution remain verification gaps.
- Initial preflight red observations: ops repo absent, router health 404; workflow, seat, signature/auth checks blocked by these; evidence absent.
  Initial full gate also skipped live checks because static setup failed.
  These skipped checks are not claimed as live red observations; retain actual later acceptance failures before marking their behavior proven.
- Judge changes: none (`accept.nu`, `contract.nuon`, `tools/ci.nu` unchanged).

## 2026-11-03: Hardening before handoff

Oliver's decisions after four independent reviews of the judge, the spec and plan, the development workflow, and the agent's autonomy.

- **Promises for all slices are written before handoff.** Otherwise `--slice 3` passed on Slice 1 checks alone, and the agent would have written promises it then checks itself.
- **The App installation covers all repositories.** Adding repositories to a selected installation needs a token gh does not have; the topic allowlist already limits what the router acts on.
- **The admin dashboard is Slice 4 with its own promises.** It stays a goal but no longer competes with the core.
- **Anthropic seats may run under `pi`.** People remain responsible for their providers' terms.
- **Router worker name and URL are fixed in `contract.nuon`.** The App's OAuth callback URL is set by hand and must not change when the agent deploys. The account's `workers.dev` subdomain `isp-insoft-gmbh` was created for this through the Cloudflare API; it can be renamed in the Cloudflare dashboard.
- **Promise kinds prerequisite, check, evidence.** Prerequisites are exempt from red before green; behavior `accept.nu` cannot drive is proven in `EVIDENCE.md`.
- **End-to-end scenarios stay out of public CI.** They need seats and private test repositories.
- **Rust is pinned by `rust-toolchain.toml`, not mise.** mise's `RUSTUP_TOOLCHAIN` would override the pin; Clippy's `cargo` group is dropped because its findings are mostly in dependencies.
- **`stopped` is an outcome**, and `no_change` belongs to runs without a person to answer.
