#!/usr/bin/env nu
# Black-box acceptance test for isp-golem-xiv.
# It talks only to GitHub and the router's public API, and checks every promise in contract.nuon.
# Fix bugs here, never weaken a promise; PLAN.md, section "Acceptance contract", has the rules.
# Every failure prints the promise, what was observed, and what to do next.

# Check every promise up to --slice against the deployed system.
def main [
  --slice: int = 1  # highest slice whose promises are checked
  --preflight       # check only the static setup, create nothing
  --keep            # keep the test repository for inspection
] {
  let c = open ($env.FILE_PWD | path join contract.nuon)
  let wanted = $c.promises | where slice <= $slice | get id

  let pre = preflight $c | where id in $wanted | needs code-repo [code-ci] | needs ops-repo [runner-workflow tester-seat]
  if ($pre | any {|r| not $r.ok }) or $preflight {
    report $c $pre
    return
  }

  let repo = $"($c.org)/($c.test_repos.prefix)accept-(date now | format date '%Y%m%d%H%M%S')"
  let live = try {
    setup $c $repo
    [(router-index $c $repo)] ++ (mention-hello $c $repo) ++ [(refused-unknown $c $repo)]
  } catch {|e|
    [(result setup false $"setting up ($repo) failed: ($e.msg)")]
  }
  if $keep { print $"kept https://github.com/($repo)" } else { ^gh repo delete $repo --yes | ignore }
  report $c ($pre ++ ($live | where {|r| $r.id == setup or $r.id in $wanted }))
}

# Static checks: no side effects, seconds to run.
def preflight [c: record] {
  [
    (check tester-scopes {||
      let scopes = ^gh api -i user | lines | where {|l| $l =~ '(?i)^x-oauth-scopes:' } | get -o 0 | default "" | str replace -r '(?i)^x-oauth-scopes:\s*' '' | split row ',' | str trim
      let missing = $c.tester.gh_scopes | where {|s| $s not-in $scopes }
      if ($missing | is-not-empty) { $"missing scopes: ($missing | str join ', ')" }
    })
    (check app-events {||
      let events = ^gh api $"/apps/($c.app.slug)" | from json | get events
      let missing = $c.app.events | where {|e| $e not-in $events }
      if ($missing | is-not-empty) { $"App does not subscribe to: ($missing | str join ', ')" }
    })
    (check app-permissions {||
      let app = ^gh api $"/apps/($c.app.slug)" | from json | get permissions
      let inst = ^gh api $"/orgs/($c.org)/installations" | from json | get installations | where id == $c.app.installation | get -o 0.permissions | default {}
      let short = [[where granted]; [App $app] [installation $inst]] | each {|g|
        $c.app.permissions | transpose name level | where {|p| (rank ($g.granted | get -o $p.name)) < (rank $p.level) } | each {|p| $"($g.where) lacks ($p.name): ($p.level)" }
      } | flatten
      if ($short | is-not-empty) { $short | str join '; ' }
    })
    (check code-repo {|| repo-state $c.code.repo false $c.code.default_branch })
    (check code-ci {||
      let runs = ^gh run list --repo $c.code.repo --branch $c.code.default_branch --event push --limit 20 --json workflowName,conclusion,status,url | complete
      if $runs.exit_code != 0 { return "cannot list runs" }
      let latest = $runs.stdout | from json | group-by workflowName | values | each { first }
      if ($latest | is-empty) { return "no CI runs on the default branch" }
      let bad = $latest | where {|r| $r.status == completed and $r.conclusion != success }
      if ($bad | is-not-empty) { $bad | each {|r| $"($r.workflowName): ($r.conclusion) ($r.url)" } | str join '; ' }
    })
    (check ops-repo {|| repo-state $c.ops.repo true $c.ops.default_branch })
    (check runner-workflow {||
      let r = ^gh api $"repos/($c.ops.repo)/actions/workflows" | complete
      if $r.exit_code != 0 { return "cannot list workflows" }
      let wf = $r.stdout | from json | get workflows | where path == $c.ops.runner_workflow | get -o 0
      if $wf == null { return $"no workflow at ($c.ops.runner_workflow)" }
      if $wf.state != active { $"workflow state is ($wf.state)" }
    })
    (check tester-seat {||
      let login = ^gh api user --jq .login | str trim
      let name = seat-secret $c $login $c.tester.provider
      let r = ^gh api $"repos/($c.ops.repo)/actions/secrets?per_page=100" | complete
      if $r.exit_code != 0 { return "cannot list ops secrets" }
      if $name not-in ($r.stdout | from json | get secrets.name) { $"secret ($name) is missing" }
    })
    (check router-deployed {||
      if $c.router.url == null { return "router.url is null in contract.nuon" }
      let r = try { http get --full --allow-errors ($c.router.url + $c.router.health) } catch {|e| return $"unreachable: ($e.msg)" }
      if $r.status != 200 { return $"health answered ($r.status)" }
      if ($r.body | get -o app) != $c.app.slug { $"health body lacks app = ($c.app.slug)" }
    })
  ]
}

# A fresh private repository, allowlisted by topic, installed, with one manual golem.
def setup [c: record, repo: string] {
  ^gh repo create $repo --private --add-readme | ignore
  ^gh repo edit $repo --add-topic $c.test_repos.topic | ignore
  let id = ^gh api $"repos/($repo)" --jq .id | str trim
  ^gh api -X PUT $"/user/installations/($c.app.installation)/repositories/($id)" | ignore
  put-file $repo ".golems/hello.md" ([
    "---"
    "---"
    ""
    "Reply to the comment that started you."
    "If it contains `say <text>`, end your reply with <text> exactly as written."
    "Change nothing in the repository."
    ""
  ] | str join "\n")
}

def router-index [c: record, repo: string] {
  check router-index {||
    let token = ^gh auth token | str trim
    let url = $c.router.url + ($c.router.golems | str replace '{repo}' $repo)
    let index = wait-for $c.limits.indexed {||
      let r = http get --full --allow-errors --headers [Authorization $"Bearer ($token)"] $url
      if $r.status == 200 and ($r.body.golems? | default [] | any {|g| $g.name == hello }) { $r.body }
    }
    if $index == null { return $"($url) did not list golem hello within ($c.limits.indexed)" }
    let hello = $index.golems | where name == hello | first
    if ($hello.errors? | default [] | is-not-empty) { $"hello has errors: ($hello.errors | to nuon)" }
  }
}

# One mention proves seen, dispatched, reply, audit, and no-loop in order.
def mention-hello [c: record, repo: string] {
  let login = ^gh api user --jq .login | str trim
  let nonce = random chars --length 12
  let issue = new-issue $repo "hello"
  # The echoed text carries a mention, so a missing loop guard would start a second run.
  let comment = new-comment $repo $issue $"@($c.app.slug) hello say ($nonce) @($c.app.slug) hello"
  let marker = $c.marker.reply | str replace '{golem}' hello
  let run_name = $c.check_run.name | str replace '{golem}' hello

  let seen = check seen {||
    if (wait-for $c.limits.seen {|| if "eyes" in (reactions $repo $comment) { true } }) == null { $"no eyes reaction within ($c.limits.seen)" }
  }
  if not $seen.ok { return [$seen ...(blocked-by seen [dispatched reply audit no-loop])] }

  let reply = wait-for $c.limits.reply {|| bot-comments $c $repo $issue | where {|m| $m.body has $nonce } | get -o 0 }
  let dispatched = check dispatched {|| if not ("rocket" in (reactions $repo $comment)) { "no rocket reaction on the mention" } }
  let replied = check reply {||
    if $reply == null { return $"no reply containing the nonce within ($c.limits.reply)" }
    if not ($reply.body | str trim | str starts-with $marker) { $"reply does not start with ($marker)" }
  }
  let audit = check audit {||
    let runs = check-runs $repo $run_name
    if ($runs | length) != 1 { return $"expected 1 check run ($run_name), found ($runs | length)" }
    let summary = $runs.0.output.summary? | default ""
    let expected = $c.check_run.summary | each {|l| $l | str replace '{outcome}' replied | str replace '{login}' $login | str replace '{provider}' $c.tester.provider }
    let missing = $expected | where {|l| not ($summary has $l) }
    if ($missing | is-not-empty) { $"summary lacks: ($missing | str join ', ')" }
  }
  if not $replied.ok { return [$seen $dispatched $replied $audit ...(blocked-by reply [no-loop])] }

  let loop = check no-loop {||
    sleep $c.limits.loop_watch
    let runs = check-runs $repo $run_name | length
    let replies = bot-comments $c $repo $issue | length
    if $runs != 1 or $replies != 1 { $"after ($c.limits.loop_watch): ($runs) runs, ($replies) replies" }
  }
  [$seen $dispatched $replied $audit $loop]
}

def refused-unknown [c: record, repo: string] {
  check refused-unknown {||
    let issue = new-issue $repo "unknown"
    new-comment $repo $issue $"@($c.app.slug) nosuchgolem" | ignore
    let marker = $c.marker.refused | str replace '{reason}' unknown-golem
    let reply = wait-for $c.limits.refusal {|| bot-comments $c $repo $issue | get -o 0 }
    if $reply == null { return $"no reply within ($c.limits.refusal)" }
    if not ($reply.body | str trim | str starts-with $marker) { return $"reply does not start with ($marker)" }
    let runs = check-runs $repo ($c.check_run.name | str replace '{golem}' nosuchgolem) | length
    if $runs != 0 { $"a refused mention started ($runs) runs" }
  }
}

# Print a table, then for every failure the promise, the observation, and the next step.
def report [c: record, results: list] {
  print ($results | select id ok | table)
  let failed = $results | where not ok
  for r in $failed {
    let p = $c.promises | where id == $r.id | get -o 0 | default {text: "setup of the test repository", guide: "Read the observed error; it comes from gh or the GitHub API."}
    print ""
    print $"✗ ($r.id)"
    print $"  promise:  ($p.text)"
    print $"  observed: ($r.observed)"
    print $"  do:       (fill $c $p.guide)"
  }
  if ($failed | is-not-empty) { exit 1 }
}

# Run a probe that returns null when the promise holds, or a description of what was observed.
def check [id: string, probe: closure] {
  let observed = try { do $probe } catch {|e| $"probe failed: ($e.msg)" }
  result $id ($observed == null) ($observed | default "")
}

# Replace every <path.to.key> with its value from the contract.
def fill [c: record, text: string] {
  $text | parse -r '<(?<key>[a-z_]+(?:\.[a-z_]+)+)>' | get key | uniq | reduce --fold $text {|key, acc|
    let value = $c | get -o ($key | split row '.' | into cell-path)
    if $value == null { $acc } else { $acc | str replace --all $"<($key)>" ($value | into string) }
  }
}

def result [id: string, ok: bool, observed: string] { {id: $id, ok: $ok, observed: $observed} }

# Replace results that cannot hold while their prerequisite failed.
def needs [cause: string, ids: list<string>] {
  let results = $in
  if ($results | where id == $cause | all {|r| $r.ok }) { return $results }
  $results | each {|r| if $r.id in $ids { result $r.id false $"not checked because ($cause) failed; fix ($cause) first" } else { $r } }
}

def blocked-by [cause: string, ids: list<string>] {
  $ids | each {|id| result $id false $"not checked because ($cause) failed; fix ($cause) first" }
}

# null when the repository exists with the expected visibility and default branch.
def repo-state [repo: string, private: bool, branch: string] {
  let r = ^gh api $"repos/($repo)" | complete
  if $r.exit_code != 0 { return $"($repo) not found" }
  let info = $r.stdout | from json
  let problems = [
    (if $info.private != $private { if $private { "is public" } else { "is private" } })
    (if $info.default_branch != $branch { $"default branch is ($info.default_branch)" })
  ] | compact
  if ($problems | is-not-empty) { $problems | str join '; ' }
}

def rank [level?: string] { {read: 1, write: 2, admin: 3} | get -o ($level | default none) | default 0 }

def seat-secret [c: record, login: string, provider: string] {
  let norm = {|s| $s | str uppercase | str replace --all '-' '_' }
  $c.seat_secret | str replace '{login}' (do $norm $login) | str replace '{provider}' (do $norm $provider)
}

# Poll until the probe returns something other than null, or give up with null.
def wait-for [timeout: duration, probe: closure] {
  let deadline = (date now) + $timeout
  loop {
    let value = do $probe
    if $value != null { return $value }
    if (date now) > $deadline { return null }
    sleep 10sec
  }
}

def put-file [repo: string, path: string, content: string] {
  ^gh api -X PUT $"repos/($repo)/contents/($path)" -f $"message=add ($path)" -f $"content=($content | encode base64)" | ignore
}

def new-issue [repo: string, title: string] {
  ^gh api -X POST $"repos/($repo)/issues" -f $"title=($title)" --jq .number | into int
}

def new-comment [repo: string, issue: int, body: string] {
  ^gh api -X POST $"repos/($repo)/issues/($issue)/comments" -f $"body=($body)" --jq .id | into int
}

def reactions [repo: string, comment: int] {
  ^gh api $"repos/($repo)/issues/comments/($comment)/reactions" | from json | get content
}

def bot-comments [c: record, repo: string, issue: int] {
  ^gh api $"repos/($repo)/issues/($issue)/comments" | from json | where {|m| $m.user.login == $c.app.bot }
}

def check-runs [repo: string, name: string] {
  let branch = ^gh api $"repos/($repo)" --jq .default_branch | str trim
  ^gh api $"repos/($repo)/commits/($branch)/check-runs?check_name=($name)" | from json | get check_runs
}
