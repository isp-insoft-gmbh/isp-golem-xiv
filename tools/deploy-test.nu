#!/usr/bin/env nu
use std/assert
use deploy.nu [deployment-metadata multipart-body]

def main [] {
  let version = "0123456789abcdef0123456789abcdef01234567"
  let metadata = deployment-metadata $version "dummy-private-key" "dummy-webhook-secret"
  assert equal $metadata.main_module "index.js"
  assert equal ($metadata.bindings | where name == VERSION | get 0.text) $version
  assert equal ($metadata.bindings | where type == secret_text | get name) [APP_PRIVATE_KEY WEBHOOK_SECRET]
  assert error { deployment-metadata "bad-version" key secret }
  assert error { deployment-metadata $version "" secret }
  assert error { deployment-metadata $version key "" }
  let boundary = "golem-test-boundary-123456"
  let javascript = 'import wasm from "./index_bg.wasm"; export default {};' | into binary
  # Includes invalid UTF-8, NUL and newline bytes: text conversion would corrupt it.
  let wasm = 0x[0061736d01000000fffe000d0a]
  let body = multipart-body $metadata $javascript $wasm $boundary
  let delimiter = $"--($boundary)" | into binary
  let parts = $body | bytes split $delimiter
  assert equal ($parts | length) 5
  assert equal ($parts | get 0) 0x[]
  assert equal ($parts | get 4) ("--\r\n" | into binary)
  let metadata_part = $parts | get 1 | bytes split ("\r\n\r\n" | into binary)
  let decoded = $metadata_part | get 1 | decode utf-8 | str trim | from json
  assert equal $decoded $metadata
  let javascript_part = $parts | get 2 | bytes split ("\r\n\r\n" | into binary)
  assert (($javascript_part | get 0 | decode utf-8) | str contains "Content-Type: application/javascript+module")
  assert equal ($javascript_part | get 1) ([$javascript ("\r\n" | into binary)] | bytes collect)
  let wasm_part = $parts | get 3 | bytes split ("\r\n\r\n" | into binary)
  assert (($wasm_part | get 0 | decode utf-8) | str contains 'filename="index_bg.wasm"')
  assert (($wasm_part | get 0 | decode utf-8) | str contains "Content-Type: application/wasm")
  assert equal ($wasm_part | skip 1 | bytes collect ("\r\n\r\n" | into binary)) ([$wasm ("\r\n" | into binary)] | bytes collect)
  assert error { multipart-body $metadata $javascript $wasm "bad\r\nboundary" }
  assert error { multipart-body $metadata ($boundary | into binary) $wasm $boundary }
  # The real entrypoint must refuse local execution before reading any secrets.
  let refused = with-env {GITHUB_ACTIONS: "false"} {
    ^nu --no-config-file ($env.FILE_PWD | path join deploy.nu) | complete
  }
  assert ($refused.exit_code != 0)
  assert ($refused.stderr | str contains "Deployment is restricted")
  print "deploy: metadata, multipart binary integrity, boundary checks, CI restriction passed"
}
