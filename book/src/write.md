# Write a golem

Put frontmatter at the top of `.golems/<name>.md` and write the task below it in plain prose.
State what to inspect, what to change, and what to report if the task is blocked.
Keep instructions shared by all golems in `.golems/_golems.md`, but leave general repository instructions where the harness already finds them.

For example, `.golems/export-notes.md` answers a question without proposing a change:

```markdown
---
harness: pi
timeout: 10m
---
Find where the CSV export chooses its column order and explain how it works.
Point to the relevant code and tests.
Do not change files.
```

## Choose when it runs

Without `triggers` or `schedule`, the golem waits for a person to start it.
Set `triggers` to a list of GitHub events or event actions, such as `issues.opened`.
Set `schedule` to `daily` or a list of weekdays, such as `[mon, thu]`.
Schedules use UTC, the router picks the hour, and missed runs are not made up.
Either field requires an `owner` who agrees to [pay for unattended runs](seats.md#who-pays).
Events caused by the App itself do not trigger golems.

## Choose how it runs

| Field | Meaning |
| --- | --- |
| `harness`, `provider`, `model`, `effort` | Preferences; [seat selection](seats.md#which-provider-runs) can change the harness or model. |
| `timeout` | Run budget; defaults to `30m`, with a maximum of `5h`. |
| `branch` | Branch a golem owns and resumes; defaults to `golems/<name>`. |
| `verify` | Command that must pass before changes are published; set it for any golem that should open a pull request. |
| `runner` | Accepted but does not select a runner. |

Choose a verification command that checks the result without changing the workspace.
Golems cannot publish changes to their own definitions, workflows, or harness configuration.
Do not name a golem `stop`, `seat`, or `help`: those names are reserved.

See the [interface specification](https://github.com/isp-insoft-gmbh/isp-golem-xiv/blob/trunk/SPEC.md#the-golems-interface) for the full field list.
