import { execFileSync } from "node:child_process";
import { appendFileSync, cpSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repository = "isp-insoft-gmbh/isp-golem-xiv";
const github = `https://github.com/${repository}`;

function gh(...args) {
  try {
    return execFileSync("gh", args, {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      timeout: 90_000,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, GH_HOST: "github.com", GH_NO_UPDATE_NOTIFIER: "1" },
    });
  } catch {
    throw new Error(`Could not read public GitHub ${args[0]} data for ${repository}`);
  }
}

function releasePage() {
  const { tag_name: tag, body_html: notes } = JSON.parse(gh(
    "api", "-H", "Accept: application/vnd.github.html+json", `repos/${repository}/releases/latest`,
  ));
  if (typeof tag !== "string" || !/^v[0-9]+$/.test(tag) || typeof notes !== "string" || !notes.trim()) {
    throw new Error("Latest published release has no usable tag or notes");
  }
  // GitHub renders and sanitizes the public release notes.
  // Escape braces so a release note cannot invoke an mdBook include.
  const body = notes
    .replace(/<br>\s*-----BEGIN SSH SIGNATURE-----[\s\S]*?-----END SSH SIGNATURE-----/, "")
    .replaceAll("{", "&#123;")
    .replaceAll("}", "&#125;");
  return `
## ${tag}

[Download ${tag} and see the original notes on GitHub](${github}/releases/tag/${tag}).

<div>

${body}

</div>
`;
}

function reportsPage() {
  const { workflow_runs: [run] } = JSON.parse(gh(
    "api", `repos/${repository}/actions/workflows/ci.yml/runs?branch=trunk&event=push&status=success&per_page=1`,
  ));
  if (!run || run.head_repository?.full_name !== repository || run.head_branch !== "trunk" ||
      run.event !== "push" || run.conclusion !== "success" || !/^[a-f0-9]{40}$/.test(run.head_sha) ||
      !Number.isSafeInteger(run.id) || run.id < 1 ||
      !Number.isSafeInteger(run.run_attempt) || run.run_attempt < 1) {
    throw new Error("No successful public trunk push CI run found");
  }

  // Only publish three numeric measurements from the coverage step, never the log.
  const job = successfulCiJob(run);
  const log = gh("api", "--allow-escape-sequences", `repos/${repository}/actions/jobs/${job}/logs`);
  const lines = [...log.matchAll(
    /^[0-9TZ:.+-]+ x: (router|runner|x) line coverage ((?:[0-9]{1,2}\.[0-9]|100\.0)%) \(baseline ((?:[0-9]{1,2}\.[0-9]|100\.0)%)\)\r?$/gm,
  )];
  const rows = ["router", "runner", "x"].map((crate) => {
    const matches = lines.filter((line) => line[1] === crate);
    if (matches.length !== 1) throw new Error(`Expected one CI coverage measurement for ${crate}`);
    const [, , measured, baseline] = matches[0];
    return `| ${crate === "x" ? "xtask" : crate} | ${measured} | ${baseline} |`;
  });
  return `
## Rust line coverage

Source: [successful trunk CI run ${run.id}, attempt ${run.run_attempt}](${github}/actions/runs/${run.id}/attempts/${run.run_attempt}) on [\`${run.head_sha.slice(0, 12)}\`](${github}/commit/${run.head_sha}).

| Crate | Measured | Minimum at that commit |
| --- | ---: | ---: |
${rows.join("\n")}

These are Linux line measurements, not branch coverage.
The host report does not cover the wasm-only Worker glue or the JavaScript coordinator.
The run's check and step logs show the other CI results.

## Mutation testing

CI does not publish a mutation report.
Scoped mutation runs during development are diagnostic, not a merge gate.
`;
}

function successfulCiJob(run) {
  const { jobs } = JSON.parse(gh(
    "api", `repos/${repository}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`,
  ));
  const job = jobs.find(({ name, conclusion }) => name === "ci" && conclusion === "success");
  if (!Number.isSafeInteger(job?.id) || job.id < 1) {
    throw new Error("Selected CI attempt has no successful ci job");
  }
  return String(job.id);
}

const [argument, ...rest] = process.argv.slice(2);
if (!argument || rest.length) throw new Error("Usage: mise run book -- <new-output-directory>");
const destination = resolve(argument);
if (existsSync(destination)) throw new Error("Book output directory already exists");

const { visibility } = JSON.parse(gh("api", `repos/${repository}`, "--jq", "{visibility}"));
if (visibility !== "public") throw new Error("Book inputs must come from the public code repository");

const book = mkdtempSync(join(tmpdir(), "golem-book-source-"));
try {
  cpSync("book/book.toml", join(book, "book.toml"));
  cpSync("book/src", join(book, "src"), { recursive: true });
  appendFileSync(join(book, "src", "release.md"), releasePage());
  appendFileSync(join(book, "src", "reports.md"), reportsPage());
  execFileSync("mdbook", ["build", book, "--dest-dir", destination], { stdio: "inherit" });
} finally {
  rmSync(book, { recursive: true, force: true });
}
