// Run with Node: node router/tests/runtime.mjs [absolute miniflare module URL].
// Synthetic RSA keys only; no production credentials or JWTs are printed.
import assert from 'node:assert/strict';
import {generateKeyPairSync, verify} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';

const moduleSpecifier = process.argv[2] ? pathToFileURL(process.argv[2]).href : 'miniflare';
const {Miniflare} = await import(moduleSpecifier);
const runtime = await readFile(new URL('../src/runtime.js', import.meta.url), 'utf8');
const {privateKey, publicKey} = generateKeyPairSync('rsa', {modulusLength: 2048});
const script = `${runtime}\nexport default {async fetch(request) {
  if (new URL(request.url).pathname === '/deadline') {
    const deadline = makeDeadline(5);
    await deadlinePromise(deadline);
    cancelDeadline(deadline);
    return new Response('elapsed');
  }
  if (new URL(request.url).pathname === '/cancel') {
    const deadline = makeDeadline(1);
    cancelDeadline(deadline);
    const canceled = await Promise.race([deadlinePromise(deadline).then(() => false), new Promise(resolve => setTimeout(() => resolve(true), 10))]);
    return new Response(String(canceled));
  }
  try { return new Response(await appJwt(await request.text())); }
  catch { return new Response('signing failed', {status: 400}); }
}};`;
const worker = new Miniflare({workers: [{config: {
  name: 'jwt-fixture',
  compatibilityDate: '2026-10-06',
  manifest: {mainModule: 'fixture.mjs', modules: {'fixture.mjs': {type: 'esm', contents: script}}},
}}]});
try {
  assert.equal(await (await worker.dispatchFetch('https://fixture.invalid/deadline')).text(), 'elapsed');
  assert.equal(await (await worker.dispatchFetch('https://fixture.invalid/cancel')).text(), 'true');
  for (const type of ['pkcs1', 'pkcs8']) {
    const before = Math.floor(Date.now() / 1000);
    const pem = privateKey.export({type, format: 'pem'});
    const response = await worker.dispatchFetch('https://fixture.invalid/', {method: 'POST', body: pem});
    assert.equal(response.status, 200);
    const jwt = await response.text();
    const pieces = jwt.split('.');
    assert.equal(pieces.length, 3);
    const [header, payload, signature] = pieces;
    assert.deepEqual(JSON.parse(Buffer.from(header, 'base64url')), {alg: 'RS256', typ: 'JWT'});
    const claims = JSON.parse(Buffer.from(payload, 'base64url'));
    assert.equal(claims.iss, '4605060');
    const after = Math.floor(Date.now() / 1000);
    assert.ok(claims.iat >= before - 60 && claims.iat <= after - 60);
    assert.ok(claims.exp >= before + 540 && claims.exp <= after + 540);
    assert.equal(claims.exp - claims.iat, 600);
    const signed = Buffer.from(`${header}.${payload}`);
    const signatureBytes = Buffer.from(signature, 'base64url');
    assert.equal(verify('RSA-SHA256', signed, publicKey, signatureBytes), true);
    assert.equal(verify('RSA-SHA256', Buffer.from(`${header}.${payload}x`), publicKey, signatureBytes), false);
  }
  for (const pem of ['not a PEM key', 'x'.repeat(16385)]) {
    const response = await worker.dispatchFetch('https://fixture.invalid/', {method: 'POST', body: pem});
    assert.equal(response.status, 400);
  }
  console.log('Workers runtime: timer expiry/cancellation; PKCS#1 + PKCS#8 verified independently; issuer/time claims + negative controls passed.');
} finally {
  await worker.dispose();
}
