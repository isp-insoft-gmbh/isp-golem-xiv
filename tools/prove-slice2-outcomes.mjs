#!/usr/bin/env node
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';

// Operational evidence only: real GitHub, Cloudflare cron, released native runner.
// --setup deliberately creates neither issues, mentions nor workflow dispatches.
const org = 'isp-insoft-gmbh';
const ops = `${org}/isp-golem-xiv-ops`;
const prefix = 'isp-golem-xiv-test-';
const topic = 'isp-golem-xiv-test';
const deadline = Date.now() + 90 * 60_000;
let stage = 'arguments';
const files = {
  '.golems/scheduled.md': '---\nschedule: daily\nowner: bugabinga\n---\nChange nothing. There is nothing to do. Do not write a response comment or modify any file.\n',
  '.golems/event.md': '---\ntriggers: issues.opened\nowner: bugabinga\n---\nChange nothing. There is nothing to do. Do not write a response comment or modify any file.\n',
  '.golems/partial.md': '---\nowner: bugabinga\nbranch: golems/partial\nverify: test -s partial.txt\ntimeout: 2m\n---\nFollow the phase and nonce in the human mention. FIRST: immediately write exactly FIRST:<nonce> followed by a newline to partial.txt, close the file, then run sleep 300 in the foreground and wait for it. Write the file FIRST, before sleeping. Do not shorten, background, or interrupt the sleep; the trusted wall timeout must end this phase. SECOND: first read the existing partial.txt on this branch and preserve its entire FIRST line verbatim. Append exactly SECOND:<nonce> followed by a newline, then return immediately without sleeping. Do not commit, push, open a pull request, or change any other file. The trusted runner verifies and publishes.\n',
  'mise.toml': '[tools]\ngithub-cli = "2.103.3"\n',
  '.github/workflows/bot.yml': `name: Bot owner evidence
on:
  workflow_dispatch:
    inputs:
      nonce:
        required: true
        type: string
permissions:
  contents: read
  issues: write
jobs:
  bot:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          persist-credentials: false
      - uses: jdx/mise-action@f5be966e4a917d54244728e6a5fdcd1d93a35e6c
        with:
          version: 2026.10.21
      - name: Create actual bot issue
        env:
          GH_TOKEN: \${{ github.token }}
          GH_REPO: \${{ github.repository }}
          NONCE: \${{ inputs.nonce }}
        run: mise exec -- gh api "repos/$GH_REPO/issues" --method POST --raw-field "title=Slice 2 bot $NONCE" --raw-field "body=Private bot-owner evidence" --silent
`,
};
function need(ok, message) { if (!ok) throw new Error(message); }
function number(value) { need(Number.isSafeInteger(value) && value > 0, 'unsafe numeric identity'); return value; }
async function gh(args, body) {
  need(Date.now() < deadline, 'deadline exceeded');
  return new Promise((resolve, reject) => {
    const child = spawn('gh', args, {windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'], timeout: Math.min(30_000, deadline - Date.now())});
    const chunks = []; let size = 0, overflow = false;
    child.stdout.on('data', chunk => { size += chunk.length; if (size > 2_000_000) { overflow = true; child.kill(); } else chunks.push(chunk); });
    child.on('error', () => reject(new Error('GitHub CLI unavailable')));
    child.on('close', code => code === 0 && !overflow ? resolve(Buffer.concat(chunks).toString('utf8')) : reject(new Error('GitHub request failed or exceeded bounds')));
    child.stdin.on('error', () => {});
    child.stdin.end(body === undefined ? undefined : JSON.stringify(body));
  });
}
async function api(path, method = 'GET', body) {
  const result = await gh(['api', path, '--method', method, ...(body === undefined ? [] : ['--input', '-'])], body);
  if (!result.trim()) return null;
  try { return JSON.parse(result); } catch { throw new Error('invalid GitHub JSON'); }
}
function decoded(file) {
  need(file.encoding === 'base64' && typeof file.content === 'string' && file.content.length < 1_000_000, 'invalid file encoding or size');
  return Buffer.from(file.content.replace(/\s/g, ''), 'base64').toString('utf8');
}
async function poll(fn) {
  while (Date.now() < deadline) { const result = await fn(); if (result) return result; await new Promise(resolve => setTimeout(resolve, Math.min(10_000, Math.max(0, deadline - Date.now())))); }
  throw new Error('deadline exceeded');
}
async function validate(repo) {
  need(new RegExp(`^${org}/${prefix}[a-z0-9-]+$`).test(repo), 'unsafe repository name');
  const data = await api(`repos/${repo}`);
  const topics = await api(`repos/${repo}/topics`);
  need(data.private === true && data.full_name === repo && data.owner?.login === org && topics.names?.includes(topic), 'repository is not private and marked');
  return data;
}
async function definitions(repo, branch) {
  for (const [path, expected] of Object.entries(files)) need(decoded(await api(`repos/${repo}/contents/${path}?ref=${encodeURIComponent(branch)}`)) === expected, 'definition or workflow differs from expected');
  const tree = await api(`repos/${repo}/git/trees/${encodeURIComponent(branch)}?recursive=1`);
  need(!tree.truncated && Array.isArray(tree.tree), 'unbounded repository tree');
  const sensitive = tree.tree.filter(x => x.type === 'blob' && (x.path.startsWith('.golems/') || x.path.startsWith('.github/workflows/'))).map(x => x.path).sort();
  need(JSON.stringify(sensitive) === JSON.stringify(Object.keys(files).filter(x => x.startsWith('.')).sort()), 'unexpected definitions or workflows');
  return tree.sha;
}
async function request(repo, check) {
  need(check.app?.id === 4605060 && /^[a-f0-9-]{36}$/.test(check.external_id ?? ''), 'check identity invalid');
  const ref = await api(`repos/${ops}/git/ref/tags/golem-request/${check.external_id}`);
  need(ref.object?.type === 'tag' && /^[a-f0-9]{40}$/.test(ref.object.sha), 'immutable request tag missing');
  const tag = await api(`repos/${ops}/git/tags/${ref.object.sha}`);
  const req = JSON.parse(tag.message);
  need(req.version === 2 && req.delivery_id === check.external_id && req.repository === repo && req.check_run_id === number(check.id) && req.head_sha === check.head_sha && req.seat_login === 'bugabinga' && req.provider === 'anthropic' && req.owner === 'bugabinga', 'request context or owner seat invalid');
  need(/^[a-f0-9-]{36}$/.test(req.queue_generation ?? ''), 'request generation invalid');
  return {req, tag: ref.object.sha};
}
async function checks(repo, head, golem) {
  const page = await api(`repos/${repo}/commits/${head}/check-runs?check_name=golem%2F${golem}&filter=all&per_page=100`);
  need(Number.isSafeInteger(page.total_count) && page.total_count < 100 && Array.isArray(page.check_runs), 'check page exceeds bounds');
  return page.check_runs.filter(c => c.name === `golem/${golem}` && c.app?.id === 4605060);
}
async function audit(repo, check, outcome) {
  const {req, tag} = await request(repo, check);
  const summary = check.output?.summary?.split('\n') ?? [];
  const model = summary.find(x => x.startsWith('model: '))?.slice(7);
  const match = summary.find(x => x.startsWith('log: '))?.match(/^log: https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/(\d+)$/);
  need(summary.includes(`outcome: ${outcome}`) && summary.includes('seat: bugabinga/anthropic') && summary.includes('harness: claude') && summary.includes(`budget: ${req.timeout_seconds}s`) && model && !['default', 'unobserved'].includes(model) && match, 'native audit outcome or attribution invalid');
  need(check.status === 'completed' && check.conclusion === (outcome === 'incomplete' ? 'neutral' : 'success'), 'native check conclusion invalid');
  if (outcome === 'no_change') need(summary.some(x => /^reason: \S/.test(x)), 'no_change lacks reason');
  const runId = number(Number(match[1]));
  const run = await poll(async () => { const r = await api(`repos/${ops}/actions/runs/${runId}`); return r.status === 'completed' ? r : null; });
  need(run.id === runId && run.conclusion === 'success' && run.run_attempt === 1 && run.display_title === `${repo} ${req.golem}` && run.event === 'workflow_dispatch' && run.path === '.github/workflows/runner.yml', 'native workflow identity or outcome invalid');
  const queue = JSON.parse(decoded(await api(`repos/${ops}/contents/queue.json?ref=golem-state`)));
  need(queue.version === 2 && Array.isArray(queue.records) && queue.records.length <= 32, 'queue format invalid');
  const row = queue.records.find(r => r.id === req.delivery_id);
  if (row) need(row.tag === tag && row.generation === req.queue_generation && row.state === 'terminal' && row.run_id === runId && row.attempt === 1 && row.resource === `${repo}/${req.golem}` && row.seat === 'bugabinga/anthropic', 'terminal queue binding invalid');
  else need((await api(`repos/${ops}/git/ref/tags/golem-terminal/${req.delivery_id}`)).object?.sha === tag, 'terminal receipt missing');
  console.log(`${req.golem}: ${outcome}; https://github.com/${ops}/actions/runs/${runId}; https://github.com/${repo}/runs/${check.id}`);
  return {req, model, runId};
}
async function completed(repo, head, golem, predicate) {
  return poll(async () => {
    const matches = [];
    for (const check of await checks(repo, head, golem)) {
      const context = await request(repo, check);
      if (predicate(context.req)) matches.push(check);
    }
    need(matches.length <= 1, 'duplicate matching runs');
    return matches[0]?.status === 'completed' ? matches[0] : null;
  });
}
async function pull(repo, model) {
  const pulls = await api(`repos/${repo}/pulls?state=all&head=${encodeURIComponent(`${org}:golems/partial`)}&per_page=100`);
  need(Array.isArray(pulls) && pulls.length === 1, 'expected one continuation pull request');
  const p = pulls[0];
  need(p.state === 'open' && p.head.ref === 'golems/partial' && p.head.repo?.full_name === repo && p.user?.login === 'isp-golem-xiv[bot]' && p.body?.split('\n').includes('seat: bugabinga/anthropic') && p.body?.split('\n').includes(`model: ${model}`), 'pull ownership or footer invalid');
  return p;
}
async function noReplies(repo, issue, golem = 'event') {
  const comments = await api(`repos/${repo}/issues/${issue}/comments?per_page=100`);
  need(Array.isArray(comments) && comments.length < 100 && !comments.some(c => c.user?.login === 'isp-golem-xiv[bot]' && c.body?.startsWith(`<!-- isp-golem-xiv kind=reply golem=${golem} -->`)), 'unattended run wrote response comment');
}
async function prove(repo, data) {
  stage = 'validate exact sandbox definitions';
  await definitions(repo, data.default_branch);
  const head = (await api(`repos/${repo}/commits/${encodeURIComponent(data.default_branch)}`)).sha;
  const nonce = randomUUID();
  stage = 'dispatch actual GitHub Actions bot';
  await api(`repos/${repo}/actions/workflows/bot.yml/dispatches`, 'POST', {ref: data.default_branch, inputs: {nonce}});
  const botIssue = await poll(async () => {
    const issues = await api(`repos/${repo}/issues?state=all&per_page=100`);
    need(Array.isArray(issues) && issues.length < 100, 'issue page exceeds bounds');
    return issues.find(i => i.title === `Slice 2 bot ${nonce}`);
  });
  need(botIssue.user?.login === 'github-actions[bot]' && botIssue.user?.type === 'Bot', 'actual issue sender is not GitHub Actions bot');
  stage = 'human FIRST mention';
  const issue = await api(`repos/${repo}/issues`, 'POST', {title: `Slice 2 partial ${nonce}`});
  const first = await api(`repos/${repo}/issues/${number(issue.number)}/comments`, 'POST', {body: `@isp-golem-xiv partial FIRST ${nonce}`});
  need(first.user?.login === 'bugabinga' && first.user?.type === 'User', 'mention sender is not human owner');
  stage = 'bot owner native audit';
  const botCheck = await completed(repo, head, 'event', r => r.trigger === 'event' && r.issue_number === botIssue.number);
  const bot = await audit(repo, botCheck, 'no_change');
  need(bot.req.actor_id === number(botIssue.user.id) && bot.req.actor_login === 'github-actions[bot]' && bot.req.comment_id === null, 'bot request actor context invalid');
  await noReplies(repo, number(botIssue.number));
  console.log(`bot source: https://github.com/${repo}/issues/${botIssue.number}`);
  stage = 'verified incomplete publication';
  const firstCheck = await completed(repo, head, 'partial', r => r.trigger === 'mention' && r.comment_id === first.id);
  const firstAudit = await audit(repo, firstCheck, 'incomplete');
  need(firstAudit.req.timeout_seconds === 120 && firstAudit.req.verify === 'test -s partial.txt' && firstAudit.req.branch === 'golems/partial', 'partial request limits invalid');
  const p1 = await pull(repo, firstAudit.model);
  const initial = decoded(await api(`repos/${repo}/contents/partial.txt?ref=${p1.head.sha}`));
  need(initial === `FIRST:${nonce}\n`, 'FIRST verified branch file invalid');
  stage = 'human SECOND continuation';
  const second = await api(`repos/${repo}/issues/${issue.number}/comments`, 'POST', {body: `@isp-golem-xiv partial SECOND ${nonce}`});
  need(second.user?.login === 'bugabinga' && second.user?.type === 'User', 'continuation sender is not human owner');
  const secondCheck = await completed(repo, head, 'partial', r => r.trigger === 'mention' && r.comment_id === second.id);
  const secondAudit = await audit(repo, secondCheck, 'changed');
  const p2 = await pull(repo, secondAudit.model);
  need(p2.number === p1.number && p2.head.sha !== p1.head.sha, 'continuation replaced pull request or did not advance');
  const commit = await api(`repos/${repo}/git/commits/${p2.head.sha}`);
  need(commit.parents?.length === 1 && commit.parents[0].sha === p1.head.sha, 'continuation did not build on first commit');
  need(decoded(await api(`repos/${repo}/contents/partial.txt?ref=${p2.head.sha}`)) === `${initial}SECOND:${nonce}\n`, 'continuation lost FIRST or failed SECOND');
  console.log(`incomplete + continuation: https://github.com/${repo}/pull/${p2.number}`);
  stage = 'genuine Cloudflare scheduled run';
  const today = new Date().toISOString().slice(0, 10);
  const scheduled = await completed(repo, head, 'scheduled', r => r.trigger === 'schedule' && r.actor_id === null && r.actor_login === null && r.issue_number === null && r.comment_id === null);
  need(scheduled.created_at?.startsWith(today), 'scheduled run is not from current UTC day');
  await audit(repo, scheduled, 'no_change');
  // Schedule has no issue/thread. Verify no unsolicited response on any sandbox issue.
  const allIssues = await api(`repos/${repo}/issues?state=all&per_page=100`);
  need(Array.isArray(allIssues) && allIssues.length < 100, 'issue page exceeds bounds');
  for (const i of allIssues) if (!i.pull_request) {
    await noReplies(repo, number(i.number), 'scheduled');
    await noReplies(repo, number(i.number), 'event');
  }
  console.log('schedule-owner, bot-owner, incomplete + continuation: verified; private sandbox retained');
}
async function main() {
  const args = process.argv.slice(2);
  need((args.length === 1 && args[0] === '--setup') || (args.length === 3 && args[0] === '--prove' && args[1] === '--repo'), 'usage invalid');
  need((await api('user')).login === 'bugabinga', 'authenticated user is not bugabinga');
  if (args[0] === '--prove') { const data = await validate(args[2]); return prove(args[2], data); }
  stage = 'create private marked sandbox';
  const repo = `${org}/${prefix}outcomes-${Date.now().toString(36)}-${randomUUID().slice(0, 8)}`;
  await gh(['repo', 'create', repo, '--private', '--disable-wiki', '--add-readme']);
  console.log(`retained sandbox: https://github.com/${repo}`);
  await api(`repos/${repo}/topics`, 'PUT', {names: [topic]});
  const data = await validate(repo);
  stage = 'install definitions and inert bot workflow';
  for (const [path, content] of Object.entries(files)) await api(`repos/${repo}/contents/${path}`, 'PUT', {message: 'Set up private Slice 2 evidence', content: Buffer.from(content).toString('base64'), branch: data.default_branch});
  await definitions(repo, data.default_branch);
  console.log('setup verified: private + prefix + topic + exact definitions; no workflow dispatches or mentions; retained for parent proof');
}
main().catch(() => {
  // Never relay API payloads, subprocess output, arbitrary exceptions, or native logs.
  process.stderr.write(`outcomes failed at ${stage}; details suppressed for private-target safety\n`);
  process.exitCode = 1;
});
