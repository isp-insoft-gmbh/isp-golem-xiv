// Checks a published runner release against the source it claims, then records the result under evidence/.
// Usage: node tools/verify-release.mjs <tag> <full source sha> [--out <path>]
// Point in time only: it associates downloaded bytes with a tag, a source and a release run; it is no
// build attestation, and release hosting stays mutable.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const REPOSITORY = 'isp-insoft-gmbh/isp-golem-xiv';
const ASSET = 'golem-runner-linux-x86_64';
const ASSET_BYTES_MAX = 64 << 20;
// Strings only this runner carries; a smoke check that the asset is ours, not proof of behavior.
export const KNOWN_STRINGS = ['golem image: loaded from cache', 'golem lifecycle: reservation_released',
  'agent reported task blocked', 'container failed after a process in it was killed at its memory limit'];

/** The Mise version the agent image asserts, from the Dockerfile's `grep '^<version> '` line. */
export function assertedMise(dockerfile) {
  const versions = [...dockerfile.matchAll(/mise --version \| grep '\^(\d{4}\.\d{1,2}\.\d{1,3}) '/g)].map(m => m[1]);
  if (versions.length !== 1) throw new Error(`agent.Dockerfile asserts ${versions.length} Mise versions, expected 1`);
  return versions[0];
}

/** The digest `SHA256SUMS` lists for a file name, or undefined. */
export function manifestDigest(manifest, name) {
  return manifest.split('\n').map(line => line.trim().split(/\s+/)).find(parts => parts[1] === name)?.[0];
}

function occurrences(bytes, needle) {
  let count = 0;
  for (let at = bytes.indexOf(needle); at !== -1; at = bytes.indexOf(needle, at + 1)) count++;
  return count;
}

/** Pure checks over the downloaded asset; `failures` is empty when the release holds. */
export function assess({bytes, assetSize, githubDigest, sidecar, manifest, mise}) {
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  const elf = bytes.length > 20 && bytes.subarray(0, 6).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46, 2, 1]))
    && bytes.readUInt16LE(18) === 62;
  const assertion = `RUN mise --version | grep '^${mise} '`;
  const result = {
    size: bytes.length, sha256,
    elf: elf ? 'ELF64 little-endian x86_64' : 'unexpected',
    githubDigestMatches: githubDigest === `sha256:${sha256}`,
    sidecarMatches: sidecar === sha256,
    manifestMatches: manifestDigest(manifest, ASSET) === sha256,
    knownStringsPresent: KNOWN_STRINGS.every(s => bytes.includes(Buffer.from(s))),
    embeddedMiseAssertion: assertion,
    embeddedMiseAssertionCount: occurrences(bytes, Buffer.from(assertion)),
  };
  const failures = [
    [assetSize === bytes.length, 'downloaded size differs from the asset size'],
    [elf, 'asset is not an x86_64 ELF64 executable'],
    [result.githubDigestMatches, "SHA-256 differs from GitHub's asset digest"],
    [result.sidecarMatches, 'SHA-256 differs from the .sha256 sidecar'],
    [result.manifestMatches, 'SHA-256 differs from SHA256SUMS'],
    [result.knownStringsPresent, 'asset lacks strings every runner carries'],
    [result.embeddedMiseAssertionCount === 1, `embedded image does not assert Mise ${mise} exactly once`],
  ].filter(([ok]) => !ok).map(([, message]) => message);
  return {result, failures};
}

function main(argv) {
  const [tag, source, flag, outArgument] = argv;
  if (!/^v\d+$/.test(tag ?? '') || !/^[0-9a-f]{40}$/.test(source ?? '') || (flag !== undefined && flag !== '--out')) {
    throw new Error('usage: node tools/verify-release.mjs <tag v<n>> <full source sha> [--out <path>]');
  }
  const gh = (...args) => execFileSync('gh', args, {maxBuffer: ASSET_BYTES_MAX});
  const api = path => JSON.parse(gh('api', path).toString('utf8'));
  const ref = api(`repos/${REPOSITORY}/git/ref/tags/${tag}`);
  if (ref.object.type !== 'tag') throw new Error(`${tag} is not an annotated tag`);
  const tagObject = api(`repos/${REPOSITORY}/git/tags/${ref.object.sha}`);
  if (tagObject.object.sha !== source) throw new Error(`${tag} targets ${tagObject.object.sha}, not ${source}`);
  const runs = api(`repos/${REPOSITORY}/actions/workflows/release.yml/runs?branch=${tag}&per_page=10`).workflow_runs
    .filter(run => run.head_sha === source && run.conclusion === 'success');
  if (runs.length !== 1) throw new Error(`expected one successful release run for ${tag}, found ${runs.length}`);
  const release = api(`repos/${REPOSITORY}/releases/tags/${tag}`);
  const assetOf = name => {
    const asset = release.assets.find(candidate => candidate.name === name);
    if (!asset) throw new Error(`release ${tag} has no asset ${name}`);
    return asset;
  };
  const download = name => gh('api', '-H', 'Accept: application/octet-stream', `repos/${REPOSITORY}/releases/assets/${assetOf(name).id}`);
  const dockerfile = gh('api', '-H', 'Accept: application/vnd.github.raw', `repos/${REPOSITORY}/contents/runner/agent.Dockerfile?ref=${source}`).toString('utf8');
  const asset = assetOf(ASSET);
  const {result, failures} = assess({
    bytes: download(ASSET), assetSize: asset.size, githubDigest: asset.digest,
    sidecar: download(`${ASSET}.sha256`).toString('utf8').trim().split(/\s+/)[0],
    manifest: download('SHA256SUMS').toString('utf8'), mise: assertedMise(dockerfile),
  });
  const record = {
    release: tag, releaseId: release.id, releaseRun: runs[0].id, source, tagObject: ref.object.sha, assetId: asset.id,
    ...result, hostingImmutable: release.immutable ?? null,
    integrityScope: 'point-in-time asset/source association; not build attestation',
    verifiedAt: new Date().toISOString(),
  };
  console.log(JSON.stringify(record, null, 2));
  if (failures.length) throw new Error(`release ${tag} failed: ${failures.join('; ')}`);
  const out = outArgument ?? `evidence/native-${tag}-integrity-${record.verifiedAt.slice(0, 10).replaceAll('-', '')}.json`;
  if (existsSync(out)) throw new Error(`${out} exists; records are never overwritten`);
  writeFileSync(out, JSON.stringify(record, null, 2) + '\n');
  console.error(`recorded ${out}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { main(process.argv.slice(2)); } catch (error) { console.error(`verify-release: ${error.message}`); process.exit(1); }
}
