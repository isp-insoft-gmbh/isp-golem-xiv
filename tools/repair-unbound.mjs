#!/usr/bin/env node
import {spawn} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {isDeepStrictEqual} from 'node:util';
import {pathToFileURL} from 'node:url';

export const OPS = 'isp-insoft-gmbh/isp-golem-xiv-ops';
export const CODE = 'isp-insoft-gmbh/isp-golem-xiv';
export const HEALTH = 'https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/health';
export const COORDINATOR_SHA256 = '306f4256092cb3d9dd6848452dea8e0f61e40bbb5947b52468740c59fe2da5e9';
const RELEASE_COMMIT = '0a9555f6c510085d251a6acf891e6740a9cabad4';
const RELEASE_TAG = '1fe798c3fed7f7206985c5ea3e944c2dbfde31d4';
const ASSET_ID = 707806211;
const ASSET_DIGEST = 'sha256:3a12f77a8b4b7197aa302fb94d298a282faefb01c1794404f000aae3f9c6a838';
const ASSET_TIME = '2026-11-04T01:37:13Z';
export const WORKFLOW_SHA256 = '59706b32306a3425f25f50e77c0317013ca467809f48c7053b20553881580f84';
// Reviewed workflow downloads v2, whose first native lifecycle marker is admitted.
export const PHASE_PROTOCOL = 'v2:admitted-before-provider';
export const FAILURE_MESSAGE = 'repair failed or outcome unconfirmed; association may have occurred; inspect private queue and evidence before any further action';
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const sha = /^[0-9a-f]{40}$/;
const positive = x => Number.isSafeInteger(x) && x > 0;
function need(ok, reason) { if (!ok) throw new Error(reason); }
export function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(',')}}`;
  return JSON.stringify(value);
}
export const digest = value => createHash('sha256').update(value).digest('hex');
function decode(file) {
  need(file.encoding === 'base64' && typeof file.content === 'string' && file.content.length < 400_000 && sha.test(file.sha), 'invalid private file');
  return Buffer.from(file.content.replace(/\s/g, ''), 'base64');
}
export function options(args) {
  const out = {apply: false};
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === '--inspect' || arg === '--apply') { need(!out.mode, 'duplicate mode'); out.mode = arg; out.apply = arg === '--apply'; }
    else if (arg === '--delivery' || arg === '--run') { const key = arg.slice(2); need(out[key] === undefined && i + 1 < args.length, 'invalid arguments'); out[key] = args[++i]; }
    else throw new Error('invalid arguments');
  }
  need(uuid.test(out.delivery ?? '') && /^[1-9][0-9]{0,15}$/.test(out.run ?? '') && positive(Number(out.run)), 'delivery UUID and run integer required');
  out.run = Number(out.run);
  return out;
}
export function verifyLogs(log, req, jobs) {
  need(typeof log === 'string' && log.length > 0 && Buffer.byteLength(log) < 16_000_000 && !/\u001b|\ufffd|truncated|truncation|truncate|truncating|logs? (?:unavailable|expired)/i.test(log), 'missing or incomplete logs');
  need(jobs.total_count === 1 && jobs.jobs?.length === 1, 'incomplete or conflicting jobs');
  const trusted = jobs.jobs.filter(job => job.name === 'runner');
  need(trusted.length === 1 && trusted[0].status === 'completed' && trusted[0].conclusion === 'failure', 'trusted job mismatch');
  const steps = trusted[0].steps ?? [];
  const native = steps.filter(step => step.name === 'Run exactly one golem');
  need(native.length === 1 && native[0].status === 'completed' && native[0].conclusion === 'failure', 'trusted step mismatch');
  need(steps.some(step => step.number > native[0].number && step.name === 'Ensure cancellation cannot leave an agent alive' && step.status === 'completed'), 'missing cleanup boundary');
  const lines = log.split('\n').filter(Boolean).map(line => {
    const match = line.replace(/\r$/, '').match(/^([^\t]+)\t([^\t]+)\t\uFEFF?(\d{4}-\d\d-\d\dT[^ ]+Z) (.*)$/);
    need(match, 'invalid full log framing');
    return {job: match[1], step: match[2], text: match[4]};
  });
  const trustedLines = lines.filter(line => line.job === 'runner' && line.step === 'Run exactly one golem');
  need(trustedLines.length > 0 && lines.some(line => line.job === 'runner' && line.step === 'Ensure cancellation cannot leave an agent alive'), 'missing full step logs');
  const env = trustedLines.filter(line => /^\s*GOLEM_REQUEST: /.test(line.text));
  need(env.length === 1, 'request env absent or nonunique');
  // Request instructions are data; every other lifecycle-like line fails closed.
  const lifecycle = /\bgolem[\s_:\-]*life[\s_:\-]*cycle\b|\blifecycle\b|\bphase\b|\badmitted\b|\bcontainers[\s_:\-]*destroyed\b|\bpublication[\s_:\-]*(?:credentials[\s_:\-]*minted|complete)\b|\breservation[\s_:\-]*released\b/i;
  need(!trustedLines.some(line => line !== env[0] && lifecycle.test(line.text)), 'native admission or later phase present');
  let parsed;
  try { parsed = JSON.parse(env[0].text.trim().slice('GOLEM_REQUEST: '.length)); } catch { throw new Error('invalid logged request'); }
  need(isDeepStrictEqual(parsed, req), 'logged immutable request mismatch');
  need(trustedLines.filter(line => line.text === 'golem runner failed: queue request denied').length === 1, 'exact pre-admission fatal absent or nonunique');
  return digest(log);
}
export async function verifyDeployment(io) {
  const health = await io.health();
  need(health?.app === 'isp-golem-xiv' && typeof health.version === 'string' && sha.test(health.version), 'worker health identity or version mismatch');
  const file = await io.api(`repos/${CODE}/contents/router/src/coordinator.js?ref=${health.version}`);
  need(digest(decode(file)) === COORDINATOR_SHA256, 'deployed coordinator is not reviewed repair code');
  return {version: health.version, coordinatorDigest: COORDINATOR_SHA256};
}
export function verifyProvenance(provenance, failedRun) {
  need(provenance && failedRun, 'missing historical runner provenance');
  const {release, build, ref, tag} = provenance;
  // v2 is NOT an immutable release. Exact asset identity and timestamps fence replacement/rename.
  need(release?.tag_name === 'v2' && release.draft === false && release.immutable === false && Array.isArray(release.assets) && release.assets.length < 100, 'release identity mismatch');
  const assets = release.assets.filter(asset => asset.name === 'golem-runner-linux-x86_64');
  need(assets.length === 1, 'historical binary asset absent or nonunique');
  const asset = assets[0];
  need(asset.id === ASSET_ID && asset.digest === ASSET_DIGEST && asset.created_at === ASSET_TIME && asset.updated_at === ASSET_TIME && asset.uploader?.login === 'github-actions[bot]', 'historical binary asset identity mismatch');
  need(build?.id === 41754384511 && build.repository?.full_name === CODE && build.head_sha === RELEASE_COMMIT && build.event === 'push' && build.head_branch === 'v2' && build.status === 'completed' && build.conclusion === 'success' && build.created_at === '2026-11-04T01:29:19Z' && build.updated_at === '2026-11-04T01:37:17Z', 'release build provenance mismatch');
  need(ref?.ref === 'refs/tags/v2' && ref.object?.type === 'tag' && ref.object.sha === RELEASE_TAG && tag?.sha === RELEASE_TAG && tag.tag === 'v2' && tag.object?.type === 'commit' && tag.object.sha === RELEASE_COMMIT, 'release annotated tag provenance mismatch');
  const assetTime = Date.parse(ASSET_TIME);
  need(Date.parse(build.created_at) <= assetTime && assetTime <= Date.parse(build.updated_at) && assetTime < Date.parse(failedRun.created_at), 'historical binary postdates failed run');
}
export function verify(snapshot, input) {
  const {queue, file, req, tag, ref, run, repo, opsRepo, workflow, runs, jobs, log} = snapshot;
  need(opsRepo.full_name === OPS && opsRepo.private === true && opsRepo.default_branch === 'trunk', 'private ops identity mismatch');
  verifyProvenance(snapshot.provenance, run);
  need(queue.version === 2 && Array.isArray(queue.records) && queue.records.length <= 32 && sha.test(file.sha), 'invalid queue');
  const selected = queue.records.filter(r => r.id === input.delivery);
  need(selected.length === 1, 'row absent or nonunique');
  const row = selected[0];
  need(row.state === 'dispatching' && row.dispatched === true && row.run_id === null && row.attempt === null && uuid.test(row.generation) && sha.test(row.tag), 'row is not unbound dispatching');
  need(ref.ref === `refs/tags/golem-request/${row.id}` && ref.object?.type === 'tag' && ref.object.sha === row.tag && tag.sha === row.tag && tag.tag === `golem-request/${row.id}`, 'immutable request ref mismatch');
  need(req.version === 2 && req.delivery_id === row.id && req.queue_generation === row.generation && row.resource === `${req.repository.toLowerCase()}/${req.golem}` && row.seat === `${req.seat_login.toLowerCase()}/${req.provider}`, 'request generation or resource mismatch');
  need(/^isp-insoft-gmbh\/isp-golem-xiv-test-[A-Za-z0-9-]+$/.test(req.repository) && /^[A-Za-z0-9_-]{1,64}$/.test(req.golem), 'target outside sandbox');
  need(repo.private === true && repo.id === req.repository_id && repo.full_name === req.repository && repo.topics?.includes('isp-golem-xiv-test'), 'target private identity or topic mismatch');
  const title = `${req.repository} ${req.golem}`;
  const matches = r => r.display_title === title && r.path === '.github/workflows/runner.yml' && r.event === 'workflow_dispatch' && r.head_branch === 'trunk';
  need(run.id === input.run && run.repository?.full_name === OPS && run.repository?.id === opsRepo.id && run.status === 'completed' && run.conclusion === 'failure' && run.run_attempt === 1 && sha.test(run.head_sha) && matches(run), 'failed run identity mismatch');
  need(!queue.records.some(r => r !== row && (r.run_id === input.run || (r.state !== 'terminal' && r.state !== 'pending' && (r.seat === row.seat || r.resource === row.resource)))), 'active queue peer conflict');
  need(digest(workflow) === WORKFLOW_SHA256 && workflow.toString('utf8').includes('/releases/download/v2\n'), 'unreviewed workflow or phase protocol');
  need(runs.total_count === runs.workflow_runs?.length && runs.total_count > 0 && runs.total_count < 100, 'run window incomplete or overflow');
  const candidates = runs.workflow_runs.filter(matches);
  need(candidates.length === 1 && candidates[0].id === input.run && candidates[0].status === 'completed' && candidates[0].conclusion === 'failure' && candidates[0].run_attempt === 1, 'ambiguous or active run peer');
  need(runs.workflow_runs.every(r => r.status === 'completed'), 'active ops workflow peer');
  const start = Date.parse(tag.tagger?.date), created = Date.parse(run.created_at), end = Date.parse(snapshot.windowEnd);
  need(Number.isFinite(start) && Number.isFinite(end) && start <= created && created <= end && end <= Date.now() + 60_000, 'invalid request run window');
  const logDigest = verifyLogs(log, req, jobs);
  return {row, requestDigest: digest(canonical(req)), workflowDigest: WORKFLOW_SHA256, logDigest, preQueueBlobSHA: file.sha, phaseProtocol: PHASE_PROTOCOL};
}

// Bounded subprocess capture: stderr and API payloads never enter diagnostics.
export async function gh(args, body, limit = 2_000_000) {
  return new Promise((resolve, reject) => {
    const child = spawn('gh', args, {stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true, timeout: 30_000});
    const chunks = []; let size = 0, overflow = false;
    child.stdout.on('data', chunk => { size += chunk.length; if (size >= limit) { overflow = true; child.kill(); } else chunks.push(chunk); });
    child.on('error', () => reject(new Error('GitHub CLI unavailable')));
    child.on('close', code => code === 0 && !overflow ? resolve(Buffer.concat(chunks).toString('utf8')) : reject(new Error('bounded GitHub operation failed')));
    child.stdin.on('error', () => {});
    child.stdin.end(body === undefined ? undefined : JSON.stringify(body));
  });
}
export function client(execute = gh) {
  return {
    async api(path, method = 'GET', body) {
      const text = await execute(['api', path, '--method', method, ...(body === undefined ? [] : ['--input', '-'])], body);
      try { return JSON.parse(text); } catch { throw new Error('invalid GitHub JSON'); }
    },
    log: run => execute(['run', 'view', String(run), '--repo', OPS, '--log'], undefined, 16_000_000),
    async health() {
      const response = await fetch(HEALTH, {redirect: 'manual', signal: AbortSignal.timeout(10_000)});
      need(response.status === 200, 'worker health unavailable');
      const reader = response.body?.getReader();
      need(reader, 'worker health absent');
      const chunks = []; let size = 0;
      try {
        for (let i = 0; i < 4096; i++) {
          const {done, value} = await reader.read();
          if (done) return JSON.parse(Buffer.concat(chunks).toString('utf8'));
          size += value.length;
          need(size <= 4096, 'worker health oversized');
          chunks.push(value);
        }
        throw new Error('worker health oversized');
      } finally { await reader.cancel(); }
    },
  };
}
export async function inspect(io, input) {
  const base = `repos/${OPS}`;
  const opsRepo = await io.api(base);
  const file = await io.api(`${base}/contents/queue.json?ref=golem-state`);
  const queue = JSON.parse(decode(file).toString('utf8'));
  const rows = queue.records?.filter(r => r.id === input.delivery);
  need(rows?.length === 1 && sha.test(rows[0].tag), 'row absent or invalid');
  const ref = await io.api(`${base}/git/ref/tags/golem-request/${input.delivery}`);
  const tag = await io.api(`${base}/git/tags/${rows[0].tag}`);
  need(typeof tag.message === 'string' && tag.message.length <= 65_536, 'invalid immutable request');
  const req = JSON.parse(tag.message);
  need(/^isp-insoft-gmbh\/isp-golem-xiv-test-[A-Za-z0-9-]+$/.test(req.repository ?? ''), 'target outside sandbox');
  const repo = await io.api(`repos/${req.repository}`);
  const run = await io.api(`${base}/actions/runs/${input.run}`);
  need(sha.test(run.head_sha), 'invalid workflow commit');
  const workflow = decode(await io.api(`${base}/contents/.github/workflows/runner.yml?ref=${run.head_sha}`));
  const publicBase = `repos/${CODE}`;
  const provenance = {
    release: await io.api(`${publicBase}/releases/tags/v2`),
    build: await io.api(`${publicBase}/actions/runs/41754384511`),
    ref: await io.api(`${publicBase}/git/ref/tags/v2`),
    tag: await io.api(`${publicBase}/git/tags/${RELEASE_TAG}`),
  };
  need(Number.isFinite(Date.parse(tag.tagger?.date)), 'missing request timestamp');
  const log = await io.log(input.run);
  const jobs = await io.api(`${base}/actions/runs/${input.run}/attempts/1/jobs?per_page=100`);
  const windowEnd = new Date().toISOString();
  // No status filter: completed, queued, waiting and running all participate.
  // Keep peer discovery last, immediately before verification and possible CAS.
  const window = `${tag.tagger.date}..${windowEnd}`;
  const runs = await io.api(`${base}/actions/workflows/runner.yml/runs?per_page=100&created=${encodeURIComponent(window)}`);
  const snapshot = {queue, file, req, tag, ref, run, repo, opsRepo, workflow, runs, jobs, log, windowEnd, provenance};
  return {snapshot, proof: verify(snapshot, input)};
}
async function annotation(io, prefix, evidence) {
  const base = `repos/${OPS}`;
  const name = `golem-repair/${prefix}/${randomUUID()}`;
  const tag = await io.api(`${base}/git/tags`, 'POST', {tag: name, message: JSON.stringify(evidence), object: evidence.preQueueBlobSHA, type: 'blob'});
  need(sha.test(tag.sha), 'invalid evidence tag');
  const ref = await io.api(`${base}/git/refs`, 'POST', {ref: `refs/tags/${name}`, sha: tag.sha});
  need(ref.object?.sha === tag.sha, 'evidence reference not persisted');
  return tag.sha;
}
export async function repair(io, input) {
  const first = await inspect(io, input);
  if (!input.apply) return {result: 'not-applied', evidence: 'candidate-verified', ...publicEvidence(first.proof, input)};
  const deployment = await verifyDeployment(io); // Must precede even the evidence annotation.
  const evidence = {...publicEvidence(first.proof, input), deployment, state: 'candidate-verified,binding-pending'};
  const evidenceTag = await annotation(io, 'candidate', evidence);
  let result = 'not-applied';
  try {
    // Re-fetch logs, all-status run window, queue, request and job state immediately before CAS.
    const fresh = await inspect(io, input);
    need(fresh.proof.preQueueBlobSHA === first.proof.preQueueBlobSHA && isDeepStrictEqual(fresh.proof, first.proof), 'CAS predicates changed');
    need(isDeepStrictEqual(await verifyDeployment(io), deployment), 'deployment changed before CAS');
    const row = fresh.snapshot.queue.records.find(r => r.id === input.delivery);
    Object.assign(row, {state: 'bound', run_id: input.run, attempt: 1, candidateEvidenceTagSHA: evidenceTag});
    result = 'association-unconfirmed';
    try {
      const updated = await io.api(`repos/${OPS}/contents/queue.json`, 'PUT', {branch: 'golem-state', sha: first.proof.preQueueBlobSHA, message: 'Associate verified pre-admission failed run', content: Buffer.from(JSON.stringify(fresh.snapshot.queue)).toString('base64')});
      need(sha.test(updated.content?.sha), 'association write unconfirmed');
      result = 'applied-association';
    } catch {
      // A timeout is not proof of rejection. Only exact read-back can confirm success.
      try {
        const file = await io.api(`repos/${OPS}/contents/queue.json?ref=golem-state`);
        const observed = JSON.parse(decode(file).toString('utf8'));
        const matches = observed.records.filter(r => r.id === input.delivery);
        if (matches.length === 1 && isDeepStrictEqual(matches[0], row)) result = 'applied-association';
      } catch { /* Preserve uncertainty, never publish a false negative. */ }
      need(result === 'applied-association', 'association unconfirmed');
    }
  } finally {
    await annotation(io, 'result', {...evidence, state: result, candidateEvidenceTagSHA: evidenceTag});
  }
  return {result, candidateEvidenceTagSHA: evidenceTag};
}
function publicEvidence(proof, input) {
  return {delivery: input.delivery, run: input.run, requestDigest: proof.requestDigest, workflowDigest: proof.workflowDigest, logDigest: proof.logDigest, preQueueBlobSHA: proof.preQueueBlobSHA, phaseProtocol: proof.phaseProtocol, historicalAssetId: ASSET_ID, historicalAssetDigest: ASSET_DIGEST, releaseBuildRun: 41754384511};
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { console.log(JSON.stringify(await repair(client(), options(process.argv.slice(2))))); }
  catch { console.error(FAILURE_MESSAGE); process.exitCode = 1; }
}
