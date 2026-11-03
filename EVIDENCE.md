# Evidence

Proofs for promises of kind `evidence` in [contract.nuon](contract.nuon), which `accept.nu` cannot drive itself.
One line per promise: `- <id>: <link to a run, comment, or screenshot, or path of a test> <what it shows>`.

- refused-no-seat: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613042205) `tools/prove-refusals.mjs --repo isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b` cloned an actual App delivery, signed a simulation for member `akonczalla` without a seat, observed the App's `no-seat` reply and registration link, with zero matching runner runs.
- refused-not-member: [deployed refusal](https://github.com/isp-insoft-gmbh/isp-golem-xiv-test-mw126nzr-2665e70b/issues/1#issuecomment-6613046125) The same harness signed the recorded delivery as verified non-member `octocat`, observed the App's `not-member` reply, with zero matching runner runs.

Slice 1 full deployed acceptance passed three consecutive times with released runner `v1`:

1. [run 41733197463](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41733197463)
2. [run 41733919165](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41733919165)
3. [run 41734601072](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/41734601072)

Each acceptance invocation independently checked repository-only sentinel content, requested exact text, completed attribution audit and absence of App loops.
Acceptance-created scenario repositories were cleaned by the unchanged judge.

Both signed simulations reached deployed Worker `8b13b0f1a33b10d9b26738002e6d697785a228d0`.
These prove router decisions using recorded deliveries, not actions performed by those account holders.
The private evidence repository is retained while these links are needed; the harness remains the reproducible test path if routine stale-repository cleanup later removes it.
