'use strict';
// Repository toolchain environment (mise env, NODE_OPTIONS, LD_PRELOAD) is never imported
// here. The repository's harness configuration loads natively inside each harness, so its
// hooks, extensions and MCP servers run as children of the harness and inherit its seat.
const fs = require('node:fs');
const cp = require('node:child_process');
const fixedPath = '/usr/local/bin:/usr/bin:/bin';
const toolRoot = '/target-tools';
const targetEnv = {
  PATH: fixedPath, HOME: '/home/agent', CI: '1', MISE_YES: '1',
  MISE_DATA_DIR: toolRoot + '/data', MISE_CACHE_DIR: toolRoot + '/cache',
  MISE_CONFIG_DIR: toolRoot + '/config', MISE_STATE_DIR: toolRoot + '/state',
  MISE_TRUSTED_CONFIG_PATHS: '/workspace',
  CARGO_HOME: toolRoot + '/cargo', RUSTUP_HOME: toolRoot + '/rustup',
  CARGO_TARGET_DIR: '/tmp/golem-target',
  TMPDIR: '/tmp', XDG_CACHE_HOME: '/tmp/golem-cache',
  npm_config_cache: '/tmp/golem-npm-cache', PIP_CACHE_DIR: '/tmp/golem-pip-cache',
  GOCACHE: '/tmp/golem-go-cache',
  // /home/agent is noexec: uv-managed interpreters and tools must live under /tmp.
  UV_PYTHON_INSTALL_DIR: '/tmp/golem-uv-python', UV_TOOL_DIR: '/tmp/golem-uv-tools',
};
function toolEnvironment() {
  // Preparation is untrusted: never follow a link or read a device/FIFO here.
  const fd = fs.openSync(toolRoot + '/paths', fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const stat = fs.fstatSync(fd);
    if (!stat.isFile() || stat.size > 8192) throw Error('invalid tool paths');
    const bytes = Buffer.alloc(8193);
    const count = fs.readSync(fd, bytes, 0, bytes.length, 0);
    if (count > 8192) throw Error('tool paths exceed limit');
    const manifest = JSON.parse(bytes.subarray(0, count).toString('utf8'));
    if (typeof manifest.paths !== 'string') throw Error('invalid tool paths');
    const value = manifest.paths.trim();
    const entries = value ? value.split('\n') : [];
    if (entries.some(p => !p.startsWith(toolRoot + '/') || /[:\x00-\x20\x7f]/.test(p))) {
      throw Error('tool path outside isolated toolchain');
    }
    const env = { ...targetEnv, PATH: [...entries, fixedPath].join(':'), CARGO_HOME: '/home/agent/.cargo' };
    if (manifest.rustToolchain !== undefined) {
      if (typeof manifest.rustToolchain !== 'string' || !/^[a-zA-Z0-9_.-]{1,128}$/.test(manifest.rustToolchain)) throw Error('invalid Rust toolchain');
      env.RUSTUP_TOOLCHAIN = manifest.rustToolchain;
    }
    return env;
  } finally { fs.closeSync(fd); }
}
function execute(executable, args, env, filter) {
  // Forward termination and preserve the watchdog's process group.
  const child = cp.spawn(executable, args, { env, stdio: filter ? ['inherit', 'pipe', 'inherit'] : 'inherit', cwd: '/workspace' });
  child.on('error', () => { process.exitCode = 1; });
  child.on(filter ? 'close' : 'exit', (code, signal) => { process.exitCode = signal ? 1 : code; });
  for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));
  if (!filter) return;
  // JSONL records end only at LF (never Unicode separators); a final partial line passes as is.
  let parts = [];
  const emit = bytes => {
    const kept = filter(bytes.toString('utf8').replace(/\r$/, ''));
    if (kept !== undefined) process.stdout.write(kept + '\n');
  };
  child.stdout.on('data', chunk => {
    let start = 0;
    for (let end = chunk.indexOf(10); end !== -1; end = chunk.indexOf(10, start)) {
      parts.push(chunk.subarray(start, end));
      emit(Buffer.concat(parts));
      parts = [];
      start = end + 1;
    }
    if (start < chunk.length) parts.push(chunk.subarray(start));
  });
  child.stdout.on('end', () => { if (parts.length) emit(Buffer.concat(parts)); });
}
// Pi's JSON stream repeats content in deltas, turn and agent summaries. Keep only the facts the
// runner judges: assistant model, stop reason and text; tool identities; the single report's
// arguments; settlement. Malformed lines pass unchanged so the runner fails closed.
function piRecord(line) {
  let record;
  try { record = JSON.parse(line); } catch { return line; }
  if (record === null || typeof record !== 'object' || Array.isArray(record)) return line;
  const { type, message, toolCallId, toolName } = record;
  if (type === 'message_end' && message?.role === 'assistant') {
    const content = Array.isArray(message.content) ? message.content.filter(block => block?.type === 'text').map(({ type, text }) => ({ type, text })) : [];
    return JSON.stringify({ type, message: { role: 'assistant', model: message.model, responseModel: message.responseModel, stopReason: message.stopReason, content } });
  }
  if (type === 'tool_execution_start') return JSON.stringify({ type, toolCallId, toolName, args: toolName === 'golem_report' ? record.args : undefined });
  if (type === 'tool_execution_end') return JSON.stringify({ type, toolCallId, toolName, isError: record.isError });
  if (type === 'agent_settled') return JSON.stringify({ type, aborted: record.aborted });
  return undefined;
}
// Codex's JSONL repeats command output and file changes. Keep only what the runner judges:
// thread and turn lifecycle, failures, agent messages and non-fatal notices. Records of other
// types (project hooks, MCP, newer versions) cannot carry the result and are dropped;
// malformed lines pass unchanged so the runner fails closed.
function codexRecord(line) {
  let record;
  try { record = JSON.parse(line); } catch { return line; }
  if (record === null || typeof record !== 'object' || Array.isArray(record)) return line;
  const { type, item } = record;
  if (type === 'thread.started' || type === 'turn.started' || type === 'turn.completed') return JSON.stringify({ type });
  if (type === 'turn.failed') return JSON.stringify({ type, error: { message: record.error?.message } });
  if (type === 'error') return JSON.stringify({ type, message: record.message });
  if (type === 'item.completed' && item?.type === 'agent_message') return JSON.stringify({ type, item: { id: item.id, type: item.type, text: item.text } });
  if (type === 'item.completed' && item?.type === 'error') return JSON.stringify({ type, item: { id: item.id, type: item.type, message: item.message } });
  if (typeof type !== 'string') return line;
  return undefined;
}
function harnessEntry(entry, args, env, filter) {
  // npm may provide either a JS launcher or a native executable. Resolve once
  // at image build, never through the target's PATH or mise configuration.
  const fd = fs.openSync(entry, 'r');
  const magic = Buffer.alloc(256);
  const count = fs.readSync(fd, magic, 0, magic.length, 0);
  fs.closeSync(fd);
  const javascript = magic.subarray(0, count).toString().startsWith('#!') && magic.subarray(0, count).toString().split('\n')[0].includes('node');
  execute(javascript ? '/usr/local/bin/node' : entry, javascript ? [entry, ...args] : args, env, filter);
}
const args = process.argv.slice(2);
if (args[0] === '--prepare-target') {
  // The whole process tree is credential-free, including plugins/install hooks.
  const install = cp.spawnSync('/usr/local/bin/mise', ['install'], { env: targetEnv, stdio: 'inherit', cwd: '/workspace' });
  if (install.error || install.status !== 0) process.exit(1);
  const bins = cp.spawnSync('/usr/local/bin/mise', ['bin-paths'], { env: targetEnv, cwd: '/workspace', encoding: 'utf8', maxBuffer: 8192 });
  if (bins.error || bins.status !== 0) process.exit(1);
  const activation = cp.spawnSync('/usr/local/bin/mise', ['env', '--json'], { env: targetEnv, cwd: '/workspace', encoding: 'utf8', maxBuffer: 65536 });
  if (activation.error || activation.status !== 0) process.exit(1);
  // Rust proxies require a persistent rustup home plus the selected toolchain.
  // Never import arbitrary repository environment (NODE_OPTIONS, LD_PRELOAD, etc.).
  const selected = JSON.parse(activation.stdout);
  fs.writeFileSync(toolRoot + '/paths', JSON.stringify({ paths: bins.stdout, rustToolchain: selected.RUSTUP_TOOLCHAIN }), { flag: 'wx', mode: 0o600 });
  toolEnvironment();
} else if (args[0] === '--verify-target') {
  const command = fs.readFileSync(0, 'utf8');
  execute('/bin/bash', ['--noprofile', '--norc', '-c', command || ':'], toolEnvironment());
} else {
  const harness = process.env.GOLEM_HARNESS ?? 'claude';
  const base = process.env.GOLEM_WRITABLE === '1' ? toolEnvironment() : targetEnv;
  // Each harness receives only its own seat variable, never the other's.
  if (harness === 'pi') {
    // A Codex seat is only Pi's auth.json in the private, writable agent directory mount;
    // an Anthropic seat is only its token variable.
    const codexSeat = process.env.GOLEM_PROVIDER === 'openai-codex';
    harnessEntry('/opt/golem/pi-entry', args, {
      ...base, PI_CODING_AGENT_DIR: codexSeat ? '/golem-pi' : '/home/agent/.pi/agent',
      PI_SKIP_VERSION_CHECK: '1', PI_TELEMETRY: '0', JITI_FS_CACHE: 'false',
      ANTHROPIC_OAUTH_TOKEN: codexSeat ? undefined : process.env.ANTHROPIC_OAUTH_TOKEN,
    }, piRecord);
  } else if (harness === 'codex') {
    // The seat is only the sign-in file in the private, writable CODEX_HOME mount.
    harnessEntry('/opt/golem/codex-entry', args, {
      ...base, CODEX_HOME: '/golem-codex', NO_COLOR: '1',
    }, codexRecord);
  } else if (harness === 'claude') {
    // `claude -p` never shows the workspace trust dialog; record its acceptance in the fresh
    // config home so trust-gated project settings (allow rules, subagent hooks and MCP,
    // headersHelper) apply like in an interactive session that trusted the folder.
    fs.mkdirSync('/home/agent/.claude', { recursive: true, mode: 0o700 });
    fs.writeFileSync('/home/agent/.claude/.claude.json',
      JSON.stringify({ projects: { '/workspace': { hasTrustDialogAccepted: true } } }), { mode: 0o600 });
    harnessEntry('/opt/golem/claude-entry', args, {
      ...base, CLAUDE_CONFIG_DIR: '/home/agent/.claude', DISABLE_AUTOUPDATER: '1',
      CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1',
      CLAUDE_CODE_OAUTH_TOKEN: process.env.CLAUDE_CODE_OAUTH_TOKEN,
    });
  } else {
    process.exit(64);
  }
}
