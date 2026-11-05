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
const state = {calls: 0, dispatches: [], checks: [], comments: [], reactions: [], refs: new Map(), tags: new Map(), tagId: 1, queue: [], enqueueAttempts: [], internal: [], documents: new Map()};
let protocol2 = false;
let installation = [{id:99, full_name:repository, default_branch:'trunk', topics:['isp-golem-xiv-test']}];
const repository2 = 'isp-insoft-gmbh/isp-golem-xiv-test-second';
const repository3 = 'isp-insoft-gmbh/isp-golem-xiv-test-late';
const repositoryFiles = new Map([[repository, files]]);
const metadata = name => ({id:name === repository ? 99 : name === repository2 ? 100 : 101, full_name:name, default_branch:'trunk', topics:allowed ? ['isp-golem-xiv-test'] : []});
let allowed = true;
let dispatchStatus = 204;
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
  if (path.startsWith('/orgs/isp-insoft-gmbh/memberships/')) return path.endsWith('/outsider') ? answer(404, {}) : answer(200, {state:'active'});
  if (path.startsWith(`/repos/${ops}/actions/secrets/`)) return answer(path.endsWith('SEAT_MEMBER__ANTHROPIC') ? 200 : 404, {});
  if (path.endsWith('/reactions')) {state.reactions.push({path, ...body}); return answer(201, {id:1});}
  if (path.endsWith('/check-runs')) {state.checks.push(body); return answer(201, {id:state.checks.length});}
  if (path === `/repos/${ops}/actions/workflows/runner.yml/dispatches`) {state.dispatches.push(body); return answer(dispatchStatus, dispatchStatus === 204 ? null : {});}
  if (path === `/repos/${repository}/issues/1/comments`) {state.comments.push(body.body); return answer(201, {id:state.comments.length});}
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
  assert.equal((await deliver('v2-stop',payload('member','@isp-golem-xiv stop'))).status,202);
  await eventually(() => state.internal.some(body => body.action === 'stop'), 'thread stop handoff');
  assert.deepEqual(state.internal.find(body => body.action === 'stop'),{action:'stop',repository,issue_number:1});
  assert.equal(state.queue.length,1,'stop never enqueues or prematurely releases');
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
  console.log('Built Workers artifact: released v1 regressions; v2 mention/config, signed comment context, native HMAC, stop handoff, bot/member owner selection, twenty-event restart continuation, real scheduled handler, selected identity and reordered installation continuation passed.');
} finally {await worker.dispose();}
