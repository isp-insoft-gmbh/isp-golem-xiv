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

def exercise [root: path, scratch: path] {
  let runner = $root | path join dist golem-runner-linux-x86_64
  if not ($runner | path exists) { error make {msg: $"runner artifact missing: ($runner)"} }
  let mise = (which mise | get 0.path)

  require-status "build pinned agent image" 0 $runner -- --prepare $mise
  require-status "agent version" 0 docker -- run --rm -e GOLEM_TIMEOUT_SECONDS=10 $image --version
  require-status "zero timeout rejection" 64 docker -- run --rm -e GOLEM_TIMEOUT_SECONDS=0 $image --version

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
