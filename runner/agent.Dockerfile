FROM node:22.22.0-bookworm-slim@sha256:dd9d21971ec4395903fa6143c2b9267d048ae01ca6d3ea96f16cb30df6187d94
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git build-essential curl unzip xz-utils pkg-config ripgrep fd-find python3 \
    && rm -rf /var/lib/apt/lists/*
COPY mise /usr/local/bin/mise
COPY agent.mise.toml /opt/mise-config/config.toml
COPY container.cjs /opt/golem/container.cjs
COPY pi-report.js /opt/golem/pi-report.js
COPY codex-result-schema.json /opt/golem/codex-result-schema.json
ENV MISE_DATA_DIR=/opt/mise MISE_CONFIG_DIR=/opt/mise-config MISE_CACHE_DIR=/opt/mise-cache MISE_YES=1 CI=1 NPM_CONFIG_YES=true
RUN mise --version | grep '^2026.10.21 ' \
    && mise install \
    && mise exec -- claude --version \
    && mise exec -- claude --help >/tmp/claude-help \
    && grep -F -- '--print' /tmp/claude-help \
    && grep -F -- '--output-format' /tmp/claude-help \
    && grep -F -- '--no-session-persistence' /tmp/claude-help \
    && grep -F -- '--setting-sources' /tmp/claude-help \
    && grep -F -- '--permission-mode' /tmp/claude-help \
    && grep -F -- 'bypassPermissions' /tmp/claude-help \
    && grep -F -- '--tools' /tmp/claude-help \
    && grep -F -- '--allowedTools' /tmp/claude-help \
    && grep -F -- '--settings' /tmp/claude-help \
    && rm -f /tmp/claude-help \
    && mise exec -- pi --version | grep -F '1.5.1' \
    && PI_OFFLINE=1 mise exec -- pi --help >/tmp/pi-help \
    && for flag in --mode --no-session --session-dir --approve --extension --no-themes --provider --model; \
         do grep -F -- "$flag" /tmp/pi-help; done \
    && rm -rf /tmp/pi-help /root/.pi \
    && mise exec -- codex --version | grep -F 'codex-cli 0.167.1' \
    && mise exec -- codex exec --help >/tmp/codex-help \
    && for flag in --json --output-schema --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox \
         --ephemeral --ignore-user-config --dangerously-bypass-hook-trust --color --config --model; do grep -F -- "$flag" /tmp/codex-help; done \
    && rm -rf /tmp/codex-help /root/.codex \
    && mise exec -- uv --version | grep -F 'uv 0.13.7' \
    && python3 --version \
    && mkdir -p /workspace /home/agent /golem-codex /golem-pi \
    && chmod -R a+rX /opt/mise /opt/mise-config \
    && chown 1000:1000 /home/agent \
    && ln -s "$(mise which claude)" /usr/local/bin/claude \
    && ln -s "$(readlink -f /usr/local/bin/claude)" /opt/golem/claude-entry \
    && ln -s "$(readlink -f "$(mise which pi)")" /opt/golem/pi-entry \
    && ln -s "$(readlink -f "$(mise which codex)")" /opt/golem/codex-entry \
    && ln -s "$(readlink -f "$(mise which uv)")" /usr/local/bin/uv \
    && ln -s "$(readlink -f "$(mise which uvx)")" /usr/local/bin/uvx \
    && test -x /usr/local/bin/uvx \
    && rm -rf /opt/mise-cache /root/.npm
# --max-turns is registered but hidden from --help in the pinned CLI.
# Prove parsing through auth, not turn-limit enforcement (requires a live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root"' EXIT; \
    probe() { \
      name="$1"; expected_stdout="$2"; expected_stderr="$3"; shift 3; \
      home="$root/$name"; mkdir "$home"; \
      code=0; \
      (cd "$home" && env -i PATH=/usr/local/bin:/usr/bin:/bin \
        HOME="$home" CLAUDE_CONFIG_DIR="$home/.claude" \
        DISABLE_AUTOUPDATER=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 CI=1 \
        /usr/bin/timeout --signal=KILL 20 /usr/local/bin/claude "$@" \
        </dev/null >stdout 2>stderr) || code=$?; \
      cat "$home/stdout" "$home/stderr"; \
      test "$code" -eq 1; \
      if [ "$name" = stream ]; then \
        /usr/local/bin/node -e 'const fs = require("node:fs"); const records = fs.readFileSync(process.argv[1], "utf8").trim().split("\n").map(JSON.parse); const result = records.find(r => r.type === "result"); if (!result || result.is_error !== true || result.result !== process.argv[2] || Object.keys(result.modelUsage).length !== 0 || !records.some(r => r.type === "assistant" && r.error === "authentication_failed")) process.exit(1);' "$home/stdout" "$expected_stdout"; \
      else test "$(cat "$home/stdout")" = "$expected_stdout"; fi; \
      test "$(cat "$home/stderr")" = "$expected_stderr"; \
    }; \
    probe unknown '' "error: unknown option '--definitely-unknown-option'" \
      --definitely-unknown-option --print probe; \
    probe invalid '' "error: option '--max-turns <turns>' argument 'not-a-number' is invalid. must be a number" \
      --max-turns not-a-number --print probe; \
    probe valid 'Not logged in · Please run /login' '' \
      --max-turns 1 --print probe; \
    probe stream 'Not logged in · Please run /login' '' \
      --max-turns 1 --print --output-format stream-json --verbose probe
# Pi through the real entry, production flags and stream filter, without network. Without a
# seat no model resolves and startup exits 1. With a syntactically valid but unusable seat,
# flags, the report extension and the trusted project extension load (a load failure exits
# 1; the project extension leaves a marker, proving --approve trusts /workspace); the request
# fails without network, which JSON mode reports as an assistant error event while exiting 0.
# A project extension registering golem_report conflicts with ours, which loads first, and Pi
# exits 1 before any event. This proves startup, project trust, the name conflict and filtered
# stream shape, not authentication, reporting, or tool use (those require a live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root" /home/agent/.pi /workspace/.pi /tmp/golem-pi-project-marker' EXIT; \
    seat="sk-ant-oat01-$(printf 'x%.0s' $(seq 1 40))"; \
    pi_probe() { \
      name="$1"; token="$2"; trust="${3:---approve}"; code=0; \
      printf 'probe' | env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=pi GOLEM_WRITABLE=0 \
        ${token:+ANTHROPIC_OAUTH_TOKEN="$token"} \
        /usr/bin/timeout --signal=KILL 90 /usr/local/bin/node /opt/golem/container.cjs \
        --mode json --no-session --session-dir /tmp/golem-pi-sessions "$trust" \
        --extension /opt/golem/pi-report.js --no-themes \
        >"$root/$name.out" 2>"$root/$name.err" || code=$?; \
      cat "$root/$name.out" "$root/$name.err"; \
      echo "$code" >"$root/$name.code"; \
    }; \
    pi_probe unseated ''; \
    test "$(cat "$root/unseated.code")" -eq 1; \
    test ! -s "$root/unseated.out"; \
    mkdir -p /workspace/.pi/extensions; \
    printf '%s\n' 'import { writeFileSync } from "node:fs";' \
      'export default function () { writeFileSync("/tmp/golem-pi-project-marker", "loaded"); }' \
      >/workspace/.pi/extensions/marker.js; \
    pi_probe untrusted "$seat" --no-approve; \
    test "$(cat "$root/untrusted.code")" -eq 0; \
    test ! -e /tmp/golem-pi-project-marker; \
    pi_probe offline "$seat"; \
    test "$(cat "$root/offline.code")" -eq 0; \
    test "$(cat /tmp/golem-pi-project-marker)" = loaded; \
    printf '%s\n' 'export default function (pi) {' \
      '  pi.registerTool({ name: "golem_report", label: "fake", description: "fake",' \
      '    parameters: { type: "object", properties: {} },' \
      '    async execute() { return { content: [{ type: "text", text: "fake" }] }; } });' \
      '}' >/workspace/.pi/extensions/dup.js; \
    pi_probe conflict "$seat"; \
    test "$(cat "$root/conflict.code")" -eq 1; \
    test ! -s "$root/conflict.out"; \
    grep -F 'Tool "golem_report" conflicts with' "$root/conflict.err"; \
    /usr/local/bin/node -e 'const fs = require("node:fs"); const records = fs.readFileSync(process.argv[1], "utf8").trim().split("\n").map(JSON.parse); const allowed = ["message_end", "tool_execution_start", "tool_execution_end", "agent_settled"]; const ends = records.filter(r => r.type === "message_end"); if (records.some(r => !allowed.includes(r.type)) || ends.length === 0 || ends.at(-1).message.stopReason !== "error" || records.filter(r => r.type === "agent_settled").length !== 1 || records.some(r => r.toolName === "golem_report")) process.exit(1);' "$root/offline.out"
# Codex through the real entry, production flags and stream filter, offline, with an invented
# sign-in. Startup, flags and every --config override must load (a config error exits before
# any event, with "Error loading config" on stderr) and the thread must start, which needs no
# network; the turn cannot complete without network. The project's .codex/config.toml
# declares a stdio MCP server that only leaves a marker: without the trust override it must
# stay absent, with it Codex starts the server. This proves startup, project trust and filtered
# stream shape as root without the runtime mount, not authentication, refresh, hooks or the
# result (live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root" /workspace/.codex /tmp/golem-codex-mcp-marker; rm -rf /golem-codex; mkdir -p /golem-codex' EXIT; \
    mkdir -p /workspace/.codex; \
    printf '%s\n' '[mcp_servers.probe]' 'command = "/usr/bin/touch"' 'args = ["/tmp/golem-codex-mcp-marker"]' \
      >/workspace/.codex/config.toml; \
    codex_probe() { \
      code=0; \
      printf 'probe' | env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=codex GOLEM_WRITABLE=0 HOME=/home/agent \
        /usr/bin/timeout --signal=KILL 120 /usr/local/bin/node /opt/golem/container.cjs \
        exec --json --output-schema /opt/golem/codex-result-schema.json --skip-git-repo-check \
        --dangerously-bypass-approvals-and-sandbox --ephemeral --ignore-user-config --dangerously-bypass-hook-trust \
        --color never "$@" \
        --config 'web_search="disabled"' --config check_for_update_on_startup=false \
        --config analytics.enabled=false --config feedback.enabled=false --config 'history.persistence="none"' - \
        >"$root/out" 2>"$root/err" || code=$?; \
      cat "$root/out" "$root/err"; \
      test "$code" -ne 0; \
    }; \
    /usr/local/bin/node -e 'const b = s => Buffer.from(s).toString("base64url"); const jwt = b(JSON.stringify({alg: "RS256", typ: "JWT"})) + "." + b(JSON.stringify({exp: 4102444800, "https://api.openai.com/auth": {chatgpt_plan_type: "plus", chatgpt_account_id: "acct-probe"}})) + "." + b("x".repeat(256)); require("node:fs").writeFileSync("/golem-codex/auth.json", JSON.stringify({auth_mode: "chatgpt", OPENAI_API_KEY: null, tokens: {id_token: jwt, access_token: jwt, refresh_token: "rt_probe-not-a-real-refresh-token", account_id: "acct-probe"}, last_refresh: new Date().toISOString()}));'; \
    codex_probe; \
    test ! -e /tmp/golem-codex-mcp-marker; \
    codex_probe --config 'projects={"/workspace"={trust_level="trusted"}}'; \
    if grep -F -e 'Error loading config' -e 'unexpected argument' "$root/err"; then exit 1; fi; \
    test -e /tmp/golem-codex-mcp-marker; \
    /usr/local/bin/node -e 'const fs = require("node:fs"); const text = fs.readFileSync(process.argv[1], "utf8").trim(); const records = text ? text.split("\n").map(JSON.parse) : []; const allowed = ["thread.started", "turn.started", "turn.completed", "turn.failed", "error", "item.completed"]; if (!records.some(r => r.type === "thread.started") || records.some(r => !allowed.includes(r.type)) || records.some(r => r.type === "turn.completed") || records.some(r => r.type === "item.completed" && !["agent_message", "error"].includes(r.item?.type))) process.exit(1);' "$root/out"
# Pi on an OpenAI Codex seat through the real entry, production flags and stream filter,
# offline, with an invented sign-in in Pi's format in /golem-pi whose expiry is far ahead, so
# Pi does not refresh. With only an Anthropic token, no model resolves and startup exits 1:
# the entry forwards no Anthropic token on a Codex seat. With the file, Pi selects
# openai-codex, its default model without --model and the requested one with
# --provider/--model, and the request fails without network, reported as an assistant error
# event while exiting 0; the file is left as it was. This proves the agent directory, the file
# shape and the flags, not authentication, refresh or write-back (those require a live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root" /home/agent/.pi; rm -rf /golem-pi; mkdir -p /golem-pi' EXIT; \
    seat="sk-ant-oat01-$(printf 'x%.0s' $(seq 1 40))"; \
    codex_pi_probe() { \
      name="$1"; shift; code=0; \
      printf 'probe' | env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=pi GOLEM_WRITABLE=0 \
        GOLEM_PROVIDER=openai-codex ANTHROPIC_OAUTH_TOKEN="$seat" \
        /usr/bin/timeout --signal=KILL 90 /usr/local/bin/node /opt/golem/container.cjs \
        --mode json --no-session --session-dir /tmp/golem-pi-sessions --approve \
        --extension /opt/golem/pi-report.js --no-themes "$@" \
        >"$root/$name.out" 2>"$root/$name.err" || code=$?; \
      cat "$root/$name.out" "$root/$name.err"; \
      echo "$code" >"$root/$name.code"; \
    }; \
    codex_pi_probe unseated; \
    test "$(cat "$root/unseated.code")" -eq 1; \
    test ! -s "$root/unseated.out"; \
    /usr/local/bin/node -e 'const b = s => Buffer.from(s).toString("base64url"); const jwt = b(JSON.stringify({alg: "RS256", typ: "JWT"})) + "." + b(JSON.stringify({exp: 4102444800, "https://api.openai.com/auth": {chatgpt_account_id: "acct-probe"}})) + "." + b("x".repeat(256)); require("node:fs").writeFileSync("/golem-pi/auth.json", JSON.stringify({"openai-codex": {type: "oauth", access: jwt, refresh: "rt_probe-not-a-real-refresh-token", expires: 4102444800000, accountId: "acct-probe"}}, null, 2));'; \
    cp /golem-pi/auth.json "$root/seat.json"; \
    codex_pi_probe default; \
    codex_pi_probe chosen --provider openai-codex --model gpt-6.1-sol; \
    for name in default chosen; do \
      test "$(cat "$root/$name.code")" -eq 0; \
      /usr/local/bin/node -e 'const fs = require("node:fs"); const records = fs.readFileSync(process.argv[1], "utf8").trim().split("\n").map(JSON.parse); const allowed = ["message_end", "tool_execution_start", "tool_execution_end", "agent_settled"]; const ends = records.filter(r => r.type === "message_end"); if (records.some(r => !allowed.includes(r.type)) || ends.length === 0 || ends.at(-1).message.stopReason !== "error" || ends.at(-1).message.model !== "gpt-6.1-sol" || records.filter(r => r.type === "agent_settled").length !== 1) process.exit(1);' "$root/$name.out"; \
    done; \
    cmp "$root/seat.json" /golem-pi/auth.json
# Claude with the production read-only flags, offline and unseated, against a project allow
# rule: once directly with an empty config home (untrusted) and once through the real entry,
# which records workspace trust in the fresh config home. Both stop at sign-in. Only if the
# untrusted run prints Claude's untrusted-workspace warning before sign-in does the trusted
# run prove the trust took effect; otherwise this proves startup, flags and the trust file
# only, and trust-gated settings need a live run.
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root" /workspace/.claude /home/agent/.claude' EXIT; \
    mkdir -p /workspace/.claude "$root/untrusted-home"; \
    printf '%s\n' '{"permissions":{"allow":["Read(//workspace/**)"]}}' >/workspace/.claude/settings.json; \
    claude_probe() { \
      name="$1"; shift; code=0; \
      printf 'probe' | /usr/bin/timeout --signal=KILL 60 "$@" \
        --print --output-format json --no-session-persistence --setting-sources project,local \
        --tools Read,Glob,Grep --allowedTools 'Read(//workspace/**)' \
        --settings '{"permissions":{"deny":["WebFetch","WebSearch"]}}' --max-turns 1 \
        >"$root/$name.out" 2>"$root/$name.err" || code=$?; \
      cat "$root/$name.out" "$root/$name.err"; \
      test "$code" -eq 1; \
      grep -F 'Not logged in' "$root/$name.out"; \
    }; \
    (cd /workspace && claude_probe untrusted env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$root/untrusted-home" \
      CLAUDE_CONFIG_DIR="$root/untrusted-home/.claude" DISABLE_AUTOUPDATER=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
      /usr/local/bin/claude); \
    claude_probe trusted env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=claude GOLEM_WRITABLE=0 \
      /usr/local/bin/node /opt/golem/container.cjs; \
    if grep -F 'has not been trusted' "$root/untrusted.err"; then \
      if grep -F 'has not been trusted' "$root/trusted.err"; then exit 1; fi; \
    else echo 'untrusted-workspace warning not observable before sign-in; trust effect unproven here'; fi; \
    /usr/local/bin/node -e 'const c = JSON.parse(require("node:fs").readFileSync("/home/agent/.claude/.claude.json", "utf8")); if (c.projects?.["/workspace"]?.hasTrustDialogAccepted !== true) process.exit(1);'
RUN printf '%s\n' \
      '#!/bin/sh' \
      'set -eu' \
      ': "${GOLEM_TIMEOUT_SECONDS:?}"' \
      'case "$GOLEM_TIMEOUT_SECONDS" in *[!0-9]*|0) exit 64;; esac' \
      'exec /usr/bin/timeout --signal=KILL "${GOLEM_TIMEOUT_SECONDS}s" /usr/local/bin/node /opt/golem/container.cjs "$@"' \
      > /usr/local/bin/golem-timeout \
    && chmod 755 /usr/local/bin/golem-timeout
ENV HOME=/home/agent CLAUDE_CONFIG_DIR=/home/agent/.claude DISABLE_AUTOUPDATER=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
USER 1000:1000
RUN --network=none GOLEM_TIMEOUT_SECONDS=20 /usr/local/bin/golem-timeout --version && rm -rf /home/agent/.claude
WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/golem-timeout"]
