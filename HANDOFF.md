# Router / runner protocol

Workflow dispatch inputs: `repo`, `golem`, `request` (JSON).
Workflow run-name: `${{ inputs.repo }} ${{ inputs.golem }}`.
Workflow default branch: `trunk`.

`request` fields:

```json
{
  "version": 1,
  "delivery_id": "github-delivery-uuid",
  "repository_id": 123,
  "repository": "isp-insoft-gmbh/isp-golem-xiv-test-example",
  "head_sha": "40-hex-sha",
  "golem": "hello",
  "issue_number": 1,
  "comment_id": 123,
  "actor_id": 123,
  "actor_login": "bugabinga",
  "seat_login": "bugabinga",
  "provider": "anthropic",
  "harness": "claude",
  "model": null,
  "timeout_seconds": 1800,
  "instructions": "User instructions after golem name",
  "check_run_id": 123
}
```

Runner crate owns `DispatchV1` validation; router serializes matching JSON independently.
Runner CLI reads `GOLEM_REQUEST`, `GOLEM_REPO`, `GOLEM_NAME`, `GOLEM_APP_PRIVATE_KEY`, `GOLEM_SEAT`, `GITHUB_RUN_ID` from environment.
App ID and installation ID come from contract constants.
Workflow selects exactly one seat secret from request seat_login/provider after router validation; runner validates mapping again.
Router bindings: `APP_PRIVATE_KEY`, `WEBHOOK_SECRET`, `VERSION`.
Router atomically claims delivery by creating ops ref `refs/tags/golem-delivery/<delivery_id>` before side effects; verifies existing ref on conflict.
Claims retained; dispatch ambiguity never automatically retried.
Agent container receives subscription token only, never GitHub credentials, Docker socket, host home, or App key.
Runner publishes after container destruction with fresh target-scoped App token.
