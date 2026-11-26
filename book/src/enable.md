# Enable a repository

An organization admin adds the repository to the GitHub App.
An installed repository enables golems by adding Markdown definitions under `.golems/` on its default branch.
You do not need a repository workflow, a local runner, or an activation flag.

## Start with a template

Sign in to the [organization dashboard](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/admin) and choose **Create starter PR** for a repository you can write to.
You must be an organization member, but creating the pull request does not use a seat.
Review and merge the pull request to add house rules and a `hello` golem that reports what it sees without changing files.
Asking again returns the existing pull request, even if it was closed.

The App opens and pins a **Golems** issue with the definitions and their latest results.
A pull request that changes `.golems/` receives the `golems/validate` check.
Invalid definitions appear under **Disabled** and do not run.

## Review what runs

Keep tool requirements in the target repository's Mise configuration.
Harnesses load repository instructions, hooks, and MCP servers themselves.
Review hooks and MCP servers before enabling golems: they run inside the agent container and can access its provider sign-in.
