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
const dispatches = [], checks = [], reactions = [], writes = [];
let index = null, initial = null, counter = 1, calls = 0, journalReads = 0;
const sha = () => (counter++).toString(16).padStart(40, "0");
const queue = () => JSON.parse(Buffer.from(index.content, "base64"));
const row = id => queue().records.find(record => record.id === id);
async function github(request) {
  assert.ok(++calls <= 600, "bounded fixture API calls");
  const url = new URL(request.url);
  assert.equal(url.origin, "https://api.github.com", "no external network");
  assert.equal(request.headers.get("X-GitHub-Api-Version"), "2022-11-28");
  const path = url.pathname, method = request.method;
  const body = ["POST", "PUT", "PATCH", "DELETE"].includes(method) ? await request.json() : null;
  if (path === "/app/installations/153964219/access_tokens") return json(201, { token: "synthetic-installation-token" });
  if (path === `${ops}/actions/variables/GOLEM_PROTOCOL_VERSION`) return json(200, { value: "2" });
  if (path === target) return json(200, { id: 99, full_name: repository, default_branch: "trunk", topics: ["isp-golem-xiv-test"] });
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
  if (path === `${ops}/contents/events-v2.json` || path.startsWith(`${ops}/contents/events-v2/`)) {
    const previous = continuations.get(path);
    if (method === "GET") {
      assert.equal(url.searchParams.get("ref"), "golem-state");
      return previous ? json(200, { ...previous, encoding: "base64" }) : json(404);
    }
    assert.equal(body.branch, "golem-state");
    if (body.sha !== previous?.sha) return json(409);
    if (method === "DELETE") { continuations.delete(path); return json(200); }
    assert.equal(method, "PUT");
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
    refs.set(body.ref, body.sha);
    if (body.ref === "refs/heads/golem-state") index = { sha: sha(), content: Buffer.from(initial).toString("base64") };
    return json(201, { object: { sha: body.sha } });
  }
  if (path.startsWith(`${ops}/git/ref/`)) {
    const ref = `refs/${path.slice(`${ops}/git/ref/`.length)}`, value = refs.get(ref);
    return value ? json(200, { object: { sha: value, type: ref.startsWith("refs/tags/") ? "tag" : "commit" } }) : json(404);
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
  console.log("Worker artifact: Rust/Coordinator SQLite admission, signed bind/complete, denial fences, journal restoration and shared-seat release passed (mock GitHub; no native/provider proof)");
} finally {
  await worker.dispose();
}
