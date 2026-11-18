#!/usr/bin/env nu

const image = "isp-golem-xiv-agent:v0"

def command-result [command: string, ...arguments: string] {
  do -i { run-external $command ...$arguments } | complete
}

def require-status [description: string, expected: int, command: string, ...arguments: string] {
  let result = command-result $command ...$arguments
  if $result.exit_code != $expected {
    error make {msg: $"($description): expected exit ($expected), got ($result.exit_code): ($result.stderr | str trim)"}
  }
}

def cleanup [scratch: path] {
  if ($scratch | path exists) { rm -rf $scratch }
  command-result docker -- image rm --force $image | ignore
}

def exercise-target [root: path, scratch: path] {
  let node = (open ($root | path join mise.toml) | get tools.node)
  let uid = (^id -u | str trim)
  let gid = (^id -g | str trim)
  for fixture in [
    {name: supported, minimum: "2026.11.9", expected: 0}
    {name: unsupported, minimum: "9999.1.0", expected: 1}
  ] {
    let workspace = $scratch | path join $fixture.name workspace
    let tools = $scratch | path join $fixture.name tools
    mkdir $workspace $tools
    [
      $'min_version = "($fixture.minimum)"'
      "[tools]"
      $'node = "($node)"'
    ] | str join (char nl) | save ($workspace | path join mise.toml)
    let arguments = [
      run --rm --interactive --read-only
      --user $"($uid):($gid)"
      --env GOLEM_TIMEOUT_SECONDS=180
      --tmpfs /tmp:rw,nosuid,nodev,exec,size=1g,mode=1777
      --tmpfs $"/home/agent:rw,nosuid,nodev,noexec,size=512m,uid=($uid),gid=($gid),mode=700"
      --mount $"type=bind,src=($workspace),dst=/workspace"
    ]
    let tool_mount = $"type=bind,src=($tools),dst=/target-tools"
    let prepared = command-result docker -- ...$arguments --mount $tool_mount $image --prepare-target
    if $prepared.exit_code != $fixture.expected {
      error make {msg: $"target minimum ($fixture.minimum): expected exit ($fixture.expected), got ($prepared.exit_code): ($prepared.stderr | str trim)"}
    }
    if $fixture.expected == 0 {
      if not (($tools | path join paths) | path exists) {
        error make {msg: "target preparation did not create a tool manifest"}
      }
      let command = ['case "$(command -v node)" in /target-tools/*) ;; *) exit 1 ;; esac; test "$(node --version)" = "v' $node '"'] | str join
      let verified = ($command | ^docker ...$arguments --mount $"($tool_mount),readonly" $image --verify-target | complete)
      if $verified.exit_code != 0 {
        error make {msg: $"prepared target Node verification failed: ($verified.stderr | str trim)"}
      }
    } else if not ($prepared.stderr | str contains $fixture.minimum) or not ($prepared.stderr | str contains "required") or (($tools | path join paths) | path exists) {
      error make {msg: "unsupported target minimum did not fail specifically before tool activation"}
    }
  }
}

def exercise [root: path, scratch: path] {
  let runner = $root | path join dist golem-runner-linux-x86_64
  if not ($runner | path exists) { error make {msg: $"runner artifact missing: ($runner)"} }
  let mise = (which mise | get 0.path)

  require-status "build pinned agent image" 0 $runner -- --prepare $mise
  require-status "agent version" 0 docker -- run --rm -e GOLEM_TIMEOUT_SECONDS=10 $image --version
  require-status "zero timeout rejection" 64 docker -- run --rm -e GOLEM_TIMEOUT_SECONDS=0 $image --version
  exercise-target $root $scratch

  let claude = $scratch | path join claude
  "#!/bin/sh\nsleep 30\n" | save --force $claude
  require-status "make synthetic claude executable" 0 chmod -- 755 $claude
  let started = date now
  let result = command-result docker -- run --rm -e GOLEM_TIMEOUT_SECONDS=1 --mount $"type=bind,src=($claude),dst=/usr/local/bin/claude,readonly" $image
  let elapsed = (date now) - $started
  if $result.exit_code != 137 {
    error make {msg: $"default timeout: expected exit 137, got ($result.exit_code): ($result.stderr | str trim)"}
  }
  if $elapsed >= 10sec {
    error make {msg: $"default timeout exceeded bound: ($elapsed)"}
  }
}

def main [] {
  let root = $env.FILE_PWD | path join ..
  let scratch = mktemp --directory
  try {
    exercise $root $scratch
  } catch {|error|
    cleanup $scratch
    error make $error
  }
  cleanup $scratch
  print "agent container: ok"
}
