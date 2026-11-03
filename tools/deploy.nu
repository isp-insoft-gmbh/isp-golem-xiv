#!/usr/bin/env nu
# Deliberately uses the Workers API: cf deploy consumes a different build format.
# Upload is atomic with secrets, so first deployment has no unbound-secret window.
export def deployment-metadata [version: string, private_key: string, webhook_secret: string] {
  if $version !~ '^[a-f0-9]{40}$' { error make {msg: "VERSION must be a commit SHA"} }
  if ($private_key | is-empty) or ($webhook_secret | is-empty) {
    error make {msg: "Worker secrets must not be empty"}
  }
  {
    main_module: "index.js"
    compatibility_date: "2026-11-01"
    bindings: [
      {name: APP_PRIVATE_KEY, type: secret_text, text: $private_key}
      {name: WEBHOOK_SECRET, type: secret_text, text: $webhook_secret}
      {name: VERSION, type: plain_text, text: $version}
    ]
  }
}

export def multipart-body [metadata: record, javascript: binary, wasm: binary, boundary: string] {
  if $boundary !~ '^[A-Za-z0-9-]{16,80}$' { error make {msg: "Invalid multipart boundary"} }
  let parts = [
    {name: metadata, mime: application/json, data: ($metadata | to json --raw | into binary)}
    {name: index.js, mime: application/javascript+module, data: $javascript}
    {name: index_bg.wasm, mime: application/wasm, data: $wasm}
  ]
  let marker = $boundary | into binary
  let crlf = "\r\n"
  let chunks = $parts | each {|part|
    if ($part.data | bytes index-of $marker) >= 0 { error make {msg: "Multipart boundary collision"} }
    let header = $'--($boundary)($crlf)Content-Disposition: form-data; name="($part.name)"; filename="($part.name)"($crlf)Content-Type: ($part.mime)($crlf)($crlf)'
    [($header | into binary) $part.data ($crlf | into binary)] | bytes collect
  }
  $chunks | append ($'--($boundary)--($crlf)' | into binary) | bytes collect
}

def accepted [response: record, operation: string] {
  # Never print full responses: they can contain request headers or secret data.
  let body = if ($response.body | describe) == string {
    try { $response.body | from json } catch { {} }
  } else { $response.body }
  if $response.status < 200 or $response.status >= 300 or not ($body.success? | default false) {
    error make {msg: $"Cloudflare ($operation) failed; HTTP ($response.status)"}
  }
}

export def upload-worker [url: string, authorization: string, body: binary, boundary: string] {
  let uploaded = try {
    http put --max-time 120sec --redirect-mode error --allow-errors --full --headers {
      Authorization: $authorization
      Content-Type: $"multipart/form-data; boundary=($boundary)"
    } $url $body
  } catch { error make {msg: "Worker upload transport failed; no automatic retry"} }
  accepted $uploaded upload
}

def main [] {
  if ($env.GITHUB_ACTIONS? | default "") != "true" or ($env.GITHUB_REF? | default "") != "refs/heads/trunk" or ($env.GITHUB_EVENT_NAME? | default "") != "push" {
    error make {msg: "Deployment is restricted to a trunk push in GitHub Actions"}
  }
  let root = $env.FILE_PWD | path join ..
  let account = $env.CLOUDFLARE_ACCOUNT_ID
  if $account !~ '^[a-f0-9]{32}$' { error make {msg: "Invalid Cloudflare account ID"} }
  let version = $env.GITHUB_SHA
  let metadata = deployment-metadata $version $env.APP_PRIVATE_KEY $env.WEBHOOK_SECRET
  let javascript = open --raw ($root | path join router build index.js) | into binary
  let wasm = open --raw ($root | path join router build index_bg.wasm)
  if ($javascript | bytes length) == 0 or ($javascript | bytes length) > 5_000_000 or ($wasm | bytes length) > 20_000_000 {
    error make {msg: "Worker module size is invalid"}
  }
  if not ($wasm | bytes starts-with 0x[0061736d01000000]) { error make {msg: "Invalid WebAssembly module"} }
  let boundary = $"golem-(random uuid)"
  let body = multipart-body $metadata $javascript $wasm $boundary
  let url = $"https://api.cloudflare.com/client/v4/accounts/($account)/workers/scripts/isp-golem-xiv"
  let authorization = $"Bearer ($env.CLOUDFLARE_API_TOKEN)"
  upload-worker $url $authorization $body $boundary
  let routed = try {
    http post --max-time 30sec --redirect-mode error --allow-errors --full --headers {
      Authorization: $authorization
    } --content-type application/json $"($url)/subdomain" {enabled: true, previews_enabled: false}
  } catch { error make {msg: "Worker workers.dev routing failed"} }
  accepted $routed routing
  for attempt in 1..6 {
    let health = try {
      http get --max-time 15sec --redirect-mode error https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/health
    } catch { {} }
    if ($health.app? | default "") == "isp-golem-xiv" and ($health.version? | default "") == $version {
      print $"Deployed isp-golem-xiv at ($version)"
      return
    }
    if $attempt < 6 { sleep 5sec }
  }
  error make {msg: "Uploaded Worker did not report the expected version; inspect production health"}
}
