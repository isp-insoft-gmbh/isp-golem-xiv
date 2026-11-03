#!/usr/bin/env nu
# Apply .github/settings.nuon to existing repositories; safe to run repeatedly.
# Every call is checked: a rejected change stops the script instead of being reported as applied.
def main [] {
  let settings = open ($env.FILE_PWD | path join .. .github settings.nuon)
  for entry in ($settings | values) {
    let repo = $entry.repo
    if (^gh api $"repos/($repo)" | complete).exit_code != 0 {
      print $"skip ($repo): it does not exist yet"
      continue
    }
    $entry.repository | send PATCH $"repos/($repo)"
    print $"($repo): repository settings applied"

    if ($entry.actions? != null) {
      $entry.actions.permissions | send PUT $"repos/($repo)/actions/permissions"
      $entry.actions.workflow | send PUT $"repos/($repo)/actions/permissions/workflow"
      print $"($repo): actions settings applied"
    }

    for environment in ($entry.environments? | default []) {
      let base = $"repos/($repo)/environments/($environment.name)"
      {deployment_branch_policy: {protected_branches: false, custom_branch_policies: true}} | send PUT $base
      let existing = gh-json api $"($base)/deployment-branch-policies?per_page=100" | get branch_policies | each {|p| $"($p.type? | default branch):($p.name)" }
      let wanted = ($environment.branches | each {|b| {name: $b, type: branch} }) ++ ($environment.tags | each {|t| {name: $t, type: tag} })
      for policy in ($wanted | where {|p| $"($p.type):($p.name)" not-in $existing }) {
        $policy | send POST $"($base)/deployment-branch-policies"
      }
      print $"($repo): environment ($environment.name) applied"
    }

    if ($entry.protection? != null) {
      $entry.protection.rules | send PUT $"repos/($repo)/branches/($entry.protection.branch)/protection"
      print $"($repo): protection of ($entry.protection.branch) applied"
    }

    let existing = if ($entry.rulesets? != null) { gh-json api $"repos/($repo)/rulesets" } else { [] }
    for ruleset in ($entry.rulesets? | default []) {
      let found = $existing | where name == $ruleset.name | get -o 0
      if $found == null {
        $ruleset | send POST $"repos/($repo)/rulesets"
      } else {
        $ruleset | send PUT $"repos/($repo)/rulesets/($found.id)"
      }
      print $"($repo): ruleset '($ruleset.name)' applied"
    }
  }
}

# Send the piped record as JSON; fail with GitHub's answer when it is rejected.
def send [method: string, path: string] {
  let r = $in | to json | ^gh api -X $method $path --input - | complete
  if $r.exit_code != 0 { error make {msg: $"($method) ($path) failed: ($r.stderr | str trim) ($r.stdout | str trim)"} }
}

def --wrapped gh-json [...args] {
  let r = ^gh ...$args | complete
  if $r.exit_code != 0 { error make {msg: $"gh ($args | str join ' ') failed: ($r.stderr | str trim)"} }
  $r.stdout | from json
}
