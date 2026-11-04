import {appJwt} from './runtime.js';

const ops = '/repos/isp-insoft-gmbh/isp-golem-xiv-ops';
const indexPath = `${ops}/contents/queue.json`;
const capacity = 32;
const bodyLimit = 65536;
const journalLimit = 131072;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const states = new Set(['pending', 'reserved', 'dispatching', 'bound', 'dispatch_uncertain', 'stop_requested', 'terminal']);
const positive = value => Number.isSafeInteger(value) && value > 0;
const shares = (a, b) => a.seat === b.seat || a.resource === b.resource;
const encode = value => btoa(String.fromCharCode(...new TextEncoder().encode(JSON.stringify(value))));
const decode = value => JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(Uint8Array.from(atob(value.replace(/\s/g, '')), c => c.charCodeAt(0))));
class Failure extends Error {
  constructor(status) { super('coordinator unavailable'); this.status = status; }
}
async function bounded(response, limit) {
  if (!response.body) return '';
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    for (let count = 0; count < limit; count++) {
      const {value, done} = await reader.read();
      if (done) return new TextDecoder('utf-8', {fatal: true}).decode(Uint8Array.from(chunks));
      size += value.length;
      if (size > limit) throw new Failure(413);
      for (const byte of value) chunks.push(byte);
    }
    throw new Failure(413);
  } finally { await reader.cancel(); }
}

// GitHub is authoritative. The local SQL projection is replaced after every read/write.
// Serialize external I/O explicitly: DO input gates alone do not serialize fetch awaits.
export class Coordinator {
  constructor(ctx, env) {
    this.ctx = ctx;
    this.env = env;
    this.tail = Promise.resolve();
    this.waiters = 0;
    ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS queue_projection (id TEXT PRIMARY KEY, record TEXT NOT NULL)');
  }
  async serial(fn) {
    if (this.waiters >= 16) throw new Failure(503);
    this.waiters++;
    const previous = this.tail;
    let release;
    this.tail = new Promise(resolve => { release = resolve; });
    await previous;
    try { return await fn(); }
    finally { this.waiters--; release(); }
  }
  async api(path, method = 'GET', body, missing = false, jwt = false) {
    if (++this.calls > 45) throw new Failure(503);
    const response = await fetch(`https://api.github.com${path}`, {
      method, redirect: 'manual', signal: AbortSignal.timeout(10000),
      headers: {Authorization: `Bearer ${jwt ? this.jwt : this.token}`, Accept: 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'isp-golem-xiv', 'Content-Type': 'application/json'},
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (response.status === 404 && missing) { await response.body?.cancel(); return null; }
    if (!response.ok || (path.endsWith('/dispatches') && response.status !== 204)) { await response.body?.cancel(); throw new Failure(response.status === 409 || response.status === 422 ? 409 : 503); }
    if (response.status === 204) return null;
    return JSON.parse(await bounded(response, 524288));
  }
  async connect() {
    this.calls = 0;
    this.jwt = await appJwt(this.env.APP_PRIVATE_KEY);
    // No repository_ids or permissions override: preserve this installation's all-repo permissions.
    this.token = (await this.api('/app/installations/153964219/access_tokens', 'POST', {}, false, true)).token;
    if (typeof this.token !== 'string' || this.token.length > 4096) throw new Failure(503);
  }
  project() {
    this.ctx.storage.transactionSync(() => {
      this.ctx.storage.sql.exec('DELETE FROM queue_projection');
      for (const row of this.queue.records) this.ctx.storage.sql.exec('INSERT INTO queue_projection VALUES (?, ?)', row.id, JSON.stringify(row));
    });
  }
  async load() {
    let file = await this.api(`${indexPath}?ref=golem-state`, 'GET', undefined, true);
    if (!file) {
      const branch = await this.api(`${ops}/git/ref/heads/golem-state`, 'GET', undefined, true);
      if (branch) throw new Failure(503); // An existing journal must never silently reset.
      const trunk = await this.api(`${ops}/git/ref/heads/trunk`);
      const commit = await this.api(`${ops}/git/commits/${trunk.object.sha}`);
      const tree = await this.api(`${ops}/git/trees`, 'POST', {base_tree: commit.tree.sha, tree: [{path: 'queue.json', mode: '100644', type: 'blob', content: JSON.stringify({version: 2, records: []})}]});
      const initial = await this.api(`${ops}/git/commits`, 'POST', {message: 'Initialize golem queue', tree: tree.sha, parents: [trunk.object.sha]});
      // Publish branch + journal atomically. Interrupted object creation leaves only unreachable objects.
      await this.api(`${ops}/git/refs`, 'POST', {ref: 'refs/heads/golem-state', sha: initial.sha});
      file = await this.api(`${indexPath}?ref=golem-state`);
    }
    if (file.encoding !== 'base64' || typeof file.content !== 'string' || file.content.length > journalLimit * 2) throw new Failure(503);
    this.queue = decode(file.content);
    this.sha = file.sha;
    if (!/^[0-9a-f]{40}$/.test(this.sha)) throw new Failure(503);
    const q = this.queue;
    if (q.version !== 2 || !Array.isArray(q.records) || q.records.length > capacity) throw new Failure(503);
    const ids = new Set();
    for (const r of q.records) {
      if (!uuid.test(r.id) || !uuid.test(r.generation) || ids.has(r.id) || !states.has(r.state) || typeof r.seat !== 'string' || typeof r.resource !== 'string' || !/^[0-9a-f]{40}$/.test(r.tag) || (r.run_id !== null && !positive(r.run_id)) || (r.attempt !== null && !positive(r.attempt)) || typeof r.dispatched !== 'boolean') throw new Failure(503);
      ids.add(r.id);
    }
    this.project();
  }
  async save() {
    const content = encode(this.queue);
    if (content.length > journalLimit) throw new Failure(503);
    const result = await this.api(indexPath, 'PUT', {branch: 'golem-state', message: 'Update golem queue', content, ...(this.sha ? {sha: this.sha} : {})});
    if (!/^[0-9a-f]{40}$/.test(result.content?.sha)) throw new Failure(503);
    this.sha = result.content.sha;
    this.project();
  }
  validate(request) {
    if (!request || request.version !== 2 || !uuid.test(request.delivery_id) || !/^isp-insoft-gmbh\/[A-Za-z0-9_.-]{1,100}$/.test(request.repository) || !/^[A-Za-z0-9_-]{1,64}$/.test(request.golem) || !/^[A-Za-z0-9-]{1,39}$/.test(request.seat_login) || !['anthropic', 'openai-codex', 'github-copilot'].includes(request.provider) || !/^[0-9a-f]{40}$/.test(request.head_sha) || !positive(request.check_run_id) || (request.issue_number !== null && !positive(request.issue_number)) || (request.comment_id !== null && !positive(request.comment_id))) throw new Failure(400);
  }
  async request(row) {
    const tag = await this.api(`${ops}/git/tags/${row.tag}`);
    if (typeof tag.message !== 'string' || tag.message.length > bodyLimit) throw new Failure(503);
    const request = JSON.parse(tag.message);
    this.validate(request);
    if (request.delivery_id !== row.id || request.queue_generation !== row.generation || `${request.repository.toLowerCase()}/${request.golem}` !== row.resource || `${request.seat_login.toLowerCase()}/${request.provider}` !== row.seat) throw new Failure(503);
    return request;
  }
  async enqueue(request) {
    this.validate(request);
    const existing = this.queue.records.find(r => r.id === request.delivery_id);
    if (existing) return existing;
    const receipt = await this.api(`${ops}/git/ref/tags/golem-terminal/${request.delivery_id}`, 'GET', undefined, true);
    if (receipt) return {id: request.delivery_id, state: 'terminal'};
    // Retire only durably terminal records, after an immutable deduplication receipt exists.
    const retired = this.queue.records.filter(r => r.state === 'terminal').slice(0, 2);
    for (const row of retired) {
      const path = `${ops}/git/ref/tags/golem-terminal/${row.id}`;
      const prior = await this.api(path, 'GET', undefined, true);
      if (!prior) await this.api(`${ops}/git/refs`, 'POST', {ref: `refs/tags/golem-terminal/${row.id}`, sha: row.tag});
      else if (prior.object.sha !== row.tag) throw new Failure(409);
    }
    if (retired.length) {
      this.queue.records = this.queue.records.filter(r => !retired.includes(r));
      await this.save();
    }
    if (this.queue.records.length >= capacity) throw new Failure(429);
    const refPath = `${ops}/git/ref/tags/golem-request/${request.delivery_id}`;
    let ref = await this.api(refPath, 'GET', undefined, true);
    let tag;
    if (ref) {
      if (ref.object.type !== 'tag') throw new Failure(409);
      tag = await this.api(`${ops}/git/tags/${ref.object.sha}`);
      const saved = JSON.parse(tag.message);
      const {queue_generation, ...original} = saved;
      const {queue_generation: ignored, ...incoming} = request;
      // A tag created before an interrupted admission is recoverable, but never replaceable.
      if (JSON.stringify(original) !== JSON.stringify(incoming) || !uuid.test(queue_generation)) throw new Failure(409);
      request = saved;
    } else {
      request = {...request, queue_generation: crypto.randomUUID()};
      const trunk = await this.api(`${ops}/git/ref/heads/trunk`);
      tag = await this.api(`${ops}/git/tags`, 'POST', {tag: `golem-request/${request.delivery_id}`, message: JSON.stringify(request), object: trunk.object.sha, type: 'commit'});
      await this.api(`${ops}/git/refs`, 'POST', {ref: `refs/tags/golem-request/${request.delivery_id}`, sha: tag.sha});
    }
    const row = {id: request.delivery_id, generation: request.queue_generation, seat: `${request.seat_login.toLowerCase()}/${request.provider}`, resource: `${request.repository.toLowerCase()}/${request.golem}`, tag: tag.sha, state: 'pending', dispatched: false, run_id: null, attempt: null};
    this.queue.records.push(row);
    await this.save();
    return row;
  }
  async legacyActive() {
    // Unknown runs include legacy v1 and unbound dispatches. Pagination overflow fails closed.
    for (const status of ['queued', 'in_progress', 'waiting', 'pending', 'requested']) {
      const page = await this.api(`${ops}/actions/workflows/runner.yml/runs?status=${status}&per_page=100`);
      if (!Array.isArray(page.workflow_runs) || page.workflow_runs.length > 100 || !Number.isSafeInteger(page.total_count) || page.total_count < 0 || page.total_count > 100 || page.total_count !== page.workflow_runs.length) return true;
      if (page.workflow_runs.some(run => !this.queue.records.some(r => r.run_id === run.id && r.state !== 'terminal'))) return true;
    }
    return false;
  }
  async advance() {
    if (!this.queue.records.some(r => r.state === 'pending' || r.state === 'reserved')) return;
    if (await this.legacyActive()) return;
    let dispatched = 0;
    for (let i = 0; i < this.queue.records.length && dispatched < 2; i++) {
      const row = this.queue.records[i];
      if (!['pending', 'reserved'].includes(row.state)) continue;
      if (this.queue.records.some((other, j) => other !== row && other.state !== 'terminal' && shares(row, other) && (j < i || other.state !== 'pending'))) continue;
      const request = await this.request(row);
      row.state = 'reserved';
      await this.save(); // Both resources + generation are one guarded GitHub write.
      row.state = 'dispatching';
      row.dispatched = true;
      await this.save(); // A crash after this point must never redispatch.
      try {
        await this.api(`${ops}/actions/workflows/runner.yml/dispatches`, 'POST', {ref: 'trunk', inputs: {repo: request.repository, golem: request.golem, request: JSON.stringify(request)}});
      } catch {
        row.state = 'dispatch_uncertain';
        await this.save();
      }
      dispatched++;
      if (positive(request.comment_id)) {
        try { await this.api(`/repos/${request.repository}/issues/comments/${request.comment_id}/reactions`, 'POST', {content: 'rocket'}); } catch { /* Feedback cannot revoke an execution reservation. */ }
      }
    }
  }
  async run(row, runId) {
    const run = await this.api(`${ops}/actions/runs/${runId}`, 'GET', undefined, true);
    if (!run) return null;
    const request = await this.request(row);
    if (run.id !== runId || run.event !== 'workflow_dispatch' || run.head_branch !== 'trunk' || run.path !== '.github/workflows/runner.yml' || run.display_title !== `${request.repository} ${request.golem}` || !positive(run.run_attempt)) throw new Failure(409);
    return run;
  }
  async callback(body) {
    const row = this.queue.records.find(r => r.id === body.request_id);
    if (!row || row.generation !== body.generation || !positive(body.run_id) || !positive(body.attempt)) throw new Failure(409);
    if (body.action === 'bind') {
      if (!['dispatching', 'dispatch_uncertain', 'bound', 'stop_requested'].includes(row.state) || !row.dispatched || (row.run_id !== null && (row.run_id !== body.run_id || row.attempt !== body.attempt))) throw new Failure(409);
      if (this.queue.records.some(r => r !== row && r.run_id === body.run_id)) throw new Failure(409);
      const run = await this.run(row, body.run_id);
      if (!run || run.status === 'completed' || run.run_attempt !== body.attempt || body.attempt !== 1) throw new Failure(409);
      row.run_id = body.run_id;
      row.attempt = body.attempt;
      const stopped = row.state === 'stop_requested';
      row.state = stopped ? 'stop_requested' : 'bound';
      await this.save();
      if (stopped) {
        try { await this.api(`${ops}/actions/runs/${row.run_id}/cancel`, 'POST'); } catch { /* Retain until terminal. */ }
      }
      return {state: stopped ? 'stop_requested' : 'granted'};
    }
    if (!['bound', 'stop_requested', 'terminal'].includes(row.state) || row.run_id !== body.run_id || row.attempt !== body.attempt) throw new Failure(409);
    if (row.state === 'stop_requested') await this.stoppedCheck(row);
    row.state = 'terminal';
    await this.save();
    await this.advance();
    return {state: 'terminal'};
  }
  async stoppedCheck(row) {
    const request = await this.request(row);
    await this.api(`/repos/${request.repository}/check-runs/${request.check_run_id}`, 'PATCH', {status: 'completed', conclusion: 'cancelled', output: {title: 'stopped', summary: `outcome: stopped\nseat: ${request.seat_login}/${request.provider}\nharness: ${request.harness}\nmodel: ${request.model ?? 'default'}\nbudget: ${request.timeout_seconds}s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${row.run_id ?? ''}`}});
  }
  async stop(body) {
    if (typeof body.repository !== 'string' || !positive(body.issue_number)) throw new Failure(400);
    let selected = null;
    for (let i = this.queue.records.length - 1; i >= 0; i--) {
      const row = this.queue.records[i];
      if (row.state === 'terminal' || !row.resource.startsWith(`${body.repository.toLowerCase()}/`)) continue;
      const request = await this.request(row);
      if (request.trigger !== 'mention' || request.issue_number !== body.issue_number) continue;
      if (row.state === 'pending' || (row.state === 'stop_requested' && !row.dispatched)) {
        selected ??= row;
      } else {
        selected = row;
        break; // Latest active mention wins over any newer pending mentions.
      }
    }
    if (selected) {
      selected.state = 'stop_requested';
      await this.save();
    }
    await this.reconcile();
    return {state: 'stop_requested', count: selected ? 1 : 0};
  }
  async terminalCheck(row, run) {
    const request = await this.request(row);
    const path = `/repos/${request.repository}/check-runs/${request.check_run_id}`;
    const check = await this.api(path);
    const lines = typeof check.output?.summary === 'string' ? check.output.summary.split('\n') : [];
    const outcomes = {changed: 'success', replied: 'success', no_change: 'success', incomplete: 'neutral', blocked: 'failure', stopped: 'cancelled'};
    const outcome = lines.find(line => line.startsWith('outcome: '))?.slice(9);
    const valid = check.id === request.check_run_id && check.name === `golem/${request.golem}` && check.app?.id === 4605060 && check.status === 'completed' && Object.hasOwn(outcomes, outcome) && check.conclusion === outcomes[outcome]
      && lines.includes(`seat: ${request.seat_login}/${request.provider}`) && lines.includes(`harness: ${request.harness}`)
      && lines.some(line => /^model: \S[^\r\n]*$/.test(line)) && lines.includes(`budget: ${request.timeout_seconds}s`)
      && lines.includes(`log: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${row.run_id}`)
      && (outcome !== 'no_change' || lines.some(line => /^reason: \S/.test(line)));
    if (valid) return; // Never replace a completed native audit with an inferred outcome.
    if (row.state === 'stop_requested' || run.conclusion === 'cancelled') {
      await this.stoppedCheck(row);
      return;
    }
    await this.api(path, 'PATCH', {status: 'completed', conclusion: 'failure', output: {title: 'blocked', summary: `outcome: blocked\nseat: ${request.seat_login}/${request.provider}\nharness: ${request.harness}\nmodel: ${request.model ?? 'default'}\nbudget: ${request.timeout_seconds}s\nlog: https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${row.run_id}\nreason: workflow ended (${run.conclusion}) without a valid completed native audit; execution/publication outcome unconfirmed`}});
  }
  async reconcile() {
    const rows = this.queue.records;
    if (!rows.length) return;
    const start = Number.isSafeInteger(this.queue.cursor) ? this.queue.cursor % rows.length : 0;
    let checked = 0;
    for (let offset = 0; offset < rows.length && checked < 4; offset++) {
      const position = (start + offset) % rows.length;
      const row = rows[position];
      this.queue.cursor = (position + 1) % rows.length;
      if (row.state === 'terminal' || (!row.run_id && !(row.state === 'stop_requested' && !row.dispatched))) continue;
      checked++;
      // Commit scan progress before fallible I/O, so one irreparable check cannot starve others.
      await this.save();
      try {
        if (!row.dispatched) {
          await this.stoppedCheck(row);
          row.state = 'terminal';
        } else {
          const run = await this.run(row, row.run_id);
          if (!run || run.run_attempt !== row.attempt || run.status !== 'completed' || !['success', 'failure', 'cancelled', 'timed_out', 'action_required', 'neutral', 'skipped', 'startup_failure', 'stale'].includes(run.conclusion)) {
            if (row.state === 'stop_requested' && run && run.run_attempt === row.attempt) {
              try { await this.api(`${ops}/actions/runs/${row.run_id}/cancel`, 'POST'); } catch { /* Retry reconciliation, never release on cancel acknowledgment. */ }
            }
            continue;
          }
          await this.terminalCheck(row, run);
          row.state = 'terminal';
        }
      } catch { /* Unknown execution or failed check repair retains ownership; scan other resources. */ }
    }
    await this.save();
  }
  async operate(body) {
    if (!body || !['enqueue', 'bind', 'complete', 'stop', 'tick'].includes(body.action)) throw new Failure(400);
    await this.ctx.storage.setAlarm(Date.now() + 60000);
    await this.connect();
    await this.load();
    if (body.action === 'enqueue') {
      const row = await this.enqueue(body.request);
      await this.advance();
      return {state: row.state === 'pending' ? 'pending' : 'dispatched', request_id: row.id};
    }
    if (body.action === 'bind' || body.action === 'complete') return this.callback(body);
    if (body.action === 'stop') return this.stop(body);
    if (body.action === 'tick') { await this.reconcile(); await this.advance(); return {state: 'ok'}; }
    throw new Failure(400);
  }
  async fetch(request) {
    if (request.method !== 'POST') return Response.json({error: 'method'}, {status: 405});
    try {
      const body = JSON.parse(await bounded(request, bodyLimit));
      return Response.json(await this.serial(() => this.operate(body)));
    } catch (error) { return Response.json({error: 'queue operation failed'}, {status: error instanceof Failure ? error.status : 503}); }
  }
  async alarm() {
    try { await this.serial(() => this.operate({action: 'tick'})); }
    finally { await this.ctx.storage.setAlarm(Date.now() + 60000); }
  }
}
