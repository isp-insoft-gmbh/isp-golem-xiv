// Delivered Rust Wasm + actual Coordinator/SQLite; only GitHub is doubled.
// Transport/protocol proof, not native processor, provider or live GitHub proof.
import assert from "node:assert/strict";
import { createHmac, generateKeyPairSync, randomUUID } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { pathToFileURL } from "node:url";

const { Miniflare, Response: RuntimeResponse } = await import(pathToFileURL(process.argv[2]).href);
const Answer = RuntimeResponse ?? Response;
const artifact = resolve(process.argv[3]);
const modules = {};
for (const name of ["index.js", "index_bg.wasm", "coordinator.js", "runtime.js"]) {
  const contents = await readFile(resolve(artifact, name));
  modules[name] = name.endsWith(".wasm") ? { type: "wasm", contents: new Uint8Array(contents) } : { type: "esm", contents: contents.toString("utf8") };
}
const key = generateKeyPairSync("rsa", { modulusLength: 2048 }).privateKey.export({ type: "pkcs8", format: "pem" });
const secret = "artifact-fixture-webhook-secret";
const repository = "isp-insoft-gmbh/isp-golem-xiv-test-artifact";
const ops = "/repos/isp-insoft-gmbh/isp-golem-xiv-ops";
const target = `/repos/${repository}`;
const head = "a".repeat(40);
const files = new Map(["hello", "second"].map(name => [`${name}.md`, "---\n---\nReply without changing files."]));
const json = (status, value = {}) => new Answer(status === 204 ? null : JSON.stringify(value), { status, headers: { "Content-Type": "application/json" } });
// GitHub Contents CAS + immutable refs/tags, translated from coordinator.mjs.
const refs = new Map(), tags = new Map(), runs = new Map(), continuations = new Map();
const dispatches = [], checks = [], reactions = [], writes = [], refEvents = [];
let index = null, initial = null, counter = 1, calls = 0, journalReads = 0;
let failEventIndexWrites = 0, interruption = null;
const sha = () => (counter++).toString(16).padStart(40, "0");
const queue = () => JSON.parse(Buffer.from(index.content, "base64"));
const row = id => queue().records.find(record => record.id === id);
async function github(request) {
  const response = await githubFixture(request);
  const path = new URL(request.url).pathname;
  if (interruption?.method === request.method && interruption.path === path) {
    interruption = null;
    return json(503); // Applied remotely, response lost at this retirement boundary.
  }
  return response;
}
async function githubFixture(request) {
  assert.ok(++calls <= 600, "bounded fixture API calls");
  const url = new URL(request.url);
  assert.equal(url.origin, "https://api.github.com", "no external network");
  assert.equal(request.headers.get("X-GitHub-Api-Version"), "2022-11-28");
  const path = url.pathname, method = request.method;
  const body = (["POST", "PUT", "PATCH"].includes(method) || (method === "DELETE" && path.includes("/contents/"))) ? await request.json() : null;
  if (path.startsWith(`${ops}/contents/`) && ["PUT", "DELETE"].includes(method) && body?.branch === "golem-state" && !refs.has("refs/heads/golem-state")) return json(404, { message: "Branch not found" });
  if (path === "/app/installations/153964219/access_tokens") return json(201, { token: "synthetic-installation-token" });
  if (path === `${ops}/actions/variables/GOLEM_PROTOCOL_VERSION`) return json(200, { value: "2" });
  if (path === target && method === "GET") return json(200, { id: 99, full_name: repository, default_branch: "trunk", topics: ["isp-golem-xiv-test"] });
  if (path === `${target}/commits/trunk`) return json(200, { sha: head });
  if (path === `${target}/contents/.golems`) return json(200, [...files].map(([name, content]) => ({ name, type: "file", size: Buffer.byteLength(content) })));
  if (path.startsWith(`${target}/contents/.golems/`)) {
    const content = files.get(path.split("/").at(-1));
    return content === undefined ? json(404) : json(200, { type: "file", encoding: "base64", content: Buffer.from(content).toString("base64") });
  }
  if (path === "/orgs/isp-insoft-gmbh/memberships/member") return json(200, { state: "active" });
  if (path === `${ops}/actions/secrets/SEAT_MEMBER__ANTHROPIC`) return json(200);
  if (path === `${target}/check-runs` && method === "POST") { checks.push(body); return json(201, { id: checks.length }); }
  if (path.endsWith("/reactions") && method === "POST") { reactions.push(body); return json(201, { id: reactions.length }); }
  if (path === `${ops}/contents/scheduler-v2.json`) return json(500);
  if (path === `${ops}/contents/events-v2.json` || path.startsWith(`${ops}/contents/events-v2/`)) {
    const previous = continuations.get(path);
    if (method === "GET") {
      assert.equal(url.searchParams.get("ref"), "golem-state");
      return previous ? json(200, { ...previous, encoding: "base64" }) : json(404);
    }
    assert.equal(body.branch, "golem-state");
    if (path === `${ops}/contents/events-v2.json` && method === "PUT" && failEventIndexWrites > 0) { failEventIndexWrites--; return json(503); }
    if (body.sha !== previous?.sha) return json(409);
    if (method === "DELETE") { refEvents.push({ method, path }); continuations.delete(path); return json(200); }
    assert.equal(method, "PUT");
    if (path === `${ops}/contents/events-v2.json`) refEvents.push({ method, path, ids: JSON.parse(Buffer.from(body.content, "base64")).ids });
    const next = { sha: sha(), content: body.content };
    continuations.set(path, next);
    return json(previous ? 200 : 201, { content: { sha: next.sha } });
  }
  if (path === `${ops}/contents/queue.json`) {
    assert.equal(url.searchParams.get("ref"), method === "GET" ? "golem-state" : null);
    if (method === "GET") { journalReads++; return index ? json(200, { ...index, encoding: "base64" }) : json(404); }
    assert.equal(method, "PUT");
    assert.equal(body.branch, "golem-state");
    if (body.sha !== index?.sha) return json(409);
    index = { sha: sha(), content: body.content };
    writes.push(structuredClone(queue().records));
    return json(200, { content: { sha: index.sha } });
  }
  if (path === `${ops}/git/ref/heads/trunk`) return json(200, { object: { sha: head, type: "commit" } });
  if (path === `${ops}/git/commits/${head}`) return json(200, { tree: { sha: head } });
  if (path === `${ops}/git/trees`) { initial = body.tree[0].content; return json(201, { sha: sha() }); }
  if (path === `${ops}/git/commits`) return json(201, { sha: sha() });
  if (path === `${ops}/git/refs` && method === "POST") {
    if (refs.has(body.ref)) return json(422);
    if (body.ref.startsWith("refs/tags/golem-event-done/")) {
      const id = body.ref.slice("refs/tags/golem-event-done/".length);
      assert.equal(body.sha, refs.get(`refs/tags/golem-event/${id}`), "receipt points to same annotated source");
    }
    refs.set(body.ref, body.sha);
    refEvents.push({ method, ref: body.ref, sha: body.sha });
    if (body.ref === "refs/heads/golem-state") index = { sha: sha(), content: Buffer.from(initial).toString("base64") };
    return json(201, { object: { sha: body.sha } });
  }
  if (path === `${ops}/git/matching-refs/tags/golem-event/` && method === "GET") {
    const pending = [...refs].filter(([ref]) => ref.startsWith("refs/tags/golem-event/"));
    assert.ok(pending.length <= 512, "fixture pending source bound");
    return json(200, pending.map(([ref, value]) => ({ ref, object: { sha: value, type: "tag" } })));
  }
  if (path.startsWith(`${ops}/git/refs/tags/`) && method === "DELETE") {
    const ref = `refs/${path.slice(`${ops}/git/refs/`.length)}`;
    refEvents.push({ method, ref, sha: refs.get(ref) });
    return refs.delete(ref) ? json(204) : json(404);
  }
  if (path.startsWith(`${ops}/git/ref/`)) {
    const ref = `refs/${path.slice(`${ops}/git/ref/`.length)}`, value = refs.get(ref);
    refEvents.push({ method, ref, sha: value });
    return value ? json(200, { ref, object: { sha: value, type: ref.startsWith("refs/tags/") ? "tag" : "commit" } }) : json(404);
  }
  if (path === `${ops}/git/tags` && method === "POST") {
    const value = sha(); tags.set(value, { ...body, sha: value }); return json(201, { sha: value });
  }
  if (path.startsWith(`${ops}/git/tags/`)) { assert.ok(tags.has(path.split("/").at(-1))); return json(200, tags.get(path.split("/").at(-1))); }
  if (path === `${ops}/actions/workflows/runner.yml/runs`) {
    const matching = [...runs.values()].filter(run => run.status === url.searchParams.get("status"));
    return json(200, { total_count: matching.length, workflow_runs: matching });
  }
  if (path === `${ops}/actions/workflows/runner.yml/dispatches`) {
    assert.equal(method, "POST");
    const sent = JSON.parse(body.inputs.request), durable = row(sent.delivery_id);
    assert.equal(body.ref, "trunk");
    assert.equal(body.inputs.repo, sent.repository);
    assert.equal(sent.repository_id, 99, "dispatch targets the fixture repository's immutable ID");
    assert.equal(body.inputs.golem, sent.golem);
    assert.equal(durable.state, "dispatching", "dispatch intent durable before dispatch");
    assert.equal(durable.dispatched, true);
    assert.equal(sent.queue_generation, durable.generation);
    dispatches.push(sent);
    return json(204);
  }
  const run = path.match(/\/actions\/runs\/(\d+)$/);
  if (run) return runs.has(Number(run[1])) ? json(200, runs.get(Number(run[1]))) : json(404);
  assert.fail(`Unexpected fixture API: ${method} ${path}`);
}
const config = {
  unsafeTriggerHandlers: true,
  workers: [{
    config: {
      name: "coordinator-artifact",
      compatibilityDate: "2026-11-01",
      manifest: { mainModule: "index.js", modules },
      exports: { Coordinator: { type: "durable-object", storage: "sqlite" } },
      env: {
        VERSION: { type: "text", value: "fixture-version" },
        APP_PRIVATE_KEY: { type: "text", value: key },
        WEBHOOK_SECRET: { type: "text", value: secret },
        COORDINATOR: { type: "durable-object", worker: "coordinator-artifact", exportName: "Coordinator" },
      },
    },
    dev: { outboundService: { type: "fetcher", handler: github } },
  }],
};
let worker = new Miniflare(config);
async function eventually(condition, description) {
  for (let attempt = 0; attempt < 200; attempt++) { if (condition()) return; await sleep(25); }
  assert.fail(`Timed out: ${description}`);
}
async function scheduled() {
  const response = await worker.dispatchFetch("https://fixture.invalid/cdn-cgi/local/scheduled?cron=*+*+*+*+*");
  assert.equal(response.status, 200, await response.text());
}
async function mention(id, golem, commentId) {
  const body = JSON.stringify({ action: "created", sender: { id: 42, login: "member", type: "User" }, repository: { id: 99, full_name: repository }, installation: { id: 153964219 }, issue: { number: 1 }, comment: { id: commentId, body: `@isp-golem-xiv ${golem} say fixture`, user: { id: 42, login: "member", type: "User" } } });
  const signature = createHmac("sha256", secret).update(body).digest("hex");
  const response = await worker.dispatchFetch("https://fixture.invalid/webhook", { method: "POST", body, headers: { "Content-Type": "application/json", "X-GitHub-Event": "issue_comment", "X-GitHub-Delivery": id, "X-Hub-Signature-256": `sha256=${signature}` } });
  assert.equal(response.status, 202);
}
// Matches runner/src/github.rs: JSON numeric identity/timestamp, HMAC over exact
// bytes with trimmed LF-normalized App PEM; no internal DO bypass for callbacks.
async function callback(action, sent, runId, fields = {}, status = 200, signingKey = key) {
  const body = JSON.stringify({ action, request_id: sent.delivery_id, generation: sent.queue_generation, run_id: runId, attempt: 1, timestamp_ms: Date.now(), ...fields });
  const signature = createHmac("sha256", signingKey.replaceAll("\r\n", "\n").trim()).update(body).digest("hex");
  const response = await worker.dispatchFetch("https://fixture.invalid/internal/queue", { method: "POST", body, headers: { "Content-Type": "application/json", "X-Golem-Signature-256": `sha256=${signature}` } });
  const text = await response.text();
  assert.equal(response.status, status, text);
  return status === 200 ? JSON.parse(text) : text;
}
try {
  const response = await worker.dispatchFetch("https://fixture.invalid/health");
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { app: "isp-golem-xiv", version: "fixture-version" });
  // Preserve constructor/relative module/SQLite and reload protection.
  for (let attempt = 0; attempt < 2; attempt++) {
    const namespace = await worker.getDurableObjectNamespace("COORDINATOR", "coordinator-artifact");
    const stub = namespace.get(namespace.idFromName("infra-fixture"));
    const rejected = await stub.fetch("https://coordinator.invalid/");
    assert.equal(rejected.status, 405);
    assert.deepEqual(await rejected.json(), { error: "method" });
    if (attempt === 0) await worker.setOptions(config);
  }
  assert.equal(calls, 0, "health and GET rejection perform no GitHub I/O");
  const firstId = randomUUID(), secondId = randomUUID();
  await mention(firstId, "hello", 100);
  await eventually(() => dispatches.length === 1 && reactions.some(r => r.content === "rocket"), "real admission and dispatch");
  await eventually(() => refs.has(`refs/tags/golem-event-done/${firstId}`) && !refs.has(`refs/tags/golem-event/${firstId}`), "cold mention source retired with receipt");
  const firstReceipt = refs.get(`refs/tags/golem-event-done/${firstId}`);
  const firstSource = tags.get(firstReceipt);
  const receiptCreated = refEvents.findIndex(e => e.method === "POST" && e.ref === `refs/tags/golem-event-done/${firstId}`);
  const receiptVerified = refEvents.findIndex((e, i) => i > receiptCreated && e.method === "GET" && e.ref === `refs/tags/golem-event-done/${firstId}` && e.sha === firstReceipt);
  const sourceDeleted = refEvents.findIndex(e => e.method === "DELETE" && e.ref === `refs/tags/golem-event/${firstId}`);
  const indexRemoved = refEvents.findIndex((e, i) => i > receiptVerified && e.path === `${ops}/contents/events-v2.json` && e.ids && !e.ids.includes(firstId));
  const cursorDeleted = refEvents.findIndex(e => e.method === "DELETE" && e.path === `${ops}/contents/events-v2/${firstId}.json`);
  assert.ok(receiptCreated >= 0 && receiptVerified > receiptCreated && indexRemoved > receiptVerified && cursorDeleted > receiptVerified && sourceDeleted > receiptVerified, "verified receipt precedes index, cursor and pending ref deletion");
  const branchLookup = refEvents.findIndex(e => e.method === "GET" && e.ref === "refs/heads/golem-state");
  const deliveryClaim = refEvents.findIndex(e => e.method === "POST" && e.ref === `refs/tags/golem-delivery/${firstId}`);
  assert.ok(branchLookup >= 0 && deliveryClaim > branchLookup, "branch checked before delivery claim");
  const first = dispatches[0];
  assert.equal(first.version, 2);
  assert.equal(first.delivery_id, firstId);
  assert.equal(first.repository, repository);
  assert.equal(first.repository_id, 99);
  assert.equal(first.golem, "hello");
  assert.equal(first.seat_login, "member");
  assert.equal(first.provider, "anthropic");
  assert.equal(first.check_run_id, 1);
  assert.equal(first.head_sha, head);
  assert.equal(first.instructions, "say fixture");
  assert.match(first.queue_generation, /^[0-9a-f-]{36}$/);
  assert.equal(checks[0].external_id, firstId);
  assert.equal(checks[0].head_sha, head);
  assert.ok(writes.some(records => records.some(r => r.id === firstId && r.state === "reserved")));
  runs.set(1001, { id: 1001, run_attempt: 1, status: "in_progress", event: "workflow_dispatch", head_branch: "trunk", path: ".github/workflows/runner.yml", display_title: `${repository} hello` });
  const beforeDenied = index.content;
  await callback("bind", first, 1001, {}, 401, "wrong-key");
  await callback("bind", first, 1001, { timestamp_ms: Date.now() - 120000 }, 401);
  await callback("bind", first, 1001, { generation: randomUUID() }, 409);
  assert.equal(index.content, beforeDenied, "denied bind cannot mutate authoritative journal");
  assert.deepEqual(await callback("bind", first, 1001), { state: "granted" });
  assert.equal(row(firstId).state, "bound");
  assert.equal(row(firstId).run_id, 1001);
  assert.equal(row(firstId).attempt, 1);
  await mention(secondId, "second", 101);
  await eventually(() => index && row(secondId)?.state === "pending", "second shared-seat request retained");
  assert.equal(row(secondId).seat, row(firstId).seat);
  assert.notEqual(row(secondId).resource, row(firstId).resource, "shared seat alone prevents overlap");
  assert.equal(dispatches.length, 1);
  // Dispose all local SQLite state. The replacement class must read GitHub,
  // recover ownership and pending work, and fence callbacks before releasing.
  await worker.dispose();
  worker = new Miniflare(config);
  const readsBefore = journalReads;
  assert.deepEqual(await callback("bind", first, 1001), { state: "granted" });
  assert.ok(journalReads > readsBefore, "restored class reads authoritative GitHub journal");
  assert.equal(dispatches.length, 1);
  const boundJournal = index.content;
  for (const fields of [{}, { containers_destroyed: true }, { publication_complete: true }, { containers_destroyed: false, publication_complete: true }, { containers_destroyed: true, publication_complete: false }]) {
    await callback("complete", first, 1001, fields, 401);
    assert.equal(index.content, boundJournal, "missing/false attestation cannot release resources");
  }
  const attestations = { containers_destroyed: true, publication_complete: true };
  await callback("complete", first, 1001, { ...attestations, attempt: 2 }, 409);
  await callback("complete", first, 1001, { ...attestations, generation: randomUUID() }, 409);
  await callback("complete", first, 1002, attestations, 409);
  assert.equal(row(firstId).state, "bound");
  assert.equal(row(secondId).state, "pending");
  assert.equal(dispatches.length, 1);
  // Match coordinator.mjs's finished-run double. An in-progress Actions run
  // remains a legacy-active fence even after protocol completion.
  Object.assign(runs.get(1001), { status: "completed", conclusion: "success" });
  assert.deepEqual(await callback("complete", first, 1001, attestations), { state: "terminal" });
  assert.equal(row(firstId).state, "terminal");
  assert.equal(dispatches.length, 2);
  assert.equal(dispatches[1].delivery_id, secondId);
  assert.equal(row(secondId).state, "dispatching");
  assert.equal(dispatches[1].queue_generation, row(secondId).generation);
  runs.set(1002, { ...runs.get(1001), id: 1002, status: "in_progress", conclusion: null, display_title: `${repository} second` });
  assert.deepEqual(await callback("bind", dispatches[1], 1002), { state: "granted" });
  assert.equal(row(secondId).state, "bound");
  await callback("complete", first, 1001, { ...attestations, generation: randomUUID() }, 409);
  assert.equal(row(secondId).state, "bound", "stale prior callback cannot release next owner");
  assert.equal(dispatches.length, 2);
  Object.assign(runs.get(1002), { status: "completed", conclusion: "success" });
  assert.deepEqual(await callback("complete", dispatches[1], 1002, attestations), { state: "terminal" });

  // Source publication succeeds but all index CAS attempts fail: restart must
  // discover the source and reuse its prepared check/request, not create another.
  const orphanId = randomUUID();
  failEventIndexWrites = 3;
  await mention(orphanId, "hello", 102);
  await eventually(() => failEventIndexWrites === 0 && refs.has(`refs/tags/golem-event/${orphanId}`), "source published before failed index insertion");
  const orphanSha = refs.get(`refs/tags/golem-event/${orphanId}`);
  const prepared = JSON.parse(tags.get(orphanSha).message).prepared;
  assert.equal(prepared.delivery_id, orphanId);
  assert.equal(prepared.check_run_id, 3);
  assert.equal(checks.length, 3);
  assert.equal(dispatches.length, 2);
  await worker.dispose();
  worker = new Miniflare(config);
  for (let i = 0; i < 8 && dispatches.length === 2; i++) await scheduled();
  assert.equal(dispatches.length, 3, "scheduled restart admits orphan exactly once");
  const { queue_generation: generation, ...actualPrepared } = dispatches[2];
  assert.match(generation, /^[0-9a-f-]{36}$/);
  assert.deepEqual(actualPrepared, prepared, "admission reuses exact prepared request/check");
  assert.equal(checks.length, 3, "orphan recovery does not create another check");
  for (let i = 0; i < 8 && refs.has(`refs/tags/golem-event/${orphanId}`); i++) await scheduled();
  assert.equal(refs.get(`refs/tags/golem-event-done/${orphanId}`), orphanSha);
  assert.equal(refs.has(`refs/tags/golem-event/${orphanId}`), false);

  // Legacy completed source has no receipt but its request is already in the
  // real coordinator journal. Re-admission must not dispatch it again.
  refs.delete(`refs/tags/golem-event-done/${firstId}`);
  refs.set(`refs/tags/golem-event/${firstId}`, firstReceipt);
  assert.equal(JSON.parse(firstSource.message).prepared.delivery_id, firstId);
  const beforeLegacy = dispatches.length;
  for (let i = 0; i < 8 && refs.has(`refs/tags/golem-event/${firstId}`); i++) await scheduled();
  assert.equal(dispatches.length, beforeLegacy, "legacy completed source does not redispatch");
  assert.equal(checks.length, 3, "legacy recovery does not create another check");
  assert.equal(refs.get(`refs/tags/golem-event-done/${firstId}`), firstReceipt);
  assert.equal(refs.has(`refs/tags/golem-event/${firstId}`), false);
  // Crash after each remotely applied retirement operation. Source/cursor may
  // be missing on restart; verified completion must prevent any re-admission.
  const eventIndexPath = `${ops}/contents/events-v2.json`;
  const eventPath = `${ops}/contents/events-v2/${firstId}.json`;
  const donePath = `${ops}/git/ref/tags/golem-event-done/${firstId}`;
  for (const boundary of [
    { method: "POST", path: `${ops}/git/refs` },
    { method: "GET", path: donePath },
    { method: "PUT", path: eventIndexPath },
    { method: "DELETE", path: eventPath },
    { method: "DELETE", path: `${ops}/git/refs/tags/golem-event/${firstId}` },
  ]) {
    refs.set(`refs/tags/golem-event/${firstId}`, firstReceipt);
    refs.delete(`refs/tags/golem-event-done/${firstId}`);
    const completed = JSON.parse(firstSource.message);
    completed.next = completed.golems.length;
    completed.prepared = null;
    continuations.set(eventIndexPath, { sha: sha(), content: Buffer.from(JSON.stringify({ version: 2, ids: [firstId] })).toString("base64") });
    continuations.set(eventPath, { sha: sha(), content: Buffer.from(JSON.stringify(completed)).toString("base64") });
    interruption = boundary;
    await scheduled();
    assert.equal(interruption, null, `reached retirement boundary ${boundary.method} ${boundary.path}`);
    await worker.dispose();
    worker = new Miniflare(config);
    for (let i = 0; i < 4 && (refs.has(`refs/tags/golem-event/${firstId}`) || continuations.has(eventPath)); i++) await scheduled();
    assert.equal(refs.get(`refs/tags/golem-event-done/${firstId}`), firstReceipt);
    assert.equal(refs.has(`refs/tags/golem-event/${firstId}`), false);
    assert.equal(continuations.has(eventPath), false);
    assert.equal(JSON.parse(Buffer.from(continuations.get(eventIndexPath).content, "base64")).ids.includes(firstId), false);
    assert.equal(dispatches.length, beforeLegacy, "retirement restart never redispatches");
    assert.equal(checks.length, 3, "retirement restart never creates checks");
  }
  // Event-index fairness is distinct from Coordinator resource FIFO.
  // An inadmissible head remains durable while its healthy peer advances.
  const blockedId = randomUUID(), healthyId = randomUUID();
  for (const [id, blocked] of [[blockedId, true], [healthyId, false]]) {
    const work = JSON.parse(firstSource.message);
    work.delivery_id = id;
    work.prepared.delivery_id = id;
    if (blocked) work.prepared.provider = "invalid-provider";
    const sourceSha = sha();
    tags.set(sourceSha, { ...firstSource, tag: `golem-event/${id}`, sha: sourceSha, message: JSON.stringify(work) });
    refs.set(`refs/tags/golem-event/${id}`, sourceSha);
    continuations.set(`${ops}/contents/events-v2/${id}.json`, { sha: sha(), content: Buffer.from(JSON.stringify(work)).toString("base64") });
  }
  continuations.set(eventIndexPath, { sha: sha(), content: Buffer.from(JSON.stringify({ version: 2, ids: [blockedId, healthyId] })).toString("base64") });
  await scheduled();
  assert.equal(row(blockedId), undefined, "head admission consistently fails");
  assert.ok(refs.has(`refs/tags/golem-event/${blockedId}`));
  await worker.dispose();
  worker = new Miniflare(config);
  await scheduled();
  assert.equal(queue().records.filter(record => record.id === healthyId).length, 1, "healthy indexed peer admitted across restart");
  assert.equal(row(healthyId).state, "pending", "existing seat owner still fences dispatch");
  for (let i = 0; i < 3; i++) await scheduled();
  assert.equal(queue().records.filter(record => record.id === healthyId).length, 1);
  assert.equal(row(blockedId), undefined);
  assert.ok(refs.has(`refs/tags/golem-event/${blockedId}`), "blocked immutable source retained");
  assert.ok(continuations.has(`${ops}/contents/events-v2/${blockedId}.json`), "blocked exact prepared cursor retained");
  assert.equal(refs.has(`refs/tags/golem-event-done/${blockedId}`), false);
  assert.equal(refs.has(`refs/tags/golem-event/${healthyId}`), false);
  assert.equal(dispatches.length, beforeLegacy);
  assert.equal(checks.length, 3);
  console.log("Worker artifact: cold mention, orphan restart, legacy dedup, retirement boundaries, event-index restart fairness, signed callbacks and shared-seat release passed (mock GitHub; no live proof)");
} finally {
  await worker.dispose();
}
