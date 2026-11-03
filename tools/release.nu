#!/usr/bin/env nu
# Push the next annotated release tag v<n>, where n counts up from 0; CI validates, builds, and publishes the release.
def main [
  message: string # one-line summary of the release, used at the top of the release notes
] {
  if (^git status --porcelain | str trim | is-not-empty) { error make {msg: "working tree is not clean"} }
  if (^git branch --show-current | str trim) != "trunk" { error make {msg: "release from trunk only"} }
  ^git fetch origin trunk --tags
  if (^git rev-parse HEAD | str trim) != (^git rev-parse origin/trunk | str trim) { error make {msg: "trunk is not equal to origin/trunk"} }
  let next = ^git tag --list 'v*' | lines | parse 'v{n}' | where {|t| $t.n =~ '^\d+$' } | get -o n | default [] | into int | append (-1) | math max | $in + 1
  let tag = $"v($next)"
  ^git tag --annotate $tag --message $message
  ^git push origin $tag
  print $"pushed ($tag); follow the release with: gh run watch --repo isp-insoft-gmbh/isp-golem-xiv"
}
