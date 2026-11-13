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
let head = 'a'.repeat(40);
const privateKey = generateKeyPairSync('rsa', {modulusLength: 2048}).privateKey.export({type:'pkcs1', format:'pem'});
const secret = 'fixture-webhook-secret';
const files = new Map([
  ['hello.md', '---\n---\nRead the private fixture repository.'],
  ['broken.md', '---\nschedule: someday\n---\nInvalid schedule.'],
  ['stop.md', '---\n---\nReserved name.'],
]);
const onboardRepo = 'isp-insoft-gmbh/isp-golem-xiv-test-onboard';
const state = {onboard: {golems: null, writes: [], branches: [], pulls: [], issues: [], pins: [], patches: 0, comments: []}, oauthExchanges: [], seatWrites: [], seatSecrets: new Set(), secretReads: [], variables: new Map(), variableWrites: [], removedMembers: new Set(), calls: 0, dispatches: [], checks: [], comments: [], reactions: [], refs: new Map(), tags: new Map(), tagId: 1, queue: [], enqueueAttempts: [], internal: [], documents: new Map()};
let protocol2 = false;
let installation = [{id:99, full_name:repository, default_branch:'trunk', topics:['isp-golem-xiv-test']}];
const repository2 = 'isp-insoft-gmbh/isp-golem-xiv-test-second';
const repository3 = 'isp-insoft-gmbh/isp-golem-xiv-test-late';
const repositoryFiles = new Map([[repository, files]]);
const metadata = name => ({id:name === repository ? 99 : name === repository2 ? 100 : 101, full_name:name, default_branch:'trunk', topics:allowed ? ['isp-golem-xiv-test'] : []});
let allowed = true;
let dispatchStatus = 204;
// Invented X25519 secret keys are SHA-256 of these labels. `sealed` was produced by libsodium
// (PyNaCl 1.6.2) as crypto_box_seal of `message` for the recipient with that ephemeral key, and
// opened by libsodium's crypto_box_seal_open; `recipientPublic` is libsodium's derived public key.
const sealedFixture = {
  recipientSecret: 'isp-golem-xiv fixture recipient',
  ephemeralSecret: 'isp-golem-xiv fixture ephemeral',
  recipientPublic: '8Y6dzHa/TUrXLhsilriU0wZWao+8W28LRjg8kve/4TI=',
  message: 'sk-ant-oat01-fixture-not-a-credential',
  sealed: '7dLGTa59piM3xAIJnP0ZXl2Y5Czbt2NB/IbWELSN8DOJHCKQYI2Vrl5PnUhXpOkywAdElx2D9uJ+Yq8iu0dAnuiyjby9ABukJJW/PPy1RMrrKZ5bZA==',
};
const answer = (status, body) => new ResponseType(body === null ? null : JSON.stringify(body), {status, headers: {'Content-Type':'application/json'}});
async function github(request) {
  state.calls++;
  const url = new URL(request.url);
  if (url.origin === 'https://fixture-coordinator.invalid') {
    const body = await request.json();
    if (body.action === 'enqueue') {
      state.enqueueAttempts.push(structuredClone(body.request));
      if (state.rejectNextEnqueue > 0) {state.rejectNextEnqueue--; state.rejectedAdmission = body.request; return answer(429,{});}
      const existing = state.queue.find(request => request.delivery_id === body.request.delivery_id);
      if (existing) assert.deepEqual(body.request,existing,'admission replay must preserve exact check/request');
      else state.queue.push(body.request);
    } else {
      state.internal.push(body);
      if (body.action === 'tick') state.refs.set('refs/heads/golem-state', head);
    }
    return answer(200, {state:body.action === 'enqueue' ? 'pending' : 'granted', request_id:body.request?.delivery_id});
  }
  if (url.origin === 'https://github.com' && url.pathname === '/login/oauth/access_token' && request.method === 'POST') {
    assert.equal(request.headers.get('Accept'), 'application/json');
    const form = new URLSearchParams(await request.text());
    state.oauthExchanges.push(Object.fromEntries(form));
    const token = {'member-code':'oauth-member-token', 'outsider-code':'oauth-outsider-token'}[form.get('code')];
    return answer(200, token && form.get('client_secret') === 'fixture-client-secret' ? {access_token:token, token_type:'bearer', scope:''} : {error:'bad_verification_code'});
  }
  assert.equal(url.origin, 'https://api.github.com');
  const path = url.pathname;
  const method = request.method;
  const body = (['POST','PUT'].includes(method) || (method === 'DELETE' && path.includes('/contents/'))) ? await request.json() : null;
  if (path.startsWith(`/repos/${ops}/contents/`) && ['PUT','DELETE'].includes(method) && body?.branch === 'golem-state' && !state.refs.has('refs/heads/golem-state')) return answer(404, {message:'Branch not found'});
  if (path === `/repos/${ops}/actions/variables/GOLEM_PROTOCOL_VERSION`) return answer(protocol2 ? 200 : 404, {value:'2'});
  if (path === '/installation/repositories') {
    assert.equal(url.searchParams.get('per_page'), '1');
    const page = Number(url.searchParams.get('page'));
    return answer(200, {repositories:installation.slice(page-1,page)});
  }
  if (path === `/repos/${ops}/contents/scheduler-v2.json` || path === `/repos/${ops}/contents/events-v2.json` || path.startsWith(`/repos/${ops}/contents/events-v2/`)) {
    const previous = state.documents.get(path);
    if (method === 'GET') return previous ? answer(200, {...previous, encoding:'base64'}) : answer(404, {});
    assert.equal(body.branch, 'golem-state');
    if (method === 'PUT' && !previous && state.failInitialDocument === path) {state.failInitialDocument = null; state.failedInitialDocument = path; return answer(503,{});}
    if (body.sha !== previous?.sha) return answer(409, {});
    if (method === 'DELETE') {state.documents.delete(path); return answer(200, {});}
    const sha = String(state.tagId++).padStart(40, '0');
    state.documents.set(path, {sha,content:body.content});
    return answer(previous ? 200 : 201, {content:{sha}});
  }
  if (path === `/repos/${ops}/contents/events-v2`) return answer(200, [...state.documents.keys()].filter(key => key.includes('/events-v2/')).sort().map(key => ({name:key.split('/').at(-1)})));
  if (path === `/repos/${onboardRepo}` || path.startsWith(`/repos/${onboardRepo}/`)) {
    const o = state.onboard;
    const sub = path.slice(`/repos/${onboardRepo}`.length);
    if (sub === '') return answer(200, {id:150, full_name:onboardRepo, default_branch:'trunk', private:true, topics:['isp-golem-xiv-test']});
    if (sub === '/commits/trunk') return answer(200, {sha:head});
    if (sub === '/contents/.golems') return o.golems ? answer(200, [...o.golems].map(([name,content]) => ({name,type:'file',size:Buffer.byteLength(content)}))) : answer(404, {});
    if (sub.startsWith('/contents/.golems/') && method === 'GET') {
      const content = o.golems?.get(sub.split('/').at(-1));
      return content === undefined ? answer(404, {}) : answer(200, {type:'file',encoding:'base64',content:Buffer.from(content).toString('base64')});
    }
    if (sub === '/contents' && method === 'GET') return answer(200, [{name:'README.md'},{name:'mise.toml'}]);
    if (sub.startsWith('/contents/') && method === 'PUT') {o.writes.push({path:sub.slice('/contents/'.length), branch:body.branch, content:Buffer.from(body.content,'base64').toString()}); return answer(201, {content:{sha:'d'.repeat(40)}});}
    if (sub === '/git/refs' && method === 'POST') {o.branches.push(body); return answer(201, {ref:body.ref});}
    if (sub === '/pulls' && method === 'POST') {o.pulls.push(body); return answer(201, {number:o.pulls.length});}
    if (sub === '/issues' && method === 'GET') return answer(200, o.issues.filter(issue => issue.state === 'open'));
    if (sub === '/issues' && method === 'POST') {const issue = {number:100+o.issues.length, node_id:`I_node${o.issues.length}`, state:'open', user:{login:'isp-golem-xiv[bot]'}, title:body.title, body:body.body}; o.issues.push(issue); return answer(201, issue);}
    const patch = sub.match(/^\/issues\/(\d+)$/);
    const issue = patch && o.issues.find(i => i.number === Number(patch[1]));
    if (patch && !issue) return answer(404, {});
    if (patch && method === 'GET') return answer(200, issue);
    if (patch && method === 'PATCH') {const update = await request.json(); issue.body = update.body; o.patches++; return answer(200, issue);}
    const comments = sub.match(/^\/issues\/(\d+)\/comments$/);
    if (comments && method === 'POST') {o.comments.push({issue:Number(comments[1]), body:body.body}); return answer(201, {id:o.comments.length});}
  }
  if (path === '/graphql' && method === 'POST') {state.onboard.pins.push(body.variables.id); return answer(200, {data:{pinIssue:{issue:{id:body.variables.id}}}});}
  if (path === `/repos/${ops}/actions/secrets`) return answer(200, {total_count:3, secrets:[{name:'APP_PRIVATE_KEY'},{name:'SEAT_MEMBER__ANTHROPIC'},{name:'SEAT_GONE_PERSON__OPENAI_CODEX'}]});
  if (path === '/orgs/isp-insoft-gmbh/members') return answer(200, [{login:'member'},{login:'other'}]);
  const target = [repository,repository2,repository3].find(name => path.startsWith(`/repos/${name}/`) || path === `/repos/${name}`);
  if (target) {
    if (path === `/repos/${target}`) return answer(200, metadata(target));
    if (path === `/repos/${target}/commits/trunk`) return answer(200, {sha:head});
    const targetFiles = repositoryFiles.get(target) ?? new Map();
    if (path === `/repos/${target}/contents/.golems`) return answer(200, [...targetFiles].map(([name,content]) => ({name,type:'file',size:Buffer.byteLength(content)})));
    if (path.startsWith(`/repos/${target}/contents/.golems/`)) {
      const content = targetFiles.get(path.split('/').at(-1));
      return content === undefined ? answer(404,{}) : answer(200,{type:'file',encoding:'base64',content:Buffer.from(content).toString('base64')});
    }
  }
  if (path === '/user') {
    const oauthLogin = {'Bearer oauth-member-token':'member', 'Bearer oauth-outsider-token':'outsider'}[request.headers.get('Authorization')];
    if (oauthLogin) return answer(200, {login:oauthLogin});
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
  if (path === `/repos/${ops}/git/ref/heads/golem-state`) return state.refs.has('refs/heads/golem-state') ? answer(200, {ref:'refs/heads/golem-state',object:{sha:head,type:'commit'}}) : answer(404, {});
  if (path === `/repos/${ops}/git/matching-refs/tags/golem-event/`) return answer(200, [...state.refs].filter(([ref]) => ref.startsWith('refs/tags/golem-event/')).map(([ref,sha]) => ({ref,object:{sha,type:'tag'}})));
  if (path.startsWith(`/repos/${ops}/git/refs/tags/`) && method === 'DELETE') {
    const ref = 'refs/' + path.slice(`/repos/${ops}/git/refs/`.length);
    return answer(state.refs.delete(ref) ? 204 : 404, null);
  }
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
  if (path.startsWith('/orgs/isp-insoft-gmbh/memberships/')) return path.endsWith('/outsider') || state.removedMembers.has(path.split('/').at(-1)) ? answer(404, {}) : answer(200, {state:'active'});
  if (path === `/repos/${ops}/actions/secrets/public-key`) return answer(200, {key_id:'568250167242549743', key:sealedFixture.recipientPublic});
  if (path.startsWith(`/repos/${ops}/actions/secrets/`)) {
    const name = path.split('/').at(-1);
    if (method === 'PUT') {
      state.seatWrites.push({name, body});
      const replaced = state.seatSecrets.has(name);
      state.seatSecrets.add(name);
      return replaced ? answer(204, null) : answer(201, {});
    }
    state.secretReads.push(name);
    return answer(['SEAT_MEMBER__ANTHROPIC', 'SEAT_CODEXONLY__OPENAI_CODEX'].includes(name) || state.seatSecrets.has(name) ? 200 : 404, {});
  }
  if (path === `/repos/${ops}/actions/variables` && method === 'POST') {
    state.variableWrites.push({method, body});
    if (state.variables.has(body.name)) return answer(409, {});
    state.variables.set(body.name, body.value);
    return answer(201, {});
  }
  if (path.startsWith(`/repos/${ops}/actions/variables/`)) {
    const name = path.split('/').at(-1);
    if (method === 'PATCH') {
      const update = await request.json();
      state.variableWrites.push({method, name, body:update});
      if (!state.variables.has(name)) return answer(404, {});
      state.variables.set(name, update.value);
      return answer(204, null);
    }
    return state.variables.has(name) ? answer(200, {name, value:state.variables.get(name)}) : answer(404, {});
  }
  if (path.endsWith('/reactions')) {state.reactions.push({path, ...body}); return answer(201, {id:1});}
  if (path.endsWith('/check-runs')) {state.checks.push(body); return answer(201, {id:state.checks.length});}
  if (path === `/repos/${ops}/actions/workflows/runner.yml/dispatches`) {state.dispatches.push(body); return answer(dispatchStatus, dispatchStatus === 204 ? null : {});}
  if (path === `/repos/${repository}/issues/1/comments`) {state.comments.push(body.body); return answer(201, {id:state.comments.length});}
  if (path === `/repos/${repository}/pulls/7`) return answer(200, {number:7, head:{sha:'c'.repeat(40)}});
  if (path === `/repos/${repository}/pulls/7/files`) return answer(200, state.pullFiles ?? []);
  if (path.startsWith('/repos/')) return answer(404, {});
  throw new Error(`Unexpected fixture API: ${method} ${path}`);
}
// Exercise Rust -> named org DO binding without conflating policy tests with queue tests.
modules['policy-entry.js'] = {type:'esm', contents:`export {default} from './index.js'; export class PolicyCoordinator { async fetch(request) { return fetch('https://fixture-coordinator.invalid', {method:'POST',body:await request.text()}); } }`};
const createWorker = () => new Miniflare({workers:[{
  config:{name:'router-fixture', compatibilityDate:'2026-10-06', manifest:{mainModule:'policy-entry.js', modules}, exports:{PolicyCoordinator:{type:'durable-object',storage:'sqlite'}}, env:{
    COORDINATOR:{type:'durable-object',worker:'router-fixture',exportName:'PolicyCoordinator'},
    VERSION:{type:'text', value:'fixture-version'},
    APP_PRIVATE_KEY:{type:'text', value:privateKey},
    WEBHOOK_SECRET:{type:'text', value:secret},
    APP_CLIENT_SECRET:{type:'text', value:'fixture-client-secret'},
  }},
  dev:{outboundService:{type:'fetcher', handler:github}},
}]});
let worker = createWorker();
const base = 'https://router.invalid';
async function tick(timestamp) {
  const before = state.calls;
  const entry = await worker.getWorker();
  await entry.scheduled({scheduledTime:timestamp,cron:'* * * * *'});
  assert.ok(state.calls-before <= 50, `scheduled batch used ${state.calls-before} calls`);
}
const cursorPath = `/repos/${ops}/contents/scheduler-v2.json`;
const cursor = () => JSON.parse(Buffer.from(state.documents.get(cursorPath).content, 'base64'));
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
async function deliver(id, value, signatureValid = true, malformedId = false, event = 'issue_comment') {
  const body = JSON.stringify(value);
  const signature = createHmac('sha256', signatureValid ? secret : 'wrong-fixture-secret').update(body).digest('hex');
  return worker.dispatchFetch(`${base}/webhook`, {method:'POST', body, headers:{'Content-Type':'application/json', 'X-GitHub-Event':event, 'X-GitHub-Delivery':malformedId ? id : deliveryId(id), 'X-Hub-Signature-256':`sha256=${signature}`}});
}
try {
  const health = await worker.dispatchFetch(`${base}/health`);
  assert.equal(health.status, 200);
  assert.deepEqual(await health.json(), {app:'isp-golem-xiv', version:'fixture-version'});
  const states = new Set();
  for (const page of ['/seats', '/admin']) {
    const signIn = await worker.dispatchFetch(`${base}${page}`, {redirect: 'manual'});
    assert.equal(signIn.status, 302, `${page} without session redirects`);
    const location = new URL(signIn.headers.get('Location'));
    assert.equal(`${location.origin}${location.pathname}`, 'https://github.com/login/oauth/authorize');
    assert.equal(location.searchParams.get('client_id'), 'Iv23liEnfLEVCSS2aUdR');
    assert.equal(location.searchParams.get('redirect_uri'), 'https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/auth/github/callback');
    const state = location.searchParams.get('state');
    assert.match(state, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    assert.equal(signIn.headers.get('Set-Cookie'), `golem_oauth_state=${state}; Path=/auth/github; Max-Age=600; HttpOnly; Secure; SameSite=Lax`);
    assert.match(signIn.headers.get('Cache-Control'), /no-store/);
    states.add(state);
  }
  assert.equal(states.size, 2, 'every sign-in gets a fresh state');
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
  assert.equal(state.calls, calls, 'App loop guard precedes all API calls');
  assert.equal((await deliver('later-mention', payload('member', 'Text\n@isp-golem-xiv hello'))).status, 200);
  await sleep(100);
  assert.equal(state.refs.size, 0, 'legacy noncommands never claim deliveries');
  assert.equal(state.dispatches.length, 0);
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
  assert.ok(state.comments.at(-1).startsWith('<!-- isp-golem-xiv kind=refused reason=unknown-golem -->'), 'legacy v1 keeps its refusal');
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
  const legacyRefs = state.refs.size;
  await tick(Date.UTC(2026,10,5,10));
  assert.equal(state.refs.size, legacyRefs, 'absent rollout variable suppresses scheduling');
  protocol2 = true;
  dispatchStatus = 204;
  files.set('hello.md', '---\nbranch: golems/retained\nverify: cargo test\nowner: member\n---\nContinue retained branch.');
  const directDispatches = state.dispatches.length;
  assert.equal((await deliver('v2-mention',payload())).status,202);
  await eventually(() => state.queue.length === 1, 'v2 mention admission');
  assert.equal(state.dispatches.length,directDispatches,'v2 router never dispatches directly');
  assert.equal(state.queue[0].version,2);
  assert.equal(state.queue[0].branch,'golems/retained');
  assert.equal(state.queue[0].verify,'cargo test');
  assert.equal(state.queue[0].owner,'member');
  assert.equal(state.queue[0].trigger,'mention');
  assert.equal(state.queue[0].head_sha,head);
  assert.equal(state.queue[0].instructions,request.instructions);
  assert.equal(state.reactions.filter(reaction => reaction.content === 'rocket').length,1,'coordinator owns v2 rockets');
  // A golem whose harness does not work with the payer's seat at its provider runs there with pi.
  files.set('foreign.md', '---\nharness: codex\nprovider: anthropic\n---\nReply with one short greeting.');
  const queuedBeforeForeign = state.queue.length;
  assert.equal((await deliver('v2-fallback-mention', payload('member', '@isp-golem-xiv foreign'))).status, 202);
  await eventually(() => state.queue.length === queuedBeforeForeign + 1, 'fallback mention admission');
  const foreign = state.queue.at(-1);
  assert.equal(foreign.golem, 'foreign');
  assert.equal(foreign.seat_login, 'member');
  assert.equal(foreign.provider, 'anthropic');
  assert.equal(foreign.harness, 'pi');
  assert.equal(foreign.model, null);
  assert.equal(foreign.fallback, 'asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic');
  const foreignCheck = state.checks.at(-1);
  assert.equal(foreignCheck.name, 'golem/foreign');
  assert.match(foreignCheck.output.summary, /^seat: member\/anthropic\nharness: pi\nmodel: default$/m);
  assert.match(foreignCheck.output.summary, /^fallback: asked for anthropic\/codex, used anthropic\/pi with its default model, because codex does not work with anthropic$/m, 'queued check names the fallback');
  const refusedBeforeForeign = state.comments.length;
  assert.equal((await deliver('v2-fallback-no-seat', payload('noseat', '@isp-golem-xiv foreign'))).status, 202);
  await eventually(() => state.comments.length > refusedBeforeForeign, 'seatless fallback refusal');
  assert.ok(state.comments.at(-1).startsWith('<!-- isp-golem-xiv kind=refused reason=no-seat -->'), 'a payer without any seat is still refused');
  assert.equal(state.queue.length, queuedBeforeForeign + 1, 'seatless mention never enqueues');
  const refusedBeforeCodex = state.comments.length;
  // pi on an openai-codex seat is a choice the runner cannot execute.
  files.set('roaming.md', '---\nharness: pi\n---\nReply with one short greeting.');
  assert.equal((await deliver('v2-fallback-unrunnable-seat', payload('codexonly', '@isp-golem-xiv roaming'))).status, 202);
  await eventually(() => state.comments.length > refusedBeforeCodex, 'unrunnable seat refusal');
  assert.ok(state.comments.at(-1).startsWith('<!-- isp-golem-xiv kind=refused reason=no-seat -->'), 'a seat the runner cannot use is refused, never rerouted');
  assert.equal(state.queue.length, queuedBeforeForeign + 1, 'unrunnable seat never enqueues');
  files.delete('foreign.md');
  files.delete('roaming.md');
  const queuedBeforeHelp = state.queue.length;
  const helpCount = state.comments.length;
  assert.equal((await deliver('v2-bare-help', payload('member', '@isp-golem-xiv\nanything'))).status, 202);
  await eventually(() => state.comments.length > helpCount, 'v2 bare mention answers help');
  const help = state.comments.at(-1);
  assert.ok(help.startsWith('<!-- isp-golem-xiv kind=help -->'), 'bare mention answers help');
  assert.match(help, /^- `hello`: seat `member\/anthropic`, harness `claude`, model `default`$/m);
  assert.ok(help.includes('Your seats: `anthropic`.') && help.includes('/seats'));
  const seatCount = state.comments.length;
  assert.equal((await deliver('v2-seat-command', payload('member', '@isp-golem-xiv seat'))).status, 202);
  await eventually(() => state.comments.length > seatCount, 'seat command answers');
  const seatReply = state.comments.at(-1);
  assert.ok(seatReply.startsWith('<!-- isp-golem-xiv kind=seat -->\n@member, your seats: `anthropic`.'), 'seat reply names only the asker');
  assert.ok(seatReply.includes('https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats'));
  assert.equal(state.queue.length, queuedBeforeHelp, 'help and seat never enqueue');
  assert.equal(state.dispatches.length, directDispatches, 'help and seat never dispatch');
  const pullEvent = {action:'opened', sender:{id:42,login:'member',type:'User'}, repository:{id:99,full_name:repository}, installation:{id:153964219}, pull_request:{number:7}};
  const validations = () => state.checks.filter(check => check.name === 'golems/validate');
  state.pullFiles = [{filename:'README.md'}];
  assert.equal((await deliver('v2-pr-unrelated', pullEvent, true, false, 'pull_request')).status, 200);
  await sleep(300);
  assert.equal(validations().length, 0, 'pull requests not touching .golems/ get no validation check');
  state.pullFiles = [{filename:'docs/x.md'}, {filename:'.golems/broken.md'}];
  assert.equal((await deliver('v2-pr-golems', pullEvent, true, false, 'pull_request')).status, 200);
  await eventually(() => validations().length === 1, 'validation check on .golems/ change');
  const validation = validations()[0];
  assert.equal(validation.head_sha, 'c'.repeat(40), 'validated at the pull request head');
  assert.equal(validation.status, 'completed');
  assert.equal(validation.conclusion, 'failure');
  assert.match(validation.output.summary, /^- `broken`: .*schedule/m);
  assert.match(validation.output.summary, /^- `hello`: valid$/m);
  assert.match(validation.output.summary, /^- `stop`: /m);
  state.pullFiles = [{filename:'.golems/hello.md'}];
  const savedBroken = files.get('broken.md'), savedStop = files.get('stop.md');
  files.delete('broken.md'); files.delete('stop.md');
  assert.equal((await deliver('v2-pr-valid', {...pullEvent, action:'synchronize'}, true, false, 'pull_request')).status, 200);
  await eventually(() => validations().length === 2, 'validation check on synchronize');
  assert.equal(validations()[1].conclusion, 'success');
  files.set('broken.md', savedBroken); files.set('stop.md', savedStop);
  // Onboarding: once per repository, exactly two files, then a pinned dashboard kept current.
  const o = state.onboard;
  const push = {sender:{id:42,login:'member',type:'User'}, repository:{id:150, full_name:onboardRepo}, installation:{id:153964219}};
  assert.equal((await deliver('v2-onboard-push', push, true, false, 'push')).status, 200);
  await eventually(() => o.pulls.length === 1, 'onboarding pull request');
  assert.deepEqual(o.pulls[0].title, 'Enable golems');
  assert.equal(o.pulls[0].head, 'golems/enable');
  assert.equal(o.pulls[0].base, 'trunk');
  assert.match(o.pulls[0].body, /`mise\.toml`/);
  assert.match(o.pulls[0].body, /`@isp-golem-xiv seat`/);
  assert.deepEqual(o.branches, [{ref:'refs/heads/golems/enable', sha:head}]);
  assert.deepEqual(o.writes.map(w => w.path).sort(), ['.golems/_golems.md', '.golems/hello.md']);
  assert.ok(o.writes.every(w => w.branch === 'golems/enable'), 'files only on the onboarding branch');
  assert.equal(o.writes.find(w => w.path === '.golems/_golems.md').content.trim(), 'You are a golem working unattended in this repository.');
  assert.ok(state.refs.has('refs/tags/golem-onboarding/150'), 'durable once-per-repository claim');
  assert.equal((await deliver('v2-onboard-again', push, true, false, 'push')).status, 200);
  await sleep(300);
  assert.equal(o.pulls.length, 1, 'onboarding opens at most once');
  o.golems = new Map(o.writes.map(w => [w.path.split('/').at(-1), w.content]));
  assert.equal((await deliver('v2-onboard-merged', push, true, false, 'push')).status, 200);
  await eventually(() => o.issues.length === 1 && o.pins.length === 1, 'dashboard created and pinned');
  const board = o.issues[0];
  assert.equal(board.title, 'Golems');
  assert.equal(o.pins[0], board.node_id);
  for (const heading of ['Golems', 'Disabled', 'Talk to golems', 'Seats']) assert.match(board.body, new RegExp(`^## ${heading}$`, 'm'));
  assert.match(board.body, /^- `hello`: Look around this repository/m);
  assert.match(board.body, /^- @member: `anthropic`$/m);
  assert.match(board.body, /^- GONE_PERSON: `openai-codex`$/m, 'seats of people who left are reported');
  assert.ok(!board.body.includes('APP_PRIVATE_KEY'), 'non-seat secret names never shown');
  o.golems.set('broken.md', '---\nschedule: someday\n---\nInvalid on purpose.');
  assert.equal((await deliver('v2-onboard-invalid', push, true, false, 'push')).status, 200);
  await eventually(() => o.patches === 1, 'dashboard updated after .golems/ change');
  assert.match(o.issues[0].body, /## Disabled\n\n- `broken`: .*schedule/);
  assert.equal(o.issues.length, 1, 'dashboard updated, never duplicated');
  assert.equal(o.pins.length, 1);
  assert.equal((await deliver('v2-onboard-unchanged', push, true, false, 'push')).status, 200);
  await sleep(300);
  assert.equal(o.patches, 1, 'unchanged dashboard is not rewritten');
  const queuedBeforeStop = state.queue.length;
  assert.equal((await deliver('v2-stop',payload('member','@isp-golem-xiv stop'))).status,202);
  await eventually(() => state.internal.some(body => body.action === 'stop'), 'thread stop handoff');
  assert.deepEqual(state.internal.find(body => body.action === 'stop'),{action:'stop',repository,issue_number:1});
  assert.equal(state.queue.length,queuedBeforeStop,'stop never enqueues or prematurely releases');
  const queueEndpoint = `${base}/internal/queue`;
  async function callback(value, key = privateKey, suffix = '') {
    const body = JSON.stringify(value) + suffix;
    const signature = createHmac('sha256',key.replaceAll('\r\n','\n').trim()).update(body).digest('hex');
    return worker.dispatchFetch(queueEndpoint,{method:'POST',body,headers:{'X-Golem-Signature-256':`sha256=${signature}`}});
  }
  const binding = {action:'bind',request_id:deliveryId('v2-mention'),generation:deliveryId('generation'),run_id:123,attempt:1,timestamp_ms:Date.now()};
  const beforeInternal = state.internal.length;
  assert.equal((await callback(binding,'wrong-fixture-key')).status,401);
  assert.equal((await callback({...binding,timestamp_ms:Date.now()-60_001})).status,401);
  assert.equal((await callback({...binding,action:'stop'})).status,401);
  assert.equal(state.internal.length,beforeInternal,'bad native auth never reaches DO');
  assert.equal((await callback(binding,privateKey.replaceAll('\n','\r\n'),'\n')).status,200);
  assert.deepEqual(state.internal.at(-1),binding,'valid native payload forwarded intact');
  assert.equal((await callback({...binding,action:'complete'})).status,401);
  assert.equal((await callback({...binding,action:'complete',containers_destroyed:true,publication_complete:true})).status,200);
  const appCalls = state.calls;
  assert.equal((await deliver('v2-app-event', {...payload('isp-golem-xiv[bot]'),action:'opened'},true,false,'issues')).status,200);
  assert.equal(state.calls,appCalls,'v2 App event protection precedes rollout metadata');
  files.clear();
  files.set('event.md','---\ntriggers: [issues.opened]\nowner: member\n---\nRespond to issue event.');
  const event = {action:'opened',sender:{id:49699333,login:'dependabot[bot]',type:'Bot'},repository:{id:99,full_name:repository},installation:{id:153964219},issue:{number:1}};
  state.rejectNextEnqueue = 1;
  const botBefore = state.queue.length;
  assert.equal((await deliver('v2-bot',event,true,false,'issues')).status,200);
  await eventually(() => state.rejectedAdmission,'coordinator capacity rejection');
  assert.equal(state.queue.length,botBefore,'rejected admission is not dispatched');
  const retainedCheck = state.rejectedAdmission.check_run_id;
  // Failure of the very first scheduler PUT must neither poison initialization
  // nor prevent independently retained event work from progressing this tick.
  state.failInitialDocument = cursorPath;
  await tick(Date.UTC(2026,10,5,10));
  assert.equal(state.failedInitialDocument,cursorPath);
  assert.ok(!state.documents.has(cursorPath));
  assert.equal(state.queue.length,botBefore+1,'event continuation progresses despite scheduler initialization failure');
  assert.equal(state.queue.at(-1).check_run_id,retainedCheck,'capacity retry reuses durable prepared check');
  await worker.dispose(); worker = createWorker();
  await tick(Date.UTC(2026,10,5,10,1));
  assert.ok(state.documents.has(cursorPath),'scheduler initialization recovers after restart');
  await eventually(() => state.queue.length === botBefore+1,'other bot owner admission');
  assert.equal(state.queue.at(-1).seat_login,'member');
  assert.equal(state.queue.at(-1).trigger,'event');
  assert.equal(state.queue.at(-1).comment_id,null);
  assert.equal(state.queue.at(-1).actor_login,'dependabot[bot]');
  const fallback = {...event,sender:{id:43,login:'noseat',type:'User'}};
  assert.equal((await deliver('v2-fallback',fallback,true,false,'issues')).status,200);
  await eventually(() => state.queue.length === 3,'member without seat falls back to owner');
  assert.equal(state.queue.at(-1).seat_login,'member');
  // Twenty matching definitions exceed a single Worker batch. Persist, restart,
  // then prove every definition gets exactly one admission rather than disappearing.
  await sleep(100);
  files.clear();
  for(let i=0;i<20;i++) files.set(`event-${String(i).padStart(2,'0')}.md`,'---\ntriggers: [issues.opened]\nowner: member\n---\nEvent task.');
  const beforeBatch = state.queue.length;
  await deliver('v2-twenty-events',event,true,false,'issues');
  await eventually(() => [...state.documents.keys()].some(path => path.endsWith(`${deliveryId('v2-twenty-events')}.json`)),'durable bounded event batch');
  await sleep(100);
  await worker.dispose(); worker = createWorker();
  const start = Date.UTC(2026,10,5,12);
  for(let i=0;i<30 && state.queue.length < beforeBatch+20;i++) await tick(start+i*60_000);
  assert.equal(state.queue.length,beforeBatch+20,'all twenty matching events resume');
  const admitted = state.queue.slice(beforeBatch);
  assert.equal(new Set(admitted.map(req => req.golem)).size,20);
  assert.equal(new Set(admitted.map(req => req.delivery_id)).size,20);
  await deliver('v2-twenty-events',event,true,false,'issues'); await sleep(100);
  assert.equal(state.queue.length,beforeBatch+20,'redelivery cannot duplicate continuation');
  // Continuation must re-read current default SHA and definitions: deleted or
  // no-longer-matching golems cannot consume a seat; remaining work still runs.
  const beforeChangedConfig = state.queue.length;
  await deliver('v2-changed-config',event,true,false,'issues');
  const changedPath = `/repos/${ops}/contents/events-v2/${deliveryId('v2-changed-config')}.json`;
  await eventually(() => state.documents.has(changedPath),'fresh-config continuation retained');
  await sleep(100);
  files.delete('event-00.md');
  files.set('event-01.md','---\ntriggers: [issues.closed]\nowner: member\n---\nNo longer matches.');
  head = 'b'.repeat(40);
  await worker.dispose(); worker = createWorker();
  for(let i=30;i<60 && state.documents.has(changedPath);i++) await tick(start+i*60_000);
  assert.ok(!state.documents.has(changedPath),'changed-config continuation finishes');
  const changed = state.queue.slice(beforeChangedConfig);
  assert.equal(changed.length,18);
  assert.ok(changed.every(req => req.golem !== 'event-00' && req.golem !== 'event-01' && req.head_sha === head));
  // Index admission followed by initial cursor PUT failure is recoverable from
  // immutable source; it must not strand subsequent deliveries behind its head.
  const failureId = deliveryId('v2-journal-failure');
  const nextId = deliveryId('v2-after-journal-failure');
  const failurePath = `/repos/${ops}/contents/events-v2/${failureId}.json`;
  const nextPath = `/repos/${ops}/contents/events-v2/${nextId}.json`;
  state.failInitialDocument = failurePath;
  const beforeRecovery = state.queue.length;
  await deliver('v2-journal-failure',event,true,false,'issues');
  await eventually(() => state.failedInitialDocument === failurePath,'injected initial journal failure after index insertion');
  assert.ok(!state.documents.has(failurePath));
  await deliver('v2-after-journal-failure',event,true,false,'issues');
  await eventually(() => state.documents.has(nextPath),'later delivery continuation admitted');
  await sleep(100);
  await worker.dispose(); worker = createWorker();
  await tick(start+60*60_000);
  assert.ok(state.documents.has(failurePath),`stranded head reconstructed from immutable private source; index=${JSON.stringify([...state.documents].filter(([path]) => path.endsWith('events-v2.json')).map(([,value]) => JSON.parse(Buffer.from(value.content,'base64'))))}`);
  for(let i=61;i<110 && (state.documents.has(failurePath) || state.documents.has(nextPath));i++) await tick(start+i*60_000);
  assert.ok(!state.documents.has(failurePath) && !state.documents.has(nextPath),'both continuations retire');
  assert.equal(state.queue.length,beforeRecovery+36);
  const laterIds = new Set([...files.keys()].filter(name => name !== 'event-01.md').map(name => deliveryId(`event-v2/${nextId}/${name.slice(0,-3)}`)));
  assert.equal(state.queue.slice(beforeRecovery).filter(req => laterIds.has(req.delivery_id)).length,18,'later delivery progresses after restart');
  // Daily cursor survives Worker restart and installation page reordering.
  files.clear();
  for(const name of ['alpha','zeta']) files.set(`${name}.md`,'---\nschedule: daily\nowner: member\n---\nScheduled task.');
  repositoryFiles.set(repository2,new Map([['beta.md','---\nschedule: daily\nowner: member\n---\nSecond repo task.']]));
  installation = [metadata(repository),metadata(repository2)];
  const day = Date.UTC(2026,10,6,12);
  const scheduledBefore = state.queue.filter(req => req.trigger === 'schedule').length;
  await tick(day);
  assert.equal(cursor().selected.repository,repository);
  assert.equal(cursor().selected.golem,'alpha');
  assert.equal(cursor().selected.minute,day/60_000+1);
  installation.reverse();
  await worker.dispose(); worker = createWorker();
  state.rejectNextEnqueue = 1;
  const beforeScheduledAdmission = state.queue.length;
  const beforeScheduledChecks = state.checks.length;
  await tick(day+60_000);
  const rejectedSchedule = structuredClone(state.rejectedAdmission);
  assert.equal(rejectedSchedule.trigger,'schedule');
  assert.equal(state.queue.length,beforeScheduledAdmission,'scheduled capacity rejection cannot consume occurrence');
  assert.ok(state.refs.has(`refs/tags/golem-delivery/${rejectedSchedule.delivery_id}`),'daily claim succeeded before admission rejection');
  await worker.dispose(); worker = createWorker();
  await tick(day+2*60_000);
  assert.equal(state.queue.length,beforeScheduledAdmission+1,'claimed scheduled occurrence retries after restart and original minute');
  assert.deepEqual(state.queue.at(-1),rejectedSchedule,'scheduled retry preserves exact payload, occurrence ID and check ID');
  assert.equal(state.checks.length,beforeScheduledChecks+1,'scheduled retry never recreates check');
  await tick(day+2*60_000);
  assert.equal(state.queue.filter(req => req.delivery_id === rejectedSchedule.delivery_id).length,1,'scheduled admission occurs exactly once');
  assert.equal(state.queue.at(-1).repository,repository);
  assert.equal(state.queue.at(-1).golem,'alpha','selected occurrence cannot move with installation page');
  for(let i=3;i<23;i++) await tick(day+i*60_000);
  const scheduled = state.queue.filter(req => req.trigger === 'schedule').slice(scheduledBefore);
  assert.equal(scheduled.length,3,'reordered installation does not skip remaining eligible definitions');
  assert.deepEqual(new Set(scheduled.map(req => `${req.repository}/${req.golem}`)),new Set([`${repository}/alpha`,`${repository}/zeta`,`${repository2}/beta`]));
  assert.ok(scheduled.every(req => req.seat_login === 'member' && req.actor_login === null && req.comment_id === null && req.head_sha === head));
  assert.equal(new Set(scheduled.map(req => req.delivery_id)).size,3,'one persistent claim per repository/golem/day');
  assert.equal(state.dispatches.length,directDispatches,'all v2 dispatch ownership remains outside Rust');
  const oldScheduleIds = scheduled.map(req => req.delivery_id);
  const attemptsBeforeSweep = new Map(oldScheduleIds.map(id => [id,state.enqueueAttempts.filter(req => req.delivery_id === id).length]));
  for(let i=23;i<40 && !cursor().done;i++) await tick(day+i*60_000);
  assert.equal(cursor().done,true,'daily scan converges before repository is added');
  const checksBeforeLate = state.checks.length;
  repositoryFiles.set(repository3,new Map([['late.md','---\nschedule: daily\nowner: member\n---\nLate repository task.']]));
  installation.push(metadata(repository3));
  await worker.dispose(); worker = createWorker();
  for(let i=40;i<100 && !state.queue.some(req => req.repository === repository3 && req.trigger === 'schedule');i++) await tick(day+i*60_000);
  const late = state.queue.filter(req => req.repository === repository3 && req.trigger === 'schedule');
  assert.equal(late.length,1,'repository added after completed scan is admitted on same UTC day');
  assert.equal(late[0].golem,'late');
  assert.equal(late[0].seat_login,'member');
  assert.equal(state.checks.length,checksBeforeLate+1,'late occurrence is prepared once');
  for(let i=100;i<110;i++) await tick(day+i*60_000);
  assert.equal(state.queue.filter(req => req.repository === repository3 && req.trigger === 'schedule').length,1,'late occurrence admitted once');
  assert.equal(state.enqueueAttempts.filter(req => req.delivery_id === late[0].delivery_id).length,1,'late occurrence has exactly one raw admission attempt');
  assert.ok(oldScheduleIds.every(id => state.enqueueAttempts.filter(req => req.delivery_id === id).length === attemptsBeforeSweep.get(id)),'rescan never retries completed daily occurrences, including previously retried admissions');
  assert.ok(oldScheduleIds.every(id => state.queue.filter(req => req.delivery_id === id).length === 1),'completed occurrences are not readmitted');
  assert.equal(state.checks.length,checksBeforeLate+1,'completed occurrences are not prepared again');
  // v2 mention admission rejection must not strand valid mention; restart retries exact check/request.
  state.rejectNextEnqueue = 1;
  const mentionDeliveryId = deliveryId('v2-mention-reject');
  const mentionPath = `/repos/${ops}/contents/events-v2/${mentionDeliveryId}.json`;
  state.rejectedAdmission = undefined;
  files.clear();
  files.set('hello.md','---\n---\nRead the private fixture repository.');
  const mentionBefore = state.queue.length;
  const mentionRockets = state.reactions.filter(r => r.content === 'rocket').length;
  assert.equal((await deliver('v2-mention-reject',payload())).status,202);
  await eventually(() => state.rejectedAdmission?.delivery_id === mentionDeliveryId,'mention admission rejected after durable preparation');
  assert.ok(state.documents.has(mentionPath),'mention EventWork persisted before admission');
  assert.equal(state.reactions.filter(r => r.content === 'rocket').length,mentionRockets,'rejected mention receives no premature rocket');
  assert.equal(state.queue.length,mentionBefore,'rejected admission not dispatched');
  // After restart, the exact same check/request is retried exactly once via event_tick.
  await worker.dispose(); worker = createWorker();
  state.rejectNextEnqueue = 0;
  await tick(Date.UTC(2026,10,7,12));
  const admittedMention = state.queue.find(req => req.delivery_id === mentionDeliveryId);
  assert.ok(admittedMention,'restarted mention admits');
  assert.equal(admittedMention.trigger,'mention');
  assert.equal(admittedMention.seat_login,'member');
  assert.equal(state.queue.filter(req => req.delivery_id === mentionDeliveryId).length,1,'restarted mention admits exactly once');
  files.clear();
  files.set('echo.md','---\ntriggers: [issue_comment.created]\nowner: member\n---\nQuote the triggering comment.');
  await eventually(() => !state.documents.has(mentionPath) && !state.refs.has(`refs/tags/golem-event/${mentionDeliveryId}`),'previous mention fully retires before side-effect snapshot');
  const apiBeforeContextRejection = state.calls;
  const refsBeforeContextRejection = state.refs.size;
  const documentsBeforeContextRejection = state.documents.size;
  const checksBeforeContextRejection = state.checks.length;
  assert.equal((await deliver('v2-oversized-context',payload('member','x'.repeat(16384)))).status,413);
  assert.equal(state.calls,apiBeforeContextRejection,'complete context bound checked before external calls');
  assert.equal(state.refs.size,refsBeforeContextRejection,'invalid context never creates durable source');
  assert.equal(state.documents.size,documentsBeforeContextRejection,'invalid context never enters event index/journal');
  assert.equal(state.checks.length,checksBeforeContextRejection,'invalid context never creates a check');
  const commentBody = 'ping event-nonce\r\n  preserve indentation and 🦀\n';
  const commentEvent = payload('member',commentBody);
  const beforeCommentEvent = state.queue.length;
  assert.equal((await deliver('v2-comment-context',commentEvent)).status,200);
  await eventually(() => state.queue.length === beforeCommentEvent + 1,'signed comment event admission');
  const commentRequest = state.queue.at(-1);
  assert.equal(commentRequest.trigger,'event');
  assert.equal(commentRequest.golem,'echo');
  assert.equal(commentRequest.comment_id,commentEvent.comment.id);
  assert.equal(commentRequest.actor_id,commentEvent.sender.id);
  assert.equal(commentRequest.instructions,`GitHub event: issue_comment.created\n\nTriggering comment:\n${commentBody}`,'signed event comment preserved byte-for-byte as task context');
  const normalEventId = deliveryId('v2-comment-context');
  await eventually(() => state.refs.has(`refs/tags/golem-event-done/${normalEventId}`),'normal comment event retires');
  // Simulate valid unprepared work persisted by an older producer before the
  // complete-context bound existed. It must retire, not poison every later tick.
  const oldEventId = deliveryId('old-unprepared-oversized-context');
  const originalSource = state.tags.get(state.refs.get(`refs/tags/golem-event-done/${normalEventId}`));
  const oldWork = JSON.parse(originalSource.message);
  oldWork.delivery_id = oldEventId;
  oldWork.delivery.comment.body = 'x'.repeat(16384);
  oldWork.next = 0;
  oldWork.prepared = null;
  const oldSourceSha = String(state.tagId++).padStart(40,'0');
  state.tags.set(oldSourceSha,{...originalSource,sha:oldSourceSha,tag:`golem-event/${oldEventId}`,message:JSON.stringify(oldWork)});
  state.refs.set(`refs/tags/golem-event/${oldEventId}`,oldSourceSha);
  const oldPath = `/repos/${ops}/contents/events-v2/${oldEventId}.json`;
  state.documents.set(oldPath,{sha:String(state.tagId++).padStart(40,'0'),content:Buffer.from(JSON.stringify(oldWork)).toString('base64')});
  const indexPath = `/repos/${ops}/contents/events-v2.json`;
  const oldIndex = JSON.parse(Buffer.from(state.documents.get(indexPath).content,'base64'));
  oldIndex.ids.unshift(oldEventId);
  state.documents.set(indexPath,{sha:String(state.tagId++).padStart(40,'0'),content:Buffer.from(JSON.stringify(oldIndex)).toString('base64')});
  const echoChecksBeforeOld = state.checks.filter(c=>c.name==='golem/echo').length;
  await tick(Date.UTC(2026,10,7,12,1));
  assert.ok(!state.documents.has(oldPath),'permanently invalid old work retires');
  assert.equal(state.refs.get(`refs/tags/golem-event-done/${oldEventId}`),oldSourceSha,'rejection archives exact original source');
  assert.ok(!JSON.parse(Buffer.from(state.documents.get(indexPath).content,'base64')).ids.includes(oldEventId),'rejection frees event index capacity');
  assert.equal(state.checks.filter(c=>c.name==='golem/echo').length,echoChecksBeforeOld,'unprepared invalid context never creates a check');
  assert.ok(!state.queue.some(r=>r.delivery_id===deliveryId(`event-v2/${oldEventId}/echo`)),'invalid old work never admits a native request');
  // At the golem limit in a private repository, upkeep must not exhaust the event golem's budget.
  o.golems = new Map([['_golems.md', 'Rules.'], ...Array.from({length:19}, (_, i) => [`g${i}.md`, '---\n---\nTask.']), ['pusher.md', '---\ntriggers: [push]\nowner: member\n---\nOn push.']]);
  const callsBeforeFull = state.calls;
  assert.equal((await deliver('v2-onboard-full', push, true, false, 'push')).status, 200);
  const enqueued = () => state.queue.some(r => r.repository === onboardRepo && r.golem === 'pusher' && r.trigger === 'event');
  for (let attempt = 0; attempt < 6 && !enqueued(); attempt++) {
    await sleep(300);
    if (!enqueued()) await tick(Date.UTC(2026,10,8,12,attempt));
  }
  assert.ok(enqueued(), 'push-triggered golem admitted at the 20-golem limit');
  await eventually(() => o.patches === 2, 'dashboard also updated at the limit');
  assert.ok(state.calls > callsBeforeFull);
  await dashboardScenarios(o, push);
  await seatRegistration();
  // With a runner that executes Codex, an OpenAI Codex seat runs codex: as fallback and natively.
  files.set('foreign.md', '---\nharness: codex\nprovider: anthropic\n---\nReply with one short greeting.');
  files.set('codexer.md', '---\nharness: codex\nprovider: openai-codex\n---\nReply with one short greeting.');
  for (const [id, golem, fallback] of [['v2-codex-fallback', 'foreign', true], ['v2-codex-native', 'codexer', false]]) {
    assert.equal((await deliver(id, payload('codexonly', `@isp-golem-xiv ${golem}`))).status, 202);
    await eventually(() => state.queue.some(r => r.golem === golem && r.seat_login === 'codexonly'), `${golem} on a codex seat`);
    const request = state.queue.find(r => r.golem === golem && r.seat_login === 'codexonly');
    assert.equal(request.provider, 'openai-codex');
    assert.equal(request.harness, 'codex');
    if (fallback) assert.equal(request.fallback, 'asked for anthropic/codex, used openai-codex/codex with its default model, because there is no anthropic seat');
    else assert.ok(!('fallback' in request), 'native codex choice carries no fallback');
  }
  console.log('Built Workers artifact: released v1 regressions; v2 mention/config, signed comment context and bounded rejection/recovery, native HMAC, stop handoff, bot/member owner selection, twenty-event restart continuation, real scheduled handler, selected identity and reordered installation continuation passed.');
} finally {await worker.dispose();}

// Last results survive push re-renders; run boxes start runs like mentions, on the ticker's seat.
async function dashboardScenarios(o, push) {
  const board = () => o.issues[0];
  const lastMarker = '  <!-- golem-last golem=g0 outcome=replied seat=member/anthropic harness=claude model=claude-opus-5-5 at=2026-11-13T10:04Z run=4242 -->';
  const lastLine = '  Last: replied on member/anthropic, claude, model claude-opus-5-5, 2026-11-13 10:04 UTC, [log](https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/4242)';
  assert.match(board().body, /^  <!-- golem-last golem=g0 -->\n  Last: never run\n  - \[ \] Run `g0` now <!-- golem-run golem=g0 -->$/m);
  // What a runner writes after a run; a forged result for another golem is not carried over.
  board().body = board().body.replace('  <!-- golem-last golem=g0 -->\n  Last: never run', `${lastMarker}\n${lastLine}`)
    .replace('  <!-- golem-last golem=g1 -->', '  <!-- golem-last golem=g1 outcome=pwned seat=member/anthropic harness=claude model=m at=2026-11-13T10:04Z run=1 -->');
  o.golems = new Map([['_golems.md', 'Rules.'], ['g0.md', '---\n---\nTask.'], ['g1.md', '---\n---\nTask.'], ['hello.md', '---\n---\nSay hello.']]);
  const patches = o.patches;
  assert.equal((await deliver('v2-dashboard-rerender', push, true, false, 'push')).status, 200);
  await eventually(() => o.patches === patches + 1, 'dashboard re-rendered after .golems/ change');
  assert.ok(board().body.includes(`${lastMarker}\n${lastLine}\n  - [ ] Run \`g0\` now`), 'push re-render preserves the last result');
  assert.ok(board().body.includes('  <!-- golem-last golem=g1 -->\n  Last: never run\n'), 'invalid results are not carried over');
  assert.ok(!board().body.includes('`pusher`'));
  const edited = (sender, from, to) => ({action:'edited', sender, repository:{id:150, full_name:onboardRepo}, installation:{id:153964219}, issue:{number:board().number, title:'Golems', user:{login:'isp-golem-xiv[bot]'}, body:to}, changes:{body:{from}}});
  const member = {id:42, login:'member', type:'User'};
  const tickBox = (body, golem) => body.replace(`  - [ ] Run \`${golem}\` now`, `  - [x] Run \`${golem}\` now`);
  const runs = () => state.queue.filter(r => r.repository === onboardRepo && r.trigger === 'dashboard');
  const clean = board().body;
  // A person ticks a box: GitHub stores the ticked body, then delivers the edit.
  const ticked = tickBox(clean, 'hello');
  board().body = ticked;
  const tickEvent = edited(member, clean, ticked);
  assert.equal((await deliver('v2-dashboard-tick', tickEvent, true, false, 'issues')).status, 202);
  await eventually(() => runs().length === 1, 'run box admits a dashboard run');
  const run = runs()[0];
  assert.deepEqual(
    {golem:run.golem, issue:run.issue_number, comment:run.comment_id, actor:run.actor_login, actorId:run.actor_id, seat:run.seat_login, provider:run.provider, harness:run.harness, instructions:run.instructions, version:run.version},
    {golem:'hello', issue:board().number, comment:null, actor:'member', actorId:42, seat:'member', provider:'anthropic', harness:'claude', instructions:'Started from the dashboard run box.', version:2},
  );
  assert.equal(run.delivery_id.length, 36);
  assert.equal(state.checks.find(check => check.external_id === run.delivery_id)?.name, 'golem/hello', 'queued check run like a mention');
  await eventually(() => board().body === clean, 'run box cleared, everything else kept');
  const attempts = () => state.enqueueAttempts.filter(r => r.delivery_id === run.delivery_id).length;
  const attemptsBefore = attempts();
  assert.equal((await deliver('v2-dashboard-tick', tickEvent, true, false, 'issues')).status, 202);
  await sleep(300);
  await tick(Date.UTC(2026,10,9,12));
  assert.equal(runs().length, 1, 'redelivery never dispatches again');
  assert.equal(attempts(), attemptsBefore, 'redelivery never re-admits');
  // The App's own edit and other bots never start runs; the App's edit costs no API call.
  const calls = state.calls;
  assert.equal((await deliver('v2-dashboard-app', edited({id:9, login:'isp-golem-xiv[bot]', type:'Bot'}, clean, tickBox(clean, 'hello')), true, false, 'issues')).status, 200);
  assert.equal(state.calls, calls, 'App edits are ignored before any API call');
  assert.equal((await deliver('v2-dashboard-bot', edited({id:7, login:'dependabot[bot]', type:'Bot'}, clean, tickBox(clean, 'hello')), true, false, 'issues')).status, 200);
  // An unrelated edit by a member is not a command.
  assert.equal((await deliver('v2-dashboard-unrelated', edited(member, clean, clean.replace('kept current by', 'kept up to date by')), true, false, 'issues')).status, 200);
  await sleep(300);
  assert.equal(runs().length, 1, 'bot and unrelated edits start nothing');
  assert.equal(o.comments.length, 0, 'and answer nothing');
  // Refusals answer the ticker in the dashboard issue and clear the boxes.
  for (const [id, sender, golem, reason] of [
    ['v2-dashboard-outsider', {id:77, login:'outsider', type:'User'}, 'hello', 'not-member'],
    ['v2-dashboard-noseat', {id:43, login:'noseat', type:'User'}, 'g0', 'no-seat'],
  ]) {
    const box = tickBox(clean, golem);
    board().body = box;
    const comments = o.comments.length;
    assert.equal((await deliver(id, edited(sender, clean, box), true, false, 'issues')).status, 202);
    await eventually(() => o.comments.length === comments + 1, `${reason} refusal`);
    const refusal = o.comments.at(-1);
    assert.equal(refusal.issue, board().number);
    assert.ok(refusal.body.startsWith(`<!-- isp-golem-xiv kind=refused reason=${reason} -->\n@${sender.login}, the run box of \`${golem}\` did not start a run.`), refusal.body);
    await eventually(() => board().body === clean, `${reason} box cleared`);
    assert.equal(runs().length, 1, `${reason} never enqueues`);
  }
}

// Runs last: registering changes member's seats and preferred seat, which earlier scenarios pin.
async function seatRegistration() {
  const router = 'https://isp-golem-xiv.isp-insoft-gmbh.workers.dev';
  const policy = "default-src 'none'; script-src 'self'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'";
  const signIn = await worker.dispatchFetch(`${base}/seats`, {redirect:'manual'});
  const oauthState = new URL(signIn.headers.get('Location')).searchParams.get('state');
  const stateCookie = `golem_oauth_state=${oauthState}`;
  const callback = (code, query, cookie) => worker.dispatchFetch(`${base}/auth/github/callback?code=${code}&state=${query}`, {redirect:'manual', headers:cookie === undefined ? {} : {Cookie:cookie}});
  const exchangesBefore = state.oauthExchanges.length;
  for (const [query, cookie, why] of [[crypto.randomUUID(), stateCookie, 'state mismatch'], [oauthState, undefined, 'no state cookie'], [oauthState, `${stateCookie}; ${stateCookie}`, 'duplicate state cookie'], [`${oauthState}&state=${oauthState}`, stateCookie, 'duplicate state']]) {
    const refused = await callback('member-code', query, cookie);
    assert.equal(refused.status, 400, why);
    assert.equal(refused.headers.get('Set-Cookie'), null, why);
  }
  assert.equal(state.oauthExchanges.length, exchangesBefore, 'state is checked before any code exchange');
  let response = await callback('forged-code', oauthState, stateCookie);
  assert.equal(response.status, 403, 'GitHub rejected the code');
  response = await callback('outsider-code', oauthState, stateCookie);
  assert.equal(response.status, 403, 'non-member rejected');
  assert.equal(response.headers.get('Set-Cookie'), null, 'non-member gets no session');
  assert.ok(!(await response.text()).includes('oauth-outsider-token'));
  response = await callback('member-code', oauthState, stateCookie);
  assert.equal(response.status, 302, 'member signs in');
  assert.equal(response.headers.get('Location'), '/seats');
  assert.deepEqual(state.oauthExchanges.at(-1), {client_id:'Iv23liEnfLEVCSS2aUdR', client_secret:'fixture-client-secret', code:'member-code', redirect_uri:`${router}/auth/github/callback`});
  const cookies = response.headers.getSetCookie();
  assert.equal(cookies.length, 2);
  const sessionCookie = cookies.find(cookie => cookie.startsWith('golem_session='));
  assert.match(sessionCookie, /^golem_session=member\.\d+\.[0-9a-f]{64}; Path=\/; Max-Age=3600; HttpOnly; Secure; SameSite=Lax$/);
  assert.ok(!sessionCookie.includes('oauth-member-token'), 'user token is never stored');
  assert.ok(cookies.includes('golem_oauth_state=; Path=/auth/github; Max-Age=0; HttpOnly; Secure; SameSite=Lax'), 'state cookie cleared on its own path');
  const session = sessionCookie.split(';')[0];
  const tampered = session.replace('golem_session=member.', 'golem_session=other.');
  response = await worker.dispatchFetch(`${base}/seats`, {redirect:'manual', headers:{Cookie:tampered}});
  assert.equal(response.status, 302, 'tampered session signs in again');
  assert.match(response.headers.get('Location'), /^https:\/\/github\.com\/login\/oauth\/authorize\?/);
  const page = async () => {
    const readsBefore = state.secretReads.length;
    const seats = await worker.dispatchFetch(`${base}/seats`, {headers:{Cookie:session}});
    assert.equal(seats.status, 200);
    assert.equal(seats.headers.get('Content-Security-Policy'), policy);
    assert.match(seats.headers.get('Cache-Control'), /no-store/);
    assert.deepEqual(state.secretReads.slice(readsBefore), ['SEAT_MEMBER__ANTHROPIC', 'SEAT_MEMBER__OPENAI_CODEX'], 'only own seat names are read');
    return seats.text();
  };
  let html = await page();
  assert.match(html, /<li data-seat="anthropic">/);
  assert.ok(!html.includes('data-seat="openai-codex"') && !html.toLowerCase().includes('codexonly'), 'only own seats listed');
  assert.ok(!/<script(?![^>]*src="\/seats\.js")/.test(html), 'no inline script');
  const csrf = html.match(/<meta name="golem-csrf" content="([0-9a-f]{64})">/)[1];
  response = await worker.dispatchFetch(`${base}/admin`, {headers:{Cookie:session}});
  assert.equal(response.status, 200);
  assert.match(await response.text(), /Slice 4/);
  assert.equal((await worker.dispatchFetch(`${base}/v1/seats/key`)).status, 401);
  response = await worker.dispatchFetch(`${base}/v1/seats/key`, {headers:{Cookie:session}});
  assert.equal(response.status, 200);
  const key = await response.json();
  assert.deepEqual(key, {key_id:'568250167242549743', key:sealedFixture.recipientPublic});
  // The served script seals byte-identically to libsodium; it is evaluated as delivered, with a page-like self.
  response = await worker.dispatchFetch(`${base}/seats.js`);
  assert.match(response.headers.get('Content-Type'), /^text\/javascript/);
  assert.equal(response.headers.get('Content-Security-Policy'), policy);
  const script = await response.text();
  assert.ok(script.includes('Permission is hereby granted') && script.includes('public domain'), 'vendored licenses ship with the code');
  const browser = {crypto: globalThis.crypto};
  new Function('self', script)(browser);
  const {nacl, golemSeal: seal, golemBase64: base64} = browser;
  const scalar = label => new Uint8Array(createHash('sha256').update(label).digest());
  const recipient = nacl.box.keyPair.fromSecretKey(scalar(sealedFixture.recipientSecret));
  assert.equal(Buffer.from(recipient.publicKey).toString('base64'), sealedFixture.recipientPublic);
  const message = new TextEncoder().encode(sealedFixture.message);
  const deterministic = seal(message, recipient.publicKey, nacl.box.keyPair.fromSecretKey(scalar(sealedFixture.ephemeralSecret)));
  assert.equal(base64.toBase64(deterministic), sealedFixture.sealed, 'byte-identical to libsodium crypto_box_seal');
  const open = text => {
    const sealed = base64.fromBase64(text);
    const ephemeral = sealed.subarray(0, 32);
    const context = browser.golemBlake2b.blake2bInit(24);
    browser.golemBlake2b.blake2bUpdate(context, Uint8Array.from([...ephemeral, ...recipient.publicKey]));
    const opened = nacl.box.open(sealed.subarray(32), browser.golemBlake2b.blake2bFinal(context), ephemeral, recipient.secretKey);
    return opened && new TextDecoder().decode(opened);
  };
  const first = base64.toBase64(seal(message, base64.fromBase64(key.key)));
  const second = base64.toBase64(seal(new TextEncoder().encode('replacement-sign-in'), base64.fromBase64(key.key)));
  assert.notEqual(first.slice(0, 44), second.slice(0, 44), 'fresh ephemeral key per seal');
  assert.equal(open(first), sealedFixture.message);
  // Writes: same origin, a session, its CSRF token, a valid request and the current key.
  const post = (path, body, change = {}) => {
    const headers = {'Content-Type':'application/json', Origin:router, Cookie:session, 'X-Golem-Csrf':csrf, ...change};
    for (const name of Object.keys(headers)) if (headers[name] === undefined) delete headers[name];
    return worker.dispatchFetch(`${base}${path}`, {method:'POST', body:typeof body === 'string' ? body : JSON.stringify(body), headers});
  };
  const registration = {provider:'openai-codex', key_id:key.key_id, encrypted_value:first};
  const writesBefore = state.seatWrites.length;
  for (const [path, body, change, status, why] of [
    ['/v1/seats', registration, {'X-Golem-Csrf':undefined}, 403, 'missing CSRF token'],
    ['/v1/seats', registration, {'X-Golem-Csrf':'0'.repeat(64)}, 403, 'wrong CSRF token'],
    ['/v1/seats', registration, {Origin:'https://evil.invalid'}, 403, 'foreign origin'],
    ['/v1/seats', registration, {Origin:undefined}, 403, 'missing origin'],
    ['/v1/seats', registration, {Cookie:undefined}, 401, 'no session'],
    ['/v1/seats', registration, {Cookie:tampered}, 401, 'tampered session'],
    ['/v1/seats', {...registration, provider:'openai'}, {}, 400, 'bad provider'],
    ['/v1/seats', {...registration, encrypted_value:'not base64!'}, {}, 400, 'not base64'],
    ['/v1/seats', {...registration, extra:true}, {}, 400, 'unknown field'],
    ['/v1/seats', {...registration, key_id:'1'}, {}, 409, 'stale key'],
    ['/v1/seats', 'x'.repeat(70_000), {}, 413, 'oversized body'],
  ]) {
    response = await post(path, body, change);
    assert.equal(response.status, status, why);
    assert.ok(!(await response.text()).includes(first), `${why}: never echoes the value`);
  }
  state.removedMembers.add('member');
  assert.equal((await post('/v1/seats', registration)).status, 403, 'a session outliving membership cannot write');
  state.removedMembers.delete('member');
  assert.equal(state.seatWrites.length, writesBefore, 'refused requests never write');
  const variablesBeforeSeat = state.variableWrites.length;
  assert.equal((await post('/v1/seats/preferred', {provider:'openai-codex'})).status, 409, 'no seat at that provider');
  assert.equal(state.variableWrites.length, variablesBeforeSeat, 'a refused preference never writes');
  response = await post('/v1/seats', registration);
  assert.equal(response.status, 200);
  const registered = await response.text();
  assert.deepEqual(JSON.parse(registered), {seat:'SEAT_MEMBER__OPENAI_CODEX', replaced:false});
  assert.ok(!registered.includes(first));
  assert.deepEqual(state.seatWrites.at(-1), {name:'SEAT_MEMBER__OPENAI_CODEX', body:{encrypted_value:first, key_id:key.key_id}}, 'exact ciphertext relayed');
  assert.equal(open(state.seatWrites.at(-1).body.encrypted_value), sealedFixture.message);
  response = await post('/v1/seats', {...registration, encrypted_value:second});
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {seat:'SEAT_MEMBER__OPENAI_CODEX', replaced:true}, 'registering again replaces');
  assert.deepEqual(state.seatWrites.slice(writesBefore).map(write => write.name), ['SEAT_MEMBER__OPENAI_CODEX', 'SEAT_MEMBER__OPENAI_CODEX']);
  assert.equal(open(state.seatWrites.at(-1).body.encrypted_value), 'replacement-sign-in');
  html = await page();
  assert.match(html, /<li data-seat="openai-codex">/);
  // Preferred seat: only a provider the person has a seat at; created once, then updated.
  const variableWrites = state.variableWrites.length;
  assert.equal((await post('/v1/seats/preferred', {provider:'other'})).status, 400);
  assert.equal((await post('/v1/seats/preferred', {provider:'anthropic'}, {'X-Golem-Csrf':'0'.repeat(64)})).status, 403);
  assert.equal(state.variableWrites.length, variableWrites, 'refused preferences never write');
  response = await post('/v1/seats/preferred', {provider:'anthropic'});
  assert.equal(response.status, 200);
  assert.deepEqual(state.variableWrites.slice(variableWrites).map(write => write.method), ['PATCH', 'POST'], 'created when missing');
  assert.deepEqual(state.variableWrites.at(-1).body, {name:'SEAT_PREFERRED_MEMBER', value:'anthropic'});
  response = await post('/v1/seats/preferred', {provider:'openai-codex'});
  assert.equal(response.status, 200);
  assert.equal(state.variableWrites.at(-1).method, 'PATCH', 'updated when present');
  assert.equal(state.variables.get('SEAT_PREFERRED_MEMBER'), 'openai-codex');
  assert.match(await page(), /<option value="openai-codex" selected>/);
  console.log('Seat registration: OAuth state/membership/session, own seats only, CSRF/origin, libsodium-identical sealing, exact relay, replacement and preferred seat passed.');
}
