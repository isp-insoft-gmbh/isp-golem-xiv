// Standalone real SQLite DO tests, independent of Wasm builds.
// node router/tests/coordinator.mjs [absolute miniflare module path] [--runner-contract]
// --runner-contract additionally compiles the actual native audit publisher in OS temp.
import assert from 'node:assert/strict';
import {generateKeyPairSync, randomUUID} from 'node:crypto';
import {copyFile, mkdtemp, mkdir, readFile, rm, writeFile} from 'node:fs/promises';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';

const miniflarePath = process.argv.slice(2).find(argument => !argument.startsWith('--'));
const {Miniflare, Response: RuntimeResponse} = await import(miniflarePath ? pathToFileURL(miniflarePath).href : 'miniflare');
const Answer = RuntimeResponse ?? Response;
const source = await readFile(new URL('../src/coordinator.js', import.meta.url), 'utf8');
const runtime = await readFile(new URL('../src/runtime.js', import.meta.url), 'utf8');
const key = generateKeyPairSync('rsa', {modulusLength: 2048}).privateKey.export({type: 'pkcs8', format: 'pem'});
const ops = '/repos/isp-insoft-gmbh/isp-golem-xiv-ops';
const repository = 'isp-insoft-gmbh/isp-golem-xiv-test-queue';
const head = 'a'.repeat(40);
const json = (status, value = {}) => new Answer(status === 204 ? null : JSON.stringify(value), {status, headers: {'Content-Type': 'application/json'}});
function request(seat = 'member', golem = 'hello', extra = {}) {
  return {version: 2, delivery_id: randomUUID(), repository_id: 99, repository, head_sha: head, golem, issue_number: 1, comment_id: 100, actor_id: 42, actor_login: 'member', seat_login: seat, provider: 'anthropic', harness: 'claude', model: null, timeout_seconds: 1800, instructions: 'fixture', check_run_id: 10, trigger: 'mention', branch: `golems/${golem}`, verify: null, owner: null, ...extra};
}
class Fixture {
  constructor() {
    this.refs = new Map();
    this.tags = new Map();
    this.runs = new Map();
    this.dispatches = [];
    this.repositories = new Map([[repository, {status: 200, body: {id: 99, full_name: repository}}]]);
    this.repositoryGets = [];
    this.checkGets = [];
    this.reactions = [];
    this.checks = [];
    this.checkState = new Map();
    this.cancels = [];
    this.counter = 1;
    this.index = null;
    this.dispatchStatus = 204;
    this.failWrite = null;
    this.puts = [];
  }
  sha() { return (this.counter++).toString(16).padStart(40, '0'); }
  queue() { return JSON.parse(Buffer.from(this.index.content, 'base64')); }
  row(id) { return this.queue().records.find(row => row.id === id); }
  async github(req) {
    assert.equal(req.headers.get('X-GitHub-Api-Version'), '2022-11-28');
    this.origins ??= [];
    this.origins.push(new URL(req.url).origin);
    assert.equal(new URL(req.url).origin, 'https://api.github.com');
    const url = new URL(req.url);
    const p = url.pathname;
    const bodyText = ['POST', 'PUT', 'PATCH'].includes(req.method) ? await req.text() : '';
    const body = bodyText ? JSON.parse(bodyText) : null;
    if (this.redirect && p === '/app/installations/153964219/access_tokens') return new Answer(null, {status: 302, headers: {Location: 'https://must-not-receive-token.invalid/'}});
    if (p === '/app/installations/153964219/access_tokens') {
      assert.deepEqual(body, {}, 'installation token must not narrow repositories or permissions');
      return json(201, {token: 'synthetic-installation-token'});
    }
    if (req.method === 'GET' && /^\/repos\/[^/]+\/[^/]+$/.test(p)) {
      const name = p.slice('/repos/'.length);
      this.repositoryGets.push(name);
      assert.ok(this.repositories.has(name), `Missing repository GET fixture: ${name}`);
      const response = this.repositories.get(name);
      return json(response.status, response.body);
    }
    if (p === `${ops}/contents/queue.json`) {
      if (req.method === 'GET') {
        if (this.failReadback) { this.failReadback = false; return json(503); }
        if (this.readbackMode && this.readbackReady) {
          const mode = this.readbackMode; this.readbackMode = null;
          if (mode === 'missing') return json(404);
          if (mode === 'malformed') return json(200, {...this.index, encoding: 'base64', content: '!!!!'});
          if (mode === 'oversize') return json(200, {...this.index, encoding: 'base64', content: 'A'.repeat(300000)});
          return json(200, {...this.index, sha: 'f'.repeat(40), encoding: 'base64'});
        }
        return this.index ? json(200, {...this.index, encoding: 'base64'}) : json(404);
      }
      assert.equal(body.branch, 'golem-state');
      if (this.conflicts) { this.conflicts--; this.readbackReady = true; return json(409); }
      if (this.failWrite === 'conflict') { this.failWrite = null; return json(422, {message: 'sensitive fixture error'}); }
      if (body.sha !== this.index?.sha) return json(409);
      const records = JSON.parse(Buffer.from(body.content, 'base64')).records;
      for (const record of records) {
        if (record.state === 'terminal' && record.auditTag) {
          assert.equal(this.refs.get(`refs/tags/golem-audit/${record.id}`), record.auditTag, 'durable private audit ref must precede terminal journal write');
          assert.ok(this.tags.has(record.auditTag));
        }
      }
      if (this.rejectTerminal && records.some(r => r.state === 'terminal')) return json(422);
      const fail = this.failWrite && (this.failWrite.state === undefined || records.some(r => r.state === this.failWrite.state));
      this.index = {sha: this.sha(), content: body.content};
      this.puts.push(structuredClone(records));
      if (this.transportAfterCommit && records.some(r => r.state === 'bound')) {
        this.transportAfterCommit = false;
        this.transportCommittedBytes = this.index.content;
        throw new Error('synthetic committed PUT transport loss');
      }
      if (fail) { this.failReadback = !this.confirmReadback; this.failWrite = null; return json(500, {message: 'credential must not escape'}); }
      return json(200, {content: {sha: this.index.sha}});
    }
    if (p === `${ops}/git/ref/heads/trunk`) return json(200, {object: {sha: head, type: 'commit'}});
    if (p === `${ops}/git/commits/${head}`) return json(200, {tree: {sha: head}});
    if (p === `${ops}/git/trees`) {
      this.initial = body.tree[0].content;
      return json(201, {sha: this.sha()});
    }
    if (p === `${ops}/git/commits`) return json(201, {sha: this.sha()});
    if (p === `${ops}/git/refs`) {
      if (this.failAudit && body.ref.startsWith('refs/tags/golem-audit/')) return json(typeof this.failAudit === 'number' ? this.failAudit : 503);
      if (this.refs.has(body.ref)) return json(422);
      this.refs.set(body.ref, body.sha);
      if (this.loseAuditRef && body.ref.startsWith('refs/tags/golem-audit/')) { this.loseAuditRef = false; return json(503); }
      if (this.failReceipt && body.ref.startsWith('refs/tags/golem-terminal/')) { this.failReceipt = false; return json(500); }
      if (body.ref === 'refs/heads/golem-state') {
        this.index = {sha: this.sha(), content: Buffer.from(this.initial).toString('base64')};
        if (this.failBootstrap) { this.failBootstrap = false; return json(500); }
      }
      return json(201, {object: {sha: body.sha}});
    }
    if (p.startsWith(`${ops}/git/ref/`)) {
      const ref = `refs/${p.slice(`${ops}/git/ref/`.length)}`;
      const sha = this.refs.get(ref);
      return sha ? json(200, {object: {sha, type: ref.startsWith('refs/tags/') ? 'tag' : 'commit'}}) : json(404);
    }
    if (p === `${ops}/git/tags`) {
      const sha = this.sha();
      this.tags.set(sha, {...body, object: {type: body.type, sha: body.object}, sha});
      return json(201, {sha});
    }
    if (p.startsWith(`${ops}/git/tags/`)) return json(200, this.tags.get(p.split('/').at(-1)));
    if (p === `${ops}/actions/workflows/runner.yml/runs`) {
      const runs = [...this.runs.values()].filter(r => r.status === url.searchParams.get('status'));
      return json(200, this.malformedList ? {workflow_runs: []} : {total_count: runs.length, workflow_runs: runs});
    }
    if (p === `${ops}/actions/workflows/runner.yml/dispatches`) {
      const sent = JSON.parse(body.inputs.request);
      assert.equal(body.ref, 'trunk');
      assert.equal(body.inputs.repo, sent.repository);
      assert.equal(body.inputs.golem, sent.golem);
      assert.ok(this.row(sent.delivery_id).dispatched, 'journal dispatch intent must precede API dispatch');
      assert.equal(this.row(sent.delivery_id).state, 'dispatching');
      assert.equal(sent.queue_generation, this.row(sent.delivery_id).generation);
      this.dispatches.push(sent);
      return json(this.dispatchStatus);
    }
    const runMatch = p.match(/\/actions\/runs\/(\d+)(\/cancel)?$/);
    if (runMatch) {
      const id = Number(runMatch[1]);
      if (runMatch[2]) { this.cancels.push(id); return json(202); }
      return this.runs.has(id) ? json(200, this.runs.get(id)) : json(404);
    }
    if (p.endsWith('/reactions')) { this.reactions.push(body); return json(201); }
    if (p.includes('/check-runs/')) {
      const id = Number(p.split('/').at(-1));
      if (req.method === 'GET') {
        this.checkGets.push({repository: p.split('/').slice(2, 4).join('/'), id});
        return this.checkMissing === id || !this.checkState.has(id) ? json(404) : json(200, this.checkState.get(id));
      }
      if (this.checkFailure === id) return json(404);
      this.onCheckPatch?.(id, body);
      this.checks.push(body);
      this.checkState.set(id, {...this.checkState.get(id), ...body});
      return json(200);
    }
    assert.fail(`Unexpected GitHub API ${req.method} ${p}`);
  }
  async start() {
    this.worker = new Miniflare({workers: [{config: {
      name: 'coordinator-fixture', compatibilityDate: '2026-10-06',
      manifest: {mainModule: 'entry.js', modules: {
        'entry.js': {type: 'esm', contents: `export {Coordinator} from './coordinator.js'; export default {fetch(request, env) { return env.COORDINATOR.get(env.COORDINATOR.idFromName('isp-insoft-gmbh')).fetch(request); }};`},
        'coordinator.js': {type: 'esm', contents: source},
        'runtime.js': {type: 'esm', contents: runtime},
      }},
      exports: {Coordinator: {type: 'durable-object', storage: 'sqlite'}},
      env: {APP_PRIVATE_KEY: {type: 'text', value: key}, COORDINATOR: {type: 'durable-object', worker: 'coordinator-fixture', exportName: 'Coordinator'}},
    }, dev: {outboundService: {type: 'fetcher', handler: req => this.github(req)}}}]});
  }
  async call(action, fields = {}, status = 200) {
    const response = await this.worker.dispatchFetch('https://queue.invalid/', {method: 'POST', body: JSON.stringify({action, ...fields})});
    const body = await response.json();
    assert.equal(response.status, status, JSON.stringify(body));
    return body;
  }
  async enqueue(req, status = 200) {
    if (!this.checkState.has(req.check_run_id)) this.checkState.set(req.check_run_id, {id: req.check_run_id, name: `golem/${req.golem}`, app: {id: 4605060}, status: 'queued', conclusion: null, output: null});
    return this.call('enqueue', {request: req}, status);
  }
  async bind(req, id = 1000 + this.runs.size, fields = {}, status = 200) {
    const row = this.row(req.delivery_id);
    this.runs.set(id, {id, run_attempt: 1, status: 'in_progress', event: 'workflow_dispatch', head_branch: 'trunk', path: '.github/workflows/runner.yml', display_title: `${req.repository} ${req.golem}`});
    return this.call('bind', {request_id: row.id, generation: row.generation, run_id: id, attempt: 1, ...fields}, status);
  }
  async complete(req, fields = {}, status = 200) {
    const row = this.row(req.delivery_id);
    // Real supervisor completion is sent only after destruction/publication. Mark run done in the double.
    const run = this.runs.get(row.run_id);
    if (run) Object.assign(run, {status: 'completed', conclusion: 'success'});
    return this.call('complete', {request_id: row.id, generation: row.generation, run_id: row.run_id, attempt: row.attempt, ...fields}, status);
  }
  async restart() { await this.worker.dispose(); await this.start(); }
  async dispose() { await this.worker.dispose(); }
}
const scenarios = [];
async function scenario(name, fn) { scenarios.push({name, fn}); }
async function runScenarios() {
  const started = performance.now();
  const timings = [];
  // Independent GitHub fixtures, SQLite stores and real process restarts remain isolated.
  // Batch startup/teardown I/O instead of serializing dozens of workerd processes.
  const batches = Array.from({length: 4}, async () => {
    while (scenarios.length) {
      const {name, fn} = scenarios.shift();
      const fixture = new Fixture();
      const start = performance.now();
      await fixture.start();
      try { await fn(fixture); console.log(`PASS ${name}`); }
      finally { await fixture.dispose(); timings.push({name, ms: Math.round(performance.now() - start)}); }
    }
  });
  const results = await Promise.allSettled(batches);
  console.log(JSON.stringify({coordinatorTiming: {totalMs: Math.round(performance.now() - started), cases: timings.length, timings, slowest: [...timings].sort((a, b) => b.ms - a.ms).slice(0, 8)}}));
  for (const result of results) if (result.status === 'rejected') throw result.reason;
}

await scenario('unchanged blob conflict retries identical mutation and lost response confirms exact bytes before grant', async f => {
  await f.call('tick');
  f.conflicts = 1;
  const req = request(); await f.enqueue(req);
  assert.equal(f.dispatches.length, 1);
  f.confirmReadback = true;
  f.failWrite = {state: 'bound'};
  assert.equal((await f.bind(req)).state, 'granted');
  await f.restart();
  assert.equal((await f.bind(req, 1000)).state, 'granted');
  assert.equal(f.dispatches.length, 1);
});
await scenario('committed PUT transport exception requires exact readback before grant and restart duplicate bind', async f => {
  const req = request(); await f.enqueue(req);
  f.transportAfterCommit = true;
  assert.equal((await f.bind(req, 7100)).state, 'granted');
  assert.equal(f.index.content, f.transportCommittedBytes);
  assert.equal(f.row(req.delivery_id).state, 'bound');
  assert.equal(f.dispatches.length, 1);
  await f.restart();
  assert.equal((await f.bind(req, 7100)).state, 'granted');
  assert.equal(f.dispatches.length, 1);
});
for (const mode of ['divergence', 'malformed', 'missing', 'oversize', 'exhausted']) {
  await scenario(`journal ${mode} readback never grants or dispatches`, async f => {
    await f.call('tick');
    const before = f.index.content;
    f.conflicts = 3;
    if (mode !== 'exhausted') f.readbackMode = mode;
    const result = await f.enqueue(request(), 503);
    assert.equal(result.safeFailure, 'journal_conflict');
    assert.equal(f.index.content, before);
    assert.equal(f.dispatches.length, 0);
    assert.ok(f.conflicts >= 0);
  });
}
await scenario('bind missing metadata is transient not identity conflict', async f => {
  const req = request(); await f.enqueue(req);
  const row = f.row(req.delivery_id);
  const result = await f.call('bind', {request_id: row.id, generation: row.generation, run_id: 999, attempt: 1}, 503);
  assert.equal(result.safeFailure, 'run_missing');
  assert.equal(f.row(req.delivery_id).state, 'dispatching');
});
for (const failure of [null, 'audit', '403', '500', 'identity', 'attempt', 'healthy-target']) {
  await scenario(`failed completed unavailable target ${failure ?? 'audit succeeds'} retains unless privately audited`, async f => {
    const req = request(); await f.enqueue(req); await f.bind(req, 7000);
    const peerRepo = 'isp-insoft-gmbh/isp-golem-xiv-test-peer';
    f.repositories.set(peerRepo, {status: 200, body: {id: 100}});
    const peer = request('other', 'peer', {repository: peerRepo, repository_id: 100, check_run_id: 11});
    f.runs.set(7999, {id: 7999, status: 'queued'});
    await f.enqueue(peer);
    assert.equal(f.row(peer.delivery_id).state, 'pending');
    assert.equal(f.dispatches.filter(r => r.delivery_id === peer.delivery_id).length, 0);
    f.runs.delete(7999);
    Object.assign(f.runs.get(7000), {status: 'completed', conclusion: 'failure'});
    if (failure === 'identity') f.runs.get(7000).display_title = 'wrong';
    if (failure === 'attempt') f.runs.get(7000).run_attempt = 2;
    f.checkMissing = req.check_run_id;
    f.repositories.set(repository, {status: failure === '403' ? 403 : failure === '500' ? 500 : failure === 'healthy-target' ? 200 : 404, body: {id: 99}});
    f.failAudit = failure === 'audit';
    await f.restart(); await f.call('tick');
    const row = f.row(req.delivery_id);
    assert.equal(row.state, failure ? 'bound' : 'terminal');
    assert.equal(f.dispatches.filter(r => r.delivery_id === peer.delivery_id).length, 1);
    assert.equal(f.row(peer.delivery_id).state, 'dispatching', 'peer must advance during reconciliation tick');
    if (!failure) {
      assert.equal(f.refs.get(`refs/tags/golem-audit/${req.delivery_id}`), row.auditTag);
      const audit = JSON.parse(f.tags.get(row.auditTag).message);
      assert.equal(audit.run_id, 7000); assert.equal(audit.model, 'unobserved');
      assert.equal(audit.reason, 'target unavailable to installation; execution/publication unconfirmed');
      const auditTag = row.auditTag;
      const requestTag = row.tag;
      await f.enqueue(request('third', 'retirement', {check_run_id: 12}));
      assert.equal(f.row(req.delivery_id), undefined, 'different admission retires terminal row');
      assert.equal(f.refs.get(`refs/tags/golem-terminal/${req.delivery_id}`), requestTag);
      await f.restart();
      const reachable = await f.github(new Request(`https://api.github.com${ops}/git/ref/tags/golem-audit/${req.delivery_id}`, {headers: {'X-GitHub-Api-Version': '2022-11-28'}}));
      const ref = await reachable.json();
      assert.equal(ref.object.sha, auditTag);
      const persistedAudit = JSON.parse(f.tags.get(ref.object.sha).message);
      assert.deepEqual(persistedAudit, audit);
      await f.enqueue(req);
      assert.equal(f.row(req.delivery_id), undefined);
      assert.equal(f.dispatches.filter(r => r.delivery_id === req.delivery_id).length, 1);
    }
  });
}

for (const fault of ['wrong-name', 'wrong-type', 'wrong-object', 'wrong-sha', 'lost-ref', 'audit-403', 'audit-500', 'terminal-save']) {
  await scenario(`private audit ${fault} retains then confirmed recovery survives retirement`, async f => {
    const req = request(); await f.enqueue(req); await f.bind(req, 7200);
    Object.assign(f.runs.get(7200), {status: 'completed', conclusion: 'failure'});
    f.checkMissing = req.check_run_id;
    f.repositories.set(repository, {status: 404, body: {}});
    if (fault.startsWith('wrong-')) {
      // First publish the valid immutable audit while withholding terminal journal persistence.
      f.rejectTerminal = true;
      await f.call('tick', {}, 409);
      f.rejectTerminal = false;
      const tag = f.tags.get(f.refs.get(`refs/tags/golem-audit/${req.delivery_id}`));
      const original = structuredClone(tag);
      if (fault === 'wrong-name') tag.tag = 'golem-audit/wrong';
      if (fault === 'wrong-type') tag.object.type = 'commit';
      if (fault === 'wrong-object') tag.object.sha = head;
      if (fault === 'wrong-sha') tag.sha = head;
      await f.restart(); await f.call('tick');
      assert.equal(f.row(req.delivery_id).state, 'bound', 'same message must not authorize wrong annotated tag identity');
      Object.assign(tag, original);
    } else {
      f.loseAuditRef = fault === 'lost-ref';
      f.failAudit = fault === 'audit-403' ? 403 : fault === 'audit-500' ? 500 : false;
      f.rejectTerminal = fault === 'terminal-save';
      await f.call('tick', {}, f.rejectTerminal ? 409 : 200);
      assert.equal(f.row(req.delivery_id).state, 'bound');
      f.failAudit = false; f.rejectTerminal = false;
    }
    assert.equal(f.dispatches.length, 1);
    await f.restart(); await f.call('tick');
    const terminal = f.row(req.delivery_id);
    assert.equal(terminal.state, 'terminal');
    const auditTag = terminal.auditTag;
    await f.enqueue(request('other', 'retire', {check_run_id: 11}));
    assert.equal(f.row(req.delivery_id), undefined);
    assert.equal(f.refs.get(`refs/tags/golem-terminal/${req.delivery_id}`), terminal.tag);
    await f.restart();
    assert.equal(f.refs.get(`refs/tags/golem-audit/${req.delivery_id}`), auditTag);
    const audit = JSON.parse(f.tags.get(auditTag).message);
    assert.equal(audit.request_id, req.delivery_id);
    assert.equal(audit.outcome, 'blocked');
    assert.equal(audit.model, 'unobserved');
    assert.equal(f.dispatches.filter(r => r.delivery_id === req.delivery_id).length, 1);
  });
}

for (const state of ['pending', 'reserved']) {
  await scenario(`${state} target GET 404 blocks privately and releases resources for healthy peer`, async f => {
    f.runs.set(1, {id: 1, status: 'queued'});
    const missing = request();
    const peerRepository = 'isp-insoft-gmbh/isp-golem-xiv-test-peer';
    const peer = request('member', 'hello', {repository: peerRepository, repository_id: 100, check_run_id: 11});
    f.repositories.set(peerRepository, {status: 200, body: {id: 100, full_name: peerRepository}});
    await f.enqueue(missing); await f.enqueue(peer);
    const queue = f.queue();
    queue.records[0].state = state;
    f.index.content = Buffer.from(JSON.stringify(queue)).toString('base64');
    const before = structuredClone(f.row(missing.delivery_id));
    f.repositories.set(repository, {status: 404, body: {message: 'Not Found'}});
    f.runs.clear();
    await f.restart(); await f.call('tick');
    assert.deepEqual(f.row(missing.delivery_id), {...before, state: 'terminal', outcome: 'blocked', reason: 'target unavailable at dispatch preflight'});
    assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id]);
    assert.ok(f.repositoryGets.includes(repository), '404 must come from target repository GET');
    assert.ok(f.repositoryGets.includes(peerRepository), 'healthy target identity must be fetched');
    assert.deepEqual(f.checkGets, [], 'unavailable target must not require a public check');
    assert.deepEqual(f.checks, []);
    assert.equal(f.reactions.length, 1, 'only healthy dispatched peer receives rocket');
    await f.restart(); await f.call('tick');
    assert.deepEqual(f.row(missing.delivery_id), {...before, state: 'terminal', outcome: 'blocked', reason: 'target unavailable at dispatch preflight'});
    assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id]);
    await f.enqueue(missing);
    assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id], 'terminal replay must not dispatch');
  });
}

for (const [failure, response] of [
  ['403', {status: 403, body: {message: 'Forbidden'}}],
  ['500', {status: 500, body: {message: 'Internal Server Error'}}],
  ['503', {status: 503, body: {message: 'Service Unavailable'}}],
  ['204', {status: 204, body: null}],
  ['201', {status: 201, body: {id: 99, full_name: repository}}],
  ['repository id mismatch', {status: 200, body: {id: 100, full_name: repository}}],
]) {
  for (const state of ['pending', 'reserved']) {
    await scenario(`${state} target ${failure} retains ownership without starving healthy peer`, async f => {
      f.runs.set(1, {id: 1, status: 'queued'});
      const retained = request();
      const waiting = request('member', 'waiting', {check_run_id: 11});
      const peerRepository = 'isp-insoft-gmbh/isp-golem-xiv-test-peer';
      const peer = request('other', 'peer', {repository: peerRepository, repository_id: 100, check_run_id: 12});
      f.repositories.set(peerRepository, {status: 200, body: {id: 100, full_name: peerRepository}});
      for (const req of [retained, waiting, peer]) await f.enqueue(req);
      const queue = f.queue();
      queue.records[0].state = state;
      f.index.content = Buffer.from(JSON.stringify(queue)).toString('base64');
      const before = structuredClone(f.row(retained.delivery_id));
      f.repositories.set(repository, response);
      f.runs.clear();
      await f.restart(); await f.call('tick');
      assert.deepEqual(f.row(retained.delivery_id), before);
      assert.equal(f.row(waiting.delivery_id).state, 'pending', 'retained seat still fences successor');
      assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id]);
      assert.ok(f.repositoryGets.includes(repository));
      assert.deepEqual(f.checkGets, []);
      assert.deepEqual(f.checks, []);
      await f.restart(); await f.call('tick');
      assert.deepEqual(f.row(retained.delivery_id), before);
      assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id]);
      f.repositories.set(repository, {status: 200, body: {id: 99, full_name: repository}});
      await f.call('tick');
      assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id, retained.delivery_id]);
      assert.equal(f.row(waiting.delivery_id).state, 'pending');
    });
  }
}

await scenario('repository failures beyond API budget do not starve a healthy peer across ticks', async f => {
  f.runs.set(1, {id: 1, status: 'queued'});
  const retained = [];
  for (let i = 0; i < 20; i++) {
    const req = request(`member${i}`, `golem${i}`, {check_run_id: 100 + i});
    retained.push(req);
    await f.enqueue(req);
  }
  const peerRepository = 'isp-insoft-gmbh/isp-golem-xiv-test-peer';
  const peer = request('peer', 'peer', {repository: peerRepository, repository_id: 100, check_run_id: 200});
  f.repositories.set(peerRepository, {status: 200, body: {id: 100, full_name: peerRepository}});
  await f.enqueue(peer);
  f.repositories.set(repository, {status: 403, body: {message: 'Forbidden'}});
  f.runs.clear();
  await f.restart(); await f.call('tick');
  await f.restart(); await f.call('tick');
  assert.deepEqual(f.dispatches.map(r => r.delivery_id), [peer.delivery_id]);
  for (const req of retained) assert.equal(f.row(req.delivery_id).state, 'pending');
});

for (const state of ['pending', 'reserved']) {
  for (const identity of [{dispatched: true}, {run_id: 9000, attempt: 1}]) {
    await scenario(`${state} with ${Object.keys(identity)[0]} is never retired or redispatched by target preflight`, async f => {
      f.runs.set(1, {id: 1, status: 'queued'});
      const req = request(); await f.enqueue(req);
      const queue = f.queue();
      Object.assign(queue.records[0], {state}, identity);
      f.index.content = Buffer.from(JSON.stringify(queue)).toString('base64');
      const before = structuredClone(f.row(req.delivery_id));
      f.repositories.set(repository, {status: 404, body: {message: 'Not Found'}});
      f.runs.clear();
      // Replay invokes advance without unrelated run reconciliation of this defensive journal shape.
      await f.restart(); await f.enqueue(req);
      assert.deepEqual(f.row(req.delivery_id), before);
      assert.deepEqual(f.repositoryGets, []);
      assert.deepEqual(f.dispatches, []);
      assert.deepEqual(f.checks, []);
    });
  }
}

await scenario('target disappearance never retires dispatched, uncertain or bound execution', async f => {
  const sent = request('alice', 'sent');
  const uncertain = request('bob', 'uncertain', {check_run_id: 11});
  const bound = request('carol', 'bound', {check_run_id: 12});
  await f.enqueue(sent);
  f.dispatchStatus = 500; await f.enqueue(uncertain);
  f.dispatchStatus = 204; await f.enqueue(bound); await f.bind(bound);
  const before = structuredClone(f.queue().records);
  f.repositoryGets.length = 0;
  f.repositories.set(repository, {status: 404, body: {message: 'Not Found'}});
  await f.restart(); await f.call('tick'); await f.call('tick');
  assert.deepEqual(f.queue().records, before);
  assert.deepEqual(f.repositoryGets, []);
  assert.deepEqual(f.dispatches.map(r => r.delivery_id), [sent.delivery_id, uncertain.delivery_id, bound.delivery_id]);
  assert.deepEqual(f.checks, []);
});

await scenario('three retained pending requests, shared seat FIFO, no waiting jobs, stale callbacks fenced', async f => {
  const requests = Array.from({length: 4}, (_, i) => request('member', `golem${i}`));
  await f.enqueue(requests[0]);
  await f.bind(requests[0]);
  for (const req of requests.slice(1)) assert.equal((await f.enqueue(req)).state, 'pending');
  assert.equal(f.dispatches.length, 1);
  assert.equal(f.queue().records.filter(r => r.state === 'pending').length, 3);
  const old = {...f.row(requests[0].delivery_id)};
  await f.complete(requests[0], {generation: randomUUID()}, 409);
  assert.equal(f.dispatches.length, 1);
  await f.complete(requests[0]);
  assert.equal(f.dispatches.length, 2);
  assert.equal(f.dispatches[1].delivery_id, requests[1].delivery_id);
  await f.bind(requests[1], 1001);
  await f.call('complete', {request_id: old.id, generation: old.generation, run_id: old.run_id, attempt: old.attempt});
  assert.equal(f.row(requests[1].delivery_id).state, 'bound');
  await f.complete(requests[1], {attempt: 2}, 409);
  for (let i = 1; i < requests.length; i++) {
    if (i > 1) await f.bind(requests[i], 1000 + i);
    await f.complete(requests[i]);
  }
  assert.deepEqual(f.dispatches.map(r => r.delivery_id), requests.map(r => r.delivery_id));
  assert.equal(f.reactions.length, 4);
});

await scenario('opposite resources respect earlier admission; unrelated pair advances', async f => {
  const a = request('alice', 'one'), b = request('bob', 'one'), c = request('bob', 'two'), d = request('carol', 'three', {comment_id: null, issue_number: null, trigger: 'schedule', owner: 'carol'});
  await f.enqueue(a); await f.bind(a);
  await f.enqueue(b); await f.enqueue(c); await f.enqueue(d);
  assert.deepEqual(f.dispatches.map(r => r.delivery_id), [a.delivery_id, d.delivery_id]);
  await f.bind(d, 1001);
  await f.complete(a);
  assert.equal(f.dispatches.at(-1).delivery_id, b.delivery_id);
  assert.equal(f.row(c.delivery_id).state, 'pending');
  await f.bind(b, 1002); await f.complete(b);
  assert.equal(f.dispatches.at(-1).delivery_id, c.delivery_id);
  assert.equal(f.reactions.length, 3, 'no reaction for absent schedule comment');
});

await scenario('GitHub journal alone restores FIFO and ownership; immutable request replay deduplicates', async f => {
  const a = request(), b = request();
  await f.enqueue(a); await f.bind(a); await f.enqueue(b);
  await f.restart();
  await f.call('tick');
  assert.equal(f.dispatches.length, 1);
  await f.complete(a);
  assert.equal(f.dispatches[1].delivery_id, b.delivery_id);
  await f.enqueue(a);
  assert.equal(f.dispatches.length, 2);
});

await scenario('ambiguous dispatch persists reservation, never retries on alarms or restoration', async f => {
  f.dispatchStatus = 500;
  const a = request(), b = request();
  await f.enqueue(a); await f.enqueue(b);
  assert.equal(f.row(a.delivery_id).state, 'dispatch_uncertain');
  await f.restart(); await f.call('tick'); await f.call('tick');
  assert.equal(f.dispatches.length, 1);
  assert.equal(f.row(b.delivery_id).state, 'pending');
  f.dispatchStatus = 204;
  assert.equal((await f.bind(a)).state, 'granted');
  await f.complete(a);
  assert.equal(f.dispatches.length, 2);
});

await scenario('stop before bind records identity without granting; cancel acknowledgment and 404 never release', async f => {
  const a = request(), b = request();
  await f.enqueue(a); await f.enqueue(b);
  await f.call('stop', {repository, issue_number: 1});
  assert.equal(f.row(a.delivery_id).state, 'stop_requested');
  assert.equal(f.row(b.delivery_id).state, 'pending', 'stop targets active mention only, not later pending mention');
  assert.equal((await f.bind(a)).state, 'stop_requested');
  const id = f.row(a.delivery_id).run_id;
  assert.ok(f.cancels.includes(id));
  await f.call('tick');
  assert.equal(f.row(a.delivery_id).state, 'stop_requested');
  const run = f.runs.get(id); f.runs.delete(id);
  await f.restart(); await f.call('tick');
  assert.equal(f.row(a.delivery_id).state, 'stop_requested');
  f.runs.set(id, {...run, status: 'completed', conclusion: 'cancelled'});
  await f.call('tick');
  assert.equal(f.row(a.delivery_id).state, 'terminal');
  assert.equal(f.row(b.delivery_id).state, 'dispatching', 'untargeted pending mention advances');
  assert.ok(f.checks.some(c => c.output.summary.includes('outcome: stopped')));
});

await scenario('separate pending stop intents survive interrupted persistence and check repair', async f => {
  const a = request(); await f.enqueue(a); await f.bind(a);
  const pending = Array.from({length: 3}, (_, i) => request('member', 'hello', {issue_number: i + 2, check_run_id: i + 20}));
  for (const req of pending) await f.enqueue(req);
  for (const req of pending) {
    f.failWrite = {state: 'stop_requested'};
    await f.call('stop', {repository, issue_number: req.issue_number}, 503);
  }
  f.checkFailure = pending[0].check_run_id;
  await f.restart(); await f.call('tick');
  assert.equal(f.row(pending[0].delivery_id).state, 'stop_requested');
  for (const req of pending.slice(1)) assert.equal(f.row(req.delivery_id).state, 'terminal');
  f.checkFailure = null;
  await f.restart(); await f.call('tick');
  for (const req of pending) assert.equal(f.row(req.delivery_id).state, 'terminal');
  assert.equal(f.row(a.delivery_id).state, 'bound', 'other thread never stopped');
  assert.equal(f.cancels.length, 0);
  assert.equal(f.dispatches.length, 1);
});

await scenario('activation blocks new grants behind unknown legacy queued or active runs', async f => {
  f.runs.set(1, {id: 1, status: 'queued'});
  const req = request();
  assert.equal((await f.enqueue(req)).state, 'pending');
  assert.equal(f.dispatches.length, 0);
  f.runs.get(1).status = 'in_progress'; await f.call('tick');
  assert.equal(f.dispatches.length, 0);
  f.runs.get(1).status = 'completed'; await f.call('tick');
  assert.equal(f.dispatches.length, 1);
});

await scenario('Contents conflicts and committed lost responses never dispatch from an unconfirmed write', async f => {
  const a = request();
  // First initialize journal, then fail admission before commit, recover immutable tag on retry.
  await f.call('tick');
  f.failWrite = 'conflict';
  await f.enqueue(a, 409);
  assert.equal(f.dispatches.length, 0);
  await f.enqueue(a);
  assert.equal(f.dispatches.length, 1);
  await f.bind(a); await f.complete(a);
  const b = request();
  f.failWrite = {state: 'dispatching'};
  await f.enqueue(b, 503);
  assert.equal(f.dispatches.length, 1);
  await f.restart(); await f.call('tick');
  assert.equal(f.row(b.delivery_id).state, 'dispatching');
  assert.equal(f.dispatches.length, 1, 'lost dispatch-intent acknowledgment must not blind retry');
});

await scenario('bounded capacity rejects before admission, retains every accepted pending request', async f => {
  const first = request(); await f.enqueue(first); await f.bind(first);
  for (let i = 0; i < 31; i++) await f.enqueue(request());
  const rejected = request();
  await f.enqueue(rejected, 429);
  assert.equal(f.queue().records.length, 32);
  assert.equal(f.queue().records.filter(r => r.state === 'pending').length, 31);
  assert.ok(!f.refs.has(`refs/tags/golem-request/${rejected.delivery_id}`));
  assert.equal(f.dispatches.length, 1);
});

await scenario('terminal retirement reclaims capacity without replaying old request tags', async f => {
  const first = request();
  for (let i = 0; i < 34; i++) {
    const req = i === 0 ? first : request();
    await f.enqueue(req); await f.bind(req, i + 1000); await f.complete(req);
  }
  assert.ok(f.queue().records.length < 32);
  assert.equal((await f.enqueue(first)).state, 'dispatched');
  assert.equal(f.dispatches.length, 34);
});

await scenario('bounded fair reconciliation reaches fifth run behind four unknown owners', async f => {
  const requests = Array.from({length: 5}, (_, i) => request(`seat${i}`, `golem${i}`));
  for (let i = 0; i < requests.length; i++) { await f.enqueue(requests[i]); await f.bind(requests[i], 1000 + i); }
  for (let i = 0; i < 4; i++) f.runs.delete(1000 + i);
  Object.assign(f.runs.get(1004), {status: 'completed', conclusion: 'cancelled'});
  await f.call('tick'); await f.restart(); await f.call('tick');
  assert.equal(f.row(requests[4].delivery_id).state, 'terminal');
  for (const req of requests.slice(0, 4)) assert.equal(f.row(req.delivery_id).state, 'bound');
});

await scenario('denied rerun attempts, oversized input, redacted errors, malformed requests', async f => {
  const req = request(); await f.enqueue(req);
  await f.bind(req, 1000, {attempt: 2}, 409);
  await f.enqueue({...request(), version: 1}, 400);
  const response = await f.worker.dispatchFetch('https://queue.invalid/', {method: 'POST', body: 'x'.repeat(65537)});
  assert.equal(response.status, 413);
  f.dispatchStatus = 500;
  const other = request('other', 'other');
  const body = await f.enqueue(other);
  assert.ok(!JSON.stringify(body).includes('credential'));
});
await scenario('GitHub redirect fails closed without forwarding credentials to another origin', async f => {
  f.redirect = true;
  const result = await f.enqueue(request(), 503);
  assert.deepEqual(result, {error: 'queue operation failed', safeFailure: 'upstream'});
  assert.equal(f.dispatches.length, 0);
  assert.equal(f.index, null);
  assert.deepEqual(f.origins, ['https://api.github.com'], 'never request redirect target');
});
await scenario('concurrent admissions serialize durably and preserve every shared-resource request', async f => {
  const requests = Array.from({length: 4}, () => request());
  const results = await Promise.all(requests.map(req => f.enqueue(req)));
  assert.equal(results.filter(r => r.state === 'dispatched').length, 1);
  assert.equal(results.filter(r => r.state === 'pending').length, 3);
  assert.equal(f.dispatches.length, 1);
  const order = f.queue().records.map(r => r.id);
  assert.equal(new Set(order).size, 4);
  for (const id of order) {
    const req = requests.find(r => r.delivery_id === id);
    await f.bind(req, 2000 + order.indexOf(id));
    await f.complete(req);
  }
  assert.deepEqual(f.dispatches.map(r => r.delivery_id), order);
});

await scenario('bootstrap branch publication with lost response restores existing journal without reset', async f => {
  f.failBootstrap = true;
  await f.call('tick', {}, 503);
  const sha = f.index.sha;
  await f.restart();
  const a = request(); await f.enqueue(a);
  assert.notEqual(f.index.sha, sha);
  assert.equal(f.dispatches.length, 1);
  assert.equal(f.row(a.delivery_id).state, 'dispatching');
});

await scenario('2022 dispatch contract requires 204, unexpected 200 remains uncertain', async f => {
  f.dispatchStatus = 200;
  const req = request(); await f.enqueue(req);
  assert.equal(f.row(req.delivery_id).state, 'dispatch_uncertain');
  await f.restart(); await f.call('tick');
  assert.equal(f.dispatches.length, 1);
});

await scenario('bind rejects wrong workflow identity and cannot steal another run or attempt', async f => {
  const req = request(); await f.enqueue(req);
  await f.bind(req, 1000);
  const row = f.row(req.delivery_id);
  const identity = {request_id: row.id, generation: row.generation, run_id: 1000, attempt: 1};
  f.runs.get(1000).head_branch = 'other';
  await f.call('bind', identity, 409);
  f.runs.get(1000).head_branch = 'trunk';
  await f.call('bind', {...identity, run_id: 1001}, 409);
  await f.call('bind', {...identity, generation: randomUUID()}, 409);
  f.runs.get(1000).run_attempt = 2;
  await f.call('tick');
  assert.equal(f.row(req.delivery_id).state, 'bound');
});
await scenario('check repair failure retains owner without starving other completions across restarts', async f => {
  const requests = Array.from({length: 5}, (_, i) => request(`seat${i}`, `golem${i}`, {check_run_id: 10 + i}));
  for (let i = 0; i < requests.length; i++) {
    await f.enqueue(requests[i]); await f.bind(requests[i], 1000 + i);
    Object.assign(f.runs.get(1000 + i), {status: 'completed', conclusion: 'cancelled'});
  }
  f.checkFailure = 10;
  await f.call('tick'); await f.restart(); await f.call('tick');
  assert.equal(f.row(requests[0].delivery_id).state, 'bound');
  for (const req of requests.slice(1)) assert.equal(f.row(req.delivery_id).state, 'terminal');
});

await scenario('committed lost reservation, bind, terminal and retirement responses remain fenced', async f => {
  await f.call('tick');
  const a = request(), b = request();
  f.failWrite = {state: 'reserved'};
  await f.enqueue(a, 503);
  assert.equal(f.dispatches.length, 0);
  await f.restart(); await f.call('tick');
  assert.equal(f.dispatches.length, 1, 'reserved-only crash is safe to dispatch');
  f.failWrite = {state: 'bound'};
  await f.bind(a, 1000, {}, 503);
  await f.restart(); await f.call('tick');
  assert.equal(f.dispatches.length, 1);
  assert.equal(f.row(a.delivery_id).state, 'bound');
  await f.enqueue(b);
  f.failWrite = {state: 'terminal'};
  await f.complete(a, {}, 503);
  await f.restart(); await f.call('tick');
  assert.equal(f.dispatches.at(-1).delivery_id, b.delivery_id);
  f.failReceipt = true;
  const c = request(); await f.enqueue(c, 503);
  assert.equal(f.dispatches.length, 2);
  await f.restart();
  await f.enqueue(a);
  assert.equal(f.dispatches.length, 2, 'retirement receipt deduplicates despite lost acknowledgment');
  await f.enqueue(c);
  assert.equal(f.dispatches.length, 2, 'same-seat successor is still pending');
});

await scenario('incomplete legacy API metadata blocks grants', async f => {
  f.malformedList = true;
  const req = request();
  assert.equal((await f.enqueue(req)).state, 'pending');
  assert.equal(f.dispatches.length, 0);
  f.malformedList = false; await f.call('tick');
  assert.equal(f.dispatches.length, 1);
});
await scenario('stop selects latest active mention only; older mention, pending mention, events and schedules remain', async f => {
  const older = request('alice', 'older', {check_run_id: 101});
  const latest = request('bob', 'latest', {check_run_id: 102});
  const event = request('carol', 'event', {trigger: 'event', check_run_id: 103});
  const schedule = request('dave', 'schedule', {trigger: 'schedule', check_run_id: 104});
  const pending = request('bob', 'pending', {check_run_id: 105});
  const elsewhere = request('erin', 'elsewhere', {issue_number: 2, check_run_id: 106});
  for (const [i, req] of [older, latest, event, schedule, elsewhere].entries()) { await f.enqueue(req); await f.bind(req, 3000 + i); }
  await f.enqueue(pending);
  const result = await f.call('stop', {repository, issue_number: 1});
  assert.equal(result.count, 1);
  assert.equal(f.row(latest.delivery_id).state, 'stop_requested');
  assert.deepEqual(f.cancels, [3001]);
  for (const req of [older, event, schedule, elsewhere]) assert.equal(f.row(req.delivery_id).state, 'bound');
  assert.equal(f.row(pending.delivery_id).state, 'pending');
  await f.restart(); await f.call('tick');
  assert.ok(f.cancels.every(id => id === 3001));
  for (const req of [older, event, schedule, elsewhere]) assert.equal(f.row(req.delivery_id).state, 'bound');
});

await scenario('without active mention stop selects only latest pending mention, never unattended work', async f => {
  f.runs.set(1, {id: 1, status: 'queued'});
  const older = request('member', 'older', {check_run_id: 201});
  const latest = request('member', 'latest', {check_run_id: 202});
  const event = request('member', 'event', {trigger: 'event', check_run_id: 203});
  const schedule = request('member', 'schedule', {trigger: 'schedule', check_run_id: 204});
  for (const req of [older, latest, event, schedule]) await f.enqueue(req);
  assert.equal((await f.call('stop', {repository, issue_number: 1})).count, 1);
  assert.equal(f.row(latest.delivery_id).state, 'terminal');
  for (const req of [older, event, schedule]) assert.equal(f.row(req.delivery_id).state, 'pending');
  assert.equal(f.cancels.length, 0);
  assert.equal(f.dispatches.length, 0);
  assert.equal((await f.call('stop', {repository, issue_number: 99})).count, 0);
});

await scenario('crash after bind: successful and failed terminal workflows receive blocked audits before release', async f => {
  for (const [i, conclusion] of ['success', 'failure'].entries()) {
    const req = request('member', `crash${i}`, {check_run_id: 301 + i, model: 'explicit-model'});
    await f.enqueue(req); await f.bind(req, 4000 + i);
    const check = f.checkState.get(req.check_run_id);
    check.status = i === 0 ? 'queued' : 'in_progress';
    check.output = {summary: 'outcome: running'};
    Object.assign(f.runs.get(4000 + i), {status: 'completed', conclusion});
    f.onCheckPatch = id => {
      assert.equal(id, req.check_run_id);
      assert.equal(f.row(req.delivery_id).state, 'bound', 'repair must precede durable resource release');
    };
    await f.restart(); await f.call('tick');
    assert.equal(f.row(req.delivery_id).state, 'terminal');
    const repaired = f.checkState.get(req.check_run_id);
    assert.equal(repaired.status, 'completed'); assert.equal(repaired.conclusion, 'failure');
    const lines = repaired.output.summary.split('\n');
    for (const line of ['outcome: blocked', 'seat: member/anthropic', 'harness: claude', 'model: explicit-model', 'budget: 1800s', `log: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${4000 + i}`]) assert.ok(lines.includes(line));
    assert.ok(lines.some(line => line.startsWith('reason: ') && line.includes(conclusion) && line.includes('unconfirmed')));
  }
});

await scenario('terminal reconciliation preserves valid native audits, repairs completed checks lacking valid audit', async f => {
  const req = request('member', 'native', {check_run_id: 401});
  await f.enqueue(req); await f.bind(req, 5000);
  const native = {...f.checkState.get(401), status: 'completed', conclusion: 'neutral', output: {summary: 'outcome: incomplete\nseat: member/anthropic\nharness: claude\nmodel: resolved-model\nbudget: 1800s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/5000\nreason: native budget exhausted'}};
  f.checkState.set(401, structuredClone(native));
  Object.assign(f.runs.get(5000), {status: 'completed', conclusion: 'failure'});
  await f.restart(); await f.call('tick');
  assert.equal(f.row(req.delivery_id).state, 'terminal');
  assert.deepEqual(f.checkState.get(401), native);
  assert.equal(f.checks.length, 0, 'valid completed native audit must not be overwritten');
  const cancelled = request('member', 'native-cancelled', {check_run_id: 403});
  await f.enqueue(cancelled); await f.bind(cancelled, 5002);
  const cancelledAudit = {...native, id: 403, name: 'golem/native-cancelled', output: {summary: native.output.summary.replace('/runs/5000', '/runs/5002')}};
  f.checkState.set(403, structuredClone(cancelledAudit));
  Object.assign(f.runs.get(5002), {status: 'completed', conclusion: 'cancelled'});
  await f.call('tick');
  assert.deepEqual(f.checkState.get(403), cancelledAudit, 'workflow cancellation after native publication must not erase valid audit');
  assert.equal(f.checks.length, 0);
  const malformed = request('member', 'missing-audit', {check_run_id: 402});
  await f.enqueue(malformed); await f.bind(malformed, 5001);
  f.checkState.set(402, {...f.checkState.get(402), status: 'completed', conclusion: 'success', output: null});
  Object.assign(f.runs.get(5001), {status: 'completed', conclusion: 'success'});
  await f.call('tick');
  assert.equal(f.row(malformed.delivery_id).state, 'terminal');
  assert.ok(f.checkState.get(402).output.summary.startsWith('outcome: blocked\n'));
});

await scenario('missing check or failed blocked-audit repair retains resources; fair cursor reaches other owners', async f => {
  const requests = Array.from({length: 5}, (_, i) => request(`seat${i}`, `crash${i}`, {check_run_id: 501 + i}));
  for (const [i, req] of requests.entries()) {
    await f.enqueue(req); await f.bind(req, 6000 + i);
    Object.assign(f.runs.get(6000 + i), {status: 'completed', conclusion: i % 2 ? 'failure' : 'success'});
  }
  f.checkMissing = 501;
  f.checkFailure = 502;
  const successor = request('seat0', 'successor', {check_run_id: 506});
  await f.enqueue(successor);
  await f.call('tick'); await f.restart(); await f.call('tick');
  assert.equal(f.row(requests[0].delivery_id).state, 'bound');
  assert.equal(f.row(requests[1].delivery_id).state, 'bound');
  assert.equal(f.row(successor.delivery_id).state, 'pending');
  for (const req of requests.slice(2)) assert.equal(f.row(req.delivery_id).state, 'terminal');
  assert.equal(f.dispatches.length, 5);
  f.checkMissing = null; f.checkFailure = null;
  await f.restart(); await f.call('tick'); await f.call('tick');
  for (const req of requests) assert.equal(f.row(req.delivery_id).state, 'terminal');
  assert.equal(f.row(successor.delivery_id).state, 'dispatching');
  assert.equal(f.dispatches.length, 6);
});
// The optional producer/consumer check invokes unchanged Github::check_reason, not a JS
// transcription of its six outcomes or DispatchV1::summary. Only transport is local.
async function nativeAudits() {
  const scratch = await mkdtemp(join(tmpdir(), 'golem-native-audits-'));
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const rustString = value => JSON.stringify(value.replaceAll('\\', '/'));
  const captured = [];
  const server = createServer(async (req, res) => {
    try {
      assert.equal(req.method, 'PATCH');
      assert.equal(req.url, `/repos/${repository}/check-runs/10`);
      const chunks = [];
      let size = 0;
      for await (const chunk of req) { size += chunk.length; assert.ok(size <= 65536); chunks.push(chunk); }
      assert.ok(captured.length < 6);
      captured.push(JSON.parse(Buffer.concat(chunks)));
      res.writeHead(200, {'Content-Type': 'application/json'}); res.end('{}');
    } catch { res.writeHead(500); res.end('{}'); }
  });
  try {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const url = `http://127.0.0.1:${server.address().port}`;
    const manifest = await readFile(new URL('../../runner/Cargo.toml', import.meta.url), 'utf8');
    const dependencies = manifest.split('[dependencies]')[1].split('[lints]')[0].replace('path = "../router"', `path = ${rustString(join(root, 'router'))}`);
    await writeFile(join(scratch, 'Cargo.toml'), `[package]\nname = "coordinator-native-audits"\nversion = "0.0.0"\nedition = "2024"\n[dependencies]\ngolem-runner = {path = ${rustString(join(root, 'runner'))}}\n${dependencies}`);
    await copyFile(join(root, 'Cargo.lock'), join(scratch, 'Cargo.lock'));
    await mkdir(join(scratch, 'src'));
    await writeFile(join(scratch, 'src/main.rs'), `#![allow(dead_code)]
mod github {
    include!(${rustString(join(root, 'runner/src/github.rs'))});
    pub fn fixture(base_url: String) -> Github {
        Github {client: reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(5)).build().unwrap(), base_url}
    }
}
use std::io::Read;
#[tokio::main]
async fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let request: golem_runner::DispatchV1 = serde_json::from_str(&input).unwrap();
    let github = github::fixture(std::env::args().nth(1).unwrap());
    for outcome in ["changed", "replied", "no_change", "incomplete", "blocked", "stopped"] {
        github.check_reason(&request, "synthetic-token", outcome, "runner-model", "7000", Some("runner-produced fixture reason"), &golem_runner::OutputGuard::default()).await.unwrap();
    }
}
`);
    const started = Date.now();
    await new Promise((resolve, reject) => {
      const child = spawn('cargo', ['run', '--offline', '--quiet', '--manifest-path', join(scratch, 'Cargo.toml'), '--target-dir', join(scratch, 'target'), '--', url], {cwd: root, stdio: ['pipe', 'inherit', 'inherit'], timeout: 300000});
      child.on('error', reject);
      child.on('close', code => code === 0 ? resolve() : reject(new Error(`native audit producer exited ${code}`)));
      child.stdin.end(JSON.stringify(request()));
    });
    assert.equal(captured.length, 6);
    assert.ok(captured.find(a => a.output.summary.startsWith('outcome: no_change\n')).output.summary.includes('\nreason: runner-produced fixture reason'));
    console.log(`Native Rust audit producer: six payloads captured in ${Date.now() - started}ms; temporary build removed after capture.`);
    return captured;
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(scratch, {recursive: true, force: true, maxRetries: 5, retryDelay: 100});
  }
}
if (process.argv.includes('--runner-contract')) {
  const audits = await nativeAudits();
  for (const audit of audits) {
    const outcome = audit.output.summary.split('\n')[0].slice(9);
    await scenario(`actual Rust ${outcome} audit survives terminal reconciliation unchanged`, async f => {
      const req = request(); await f.enqueue(req); await f.bind(req, 7000);
      const check = {...f.checkState.get(10), ...audit};
      f.checkState.set(10, structuredClone(check));
      Object.assign(f.runs.get(7000), {status: 'completed', conclusion: 'success'});
      await f.restart(); await f.call('tick');
      assert.equal(f.row(req.delivery_id).state, 'terminal');
      assert.deepEqual(f.checkState.get(10), check);
      assert.equal(f.checks.length, 0, 'reconciliation must not PATCH a valid runner-produced audit');
    });
  }
  const noChange = audits.find(a => a.output.summary.startsWith('outcome: no_change\n'));
  await scenario('actual Rust no_change audit without its required reason is not preserved', async f => {
    const req = request(); await f.enqueue(req); await f.bind(req, 7000);
    const invalid = structuredClone(noChange);
    invalid.output.summary = invalid.output.summary.split('\n').filter(line => !line.startsWith('reason: ')).join('\n');
    f.checkState.set(10, {...f.checkState.get(10), ...invalid});
    Object.assign(f.runs.get(7000), {status: 'completed', conclusion: 'success'});
    await f.call('tick');
    assert.equal(f.row(req.delivery_id).state, 'terminal');
    assert.ok(f.checkState.get(10).output.summary.startsWith('outcome: blocked\n'));
    assert.equal(f.checks.length, 1);
  });
}
await runScenarios();
console.log('Coordinator: real Miniflare SQLite class + mock GitHub journal scenarios passed.');
