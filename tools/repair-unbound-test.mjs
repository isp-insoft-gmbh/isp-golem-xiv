import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {repair, verify, options, OPS, CODE, WORKFLOW_SHA256, COORDINATOR_SHA256, digest, FAILURE_MESSAGE} from './repair-unbound.mjs';

const delivery = 'f37f6672-d908-4ffa-6488-c011eb2f94c7';
const generation = '3bdb4eb1-d440-4262-80ef-387193452211';
const input = {delivery, run: 41774622060, apply: false};
const tagSHA = '7'.repeat(40), fileSHA = '8'.repeat(40), head = '9'.repeat(40);
const workflow = await readFile(new URL('../ops/runner.yml', import.meta.url));
assert.equal(digest(workflow), WORKFLOW_SHA256);
const coordinator = await readFile(new URL('../router/src/coordinator.js', import.meta.url));
assert.equal(digest(coordinator), COORDINATOR_SHA256);
function provenance() {
  return {
    release: {tag_name: 'v2', draft: false, immutable: false, assets: [{name: 'golem-runner-linux-x86_64', id: 707806211, digest: 'sha256:3a12f77a8b4b7197aa302fb94d298a282faefb01c1794404f000aae3f9c6a838', created_at: '2026-11-04T01:37:13Z', updated_at: '2026-11-04T01:37:13Z', uploader: {login: 'github-actions[bot]'}}]},
    build: {id: 41754384511, repository: {full_name: CODE}, head_sha: '0a9555f6c510085d251a6acf891e6740a9cabad4', event: 'push', head_branch: 'v2', status: 'completed', conclusion: 'success', created_at: '2026-11-04T01:29:19Z', updated_at: '2026-11-04T01:37:17Z'},
    ref: {ref: 'refs/tags/v2', object: {type: 'tag', sha: '1fe798c3fed7f7206985c5ea3e944c2dbfde31d4'}},
    tag: {sha: '1fe798c3fed7f7206985c5ea3e944c2dbfde31d4', tag: 'v2', object: {type: 'commit', sha: '0a9555f6c510085d251a6acf891e6740a9cabad4'}},
  };
}
const req = {version: 2, delivery_id: delivery, queue_generation: generation, repository: 'isp-insoft-gmbh/isp-golem-xiv-test-repair', repository_id: 44, golem: 'hello', seat_login: 'member', provider: 'anthropic', instructions: 'immutable full body'};
const row = {id: delivery, generation, tag: tagSHA, state: 'dispatching', dispatched: true, run_id: null, attempt: null, resource: `${req.repository}/hello`, seat: 'member/anthropic'};
const run = {id: input.run, repository: {full_name: OPS, id: 33}, status: 'completed', conclusion: 'failure', run_attempt: 1, head_sha: head, display_title: `${req.repository} hello`, path: '.github/workflows/runner.yml', event: 'workflow_dispatch', head_branch: 'trunk', created_at: '2026-11-04T04:40:28Z'};
const line = text => `runner\tRun exactly one golem\t2026-11-01T01:00:02.0000000Z ${text}\n`;
const log = line(`  GOLEM_REQUEST: ${JSON.stringify(req)}`) + line('golem runner failed: queue request denied') + 'runner\tEnsure cancellation cannot leave an agent alive\t2026-11-01T01:00:03.0000000Z cleanup\n';
function snapshot() {
  return {provenance: provenance(), health: {app: 'isp-golem-xiv', version: head}, deployedSource: coordinator, queue: {version: 2, records: [structuredClone(row)]}, file: {sha: fileSHA}, req: structuredClone(req), tag: {sha: tagSHA, tag: `golem-request/${delivery}`, tagger: {date: '2026-11-01T01:00:00Z'}}, ref: {ref: `refs/tags/golem-request/${delivery}`, object: {type: 'tag', sha: tagSHA}}, run: structuredClone(run), repo: {id: 44, private: true, full_name: req.repository, topics: ['isp-golem-xiv-test']}, opsRepo: {id: 33, full_name: OPS, private: true, default_branch: 'trunk'}, workflow, runs: {total_count: 1, workflow_runs: [structuredClone(run)]}, jobs: {total_count: 1, jobs: [{name: 'runner', status: 'completed', conclusion: 'failure', steps: [{number: 6, name: 'Run exactly one golem', status: 'completed', conclusion: 'failure'}, {number: 7, name: 'Ensure cancellation cannot leave an agent alive', status: 'completed'}]}]}, log, windowEnd: new Date().toISOString()};
}
function mock(mode = '') {
  const s = snapshot(), writes = [], evidence = [];
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
    if (path === `repos/${CODE}/releases/tags/v2`) return s.provenance.release;
    if (path === `repos/${CODE}/actions/runs/41754384511`) return s.provenance.build;
    if (path === `repos/${CODE}/git/ref/tags/v2`) return s.provenance.ref;
    if (path === `repos/${CODE}/git/tags/1fe798c3fed7f7206985c5ea3e944c2dbfde31d4`) return s.provenance.tag;
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
  return {io, s, writes, evidence, get healthReads() { return healthReads; }};
}

{
  const s = snapshot(); s.log = s.log.replaceAll('\t2026-', '\t\uFEFF2026-');
  assert.doesNotThrow(() => verify(s, input));
}
{
  const s = snapshot();
  s.req.instructions = 'Document phase one';
  s.log = s.log.replace(JSON.stringify(req), JSON.stringify(s.req));
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
console.log('repair-unbound: deployment gate, historical asset/build/tag provenance, private trunk, lifecycle variants, inspection, CAS ambiguity/conflict and evidence tests passed');
