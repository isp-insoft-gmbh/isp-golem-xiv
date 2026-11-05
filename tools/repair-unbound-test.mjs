import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {repair, verify, options, OPS, CODE, WORKFLOW_SHA256, COORDINATOR_SHA256, digest, FAILURE_MESSAGE, client, GitHubHttpError} from './repair-unbound.mjs';

const delivery = 'f37f6672-d908-4ffa-6488-c011eb2f94c7';
const generation = '3bdb4eb1-d440-4262-80ef-387193452211';
const input = {delivery, run: 41774622060, apply: false};
const tagSHA = '7'.repeat(40), fileSHA = '8'.repeat(40), head = '9'.repeat(40);
// Reconstruct historical bytes as the live pin advances; exact digests still gate both profiles.
const currentWorkflow = await readFile(new URL('../ops/runner.yml', import.meta.url));
const workflowV3 = Buffer.from(currentWorkflow.toString('utf8').replace(
  /(RUNNER_RELEASE_URL: https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv\/releases\/download\/)v[0-9]+(?=\n)/,
  '$1v3',
));
assert.equal(digest(workflowV3), 'b6dde8ad547a36bf97ffb97e55fc6bd72a2380fbbf10ab01ef461c3e3d8a95ea');
const workflow = Buffer.from(workflowV3.toString('utf8').replace(
  'RUNNER_RELEASE_URL: https://github.com/isp-insoft-gmbh/isp-golem-xiv/releases/download/v3\n',
  'RUNNER_RELEASE_URL: https://github.com/isp-insoft-gmbh/isp-golem-xiv/releases/download/v2\n',
));
assert.equal(digest(workflow), WORKFLOW_SHA256);
const coordinator = await readFile(new URL('../router/src/coordinator.js', import.meta.url));
assert.equal(digest(coordinator), COORDINATOR_SHA256);
function provenance() {
  return {
    release: {id: 433413769, created_at: '2026-11-04T01:29:14Z', published_at: '2026-11-04T01:37:14Z', updated_at: '2026-11-04T01:37:14Z', author: {login: 'github-actions[bot]'}, tag_name: 'v2', draft: false, immutable: false, assets: [{name: 'golem-runner-linux-x86_64', id: 707806211, digest: 'sha256:3a12f77a8b4b7197aa302fb94d298a282faefb01c1794404f000aae3f9c6a838', created_at: '2026-11-04T01:37:13Z', updated_at: '2026-11-04T01:37:13Z', uploader: {login: 'github-actions[bot]'}}]},
    build: {id: 41754384511, repository: {full_name: CODE}, head_sha: '0a9555f6c510085d251a6acf891e6740a9cabad4', event: 'push', head_branch: 'v2', status: 'completed', conclusion: 'success', created_at: '2026-11-04T01:29:19Z', updated_at: '2026-11-04T01:37:17Z'},
    ref: {ref: 'refs/tags/v2', object: {type: 'tag', sha: '1fe798c3fed7f7206985c5ea3e944c2dbfde31d4'}},
    tag: {sha: '1fe798c3fed7f7206985c5ea3e944c2dbfde31d4', tag: 'v2', object: {type: 'commit', sha: '0a9555f6c510085d251a6acf891e6740a9cabad4'}},
  };
}
const req = {version: 2, delivery_id: delivery, queue_generation: generation, repository: 'isp-insoft-gmbh/isp-golem-xiv-test-repair', repository_id: 44, golem: 'hello', seat_login: 'member', provider: 'anthropic', instructions: 'immutable full body'};
const row = {id: delivery, generation, tag: tagSHA, state: 'dispatching', dispatched: true, run_id: null, attempt: null, resource: `${req.repository}/hello`, seat: 'member/anthropic'};
const run = {id: input.run, repository: {full_name: OPS, id: 33}, status: 'completed', conclusion: 'failure', run_attempt: 1, head_sha: head, display_title: `${req.repository} hello`, path: '.github/workflows/runner.yml', event: 'workflow_dispatch', head_branch: 'trunk', created_at: '2026-11-04T04:40:28Z'};
const line = text => `runner\tRun exactly one golem\t2026-11-01T01:00:02.0000000Z ${text}\n`;
const validatorLine = text => `runner\tValidate dispatch before selecting credentials\t2026-11-01T01:00:01.0000000Z ${text}\n`;
const log = validatorLine(`  GOLEM_REQUEST: ${JSON.stringify(req)}`) + line(`  GOLEM_REQUEST: ${JSON.stringify(req)}`) + line('golem runner failed: queue request denied') + 'runner\tEnsure cancellation cannot leave an agent alive\t2026-11-01T01:00:03.0000000Z cleanup\n';
function snapshot() {
  return {provenance: provenance(), health: {app: 'isp-golem-xiv', version: head}, deployedSource: coordinator, queue: {version: 2, records: [structuredClone(row)]}, file: {sha: fileSHA}, req: structuredClone(req), tag: {sha: tagSHA, tag: `golem-request/${delivery}`, tagger: {date: '2026-11-01T01:00:00Z'}}, ref: {ref: `refs/tags/golem-request/${delivery}`, object: {type: 'tag', sha: tagSHA}}, run: structuredClone(run), repo: {id: 44, private: true, full_name: req.repository, topics: ['isp-golem-xiv-test']}, opsRepo: {id: 33, full_name: OPS, private: true, default_branch: 'trunk'}, workflow, runs: {total_count: 1, workflow_runs: [structuredClone(run)]}, jobs: {total_count: 1, jobs: [{name: 'runner', status: 'completed', conclusion: 'failure', steps: [{number: 5, name: 'Validate dispatch before selecting credentials', status: 'completed', conclusion: 'success'}, {number: 6, name: 'Run exactly one golem', status: 'completed', conclusion: 'failure'}, {number: 7, name: 'Ensure cancellation cannot leave an agent alive', status: 'completed'}]}]}, log, windowEnd: new Date().toISOString()};
}
function snapshotV3() {
  const s = snapshot();
  s.workflow = workflowV3;
  s.run.created_at = '2026-11-04T08:00:00Z';
  s.runs.workflow_runs = [structuredClone(s.run)];
  s.log = s.log.replace('queue request denied', 'queue conflict');
  s.provenance = {
    release: {id: 433607642, created_at: '2026-11-04T06:55:31Z', published_at: '2026-11-04T07:06:07Z', updated_at: '2026-11-04T07:06:07Z', author: {login: 'github-actions[bot]'}, tag_name: 'v3', draft: false, immutable: false, assets: [{name: 'golem-runner-linux-x86_64', id: 708587759, digest: 'sha256:8aa6f65d1846354792bbc5bef64af84b22f2194a209d125060c037f755c88373', created_at: '2026-11-04T07:06:04Z', updated_at: '2026-11-04T07:06:06Z', uploader: {login: 'github-actions[bot]'}}]},
    build: {id: 41788750529, repository: {full_name: CODE}, head_sha: '00d62520f873ec404d66f409d51db18933f82ae8', event: 'push', head_branch: 'v3', status: 'completed', conclusion: 'success', created_at: '2026-11-04T06:55:37Z', updated_at: '2026-11-04T07:06:10Z'},
    ref: {ref: 'refs/tags/v3', object: {type: 'tag', sha: 'a4cf89f72e30cb3cc353f23fc4397d5fafa58e59'}},
    tag: {sha: 'a4cf89f72e30cb3cc353f23fc4397d5fafa58e59', tag: 'v3', object: {type: 'commit', sha: '00d62520f873ec404d66f409d51db18933f82ae8'}},
  };
  return s;
}
for (const makeSnapshot of [snapshot, snapshotV3]) {
  for (const [key, value] of [['id', 1], ['created_at', '2026-11-04T00:00:00Z'], ['published_at', '2026-11-04T00:00:00Z'], ['updated_at', '2026-11-04T00:00:00Z'], ['author', {login: 'impostor'}], ['tag_name', 'renamed']]) {
    const s = makeSnapshot();
    s.provenance.release[key] = value;
    assert.throws(() => verify(s, input), /release/);
  }
}
for (const makeSnapshot of [snapshot, snapshotV3]) {
  for (const mutate of [
    s => { s.log = s.log.replace(line(`  GOLEM_REQUEST: ${JSON.stringify(req)}`), line(`  GOLEM_REQUEST: ${JSON.stringify({...req, instructions: 'wrong native request'})}`)); },
    s => { s.log = s.log.replace(validatorLine(`  GOLEM_REQUEST: ${JSON.stringify(req)}`), validatorLine(`  GOLEM_REQUEST: ${JSON.stringify({...req, instructions: 'wrong validator request'})}`)); },
    s => { s.log += line(`GOLEM_REQUEST: ${JSON.stringify(req)}`); },
    s => { s.log += `runner\tUnexpected step\t2026-11-01T01:00:04Z GOLEM_REQUEST: ${JSON.stringify(req)}\n`; },
    s => { s.jobs.jobs[0].steps = s.jobs.jobs[0].steps.filter(step => step.name !== 'Validate dispatch before selecting credentials'); },
    s => { s.jobs.jobs[0].steps[0].number = 8; },
    s => { s.jobs.jobs[0].steps[0].conclusion = 'failure'; },
    s => { s.jobs.jobs[0].steps[0].status = 'in_progress'; },
    s => { s.log = s.log.replace(validatorLine(`  GOLEM_REQUEST: ${JSON.stringify(req)}`), ''); },
    s => { const record = validatorLine(`  GOLEM_REQUEST: ${JSON.stringify(req)}`); s.log = s.log.replace(record, '') + record; },
    s => { s.log += validatorLine('PHASE: provider-started'); },
    s => { s.log += 'runner\tUNKNOWN STEP\t2026-11-01T01:00:04Z phase: admitted\n'; },
  ]) {
    const s = makeSnapshot(); mutate(s);
    assert.throws(() => verify(s, input));
  }
}
function mock(mode = '', makeSnapshot = snapshot) {
  const s = makeSnapshot(), writes = [], evidence = [];
  let tags = 0, queueReads = 0, healthReads = 0;
  const io = {health: async () => {
    healthReads++;
    if (healthReads === 2) {
      if (mode === 'recheck-health-error') throw new Error('health unavailable on recheck');
      if (mode === 'recheck-version') s.health = {...s.health, version: s.health.version[0] === 'f' ? 'e'.repeat(40) : 'f'.repeat(40)};
      if (mode === 'recheck-source') s.deployedSource = Buffer.from('different coordinator after candidate annotation');
    }
    if (mode === 'health-error') throw new Error('health unavailable');
    return s.health;
  }, log: async () => s.log, api: async (path, method = 'GET', body) => {
    if (method !== 'GET') {
      writes.push({path, method, body});
      if (path.endsWith('/git/tags')) { if (mode === 'annotation-lost' && tags > 0) throw new Error('annotation failed'); evidence.push(JSON.parse(body.message)); return {sha: (++tags).toString(16).padStart(40, '0')}; }
      if (path.endsWith('/git/refs')) return {object: {sha: body.sha}};
      if (path.endsWith('/contents/queue.json')) {
        if (mode === 'conflict') throw new Error('CAS conflict');
        s.queue = JSON.parse(Buffer.from(body.content, 'base64'));
        s.file.sha = 'a'.repeat(40);
        if (mode === 'lost' || mode === 'unconfirmed') throw new Error('lost response');
        return {content: {sha: s.file.sha}};
      }
      throw new Error('unexpected write');
    }
    const p = makeSnapshot().provenance;
    if (path === `repos/${CODE}/releases/tags/${p.release.tag_name}`) return s.provenance.release;
    if (path === `repos/${CODE}/actions/runs/${p.build.id}`) return s.provenance.build;
    if (path === `repos/${CODE}/git/ref/tags/${p.release.tag_name}`) return s.provenance.ref;
    if (path === `repos/${CODE}/git/tags/${p.tag.sha}`) return s.provenance.tag;
    if (path.startsWith(`repos/${CODE}/contents/router/src/coordinator.js?ref=`)) {
      assert.equal(path, `repos/${CODE}/contents/router/src/coordinator.js?ref=${s.health.version}`);
      if (mode === 'missing-code') throw new Error('source unavailable');
      return {sha: head, encoding: 'base64', content: s.deployedSource.toString('base64')};
    }
    if (path === `repos/${OPS}`) return s.opsRepo;
    if (path.startsWith(`repos/${OPS}/contents/queue.json`)) {
      queueReads++;
      if (queueReads > 2 && mode === 'unconfirmed') throw new Error('readback unavailable');
      if (queueReads === 2 && mode === 'generation') s.queue.records[0].generation = '1'.repeat(36);
      if (queueReads === 2 && mode === 'request') s.req.instructions = 'changed';
      if (queueReads === 2 && mode === 'active') { s.runs.workflow_runs.push({...run, id: 777, status: 'in_progress'}); s.runs.total_count++; }
      return {...s.file, encoding: 'base64', content: Buffer.from(JSON.stringify(s.queue)).toString('base64')};
    }
    if (path.includes('/git/ref/tags/')) return s.ref;
    if (path.includes('/git/tags/')) return {...s.tag, message: JSON.stringify(s.req)};
    if (path === `repos/${req.repository}`) return s.repo;
    if (path.includes('/contents/.github/workflows/')) return {sha: head, encoding: 'base64', content: s.workflow.toString('base64')};
    if (path.includes('/actions/workflows/')) return s.runs;
    if (path.includes('/attempts/1/jobs')) return s.jobs;
    if (path.includes('/actions/runs/')) return s.run;
    throw new Error('unexpected read');
  }};
  io.optionalRepo = async path => {
    if (mode === 'repo403' || mode === 'repo500') throw new GitHubHttpError(mode === 'repo403' ? 403 : 500);
    return io.api(path);
  };
  return {io, s, writes, evidence, get healthReads() { return healthReads; }};
}

{
  const s = snapshot(); s.log = s.log.replaceAll('\t2026-', '\t\uFEFF2026-');
  assert.doesNotThrow(() => verify(s, input));
}
{
  const s = snapshot();
  s.req.instructions = 'Document phase one';
  s.log = s.log.replaceAll(JSON.stringify(req), JSON.stringify(s.req));
  assert.doesNotThrow(() => verify(s, input), 'immutable instructions are not native lifecycle evidence');
}
// Regression first: server persists PUT but client loses response.
{
  const m = mock('lost');
  const result = await repair(m.io, {...input, apply: true});
  assert.equal(result.result, 'applied-association');
  assert.equal(m.s.queue.records[0].state, 'bound');
  assert.ok(!m.evidence.some(e => e.state === 'not-applied'), 'accepted PUT must never emit false not-applied');
}
{
  const m = mock();
  const result = await repair(m.io, input);
  assert.equal(result.result, 'not-applied');
  assert.equal(m.writes.length, 0);
  assert.equal(options(['--delivery', delivery, '--run', String(input.run)]).apply, false);
}
for (const mode of ['unconfirmed', 'annotation-lost']) {
  const m = mock(mode);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.s.queue.records[0].state, 'bound');
  assert.ok(!m.evidence.some(e => e.state === 'not-applied'));
  if (mode === 'unconfirmed') assert.equal(m.evidence.at(-1).state, 'association-unconfirmed');
  assert.ok(!FAILURE_MESSAGE.includes('not-applied'));
}
for (const mode of ['recheck-version', 'recheck-source', 'recheck-health-error']) {
  const m = mock(mode);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.healthReads, 2, 'both initial deployment and immediate pre-CAS recheck ran');
  assert.equal(m.evidence[0].state, 'candidate-verified,binding-pending', 'initial deployment passed before candidate annotation');
  assert.equal(m.writes.filter(w => w.path.endsWith('/contents/queue.json')).length, 0, 'deployment change prevents association PUT');
  assert.equal(m.s.queue.records[0].state, 'dispatching');
  assert.ok(!m.evidence.some(e => e.state === 'applied-association'));
  assert.equal(m.evidence.at(-1).state, 'not-applied');
}
for (const mode of ['generation', 'request', 'active', 'conflict']) {
  const m = mock(mode);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.s.queue.records[0].state, 'dispatching');
  assert.ok(!m.evidence.some(e => e.state === 'applied-association'));
}
{
  const m = mock();
  const result = await repair(m.io, {...input, apply: true});
  assert.equal(result.result, 'applied-association');
  assert.equal(m.s.queue.records[0].candidateEvidenceTagSHA, result.candidateEvidenceTagSHA);
  assert.equal(m.evidence[0].state, 'candidate-verified,binding-pending');
  assert.equal(m.evidence[0].deployment.version, head);
  assert.equal(m.evidence[0].deployment.coordinatorDigest, COORDINATOR_SHA256);
  assert.equal(m.evidence[0].historicalAssetId, 707806211);
  assert.equal(m.evidence.at(-1).state, 'applied-association');
  assert.equal(m.writes.filter(w => w.path.endsWith('/contents/queue.json')).length, 1);
}
for (const mutate of [
  s => { s.run.id++; },
  s => { s.opsRepo.default_branch = 'main'; },
  s => { s.provenance.release.tag_name = 'v3'; },
  s => { s.provenance.release.assets = []; },
  s => { s.provenance.release.assets[0].id++; },
  s => { s.provenance.release.assets[0].digest = 'sha256:' + '0'.repeat(64); },
  s => { s.provenance.release.assets[0].created_at = '2026-11-04T01:37:14Z'; },
  s => { s.provenance.release.assets[0].updated_at = '2026-11-04T01:37:14Z'; },
  s => { s.provenance.release.assets[0].uploader.login = 'someone'; },
  s => { s.provenance.release.assets.push({...s.provenance.release.assets[0]}); },
  s => { s.provenance.ref.ref = 'refs/tags/v3'; },
  s => { s.provenance.ref.object.type = 'commit'; },
  s => { s.provenance.ref.object.sha = head; },
  s => { s.provenance.tag.sha = head; },
  s => { s.provenance.tag.tag = 'v3'; },
  s => { s.provenance.tag.object.sha = head; },
  s => { s.provenance.tag.object.type = 'tag'; },
  s => { s.provenance.build.id++; },
  s => { s.provenance.build.head_sha = head; },
  s => { s.provenance.build.event = 'workflow_dispatch'; },
  s => { s.provenance.build.head_branch = 'trunk'; },
  s => { s.provenance.build.conclusion = 'failure'; },
  s => { s.provenance.build.repository.full_name = OPS; },
  s => { s.provenance.build.created_at = '2026-11-04T01:29:20Z'; },
  s => { s.provenance.build.updated_at = '2026-11-04T01:37:18Z'; },
  s => { s.run.created_at = '2026-11-04T01:37:12Z'; },
  ...['GOLEM LIFECYCLE: ADMITTED', 'Golem-Life-Cycle ->', 'PHASE: started', 'phase provider-started', 'admitted', 'ADMITTED', 'containers-destroyed', 'publication credentials minted', 'publication_complete', 'reservation:released'].map(marker => s => { s.log += line(marker); }),
  s => { s.queue.records[0].generation = 'bad'; },
  s => { s.req.instructions = 'changed body'; },
  s => { s.workflow = Buffer.from('wrong phase pin'); },
  s => { s.log = ''; },
  s => { s.log += line(`GOLEM_REQUEST: ${JSON.stringify(req)}`); },
  s => { s.log += line('golem lifecycle: admitted'); },
  s => { s.log = s.log.replace('queue request denied', 'queue unauthorized'); },
  s => { s.log = s.log.replace(/.*Ensure cancellation.*\n/, ''); },
  s => { s.runs.workflow_runs.push({...run, id: 777, status: 'queued'}); s.runs.total_count++; },
  s => { s.runs.total_count = 101; },
  s => { s.runs.workflow_runs.push({...run, id: 777, display_title: 'another target', status: 'in_progress'}); s.runs.total_count++; },
  s => { s.jobs.jobs.push({...s.jobs.jobs[0], name: 'unexpected'}); s.jobs.total_count++; },
  s => { s.repo.private = false; },
  s => { s.ref.object.sha = head; },
  s => { s.run.run_attempt = 2; },
]) {
  const s = snapshot(); mutate(s); assert.throws(() => verify(s, input));
  const m = mock(); mutate(m.s);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0, 'failed candidate proof must precede all writes');
}
for (const mutate of [
  s => { s.health = null; },
  s => { s.health.app = 'wrong'; },
  s => { delete s.health.version; },
  s => { s.health.version = 'v2'; },
  s => { s.health.version = 'g'.repeat(40); },
  s => { s.deployedSource = Buffer.from('old deployed coordinator'); },
  s => { s.deployedSource = Buffer.concat([coordinator, Buffer.from('\\n')]); },
]) {
  const m = mock(); mutate(m.s);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0, 'deployment gate must precede even evidence writes');
  assert.equal((await repair(m.io, input)).evidence, 'candidate-verified', 'inspection does not require current deployment');
}
for (const mode of ['health-error', 'missing-code']) {
  const m = mock(mode);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0);
}
// Exercise the HTTP boundary independently of the in-memory repair transport.
const http = (status, body) => `HTTP/2.0 ${status} Test\r\nContent-Type: application/json\r\nX-Test: bounded\r\n\r\n${body}`;
const targetPath = `repos/${req.repository}`;
{
  const m = mock('', snapshotV3);
  const io = client(async args => {
    assert.deepEqual(args, ['api', targetPath, '--method', 'GET', '--include']);
    return http(200, JSON.stringify(m.s.repo));
  });
  assert.deepEqual(await io.optionalRepo(targetPath), m.s.repo);
}
for (const execute of [
  async () => http(404, '{"message":"Not Found"}'),
  async () => { throw new GitHubHttpError(404); },
]) {
  assert.equal(await client(execute).optionalRepo(targetPath), null);
  const m = mock('', snapshotV3);
  m.io.optionalRepo = client(execute).optionalRepo;
  const result = await repair(m.io, {...input, apply: true});
  assert.equal(result.result, 'applied-association');
  assert.equal(m.s.queue.records[0].state, 'bound');
  assert.equal(m.healthReads, 2);
  assert.equal(m.evidence[0].targetUnavailable, true);
  assert.equal(m.evidence[0].historicalAssetId, 708587759);
  assert.equal(m.evidence[0].historicalAssetDigest, 'sha256:8aa6f65d1846354792bbc5bef64af84b22f2194a209d125060c037f755c88373');
  assert.equal(m.evidence[0].releaseBuildRun, 41788750529);
  assert.equal(m.evidence[0].workflowDigest, digest(workflowV3));
  assert.equal(m.evidence.at(-1).state, 'applied-association');
  assert.equal(m.writes.filter(w => w.path.endsWith('/contents/queue.json')).length, 1);
  assert.ok(m.writes.every(w => w.path.startsWith(`repos/${OPS}/`)), 'never write target repository');
  const v2 = mock(); v2.io.optionalRepo = client(execute).optionalRepo;
  await assert.rejects(repair(v2.io, {...input, apply: true}));
  assert.equal(v2.writes.length, 0, 'v2 must retain target identity requirement');
}
for (const execute of [
  ...[403, 500, 503].flatMap(status => [async () => http(status, '{}'), async () => { throw new GitHubHttpError(status); }]),
  async () => 'not HTTP',
  async () => '{"status":404}',
  async () => http(200, '{'),
  async () => http(200, 'null'),
  async () => http(200, '[]'),
  async () => http(200, '404'),
  async () => http(302, '{}'),
  async () => { const error = new Error('404'); error.status = 404; throw error; },
  async () => { throw new Error('network unavailable'); },
]) {
  const getter = client(execute).optionalRepo;
  await assert.rejects(getter(targetPath));
  const m = mock('', snapshotV3); m.io.optionalRepo = getter;
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0, 'only explicit typed or HTTP 404 permits missing target');
}
{
  let calls = 0;
  await assert.rejects(client(async () => { calls++; return http(404, '{}'); }).optionalRepo(`repos/${CODE}`));
  assert.equal(calls, 0, 'optional lookup cannot inspect outside approved sandbox prefix');
}
for (const unavailable of [false, true]) {
  const make = () => { const s = snapshotV3(); if (unavailable) s.repo = null; return s; };
  const m = mock('', make);
  const result = await repair(m.io, input);
  assert.equal(result.result, 'not-applied');
  assert.equal(result.targetUnavailable, unavailable);
  assert.equal(result.phaseProtocol, 'v3:admitted-before-provider');
  assert.equal(m.writes.length, 0);
  assert.equal(m.healthReads, 0);
  for (const mode of ['repo403', 'repo500', 'generation', 'request', 'active', 'conflict', 'health-error', 'missing-code', 'recheck-version', 'recheck-source', 'recheck-health-error']) {
    const n = mock(mode, make);
    await assert.rejects(repair(n.io, {...input, apply: true}));
    assert.equal(n.s.queue.records[0].state, 'dispatching');
    assert.ok(!n.evidence.some(e => e.state === 'applied-association'));
    if (mode !== 'conflict') assert.equal(n.writes.filter(w => w.path.endsWith('/contents/queue.json')).length, 0);
  }
}
{
  const m = mock('', snapshotV3);
  m.s.req.instructions = 'Document phase one; admitted is instruction data';
  m.s.log = m.s.log.replaceAll(JSON.stringify(req), JSON.stringify(m.s.req));
  const result = await repair(m.io, {...input, apply: true});
  assert.equal(result.result, 'applied-association');
  assert.equal(m.evidence[0].targetUnavailable, false);
  assert.equal(m.healthReads, 2);
  assert.ok(m.writes.every(w => w.path.startsWith(`repos/${OPS}/`)));
}
for (const mode of ['lost', 'unconfirmed', 'annotation-lost']) {
  const m = mock(mode, snapshotV3); m.s.repo = null;
  if (mode === 'lost') assert.equal((await repair(m.io, {...input, apply: true})).result, 'applied-association');
  else await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.s.queue.records[0].state, 'bound');
  assert.ok(!m.evidence.some(e => e.state === 'not-applied'), 'v3 lost responses cannot produce false rejection evidence');
}
{
  const m = mock('', snapshotV3);
  let lookups = 0;
  m.io.optionalRepo = async () => ++lookups === 1 ? m.s.repo : null;
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.filter(w => w.path.endsWith('/contents/queue.json')).length, 0, 'target availability change invalidates fresh proof');
}
const v3Negatives = [
  s => { s.provenance.release.tag_name = 'v2'; },
  s => { s.provenance.release.draft = true; },
  s => { s.provenance.release.immutable = true; },
  s => { s.provenance.release.assets = []; },
  s => { s.provenance.release.assets.push({...s.provenance.release.assets[0]}); },
  ...['id', 'digest', 'created_at', 'updated_at'].map(key => s => { s.provenance.release.assets[0][key] = 'wrong'; }),
  s => { s.provenance.release.assets[0].uploader.login = 'someone'; },
  ...['id', 'head_sha', 'event', 'head_branch', 'status', 'conclusion', 'created_at', 'updated_at'].map(key => s => { s.provenance.build[key] = 'wrong'; }),
  s => { s.provenance.build.repository.full_name = OPS; },
  s => { s.provenance.ref.ref = 'refs/tags/v2'; },
  s => { s.provenance.ref.object.type = 'commit'; },
  s => { s.provenance.ref.object.sha = head; },
  s => { s.provenance.tag.sha = head; },
  s => { s.provenance.tag.tag = 'v2'; },
  s => { s.provenance.tag.object.type = 'tag'; },
  s => { s.provenance.tag.object.sha = head; },
  s => { s.run.created_at = '2026-11-04T07:06:05Z'; },
  s => { s.run.run_attempt = 2; },
  s => { s.run.status = 'in_progress'; },
  s => { s.run.conclusion = 'success'; },
  s => { s.run.id++; },
  s => { s.run.repository.id++; },
  s => { s.run.path = '.github/workflows/other.yml'; },
  s => { s.run.event = 'push'; },
  s => { s.run.head_branch = 'other'; },
  s => { s.opsRepo.private = false; },
  s => { s.queue.records[0].state = 'bound'; },
  s => { s.queue.records[0].dispatched = false; },
  s => { s.queue.records[0].attempt = 1; },
  s => { s.queue.records[0].generation = 'bad'; },
  s => { s.ref.object.sha = head; },
  s => { s.req.instructions = 'altered full request'; },
  s => { s.req.repository = CODE; },
  s => { s.req.repository_id = 0; },
  s => { s.req.repository_id = '44'; },
  s => { s.workflow = Buffer.concat([workflowV3, Buffer.from('\n')]); },
  s => { s.workflow = workflow; },
  s => { s.log = ''; },
  s => { s.log = 'x'.repeat(16_000_000); },
  s => { s.queue.records = Array.from({length: 33}, () => structuredClone(row)); },
  s => { s.log = s.log.replace('queue conflict', 'queue request denied'); },
  s => { s.log += line('golem runner failed: queue conflict'); },
  s => { s.log += line(`GOLEM_REQUEST: ${JSON.stringify(req)}`); },
  s => { s.log += 'runner\tAnother step\t2026-11-01T01:00:03.0000000Z GOLEM_REQUEST: {}\n'; },
  ...['golem lifecycle: admitted', 'PHASE: provider-started', 'admitted', 'containers-destroyed', 'publication_complete', 'reservation:released'].flatMap(marker => [
    s => { s.log += line(marker); },
    s => { s.log += `runner\tAnother step\t2026-11-01T01:00:03.0000000Z ${marker}\n`; },
  ]),
  s => { s.log += line('logs truncated'); },
  s => { s.log = s.log.replace(/.*Ensure cancellation.*\n/, ''); },
  s => { s.jobs.jobs[0].steps.find(step => step.name === 'Run exactly one golem').conclusion = 'success'; },
  s => { s.jobs.total_count = 2; },
  s => { s.runs.total_count = 100; },
  s => { s.runs.workflow_runs.push({...s.run, id: 777}); s.runs.total_count++; },
  s => { s.runs.workflow_runs.push({...s.run, id: 777, display_title: 'another', status: 'queued'}); s.runs.total_count++; },
  s => { s.queue.records.push({...s.queue.records[0], id: generation, state: 'bound', run_id: 777}); },
];
for (const mutate of v3Negatives) {
  const m = mock('', snapshotV3); m.s.repo = null; mutate(m.s);
  assert.throws(() => verify(m.s, input));
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0, 'v3 candidate failure precedes all writes');
}
for (const mutate of [
  s => { s.repo.private = false; },
  s => { s.repo.id++; },
  s => { s.repo.full_name = CODE; },
  s => { s.repo.topics = []; },
]) {
  const m = mock('', snapshotV3); mutate(m.s);
  await assert.rejects(repair(m.io, {...input, apply: true}));
  assert.equal(m.writes.length, 0, 'v3 HTTP 200 retains private/id/name/topic guards');
}
console.log('repair-unbound: v2 preservation, v3 available/unavailable targets, HTTP status classification, provenance/lifecycle guards, deployment rechecks, read-only default and guarded CAS tests passed');
