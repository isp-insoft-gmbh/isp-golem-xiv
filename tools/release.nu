#!/usr/bin/env nu
# Push the next annotated release tag v<n>, where n counts up from 0; CI validates, builds, and publishes the release.
# Release tags are immutable: a release that fails in CI costs its number, the next one takes n + 1.
def main [
  message: string # one-line summary of the release, used at the top of the release notes
] {
  if (^git status --porcelain | str trim | is-not-empty) { error make {msg: "working tree is not clean"} }
  if (^git branch --show-current | str trim) != "trunk" { error make {msg: "release from trunk only"} }
  ^git fetch origin trunk --tags
  let head = ^git rev-parse HEAD | str trim
  if $head != (^git rev-parse origin/trunk | str trim) { error make {msg: "trunk is not equal to origin/trunk"} }
  let ci = ^gh run list --repo isp-insoft-gmbh/isp-golem-xiv --commit $head --workflow ci --json conclusion | from json
  if ($ci | where conclusion == success | is-empty) { error make {msg: $"ci has not passed on ($head); wait for it or fix it first"} }
  let next = ^git tag --list 'v*' | lines | parse 'v{n}' | where {|t| $t.n =~ '^\d+$' } | get -o n | default [] | into int | append (-1) | math max | $in + 1
  let tag = $"v($next)"
  ^git tag --annotate $tag --message $message
  let pushed = ^git push origin $tag | complete
  if $pushed.exit_code != 0 {
    ^git tag --delete $tag
    error make {msg: $"pushing ($tag) failed, the local tag was removed: ($pushed.stderr | str trim)"}
  }
  print $"pushed ($tag); follow the release with: gh run watch --repo isp-insoft-gmbh/isp-golem-xiv"
}
