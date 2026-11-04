'use strict';
// Never activate repository configuration around the credentialed harness.
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
function execute(executable, args, env) {
  // Forward termination and preserve the watchdog's process group.
  const child = cp.spawn(executable, args, { env, stdio: 'inherit', cwd: '/workspace' });
  child.on('error', () => { process.exitCode = 1; });
  child.on('exit', (code, signal) => { process.exitCode = signal ? 1 : code; });
  for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));
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
  const env = {
    ...(process.env.GOLEM_WRITABLE === '1' ? toolEnvironment() : targetEnv),
    CLAUDE_CONFIG_DIR: '/home/agent/.claude', DISABLE_AUTOUPDATER: '1',
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1',
    CLAUDE_CODE_OAUTH_TOKEN: process.env.CLAUDE_CODE_OAUTH_TOKEN,
  };
  // npm may provide either a JS launcher or a native executable. Resolve once
  // at image build, never through the target's PATH or mise configuration.
  const entry = '/opt/golem/claude-entry';
  const fd = fs.openSync(entry, 'r');
  const magic = Buffer.alloc(256);
  const count = fs.readSync(fd, magic, 0, magic.length, 0);
  fs.closeSync(fd);
  const javascript = magic.subarray(0, count).toString().startsWith('#!') && magic.subarray(0, count).toString().split('\n')[0].includes('node');
  execute(javascript ? '/usr/local/bin/node' : entry, javascript ? [entry, ...args] : args, env);
}
