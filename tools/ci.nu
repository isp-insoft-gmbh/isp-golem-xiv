#!/usr/bin/env nu
# Repository gate run by CI; extend it as code arrives.
def main [] {
  let root = $env.FILE_PWD | path join ..
  for script in [accept.nu tools/release.nu tools/gh-settings.nu tools/ci.nu] {
    if not (nu-check ($root | path join $script)) { error make {msg: $"($script) does not parse"} }
  }
  for data in [contract.nuon .github/settings.nuon] { open ($root | path join $data) | ignore }
  print "ci: ok"
}
