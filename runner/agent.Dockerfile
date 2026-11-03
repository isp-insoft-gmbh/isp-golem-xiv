FROM node:22.22.0-bookworm-slim@sha256:dd9d21971ec4395903fa6143c2b9267d048ae01ca6d3ea96f16cb30df6187d94
COPY mise /usr/local/bin/mise
COPY agent.mise.toml /opt/mise-config/config.toml
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
    && mkdir -p /workspace /home/agent \
    && chmod -R a+rX /opt/mise /opt/mise-config \
    && chown 1000:1000 /home/agent \
    && ln -s "$(mise which claude)" /usr/local/bin/claude \
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
      test "$(cat "$home/stdout")" = "$expected_stdout"; \
      test "$(cat "$home/stderr")" = "$expected_stderr"; \
    }; \
    probe unknown '' "error: unknown option '--definitely-unknown-option'" \
      --definitely-unknown-option --print probe; \
    probe invalid '' "error: option '--max-turns <turns>' argument 'not-a-number' is invalid. must be a number" \
      --max-turns not-a-number --print probe; \
    probe valid 'Not logged in · Please run /login' '' \
      --max-turns 1 --print probe
RUN printf '%s\n' \
      '#!/bin/sh' \
      'set -eu' \
      ': "${GOLEM_TIMEOUT_SECONDS:?}"' \
      'case "$GOLEM_TIMEOUT_SECONDS" in *[!0-9]*|0) exit 64;; esac' \
      'exec /usr/bin/timeout --signal=KILL "${GOLEM_TIMEOUT_SECONDS}s" /usr/local/bin/claude "$@"' \
      > /usr/local/bin/golem-timeout \
    && chmod 755 /usr/local/bin/golem-timeout
ENV HOME=/home/agent CLAUDE_CONFIG_DIR=/home/agent/.claude DISABLE_AUTOUPDATER=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
USER 1000:1000
RUN claude --version
WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/golem-timeout"]
