#!/usr/bin/env node
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';

const org = 'isp-insoft-gmbh';
const ops = `${org}/isp-golem-xiv-ops`;
const prefix = 'isp-golem-xiv-test-';
const topic = 'isp-golem-xiv-test';
const app = 4605060;
const deadline = Date.now() + 14 * 60_000;
const phases = ['admitted', 'containers_destroyed', 'publication_credentials_minted', 'publication_complete', 'reservation_released'];
let stage = 'initialization';
function requireThat(condition, message) { if (!condition) throw new Error(message); }
async function gh(args, input, limit = 2_000_000, missing = false) {
  requireThat(Date.now() < deadline, 'overall deadline exceeded');
  return new Promise((resolve, reject) => {
    const child = spawn('gh', args, {stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true, timeout: Math.min(30_000, deadline - Date.now())});
    let size = 0, overflow = false;
    const chunks = [];
    child.stdout.on('data', chunk => { size += chunk.length; if (size > limit) { overflow = true; child.kill(); } else chunks.push(chunk); });
    child.on('error', () => reject(new Error('GitHub CLI unavailable')));
    child.on('close', code => {
      const text = Buffer.concat(chunks).toString('utf8');
      if (missing && code !== 0 && !overflow) {
        try { if (String(JSON.parse(text).status) === '404') { resolve(null); return; } } catch {}
      }
      if (code !== 0 || overflow) reject(new Error(overflow ? 'GitHub response exceeded limit' : 'GitHub request failed'));
      else resolve(text);
    });
    child.stdin.on('error', () => {});
    child.stdin.end(input === undefined ? undefined : JSON.stringify(input));
  });
}
async function api(path, method = 'GET', body, missing = false) {
  const text = await gh(['api', path, '--method', method, ...(body === undefined ? [] : ['--input', '-'])], body, 2_000_000, missing);
  if (text === null) return null;
  try { return JSON.parse(text); } catch { throw new Error('GitHub returned invalid JSON'); }
}
function decoded(file) {
  requireThat(file.encoding === 'base64' && typeof file.content === 'string' && file.content.length <= 1_000_000, 'invalid file response');
  return Buffer.from(file.content.replace(/\s/g, ''), 'base64').toString('utf8');
}
async function waitFor(fn, description) {
  while (Date.now() < deadline) { const value = await fn(); if (value) return value; await new Promise(resolve => setTimeout(resolve, 5000)); }
  throw new Error(`deadline waiting for ${description}`);
}
async function requestFor(check) {
  requireThat(/^[a-f0-9-]{36}$/i.test(check.external_id ?? ''), 'invalid request identity');
  const reference = await api(`repos/${ops}/git/ref/tags/golem-request/${check.external_id}`, 'GET', undefined, true);
  if (!reference) return null;
  const tag = await api(`repos/${ops}/git/tags/${reference.object.sha}`);
  requireThat(typeof tag.message === 'string' && tag.message.length <= 65_536, 'invalid request tag');
  return {request: JSON.parse(tag.message), tagSha: reference.object.sha};
}
async function main() {
  const args = process.argv.slice(2);
  let repo, commentId, explicitCheck, details;
  const observe = args.length === 4 && args[0] === '--observe' && args[2] === '--check';
  requireThat(args.length === 0 || observe, 'usage: node tools/prove-slice2-pilot.mjs [--observe <commentURL> --check <check-id>]');
  const me = await api('user');
  requireThat(me.login?.toLowerCase() === 'bugabinga', 'authenticated user must be bugabinga');
  if (observe) {
    const match = args[1].match(/^https:\/\/github\.com\/(isp-insoft-gmbh\/isp-golem-xiv-test-[a-z0-9-]+)\/issues\/(\d+)#issuecomment-(\d+)$/i);
    requireThat(match && /^[1-9]\d{0,15}$/.test(args[3]), 'invalid observation identity');
    repo = match[1]; commentId = Number(match[3]); explicitCheck = Number(args[3]);
    requireThat(Number.isSafeInteger(commentId) && Number.isSafeInteger(explicitCheck), 'unsafe observation identity');
  } else {
    stage = 'create private sandbox';
    repo = `${org}/${prefix}${Date.now().toString(36)}-${randomUUID().slice(0, 8)}`;
    await gh(['repo', 'create', repo, '--private', '--disable-wiki', '--add-readme']);
    console.log(`retained test repository: https://github.com/${repo}`);
    await api(`repos/${repo}/topics`, 'PUT', {names: [topic]});
  }
  details = await api(`repos/${repo}`);
  const topics = await api(`repos/${repo}/topics`);
  requireThat(details.private === true && details.full_name?.toLowerCase() === repo.toLowerCase() && details.owner?.login?.toLowerCase() === org && topics.names?.includes(topic), 'invalid private marked sandbox');
  repo = details.full_name;
  if (!observe) {
    stage = 'install manual definitions';
    const definitions = {
      '.golems/hello.md': '---\nowner: bugabinga\n---\nReply with a short greeting.\n',
      '.golems/pilot.md': '---\nowner: bugabinga\nharness: claude\nprovider: anthropic\nbranch: golems/pilot\nverify: test -s pilot.txt\ntimeout: 10m\n---\nWrite exactly the requested nonce to pilot.txt, with no other content. Do not commit, push, or open a pull request. The trusted runner handles publication.\n',
    };
    for (const [path, content] of Object.entries(definitions)) await api(`repos/${repo}/contents/${path}`, 'PUT', {message: 'Add manual pilot definition', content: Buffer.from(content).toString('base64'), branch: details.default_branch});
    const nonce = randomUUID();
    const issue = await api(`repos/${repo}/issues`, 'POST', {title: 'Slice 2 native pilot'});
    const comment = await api(`repos/${repo}/issues/${issue.number}/comments`, 'POST', {body: `@isp-golem-xiv pilot Write exactly this nonce to pilot.txt with no newline or other content: ${nonce}`});
    commentId = comment.id;
    console.log(`mention: ${comment.html_url}`);
  }
  stage = 'bind comment and immutable request';
  const comment = await api(`repos/${repo}/issues/comments/${commentId}`);
  const nonceMatch = comment.body?.match(/^@isp-golem-xiv pilot Write exactly this nonce to pilot\.txt with no newline or other content: ([a-f0-9-]{36})$/i);
  const issueMatch = comment.issue_url?.match(/\/issues\/(\d+)$/);
  requireThat(nonceMatch && issueMatch && comment.user?.id === me.id && comment.user?.login === me.login, 'comment identity or nonce invalid');
  const nonce = nonceMatch[1];
  const bound = await waitFor(async () => {
    const candidates = explicitCheck ? [await api(`repos/${repo}/check-runs/${explicitCheck}`)] : (await api(`repos/${repo}/commits/${encodeURIComponent(details.default_branch)}/check-runs?check_name=golem%2Fpilot&filter=all&per_page=100`)).check_runs;
    requireThat(Array.isArray(candidates) && candidates.length <= 100, 'unbounded check list');
    for (const check of candidates) {
      if (check.name !== 'golem/pilot' || check.app?.id !== app) continue;
      const stored = await requestFor(check);
      if (!stored) { if (explicitCheck) throw new Error('unresolved admission: immutable request tag missing'); continue; }
      const r = stored.request;
      if (r.comment_id !== commentId) { if (explicitCheck) throw new Error('check belongs to another comment'); continue; }
      requireThat(r.version === 2 && r.trigger === 'mention' && r.repository === repo && r.repository_id === details.id && r.delivery_id === check.external_id && r.check_run_id === check.id && r.head_sha === check.head_sha && r.issue_number === Number(issueMatch[1]) && r.actor_id === me.id && r.actor_login === me.login && r.seat_login === me.login && r.provider === 'anthropic' && r.branch === 'golems/pilot' && r.verify === 'test -s pilot.txt' && r.owner === me.login, 'immutable request identity mismatch');
      return check.status === 'completed' ? {check, ...stored} : null;
    }
    return null;
  }, 'matching completed pilot check');
  const {check, request, tagSha} = bound;
  const lines = (check.output?.summary ?? '').split('\n');
  const model = lines.find(line => line.startsWith('model: '))?.slice(7);
  const runMatch = lines.find(line => line.startsWith('log: '))?.match(/^log: https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/(\d+)$/);
  requireThat(check.conclusion === 'success' && lines.includes('outcome: changed') && lines.includes('seat: bugabinga/anthropic') && lines.includes('harness: claude') && lines.includes(`budget: ${request.timeout_seconds}s`) && model && model !== 'unobserved' && runMatch, 'check does not prove fresh changed outcome with attribution');
  const runId = Number(runMatch[1]);
  requireThat(Number.isSafeInteger(runId), 'unsafe run identity');
  stage = 'verify actual workflow and fresh publication';
  const run = await waitFor(async () => { const value = await api(`repos/${ops}/actions/runs/${runId}`); return value.status === 'completed' ? value : null; }, 'completed native workflow');
  requireThat(run.conclusion === 'success' && run.run_attempt === 1 && run.event === 'workflow_dispatch' && run.head_branch === 'trunk' && run.display_title === `${repo} pilot`, 'workflow identity invalid');
  const start = Date.parse(run.created_at) - 10_000, end = Date.parse(run.updated_at) + 10_000;
  requireThat(Number.isFinite(start) && Number.isFinite(end) && end >= start, 'workflow time window invalid');
  const pulls = await api(`repos/${repo}/pulls?state=open&head=${encodeURIComponent(`${org}:golems/pilot`)}&per_page=100`);
  requireThat(pulls.length === 1, 'expected exactly one owned pull request');
  const pull = pulls[0];
  requireThat(pull.user?.login === 'isp-golem-xiv[bot]' && pull.head.ref === 'golems/pilot' && pull.head.repo?.full_name === repo && pull.body?.split('\n').includes('seat: bugabinga/anthropic') && pull.body?.split('\n').includes(`model: ${model}`), 'pull request ownership or footer invalid');
  const commits = await api(`repos/${repo}/pulls/${pull.number}/commits?per_page=100`);
  requireThat(Array.isArray(commits) && commits.length < 100, 'unbounded publication history');
  const published = commits.filter(commit => { const date = Date.parse(commit.commit?.committer?.date); return date >= start && date <= end && commit.commit.message.includes(`<!-- isp-golem-xiv repository=${details.id} golem=pilot -->`) && commit.commit.message.split('\n').includes(`model: ${model}`) && commit.commit.message.split('\n').includes('seat: bugabinga/anthropic'); });
  requireThat(published.length === 1, 'no unique fresh attributed commit during this invocation');
  requireThat(decoded(await api(`repos/${repo}/contents/pilot.txt?ref=${published[0].sha}`)) === nonce, 'published content differs from requested nonce');
  stage = 'verify authoritative terminal and bound history';
  const current = JSON.parse(decoded(await api(`repos/${ops}/contents/queue.json?ref=golem-state`)));
  const currentRow = current.records?.find(row => row.id === request.delivery_id);
  if (currentRow) requireThat(currentRow.state === 'terminal' && currentRow.run_id === runId && currentRow.generation === request.queue_generation && currentRow.attempt === 1, 'current reservation is not matching terminal owner');
  else { const receipt = await api(`repos/${ops}/git/ref/tags/golem-terminal/${request.delivery_id}`); requireThat(receipt.object?.sha === tagSha, 'retired terminal receipt does not match immutable request'); }
  const history = await api(`repos/${ops}/commits?sha=golem-state&path=queue.json&since=${encodeURIComponent(new Date(start).toISOString())}&until=${encodeURIComponent(new Date(end).toISOString())}&per_page=100`);
  requireThat(Array.isArray(history) && history.length > 0 && history.length < 100, 'bounded lifecycle history unavailable');
  let terminal = false, historicalBind = false;
  for (const commit of history) {
    const snapshot = JSON.parse(decoded(await api(`repos/${ops}/contents/queue.json?ref=${commit.sha}`)));
    const entry = snapshot.records?.find(row => row.id === request.delivery_id && row.generation === request.queue_generation && row.run_id === runId && row.attempt === 1);
    if (entry?.state === 'terminal') terminal = true;
    if (terminal && entry?.state === 'bound') { historicalBind = true; break; }
  }
  requireThat(historicalBind, 'no matching bound-before-terminal journal history');
  stage = 'verify trusted native phase order';
  const log = await gh(['run', 'view', String(runId), '--repo', ops, '--log'], undefined, 8_000_000);
  const observed = log.split('\n').flatMap(line => { const match = line.match(/^runner\s+Run exactly one golem\s+\S+\s+golem lifecycle: (\w+)\s*$/); return match ? [match[1]] : []; });
  requireThat(JSON.stringify(observed) === JSON.stringify(phases), 'native lifecycle markers missing or out of order');
  console.log('verified actual GitHub/native/provider fresh publication');
  console.log(`mention: ${comment.html_url}`);
  console.log(`check: https://github.com/${repo}/runs/${check.id}`);
  console.log(`pull request: ${pull.html_url}`);
  console.log(`commit: https://github.com/${repo}/commit/${published[0].sha}`);
  console.log(`private workflow: https://github.com/${ops}/actions/runs/${runId}`);
  console.log(`delivery: ${request.delivery_id} run: ${runId} terminal: verified`);
  console.log(`native phases: ${observed.join(' -> ')}`);
}
main().catch(error => {
  const safe = /^(overall deadline|GitHub (CLI|response|request|returned)|usage:|authenticated user|invalid |unsafe |unbounded |comment |immutable |check |unresolved admission|deadline waiting|workflow |expected exactly|pull request |no unique|published content|current reservation|retired terminal|bounded lifecycle|no matching|native lifecycle)/;
  const message = typeof error?.message === 'string' && safe.test(error.message) ? error.message : 'operation failed';
  process.stderr.write(`pilot failed at ${stage}: ${message}\n`);
  process.exitCode = 1;
});
