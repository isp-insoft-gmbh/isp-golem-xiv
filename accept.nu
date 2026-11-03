#!/usr/bin/env nu
# Black-box acceptance test for isp-golem-xiv.
# It talks only to GitHub, Cloudflare's token check, and the router's public API, and checks every promise in contract.nuon.
# Fix bugs here, never weaken a promise; PLAN.md, section "Acceptance contract", has the rules.
# Every failure prints the promise, what was observed, and what to do next.

# Check every promise up to --slice against the deployed system.
def main [
  --slice: int = 1  # highest slice whose promises are checked
  --preflight       # check only the static setup and the evidence, create nothing
  --keep            # keep the test repositories for inspection
] {
  let c = open ($env.FILE_PWD | path join contract.nuon)
  let slices = $c.promises | get slice | uniq | sort
  if $slice not-in $slices { error make {msg: $"--slice must be one of ($slices | str join ', ')"} }
  let wanted = $c.promises | where slice <= $slice
  let ids = $wanted | get id

  let static = preflight $c | where id in $ids
    | needs code-repo [code-ci]
    | needs ops-repo [runner-workflow tester-seat]
    | needs app-installation [ops-repo runner-workflow tester-seat]
    | needs router-deployed [webhook-signature router-index-auth seat-page admin-auth]
  let proofs = evidence $c ($wanted | where kind == evidence)
  if $preflight {
    report $c ($static ++ $proofs)
    return
  }
  let live_ids = $wanted | where kind == check | get id | where {|id| $id not-in ($static | get id) }
  if ($static | any {|r| not $r.ok }) {
    report $c ($static ++ $proofs ++ ($live_ids | each {|id| result $id false "not checked because a static check failed; fix those first" }))
    return
  }

  sweep $c
  let stamp = date now | format date '%Y%m%d%H%M%S'
  let ctx = {c: $c, login: (gh-ok api user --jq .login | str trim), stamp: $stamp, keep: $keep}
  let core = phase $ctx core {|| core $ctx }
  let changes = if $slice >= 2 and $core.repo != null { phase $ctx changes {|| changes $ctx $core.repo } } else { {results: []} }
  let events = if $slice >= 2 { phase $ctx events {|| events $ctx } } else { {results: []} }
  let onboard = if $slice >= 3 { phase $ctx onboarding {|| onboarding $ctx } } else { {results: []} }
  let admin = if $slice >= 4 and $core.repo != null { phase $ctx admin {|| admin $ctx $core.repo } } else { {results: []} }
  let live = [$core $changes $events $onboard $admin] | get results | flatten | where {|r| $r.id starts-with "setup " or $r.id in $ids }
  cleanup $ctx

  let missing = $live_ids | where {|id| $id not-in ($live | get id) } | each {|id| result $id false "no check ran for this promise; accept.nu must check it" }
  report $c ($static ++ $proofs ++ $live ++ $missing)
}

# Static checks: no side effects, seconds to run.
def preflight [c: record] {
  [
    (check tester-scopes {||
      let scopes = gh-ok api -i user | lines | where {|l| $l =~ '(?i)^x-oauth-scopes:' } | get -o 0 | default "" | str replace -r '(?i)^x-oauth-scopes:\s*' '' | split row ',' | str trim
      let missing = $c.tester.gh_scopes | where {|s| $s not-in $scopes }
      if ($missing | is-not-empty) { return $"missing scopes: ($missing | str join ', ')" }
      let role = gh-json api $"user/memberships/orgs/($c.org)" | get role
      if $role != $c.tester.org_role { $"role in ($c.org) is ($role)" }
    })
    (check app-events {||
      let events = gh-json api $"/apps/($c.app.slug)" | get events
      let missing = $c.app.events | where {|e| $e not-in $events }
      if ($missing | is-not-empty) { $"App does not subscribe to: ($missing | str join ', ')" }
    })
    (check app-permissions {||
      let app = gh-json api $"/apps/($c.app.slug)" | get permissions
      let inst = installation $c
      if $inst == null { return $"installation ($c.app.installation) not found in ($c.org)" }
      let short = [[where granted]; [App $app] [installation $inst.permissions]] | each {|g|
        $c.app.permissions | transpose name level | where {|p| (rank ($g.granted | get -o $p.name)) < (rank $p.level) } | each {|p| $"($g.where) lacks ($p.name): ($p.level)" }
      } | flatten
      if ($short | is-not-empty) { $short | str join '; ' }
    })
    (check app-installation {||
      let inst = installation $c
      if $inst == null { return $"installation ($c.app.installation) not found in ($c.org)" }
      if $inst.suspended_at? != null { return "installation is suspended" }
      if $inst.repository_selection != $c.app.repository_selection { $"repository access is ($inst.repository_selection)" }
    })
    (check cloudflare-token {||
      let token = secret $c $c.secrets.cloudflare_token
      let account = secret $c $c.secrets.cloudflare_account
      let headers = [Authorization $"Bearer ($token)"]
      let verify = http get --full --allow-errors --headers $headers $"https://api.cloudflare.com/client/v4/accounts/($account)/tokens/verify"
      if $verify.status != 200 or ($verify.body.result?.status? != active) { return $"token verify answered ($verify.status)" }
      let sub = http get --full --allow-errors --headers $headers $"https://api.cloudflare.com/client/v4/accounts/($account)/workers/subdomain"
      if ($sub.body.result?.subdomain? | default "") != $c.router.workers_subdomain { $"workers.dev subdomain is not ($c.router.workers_subdomain)" }
    })
    (check code-repo {|| repo-state $c.code.repo false $c.code.default_branch })
    (check code-ci {||
      let runs = gh-json run list --repo $c.code.repo --branch $c.code.default_branch --event push --limit 50 --json workflowName,conclusion,status,url
      if ($runs | is-empty) { return "no CI runs on the default branch" }
      let running = $runs | where status != completed
      if ($running | is-not-empty) { return $"still running, retry later: ($running.url | str join ' ')" }
      let bad = $runs | group-by workflowName | values | each { first } | where conclusion != success
      if ($bad | is-not-empty) { $bad | each {|r| $"($r.workflowName): ($r.conclusion) ($r.url)" } | str join '; ' }
    })
    (check ops-repo {|| repo-state $c.ops.repo true $c.ops.default_branch })
    (check runner-workflow {||
      let wf = gh-json api $"repos/($c.ops.repo)/actions/workflows" | get workflows | where path == $c.ops.runner_workflow | get -o 0
      if $wf == null { return $"no workflow at ($c.ops.runner_workflow)" }
      if $wf.state != active { return $"workflow state is ($wf.state)" }
      let text = gh-ok api $"repos/($c.ops.repo)/contents/($c.ops.runner_workflow)" -H "Accept: application/vnd.github.raw"
      if not ($text has $c.code.repo) or not ($text =~ '\bv\d+\b') { "the workflow does not pin a release tag v<n> of the code repository" }
    })
    (check tester-seat {||
      let name = seat-secret $c (gh-ok api user --jq .login | str trim) $c.tester.provider
      let names = gh-json api $"repos/($c.ops.repo)/actions/secrets?per_page=100" | get secrets.name
      if $name not-in $names { $"secret ($name) is missing" }
    })
    (check router-deployed {||
      let r = try { http get --full --allow-errors ($c.router.url + $c.router.health) } catch {|e| return $"unreachable: ($e.msg)" }
      if $r.status != 200 { return $"health answered ($r.status)" }
      let body = if ($r.body | describe) == string { try { $r.body | from json } catch { null } } else { $r.body }
      if not ($body | describe | str starts-with record) { return "health body is not a JSON object" }
      if ($body | columns | sort) != [app version] { return $"health keys are ($body | columns | str join ', ')" }
      if $body.app != $c.app.slug or ($body.version | is-empty) { $"health body is ($body | to nuon)" }
    })
    (check webhook-signature {||
      let r = try {
        http post --full --allow-errors --content-type application/json --headers [X-GitHub-Event ping X-GitHub-Delivery $"accept-(random uuid)" X-Hub-Signature-256 $"sha256=(1..64 | each { '0' } | str join)"] ($c.router.url + $c.router.webhook) {zen: "accept.nu"}
      } catch {|e| return $"unreachable: ($e.msg)" }
      if $r.status != 401 { $"invalid signature answered ($r.status)" }
    })
    (check router-index-auth {||
      let url = $c.router.url + ($c.router.golems | str replace '{repo}' $c.ops.repo)
      let anonymous = try { http get --full --allow-errors $url } catch {|e| return $"unreachable: ($e.msg)" }
      if $anonymous.status != 401 { return $"without a token answered ($anonymous.status)" }
      let bogus = http get --full --allow-errors --headers [Authorization "Bearer ghp_invalidtokenforacceptance0000000000"] $url
      if $bogus.status not-in [401 403 404] { $"with an invalid token answered ($bogus.status)" }
    })
    (check app-oauth {||
      let client = gh-json api $"/apps/($c.app.slug)" | get client_id
      let r = http post --full --allow-errors --headers [Accept application/json] --content-type application/x-www-form-urlencoded https://github.com/login/oauth/access_token ({client_id: $client, client_secret: (secret $c $c.secrets.app_client_secret), code: "accept-invalid"} | url build-query)
      let error = $r.body.error? | default ""
      if $error != bad_verification_code { $"client secret check answered ($error)" }
    })
    (check seat-page {|| sign-in-redirect $c $c.router.seats })
    (check admin-auth {|| sign-in-redirect $c $c.router.admin })
  ]
}

# Evidence promises hold when EVIDENCE.md has a line `- <id>: ` with a link or a test path.
def evidence [c: record, promises: list] {
  let file = $env.FILE_PWD | path join EVIDENCE.md
  let lines = if ($file | path exists) { open --raw $file | lines } else { [] }
  $promises | each {|p|
    check $p.id {||
      let line = $lines | where {|l| $l starts-with $"- ($p.id): " } | get -o 0
      if $line == null or not ($line =~ 'https://\S+|\S+\.(rs|nu)\b') { $"EVIDENCE.md has no line `- ($p.id): <link to a run or path of a test>`" }
    }
  }
}

# Slice 1 in repository A, which also holds the golems of slice 2.
def core [ctx: record] {
  let c = $ctx.c
  let secret_text = random chars --length 16
  let repo = new-repo $ctx core [
    [path content];
    [".golems/hello.md" (golem {} "Read the file golem-secret.txt and quote its content in your reply.\nIf the comment that started you contains `say <text>`, end your reply with <text> exactly as written.\nChange nothing in the repository.")]
    [".golems/hi.md" (golem {} "Reply with one short greeting.\nChange nothing in the repository.")]
    [".golems/broken.md" (golem {schedule: someday} "This golem is invalid: its schedule is unknown and it has no owner.")]
    [".golems/changer.md" (golem {branch: "golems/changer", verify: "test -s golem.txt"} "Append the text after `write` in the comment that started you as a new line to golem.txt, creating the file if needed, and commit it.\nChange nothing else.")]
    [".golems/sneaky.md" (golem {branch: "golems/sneaky"} "Append the line `sneaky` to .golems/hello.md and commit it.")]
    [".golems/slow.md" (golem {timeout: "20m"} "Run the shell command `sleep 900`, then reply `done`.")]
    ["golem-secret.txt" $secret_text]
  ]
  let sha = head-sha $repo
  let index = router-index $c $repo
  let hello = mention-hello $ctx $repo $sha $secret_text
  let refused = refused-unknown $ctx $repo
  {repo: $repo, results: ([$index] ++ $hello ++ [$refused])}
}

def router-index [c: record, repo: string] {
  check router-index {||
    let fetch = {|| http get --full --allow-errors --headers [Authorization $"Bearer (gh-ok auth token | str trim)"] ($c.router.url + ($c.router.golems | str replace '{repo}' $repo)) }
    let index = wait-for $c $c.limits.indexed {||
      let r = do $fetch
      let names = $r.body.golems?.name? | default []
      if $r.status == 200 and ("hello" in $names) and ("broken" in $names) { $r.body }
    }
    if $index == null { return $"did not list hello and broken within ($c.limits.indexed)" }
    let bad = $index.golems | where {|g| $g.name != broken and ($g | get -o errors) != [] } | get name
    if ($bad | is-not-empty) { return $"valid golems without `errors: []`: ($bad | str join ', ')" }
    if ($index.golems | where name == broken | get 0 | get -o errors | default [] | is-empty) { return "broken is listed without errors" }
    put-file $repo ".golems/late.md" (golem {} "Reply with one short greeting.")
    let late = wait-for $c $c.limits.indexed {|| let r = do $fetch; if $r.status == 200 and ("late" in ($r.body.golems.name? | default [])) { true } }
    if $late == null { $"a golem pushed later was not listed within ($c.limits.indexed)" }
  }
}

# One mention proves seen, dispatched, reply, audit, and no-loop in order.
def mention-hello [ctx: record, repo: string, sha: string, secret_text: string] {
  let c = $ctx.c
  let nonce = random chars --length 12
  let issue = new-issue $repo "hello"
  let comment = new-comment $repo $issue $"@($c.app.slug) hello say ($nonce)"
  let seen = seen $c $repo $comment
  if not $seen.ok { return [$seen ...(blocked-by seen [dispatched reply audit no-loop])] }

  let reply = wait-for $c $c.limits.reply {|| bot-comments $c $repo $issue | where {|m| $m.body has $nonce } | get -o 0 }
  let dispatched = dispatched $c $repo $comment hello
  let replied = check reply {||
    if $reply == null { return $"no reply containing the nonce within ($c.limits.reply)" }
    let marker = marker $c reply hello
    if not ($reply.body | str trim | str starts-with $marker) { return $"reply does not start with ($marker)" }
    if not ($reply.body has $secret_text) { return "reply lacks the content of golem-secret.txt" }
    if (($reply.created_at | into datetime) - $comment.created) > $c.limits.reply { $"reply came after ($c.limits.reply)" }
  }
  let audit = check audit {|| audit-run $ctx $repo $sha hello replied }
  if not $replied.ok { return [$seen $dispatched $replied $audit ...(blocked-by reply [no-loop])] }

  let loop = check no-loop {||
    sleep $c.limits.loop_watch
    let runs = ops-runs $c $repo hello $comment.created | length
    let replies = bot-comments $c $repo $issue | length
    if $runs != 1 or $replies != 1 { $"after ($c.limits.loop_watch): ($runs) runner runs, ($replies) replies" }
  }
  [$seen $dispatched $replied $audit $loop]
}

def refused-unknown [ctx: record, repo: string] {
  let c = $ctx.c
  check refused-unknown {||
    let issue = new-issue $repo "unknown"
    let comment = new-comment $repo $issue $"@($c.app.slug) nosuchgolem"
    let marker = $c.marker.refused | str replace '{reason}' unknown-golem
    let reply = wait-for $c $c.limits.refusal {|| bot-comments $c $repo $issue | get -o 0 }
    if $reply == null { return $"no reply within ($c.limits.refusal)" }
    if not ($reply.body | str trim | str starts-with $marker) { return $"reply does not start with ($marker)" }
    if not ($reply.body has "hello") { return "the refusal does not list the existing golem hello" }
    let runs = ops-runs $c $repo nosuchgolem $comment.created | length
    if $runs != 0 { $"a refused mention started ($runs) runner runs" }
  }
}

# Slice 2 golems that change code, refuse forbidden paths, run one after another, and stop.
def changes [ctx: record, repo: string] {
  let c = $ctx.c
  let sha = head-sha $repo
  let first = random chars --length 12
  let second = random chars --length 12
  let issue = new-issue $repo "change"

  let pr = check change-pr {||
    let comment = new-comment $repo $issue $"@($c.app.slug) changer write ($first)"
    let pull = wait-for $c $c.limits.change {|| bot-pulls $c $repo "golems/changer" | get -o 0 }
    if $pull == null { return $"no pull request from golems/changer within ($c.limits.change)" }
    let text = file-at $repo golem.txt "golems/changer"
    if not ($text has $first) { return "golem.txt on golems/changer lacks the instructed text" }
    let missing = $c.pull_request_footer | each {|l| pattern $l {login: $ctx.login, provider: $c.tester.provider} } | where {|p| not ($pull.body | default "" | $in =~ $p) }
    if ($missing | is-not-empty) { return $"pull request body lacks lines matching: ($missing | str join ', ')" }
    audit-latest $ctx $repo $sha changer changed
  }
  let continued = if not $pr.ok { blocked-by change-pr [continue-branch] | first } else {
    check continue-branch {||
      let before = bot-pulls $c $repo "golems/changer" | get 0.number
      new-comment $repo $issue $"@($c.app.slug) changer write ($second)" | ignore
      let done = wait-for $c $c.limits.change {|| let t = file-at $repo golem.txt "golems/changer"; if ($t has $second) { $t } }
      if $done == null { return $"golems/changer did not get the second text within ($c.limits.change)" }
      if not ($done has $first) { return "the second run started over: the first text is gone" }
      let pulls = bot-pulls $c $repo "golems/changer"
      if ($pulls | length) != 1 or $pulls.0.number != $before { $"expected the same single pull request #($before), found ($pulls.number | str join ', ')" }
    }
  }

  let forbidden = check forbidden-path {||
    new-comment $repo (new-issue $repo "sneaky") $"@($c.app.slug) sneaky" | ignore
    let result = audit-latest $ctx $repo $sha sneaky blocked --wait $c.limits.change
    if $result != null { return $result }
    if (^gh api $"repos/($repo)/branches/golems/sneaky" --silent | complete).exit_code == 0 { return "branch golems/sneaky was pushed" }
    if (bot-pulls $c $repo "golems/sneaky" | is-not-empty) { return "a pull request from golems/sneaky was opened" }
    if (file-at $repo .golems/hello.md "" | str contains "sneaky") { "the default branch's .golems/hello.md was changed" }
  }

  let serial = check serial {||
    let since = date now
    let issues = [hello hello hi] | each {|g| {golem: $g, issue: (new-issue $repo $"serial ($g)")} }
    $issues | par-each {|i| new-comment $repo $i.issue $"@($c.app.slug) ($i.golem)" } | ignore
    let runs = wait-for $c $c.limits.serial {||
      let r = (ops-runs $c $repo hello $since) ++ (ops-runs $c $repo hi $since)
      if ($r | length) >= 3 and ($r | all {|x| $x.status == completed }) { $r }
    }
    if $runs == null { return $"three completed runner runs did not appear within ($c.limits.serial)" }
    let failed = $runs | where conclusion != success
    if ($failed | is-not-empty) { return $"runs ended as ($failed.conclusion | str join ', ')" }
    let spans = $runs | each {|r| run-span $c $r.id } | sort-by start
    let overlaps = $spans | window 2 | where {|w| $w.0.end > $w.1.start }
    if ($overlaps | is-not-empty) { return $"($overlaps | length) pairs of runs overlapped" }
    let unanswered = $issues | where {|i| bot-comments $c $repo $i.issue | is-empty }
    if ($unanswered | is-not-empty) { $"no reply in ($unanswered | length) of 3 issues" }
  }

  let stopped = check stop {||
    let issue = new-issue $repo "slow"
    let comment = new-comment $repo $issue $"@($c.app.slug) slow"
    let rocket = wait-for $c $c.limits.reply {|| if (bot-reactions $c $repo $comment.id | where content == rocket | is-not-empty) { true } }
    if $rocket == null { return "slow was never dispatched" }
    new-comment $repo $issue $"@($c.app.slug) stop" | ignore
    let run = wait-for $c ($c.limits.stop + $c.limits.poll) {|| ops-runs $c $repo slow $comment.created | where status == completed | get -o 0 }
    if $run == null { return $"the runner run did not end within ($c.limits.stop)" }
    if $run.conclusion != cancelled { return $"the runner run ended as ($run.conclusion)" }
    audit-latest $ctx $repo $sha slow stopped --wait $c.limits.stop
  }
  {results: [$pr $continued $forbidden $serial $stopped]}
}

# Slice 2 event triggers in repository B.
def events [ctx: record] {
  let c = $ctx.c
  let repo = new-repo $ctx events [
    [path content];
    [".golems/quiet.md" (golem {triggers: ["issues.opened"], owner: $ctx.login} "Do nothing and change nothing; there is nothing to do.")]
    [".golems/echo.md" (golem {triggers: ["issue_comment.created"], owner: $ctx.login} "Reply by quoting the comment that triggered you.")]
  ]
  let sha = head-sha $repo
  let since = date now
  let issue = new-issue $repo "event"
  let trigger = check event-trigger {||
    let result = audit-latest $ctx $repo $sha quiet no_change --wait $c.limits.reply
    if $result != null { return $result }
    let summary = check-runs $repo $sha (golem-check $c quiet) | sort-by started_at | reverse | get -o 0.output.summary | default ""
    if not ($summary =~ (pattern $c.check_run.no_change {})) { return "the check run has no reason line" }
    if (bot-comments $c $repo $issue | is-not-empty) { "quiet wrote a comment" }
  }
  let nonce = random chars --length 12
  let loop = check no-loop-event {||
    let comment = new-comment $repo $issue $"ping ($nonce)"
    let reply = wait-for $c $c.limits.reply {|| bot-comments $c $repo $issue | where {|m| $m.body has $nonce } | get -o 0 }
    if $reply == null { return $"echo did not reply within ($c.limits.reply)" }
    sleep $c.limits.loop_watch
    let runs = ops-runs $c $repo echo $comment.created | length
    let replies = bot-comments $c $repo $issue | length
    if $runs != 1 or $replies != 1 { $"after ($c.limits.loop_watch): ($runs) runner runs, ($replies) replies" }
  }
  {results: [$trigger $loop]}
}

# Slice 3 onboarding, dashboard, mentions, seats, fallback, and validation in repository C.
def onboarding [ctx: record] {
  let c = $ctx.c
  let repo = new-repo $ctx onboarding []
  put-file $repo README.md "Onboarding test repository of accept.nu.\n"

  let pull = wait-for $c $c.limits.onboarding {|| bot-pulls-titled $c $repo $c.onboarding.title | get -o 0 }
  let onboard = check onboarding {||
    if $pull == null { return $"no pull request ($c.onboarding.title) within ($c.limits.onboarding)" }
    let files = gh-json api $"repos/($repo)/pulls/($pull.number)/files" | get filename | sort
    if $files != ($c.onboarding.files | sort) { return $"it changes ($files | str join ', ')" }
    let rules = file-at $repo .golems/_golems.md $pull.head.ref | str trim
    if $rules != $c.onboarding.house_rules { $"house rules are: ($rules)" }
  }
  if not $onboard.ok { return {results: [$onboard ...(blocked-by onboarding [dashboard help seat-command fallback validation])]} }
  gh-ok pr merge $pull.number --repo $repo --squash | ignore

  let board = check dashboard {||
    let issue = wait-for $c $c.limits.dashboard {|| pinned-dashboard $c $repo }
    if $issue == null { return $"no pinned issue ($c.dashboard.title) by ($c.app.bot) within ($c.limits.dashboard)" }
    let missing = $c.dashboard.sections | where {|s| not ($issue.body =~ $"\(?m\)^#+\\s*($s)\\s*$") }
    if ($missing | is-not-empty) { return $"dashboard lacks sections: ($missing | str join ', ')" }
    if not ($issue.body has hello) { "dashboard does not list hello" }
  }

  let ask = new-issue $repo "ask"
  let help = check help {||
    new-comment $repo $ask $"@($c.app.slug)" | ignore
    let reply = wait-for $c $c.limits.refusal {|| bot-comments $c $repo $ask | where {|m| $m.body | str trim | str starts-with $c.marker.help } | get -o 0 }
    if $reply == null { return $"no reply starting with ($c.marker.help) within ($c.limits.refusal)" }
    if not ($reply.body has hello) or not ($reply.body has $c.tester.provider) { "help does not list hello and the tester's provider" }
  }
  let seat = check seat-command {||
    new-comment $repo $ask $"@($c.app.slug) seat" | ignore
    let reply = wait-for $c $c.limits.refusal {|| bot-comments $c $repo $ask | where {|m| $m.body | str trim | str starts-with $c.marker.seat } | get -o 0 }
    if $reply == null { return $"no reply starting with ($c.marker.seat) within ($c.limits.refusal)" }
    if not ($reply.body has $c.tester.provider) or not ($reply.body has ($c.router.url + $c.router.seats)) { "the reply does not name the tester's provider and link the seats page" }
  }

  put-file $repo .golems/foreign.md (golem {harness: codex, provider: github-copilot} "Reply with one short greeting.\nChange nothing in the repository.")
  let sha = head-sha $repo
  let fallback = check fallback {||
    let issue = new-issue $repo "fallback"
    new-comment $repo $issue $"@($c.app.slug) foreign" | ignore
    let reply = wait-for $c $c.limits.reply {|| bot-comments $c $repo $issue | where {|m| $m.body | str trim | str starts-with (marker $c reply foreign) } | get -o 0 }
    if $reply == null { return $"foreign did not reply within ($c.limits.reply)" }
    let run = wait-for $c $c.limits.reply {|| check-runs $repo $sha (golem-check $c foreign) | where status == completed | get -o 0 }
    if $run == null { return "foreign has no check run" }
    let summary = $run.output.summary? | default ""
    if not ($summary =~ (pattern $c.check_run.fallback {})) { return "the check run does not say it fell back" }
    if ($summary =~ 'seat: \S+/github-copilot') { "the run claims a github-copilot seat the tester does not have" }
  }

  let validation = check validation {||
    let branch = "accept/broken"
    gh-ok api -X POST $"repos/($repo)/git/refs" -f $"ref=refs/heads/($branch)" -f $"sha=($sha)" | ignore
    put-file $repo .golems/invalid.md (golem {schedule: someday} "Invalid on purpose.") --branch $branch
    let number = gh-ok pr create --repo $repo --head $branch --title "add invalid golem" --body "accept.nu validation scenario" | str trim | split row '/' | last | into int
    let head = gh-json api $"repos/($repo)/pulls/($number)" | get head.sha
    let run = wait-for $c $c.limits.validation {|| check-runs $repo $head $c.validation_check | where status == completed | get -o 0 }
    if $run == null { return $"no completed check ($c.validation_check) within ($c.limits.validation)" }
    if $run.conclusion != failure { return $"the check concluded ($run.conclusion)" }
    gh-ok pr merge $number --repo $repo --squash | ignore
    let listed = wait-for $c $c.limits.dashboard {|| let d = pinned-dashboard $c $repo; if $d != null and ($d.body | str contains "invalid") { true } }
    if $listed == null { $"the dashboard did not list invalid within ($c.limits.dashboard)" }
  }
  {results: [$onboard $board $help $seat $fallback $validation]}
}

# Slice 4 admin dashboard data for the hello run of the core phase.
def admin [ctx: record, repo: string] {
  let c = $ctx.c
  [
    (check admin-runs {||
      let url = $c.router.url + $c.router.admin_runs
      let run = wait-for $c $c.limits.admin {||
        let r = http get --full --allow-errors --headers [Authorization $"Bearer (gh-ok auth token | str trim)"] $url
        if $r.status == 200 { $r.body.runs? | default [] | where {|x| $x.repo? == $repo and $x.golem? == hello } | get -o 0 }
      }
      if $run == null { return $"($url) did not list the hello run of ($repo) within ($c.limits.admin)" }
      let fields = [repo golem trigger seat model outcome duration_s log] | where {|f| ($run | get -o $f) == null }
      if ($fields | is-not-empty) { return $"the run lacks: ($fields | str join ', ')" }
      if $run.seat != $"($ctx.login)/($c.tester.provider)" { return $"seat is ($run.seat)" }
      if not ($run.log | str starts-with $"https://github.com/($c.ops.repo)/actions/runs/") { $"log is ($run.log)" }
    })
  ] | {results: $in}
}

# Print a table, then for every failure the promise, the observation, and the next step.
def report [c: record, results: list] {
  print ($results | select id ok | table)
  let failed = $results | where not ok
  for r in $failed {
    let p = $c.promises | where id == $r.id | get -o 0 | default {text: $"($r.id) of the test repositories", guide: "Read the observed error; it comes from gh, the GitHub API, or accept.nu itself."}
    print ""
    print $"✗ ($r.id)"
    print $"  promise:  (fill-in $c $p.text)"
    print $"  observed: ($r.observed)"
    print $"  do:       (fill-in $c $p.guide)"
  }
  if ($failed | is-not-empty) { exit 1 }
}

# Run a phase; a failing setup becomes a result instead of ending the run.
def phase [ctx: record, name: string, body: closure] {
  try { do $body } catch {|e| {repo: null, results: [(result $"setup ($name)" false $e.msg)]} }
}

# Run a probe that returns null when the promise holds, or a description of what was observed.
def check [id: string, probe: closure] {
  let observed = try { do $probe } catch {|e| $"probe failed: ($e.msg)" }
  result $id ($observed == null) ($observed | default "" | into string)
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

# Replace every <path.to.key> with its value from the contract.
def fill-in [c: record, text: string] {
  $text | parse -r '<(?<key>[a-z_]+(?:\.[a-z_]+)+)>' | get key | uniq | reduce --fold $text {|key, acc|
    let value = $c | get -o ($key | split row '.' | into cell-path)
    if $value == null { $acc } else { $acc | str replace --all $"<($key)>" ($value | into string) }
  }
}

# A regex for a contract line: placeholders filled from values, {any} as one or more non-space characters.
def pattern [line: string, values: record] {
  let filled = $values | transpose key value | reduce --fold $line {|it, acc| $acc | str replace --all $"{($it.key)}" ($it.value | into string) }
  $filled | split row '{any}' | each {|part| $part | str replace --all --regex '([\\.^$|?*+()\[\]{}])' '\$1' } | str join '\S+'
}

# null when the latest check run of the golem completed with the outcome and a summary matching every contract line.
def audit-latest [ctx: record, repo: string, sha: string, golem: string, outcome: string, --wait: duration = 1min] {
  let c = $ctx.c
  let done = wait-for $c $wait {|| check-runs $repo $sha (golem-check $c $golem) | where status == completed | sort-by started_at | reverse | get -o 0 }
  if $done == null { return $"no completed check run (golem-check $c $golem) within ($wait)" }
  summary-problems $ctx $done.output.summary? $outcome $c.tester.provider
}

# null when exactly one check run of the golem exists, completes, and its summary matches.
def audit-run [ctx: record, repo: string, sha: string, golem: string, outcome: string] {
  let c = $ctx.c
  let done = wait-for $c $c.limits.reply {|| let r = check-runs $repo $sha (golem-check $c $golem); if ($r | is-not-empty) and ($r | all {|x| $x.status == completed }) { $r } }
  if $done == null { return $"no completed check run (golem-check $c $golem) within ($c.limits.reply)" }
  if ($done | length) != 1 { return $"expected 1 check run (golem-check $c $golem), found ($done | length)" }
  summary-problems $ctx $done.0.output.summary? $outcome $c.tester.provider
}

def summary-problems [ctx: record, summary: any, outcome: string, provider: string] {
  let c = $ctx.c
  let text = $summary | default ""
  let values = {outcome: $outcome, login: $ctx.login, provider: $provider, ops: $c.ops.repo}
  let missing = $c.check_run.summary | where {|l| not ($text =~ (pattern $l $values)) }
  if ($missing | is-not-empty) { $"summary lacks lines matching: ($missing | each {|l| pattern $l $values } | str join ', ')" }
}

def seen [c: record, repo: string, comment: record] {
  check seen {||
    let eyes = wait-for $c ($c.limits.seen + $c.limits.poll) {|| bot-reactions $c $repo $comment.id | where content == eyes | get -o 0 }
    if $eyes == null { return $"no eyes reaction by ($c.app.bot) within ($c.limits.seen)" }
    let latency = ($eyes.created_at | into datetime) - $comment.created
    if $latency > $c.limits.seen { $"eyes came after ($latency)" }
  }
}

def dispatched [c: record, repo: string, comment: record, golem: string] {
  check dispatched {||
    if (bot-reactions $c $repo $comment.id | where content == rocket | is-empty) { return $"no rocket reaction by ($c.app.bot)" }
    let runs = ops-runs $c $repo $golem $comment.created | length
    if $runs != 1 { $"expected 1 runner run named (run-title $c $repo $golem), found ($runs)" }
  }
}

def sign-in-redirect [c: record, path: string] {
  let client = gh-json api $"/apps/($c.app.slug)" | get client_id
  let r = try { http get --full --allow-errors --redirect-mode manual ($c.router.url + $path) } catch {|e| return $"unreachable: ($e.msg)" }
  let location = $r.headers.response? | default [] | where {|h| ($h.name | str lowercase) == location } | get -o 0.value | default ""
  if $r.status not-in [302 303 307] or not ($location | str starts-with "https://github.com/login/oauth/authorize") or not ($location has $client) {
    $"answered ($r.status) with location ($location)"
  }
}

# Delete leftover test repositories older than test_repos.stale_after.
def sweep [c: record] {
  let cutoff = (date now) - $c.test_repos.stale_after
  let stale = gh-json repo list $c.org --limit 1000 --json name,createdAt | where {|r| ($r.name starts-with $c.test_repos.prefix) and (($r.createdAt | into datetime) < $cutoff) }
  for r in $stale {
    let gone = ^gh repo delete $"($c.org)/($r.name)" --yes | complete
    if $gone.exit_code != 0 { print $"warning: could not delete stale ($r.name): ($gone.stderr | str trim)" }
  }
}

# A fresh private test repository: allowlisted first, then .golems/ before anything else, so the router never sees it without golems unless files is empty.
def new-repo [ctx: record, name: string, files: list] {
  let c = $ctx.c
  let repo = $"($c.org)/($c.test_repos.prefix)($name)-($ctx.stamp)"
  gh-ok repo create $repo --private --add-readme | ignore
  if $ctx.keep {
    print $"keeping https://github.com/($repo); delete it with: gh repo delete ($repo) --yes"
  }
  gh-ok repo edit $repo --add-topic $c.test_repos.topic | ignore
  for f in $files { put-file $repo $f.path $f.content }
  $repo
}

# Delete the test repositories of this run unless --keep.
def cleanup [ctx: record] {
  if $ctx.keep { return }
  let c = $ctx.c
  let mine = gh-json repo list $c.org --limit 1000 --json name | get name | where {|n| ($n starts-with $c.test_repos.prefix) and ($n ends-with $ctx.stamp) }
  for name in $mine {
    let gone = ^gh repo delete $"($c.org)/($name)" --yes | complete
    if $gone.exit_code != 0 { print $"warning: could not delete ($name): ($gone.stderr | str trim)" }
  }
}

# Golem Markdown with the given frontmatter fields; values are strings or lists of strings.
def golem [fields: record, task: string] {
  let front = $fields | transpose key value | each {|f| $"($f.key): ($f.value | to json -r)" }
  ["---" ...$front "---" "" $task ""] | str join "\n"
}

def marker [c: record, kind: string, golem: string] { $c.marker | get $kind | str replace '{golem}' $golem }

def golem-check [c: record, golem: string] { $c.check_run.name | str replace '{golem}' $golem }

def run-title [c: record, repo: string, golem: string] { $c.ops.run_name | str replace '{repo}' $repo | str replace '{golem}' $golem }

def secret [c: record, name: string] {
  let file = $c.secrets.dir | path expand | path join $name
  if not ($file | path exists) { error make {msg: $"($file) is missing"} }
  open --raw $file | str trim
}

def installation [c: record] {
  gh-json api $"/orgs/($c.org)/installations?per_page=100" | get installations | where id == $c.app.installation | get -o 0
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

# Poll until the probe returns something other than null, or give up with null; probe errors count as not yet.
def wait-for [c: record, timeout: duration, probe: closure] {
  let deadline = (date now) + $timeout
  loop {
    let value = try { do $probe } catch { null }
    if $value != null { return $value }
    if (date now) > $deadline { return null }
    sleep $c.limits.poll
  }
}

# gh with its exit code checked; stdout on success, an error with stderr otherwise.
def --wrapped gh-ok [...args] {
  let input = $in
  let r = if $input == null { ^gh ...$args | complete } else { $input | ^gh ...$args | complete }
  if $r.exit_code != 0 { error make {msg: $"gh ($args | str join ' ') failed: ($r.stderr | str trim)"} }
  $r.stdout
}

def --wrapped gh-json [...args] { $in | gh-ok ...$args | from json }

def head-sha [repo: string] {
  let branch = gh-ok api $"repos/($repo)" --jq .default_branch | str trim
  gh-ok api $"repos/($repo)/commits/($branch)" --jq .sha | str trim
}

def put-file [repo: string, path: string, content: string, --branch: string = ""] {
  let ref = if $branch == "" { "" } else { $"?ref=($branch)" }
  let existing = ^gh api $"repos/($repo)/contents/($path)($ref)" --jq .sha | complete
  let args = (
    [-X PUT $"repos/($repo)/contents/($path)" -f $"message=add ($path)" -f $"content=($content | encode base64)"]
    ++ (if $existing.exit_code == 0 { [-f $"sha=($existing.stdout | str trim)"] } else { [] })
    ++ (if $branch == "" { [] } else { [-f $"branch=($branch)"] })
  )
  gh-ok api ...$args | ignore
}

def file-at [repo: string, path: string, ref: string] {
  let query = if $ref == "" { "" } else { $"?ref=($ref | url encode --all)" }
  let r = ^gh api $"repos/($repo)/contents/($path)($query)" -H "Accept: application/vnd.github.raw" | complete
  if $r.exit_code == 0 { $r.stdout } else { "" }
}

def new-issue [repo: string, title: string] {
  gh-ok api -X POST $"repos/($repo)/issues" -f $"title=($title)" --jq .number | into int
}

def new-comment [repo: string, issue: int, body: string] {
  let r = gh-json api -X POST $"repos/($repo)/issues/($issue)/comments" -f $"body=($body)"
  {id: $r.id, created: ($r.created_at | into datetime)}
}

def bot-reactions [c: record, repo: string, comment: int] {
  gh-json api $"repos/($repo)/issues/comments/($comment)/reactions?per_page=100" | where {|r| $r.user.login == $c.app.bot }
}

def bot-comments [c: record, repo: string, issue: int] {
  gh-json api $"repos/($repo)/issues/($issue)/comments?per_page=100" | where {|m| $m.user.login == $c.app.bot }
}

def bot-pulls [c: record, repo: string, branch: string] {
  let owner = $repo | split row '/' | first
  gh-json api $"repos/($repo)/pulls?state=all&per_page=100&head=($owner):($branch | url encode --all)" | where {|p| $p.user.login == $c.app.bot }
}

def bot-pulls-titled [c: record, repo: string, title: string] {
  gh-json api $"repos/($repo)/pulls?state=open&per_page=100" | where {|p| $p.user.login == $c.app.bot and $p.title == $title }
}

def pinned-dashboard [c: record, repo: string] {
  let parts = $repo | split row '/'
  let query = 'query($o:String!,$n:String!){repository(owner:$o,name:$n){pinnedIssues(first:3){nodes{issue{number title state body author{login}}}}}}'
  let nodes = gh-json api graphql -f $"query=($query)" -f $"o=($parts.0)" -f $"n=($parts.1)" | get data.repository.pinnedIssues.nodes.issue
  $nodes | where {|i| $i.title == $c.dashboard.title and $i.state == OPEN and ($i.author.login | str starts-with $c.app.slug) } | get -o 0
}

def check-runs [repo: string, sha: string, name: string] {
  gh-json api $"repos/($repo)/commits/($sha)/check-runs?check_name=($name | url encode --all)&filter=all&per_page=100" | get check_runs
}

def ops-runs [c: record, repo: string, golem: string, since: datetime] {
  let iso = $since - 5sec | date to-timezone UTC | format date '%Y-%m-%dT%H:%M:%SZ'
  let workflow = $c.ops.runner_workflow | path basename
  gh-json api $"repos/($c.ops.repo)/actions/workflows/($workflow)/runs?event=workflow_dispatch&per_page=100&created=%3E%3D($iso)" | get workflow_runs | where display_title == (run-title $c $repo $golem)
}

# Start and end of a runner run, taken from its jobs so queued time does not count.
def run-span [c: record, run: int] {
  let jobs = gh-json api $"repos/($c.ops.repo)/actions/runs/($run)/jobs" | get jobs
  {
    start: ($jobs.started_at | compact | each { into datetime } | sort | first)
    end: ($jobs.completed_at | compact | each { into datetime } | sort | last)
  }
}
