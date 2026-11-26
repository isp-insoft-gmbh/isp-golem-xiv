# Run and troubleshoot

Start the first line of an issue or pull request comment with the command:

```text
@isp-golem-xiv export-notes
Which columns are included when I export an empty list?
```

A bare `@isp-golem-xiv` lists the golems available in the repository.
`@isp-golem-xiv seat` shows your providers and the registration link.
You can also tick **Run now** for a golem in the pinned **Golems** issue; the App clears the box after handling it.

## Follow a run

Eyes mean the App saw the mention; a rocket means it dispatched the run.
Runs on the same seat or for the same golem in the same repository wait rather than overlap.
The `golem/<name>` check shows who pays, which harness and model ran, the budget, and the outcome.
A golem can reply in the thread or publish verified changes on its own branch and pull request.

To request cancellation, comment `@isp-golem-xiv stop` in the run's thread.
The runner handles the stop after preparation; an acknowledgement does not mean the run has ended.

## When a run does not finish

| What you see | What to do |
| --- | --- |
| Unknown golem | Check spelling, the **Disabled** section, and `golems/validate`. |
| No seat | [Register a supported seat](seats.md), then make a new mention. |
| `blocked` | Read the `reason:` line in the check before trying again. |
| `incomplete` | Review any verified work already published; a later run continues the golem's branch. |

The check links to the private run log, which requires separate access.
Do not copy private logs, target code, or credentials into the public code repository.
The [organization dashboard](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/admin) shows recent runs to signed-in organization members; load more records before interpreting totals.
Its usage figures are observed runs and durations, not provider bills or billed Actions minutes.
