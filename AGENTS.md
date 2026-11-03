# Agents

Read [STATUS.md](STATUS.md) first, then [SPEC.md](SPEC.md), [PLAN.md](PLAN.md), [DEVELOPMENT.md](DEVELOPMENT.md), and [contract.nuon](contract.nuon) before changing anything.
Then work through the plan's slices without pausing for approval, as the plan describes.

- `mise run accept -- --preflight --slice <current slice>` says what is missing next; its output is your instruction.
- Update `STATUS.md` at every milestone, so a fresh session can continue where you stopped.
- Commit as `git -c user.name="isp-golem-xiv agent" commit …`, so your changes to the judge stay distinguishable from Oliver's.
- Changes reach `trunk` only through pull requests with a green `ci` check; merge your own once green.
- Secrets are in `~/.secrets/isp-golem-xiv/`, described in its `README.md`; copy them into GitHub or Cloudflare secrets, never print or commit them.
- Record decisions and gaps in [DECISIONS.md](DECISIONS.md), proofs of evidence promises in [EVIDENCE.md](EVIDENCE.md).
