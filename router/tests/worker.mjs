// Executes the delivered worker-build artifact in workerd via Miniflare 5.
// Usage: node router/tests/worker.mjs <miniflare module path> <worker-build output>
// GitHub is a stateful HTTP double; this proves runtime wiring, not live GitHub behavior.
import assert from 'node:assert/strict';
import {createHash, createHmac, generateKeyPairSync} from 'node:crypto';
import {readdir, readFile} from 'node:fs/promises';
import {join, relative, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {setTimeout as sleep} from 'node:timers/promises';

const {Miniflare, Response: RuntimeResponse} = await import(pathToFileURL(process.argv[2]).href);
const ResponseType = RuntimeResponse ?? Response;
const artifact = resolve(process.argv[3]);
const modules = {};
for (const file of await readdir(artifact, {recursive: true, withFileTypes: true})) {
  if (!file.isFile()) continue;
  const path = join(file.parentPath, file.name);
  const name = relative(artifact, path).replaceAll('\\', '/');
  if (name.endsWith('.wasm')) modules[name] = {type: 'wasm', contents: new Uint8Array(await readFile(path))};
  else if (name.endsWith('.js') || name.endsWith('.mjs')) modules[name] = {type: 'esm', contents: await readFile(path, 'utf8')};
}
assert.ok(modules['index.js'], 'worker-build entry missing');
const repository = 'isp-insoft-gmbh/isp-golem-xiv-test-fixture';
const ops = 'isp-insoft-gmbh/isp-golem-xiv-ops';
const head = 'a'.repeat(40);
const privateKey = generateKeyPairSync('rsa', {modulusLength: 2048}).privateKey.export({type:'pkcs1', format:'pem'});
const secret = 'fixture-webhook-secret';
const files = new Map([
  ['hello.md', '---\n---\nRead the private fixture repository.'],
  ['broken.md', '---\nschedule: someday\n---\nInvalid schedule.'],
  ['stop.md', '---\n---\nReserved name.'],
]);
const state = {calls: 0, dispatches: [], checks: [], comments: [], reactions: [], refs: new Map(), tags: new Map(), tagId: 1};
let allowed = true;
let dispatchStatus = 204;
const answer = (status, body) => new ResponseType(body === null ? null : JSON.stringify(body), {status, headers: {'Content-Type':'application/json'}});
async function github(request) {
  state.calls++;
  const url = new URL(request.url);
  assert.equal(url.origin, 'https://api.github.com');
  const path = url.pathname;
  const method = request.method;
  const body = method === 'POST' ? await request.json() : null;
  if (path === '/user') {
    if (request.headers.get('Authorization') === 'Bearer redirect-fixture-token') return new ResponseType('{}', {status:302, headers:{Location:'https://must-not-receive-token.invalid/'}});
    return answer(request.headers.get('Authorization') === 'Bearer valid-fixture-token' ? 200 : 401, {login:'member'});
  }
  if (path === '/app/installations/153964219/access_tokens') return answer(201, {token:'fixture-installation-token'});
  if (path === `/repos/${repository}`) return answer(200, {id:99, full_name:repository, default_branch:'trunk', topics:allowed ? ['isp-golem-xiv-test'] : []});
  if (path === `/repos/${repository}/commits/trunk`) return answer(200, {sha:head});
  if (path === `/repos/${repository}/contents/.golems`) return answer(200, [...files].map(([name, content]) => ({name, type:'file', size:Buffer.byteLength(content)})));
  if (path.startsWith(`/repos/${repository}/contents/.golems/`)) {
    const content = files.get(path.split('/').at(-1));
    return content === undefined ? answer(404, {}) : answer(200, {type:'file', encoding:'base64', content:Buffer.from(content).toString('base64')});
  }
  if (path === `/repos/${ops}/git/ref/heads/trunk`) return answer(200, {object:{sha:head, type:'commit'}});
  if (path === `/repos/${ops}/git/tags` && method === 'POST') {
    const sha = String(state.tagId++).padStart(40, '0');
    state.tags.set(sha, {...body, sha});
    return answer(201, {sha});
  }
  if (path === `/repos/${ops}/git/refs` && method === 'POST') {
    if (state.refs.has(body.ref)) return answer(422, {message:'Reference already exists'});
    state.refs.set(body.ref, body.sha);
    return answer(201, {ref:body.ref, object:{sha:body.sha, type:'tag'}});
  }
  if (path.startsWith(`/repos/${ops}/git/ref/tags/`)) {
    const ref = 'refs/' + path.slice(`/repos/${ops}/git/ref/`.length);
    const sha = state.refs.get(ref);
    return sha ? answer(200, {ref, object:{sha, type:'tag'}}) : answer(404, {});
  }
  if (path.startsWith(`/repos/${ops}/git/tags/`)) return answer(200, state.tags.get(path.split('/').at(-1)));
  if (path.startsWith('/orgs/isp-insoft-gmbh/memberships/')) return path.endsWith('/outsider') ? answer(404, {}) : answer(200, {state:'active'});
  if (path.startsWith(`/repos/${ops}/actions/secrets/`)) return answer(path.endsWith('SEAT_MEMBER__ANTHROPIC') ? 200 : 404, {});
  if (path.endsWith('/reactions')) {state.reactions.push({path, ...body}); return answer(201, {id:1});}
  if (path.endsWith('/check-runs')) {state.checks.push(body); return answer(201, {id:state.checks.length});}
  if (path === `/repos/${ops}/actions/workflows/runner.yml/dispatches`) {state.dispatches.push(body); return answer(dispatchStatus, dispatchStatus === 204 ? null : {});}
  if (path === `/repos/${repository}/issues/1/comments`) {state.comments.push(body.body); return answer(201, {id:state.comments.length});}
  if (path.startsWith('/repos/')) return answer(404, {});
  throw new Error(`Unexpected fixture API: ${method} ${path}`);
}
const worker = new Miniflare({workers:[{
  config:{name:'router-fixture', compatibilityDate:'2026-10-06', manifest:{mainModule:'index.js', modules}, env:{
    VERSION:{type:'text', value:'fixture-version'},
    APP_PRIVATE_KEY:{type:'text', value:privateKey},
    WEBHOOK_SECRET:{type:'text', value:secret},
  }},
  dev:{outboundService:{type:'fetcher', handler:github}},
}]});
const base = 'https://router.invalid';
async function eventually(condition, description) {
  for (let attempt = 0; attempt < 200; attempt++) {if (condition()) return; await sleep(25);}
  assert.fail(`Timed out: ${description}`);
}
function payload(login = 'member', command = '@isp-golem-xiv hello say fixture\n\n  Keep this indentation.\r\n@isp-golem-xiv other\n') {
  return {action:'created', sender:{id:42, login, type:'User'}, repository:{id:99, full_name:repository}, installation:{id:153964219}, issue:{number:1}, comment:{id:100, body:command, user:{id:42, login, type:'User'}}};
}
function deliveryId(name) {
  const hex = createHash('sha256').update(name).digest('hex');
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20,32)}`;
}
async function deliver(id, value, signatureValid = true, malformedId = false) {
  const body = JSON.stringify(value);
  const signature = createHmac('sha256', signatureValid ? secret : 'wrong-fixture-secret').update(body).digest('hex');
  return worker.dispatchFetch(`${base}/webhook`, {method:'POST', body, headers:{'Content-Type':'application/json', 'X-GitHub-Event':'issue_comment', 'X-GitHub-Delivery':malformedId ? id : deliveryId(id), 'X-Hub-Signature-256':`sha256=${signature}`}});
}
try {
  const health = await worker.dispatchFetch(`${base}/health`);
  assert.equal(health.status, 200);
  assert.deepEqual(await health.json(), {app:'isp-golem-xiv', version:'fixture-version'});
  const index = `${base}/v1/repos/${repository}/golems`;
  assert.equal((await worker.dispatchFetch(index)).status, 401);
  assert.equal((await worker.dispatchFetch(index, {headers:{Authorization:'Bearer invalid-fixture-token'}})).status, 401);
  assert.equal((await worker.dispatchFetch(index, {headers:{Authorization:'Bearer redirect-fixture-token'}})).status, 403);
  const auth = {headers:{Authorization:'Bearer valid-fixture-token'}};
  let response = await worker.dispatchFetch(index, auth);
  assert.equal(response.status, 200);
  assert.match(response.headers.get('Cache-Control'), /no-store/);
  let golems = (await response.json()).golems;
  assert.deepEqual(golems.find(g => g.name === 'hello').errors, []);
  assert.ok(golems.find(g => g.name === 'broken').errors.length > 0);
  assert.ok(golems.find(g => g.name === 'stop').errors.length > 0);
  files.set('late.md', '---\n---\nAdded after first query.');
  golems = (await (await worker.dispatchFetch(index, auth)).json()).golems;
  assert.ok(golems.some(g => g.name === 'late'));
  for (let index = 0; index < 21; index++) files.set(`limit-${index}.md`, '---\n---\nTask');
  assert.equal((await worker.dispatchFetch(index, auth)).status, 502);
  for (let index = 0; index < 21; index++) files.delete(`limit-${index}.md`);
  assert.equal((await worker.dispatchFetch(`${base}/v1/repos/isp-insoft-gmbh/absent/golems`, auth)).status, 404);
  const calls = state.calls;
  assert.equal((await deliver('malformed-id', payload(), true, true)).status, 400);
  assert.equal(state.calls, calls);
  assert.equal((await deliver('invalid-payload', {})).status, 400);
  assert.equal((await deliver('oversized-body', payload('member', 'x'.repeat(262144)))).status, 413);
  assert.equal((await deliver('invalid-signature', payload(), false)).status, 401);
  assert.equal(state.calls, calls);
  const bot = payload('isp-golem-xiv[bot]'); bot.sender.type = bot.comment.user.type = 'Bot';
  assert.equal((await deliver('app-origin', bot)).status, 200);
  assert.equal((await deliver('later-mention', payload('member', 'Text\n@isp-golem-xiv hello'))).status, 200);
  assert.equal(state.calls, calls);
  allowed = false;
  assert.equal((await deliver('not-allowlisted', payload())).status, 202);
  await sleep(100);
  assert.equal(state.refs.size, 0);
  allowed = true;
  assert.equal((await deliver('non-member', payload('outsider'))).status, 202);
  await eventually(() => state.comments.some(body => body.startsWith('<!-- isp-golem-xiv kind=refused reason=not-member -->')), 'non-member refusal');
  assert.equal((await deliver('no-seat', payload('noseat'))).status, 202);
  await eventually(() => state.comments.some(body => body.startsWith('<!-- isp-golem-xiv kind=refused reason=no-seat -->') && body.includes('/seats')), 'no-seat refusal');
  assert.equal((await deliver('unknown', payload('member', '@isp-golem-xiv missing'))).status, 202);
  await eventually(() => state.comments.some(body => body.startsWith('<!-- isp-golem-xiv kind=refused reason=unknown-golem -->') && body.includes('hello')), 'unknown golem refusal');
  assert.equal(state.dispatches.length, 0);
  const refusalCount = state.comments.length;
  assert.equal((await deliver('bare-first-line', payload('member', '@isp-golem-xiv\nhello say this'))).status, 202);
  await eventually(() => state.comments.length > refusalCount, 'bare first line remains a bare mention');
  assert.ok(state.comments.at(-1).startsWith('<!-- isp-golem-xiv kind=refused reason=unknown-golem -->'));
  assert.equal(state.dispatches.length, 0);
  const deliveries = await Promise.all([deliver('duplicate', payload()), deliver('duplicate', payload())]);
  assert.ok(deliveries.every(response => response.status === 202));
  await eventually(() => state.reactions.some(reaction => reaction.content === 'rocket'), 'dispatch and rocket');
  await sleep(100);
  assert.equal(state.dispatches.length, 1);
  assert.equal(state.checks.length, 1);
  assert.equal(state.checks[0].head_sha, head);
  assert.equal(state.checks[0].name, 'golem/hello');
  const dispatched = state.dispatches[0];
  assert.equal(dispatched.ref, 'trunk');
  assert.equal(dispatched.inputs.repo, repository);
  assert.equal(dispatched.inputs.golem, 'hello');
  const request = JSON.parse(dispatched.inputs.request);
  assert.equal(request.delivery_id, deliveryId('duplicate'));
  assert.equal(request.instructions, 'say fixture\n\n  Keep this indentation.\r\n@isp-golem-xiv other\n');
  assert.equal(request.provider, 'anthropic');
  assert.equal(request.seat_login, 'member');
  assert.equal(request.check_run_id, 1);
  assert.equal(request.timeout_seconds, 1800);
  assert.equal(request.model, null);
  assert.equal(state.reactions.filter(reaction => reaction.content === 'rocket').length, 1);
  // A reused delivery ID with different signed bytes must never cause new side effects.
  assert.equal((await deliver('duplicate', payload('member', '@isp-golem-xiv hello changed'))).status, 202);
  await sleep(100);
  assert.equal(state.dispatches.length, 1);
  assert.equal(state.checks.length, 1);
  // Even an ambiguous dispatch response consumes the durable claim permanently.
  dispatchStatus = 503;
  assert.equal((await deliver('ambiguous', payload())).status, 202);
  await eventually(() => state.dispatches.length === 2, 'ambiguous dispatch attempt');
  assert.equal((await deliver('ambiguous', payload())).status, 202);
  await sleep(100);
  assert.equal(state.dispatches.length, 2);
  assert.equal(state.checks.length, 2);
  assert.equal(state.reactions.filter(reaction => reaction.content === 'rocket').length, 1);
  console.log('Built Workers artifact: health, index auth/refresh/errors, signed webhook refusals, topic/App/first-line guards, atomic concurrent dedup, check/dispatch/rocket passed.');
} finally {await worker.dispose();}
