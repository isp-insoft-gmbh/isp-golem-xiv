import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";

// Real Nu HTTP transport, local receiver only. No Cloudflare credentials or calls.
const modulePath = fileURLToPath(new URL("./deploy.nu", import.meta.url)).replaceAll("\\", "/");
const boundary = "golem-http-test-123456";
const wasm = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 255, 254, 0, 13, 10]);
const javascript = 'export default {}; export { Coordinator } from "./coordinator.js";';
const coordinator = 'import { appJwt } from "./runtime.js"; export class Coordinator {}';
const runtime = 'export function appJwt() {}';
const modules = [
  ["index.js", "application/javascript+module", Buffer.from(javascript)],
  ["index_bg.wasm", "application/wasm", wasm],
  ["coordinator.js", "application/javascript+module", Buffer.from(coordinator)],
  ["runtime.js", "application/javascript+module", Buffer.from(runtime)],
];
let received = 0;
let migrationLookups = 0;
let schedules = 0;
let redeployments = 0;
const responses = [200, 403, 200];
const server = createServer(async (request, response) => {
  try {
    assert.equal(request.headers.authorization, "Bearer dummy-token");
    let size = 0;
    const chunks = [];
    for await (const chunk of request) {
      size += chunk.length;
      assert.ok(size < 65536, "test request must stay bounded");
      chunks.push(chunk);
    }
    const body = Buffer.concat(chunks);
    response.setHeader("content-type", "application/json");
    if (request.url.startsWith("/migration/")) {
      assert.equal(request.method, "GET");
      migrationLookups++;
      const state = request.url.split("/").at(-1);
      if (state === "denied") response.statusCode = 403;
      const script = { id: "isp-golem-xiv" };
      if (state === "applied" || (state === "stateful" && redeployments > 0)) script.migration_tag = "coordinator-sqlite-v1";
      if (state === "unknown") script.migration_tag = "unrecognized";
      if (state === "invalid-tag") script.migration_tag = 42;
      response.end(JSON.stringify({ success: state !== "denied", result: state === "malformed" ? {} : { default_environment: { script } }, errors: [{message: "dummy-secret-must-not-leak"}] }));
      return;
    }
    if (request.url.startsWith("/schedules/")) {
      assert.equal(request.method, "PUT");
      assert.match(request.headers["content-type"], /application\/json/);
      assert.deepEqual(JSON.parse(body), [{ cron: "* * * * *" }]);
      schedules++;
      const success = request.url.endsWith("/ok");
      response.statusCode = success ? 200 : 403;
      response.end(JSON.stringify({success, errors: [{message: "dummy-secret-must-not-leak"}]}));
      return;
    }
    assert.ok(["/upload", "/redeploy"].includes(request.url));
    assert.equal(request.method, "PUT");
    assert.equal(request.headers["content-type"], `multipart/form-data; boundary=${boundary}`);
    const marker = `--${boundary}`;
    // latin1 roundtrips arbitrary binary bytes exactly, unlike UTF-8 decoding.
    const parts = body.toString("latin1").split(marker);
    assert.equal(parts.length, 7);
    assert.equal(parts[0], "");
    assert.equal(parts[6], "--\r\n");
    for (const [index, [name, mime, bytes]] of modules.entries()) {
      assert.equal(parts[index + 2], `\r\nContent-Disposition: form-data; name="${name}"; filename="${name}"\r\nContent-Type: ${mime}\r\n\r\n${bytes.toString("latin1")}\r\n`);
    }
    const [header, encoded] = parts[1].split("\r\n\r\n");
    assert.equal(header, '\r\nContent-Disposition: form-data; name="metadata"; filename="metadata"\r\nContent-Type: application/json');
    const metadata = JSON.parse(encoded);
    assert.equal(metadata.main_module, "index.js");
    if (request.url === "/redeploy" && redeployments > 0) assert.ok(!("migrations" in metadata), "redeployment must not recreate SQLite namespace");
    else assert.deepEqual(metadata.migrations, {new_tag: "coordinator-sqlite-v1", steps: [{new_sqlite_classes: ["Coordinator"]}]});
    assert.deepEqual(metadata.bindings.find(binding => binding.name === "COORDINATOR"), {name: "COORDINATOR", type: "durable_object_namespace", class_name: "Coordinator"});
    assert.deepEqual(metadata.bindings.find(binding => binding.name === "APP_CLIENT_SECRET"), {name: "APP_CLIENT_SECRET", type: "secret_text", text: "dummy-client"});
    const index = request.url === "/redeploy" ? (redeployments++, 0) : received++;
    response.statusCode = responses[index];
    response.end(JSON.stringify({ success: index === 0, errors: [{ message: "dummy-secret-must-not-leak" }] }));
  } catch (error) {
    response.statusCode = 500;
    response.end('{"success":false}');
    server.emit("testFailure", error);
  }
});
server.requestTimeout = 10000;
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
let failure;
server.on("testFailure", (error) => { failure = error; });
try {
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const base = `http://127.0.0.1:${address.port}`;
  const prelude = `use ${JSON.stringify(modulePath)} [deployment-metadata multipart-body upload-worker current-migration configure-schedules];\n`;
  async function execute(source, success, diagnostic) {
    const result = await new Promise((resolve, reject) => {
      const child = spawn("nu", ["--no-config-file", "-c", prelude + source], {
        stdio: ["ignore", "pipe", "pipe"], timeout: 10000,
      });
      let output = "";
      const append = chunk => { output += chunk; if (output.length > 65536) child.kill(); };
      child.stdout.on("data", append);
      child.stderr.on("data", append);
      child.on("error", reject);
      child.on("close", (code) => resolve({ code, output }));
    });
    if (failure) throw failure;
    assert.equal(result.code === 0, success, result.output);
    for (const secret of ["dummy-secret-must-not-leak", "dummy-token", "dummy-key", "dummy-hook", "dummy-client"]) assert.ok(!result.output.includes(secret));
    if (diagnostic) assert.ok(result.output.includes(diagnostic), result.output);
  }
  const metadata = 'deployment-metadata "0123456789abcdef0123456789abcdef01234567" "dummy-key" "dummy-hook" "dummy-client"';
  const upload = `let metadata = ${metadata};
let body = multipart-body $metadata (${JSON.stringify(javascript)} | into binary) 0x[0061736d01000000fffe000d0a] (${JSON.stringify(coordinator)} | into binary) (${JSON.stringify(runtime)} | into binary) "${boundary}";
upload-worker "${base}/upload" "Bearer dummy-token" $body "${boundary}"`;
  for (let index = 0; index < responses.length; index++) await execute(upload, index === 0, index === 0 ? undefined : "Cloudflare upload failed");
  for (const state of ["fresh", "applied", "unknown", "malformed", "denied", "invalid-tag"]) {
    await execute(`let tag = current-migration "${base}/migration/${state}" "Bearer dummy-token"; let metadata = ${metadata} $tag; if $tag == "coordinator-sqlite-v1" and ("migrations" in $metadata) { error make {msg: "migration replay"} }`, state === "fresh" || state === "applied");
  }
  const redeploy = upload.replace(`let metadata = ${metadata};`, `let tag = current-migration "${base}/migration/stateful" "Bearer dummy-token"; let metadata = ${metadata} $tag;`).replace(`${base}/upload`, `${base}/redeploy`);
  await execute(redeploy, true);
  await execute(redeploy, true);
  assert.equal(redeployments, 2);
  await execute(redeploy.replace("/migration/stateful", "/migration/unknown"), false);
  assert.equal(redeployments, 2, "unknown migration must fail before uploading");
  for (const state of ["ok", "denied"]) await execute(`configure-schedules "${base}/schedules/${state}" "Bearer dummy-token"`, state === "ok", state === "ok" ? undefined : "Cloudflare schedules failed");
  assert.equal(received, 3);
  assert.equal(migrationLookups, 9);
  assert.equal(schedules, 2);
  console.log("deploy: four binary-exact modules; first/redeploy/unknown migrations; minute cron; HTTP/API failures and redaction passed");
} finally {
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
}
