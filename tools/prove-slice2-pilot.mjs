#!/usr/bin/env node
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';

// Actual GitHub / native runner / provider integration, never a mock.
const org = 'isp-insoft-gmbh';
const ops = `${org}/isp-golem-xiv-ops`;
const prefix = 'isp-golem-xiv-test-';
const topic = 'isp-golem-xiv-test';
const deadline = Date.now() + 14 * 60_000;
const phases = ['admitted', 'containers_destroyed', 'publication_credentials_minted', 'publication_complete', 'reservation_released'];
let stage = 'initialization';

function requireThat(condition, message) { if (!condition) throw new Error(message); }
async function gh(args, input, limit = 2_000_000) {
  const remaining = deadline - Date.now();
  requireThat(remaining > 0, 'overall deadline exceeded');
  return new Promise((resolve, reject) => {
    const child = spawn('gh', args, {stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true, timeout: Math.min(30_000, remaining)});
    let size = 0, overflow = false;
    const chunks = [];
    child.stdout.on('data', chunk => {
      size += chunk.length;
      if (size > limit) { overflow = true; child.kill(); } else chunks.push(chunk);
    });
    child.on('error', () => reject(new Error('GitHub CLI unavailable')));
    child.on('close', code => {
      if (code !== 0 || overflow) reject(new Error(overflow ? 'GitHub response exceeded limit' : 'GitHub request failed or timed out'));
      else resolve(Buffer.concat(chunks).toString('utf8'));
    });
    child.stdin.on('error', () => {});
    child.stdin.end(input === undefined ? undefined : JSON.stringify(input));
  });
}
async function api(path, method = 'GET', body) {
  const text = await gh(['api', path, '--method', method, ...(body === undefined ? [] : ['--input', '-'])], body);
  try { return JSON.parse(text); } catch { throw new Error('GitHub returned invalid JSON'); }
}
function decoded(file) {
  requireThat(file.encoding === 'base64' && typeof file.content === 'string' && file.content.length <= 1_000_000, 'invalid or oversized file response');
  return Buffer.from(file.content.replace(/\s/g, ''), 'base64').toString('utf8');
}
async function pause() { await new Promise(resolve => setTimeout(resolve, Math.min(5000, Math.max(0, deadline - Date.now())))); }
async function waitFor(fn, description) {
  while (Date.now() < deadline) { const value = await fn(); if (value) return value; await pause(); }
  throw new Error(`deadline waiting for ${description}`);
}
async function main() {
  const args = process.argv.slice(2);
  requireThat(args.length === 0 || (args.length === 2 && args[0] === '--repo'), 'usage: node tools/prove-slice2-pilot.mjs [--repo owner/name]');
  const me = await api('user');
  requireThat(me.login?.toLowerCase() === 'bugabinga', 'authenticated GitHub user must be bugabinga');
  let repo = args[1];
  if (!repo) {
    repo = `${org}/${prefix}${Date.now().toString(36)}-${randomUUID().slice(0, 8)}`;
    stage = 'create private test repository';
    await gh(['repo', 'create', repo, '--private', '--disable-wiki', '--add-readme']);
    console.log(`retained test repository: https://github.com/${repo}`);
    await api(`repos/${repo}/topics`, 'PUT', {names: [topic]});
  }
  requireThat(new RegExp(`^${org}/${prefix}[a-z0-9-]+$`, 'i').test(repo), 'repository organization or prefix invalid');
  stage = 'verify test repository';
  const details = await api(`repos/${repo}`);
  const topics = await api(`repos/${repo}/topics`);
  requireThat(details.private === true && details.full_name?.toLowerCase() === repo.toLowerCase() && details.owner?.login?.toLowerCase() === org && topics.names?.includes(topic), 'repository is not a private marked organization test repository');
  repo = details.full_name;
  console.log(`retained test repository: https://github.com/${repo}`);
  // Refuse to overwrite existing definitions or an existing owned branch on resume.
  const tree = await api(`repos/${repo}/git/trees/${encodeURIComponent(details.default_branch)}?recursive=1`);
  requireThat(!tree.truncated && Array.isArray(tree.tree) && !tree.tree.some(x => ['.golems/hello.md', '.golems/pilot.md', 'pilot.txt'].includes(x.path)), 'resume repository already contains pilot files');
  const branches = await api(`repos/${repo}/branches?per_page=100`);
  requireThat(Array.isArray(branches) && branches.length < 100 && !branches.some(b => b.name === 'golems/pilot'), 'existing or unbounded branches prevent safe setup');
  stage = 'install manual definitions';
  const definitions = {
    '.golems/hello.md': '---\nowner: bugabinga\n---\nReply with a short greeting.\n',
    '.golems/pilot.md': '---\nowner: bugabinga\nharness: claude\nprovider: anthropic\nbranch: golems/pilot\nverify: test -s pilot.txt\ntimeout: 10m\n---\nWrite exactly the requested nonce to pilot.txt, with no other content. Do not commit, push, or open a pull request. The trusted runner handles publication.\n',
  };
  for (const [path, content] of Object.entries(definitions)) await api(`repos/${repo}/contents/${path}`, 'PUT', {message: 'Add manual pilot definition', content: Buffer.from(content).toString('base64'), branch: details.default_branch});
  const head = (await api(`repos/${repo}/commits/${encodeURIComponent(details.default_branch)}`)).sha;
  const nonce = randomUUID();
  stage = 'create issue and real mention';
  const issue = await api(`repos/${repo}/issues`, 'POST', {title: 'Slice 2 native pilot'});
  const comment = await api(`repos/${repo}/issues/${issue.number}/comments`, 'POST', {body: `@isp-golem-xiv pilot Write exactly this nonce to pilot.txt with no newline or other content: ${nonce}`});
  console.log(`mention: ${comment.html_url}`);
  stage = 'wait for check';
  const check = await waitFor(async () => {
    const result = await api(`repos/${repo}/commits/${head}/check-runs?check_name=golem%2Fpilot&per_page=100`);
    const matches = result.check_runs?.filter(c => c.name === 'golem/pilot' && c.app?.id === 4605060 && Date.parse(c.created_at) >= Date.parse(comment.created_at) - 5000) ?? [];
    requireThat(matches.length <= 1, 'multiple pilot checks found');
    return matches[0]?.status === 'completed' ? matches[0] : null;
  }, 'completed pilot check');
  requireThat(check.conclusion === 'success' && /^[0-9a-f-]{36}$/i.test(check.external_id ?? ''), 'check outcome or request ID invalid');
  const lines = (check.output?.summary ?? '').split('\n');
  const model = lines.find(x => x.startsWith('model: '))?.slice(7);
  const logLine = lines.find(x => x.startsWith('log: '));
  const runMatch = logLine?.match(/^log: https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/(\d+)$/);
  requireThat(lines.includes('outcome: changed') && lines.includes('seat: bugabinga/anthropic') && lines.includes('harness: claude') && model && model !== 'unobserved' && runMatch, 'check lacks changed outcome, seat, observed model or run link');
  const runId = Number(runMatch[1]);
  requireThat(Number.isSafeInteger(runId), 'invalid run ID');
  stage = 'verify workflow';
  const run = await waitFor(async () => { const r = await api(`repos/${ops}/actions/runs/${runId}`); return r.status === 'completed' ? r : null; }, 'completed workflow');
  requireThat(run.conclusion === 'success' && run.run_attempt === 1 && run.display_title === `${repo} pilot`, 'workflow identity or conclusion invalid');
  stage = 'verify pull request and file';
  const pulls = await api(`repos/${repo}/pulls?state=open&head=${encodeURIComponent(`${org}:golems/pilot`)}&per_page=100`);
  requireThat(pulls.length === 1, 'expected exactly one owned branch pull request');
  const pull = pulls[0];
  requireThat(pull.head.ref === 'golems/pilot' && pull.head.repo?.full_name?.toLowerCase() === repo.toLowerCase() && pull.user?.login === 'isp-golem-xiv[bot]' && pull.body?.split('\n').includes('seat: bugabinga/anthropic') && pull.body?.split('\n').includes(`model: ${model}`), 'pull request ownership or footer invalid');
  const file = await api(`repos/${repo}/contents/pilot.txt?ref=${encodeURIComponent(pull.head.sha)}`);
  requireThat(decoded(file) === nonce, 'published pilot file does not exactly match nonce');
  stage = 'verify authoritative queue';
  const current = JSON.parse(decoded(await api(`repos/${ops}/contents/queue.json?ref=golem-state`)));
  const row = current.records?.find(r => r.id === check.external_id);
  requireThat(row?.state === 'terminal' && row.run_id === runId && row.attempt === 1 && row.resource === `${repo.toLowerCase()}/pilot` && row.seat === 'bugabinga/anthropic' && /^[0-9a-f-]{36}$/i.test(row.generation), 'current journal lacks matching terminal reservation');
  stage = 'verify historical binding';
  const commits = await api(`repos/${ops}/commits?sha=golem-state&path=queue.json&per_page=30`);
  requireThat(Array.isArray(commits) && commits.length > 0 && commits.length <= 30, 'queue history unavailable');
  let sawTerminal = false, bound = false;
  for (const commit of commits) {
    const history = JSON.parse(decoded(await api(`repos/${ops}/contents/queue.json?ref=${commit.sha}`)));
    const entry = history.records?.find(r => r.id === row.id);
    if (entry?.state === 'terminal' && entry.generation === row.generation && entry.run_id === runId) sawTerminal = true;
    if (sawTerminal && entry?.state === 'bound' && entry.generation === row.generation && entry.run_id === runId && entry.attempt === 1) { bound = true; break; }
  }
  requireThat(bound, 'no bound revision before terminal within newest 30 queue commits');
  stage = 'verify private native lifecycle log';
  const log = await gh(['run', 'view', String(runId), '--repo', ops, '--log'], undefined, 8_000_000);
  const observed = [];
  for (const line of log.split('\n')) {
    const match = line.match(/^runner\s+Run exactly one golem\s+\S+\s+golem lifecycle: (admitted|containers_destroyed|publication_credentials_minted|publication_complete|reservation_released)\s*$/);
    if (match) observed.push(match[1]);
  }
  requireThat(JSON.stringify(observed) === JSON.stringify(phases), 'trusted native lifecycle markers missing, duplicated or out of order');
  console.log('actual GitHub / native runner / provider integration: changed');
  console.log(`check: https://github.com/${repo}/runs/${check.id}`);
  console.log(`pull request: ${pull.html_url}`);
  console.log(`private workflow log: https://github.com/${ops}/actions/runs/${runId}`);
  console.log(`queue: https://github.com/${ops}/commits/golem-state/queue.json`);
  console.log(`request: ${row.id} generation: ${row.generation} run: ${runId} check: ${check.id}`);
  console.log(`native phases: ${observed.join(' -> ')}`);
}
main().catch(error => {
  // Never relay subprocess output, API payloads, log lines, or arbitrary exception text.
  const allowed = /^(usage:|authenticated GitHub|repository |resume repository|existing |GitHub |overall deadline|deadline waiting|multiple pilot|check |workflow |expected exactly|pull request |published pilot|current journal|queue history|no bound revision|trusted native|invalid or oversized|invalid run)/;
  const message = typeof error?.message === 'string' && allowed.test(error.message) ? error.message : 'operation failed';
  process.stderr.write(`pilot failed at ${stage}: ${message}\n`);
  process.exitCode = 1;
});
