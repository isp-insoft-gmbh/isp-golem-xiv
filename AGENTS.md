# Agents

Read [STATUS.md](STATUS.md) first, then [SPEC.md](SPEC.md), [DEVELOPMENT.md](DEVELOPMENT.md), [OPERATIONS.md](OPERATIONS.md) and [contract.nuon](contract.nuon) before changing anything.
All four slices are accepted; work arrives as tasks from Oliver, and [DEVELOPMENT.md](DEVELOPMENT.md#working-rules) has the working rules.

- Policy goes to Oliver: limits, security and trust boundaries, promises in `contract.nuon`, test skips, advisory ignores, and every run that spends a seat outside `accept.nu`.
- Commit as `git -c user.name="isp-golem-xiv agent" commit …`, so your changes to the judge stay distinguishable from Oliver's.
- Changes reach `trunk` only through pull requests with a green `ci` check on the exact head; read every review and thread, then merge your own with `--match-head-commit`.
- Merged is not deployed, and deployed is not working: check trunk CI, the deploy and `/health`, and prove behavior with run logs before saying it works.
- A runner change reaches golems only through the order in [OPERATIONS.md](OPERATIONS.md#release-and-verify): release, integrity record, ops pin, `ops/` template.
- After changing promised behavior, ask Oliver before `mise run accept -- --slice 4`, which spends about a dozen seat runs.
- Secrets are in `~/.secrets/isp-golem-xiv/`, described in its `README.md`; copy them into GitHub or Cloudflare secrets, never print or commit them.
- Record decisions in [DECISIONS.md](DECISIONS.md), proofs in [EVIDENCE.md](EVIDENCE.md), and the current state in [STATUS.md](STATUS.md) at every milestone.
