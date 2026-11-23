# isp-golem-xiv

Unattended agents, called golems, for the repositories of isp-insoft-gmbh.
A golem is a Markdown file in a repository's `.golems/` directory: its frontmatter says when it runs and with which harness, its body says what to do.

## How it works

Golems start from a mention such as `@isp-golem-xiv bugs`, a schedule, or a repository event.
Each run uses the subscription seat of the person who asked or owns the golem, inside an isolated container in the private ops repository.
A golem replies in the thread, or proposes changes as a pull request on its own branch after the repository's own verification command passed.
The `golem/<name>` check shows the outcome, seat, model and a link to the private log; `@isp-golem-xiv stop` cancels a run.

## Getting started

1. Register a seat at [Seats](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats) with a sign-in used only for golems.
2. Add `.golems/<name>.md` to a repository, as described in [SPEC.md](SPEC.md).
3. Mention `@isp-golem-xiv <name>` in an issue or pull request, followed by your instruction.

[OPERATIONS.md](OPERATIONS.md) covers seats, running and stopping golems, reading logs, the dashboard and troubleshooting.
[SPEC.md](SPEC.md) defines behavior, [PLAN.md](PLAN.md) the implementation sequence, [contract.nuon](contract.nuon) the acceptance promises, and [STATUS.md](STATUS.md) the current state.

Licensed under the [MIT License](LICENSE).
