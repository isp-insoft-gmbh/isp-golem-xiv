import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {assess, assertedMise, manifestDigest, KNOWN_STRINGS} from './verify-release.mjs';

const elfHeader = () => {
  const header = Buffer.alloc(64);
  Buffer.from([0x7f, 0x45, 0x4c, 0x46, 2, 1]).copy(header);
  header.writeUInt16LE(62, 18);
  return header;
};
const assertion = mise => `RUN mise --version | grep '^${mise} '`;
const binary = (...strings) => Buffer.concat([elfHeader(), ...strings.map(s => Buffer.from(`\0${s}\0`))]);
const facts = bytes => {
  const sha = createHash('sha256').update(bytes).digest('hex');
  return {bytes, assetSize: bytes.length, githubDigest: `sha256:${sha}`, sidecar: sha,
    manifest: `aaaa  golem-worker.tar.gz\n${sha}  golem-runner-linux-x86_64\n`, mise: '2026.11.14'};
};

// A consistent release passes and records what it checked.
const good = binary(...KNOWN_STRINGS, assertion('2026.11.14'));
const passing = assess(facts(good));
assert.deepEqual(passing.failures, []);
assert.equal(passing.result.embeddedMiseAssertionCount, 1);
assert.equal(passing.result.elf, 'ELF64 little-endian x86_64');

// Each broken fact is named.
const failing = (change, expected) => {
  const {failures} = assess({...facts(good), ...change});
  assert.ok(failures.some(f => f.includes(expected)), `${expected}: ${failures}`);
};
failing({githubDigest: 'sha256:00'}, "GitHub's asset digest");
failing({sidecar: '00'}, 'sidecar');
failing({manifest: '00  golem-runner-linux-x86_64\n'}, 'SHA256SUMS');
failing({assetSize: 1}, 'size');
failing({mise: '2026.11.12'}, 'Mise 2026.11.12');
for (const bytes of [binary(...KNOWN_STRINGS), binary(...KNOWN_STRINGS, assertion('2026.11.14'), assertion('2026.11.14'))]) {
  assert.ok(assess(facts(bytes)).failures.some(f => f.includes('exactly once')));
}
const noElf = Buffer.concat([Buffer.from('#!/bin/sh\n'), good]);
assert.ok(assess(facts(noElf)).failures.some(f => f.includes('ELF64')));
assert.ok(assess(facts(binary(assertion('2026.11.14')))).failures.some(f => f.includes('strings')));

// The expected Mise comes from exactly one assertion in the Dockerfile.
assert.equal(assertedMise("RUN mise --version | grep '^2026.11.14 ' \\\n  && true"), '2026.11.14');
assert.throws(() => assertedMise('FROM debian'), /0 Mise versions/);
assert.throws(() => assertedMise("mise --version | grep '^2026.1.1 '\nmise --version | grep '^2026.1.2 '"), /2 Mise versions/);
assert.throws(() => assertedMise("grep '^2026.1.1 '"), /0 Mise versions/);

assert.equal(manifestDigest('abc  x\ndef  golem-runner-linux-x86_64', 'golem-runner-linux-x86_64'), 'def');
assert.equal(manifestDigest('abc  x', 'golem-runner-linux-x86_64'), undefined);
console.log('verify-release: ok');
