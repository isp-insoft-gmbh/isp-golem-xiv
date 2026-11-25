#!/usr/bin/env nu
# Checks the development prerequisites mise cannot install, and says how to fix each one.
# Runs before `mise install`, so it uses no project tool except Nushell:
#   mise x nushell -- nu tools/doctor.nu

def present [command: string] { which $command | is-not-empty }

def --wrapped outcome [command: string, ...arguments: string] {
  if not (present $command) { return {exit_code: 127, stdout: '', stderr: $'($command) not found'} }
  do -i { run-external $command ...$arguments } | complete
}

def first-line [text: string] {
  $text | lines | where {|line| $line =~ 'ERROR|error:' and not ($line =~ 'Version:|--verbose') } | get -o 0 | default ($text | lines | get -o 0 | default '') | str replace -r '^.*?ERROR\s+' '' | str trim
}

def check [name: string, ok: bool, detail: string, fix: string, --optional] {
  {check: $name, status: (if $ok { 'ok' } else if $optional { 'warn' } else { 'FAIL' }), detail: $detail, fix: (if $ok { '' } else { $fix })}
}

def rust-checks [root: string] {
  let pin = (open ($root | path join rust-toolchain.toml) | get toolchain)
  if not (present rustup) {
    return [(check rustup false 'not found' 'Install rustup from https://rustup.rs; it installs the pinned Rust from rust-toolchain.toml.')]
  }
  # Outside the checkout, so the pinned toolchain is neither required nor installed by this query.
  let host = (do { cd $nu.temp-dir; outcome rustup show } | get stdout | parse --regex 'Default host: (?<host>\S+)' | get -o 0.host | default unknown)
  # rustup may be a shim that reads the checkout's config, so every query runs from the temp directory.
  let installed = (do { cd $nu.temp-dir; outcome rustup component list --installed --toolchain $pin.channel })
  let components = ($installed.stdout | lines | each {|line| $line | str replace -r '-(x86_64|aarch64|i686)-.*$' '' })
  let targets = (do { cd $nu.temp-dir; outcome rustup target list --installed --toolchain $pin.channel } | get stdout | lines)
  let missing = ($pin.components | where {|c| $c not-in $components }) ++ ($pin.targets | where {|t| $t not-in $targets })
  mut results = [(check rustup true $'host ($host)' '')]
  $results ++= [(check $'Rust ($pin.channel)' ($installed.exit_code == 0 and ($missing | is-empty))
    (if $installed.exit_code != 0 { $'toolchain not available: (first-line $installed.stderr)' } else if ($missing | is-empty) { 'toolchain, components and targets installed' } else { $'missing ($missing | str join ", ")' })
    'rustup installs it on the first cargo command in this checkout; or run `rustup show` here now.' --optional)]
  if $nu.os-info.name == windows {
    $results ++= [(check 'MSVC host' ($host | str ends-with '-windows-msvc') $host 'Use the MSVC host: `rustup set default-host x86_64-pc-windows-msvc`.')]
    let vswhere = 'C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe'
    let tools = (if ($vswhere | path exists) { outcome $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | get stdout | str trim } else { '' })
    $results ++= [(check 'MSVC build tools' ($tools | is-not-empty) (if ($tools | is-empty) { 'C++ build tools not found' } else { $tools })
      'Install them: `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`.')]
  }
  $results
}

def mise-checks [root: string] {
  # `--show` also lists parent directories; only this checkout's own entry counts.
  let trust = (outcome mise trust --show)
  let own = ($root | path expand | str lowercase)
  let trusted = ($trust.stdout | lines | parse --regex '^(?<path>.+): (?<status>\S+)$'
    | any {|entry| ($entry.path | path expand | str lowercase) == $own and $entry.status == trusted })
  [(check 'mise trust' $trusted (if $trusted { 'checkout trusted' } else { 'checkout not trusted' }) 'Review mise.toml, then run `mise trust` in this checkout.')]
}

# The Worker suites run in the default WSL distribution on Windows, from this checkout's mise tools.
# Read-only queries: nothing here installs a tool.
def wsl-checks [root: string] {
  if $nu.os-info.name != windows { return [] }
  if (outcome wsl.exe --status | get exit_code) != 0 {
    return [(check WSL false 'no default distribution' 'Install one: `wsl --install -d Ubuntu`, then install mise in it.')]
  }
  if (outcome wsl.exe --cd $root --exec mise --version | get exit_code) != 0 {
    return [(check 'WSL mise' false 'mise not on the WSL path' 'In WSL, install mise (https://mise.jdx.dev) so that `mise` is on the PATH of non-interactive shells.')]
  }
  let path = (outcome wsl.exe --cd $root --exec pwd | get stdout | str trim)
  let trusted = (outcome wsl.exe --cd $root --exec mise trust --show | get stdout | lines | any {|line| $line == $'($path): trusted' })
  let listing = (outcome wsl.exe --cd $root --exec mise ls --missing --json)
  let missing = (if $listing.exit_code != 0 { [$"\(mise ls failed: (first-line $listing.stderr)\)"] } else {
    $listing.stdout | from json | transpose tool entries
      | where {|t| $t.entries | any {|e| ($e.source?.path? | default '') == $'($path)/mise.toml' } } | get tool
  })
  [
    (check 'WSL trust' $trusted (if $trusted { $'($path) trusted' } else { $'($path) not trusted' }) $'In WSL: `cd "($path)" && mise trust`.')
    (check 'WSL tools' ($missing | is-empty) (if ($missing | is-empty) { "this checkout's tools installed" } else { $'missing ($missing | str join ", ")' }) $'In WSL: `cd "($path)" && mise install`.')
  ]
}

def docker-checks [] {
  let server = (outcome docker version --format '{{.Server.Version}}')
  [(check Docker ($server.exit_code == 0) (if $server.exit_code == 0 { $'server ($server.stdout | str trim)' } else { 'not available' })
    'Needed only for `mise run test-agent`, which CI runs on every pull request; install Docker Engine or Docker Desktop to run it locally.' --optional)]
}

def main [] {
  # A tool shim, such as a mise-managed rustup, must not install anything while the doctor looks.
  $env.MISE_AUTO_INSTALL = 'false'
  let root = ($env.FILE_PWD | path dirname)
  let results = (rust-checks $root) ++ (mise-checks $root) ++ (wsl-checks $root) ++ (docker-checks)
  for result in $results {
    print $"($result.status | fill --width 4)  ($result.check): ($result.detail)"
    if $result.fix != '' { print $"      fix: ($result.fix)" }
  }
  let failed = ($results | where status == FAIL)
  if ($failed | is-not-empty) {
    error make --unspanned {msg: $"($failed | length) prerequisite\(s\) missing; fix them, then run `mise install` and `mise run ci`."}
  }
  print 'Ready: run `mise install`, then `mise run ci`.'
}
