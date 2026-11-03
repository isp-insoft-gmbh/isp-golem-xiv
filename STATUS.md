# Status

Where the work stands; the agent rewrites this file at every milestone.

- **Current slice:** 1, not started.
- **Next step:** run `mise run accept -- --preflight --slice 1` and follow its first failure.
- **Blocked on Oliver** at handoff time, each checked by the preflight:
  - `app-installation`: the App installation must cover all repositories.
  - `app-oauth`, needed from Slice 3: the client secret in `~/.secrets/isp-golem-xiv/app.client-secret` and the callback URL in the App settings.
  - Not checkable before the router exists: the App webhook must be ticked Active; `seen` fails if it is not.
- **Leftover test repositories:** none.
- **Last accept output:** preflight `--slice 1` on 2026-11-03: prerequisites green except `app-installation`; `ops-repo` and everything after it not checked yet; `router-deployed` answers 404.
