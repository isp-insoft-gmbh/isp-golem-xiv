#!/usr/bin/env nu
# Apply .github/settings.nuon to existing repositories; safe to run repeatedly.
def main [] {
  let settings = open ($env.FILE_PWD | path join .. .github settings.nuon)
  for entry in ($settings | values) {
    let repo = $entry.repo
    if (^gh api $"repos/($repo)" | complete).exit_code != 0 {
      print $"skip ($repo): it does not exist yet"
      continue
    }
    $entry.repository | to json | ^gh api -X PATCH $"repos/($repo)" --input - | ignore
    print $"($repo): repository settings applied"
    if ($entry.protection? != null) {
      $entry.protection.rules | to json | ^gh api -X PUT $"repos/($repo)/branches/($entry.protection.branch)/protection" --input - | ignore
      print $"($repo): protection of ($entry.protection.branch) applied"
    }
    let existing = if ($entry.rulesets? != null) { ^gh api $"repos/($repo)/rulesets" | from json } else { [] }
    for ruleset in ($entry.rulesets? | default []) {
      let found = $existing | where name == $ruleset.name | get -o 0
      let body = $ruleset | to json
      if $found == null {
        $body | ^gh api -X POST $"repos/($repo)/rulesets" --input - | ignore
      } else {
        $body | ^gh api -X PUT $"repos/($repo)/rulesets/($found.id)" --input - | ignore
      }
      print $"($repo): ruleset '($ruleset.name)' applied"
    }
  }
}
