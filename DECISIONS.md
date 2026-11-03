# Decisions

Why things are the way they are, newest first.
Each entry: date, decision, reason, and evidence where it exists.

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
