FROM node:22.22.0-bookworm-slim@sha256:dd9d21971ec4395903fa6143c2b9267d048ae01ca6d3ea96f16cb30df6187d94
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git build-essential curl unzip xz-utils pkg-config ripgrep fd-find \
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
    && grep -F -- '--safe-mode' /tmp/claude-help \
    && grep -F -- '--print' /tmp/claude-help \
    && grep -F -- '--output-format' /tmp/claude-help \
    && grep -F -- '--no-session-persistence' /tmp/claude-help \
    && grep -F -- '--setting-sources' /tmp/claude-help \
    && grep -F -- '--strict-mcp-config' /tmp/claude-help \
    && grep -F -- '--mcp-config' /tmp/claude-help \
    && grep -F -- '--disable-slash-commands' /tmp/claude-help \
    && grep -F -- '--tools' /tmp/claude-help \
    && grep -F -- '--allowedTools' /tmp/claude-help \
    && grep -F -- '--settings' /tmp/claude-help \
    && rm -f /tmp/claude-help \
    && mise exec -- pi --version | grep -F '1.5.1' \
    && PI_OFFLINE=1 mise exec -- pi --help >/tmp/pi-help \
    && for flag in --mode --no-session --session-dir --offline --no-approve --no-extensions --extension \
         --no-skills --no-prompt-templates --no-themes --no-context-files --no-mcp --tools \
         --append-system-prompt --provider --model; do grep -F -- "$flag" /tmp/pi-help; done \
    && rm -rf /tmp/pi-help /root/.pi \
    && mise exec -- codex --version | grep -F 'codex-cli 0.167.1' \
    && mise exec -- codex exec --help >/tmp/codex-help \
    && for flag in --json --output-schema --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox \
         --ephemeral --ignore-user-config --ignore-rules --color --config --model; do grep -F -- "$flag" /tmp/codex-help; done \
    && rm -rf /tmp/codex-help /root/.codex \
    && mkdir -p /workspace /home/agent /golem-codex \
    && chmod -R a+rX /opt/mise /opt/mise-config \
    && chown 1000:1000 /home/agent \
    && ln -s "$(mise which claude)" /usr/local/bin/claude \
    && ln -s "$(readlink -f /usr/local/bin/claude)" /opt/golem/claude-entry \
    && ln -s "$(readlink -f "$(mise which pi)")" /opt/golem/pi-entry \
    && ln -s "$(readlink -f "$(mise which codex)")" /opt/golem/codex-entry \
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
# Pi through the real entry and stream filter, offline. Without a seat no model resolves and
# startup exits 1. With a syntactically valid but unusable seat, flags, the report extension
# and the tool allowlist load (a load failure exits 1); the request fails without network,
# which JSON mode reports as an assistant error event while exiting 0. This proves startup and
# filtered stream shape, not authentication, reporting, or tool use (those require a live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root" /home/agent/.pi' EXIT; \
    pi_probe() { \
      name="$1"; token="$2"; code=0; \
      printf 'probe' | env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=pi GOLEM_WRITABLE=0 \
        ${token:+ANTHROPIC_OAUTH_TOKEN="$token"} \
        /usr/bin/timeout --signal=KILL 90 /usr/local/bin/node /opt/golem/container.cjs \
        --mode json --no-session --session-dir /tmp/golem-pi-sessions --offline --no-approve \
        --no-extensions --extension /opt/golem/pi-report.js --no-skills --no-prompt-templates \
        --no-themes --no-context-files --no-mcp --tools read,bash,edit,write,grep,find,ls,golem_report \
        >"$root/$name.out" 2>"$root/$name.err" || code=$?; \
      cat "$root/$name.out" "$root/$name.err"; \
      echo "$code" >"$root/$name.code"; \
    }; \
    pi_probe unseated ''; \
    test "$(cat "$root/unseated.code")" -eq 1; \
    test ! -s "$root/unseated.out"; \
    pi_probe offline "sk-ant-oat01-$(printf 'x%.0s' $(seq 1 40))"; \
    test "$(cat "$root/offline.code")" -eq 0; \
    /usr/local/bin/node -e 'const fs = require("node:fs"); const records = fs.readFileSync(process.argv[1], "utf8").trim().split("\n").map(JSON.parse); const allowed = ["message_end", "tool_execution_start", "tool_execution_end", "agent_settled"]; const ends = records.filter(r => r.type === "message_end"); if (records.some(r => !allowed.includes(r.type)) || ends.length === 0 || ends.at(-1).message.stopReason !== "error" || records.filter(r => r.type === "agent_settled").length !== 1 || records.some(r => r.toolName === "golem_report")) process.exit(1);' "$root/offline.out"
# Codex through the real entry and stream filter, offline, with an invented sign-in. Startup,
# flags and every --config override must load (a config error exits before any event, with
# "Error loading config" on stderr) and the thread must start, which needs no network; the
# turn cannot complete without network. This proves startup and filtered stream shape as
# root without the runtime mount, not authentication, refresh or the result (live run).
RUN --network=none set -eu; \
    root="$(mktemp -d)"; \
    trap 'rm -rf "$root"; rm -rf /golem-codex; mkdir -p /golem-codex' EXIT; \
    /usr/local/bin/node -e 'const b = s => Buffer.from(s).toString("base64url"); const jwt = b(JSON.stringify({alg: "RS256", typ: "JWT"})) + "." + b(JSON.stringify({exp: 4102444800, "https://api.openai.com/auth": {chatgpt_plan_type: "plus", chatgpt_account_id: "acct-probe"}})) + "." + b("x".repeat(256)); require("node:fs").writeFileSync("/golem-codex/auth.json", JSON.stringify({auth_mode: "chatgpt", OPENAI_API_KEY: null, tokens: {id_token: jwt, access_token: jwt, refresh_token: "rt_probe-not-a-real-refresh-token", account_id: "acct-probe"}, last_refresh: new Date().toISOString()}));'; \
    code=0; \
    printf 'probe' | env -i PATH=/usr/local/bin:/usr/bin:/bin GOLEM_HARNESS=codex GOLEM_WRITABLE=0 HOME=/home/agent \
      /usr/bin/timeout --signal=KILL 120 /usr/local/bin/node /opt/golem/container.cjs \
      exec --json --output-schema /opt/golem/codex-result-schema.json --skip-git-repo-check \
      --dangerously-bypass-approvals-and-sandbox --ephemeral --ignore-user-config --ignore-rules --color never \
      --config 'web_search="disabled"' --config project_doc_max_bytes=0 --config check_for_update_on_startup=false \
      --config analytics.enabled=false --config feedback.enabled=false --config 'history.persistence="none"' - \
      >"$root/out" 2>"$root/err" || code=$?; \
    cat "$root/out" "$root/err"; \
    test "$code" -ne 0; \
    if grep -F -e 'Error loading config' -e 'unexpected argument' "$root/err"; then exit 1; fi; \
    /usr/local/bin/node -e 'const fs = require("node:fs"); const text = fs.readFileSync(process.argv[1], "utf8").trim(); const records = text ? text.split("\n").map(JSON.parse) : []; const allowed = ["thread.started", "turn.started", "turn.completed", "turn.failed", "error", "item.completed"]; if (!records.some(r => r.type === "thread.started") || records.some(r => !allowed.includes(r.type)) || records.some(r => r.type === "turn.completed") || records.some(r => r.type === "item.completed" && !["agent_message", "error"].includes(r.item?.type))) process.exit(1);' "$root/out"
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
RUN --network=none GOLEM_TIMEOUT_SECONDS=20 /usr/local/bin/golem-timeout --version
WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/golem-timeout"]
