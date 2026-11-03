import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";

// Exercises Nu's real HTTP encoder against a local receiver, never Cloudflare.
const modulePath = fileURLToPath(new URL("./deploy.nu", import.meta.url)).replaceAll("\\", "/");
const boundary = "golem-http-test-123456";
const wasm = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 255, 254, 0, 13, 10]);
let received = 0;
const responses = [200, 403, 200];
const server = createServer(async (request, response) => {
  try {
    assert.equal(request.method, "PUT");
    assert.equal(request.headers.authorization, "Bearer dummy-token");
    assert.equal(request.headers["content-type"], `multipart/form-data; boundary=${boundary}`);
    let size = 0;
    const chunks = [];
    for await (const chunk of request) {
      size += chunk.length;
      assert.ok(size < 65536, "test request must stay bounded");
      chunks.push(chunk);
    }
    const body = Buffer.concat(chunks);
    const marker = Buffer.from(`--${boundary}`);
    assert.ok(body.subarray(0, marker.length).equals(marker));
    assert.ok(body.subarray(-marker.length - 4).equals(Buffer.from(`--${boundary}--\r\n`)));
    const text = body.toString("latin1");
    assert.ok(text.includes('name="metadata"; filename="metadata"\r\nContent-Type: application/json'));
    assert.ok(text.includes('name="index.js"; filename="index.js"\r\nContent-Type: application/javascript+module'));
    const header = Buffer.from('name="index_bg.wasm"; filename="index_bg.wasm"\r\nContent-Type: application/wasm\r\n\r\n');
    const start = body.indexOf(header);
    assert.ok(start >= 0);
    assert.ok(body.subarray(start + header.length, start + header.length + wasm.length).equals(wasm));
    const index = received++;
    response.writeHead(responses[index], { "content-type": "application/json" });
    response.end(JSON.stringify({ success: index === 0, errors: [{ message: "dummy-secret-must-not-leak" }] }));
  } catch (error) {
    response.writeHead(500, { "content-type": "application/json" });
    response.end('{"success":false}');
    server.emit("testFailure", error);
  }
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
let failure;
server.on("testFailure", (error) => { failure = error; });
try {
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const source = `use ${JSON.stringify(modulePath)} [deployment-metadata multipart-body upload-worker];
let metadata = deployment-metadata "0123456789abcdef0123456789abcdef01234567" "dummy-key" "dummy-hook";
let body = multipart-body $metadata ('export default {};' | into binary) 0x[0061736d01000000fffe000d0a] "${boundary}";
upload-worker "http://127.0.0.1:${address.port}" "Bearer dummy-token" $body "${boundary}"`;
  for (let index = 0; index < responses.length; index++) {
    const result = await new Promise((resolve, reject) => {
      const child = spawn("nu", ["--no-config-file", "-c", source], {
        stdio: ["ignore", "pipe", "pipe"],
        timeout: 10000,
      });
      let output = "";
      child.stdout.on("data", (chunk) => { output += chunk; });
      child.stderr.on("data", (chunk) => { output += chunk; });
      child.on("error", reject);
      child.on("close", (code) => resolve({ code, output }));
    });
    if (failure) throw failure;
    assert.equal(result.code === 0, index === 0, result.output);
    assert.ok(!result.output.includes("dummy-secret-must-not-leak"));
    assert.ok(!result.output.includes("dummy-token"));
    if (index !== 0) assert.ok(result.output.includes("Cloudflare upload failed"));
  }
  assert.equal(received, 3);
  console.log("deploy: real multipart HTTP upload, binary MIME/integrity, HTTP/API errors and redaction passed");
} finally {
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
}
