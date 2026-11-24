# Decisions

Why things are the way they are, newest first.
Each entry: date, decision, reason, and evidence where it exists.

## 2026-11-24: Line coverage ratchet

`mise run ci` runs the Rust tests once under `cargo-llvm-cov` 0.9.2 (aqua, checksum-verified) instead of plain `cargo test`, then reports line coverage per crate and fails when a crate falls below `coverage.toml`; `mise run coverage` runs only that part.
The rustup component `llvm-tools` joins the pinned toolchain; it is a component of the same 1.99.1, not a toolchain change.
Each crate's report ignores the other crates and test-only files (`tests/`, `tests.rs`, `*_tests.rs`) with a filter anchored at the repository root, because an unanchored `runner/` also matches a CI runner's home `/home/runner`.
The xtask reads the summary's totals itself instead of adding `serde_json`; unit tests pin the baseline format, the totals parsing and the anchored filter.
Only Linux enforces the ratchet: Windows skips `cfg(unix)` code and tests and measures runner 83.2% against Linux's 88.9%, so other hosts print the comparison without failing.
The first baseline is two identical WSL Linux measurements: router 98.4%, runner 88.9%, xtask 46.7%; the router's Worker glue is wasm-only and the coordinator is JavaScript, so neither is in these numbers.
A red check raised the router's value to 98.5% and the Linux run failed naming only the router, while crates exactly at their value passed.
The baseline carries no margin, and because deploy, release and the weekly schedule run the same `mise run ci`, a coverage dip blocks them as well as pull requests: one gate everywhere, as DEVELOPMENT.md asks.
That makes nondeterministic coverage a deploy risk, so the baseline is adopted only after two Blacksmith `ci` runs of the same commit report the same numbers as above; a later flaky line is fixed in its test, not absorbed by a margin.

## 2026-11-24: Development goals re-evaluated, then realised

Oliver asked to re-evaluate the goals in DEVELOPMENT.md and realise them, rather than describe today's state next to them.
A goal-by-goal check against the repository found most goals met and four open, which Oliver decided:

- **Coverage:** a line coverage ratchet per crate with `cargo-llvm-cov` and a committed baseline; branch coverage is dropped from the goal, because the pinned stable toolchain rejects its experimental option (2026-11-04 entry "Guarded Slice 2 implementation, live proof pending").
- **GitHub Pages:** the mdBook site with user documentation, release notes and CI reports is built.
- **Snapshots and properties:** `insta` for text a human reviews and `proptest` for parsers and seat names are both adopted; the existing hand-written exact-text tests move to snapshots where they cover that text.
- **Real deliveries:** payload parsers are tested with webhook deliveries captured from the App and scrubbed, replacing hand-written fixtures where they cover the same events.

The goals that were stale were corrected to fact: every crate denies `unwrap_used` and its siblings, the task list grew, the xtask runs `tools/ci.nu` only for Nushell parse checks, and there is no job summary; check runs and step logs report results.
Golem examples in the documentation are now parsed by `parse_golem` in `router/tests/docs.rs`; an invalid example in the README fails it.

## 2026-11-23: Releases check out with upstream `actions/checkout`; v22 is burned

The v22 release run (45288450849), the first `release.yml` run after the Blacksmith migration, failed at "Validate release tag".
Its log shows `useblacksmith/checkout` at `75c49b6` fetching `+fdaad78…:refs/tags/v22`: its tag test compares the commit with `rev-parse` of the ref, which for an annotated tag is the tag object, so it always falls back and overwrites the annotated tag with a lightweight ref to the commit.
The gate's `git cat-file -t` then reads `commit`; the remote `v22` is still an annotated tag.
Loosening the gate is no fix: the publish step reads the notes from the tag object and would ship the commit message instead.
A probe workflow on a scratch branch, triggered by an annotated `golem-probe-*` tag on the same Blacksmith runner label, reproduced it without spending a release tag (run 45289013436): `useblacksmith/checkout` gave `commit` and failed, `actions/checkout` at the previous pin gave `tag` and the tag's own subject; probe tag, branch and workflow were deleted afterwards.
`release.yml` returns to upstream `actions/checkout`; CI and deploy keep `useblacksmith/checkout`, which only checks out branches.
Release tags are immutable and the workflow runs from the tagged commit, so v22 can never pass; it stays unpublished and the next release is v23.
Until a release publishes and ops pins it, golems still carry Mise 2026.11.10.

## 2026-11-23: Golems carry Mise 2026.11.12

Toktrak raised its Mise `min_version` to 2026.11.12 (Toktrak #166, merged 19:45Z), and both canary runs on Blacksmith then failed target preparation: the guarded output tail showed "mise version 2026.11.12 is required, but you are using 2026.11.10" (ops runs 45272876593 and 45273147964).
Oliver approved advancing in the recorded order: the agent image's version assertion, CI's Mise for the container test and the supported-minimum fixture here, then a verified release, the ops runner pin with its Mise version, and the matching `ops/` template.
A Mise bump in any target repository blocks its golems until golems carry that Mise; the failure is now named in the check and the run log.

## 2026-11-23: Queue and event records are rooted in blobs

After the Blacksmith ops change, the router logged every retirement of one terminal row as deferred with GitHub status 403.
A live probe with the App's installation token in the ops repository showed the pattern: a new tag ref to a blob, or to an annotated tag of a blob, is created (201); a ref to the row's request tag, to a tag chained on it, or straight to its commit `4078fa5`, which is no longer trunk and has different workflow files, is refused with 403 "Resource not accessible by integration"; a ref to the current trunk commit is created.
The App has no `workflows` permission; the workflow difference is the best-supported cause, though GitHub's message is generic.
Request tags, and the audits, run aliases and receipts chained on them, all pointed at the trunk commit current at admission, so every row in flight across a change to the ops workflows, which every runner pin is, could never be audited or retired and kept one of the 32 queue slots.
Event sources and their done receipts had the same shape.
Oliver chose blob-rooted records over granting the App `workflows: write`, which would let it rewrite workflows in every organization repository.
New request tags and event source tags are annotated tags of a blob holding their message; a retirement receipt is a blob holding the request tag's id followed by a newline, so anyone can check it from Git's blob id without reading it.
Receipts that point at the request tag itself remain valid, and the Slice 2 proof tools accept both forms.
The coordinator test double models the probe: after a pin moves trunk, refs whose chain ends at an older commit get 403.
Against the previous coordinator, the scenarios for a row in flight across a pin and for a legacy row with a blob receipt fail for exactly those reasons, while legacy-receipt compatibility passes on both.
Rows admitted before this change stay commit-rooted: they still retire if audited before the next workflow change, as today's stuck row was, but one audited only after it can never be, so pin a runner only when no such row is unaudited.
The repair helper's reviewed coordinator digest advances; its association rules for `dispatching` rows are unchanged, and it refuses `--apply` until this coordinator is deployed.

## 2026-11-23: Failed retirements no longer refuse admission; failures are logged

A canary mention on Toktrak at 20:37:52Z got its 202, eyes reaction and a queued `golem/canary` check, but no `golem-request` tag, queue row or run.
The only queue row then was a terminal row of a deleted acceptance repository; its immutable audit validated in a read-only replay, so `enqueue` had gone on to the retirement receipt, whose failure, like a journal conflict before it, aborted the whole admission.
Which of the two failed is unknown: the Worker had no logs, and invocation analytics only showed every request completing.
The router never retries an admission and keeps the delivery claim, so a refused admission lost the mention and left its check queued; the test "committed lost … retirement responses remain fenced" had pinned that refusal, assuming a retry.
Now a failed audit or receipt only defers that row's retirement, which still requires its receipt; the new request is admitted.
A receipt naming a different request tag used to fail the admission with 409; now the row stays terminal and is logged, holding no seat but one of the 32 queue slots until someone investigates.
Journal write conflicts still refuse an admission, since the admission itself must be saved.
Reconciliation ticks now skip journal writes that change nothing: with a terminal row present, every one-minute tick had committed twice and set off push webhooks; callbacks and admissions write as before.
Router and coordinator log fixed failure stages and statuses, never request data; Worker observability keeps these logs, with invocation logs off so request URLs such as the OAuth callback's code are not recorded, and query strings redacted.
The repair helper's reviewed coordinator digest advances to this coordinator; its association rules for `dispatching` rows are unchanged, and it refuses `--apply` until this coordinator is deployed.

## 2026-11-23: CI and golems run on Blacksmith runners

The organization's GitHub Actions budget stopped private-repository jobs (ops run 45250684632: "an Actions budget is preventing further use"), and every golem runs in the private ops repository.
Oliver chose Blacksmith runners and trusts Blacksmith with what those jobs hold, including the App private key, seat sign-ins and private logs; Blacksmith's migration wizard opened the change for this repository.
All jobs use `blacksmith-*-ubuntu-2404`, not 26.04: the runner is built for `x86_64-unknown-linux-gnu` and links the build host's glibc, so it must be built on an Ubuntu no newer than the one golems run on.
Third-party actions stay pinned to full commit SHAs, as before.
Blacksmith transparently serves `actions/cache` from a colocated, branch-scoped cache, so the agent image cache keeps its rule: only the save step right after preparation, before any seat, writes it.
Sticky disks and Blacksmith's Docker layer builder commit their state at the end of a job, after the agent ran, so the golem runner does not use them for the agent image; public CI, where no agent runs, may.
Blacksmith's Docker container caching keeps an organization-wide image store and merges newly pulled images back at the end of each job, including golem jobs after their agent ran.
A digest-pinned base image does not protect a tampered local image store, so this stays an open risk until container caching is turned off for the organization or the risk is accepted.

## 2026-11-23: Seat health from audits; runs before audits counted apart

SPEC promised a warning for refreshing seats close to expiry, but nothing the router can read holds an expiry: seat secrets are write-only, a run learns only an access token's expiry, a refresh token's lifetime is not published, and the start of an Anthropic setup token's year is never recorded.
Refresh results are not router-readable either: a successful write-back is only printed to the private Actions log, and a failed one is a check-summary line that sanitized audits do not carry.
Oliver first chose "router-only health" from a question that wrongly claimed audits and check notes already held write-back state; asked again with these facts, he chose audit-only signals.
The seats table now shows each seat's latest successful (`changed`, `replied`, `no_change`) and latest blocked run among the audited runs loaded so far, matched in the seat secret's form (login uppercased, `-` as `_`), because departed members are known only by that form; the health panel states that seat refresh state is not recorded.
SPEC's seat and dashboard lines now promise exactly that; warning before a sign-in expires remains an open gap that would need the runner to record expiry or refresh results.
The 87 runner runs created before 2026-11-13T12:51:43Z, when run 43428751065 became the first with a sanitized audit, never had one, while all 47 later completed runs do.
The dashboard counts such earlier runs as predating audits instead of as missing audits, so missing now means an audit that should exist; nothing is backfilled or fabricated, and these runs leave the 30-day window by mid-December.

## 2026-11-23: A blocked golem's reply says why

SPEC says a `blocked` run needs a human, yet its reply only said "Run blocked: agent reported task blocked" and dropped the golem's own reason, which was neither posted nor logged.
Oliver chose to quote the golem's message after that static line, guarded like any reply.
Only a typed blocked result carries it; runs blocked by the runner itself, or with a missing or malformed disposition, keep the static reason alone, so untyped prose is never presented as an explanation.

## 2026-11-21: Golems start faster so stop stays prompt

Slice 4 acceptance failed only on `stop`: a stop reaches a dispatched run when its runner binds, and binding waits for harness preparation, which grew from about 40 s to about 6 min.
240 s of it was the Codex offline self-test, whose two probes each waited out a 120 s kill timeout because offline Codex retries forever and the built-in `openai` provider's retry counts cannot be overridden.
Oliver chose both remedies: cache the built image across runs, and end self-tests as soon as they are decided.
The trusted Codex probe now ends once its MCP marker and `thread.started` are present (at most 120 s).
The untrusted probe asserts that marker never appears, which no event decides early; Oliver chose a 30 s window instead of 120 s, about three times the 10.5 s by which the trusted server had started in CI.
The runner labels the image with a SHA-256 over its staged sources and the pinned Mise executable; `--image-key` prints that digest plus the UTC month as the workflow cache key, so unpinned base packages are still rebuilt at least monthly.
`--prepare <mise> <cache>` loads a cached image only if its label matches and otherwise builds, including the self-tests, and saves the new image.
The label is self-asserted, so it only rejects stale or corrupt caches; integrity rests on who can write that cache key.
The runner workflow will restore the cache before preparation and save it in an explicit step right after preparation, before any step that holds a seat or runs an agent, so an agent can never write an image a later run loads.
Runs are dispatched on the ops default branch, whose caches only default-branch workflows write, and whoever can change those can already change the workflow itself.
On hosted CI (run 45144744220) a cold build and save took 2 min 51 s, a warm load 9.6 s, and the cached image is 660.7 MB.
A cold golem run therefore still sits at the three-minute stop limit, so the ops repository will warm the cache in its own workflow whenever its trunk or runner pin changes and monthly, and golem runs normally load a warm image.

## 2026-11-20: Container failures are diagnosable from the private run log

Diagnosing why Toktrak's verification failed took five probe pull requests, because the runner discarded container output and named only the failing caller.
Oliver chose to print a bounded tail of target preparation and verification output to the private Actions log on every run, passing or failing, for diagnosis and performance trends; SPEC already lets the private ops logs contain target code.
This supersedes the 2026-11-19 entry's rule that captured streams never reach diagnostics, for these two credential-free stages only; agent output is still never logged.
Each tail is at most 200 lines and 16 KiB per stream, is withheld entirely if it contains a guarded credential as UTF-8 or UTF-16, has control characters replaced, and is fenced by a random `stop-commands` token so target output cannot issue workflow commands.
Every container stage also logs its exit code, duration and output sizes, a phase line precedes each runner step, and blocked or incomplete checks name their static reason, which the requester reply already showed.
Oliver chose that an exit 137 before the host deadline blocks as a kill instead of counting as a timeout with partial publication: the in-image watchdog starts after the host deadline, so it never fires first.
Exit 124 keeps its previous timeout handling; changing it was not decided.
A hosted Linux run (cgroup v2, Docker 29.4.2) showed that real memory kills reach the host as exit 1, never 137, because the entrypoint reports a killed child as an ordinary failure, while Docker's `OOMKilled` record is set even when only a child process was killed.
The runner therefore asks Docker after any unsuccessful exit and, if a memory kill was recorded, blocks with "container failed after a process in it was killed at its memory limit".
The record is sticky and container-wide, set even when the command survived the kill and failed for another reason, so the message states an observation, not a cause.
The container test keeps checking both shapes on real Docker.

## 2026-11-20: Workspace links are recorded, never followed, and published only when safe

With the v18 process limit, a pristine Toktrak `mise run verify` passed fully in the verifier container (JUnit 277 of 277), then the production workspace scan panicked.
On Linux, jlink writes relative symbolic links under each runtime's `legal/` directory, and the scan rejected every link.
Oliver asked whether rejecting all links was too strict, then chose to also publish safe in-repo links.
The host still never follows a link: scans read link targets with `read_link` and never descend through directory links.
New links in paths the snapshot's `.gitignore` ignores are disposable like other ignored output and are never published.
A changed link is published, as a Git symlink (mode 120000), only if its target is relative, contains no backslash, drive or control characters, uses `..` only as a leading prefix, resolves lexically inside the repository, and does not resolve into a path golems may not change.
Published targets are limited to 100 bytes: Git archives store longer targets in a PAX `linkpath` record, which extraction does not accept, so a longer published link would block later runs on that branch.
Forbidden link paths and credential checks on link targets apply as for files.
A link may never stand at a protected path itself (`.claude`, `.codex`, `.pi`, `.agents`, `.golems`, `.git`, `CLAUDE.md`, `AGENTS.md`, `.mcp.json`, workflows): it would move guarded content to an unguarded path, so extraction and baseline scans refuse it; links into protected content stay allowed because writing through them changes the real, guarded path.
Snapshot extraction accepts the same inside links, so a later run on a branch with a published link still works; links are created after every file, and a link below another link is rejected, so extraction never writes through a link.
Allowing `..` only before normal components prevents escapes through intermediate links; it rejects some harmless targets.
The duplicate permission block after `set_file_mode` in extraction was removed.

## 2026-11-20: Golem containers allow 1024 processes and threads

A pristine Toktrak `mise run verify` in the v17 verifier container passed check, node and tracker tests, then its JUnit suite failed with `OutOfMemoryError: unable to create native thread` (276 of 277 tests passed), at 7 s in one run and 19 s in another.
Threads count toward `--pids-limit`, which was 128 for every golem container; agents running the same build hit the same cap.
Oliver chose to raise the single runner limit to 1024 for preparation, agent and verifier containers alike.
It still bounds fork bombs; memory (2 GiB), CPUs, read-only root, capabilities and all other container controls are unchanged.

## 2026-11-20: One definition per scan bound; ignored outputs never publish

v16 scheduled tester and refactor runs both blocked with `snapshot file limit exceeded`: agents ran the target build, and the post-agent scan still used repository bounds, so the verifier-only bound raised on 2026-11-19 never applied there.
Oliver asked for a single source of truth and approved this shape:
`changes::SOURCE_LIMITS` (4 MiB per file, 64 MiB total) is the only definition for archives, baseline scans and published changes; `snapshot.rs` imports it.
`changes::WORKSPACE_LIMITS` (64 MiB, 512 MiB) is the only definition for the workspace after agent or verifier; `scan_workspace` replaces both `scan_after` and `scan_verification`, `validate_workspace` replaces `validate_verification`.
`publishable` drops new files that the snapshot's own `.gitignore` files ignore, as Git would; baseline bytes decide, so run-time edits cannot hide a change, and files present in the snapshot, including their deletion, always remain candidates.
Every workspace change, ignored or not, is still checked for forbidden paths, links and handled credentials; only published bytes are frozen against verifier mutation, because verifiers legitimately rebuild ignored outputs.
Git semantics come from the `ignore` crate's gitignore matcher (ripgrep, MIT or Unlicense, no walking used) rather than a hand-written subset that could silently drop agent files; a leading UTF-8 BOM is skipped like Git.
The lock change arms the advisory gate: Oliver approved ignoring RUSTSEC-2026-0543 in `deny.toml`, `chrono` being unmaintained rather than vulnerable and reached only through the wasm `worker` crate (0.9.0 still requires it); yanked `serde-saphyr` moves 2.1.0 → 2.2.0.
Peak memory of the new shape (agent and verifier workspaces both held, each up to 512 MiB) is unmeasured; the 2026-11-19 profile covered one scan only.

## 2026-11-19: Match the verified v16 pin after composed Linux scans

Protectedc27 passed trunk CI/deployment/exact health; v16 release44551469572/source/tag/ELF/GitHub digest/sidecar/manifest match6d0cbe3be6a94a6adf7f93c9e3c1d57232bfdb991b0a20a969b0c9b830227b31.
OpsPR25 pinned reviewed bytes before credentials; reviewed/merged trees and postmerge CI matched, with private-plan manual gates rather than claimed server protection.
The public template now mirrors actual main workflowSHA77878193c3ea2eb60171060dcc648f062397bbd7de2ffff2a4fbc500a76f7107, retaining Mise10 and checksum/permission/cleanup fences.
Only the current fixed-digest fragment in historical test reconstruction changes; original v2/v3 golden hashes and repair scenarios remain.
Actual pristine Linux run44560461292 validated preparation/check plus baseline/post-preparation/post-verifier scans through a separately rebuilt source-pinned production-library helper, frozen scanned inputs and zero calculated publication changes.
That composition is not the released executable scan entrypoint, actual publication, registered-live-secret guard or provider proof; existing harness-state exclusions remain.
The earlier diagnostic rejected pre-start config, not target verification; Docker28.0.4 scratch evidence established Linux default-network representation bridge and unchanged launch flags.
No historical live failing predicate is inferred from that later fixture; no additional model canary until matching template gates and fresh rollout/target checks.

## 2026-11-19: Larger disposable verifier outputs, unchanged publication bounds

Oliver explicitly approved 64 MiB per file and 512 MiB total for verifier-output scanning.
Archive extraction, baseline and post-agent scans, and publication diffs remain at 4 MiB per file and 64 MiB total.
Entry/depth/path/collision/link/hard-link/forbidden-path/credential guards remain unchanged.
Verifier validation borrows changed paths rather than cloning disposable output bytes into candidate publication.
Frozen agent inputs still may not change or disappear; verifier-added artifacts are validated, never published.
Local pristine native check passed before the old production scan rejected an oversized file; this establishes local check-to-scan incompatibility, not the live failing phase or artifact.
Live recovery still requires complete Linux verifier-to-scan coverage and a strictly bound useful App answer.
No judge, target configuration, container resource, time or output-stream bounds change.

## 2026-11-19: Consume reviewed native bytes despite mutable release hosting

- v15 release44509183907 passed at protected3d; annotated tag/source, ELF64, GitHub digest, sidecar and manifest independently match c8343ddee808405286c97b4073ad555472a35fee1ef5c33925ca2d58be018d58.
  Embedded Mise10 assertion and fixed phase literals are byte evidence, not execution.
  Hosting reports immutable=false; never describe this as build attestation or immutable hosting.
- Ops PR22 changes only release URL, Mise10 and a fixed reviewed SHA256 check before credential selection, retaining sidecar verification.
  Credential-free actual sha256sum accepts approved bytes; after both fixture bytes and sidecar change together, the old sidecar check passes but the fixed digest rejects.
  Exact8b7 CI44511067094, refreshed target/reviews/threads and head-matched mergef115, tree equality and trunk44511335862 all verified.
  Private-plan branch-protection API returns entitlement403; manual gates enforced without bypass or visibility/plan changes, not claimed as server enforcement.
- Live workflow SHA25610fd5d4cb98b6a5ac97051c3c49857041ffbda27468a494c454fe3d6c7bf1387 equals reviewed committed bytes.
  Public template follows only after that delivery; historical v2/v3 reconstruction strips exactly the new fixed-digest lines and restores historical Mise.
  Actual old reconstruction was red at pin drift; corrected Node rerun reached unchanged original digest/provenance/lifecycle/CAS checks and exited0.
  Production repair code, trusted historical digests, judge and limits remain unchanged.
- Neither deployment, artifact integrity nor source pinning proves useful Toktrak execution.
  Refresh current target and bind one first-attempt canary to immutable request/authenticated bind/v3 audit plus actual App findings; do not unmask log JSON or infer from timing.

## 2026-11-19: Keep the native Mise pin compatible with the current target

- Whilev14 release CI ran, a fresh authoritative default ref revealed Toktrak0472, descending from repaired49b with dependency/tool/workflow/container pin changes only.
  Current minimum is2026.11.10; canary, native configuration and build contract remain byte-identical, and fourteen regular skill mirrors remain valid.
  Actual isolated Mise2026.11.9 probe rejects a minimal config requiring2026.11.10 with exit1, without installation or provider access.
- Never rewritev14 tag or replace its assets; hosting is mutable and integrity is point-in-time, not build attestation.
  Verifiedv14 contains the reviewed fixed-phase diagnostics but still embeds Mise2026.11.9 and must not be pinned for this current target.
  Advance only the image assertion, image-test CI tool version and existing supported-minimum fixture to2026.11.10.
  Keep the unsupported-minimum failure case, manifest/isolated-Node/read-only verification checks and every production bound unchanged.
  The already-approved fixed-phase diagnostics remain present; no target requirement is downgraded and no global tool or original user lock is touched.
- After required real-image CI and protected delivery, publish the next native release and verify its bytes before atomically updating live ops runner/Mise pins, then the public template.
  Fresh source evidence identifies a current minimum incompatibility, not the phase or cause of the historical generic failure.

## 2026-11-19: Distinguish nonzero container caller phases without exposing streams

- Original Toktrak49b Pi/Codex canary44336881526 reported only generic container execution failure.
  Later credential-free exact-target preparation succeeded on a v13 rebuild; neither fact identifies the historical failure phase or establishes auth failure.
  Independent source/pinned-Pi review found no concrete home/refresh/schema defect to justify speculative auth repair.
- Shared lifecycle receives only a fixed static caller message for nonzero start/attach outcome: preparation, harness or verification.
  This identifies the caller, not proof that the harness launched, the provider failed or a target command ran.
  Existing useful creation/validation errors, timeout handling, nested result boundaries and destruction acknowledgments stay unchanged; captured streams never enter these diagnostics.
- Existing fake-Docker scenario matrix now reaches production preparation/v1/writable/verification callers, emits secret sentinels on failure and checks destruction-failure precedence.
  Parent full local gate -120 passed under unchanged-lock advisory policy; Linux/Windows focused tests and narrow independent review found no concrete blocker.
  No actual Docker/provider/live execution or historical-diagnosis claim.
- Native publication was held until priority onboarding acceptance passed.
  After reviewed release/artifact integrity and live ops pin, observe a fresh causal canary; do not request login without evidence.

## 2026-11-19: Unchanged onboarding helper and caller passed after reviewed deployment

- PR51 exact7f CI44490179837 passed; protected mergef37 tree matches reviewed7f.
  Trunk CI44491782606 and deployment44491782598 passed; independent health exactf37.
- Actual `onboarding-request` blob `a2fde3c1c6e8c5b1c0996fbc3db1a606da955272` is unchanged from authorized141; neither judge file changed during JSON compatibility repair.
  Focused -121 executed it and caller merge/dashboard steps: exit0, `onboarding.ok:true`.
  It verified the no-auto window, explicit creation/exact files/rules, open and closed repeat identity, no unexpected reopen/duplicate, then explicit tester reopen, unchanged default branch before merge, merge65d0 and pinned dashboard2.
  Independent GitHub readbacks confirm merge/default head65d0 and exact App issue identity.
- Frozen receipt hash `4e6a2756af4bbdb7e91d7f6d666fcde3bb6f0d89107ccc833a4ee27b0b584700` preserved unchanged at `evidence/onboarding-acceptance-20261119.json`.
  Old-deployment unsolicited-PR red and intermediate415 failure remain recorded, not overwritten into success.
  This completes the changed onboarding check, not the later model scenarios or full Slice4 acceptance.

## 2026-11-18: Accept the JSON media type, not one header spelling

- Live unchanged acceptance helper on deployedbb9 completed its no-auto observation, then failed415 before creating any PR.
  Credential-free loopback inspection of the exact Nu command confirmed `Content-Type: application/json; charset=utf-8`.
  The endpoint's exact-string comparison rejected valid parameterized JSON; this is a product compatibility defect, not an auth failure or an excuse to change the judge.
- Match the case-insensitive media-type essence, preserving JSON parsing, body/time limits, membership, repository permissions and CSRF.
  Runtime regressions exercise charset and case variants while continuing to reject text/plain and JSON lookalikes.
  Full local required gate -116 passed under unchanged-lock advisory policy; reviewed deployment and unchanged helper/caller rerun remain pending.
- Separately, signed-in live Firefox physical clicks onbb9 created the exact App starter files, reused the open/closed PR without duplication or reopening, followed by an explicit tester reopen/merge.
  Fresh admin readback shows the exact repository and hello enabled, no starter action, with independent head reads fixed at6094; App dashboard2 is pinned with all sections.
  See `evidence/onboarding-live-20261118.json`; this does not repair or pass the Nu helper.

## 2026-11-18: Starter PRs require an explicit UI request

- Oliver rejected automatic starter PRs on push/App installation and explicitly selected implementation plus the matching specification/judge correction.
  Installed App plus existing `.golems/` still enables ordinary execution; no replacement activation flag.
  Existing unsolicited PRs remain untouched.
- Preserve push dashboard upkeep but return without onboarding when `.golems/` is absent.
  Admin UI offers **Create starter PR** only for known missing directories.
  Fresh organization membership plus requester repository write/admin permission is required; GitHub maps maintain to write, so custom role names never grant access.
  Cookie requests reuse session-derived CSRF and exact-origin checks; explicit bearer requests never fall back to a cookie and permit the independent acceptance driver.
- Reuse fixed starter files and once-per-repository ops claims.
  Return the existing verified App PR, even closed; never reopen, duplicate, overwrite an unrelated branch, or automatically resume an incomplete/ambiguous claim.
  A 422 must be followed by exact claim verification, not treated as success by itself.
- Judge change authorized separately: `contract.nuon` replaces automatic onboarding with no-auto plus explicit request/repeat/closed-PR reuse; `accept.nu` independently exercises that endpoint before its existing merge/dashboard caller.
  No removed checks, removed promises or raised limits; this is a user-approved changed promise, not weakening an unmet requirement.
  Standalone endpoint exercise is not browser wiring proof.
- Live red: exact updated `onboarding-request` against deployed085, owned sandbox `isp-golem-xiv-test-ui-onboard-red-20261118172608`, activity -104.
  Process exit0 wrapped actual `{"id":"onboarding","ok":false,"observed":"push or installation opened an unsolicited starter PR"}`.
  Green deployment/endpoint/caller proof remains pending.
- Local runtime gate -109 passed all four unchanged runtime/coordinator suites with new positive/negative onboarding cases, then failed status-list formatting.
  Corrected guards were never relaxed: error fixture now expects existing502 mapping, canonical-origin fixture replaces a correctly rejected foreign origin.
  Local Firefox160 pointer clicks and inspected mobile/desktop screenshots independently verify the delivered UI against an HTTP double; they do not establish real GitHub mutations.
- Existing admin-page evidence pointed at a relative JSON artifact, while unchanged evidence lookup recognizes HTTPS/test paths.
  Converted only its link to the immutable public085 blob URL; no new evidence, judge regex change or current-button claim.

## 2026-11-18: Preserve native skills and honor the target's Mise minimum

- Toktrak's first admitted recovery canary stopped at snapshot validation on its sole tracked link, `.agents/skills` → `.claude/skills`.
  Keep archive link/special-file rejection unchanged; materialize the fourteen byte-identical ordinary files in Toktrak and enforce mirror inventory/type/byte parity in its existing build check.
  Canonical `.claude/skills`, native Pi/Codex configuration and harness loading remain unchanged.
- Discovery and green repository CI do not prove agent execution.
  Require a fresh repaired-default-SHA canary and genuine App findings with its nonce; no provider-authentication failure or need for a new sign-in has been established.
- Native v12 preparation copies ops Mise 2026.10.21 and asserts that exact version in its embedded Dockerfile.
  Target preparation then evaluates workspace configuration before provider execution; Toktrak originally required 2026.11.2 and now requires 2026.11.9.
  Actual host 2026.11.2 rejects the current target with exit 1; independently integrity-verified task-local 2026.11.9 accepts the same config with exit 0.
  This is a source-grounded next preparation blocker, not the observed cause of the earlier snapshot failure.
- Update the exact image and image-test CI pin to 2026.11.9, not a config bypass, downgraded target requirement or custom version-negotiation layer.
  The credential-free container regression uses production preparation/verification entries, requires isolated Node resolution and version, verifies tools read-only, and rejects an unsupported minimum before writing a tool manifest.
  Local Docker execution is unavailable; its runtime proof remains the required Linux CI image job.
- Release and independently verify the new native artifact before atomically updating the live ops runner digest and Mise pin; publish the matching public template afterward.
  Keep unrelated deploy/release lint pins unchanged, because they do not prepare target images.
  The user's global WinGet Mise and original Toktrak lockfile are untouched.

## 2026-11-18: Finish ordinary-repository support at both execution boundaries

- The router-only rollout change left the same temporary topic predicate in the native runner's target validation.
  Remove that predicate too; repository identity, current membership, human/check binding and pinned definition validation remain.
  A real native HTTP-fixture test for an ordinary, topic-free repository was red with `target identity or allowlist mismatch`; the corrected v2 path and identity-negative cases pass.
  Release and pin the corrected native runner before deploying the separately held head-reader fix.
- The migration's large deletion commit exposed a second issue: the GitHub full-commit response is 526,823 bytes, above the unchanged 524,288-byte response cap.
  Head discovery needs only the branch SHA, so use the exact compact Git-ref endpoint rather than raising bounds or fetching patches.
  Encode branch path segments; require a commit object and valid SHA.
  Built-Worker regressions cover oversized commit data, pinned contents, nested and escaped branches, invalid references and unchanged oversize rejection.
- Full compact-head CI -53 completed exit 0 under the existing unchanged-lock advisory policy, with all four runtime/coordinator summaries confirmed.
  Gate -50 first exposed the coordinator fixture's stale endpoint; that fixture was corrected without relaxing assertions.
  Gate -52's suite log was green but its final local Nu exit wrapper failed on a string exit code; -53 captured the actual typed child exit code.
  These are local integration results, not live nine-valid discovery, provider execution or full Slice 4 acceptance.

## 2026-11-18: Installed organization repository plus Markdown is sufficient

- Oliver explicitly rejected per-repository activation and authorized removing the temporary sandbox-topic rollout restriction.
  An installed organization repository with `.golems/` uses the existing mention, event and schedule behavior; no replacement allowlist, flag or local workflow.
- Repository lookup and scheduled discovery no longer inspect topics; App access and existing repository identity, membership, seat and execution rules remain unchanged.
  Admin repository metadata and text no longer present a separate dispatch-allowlist state.
  Acceptance-test repository names/topics remain cleanup markers, not production dispatch controls; the judge is unchanged.
- The topic-free built-Worker fixture was red at its authenticated index lookup (404 rather than 200), then passed after the policy change.
  Full Worker gate -38 completed exit 0/78.80 s; pinned host/wasm Clippy, 39 router tests and the DOM consumer passed.
  Full local CI -43 completed exit 0 with the existing unchanged-lock PR advisory policy.
  The broader advisory scan separately reported transitive `chrono` RUSTSEC-2026-0543 and yanked `serde-saphyr` 2.1.0; dependency inputs are unchanged and no advisory ignore was added.
- Integrated fixture diagnosis found a preceding event source's final admission interleaved with the next case.
  Captured occurrence IDs established provenance; tests now assert the intended source's exact 20 admissions and retirement before changing definitions, without loosening the subsequent 18-row expectation, request bounds or tick limits.
  Three focused Worker repetitions passed before the full gate.
- This is rollout simplification, not a claim of full Slice 4 acceptance or successful new provider execution.

## 2026-11-17: Rewrite the admin UI as a plain, actionable document

- Oliver likes the simple admin interface and requested a rewrite that is understandable, semantic, readable, pleasant, fast and actionable without elaborate CSS.
  The rewrite uses native sections, navigation, tables, definition lists, buttons and details; no framework, remote font, animation or polling.
- A roughly 2 KB same-origin stylesheet provides readable spacing and bounded horizontal table regions on small screens.
  Only admin HTML adds `style-src 'self'`; existing script/connect/frame restrictions, fresh membership checks and API behavior remain unchanged.
- Coverage gaps and source failures stay visible; explanatory implementation details are collapsed.
  An independent review found undispatched failure/retry hidden in a disclosure; moving status and controls outside it resolved that finding.
  Retry disables during requests; expired pagination offers reload rather than resending an expired cursor; empty metadata never claims there were no runs.
- Current DOM regressions passed, including mixed-source recovery retaining existing rows without adding undispatched work to usage.
  Final parent full Worker gate completed with exit 0 in 60.21 s; all four suite summaries confirmed; CSS delivery, MIME, no-store, HTML reference, scoped CSP and non-GET rejection are covered.
- Firefox 160 local HTTP fixture served actual Rust-rendered HTML and current JS/CSS.
  Desktop 1440 px and mobile 390 px had no document overflow; labelled tables keep their own horizontal overflow instead of making text unreadable.
  Real retry terminal state showed two rows (one dispatched, one undispatched), one unique dispatched log and usage count 1; final pagination showed three audited records (two dispatched, one undispatched).
  These are invented fixture records, not live execution evidence.
  Keyboard focusability is verified, but native ArrowRight scrolling was not established after a headed-browser redraw timeout.
- PR #40 deployment and trunk CI subsequently passed; live health matched `0c06d46`, and authenticated browser/API checks showed historical run `43428751065` as model null/rendered unknown.
  At the local-review milestone the rewritten UI was not deployed; no full Slice 4 acceptance or judge change was claimed.
- Subsequent publication: PR #41 merged `b88f5f8` after exact-head CI `44108681183`; trunk CI `44110999820` and deployment `44111000121` passed.
  Actual deployed HTTP probe exited 0 with matching health/CSS/JS hashes, method/cache/MIME/auth guards and four member APIs; sanitized record `evidence/admin-ui-http-20261117.json`.
  Fresh `/seats` navigation recovered expired sign-in state; actual authenticated browser verification then matched `b88f5f8`, with HTML status 200, loaded CSS, same-origin style CSP, all five sections and private desktop/mobile screenshots inspected.
  Document widths matched 1440 px and 390 px viewports; native ArrowRight moved the focused table by 43.17 px without document overflow.
  Live pagination traversed an audit-empty page: two audited rows remained unique while workflow metadata advanced 10→20, missing audits 8→18 and repository sections 1→2.
  This does not establish new audited-row appending live; the existing automated overlapping-page scenario now explicitly asserts the new row appears once and existing run links remain unique.
  Sanitized record: `evidence/admin-ui-browser-20261117.json`; historical/expiry gaps and full Slice 4 acceptance remain incomplete.

## 2026-11-17: Live admin projection and unknown model correction

- PR #39 merged `da28fd6` after exact-head CI `44084644483`; trunk CI `44086507038` and deploy `44086507106` passed, and live health matches.
  Authenticated runs, seats, repositories and undispatched endpoints returned 200 with private/no-store responses.
  Page 1 reported one verified audit, nine missing audits and 88 workflows; this is not complete historical coverage.
- One read-only historical join succeeded for run `43428751065`, request tag `5aa18f1c36ed5752611de04281976b5ba4791324`, bound snapshot `08775555fe6e874b208704a899a1bd196b391aa5`, terminal snapshot `a321c61112bcfaa4e13f9c8ae2326f08a40fb2c5`, final check `131896504858`.
  Repository/head/App/seat/harness/run identities matched; duration is 387 s, while check `model: default` does not establish an actual model.
  Seven GETs, no writes; a recovery candidate for one run, not proof for all 88.
- The first v3 audit producer incorrectly accepted `default` as an observed model identifier.
  A Rust regression went red (`Some("default")` versus expected `None`); new audits now record null, existing immutable v3 placeholders normalize to null at read time, and retirement accepts that historical unknown without rewriting its audit.
  This preserves provenance rather than inventing observations or changing published history.
  Parent full Worker gate passed exit 0 in 73.53 s; pinned host/wasm Clippy, four admin tests and repair/digest checks passed.
  This only corrects the known placeholder; requested non-placeholder Codex model names still do not establish an independent provider observation.
- Fresh run `44089143230` was blocked (`container execution failed`), not a successful harness proof.
  Its v3 audit and admin record bind the source comment/request/check/run and correctly report blocked/null-model/367 s; the container stage is not retained by current diagnostics.
  Do not infer credential failure from that generic error or blindly redispatch.
- Browser proof is awaiting Oliver's sign-in to the fresh test Firefox window.
  Full Slice 4 acceptance remains unrun; judge unchanged.

## 2026-11-17: Organization dashboard and durable sanitized audits

- Red before implementation: tester-authorized `GET /v1/admin/runs` returned HTTP 404 on the deployed system.
  `admin-auth` already redirected to GitHub; `admin-page` evidence is still missing.
- Coordinator publishes an immutable v3 annotated `golem-admin/<request-id>` tag targeting the original request tag, plus a `golem-run/<run-id>` alias for dispatched work, before retirement.
  The closed audit schema includes request/run identity, repository, golem, trigger, seat, observed model, outcome, duration, completion time and fixed ops log URL; request instructions and credential values are never projected into dashboard output.
  Unknown observations stay null, not requested models or inferred successful outcomes.
- Duration means Actions `run_started_at` to final target check completion, not billing time.
  Check App/name/head/seat/harness/budget/log and repository identity are validated before audit construction; retries validate existing immutable records and never overwrite conflicts.
- Every admin data request checks current organization membership; bearer identity comes from GitHub `/user`, never from caller claims.
  Cookie sign-in reuses the existing OAuth session, returns to the fixed `/admin` destination, and also rechecks membership.
  Responses are private/no-store, text rendering is escaped, and log URLs remain fixed to ops.
- Pagination is bounded per request, with explicit missing/malformed/pending coverage and partial-page messages.
  A repository-specific metadata failure preserves pagination and is shown as unavailable, never as a disabled repository.
  Usage and health describe loaded verified records, not silently claimed whole-window totals.
- Historical retired runs lack the new sanitized audits and require verified backfill; no synthetic history is created.
  Refreshing-seat expiry is unknown because GitHub secret names do not contain expiry; secret update time is not expiry.
  Both gaps remain unresolved spec work, not accepted omissions.
- Independent review found audit maintenance could starve unrelated pending execution, and journal retries could exhaust the call budget after persisting dispatch intent.
  Reproductions went red before fixes: maintenance now leaves dispatch headroom, validated immutable requests are reused only within an operation, and dispatch-intent publication checks retry headroom after reservation.
  Unknown-dispatch fencing and late-bind refusal remain intact.
- Two parent full Worker gates failed integrated fixtures (selected-repository mismatch, then unsupported new check metadata reads exhausting the existing mock bound).
  Fixture resources were corrected; original scenarios, assertions and limits remain.
  Parent gate then passed exit 0 in 69.84 s.
  After the repository-pagination correction, final parent gate passed exit 0 in 42.17 s with all four suite summaries.
  Pinned Rust 1.99.1 Clippy passed on host and wasm; inherited Rust 1.100 had reported newer lints in unchanged code, so verification used explicit `rustup run 1.99.1` rather than modifying source for that environment.
  No judge changes, deployment or full Slice 4 acceptance claimed.

## 2026-11-17: Run Windows Worker suites in WSL

- Oliver selected WSL rather than skipping oversized-body assertions or draining unauthenticated request bodies.
  The 2026-11-03 transport experiment and the no-arbitrary-draining decision still hold; production router behavior is unchanged.
- `x::runtime_tests` invokes the same `tools/router-runtime-tests.mjs` with Linux mise and Node in the default WSL distribution on Windows; Worker compilation stays native.
  Linux CI keeps its native path and all four existing suites, assertions and timeouts stay unchanged.
- WSL startup, tool discovery and suite failures propagate through the existing bounded command runner; no unbounded readiness probe or automatic native retry.
  Presence of `GOLEM_WORKER_TESTS_NATIVE` explicitly selects native diagnostic execution.
- Oliver trusted this repository and installed its tools in WSL.
  `mise run test-worker` completed (exit 0, 210.41 s): runtime helpers, delivered Worker, integrated coordinator Worker, and coordinator SQLite/journal suites all passed through WSL.
  The delivered Worker suite includes the unchanged oversized-body 413 assertion; this is local built-artifact evidence, not deployed acceptance.
  Fmt, `x` Clippy and three unit tests passed; Linux PR CI remains the merge gate.

## 2026-11-13: Pi on OpenAI Codex seats (runner only)

- SPEC Credentials says one seat serves every harness that works with its provider, and `contract.nuon` lists `codex` and `pi` for `openai-codex`; the runner now executes `(openai-codex, pi)` (`RUNNER_PAIRS`), recomputing the choice from the pinned definition like every other pair.
  Runner v11 (release `43421782938`) was pinned by ops PR #17 first; only then did the router's `runner_supports` admit `openai-codex`/`pi`, so a `claude` or `pi` golem on a Codex-only payer now dispatches as `pi` instead of being refused.
- The stored seat stays in the Codex CLI `auth.json` format; Pi gets a converted copy.
  Pi 1.5.1 keeps sign-ins in `<PI_CODING_AGENT_DIR>/auth.json` (`pi-coding-agent/dist/config.js` `getAuthPath`, no separate override), keyed by provider: `{"openai-codex": {type: "oauth", access, refresh, expires, accountId}}` (`pi-ai/dist/auth/oauth/openai-codex.js` `credentialsFromToken`); `auth-storage.js` accepts an oauth entry only with string `access`/`refresh` and a finite number `expires`.
- `expires` (milliseconds) is the access token's own JWT `exp`, the same expiry Codex 0.167.1 refreshes on (`login/src/auth/manager.rs`: access-token `exp` first, the 8-day `last_refresh` rule only without one); Pi refreshes when less than five minutes remain (`pi-ai/dist/auth/resolve.js`).
  A seat whose access token has no decodable `exp`, or no `chatgpt_account_id` claim (Pi derives the request's account from the token on every call, `openai-codex-responses.js` `extractAccountId`), is refused for Pi before any container: guessing an expiry would make Pi refresh, and rotate the seat, on every run. Such a seat still runs `codex`.
- The copy lives in a private per-run directory mounted writable at `/golem-pi`, outside `/workspace`, and `container.cjs` points `PI_CODING_AGENT_DIR` there when `GOLEM_PROVIDER=openai-codex`; it is Pi's whole agent directory for that run, and Pi keeps its `proper-lockfile` lock `auth.json.lock` beside the file, so the directory, not just the file, must be writable.
  No Anthropic token is forwarded on a Codex seat; Anthropic seats keep `/home/agent/.pi/agent` and `ANTHROPIC_OAUTH_TOKEN` unchanged.
- `--provider` without `--model` is a Pi startup error (`main.js`), so `--provider openai-codex --model <m>` is passed only for a requested model; otherwise Pi picks its default for the only provider it has a sign-in for (`model-resolver.js` `findInitialModel`, `openai-codex` → `gpt-6.1-sol`).
  Project and runner flags are the same as for Anthropic Pi runs (`--approve`, `golem_report` extension, JSON mode).
- Write-back: after confirmed destruction the runner reads back only `auth.json` (bounded regular file, as for Codex).
  Pi refreshes under its file lock and rewrites the file in place (`writeFileSync`, `JSON.stringify(…, null, 2)`) but keeps only `access`, `refresh` and `expires`, discarding the refreshed `id_token`.
  If access or refresh token changed, the runner rebuilds Codex's file from the original seat's fields with the new tokens and a `last_refresh` it stamps itself at read-back (RFC 3339, seconds); `id_token`, `account_id`, `auth_mode`, `OPENAI_API_KEY` and unknown fields keep their values, keys are re-serialized in sorted order.
  Codex 0.167.1 reads the `id_token` only for claims (email, plan, account), never its expiry (`login/src/token_data.rs` `parse_chatgpt_jwt_claims`), and keeps the old one itself when a refresh response lacks one (`persist_tokens`).
  The account is compared on the access tokens' `chatgpt_account_id` claims (and Pi's `accountId` if present), then the result passes the existing Codex checks and the same sealed-box write-back with the ops-only `secrets: write` token and `golem seat: refreshed|unchanged|write-back failed` logs.
- Output guard: the seat in both formats as whole texts and every token value; after a Pi run the raw read-back and every string in it, and the converted Codex text before it is written.
- Agent image: Pi needs nothing new except the `/golem-pi` mount point; a new offline build probe seeds an invented Pi-format sign-in and expects an Anthropic-token-only control to exit 1 without output, then the default and the `--provider/--model` runs to select `gpt-6.1-sol`, fail the request offline as an assistant error event with exit 0, and leave the file byte-identical.
- Evidence: runner unit tests (conversion both ways, refusal of unusable seats, write-back keeping every other field, other-account and malformed Pi files rejected, guard contents, Pi arguments and environment for both providers, the choice binding and the `RUNNER_PAIRS` matrix).
- Unverified: the Dockerfile probe has not executed anywhere yet; its first run is the CI `test-agent` job of the pull request.
  No live Pi run on an OpenAI Codex seat: Pi authenticating, refreshing and the converted write-back being accepted by the next Codex run need a real seat.
  Whether the offline Codex transport fails fast enough (WebSocket first, then SSE) to stay inside the probe's 90-second kill is inferred from DNS failing immediately without network.
  Pi refreshes only near `exp`, Codex also by its 8-day rule only without `exp`; both stay inside the access token's lifetime, which is inference from source, not observation.
- Known gaps: the seat is readable by the agent and by repository-defined Pi extensions, as for every harness; `/golem-pi` is a host bind mount without a size bound, like `/golem-codex`; Pi decodes the token payload with `atob`, which rejects base64url `-`/`_`, so a token whose claims encode to those characters fails inside Pi (rare for ASCII claims, not observed).
  Runs on one seat never overlap (the coordinator serializes rows sharing `seat` = `<login>/<provider>`, independent of harness, `router/src/coordinator.js` `shares`), so `codex` and `pi` runs cannot race one refresh token; a run that refreshes but fails write-back leaves a rotated-away seat, noted in the check summary as for Codex.

## 2026-11-13: Dashboard last result and run boxes

- Closes the open gap of 2026-11-11 "Onboarding and dashboard on push".
  Each enabled golem's dashboard entry now has a hidden marker line carrying its last result, a visible `Last:` line derived from it, and a run box ``- [ ] Run `NAME` now <!-- golem-run golem=NAME -->``.
- The marker is the source of truth: `outcome` from the fixed outcome set, `seat=<login>/<provider>`, `harness`, `model`, `at` (UTC minute) and the ops run id are validated before rendering and when parsed; the only link is `https://github.com/<ops>/actions/runs/<id>`.
  Invalid or hand-edited markers are not carried over and render as "never run"; a hand-edited visible line is overwritten on the next render.
  Purposes escape `<`, so a purpose cannot open an HTML comment that hides markers or boxes.
- Router re-renders on push parse the current dashboard's markers and keep them.
- The runner, after the final check update succeeded and after container destruction, re-reads the dashboard (first page of open issues, App author, title, marker, then a fresh GET of that issue) and PATCHes only this golem's two lines with the existing target publication token (`issues: write`).
  It is best effort and logs exactly `golem dashboard: updated|not found|failed`; it never changes the outcome or queue completion.
  An unobservable model is shown as `unobserved`.
  Known race: concurrent runs of different golems, and a router re-render, each write the whole body; the re-read keeps the window short, last writer wins, and a lost line returns with that golem's next run.
  Not covered: outcomes written only by the coordinator (stopped before admission, blocked reconciliation) and the disabled v1 protocol do not update the dashboard.
- Run boxes: an `issues.edited` delivery whose sender is a `User`, on the App's dashboard (author, title, marker in both old and new body), whose box changed from `[ ]` to `[x]` is a dashboard command, not an `issues.edited` event trigger; other edits of the dashboard stay ordinary events.
  Old and new bodies are parsed from the delivery only to find ticks (each at most 64 KiB) and are never journaled, so event work does not grow.
  At most three boxes per edit start runs; further ticks in the same edit are cleared unrun.
- Mention rules apply: an inactive member gets one `not-member` refusal for the edit; per golem, a missing or disabled golem gets `unknown-golem` and a person without a runnable seat `no-seat`; the payer is always the ticking person, never the owner.
  Refusals in the shared dashboard issue start with the refused marker and then `@<login>`.
- Ticked golems become ordinary durable event work (`event: "dashboard"`) behind the delivery claim, so a redelivery is ignored and a restart resumes the remaining golems on the cron instead of dispatching twice; each request has id `occurrence_id(dashboard-v2/<delivery>/<golem>)`, trigger `dashboard`, the dashboard issue, no comment, actor and seat the ticking person, and instructions `Started from the dashboard run box.`.
  A refusal posted just before a crash may be posted again on resume.
- After the work is durable, the router clears ticked boxes in the freshest body (best effort; a box left ticked is never counted as newly ticked again).
- Runner: `dashboard` is human like `mention` (seat equals actor, no owner fallback, always answered, `no_change` never chosen); it requires no comment, and instead of the comment identity it verifies that the issue is the App's dashboard.
  Replies start with `@<actor>` after the marker.
- Coordinator: `@isp-golem-xiv stop` in the dashboard issue also stops the latest dashboard run, as the queued check summary promises; the reviewed digest in `tools/repair-unbound.mjs` advances to `75a4dd6b…23c0`, so the repair helper refuses the deployed Worker until this change is deployed.
- Rollout order: pinned runner v9 rejects trigger `dashboard` at `--validate`, before bind, leaving an unbound dispatched row that holds the golem and seat until a stop fence or repair.
  A runner release containing this change must be pinned in ops before the router deploys; the router has no runner-version gate.
- Unverified: that GitHub delivers `issues.edited` with `changes.body.from` when a box is clicked in the web interface, and that nested task-list boxes under a list item stay clickable; both need a live tick.

## 2026-11-12: Harnesses run the repository's own configuration

- Oliver decided that a golem runs each harness the way the target repository configured it: project settings, hooks, skills, commands, agents, extensions, rules, MCP servers and context files load natively, and the runner reimplements none of them.
- The prompt keeps only golem-owned content: capabilities and boundaries, house rules, golem task, trigger context and instructions.
  `CLAUDE.md`/`AGENTS.md` are no longer injected, and the sentence telling the agent to read nested ones is gone, because every harness loads them itself; injecting them now would duplicate them.
- Claude (`claude -p`): dropped `--safe-mode`, empty `--setting-sources`, `--strict-mcp-config` with an empty `--mcp-config`, `--disable-slash-commands`, and `disableAllHooks` in our `--settings`; now `--setting-sources project,local` (the user directory is fresh and empty).
  `-p` loads `.mcp.json` servers without asking and runs project hooks without trust; the entry also writes `projects["/workspace"].hasTrustDialogAccepted` into the fresh `$CLAUDE_CONFIG_DIR/.claude.json`, so trust-gated items (project `permissions.allow`, subagent frontmatter hooks and MCP, `headersHelper`) apply as after accepting the trust dialog (permissions docs, "Project allow rules and workspace trust").
  Writable runs use `--permission-mode bypassPermissions` instead of `--tools`/`--allowedTools` lists, so project skills, subagents and MCP tools are callable unattended; our `--settings` only adds `deny` for `WebFetch`/`WebSearch`.
  Deny rules from every source combine and apply even in bypass mode, so project denies still bind; project allows add nothing under bypass.
  The read-only (v1) reply protocol keeps its own `Read`/`Glob`/`Grep` tools and denies; project hooks run and `.mcp.json` servers start there too, against the read-only mount, but MCP tool calls are refused because only those three tools are offered.
- Pi: dropped `--offline`/`PI_OFFLINE` (offline skips installing project packages), `--no-extensions`, `--no-skills`, `--no-prompt-templates`, `--no-context-files`, `--no-mcp`, the `--tools` allowlist and `--append-system-prompt`; `PI_SKIP_VERSION_CHECK=1` and `PI_TELEMETRY=0` stay; `--no-themes` stays because themes only affect the interface.
  Deviation from the instruction "keep `--no-approve`": in Pi 1.5.1 `--no-approve` declines project trust (`cli/args.js` sets `projectTrustOverride=false`; docs `cli.md`, `security.md`), and Pi never asks before tool calls, so there is nothing unattended about it.
  Runs pass `--approve` instead, the documented one-process trust override that applies before extensions, saved decisions and `defaultProjectTrust`.
  `--append-system-prompt` would hide a project `.pi/APPEND_SYSTEM.md` (it is only discovered without a CLI append, `resource-loader.js`), while a project `.pi/SYSTEM.md` drops the section carrying the tool's guidelines; so the `golem_report` instruction now travels in the prompt for `pi` runs.
  `/opt/golem/pi-report.js` still loads via `--extension`; CLI extensions load before project ones, so a project extension registering `golem_report` is a load-time tool conflict, which Pi reports as an extension error and exits 1 before any event in non-interactive modes (`resource-loader.js` `detectExtensionConflicts`, `main.js`): fail closed.
  The same rule means any project extension that fails to load fails the run.
- Codex (`codex exec`): dropped `--ignore-rules` and `project_doc_max_bytes=0`; added `--dangerously-bypass-hook-trust`, because project hooks otherwise wait for an interactive review, and `--config 'projects={"/workspace"={trust_level="trusted"}}'`.
  The loader merges `--config` before resolving project trust (`config/src/loader/mod.rs` 0.167.1); the inline table is deliberate, because `overrides.rs` splits dotted keys on every `.` without honoring quotes, so `projects."/workspace".trust_level` would never match.
  `--ignore-user-config` stays: it only blanks the user layer (`loader/mod.rs` `load_user_config_layer`), never the project one, and keeps anything Codex writes into the private home out.
  Our `--config` keys (web search off, no history, analytics, feedback or update check) sit above project layers and win over `.codex/config.toml` for those keys only.
- Streams stay strict: exactly one typed result per harness.
  Records of unknown types, which hooks, MCP or extensions may add, are ignored by the parsers and dropped by the stream filters (Codex unknown types used to fail the run); malformed lines still fail closed.
  An in-process Pi extension could write anything to stdout, so the name conflict is a guard against accidental clashes, not a boundary; trusting project configuration means trusting the repository's committed harness code like its build scripts.
- Agent image: `python3` (Debian) and pinned `uv` 0.13.7 with `uv`/`uvx` on the fixed PATH, because project MCP servers and skills commonly start with `uvx`; `node`/`npx` were already there.
  The home is `noexec`, so the entry points uv's interpreter and tool directories at `/tmp` (its cache already lives there); that `/tmp` is executable only in writable runs, and no `uvx` server has run in a golem yet.
  Network stays as before.
- Protected paths are unchanged (`runner/src/changes.rs`): golems may read but never change `.golems/`, workflows or harness configuration (`.claude`, `.codex`, `.pi`, `.agents`, `.mcp.json`, `.claude.json`, `CLAUDE.md`, `AGENTS.md`), as SPEC Security requires; configuration that could rewrite itself would let a golem change how every later golem runs.
- Credential exposure changed: repository-defined hooks, extensions and MCP servers run as children of the harness and inherit its seat (`CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_OAUTH_TOKEN`, Codex `auth.json` under `/golem-codex`), deterministically, not only when the model is talked into it.
  Anyone who can push harness configuration to a repository can therefore read the seat of whoever runs a golem there; output scanning still blocks publishing it, not exfiltrating it over the network.
- Harness state: after the agent, the workspace is scanned against the snapshot (`changes::scan_after`); paths inside `.claude`, `.codex`, `.pi` or `.agents` that did not exist in the snapshot (package installs, caches, symlinks) are skipped entirely: not counted against limits, not link-checked, never published.
  Files present in the snapshot are still compared, so changing or deleting project configuration still blocks; new files under `.golems/`, workflows, `CLAUDE.md`, `AGENTS.md` and `.mcp.json` still block.
- Known gaps: Claude plugins install into the `noexec` home and may not run; read-only runs have a `noexec` `/tmp`, so `npx`/`uvx` MCP servers fail there.
  Whether Codex 0.167.1 also needs `[features].hooks = true` for project hooks is unverified; a repository can set it in its own `.codex/config.toml`.
  Larger streams from hooks and MCP count against the 1 MiB output bound.
- Evidence: native unit tests (absent disabling flags, required flags, prompt without repository instructions, parser cases for extra records and the duplicate-name exit).
  Image probes, run by CI only, each with a negative control: Pi loads a project extension (marker) under `--approve` and not under `--no-approve`, and the `golem_report` conflict exits 1; Codex starts a project `.codex/config.toml` MCP server (marker) with the inline-table trust override and not without it.
  Claude's probe proves startup with the production read-only flags, project settings and the trust file; it proves the trust effect only if Claude prints its untrusted-workspace warning before stopping at sign-in, which the untrusted control run checks and the build log reports.
  No live run with project configuration yet.

## 2026-11-12: No Copilot provider; harness-only fallback

- Oliver removed `github-copilot`: nobody uses it. Providers are `anthropic` and `openai-codex`; a golem naming `github-copilot` is now invalid like any unknown provider, and the seats page, help, dashboard and coordinator no longer offer or accept it.
- A payer with a seat at the golem's provider whose harness does not work there (e.g. `codex` on `anthropic`) now keeps that seat and runs `pi` with its default model, saying `asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic`.
  Before, `choose_seat` returned the unrunnable pair unchanged and dispatch refused it as `no-seat` (red: probe issue 8 comment `6822171957` got `kind=refused reason=no-seat`).
  A runner built from this change recomputes the same choice from the pinned definition and accepts exactly this fallback text.
  The pinned v7 runner still derives `anthropic/codex` without fallback and would reject such a dispatch after it started, so a runner release must be pinned in ops before the router deploys (same ordering lesson as v6).
  SPEC "Which seat and model" gained this rule; provider fallback is unchanged.
- The coordinator's provider allowlist shrank, so the reviewed coordinator digest in `tools/repair-unbound.mjs` advanced to `7f40de7c…f45a`; the repair helper refuses the deployed Worker until this change is deployed.
- Oliver-authorized judge change: the `fallback` promise now covers this case (`codex` golem at the tester's provider runs with `pi` on the tester's seat), because without Copilot no provider is left that the tester has no seat at and the scenario could still target; `accept.nu` additionally asserts `harness: pi` and `seat: <login>/<tester provider>`.
  It lands as its own `fix(accept)` commit with the red evidence above; the live check stays red until the new runner is pinned and the router deployed.
- Remaining gap: `pi` on `openai-codex` is not runnable yet, so a `claude` or `pi` golem paid by a Codex-only person is still refused with `no-seat`.

## 2026-11-12: Green for the reworded fallback check

- Red (old Worker): probe issue 8 comment `6822171957`, golem `harness: codex, provider: anthropic`, answered `<!-- isp-golem-xiv kind=refused reason=no-seat -->`.
- Green (`696aafb`, runtime v8): full Slice 3 acceptance 44/44; `fallback` check `131212249423` replied with `harness: pi`, `seat: bugabinga/anthropic` and `fallback: asked for anthropic/codex, used anthropic/pi with its default model, because codex does not work with anthropic`.
- Oliver chose "harness only"; it is exercised with the tester's own seat and a golem naming `provider: anthropic`, not a dedicated member without a Codex seat.

## 2026-11-11: Seat registration page and refreshing Codex seats

- Sign-in uses the App's user authorization: the callback checks the state cookie in constant time, exchanges the code with `APP_CLIENT_SECRET` (Worker secret, from the `production` environment), reads `/user` once, requires active membership, and sets a one-hour `golem_session` cookie MAC'd with a key derived from the client secret.
  Writes require an exact same-origin `Origin`, the session and an `X-Golem-Csrf` HMAC; membership is rechecked on every write.
- The browser seals credentials with a libsodium-compatible sealed box against the ops repository's Actions public key; the router only relays `encrypted_value` to `SEAT_<LOGIN>__<PROVIDER>`, so it never sees a credential; GitHub replaces an existing secret, which is how registering again replaces a seat.
  Vendored TweetNaCl.js 1.0.3 (Unlicense) and blakejs 1.2.1 (MIT) in `router/src/vendor/`, byte-identical to their npm tarballs; cargo-deny does not see them.
  Strict CSP, no inline script; the preferred-seat variable only accepts a provider the person has.
- OpenAI Codex seats run on the pinned Codex CLI (`codex exec --json --output-schema`, ephemeral, user config ignored) with a private per-run `CODEX_HOME` holding only the seat's `auth.json`, mounted outside `/workspace`.
  The model shown is the requested one or `default`; Codex's stream does not report it.
- After confirmed container destruction and before target publication credentials, a refreshed `auth.json` (same account, newer `last_refresh`, valid tokens) is sealed with `dryoc` 2.0.0 and written back with an ops-only `secrets: write` token; failures are noted in summaries but never fail the run.
  `dryoc` avoids duplicate crates `crypto_box` would add; a libsodium ciphertext test anchors compatibility.
- Known gaps: `golem_session` is not a `__Host-` cookie, so a sibling workers.dev subdomain could plant one (MAC still prevents forgery); the agent can read and alter the mounted `auth.json` like Claude's token.
- Rollout: runner v7 (release `43067406314`) was pinned in ops PR #13 before the router began dispatching `openai-codex`/`codex` choices, respecting the ordering lesson from v6.

## 2026-11-11: Seat fallback runs on a `pi` harness

- Dispatch uses `choose_seat` with the payer's seats and preferred seat; it dispatches only choices the runner can execute (Anthropic with `claude` or `pi`) and otherwise refuses mentions with no-seat or skips unattended work, never moving cost to another payer.
  Requests carry the chosen provider, harness and model, plus an optional bounded `fallback` text that every check summary, reply and pull request repeats as `fallback: ...`; requests without fallback stay byte-identical.
  The runner recomputes the choice from the pinned definition and the paying seat and rejects any mismatch.
- The agent image adds Pi `1.5.1` (pinned) with `ripgrep`/`fd`, run in JSON mode with sessions, extensions, skills, context files and MCP disabled except a baked `golem_report` extension; the seat token is passed as `ANTHROPIC_OAUTH_TOKEN` only to Pi.
  The typed result is exactly one successful `golem_report` call made last, parsed from Pi's events; missing, duplicate or malformed reports, unsettled or aborted runs fail closed, because Pi's JSON mode exits 0 even on provider errors.
- Pi runs only on writable (version 2) requests; read-only v1 stays on `claude`.
- Known gaps: Pi has no turn limit (timeout bounds it); the seat token is visible to Pi's bash tool like Claude's, guarded only by output scanning; coordinator-written stopped/blocked summaries lack the fallback line; help may show choices the runner cannot yet run; codex and Copilot providers still unsupported.
  Live run `42970383464` showed Pi authenticating with the Anthropic setup token, calling `golem_report`, and observing `claude-opus-5-5`.
- Rollout order matters: a Worker that emits new request fields must not deploy before the runner release accepting them is pinned; PR #24 briefly violated this until v6 was pinned (fail-closed at `--validate`, no seat cost).

## 2026-11-11: Onboarding and dashboard on push

- Pushes by people to an allowlisted repository trigger upkeep after event claiming: without `.golems/` at the default-branch head, the App opens **Enable golems** from branch `golems/enable` with exactly `.golems/_golems.md` (house rules) and `.golems/hello.md`; with `.golems/`, it keeps the **Golems** dashboard current.
- Onboarding happens once per repository: an ops tag `golem-onboarding/<repository id>` is claimed before any write, so closing the pull request is respected; a failure after the claim is not retried and needs manual cleanup (accepted trade-off).
- The dashboard is found by App author, title and a hidden marker; it is created and pinned once, and patched only when its rendered body changes.
  Sections follow SPEC: Golems (purpose, wakes, who pays, how to start), Disabled with reasons, Talk to golems, Seats.
  Seats are listed only in private repositories, from ops secret names matched to current members; unmatched seat names are reported as people who left; values are never read.
- Open gaps: last outcome per golem and updates after every run, and dashboard run boxes, are not implemented yet; concurrent first pushes could create two dashboards.
- Upkeep failures are logged and never drop event golems.

## 2026-11-11: Pull-request validation check

- `pull_request` opened/synchronize/reopened events whose files touch `.golems/` (or fill a whole files page) get a completed `golems/validate` check at the head commit, built from the same index parser the router uses; any invalid golem fails it and the summary lists each golem's reasons.
- Validation runs before trigger matching and its failure is logged, never allowed to drop matching event golems; App-authored events stay ignored, so the onboarding pull request is not validated by this path.

## 2026-11-11: Help and seat commands; harness/provider are preferences

- A bare mention or `@isp-golem-xiv help` replies with the help marker, every enabled golem and the seat, harness and model it would use for the asker, disabled golems, the commands, and the asker's seats with the registration link.
  `@isp-golem-xiv seat` replies with only the asker's own seats (secret names, never values), their preferred seat if set, and the registration link.
  Neither dispatches anything; the earlier unknown-golem refusal for a bare mention is replaced by help as the Slice 3 `help` promise requires.
- Seat choice is one pure function used by help now and by dispatch later: golem provider if the holder has that seat, else the preferred seat variable, else the first registered seat in contract provider order; the harness stays where it works, else becomes `pi`, with that harness's default model; a fallback names what was asked, what was used, and why.
- Validation no longer disables incompatible harness/provider pairs such as `codex` with `github-copilot`: SPEC calls them preferences resolved at run time, and the contract's fallback scenario requires such a golem to run.
  Unknown harness or provider values stay invalid.
  Dispatch still refuses golems not on `anthropic`/`claude` until the runner supports `pi` and other providers.
- `/seats` and `/admin` without a session redirect to GitHub's App user authorization with the public client id, the contract callback, and a fresh `crypto.randomUUID` state mirrored in a ten-minute `HttpOnly; Secure; SameSite=Lax` cookie scoped to `/auth/github`; the callback and sessions follow once the callback URL is registered.

## 2026-11-06: Stop fence for dispatched rows that never bound

- An explicit stop on a dispatched row with null run/attempt now retires it, without associating any workflow run.
  Every v2 grant follows a persisted bind on a non-terminal row, and a stopped row never grants; terminal rows reject late binds with 409.
  Null run/attempt in the authoritative journal therefore proves no agent admission; it does not claim no workflow activity.
- Retirement waits 15 minutes after the durable stop timestamp and requires no active runner run with the request's display title.
  An active same-title run may still bind, receive `stop_requested`, and be cancelled through the existing path, preserving stop-cancels-runner behaviour.
  Cancellation is best effort: a run appearing after retirement gets 409 on bind and self-denies without agent admission, while `legacyActive` pauses new dispatches until it ends.
  Malformed or overflowing run listings, missing timestamps, or inconsistent bind identity retain ownership; legacy stopped rows get a fresh grace.
  The cancelled check states `reason: stopped before runner admission; no agent execution was granted`; its log line carries no run id, and no model or audit is invented.
- Unstopped unbound dispatches are never retired by this path; repair association remains limited to unbound `dispatching` rows.
  Same-title active runs consume up to five listing calls per affected row per tick; the fence is skipped once an operation has used 36 of its 45 API calls.
  Known gap, unchanged class: a stopped row whose target repository is gone keeps failing its check update and stays retained, blocking its seat.
- Runner artifact GETs retry curl-transient failures (408/429/5xx/timeouts) for at most 180 seconds per file with 60-second attempts, inside a ten-minute step bound; checksum verification is unchanged.
  Connection-refused, reset and DNS failures are not retried.
  Historical v2/v3 workflow digests stay pinned; tests reconstruct them by removing exactly these additions.
- Repair helper's reviewed coordinator digest advances to the fenced coordinator; its association semantics for `dispatching` rows are unchanged.

## 2026-11-05: Startup download failure without run-linked inputs

- V5 first retained changer passed actual publication/lifecycle audit; continuation remains unbound.
  Matching-title workflow download failed HTTP504 before credential selection or native execution, but title/timing/uniqueness do not prove the complete dispatched request.
  Inspected complete job/run logs have zero request envelopes, no artifacts or API inputs were found; archive scratch removed.
  Original request/check/generation and reservation remain unchanged, echo unexecuted, no retry/association/manual terminalization.
- Future prevention should bound only artifact GET retries and record authenticated full-input identity before download, without retrying an agent or weakening checksum/TLS checks.
  Such records cannot be backfilled for this workflow.
- Conditional design investigation found a possible distinct pre-admission cancellation path: persisted bind identity precedes every v2 grant, while a durable stop fence denies late grants.
  This is not implemented or approved and does not authorize current-state mutation.
  Prerequisites include monotonic no-prior-bind provenance, null run/attempt, exact CAS/readback fence, eligibility recheck after races, durable original-tag receipt, truthful no-admission audit and retention on unknown/bound states.
  Existing destruction/completion and legacy-active barriers remain; it would not prove cancellation of an unidentified running workflow or satisfy that acceptance case.

## 2026-11-05: Event context, delegated commits, separate feedback authority

- Unchanged Slice 2 acceptance failed changer and echo while protected-path, serial, stop and issue-opened no-change passed.
  Historical request inspection confirmed changer instructions preserved but echo's triggering comment body absent.
  Both runs reported validated typed blocked with observed model and all lifecycle phases; no startup/schema/publication failure is inferred from that terminal path.
- Candidate event preparation copies signed comment text into existing bounded instructions without truncation; event task text remains untrusted, not authority to bypass runner restrictions.
  Native feedback eligibility stays separate from `human()` authorization and payer rules.
  Human event-comment feedback requires API-bound comment ID, actor, issue and User type; owner fallback validates actor membership independently, bots and absent issue/comment context remain silent.
- Writable prompt explicitly delegates commits/push/PR creation to trusted host after verification.
  This addresses a plausible changer conflict; its model's actual refusal reasoning was not retained, so behavioral resolution requires the unchanged task on the candidate.
- A comment lookup may fail after initial successful validation.
  Candidate attempts a blocked check retaining the observed model, prohibits change publication and does not release ownership while feedback validation remains uncertain.
  Regression exercises initial success followed by lookup failures through the destruction-guard callback; it is a unit boundary, not real Docker/coordinator proof.
- Duplicate queued echo check with the same delivery is a separate unresolved creation/journal gap.
  It does not establish duplicate execution; preserve identities, no deletion/reset/manual terminalization.
- Local full CI twice reset the Windows Miniflare connection at unchanged oversized-body fixture before event context tests.
  The same built modules and all four unmodified runtime harness stages subsequently passed once without rebuilding; hashes remained equal.
  This is diagnostic timing evidence, not full-CI success or proof of exclusive Windows scope; no bounds, assertions, test selection or timeouts changed.

## 2026-11-04: Protected-path refusal needs an explicit disposition

- Retained reproduction on native v3 produced replied/success for a correctly refused protected-path request.
  Check `126651646306`, run `41852382351`, source comment `6633850119`; original protected file unchanged, no owned branch or PR.
  This establishes outcome misclassification, not a filesystem guard bypass.
  Private response content is not copied into public evidence or test fixtures.
- Writable results now request a strict completed/blocked disposition and bounded message from the pinned harness.
  Native parsing validates the structure; ordinary prose, missing structure and malformed terminal streams never infer a successful task.
  Typed blocked results cannot publish changes, and known observed model attribution survives post-agent validation failures.
  Read-only behavior and bounded timeout partial-publication semantics remain separate.
- Synthetic regressions and pinned CLI feature inspection establish handling and feature presence only.
  Actual Linux/provider controls must show both protected refusal and allowed work before production rollout; fail-closed behavior alone could hide incompatibility.
  A dedicated opt-in test uses actual containers/provider with generic fixtures, then asserts mock publication payloads.
  Real GitHub publication, retained-scenario rerun and unchanged full acceptance remain separate required proofs.
- The protected case later passed against the actual provider, while allowed work reported completed with correct count/path/mode but wrong exact bytes.
  No private contents were disclosed; the exact content mismatch remains unknown.
  Oliver approved a deterministic printf command as the control's artifact producer rather than prose-directed punctuation.
  Only control instructions change; byte assertions, verification, protection and publication criteria remain intact, with no judge or production-code change.
- Review found duplicate timeout terminal records, lost model attribution after validation failure, and incomplete positive-control payload assertions; candidate corrections address each.
  Fresh full CI passed after one unchanged retry of a Windows Miniflare ECONNRESET failure.

## 2026-11-04: Full acceptance failure and shared-branch journal fencing

- The real v3 outcome proof process exited 0, including bot-owner, timeout publication/continuation and actual Cloudflare scheduled owner execution.
  Full unchanged Slice 2 acceptance then exited 1: all static/evidence and Slice 1 checks, changed PR and continuation passed; forbidden audit lacked blocked outcome, serial timed out, stop never dispatched, event setup returned GitHub HTTP 500 and its checks were unexecuted.
- Failed native v3 run `41802954891` reported static `queue conflict`, leaving request `eec9cfc0-d82c-11f1-97dc-9f61ec87b7e3` unbound dispatching and two other requests pending.
  The judge's cleanup made the target unavailable; do not discard requests or infer the forbidden task's actual outcome from a missing check.
- An isolated marked private repository reproduced five concurrent Contents PUT pairs to different files on one branch.
  Every pair produced one HTTP 409 reference conflict while the losing file blob remained unchanged; owned repository deletion/readback confirmed cleanup.
  This establishes the API conflict mechanism affecting queue and cursor writers sharing `golem-state`, not the historical rejection's exact predicate.
- Queue saves now freeze expected blob SHA and intended bytes, confirm exact committed readback, retry only unchanged-preimage branch conflicts within bounds, and reject divergent or malformed state without merging or granting execution.
  Missing run metadata is transient; ownership mismatches remain denied.
  No blind dispatch or agent-execution retry.
- A correlated completed failed run with both check and target unavailable can persist an immutable private blocked/unconfirmed audit before terminalization; retirement preserves its discoverable audit ref.
  Existing target with missing check, ambiguous API failures, wrong identity/attempt or failed audit persistence retain ownership.
  Audit reuse validates tag name, SHA, subject type/SHA and full message.
- Known v3 maintenance association adds exact release routing, asset, source tag, build and timestamp checks under the declared trusted GitHub/CI boundary.
  Both legitimate request environment records in the pinned workflow are required and deep-equal to the immutable request; extra records fail closed.
  Read-only inspection passed; apply still requires corrected live Coordinator source and unchanged pre-CAS deployment proof.
- Full CI initially hit the unchanged 60-second Coordinator suite wrapper after added cases.
  Four independent fixture batches preserve isolated SQLite stores, real restarts and all assertions; parent fresh full CI then exited 0, with no wrapper, limit or selection changes.
  Different-model security review closed findings within three calls of its finite budget.
- Forbidden behavior still needs a retained live reproduction after seat recovery; no prompt safety guard, promise or judge check has been weakened.

## 2026-11-04: Released runtime repair and continued repository discovery

- PR #9 passed exact-head CI on `644c930` (`41787665840`), merged as `00d62520`, passed trunk CI `41788243519` and deployment `41788243140`, and published `v3` (`41788750529`).
  Private ops PR #3 passed CI on exact head `87e5bff` (`41791328946`), merged as `8aca099`, then runtime workflow byte-matched the reviewed `v3` pin.
- Guarded operator association executed only after corrected deployment, producing candidate evidence tag `31a8daa0569f0ccc8c6e67587c03d384a41a6916`.
  Normal reconciliation produced blocked audit and terminal retirement for failed scheduled request `f37f6672-d908-4ffa-6488-c011eb2f94c7`, check `126391910880`, run `41774622060`.
  Read-only observer exited 0 against matching bound/terminal snapshots, immutable request and receipt, with current row absent.
  Provider execution is unproven; never count this cleanup as schedule-owner evidence.
- Fresh valid outcome sandbox `isp-golem-xiv-test-outcomes-mw1sksn2-4472a81e` is running real bot, timeout, continuation and cron proofs with released `v3`; no terminal proof result yet.
- Live scheduler cursor had completed 38 repositories and set done for the UTC day.
  Its early return prevented newly added repositories from discovery until tomorrow, so resume bounded metadata sweeps while preserving completed IDs and daily claims.
  Do not rescan definitions of completed repositories or reset prepared work; that separate limitation remains explicit.
- New built-Worker regression was red before the correction and green afterward, with persisted restart, same-day late repository admission and preserved request bounds.
  Raw enqueue attempts are recorded before rejection/deduplication; old occurrence counts must not increase during rescans, while the late occurrence requires exactly one attempt and one check.
  This preserves legitimate initial 429 retries rather than pretending every occurrence always admitted on its first attempt.
- A substituted different-model reviewer found no scoped blocker after the raw-attempt assertion fix.
  Historical maintenance fixture reconstructs exact v2 workflow bytes from the current v3 URL and still checks the full pinned v2 digest; production historical validation is unchanged.
  Latest fresh full CI passed; no judge selection, limit, promise or gate was changed.

## 2026-11-04: Local PAX paths and failed startup recovery

- Original orphan recovery was independently observed after deployment: original check `126334264637`, run `41772599688`, exact original prepared identity, receipts, bound-before-terminal history, retirement and ordered native phases.
  Its outcome is replied, not fresh publication; see `EVIDENCE.md`.
- Real outcomes proof process stopped with no accepted outcome evidence.
  Partial run `41774486415` blocked before provider execution because the actual codeload archive contains a local PAX `path` record.
  Scheduled run `41774622060` failed before admission with the old generic queue denial; its HTTP cause remains unidentified and its reservation is preserved.
  Bot fixture used scalar triggers although frontmatter requires a sequence; deployed index confirmed it disabled, so correct fixture syntax rather than bot filtering.
- The immutable private archive at commit `6707a8bbef9526cbdc467a287c1089454e164f7a`, compressed SHA-256 `2d4de7a25756c7703182a7c14f44b557a8043e9c525aeb0c41f9d0f46ba17461`, reproduced the old exact extraction error.
  Local PAX support permits one bounded UTF-8 path record for the next real entry, no other overrides; existing path, link, root, size and collision guards remain.
  Review found inherited Win32 alias/device hazards; raw component checks now reject them before normalization or writes for ordinary and effective PAX paths.
  Parent executed the ignored private integration test explicitly: tracked digest and embedded commit checked before extraction, independent tar comparison passed, scratch archive removed.
- Native control requests now use bounded same-run retries for transport, 429 and 5xx only, with fresh signed timestamps; authorization, ownership and malformed grants remain fail-closed.
  This retries control communication, never agent execution, and does not retroactively identify the old denial's cause.
- Undispatched pending/reserved requests now preflight target identity; 404 records terminal blocked reason `target unavailable at dispatch preflight` privately.
  Do not claim deletion: GitHub also hides authorization failures as 404.
  Dispatched/bound work, 403, 5xx and identity mismatch retain their claims.
- A narrow operator helper can associate one proven completed failed workflow with its exact existing unbound reservation, never reset, redispatch or manually mark it completed.
  It requires exact immutable request, generation, workflow bytes, full trusted-step request/log evidence, unique run, known historical runner asset identity, candidate evidence and queue CAS; normal coordinator reconciliation then owns blocked auditing and release.
  Apply additionally requires the reviewed Coordinator source at the exact live Worker health version, before evidence writes and again immediately before CAS.
  Version drift, source drift and health failure on the second check all prove zero queue PUTs, retained dispatching state and no applied evidence.
  Actual read-only inspection passed; no remote apply has occurred.
- Release `v2` is not an immutable release container.
  Historical binary identity is pinned by asset `707806211`, its exact GitHub digest, unchanged creation/update times preceding the failed run, uploader, annotated source tag and successful release-build metadata.
  This uses trusted GitHub metadata and CI, not an independent cryptographic build attestation.
- Security review completed within four calls and the 60-minute bound, closing path aliases, deployment gating, historical asset identity, default branch, tracked archive identifiers and lifecycle detection findings.
  Fresh final Node repair suite and full `mise run ci` exited 0.
  Earlier local simulator resets and a regex-stem typo were not hidden with gate changes, ignores or raised limits.
- These corrections remain local until protected PR CI, deployment and a new released runner succeed.
  Schedule-owner, bot-owner, incomplete/continuation and full Slice 2 acceptance are still unverified.

## 2026-11-04: Cold-start bootstrap and orphan recovery correction

- A controlled Contents API write naming a nonexistent private-ops branch returned HTTP 404 (`Branch ... not found`), without creating a file.
  The actual canary's prepared source predates `golem-state` initialization; the old mention path attempted continuation writes without establishing that branch.
- V2 processing now verifies the state branch before consuming its delivery claim and invokes the coordinator's atomic bootstrap when absent.
  Legacy routing returns before this new precondition.
  Strict mock GitHub Contents semantics reproduced the old cold-start failure before the correction.
- Cron discovers at most one orphan source per tick from at most 512 pending source refs, preserving its exact prepared request and check.
  Event-index rotation prevents one failing indexed head from indefinitely starving healthy peers.
  Completion receipts retain original annotated-source identity before removing index/cursor/pending-ref state; the immutable source remains reachable through that receipt.
  Existing completed sources are safely re-admitted through coordinator idempotence, not dispatched again.
- Real built edge + actual SQLite tests cover first bootstrap, orphan restart with no recreated checks, legacy completed-source dedup, five interrupted retirement boundaries and blocked-head fairness across restart.
  Existing multi-event +36 recovery and scheduled-call bounds remain asserted.
  Full 512/513-source and full-index-capacity matrices remain unverified.
- Fresh full CI initially caught an overlong `process` function.
  Removed duplicated reaction/membership/stop handling already implemented by `process_mention`, passing repository metadata so authorized normal requests look up head only after stop handling.
  No lint suppression or gate relaxation.
- Subsequent fresh full `mise run ci` exited 0; focused different-model review found no scoped correctness/security/regression blocker.
  A second scoped review covered both operational proof helpers and the already-applied ops `v2` pin without finding a publication blocker.
  Outcomes polling tolerates a missing immutable request tag only while its check is non-completed; completed checks remain strict.
  Current-day schedule freshness uses the actual Actions run's `created_at`, never a nonexistent CheckRun field.
  The pilot observer additionally checks golem/harness, workflow path and exact journal tag/resource/seat; the real changed observation passed again.
  Original live orphan remains unresolved until this correction is deployed and independently observed serving it.

## 2026-11-04: Live canary identities and cold-start admission gap

- Guarded Slice 2 PR #7 merged only after exact-head CI passed on `0bf104d`; trunk `0a9555f` passed CI, deployed, and published `v2`.
  Private ops PR #2 merged with green CI after release publication; its runtime workflow was byte-compared with the `v2` pin.
- First observer incorrectly filtered on nonexistent CheckRun `created_at`; GitHub's default latest-name filter also hides older same-name checks.
  The corrected read-only observer binds an explicit check to immutable request/comment identity, uses actual workflow timestamps for bounded lifecycle history and fresh-commit causality, and preserves all changed-outcome, content, audit, queue and phase assertions.
- Delivery `7d8caf40-d7f4-11f1-9909-46c276fd737e` is the fresh-publication proof: check `126340791321`, run `41758683949`, new commit `318d77b` during that invocation, matching immutable request/terminal receipt and bound-before-terminal history.
  Trusted native logs place confirmed destruction before publication credentials, publication completion before reservation release.
- Later delivery `7e3b7af0-d7f6-11f1-8d54-9388c7d01961` completed as replied against existing content; its successful run is not evidence of a fresh commit.
  Original delivery `f6f9c410-d7f1-11f1-8c7e-16908e2f3f3a` remains queued with an immutable prepared source but no request tag, receipt, queue row or run.
  All three were independently correlated through their source comments/checks; later success does not supersede the original request.
- The source of the unresolved request was written at `01:49:30Z`, before `golem-state` initialization at `01:50:11Z`.
  Mention persistence does not establish the coordinator journal before writing its continuation, unlike the event/schedule paths.
  This cold-start path and the missing orphan-source discovery require a product correction; do not mark or discard the original check to obtain a pass.
- Canary routing was disabled again; full deployed Slice 1 regression passed with released runner `v2` (`41759292169`).
  The attributable tail observed a later request only; deletion was verified independently and no tail result is treated as provider/publication proof.
- Real schedule-owner, bot-owner, incomplete/continuation and full Slice 2 acceptance remain unproven.
  Auxiliary coverage work is an isolated measured proposal, not a CI ratchet yet.

## 2026-11-04: Guarded Slice 2 implementation, live proof pending

- Version 2 routing is opt-in via private ops variable `GOLEM_PROTOCOL_VERSION=2`.
  Absence preserves the released mention path; enable only after publishing and pinning the backward-compatible runner.
- One SQLite Durable Object arbitrates seat and repository/golem resources together; queued requests do not consume Actions runners.
  Private ops `golem-state:queue.json` and immutable request tags are authoritative; SQLite is a rebuildable projection.
  Accepted pending records are never evicted; capacity is explicitly 32 current records.
  Dispatch ambiguity retains its reservation instead of risking a second charge.
- Signed native callbacks use exact-body HMAC-SHA256 with normalized App PEM, freshness checking, generation and numeric safe-integer run/attempt identity.
  Parent integration found string/number mismatch between native producer and JS consumer; wire producer corrected and regression added.
  Only trusted supervisor may attest all containers destroyed and publication complete before completion releases resources.
- Writable execution, target mise preparation and verification remain in isolated containers without App/GitHub credentials.
  Native publisher checks bounded filesystem changes and forbidden paths, continues the owned branch/PR, publishes through scoped Git Data APIs, and keeps attribution observable.
  Host runner reuses the pure router parser through an explicitly versioned path dependency; no second frontmatter parser.
- Independent different-model review found three concrete defects: scheduled claims could strand rejected admissions, stop cancelled multiple/unattended runs, and terminal-run reconciliation could release resources without final audit.
  Corrections retain exact prepared schedule/check across restart, stop only the latest active mention (otherwise latest pending mention), and repair missing terminal audit before release.
  Native audit producer payloads were exercised against actual SQLite reconciliation with mocked GitHub; valid audit outcomes are preserved.
- V2 mention admissions now reuse durable prepared continuations after rejection, with person seat and mention trigger unchanged.
  Unindexed sources before successful index admission remain a separate open recovery gap, not a completed behavior.
- Consolidated rebuilt Worker tests initially passed, then exposed a new fixture-state error after adding mention recovery: its extra captured admission contaminated a later hard-coded queue count.
  Isolated the new case after existing scenarios, kept original assertions, waited for the actual rejected admission rather than merely a preparatory write, and asserted no premature rocket.
  The actual-edge/actual-Coordinator mock GitHub adapter then rejected newly required continuation endpoints; added bounded Contents CAS semantics, not bypassed production persistence.
- Final rebuilt Worker suite passes: released-v1 path, v2 owner/event/schedule continuation, actual Rust edge → actual SQLite admission/dispatch/signed bind/completion/rebuild/resource release, plus 25 coordinator scenarios.
  Subsequent fresh full `mise run ci` exited 0 on the corrected shared tree.
  These use mocked GitHub and simulated native callbacks; authenticated writable execution/publication and deployed v1 regression remain unproven.
- Latest native WSL Linux tests/strict Clippy include label handling and numeric queue-wire correction; image probes establish executable/parser behavior only.
  Scoped Slice 2 mutation testing remains unrun.
  Branch coverage ratchet is not fabricated: exact stable 1.99.1 rejects the required experimental branch-coverage option; toolchain/coverage decision remains open.
- Other open gaps: event-index admission capacity/CAS can leave undiscovered source tags, unbound ambiguous dispatch can remain reserved indefinitely, large/changing installations can exceed daily scan throughput, non-Linux hosted labels are not implemented.
  No claim of universal credential-exfiltration prevention: scanning blocks handled/recognized representations, not arbitrary covert encodings.
- Judge and configured acceptance commands remain unchanged.

## 2026-11-03: First authenticated-path acceptance fails before agent startup

- After successful release `v0` (`41700313107`), full `mise run accept -- --slice 1` passed all static promises plus index, eyes, dispatch, unknown-golem refusal.
  It failed reply (`no reply containing the nonce within 15min`) and audit (`summary lacks lines matching: outcome: replied`); no-loop was skipped, not observed failing.
- Private runner job `41702041018` failed before provider execution: `snapshot contains a link or special file`.
  This is an archive handling defect, not evidence of invalid subscription credentials.
- Actual codeload archive of the public code repository starts with `pax_global_header`, type `g`, containing `52 comment=<40-hex-commit>\n`.
  `tar` exposes this global header; the old extractor rejected it.
- Runner now inspects raw entries and permits only that first, exact-sized commit-comment record.
  Path/link/size overrides, malformed or oversized metadata, other extensions, links and devices remain rejected.
  Expanded-size accounting includes metadata, headers, padding and trailing bytes without retaining trailing data.
  Permanent representative regression was red with the deployed error, then green; extraction of the actual downloaded public code archive also passed.
  Temporary archive and probe artifacts were cleaned; no target source was placed in the public code repository.
- Actual immutable public archive differential proof (not the synthetic regression fixture): downloaded `https://codeload.github.com/isp-insoft-gmbh/isp-golem-xiv/tar.gz/8b13b0f1a33b10d9b26738002e6d697785a228d0`, 108774 compressed bytes, SHA-256 `2db21cc3ad4459c8ff563725aeda8b963547fe1b3b5a3e17eaff306024075369`.
  An OS-temp Rust driver included old source obtained by `git show 8b13b0f:runner/src/snapshot.rs` and current source as separate modules, linked existing locked dependency artifacts, and ran both on identical downloaded bytes.
  Old source SHA-256 `d0e0bdba034bd61351489c66d76ee8879eba6bc54a5428c72cb069fd7decf601` returned exactly `snapshot contains a link or special file`, extracting zero entries.
  Current source SHA-256 `6663793378db19160a3489767ee970d20ba9f817915d5b2adeabf13cad678b3c` extracted 54 files and 12 directories, 364219 payload bytes, with no metadata file.
  Extracted README.md (310 bytes, Git blob `1c7c7483c107e9f0cc97a746b3c17074140b7bf3`) and LICENSE (1072 bytes, Git blob `55ec06c8f9e9c69ea71755e61081340d5460103a`) exactly matched independent Contents API bytes at the same immutable commit; API blob hashes were recomputed.
  Identical assertions passed twice; temporary downloads, old/new source copies, driver, executable and extracted trees were removed.
- This still does not prove authenticated agent execution; a new released runner and three consecutive full acceptance passes remain required.

## 2026-11-03: Deployed Slice 1 preflight

- PR #4 merged only after required Linux `ci` succeeded on exact intended head `b18fa24` (run `41696918035`), including actual pinned image build and timeout entrypoint tests.
  Rebase merge preserved agent authors; trunk `8b13b0f` subsequently passed `ci` (`41697718686`) and deployed through CI (`41697718711`).
- All Slice 1 static checks now pass, including health/version, bad signature rejection, index auth.
  Earlier red observations for these static promises are retained above/below; live acceptance scenarios are still pending.
- `tools/prove-refusals.mjs` captured a real App delivery and sent signed actor simulations to the deployed Worker.
  Both no-seat and non-member refusals were independently read back; no matching runner workflows started.
  These are simulations, not actions performed by the named account holders; links and reproducible path are in `EVIDENCE.md`.
- Proof helper initially failed delivery-detail lookup because JSON numeric delivery IDs exceeded JS safe integer precision.
  Preserving their exact decimal strings corrected the URL; resumed the same repository rather than generating more test repositories.
- Release `v0` was tagged only after trunk CI passed and real distribution packaging had passed in PR CI; publishing remains CI-only.

## 2026-11-03: Container compatibility diagnosis

- Linux CI `41693321024` (`69a0b0c`) and `41695226429` (`14a8728`) passed `mise run ci` but failed the separate agent-container step; neither constitutes a green required `ci` check.
- Trusted preparation now exposes credential-free image build diagnostics; agent execution diagnostics remain suppressed and bounded.
  Second run identifies the final `--max-turns` help-text grep as the failing build operation.
- Exact Claude 2.1.317 Linux ELF (SHA-256 `eb224419e344575faa4a1f39b290b649425a10cd5ee2ef1d003480cd9ae57eb0`) was installed through mise into isolated WSL temp space.
  In a fresh HOME with cleared environment and network namespace disabled: unknown option rejected; invalid `--max-turns` value rejected with numeric parser error; valid value reached the not-logged-in error.
  Binary option registration also contains `--max-turns <turns>`; ordinary help omits it.
- Dockerfile replaces the false help-text assumption with the same positive/negative parser probes under `RUN --network=none`, fresh HOME, cleared env, hard timeout.
  This proves parser compatibility only, not authenticated turn-limit enforcement.
  Existing image-build/watchdog test and acceptance gates are unchanged.
  WSL probe block passed twice; temporary 638 MiB install cleaned.
- Intermittent local oversized-request ECONNRESET reproduced with a trivial early-413 Worker, both Miniflare dispatchFetch and direct HTTP, independent of Rust code.
  Fixed 20-trial conditions included 262186-byte and 2097194-byte JSON and chunked bodies; early-response variants failed transport requests, while all following requests and valid-body controls returned 200.
  Bounded reading eliminated resets only for the near-limit body, not larger/chunked uploads; no arbitrary draining, retries, assertion removal, or dependency change made to manufacture a pass.
  Linux full CI has passed this unchanged test on both completed runs; Windows transport nondeterminism remains recorded.

## 2026-11-03: Slice 1 implementation

- Rust workspace separates host-tested router policy, wasm edge I/O, trusted native runner, stdlib-only xtask.
- Router reads the index fresh rather than retaining a cache; bounded to 20 golems of 64 KiB each.
- Atomic create-only GitHub refs in private ops claim delivery IDs, with payload fingerprints; no eventually consistent KV lock.
  This prevents duplicate dispatch attempts, not transactional delivery guarantees across GitHub APIs.
  Failed or ambiguous deliveries retain claims; absent rocket does not establish dispatch failure.
  Reconciliation requires checking delivery, check external ID, and ops runs before any operator action; workflow reruns are rejected before credential access.
- Runner snapshots the exact dispatch SHA, runs Claude in an unprivileged read-only container with only read tools, subscription token, target snapshot, bounded resources, independent timeout entrypoint.
  No GitHub token, App key, host home, Docker socket, or workflow control files reach the container.
  Confirmed container destruction precedes fresh scoped publication credentials.
  Output scanning is defense in depth, not a claim of arbitrary covert-channel prevention.
- Direct Workers REST multipart upload supports Rust SDK build artifacts without a second CLI build format; production deployment happens only in CI after validation.
- Dependencies: serde/serde_json for bounded protocol I/O; serde-saphyr for strict bounded YAML; hmac/sha2 for constant-time webhook signatures and fingerprints; worker/wasm-bindgen/futures-util/base64 for Workers runtime APIs, native crypto bridge, cancellation, contents decoding.
  Runner uses reqwest/tokio for bounded async HTTP/process I/O, jsonwebtoken for native App JWTs, flate2/tar for validated snapshots, tempfile for automatically cleaned workspaces, base64 for secret scanning.
- Global duplicate-dependency denial remains; exact-version exceptions in `deny.toml` cover incompatible upstream base64/getrandom/syn/windows-sys generations.
  CDLA-Permissive-2.0 allowed only for webpki-roots 1.0.11; its license ships in releases.
- Local `mise run ci` passed after multiline/rerun hardening and again after container-test wiring: 27 Rust tests, wasm Clippy/build, built Worker workerd tests, packaging, deploy transport tests, all linters, original judge parser checks.
  Docker is unavailable locally; Linux CI must build the actual pinned agent image and exercise its timeout entrypoint before integration is accepted.
- Scoped mutation evidence: router 3 selected mention mutants caught (~49s); xtask 12/12 selected mutants caught after boundary-test improvements (38.49s wall, 0.31s driver CPU, sampled peak scratch 17.6 MB).
  Compiler/test CPU and router peak disk were not captured; no complete resource-accounting claim.
- Independent review by gpt-5.6-terra found no demonstrated auth/secret exploit; live deployment and Docker execution remain verification gaps.
- Initial preflight red observations: ops repo absent, router health 404; workflow, seat, signature/auth checks blocked by these; evidence absent.
  Initial full gate also skipped live checks because static setup failed.
  These skipped checks are not claimed as live red observations; retain actual later acceptance failures before marking their behavior proven.
- Judge changes: none (`accept.nu`, `contract.nuon`, `tools/ci.nu` unchanged).

## 2026-11-03: Hardening before handoff

Oliver's decisions after four independent reviews of the judge, the spec and plan, the development workflow, and the agent's autonomy.

- **Promises for all slices are written before handoff.** Otherwise `--slice 3` passed on Slice 1 checks alone, and the agent would have written promises it then checks itself.
- **The App installation covers all repositories.** Adding repositories to a selected installation needs a token gh does not have; the topic allowlist already limits what the router acts on.
- **The admin dashboard is Slice 4 with its own promises.** It stays a goal but no longer competes with the core.
- **Anthropic seats may run under `pi`.** People remain responsible for their providers' terms.
- **Router worker name and URL are fixed in `contract.nuon`.** The App's OAuth callback URL is set by hand and must not change when the agent deploys. The account's `workers.dev` subdomain `isp-insoft-gmbh` was created for this through the Cloudflare API; it can be renamed in the Cloudflare dashboard.
- **Promise kinds prerequisite, check, evidence.** Prerequisites are exempt from red before green; behavior `accept.nu` cannot drive is proven in `EVIDENCE.md`.
- **End-to-end scenarios stay out of public CI.** They need seats and private test repositories.
- **Rust is pinned by `rust-toolchain.toml`, not mise.** mise's `RUSTUP_TOOLCHAIN` would override the pin; Clippy's `cargo` group is dropped because its findings are mostly in dependencies.
- **`stopped` is an outcome**, and `no_change` belongs to runs without a person to answer.
