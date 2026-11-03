#!/usr/bin/env node
import {createHmac, createSign, randomUUID} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';

const organization = 'isp-insoft-gmbh';
const appId = '4605060';
const appSlug = 'isp-golem-xiv';
const opsRepository = `${organization}/isp-golem-xiv-ops`;
const testTopic = 'isp-golem-xiv-test';
const testPrefix = 'isp-golem-xiv-test-';
const router = 'https://isp-golem-xiv.isp-insoft-gmbh.workers.dev';
const webhook = `${router}/webhook`;
const seats = `${router}/seats`;
const secretDirectory = `${process.env.USERPROFILE ?? process.env.HOME}/.secrets/isp-golem-xiv`;
const webhookSecretPath = `${secretDirectory}/webhook.secret`;
const appPrivateKeyPath = `${secretDirectory}/app.private-key.pem`;
const refusalTimeoutMs = 120_000;
const deliveryTimeoutMs = 60_000;
const pollMs = 2_000;

function redact(value) {
  return String(value)
    .replaceAll(secretDirectory, '~/.secrets/isp-golem-xiv')
    .replace(/-----BEGIN [^-]+-----[\s\S]*?-----END [^-]+-----/g, '[redacted key]')
    .replace(/\b(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|sk-[A-Za-z0-9_-]+)\b/g, '[redacted token]')
    .replace(/\b[a-f0-9]{64}\b/gi, '[redacted digest]');
}

function fail(message) {
  throw new Error(message);
}

async function gh(args, {json = false} = {}) {
  const result = await new Promise((resolve, reject) => {
    const child = spawn('gh', args, {stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, timeout: 120_000});
    const stdout = [];
    let bytes = 0;
    child.stdout.on('data', chunk => {
      bytes += chunk.length;
      if (bytes > 4_194_304) child.kill();
      else stdout.push(chunk);
    });
    child.stderr.resume();
    child.on('error', reject);
    child.on('close', code => resolve({code, stdout: Buffer.concat(stdout).toString('utf8')}));
  });
  if (result.code !== 0) fail('GitHub CLI request failed');
  if (!json) return result.stdout.trim();
  try {
    return JSON.parse(result.stdout);
  } catch {
    fail('GitHub CLI returned invalid JSON');
  }
}

function api(path, options = {}) {
  const args = ['api', path, '--silent'];
  if (options.method) args.push('--method', options.method);
  for (const [name, value] of Object.entries(options.fields ?? {})) args.push('--raw-field', `${name}=${value}`);
  if (options.paginate) args.push('--paginate');
  if (options.json) args.push('--include');
  return gh(args, {json: options.json});
}

async function apiJson(path, options = {}) {
  const text = await gh([
    'api', path,
    ...(options.method ? ['--method', options.method] : []),
    ...Object.entries(options.fields ?? {}).flatMap(([name, value]) => ['--raw-field', `${name}=${value}`]),
    ...(options.paginate ? ['--paginate'] : []),
  ]);
  try {
    return JSON.parse(text);
  } catch {
    fail('GitHub API returned invalid JSON');
  }
}

function seatPrefix(login) {
  return `SEAT_${login.toUpperCase().replaceAll('-', '_')}__`;
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function until(timeoutMs, action, description) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await action();
    if (value !== null && value !== undefined) return value;
    await sleep(pollMs);
  }
  fail(`Timed out waiting for ${description}`);
}

function appJwt(privateKey) {
  const encode = value => Buffer.from(JSON.stringify(value)).toString('base64url');
  const now = Math.floor(Date.now() / 1000);
  const unsigned = `${encode({alg: 'RS256', typ: 'JWT'})}.${encode({iat: now - 60, exp: now + 540, iss: appId})}`;
  const signer = createSign('RSA-SHA256');
  signer.update(unsigned);
  signer.end();
  return `${unsigned}.${signer.sign(privateKey).toString('base64url')}`;
}

async function appGet(path, privateKey, preserveDeliveryIds = false) {
  const response = await fetch(`https://api.github.com${path}`, {
    signal: AbortSignal.timeout(30_000),
    headers: {
      Accept: 'application/vnd.github+json',
      Authorization: `Bearer ${appJwt(privateKey)}`,
      'User-Agent': 'isp-golem-xiv-prove-refusals',
      'X-GitHub-Api-Version': '2022-11-28',
    },
  });
  if (!response.ok) fail(`GitHub App API answered ${response.status}`);
  try {
    if (!preserveDeliveryIds) return await response.json();
    const text = await response.text();
    return JSON.parse(text.replaceAll(/"id":(\d{16,})/g, '"id":"$1"'));
  } catch {
    fail('GitHub App API returned invalid JSON');
  }
}

async function findCapturedDelivery(privateKey, repository, commentId) {
  return until(deliveryTimeoutMs, async () => {
    const deliveries = await appGet('/app/hook/deliveries?per_page=100', privateKey, true);
    for (const delivery of deliveries) {
      if (delivery.event !== 'issue_comment') continue;
      const detail = await appGet(`/app/hook/deliveries/${delivery.id}`, privateKey);
      const payload = detail.request?.payload;
      if (payload?.repository?.full_name === repository && payload?.comment?.id === commentId) return payload;
    }
    return null;
  }, 'the App webhook delivery');
}

async function user(login) {
  return apiJson(`users/${encodeURIComponent(login)}`);
}

async function memberWithoutSeat() {
  const [members, secretList] = await Promise.all([
    apiJson(`orgs/${organization}/members?per_page=100`),
    apiJson(`repos/${opsRepository}/actions/secrets?per_page=100`),
  ]);
  const secrets = new Set(secretList.secrets.map(secret => secret.name));
  for (const member of members) {
    if (![...secrets].some(name => name.startsWith(seatPrefix(member.login)))) return user(member.login);
  }
  fail('No organization member without an ops seat secret is available');
}

async function verifyNonmember(login) {
  const response = await new Promise((resolve, reject) => {
    const child = spawn('gh', ['api', `orgs/${organization}/memberships/${login}`, '--silent'], {stdio: ['ignore', 'ignore', 'ignore'], windowsHide: true, timeout: 30_000});
    child.on('error', reject);
    child.on('close', code => resolve(code));
  });
  if (response === 0) fail(`${login} is an organization member`);
  return user(login);
}

function cloneForActor(template, actor) {
  const delivery = structuredClone(template);
  delivery.sender = {id: actor.id, login: actor.login, type: actor.type};
  delivery.comment.user = {id: actor.id, login: actor.login, type: actor.type};
  delivery.comment.body = `@${appSlug} hello`;
  return delivery;
}

async function deliver(payload, secret) {
  const body = JSON.stringify(payload);
  const signature = createHmac('sha256', secret).update(body).digest('hex');
  const response = await fetch(webhook, {
    signal: AbortSignal.timeout(30_000),
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-GitHub-Event': 'issue_comment',
      'X-GitHub-Delivery': randomUUID(),
      'X-Hub-Signature-256': `sha256=${signature}`,
    },
    body,
  });
  if (response.status !== 202) fail(`Webhook answered ${response.status}`);
}

async function issueComments(repository, issue) {
  return apiJson(`repos/${repository}/issues/${issue}/comments?per_page=100`);
}

async function waitForRefusal(repository, issue, reason) {
  const marker = `<!-- isp-golem-xiv kind=refused reason=${reason} -->`;
  return until(refusalTimeoutMs, async () => {
    const comments = await issueComments(repository, issue);
    return comments.find(comment => comment.body?.startsWith(marker)) ?? null;
  }, `${reason} refusal`);
}

async function repositoryForResume(repository) {
  if (!repository.startsWith(`${organization}/${testPrefix}`)) fail('Resume repository lacks the required test prefix');
  const [details, topics] = await Promise.all([
    apiJson(`repos/${repository}`),
    apiJson(`repos/${repository}/topics`),
  ]);
  if (!details.private) fail('Resume repository is not private');
  if (!topics.names.includes(testTopic)) fail('Resume repository lacks the required test topic');
  return repository;
}

async function existingRefusalEvidence(repository) {
  const issues = await apiJson(`repos/${repository}/issues?state=open&per_page=100`);
  const issue = issues.find(candidate => candidate.title === 'Refusal evidence');
  if (!issue) fail('Resume repository has no refusal evidence issue');
  const comments = await issueComments(repository, issue.number);
  const templateComment = comments.find(comment => comment.body === `@${appSlug} template`);
  if (!templateComment) fail('Resume repository has no template comment');
  return {issue, templateComment};
}

async function matchingRuns(repository) {
  const runs = await apiJson(`repos/${opsRepository}/actions/workflows/runner.yml/runs?per_page=100`);
  return runs.workflow_runs.filter(run => run.display_title === `${repository} hello`);
}

async function noNewDispatch(repository, baseline) {
  await sleep(5_000);
  const after = await matchingRuns(repository);
  const old = new Set(baseline.map(run => run.id));
  if (after.some(run => !old.has(run.id))) fail('A refused delivery dispatched a runner workflow');
}

async function main() {
  const arguments_ = process.argv.slice(2);
  const keep = arguments_.filter(argument => argument === '--keep');
  const repositoryIndex = arguments_.indexOf('--repo');
  const resumeRepository = repositoryIndex === -1 ? null : arguments_[repositoryIndex + 1];
  const allowed = keep.length === 1 && (arguments_.length === 1 || (arguments_.length === 3 && repositoryIndex !== -1 && resumeRepository));
  if (!allowed) fail('Usage: node tools/prove-refusals.mjs --keep [--repo owner/name]');
  const [webhookSecret, appPrivateKey] = await Promise.all([
    readFile(webhookSecretPath, 'utf8').then(value => value.trim()),
    readFile(appPrivateKeyPath, 'utf8'),
  ]);
  if (!webhookSecret || !appPrivateKey) fail('Required local webhook or App key is empty');

  let repository;
  let issue;
  let templateComment;
  if (resumeRepository) {
    repository = await repositoryForResume(resumeRepository);
    ({issue, templateComment} = await existingRefusalEvidence(repository));
  } else {
    const suffix = `${Date.now().toString(36)}-${randomUUID().slice(0, 8)}`;
    repository = `${organization}/${testPrefix}${suffix}`;
    await gh(['repo', 'create', repository, '--private', '--disable-wiki', '--add-readme']);
    await api(`repos/${repository}/topics`, {method: 'PUT', fields: {'names[]': testTopic}});
    const hello = Buffer.from('---\n---\nReply with one short greeting.\n').toString('base64');
    await api(`repos/${repository}/contents/.golems/hello.md`, {
      method: 'PUT',
      fields: {message: 'Add hello golem', content: hello},
    });
    issue = await apiJson(`repos/${repository}/issues`, {method: 'POST', fields: {title: 'Refusal evidence'}});
    templateComment = await apiJson(`repos/${repository}/issues/${issue.number}/comments`, {
      method: 'POST',
      fields: {body: `@${appSlug} template`},
    });
  }

  const template = await findCapturedDelivery(appPrivateKey, repository, templateComment.id);
  const [withoutSeat, octocat] = await Promise.all([memberWithoutSeat(), verifyNonmember('octocat')]);
  const baseline = await matchingRuns(repository);

  await deliver(cloneForActor(template, withoutSeat), webhookSecret);
  const noSeat = await waitForRefusal(repository, issue.number, 'no-seat');
  if (!noSeat.body.includes(seats)) fail('No-seat refusal lacks the seats link');
  await noNewDispatch(repository, baseline);

  await deliver(cloneForActor(template, octocat), webhookSecret);
  const notMember = await waitForRefusal(repository, issue.number, 'not-member');
  await noNewDispatch(repository, baseline);

  console.log(`https://github.com/${repository}`);
  console.log(`no-seat actor: ${withoutSeat.login}`);
  console.log(`not-member actor: ${octocat.login}`);
  console.log(noSeat.html_url);
  console.log(notMember.html_url);
}

main().catch(error => {
  process.stderr.write(`prove-refusals: ${redact(error?.message ?? 'failed')}\n`);
  process.exitCode = 1;
});
