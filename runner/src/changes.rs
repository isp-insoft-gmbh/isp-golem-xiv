use crate::{OutputGuard, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{collections::BTreeMap, fs, io::Read as _, path::Path};

const ENTRIES_MAX: usize = 20_000;
const DEPTH_MAX: usize = 32;

/// Byte bounds; the two values below are the only definitions of these caps.
#[derive(Clone, Copy)]
pub struct Limits {
    pub file_bytes: usize,
    pub total_bytes: usize,
}

/// Repository content: snapshot archives, baseline scans and published changes.
pub const SOURCE_LIMITS: Limits = Limits {
    file_bytes: 4 * 1024 * 1024,
    total_bytes: 64 * 1024 * 1024,
};
/// Workspace after agent or verifier ran: build outputs are inspected, never published as such.
pub const WORKSPACE_LIMITS: Limits = Limits {
    file_bytes: 64 * 1024 * 1024,
    total_bytes: 512 * 1024 * 1024,
};

/// A regular file, or a symbolic link whose `bytes` are its UTF-8 target. Links are recorded,
/// never followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub bytes: Vec<u8>,
    pub executable: bool,
    pub link: bool,
}

const LINK_TARGET_BYTES_MAX: usize = 1024;
/// Git archives store longer link targets in a PAX `linkpath` record, which snapshot
/// extraction does not accept; publishing only what extraction can read back keeps later runs
/// on the branch working.
const PUBLISHED_LINK_TARGET_BYTES_MAX: usize = 100;

/// Where a relative link at `path` points, resolved lexically against the repository root.
/// Rejects absolute, drive or backslash targets, `..` after a normal component (which could
/// leave the root through an intermediate link), and anything resolving to or above the root.
#[must_use]
pub fn link_target_inside(path: &str, target: &str) -> Option<String> {
    if target.is_empty()
        || target.len() > LINK_TARGET_BYTES_MAX
        || target.starts_with('/')
        || target.contains(['\\', ':'])
        || target.chars().any(char::is_control)
    {
        return None;
    }
    let mut resolved: Vec<&str> = path.split('/').collect();
    resolved.pop()?;
    let mut descending = false;
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if descending {
                    return None;
                }
                resolved.pop()?;
            }
            _ => {
                descending = true;
                resolved.push(part);
            }
        }
    }
    let joined = resolved.join("/");
    (!joined.is_empty() && valid_path(&joined)).then_some(joined)
}

/// Records a link's target without following it.
fn link_state(path: &Path) -> Result<FileState> {
    let target = fs::read_link(path).map_err(|_| "cannot read snapshot link")?;
    let target = target
        .to_str()
        .filter(|target| !target.is_empty() && target.len() <= LINK_TARGET_BYTES_MAX)
        .ok_or("invalid snapshot link target")?;
    Ok(FileState {
        bytes: target.as_bytes().to_vec(),
        executable: false,
        link: true,
    })
}

/// Whether a snapshot may contain this link: it stays inside the repository and does not itself
/// stand at a protected path. A protected link would move the guarded content to an unguarded
/// path (`.claude -> config` makes `config/settings.json` editable), so it is refused outright.
#[must_use]
pub fn snapshot_link_allowed(path: &str, target: &str) -> bool {
    !forbidden(path) && link_target_inside(path, target).is_some()
}

/// A link may be published only if it stays inside the repository and points nowhere a golem
/// may not change.
fn publishable_link(path: &str, target: &[u8]) -> bool {
    if target.len() > PUBLISHED_LINK_TARGET_BYTES_MAX {
        return false;
    }
    std::str::from_utf8(target)
        .ok()
        .and_then(|target| link_target_inside(path, target))
        .is_some_and(|resolved| !forbidden(&resolved))
}
pub type Files = BTreeMap<String, FileState>;

#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub file: Option<FileState>,
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && path.split('/').count() <= DEPTH_MAX
        && path.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase()
                        .as_str(),
                    "con"
                        | "prn"
                        | "aux"
                        | "nul"
                        | "com1"
                        | "com2"
                        | "com3"
                        | "com4"
                        | "com5"
                        | "com6"
                        | "com7"
                        | "com8"
                        | "com9"
                        | "lpt1"
                        | "lpt2"
                        | "lpt3"
                        | "lpt4"
                        | "lpt5"
                        | "lpt6"
                        | "lpt7"
                        | "lpt8"
                        | "lpt9"
                )
        })
}

fn forbidden(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let parts: Vec<_> = lower.split('/').collect();
    parts.iter().any(|part| {
        matches!(
            *part,
            ".golems"
                | ".git"
                | ".claude"
                | ".codex"
                | ".pi"
                | ".agents"
                | ".mcp.json"
                | ".claude.json"
                | "claude.md"
                | "agents.md"
        )
    }) || parts
        .windows(2)
        .any(|pair| pair == [".github", "workflows"])
}

/// Directories a harness owns for project configuration and its own state.
fn harness_area(path: &str) -> bool {
    path.to_ascii_lowercase()
        .split('/')
        .any(|part| matches!(part, ".claude" | ".codex" | ".pi" | ".agents"))
}

/// True when `path` is inside a harness directory and the snapshot had nothing at or below it:
/// state a harness created during the run (package installs, caches), never published.
fn new_harness_state(path: &str, baseline: &Files) -> bool {
    if !harness_area(path) || baseline.contains_key(path) {
        return false;
    }
    let prefix = format!("{path}/");
    !baseline
        .range(prefix.clone()..)
        .next()
        .is_some_and(|(existing, _)| existing.starts_with(&prefix))
}

/// # Errors
/// Rejects special files, hard links, nonportable paths, non-UTF-8 or oversized link targets, or
/// snapshots exceeding explicit limits. Symbolic links are recorded with their target and never
/// followed. Existing forbidden files are retained so changes, including deletion, can be
/// detected.
pub fn scan(root: &Path) -> Result<Files> {
    scan_with(root, None, SOURCE_LIMITS)
}

/// Scans the workspace after the agent or verifier against the snapshot, within
/// [`WORKSPACE_LIMITS`]: harness-created state under `.claude`, `.codex`, `.pi` or `.agents`
/// that did not exist before is skipped entirely (not counted, not published, links allowed),
/// while every file present in the snapshot is still compared.
///
/// # Errors
/// As [`scan`] for everything outside skipped harness state.
pub fn scan_workspace(root: &Path, baseline: &Files) -> Result<Files> {
    scan_with(root, Some(baseline), WORKSPACE_LIMITS)
}

fn scan_with(root: &Path, baseline: Option<&Files>, limits: Limits) -> Result<Files> {
    if !fs::symlink_metadata(root)
        .map_err(|_| "cannot inspect snapshot root")?
        .file_type()
        .is_dir()
    {
        return Err("snapshot root is not a directory");
    }
    let mut pending = vec![String::new()];
    let mut files = Files::new();
    let mut names = std::collections::BTreeSet::new();
    let mut entries = 0;
    let mut total = 0;
    while let Some(directory) = pending.pop() {
        for entry in
            fs::read_dir(root.join(&directory)).map_err(|_| "cannot read snapshot directory")?
        {
            entries += 1;
            if entries > ENTRIES_MAX {
                return Err("snapshot entry limit exceeded");
            }
            let entry = entry.map_err(|_| "cannot read snapshot entry")?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "non UTF-8 snapshot path")?;
            let path = if directory.is_empty() {
                name
            } else {
                format!("{directory}/{name}")
            };
            if baseline.is_some_and(|baseline| new_harness_state(&path, baseline)) {
                entries -= 1;
                continue;
            }
            if !valid_path(&path) || !names.insert(path.to_ascii_lowercase()) {
                return Err("invalid or colliding snapshot path");
            }
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| "cannot inspect snapshot entry")?;
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if metadata.is_symlink() {
                // The snapshot itself must never carry a protected link; see `snapshot_link_allowed`.
                if baseline.is_none() && forbidden(&path) {
                    return Err("snapshot contains a protected link");
                }
                let link = link_state(&entry.path())?;
                total += link.bytes.len();
                if total > limits.total_bytes {
                    return Err("snapshot byte limit exceeded");
                }
                files.insert(path, link);
                continue;
            }
            if !metadata.is_file() {
                return Err("snapshot contains special file");
            }
            if metadata.len() > limits.file_bytes as u64 {
                return Err("snapshot file limit exceeded");
            }
            let mut bytes = Vec::new();
            fs::File::open(entry.path())
                .map_err(|_| "cannot open snapshot file")?
                .take(limits.file_bytes as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "cannot read snapshot file")?;
            total += bytes.len();
            if bytes.len() > limits.file_bytes || total > limits.total_bytes {
                return Err("snapshot byte limit exceeded");
            }
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::MetadataExt as _;
                use std::os::unix::fs::PermissionsExt as _;
                if metadata.nlink() != 1 {
                    return Err("snapshot contains hard link");
                }
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            files.insert(
                path,
                FileState {
                    bytes,
                    executable,
                    link: false,
                },
            );
        }
    }
    Ok(files)
}

fn bounded(files: &Files, limits: Limits) -> Result<()> {
    if files.len() > ENTRIES_MAX {
        return Err("snapshot entry limit exceeded");
    }
    let mut total = 0usize;
    let mut names = std::collections::BTreeSet::new();
    for (path, file) in files {
        if !valid_path(path) || !names.insert(path.to_ascii_lowercase()) {
            return Err("invalid or colliding snapshot path");
        }
        if file.bytes.len() > limits.file_bytes {
            return Err("snapshot file limit exceeded");
        }
        total += file.bytes.len();
        if total > limits.total_bytes {
            return Err("snapshot byte limit exceeded");
        }
    }
    Ok(())
}

fn check_bytes(bytes: &[u8], guard: &OutputGuard) -> Result<()> {
    guard.check(&String::from_utf8_lossy(bytes))?;
    // Scan UTF-16 in both byte orders and alignments, including BOM-less binary containers.
    for offset in [0, 1] {
        for little in [false, true] {
            let words: Vec<_> = bytes
                .get(offset..)
                .unwrap_or_default()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[a, b]| {
                    if little {
                        u16::from_le_bytes([a, b])
                    } else {
                        u16::from_be_bytes([a, b])
                    }
                })
                .collect();
            guard.check(&String::from_utf16_lossy(&words))?;
        }
    }
    Ok(())
}

/// # Errors
/// Rejects forbidden edits/deletions, invalid snapshots, or credential-bearing publication content.
pub fn diff(before: &Files, after: &Files, guard: &OutputGuard) -> Result<Vec<Change>> {
    let paths = checked_paths(before, after, guard, SOURCE_LIMITS)?;
    if paths.iter().any(|path| {
        after
            .get(*path)
            .is_some_and(|file| file.link && !publishable_link(path, &file.bytes))
    }) {
        return Err("unsafe link target; publication blocked");
    }
    Ok(paths
        .into_iter()
        .map(|path| Change {
            path: path.clone(),
            file: after.get(path).cloned(),
        })
        .collect())
}

/// Checks every workspace change, publishable or not, without cloning disposable artifacts.
///
/// # Errors
/// Rejects oversized outputs, forbidden changes, invalid paths, or credential-bearing bytes.
pub fn validate_workspace(before: &Files, after: &Files, guard: &OutputGuard) -> Result<()> {
    checked_paths(before, after, guard, WORKSPACE_LIMITS).map(drop)
}

/// The publication candidate: `after` without new files Git would not track per the
/// baseline's `.gitignore` files. Baseline bytes decide, so edits during the run cannot
/// hide a file; files present in the snapshot are always kept, including their deletion.
///
/// # Errors
/// Rejects a kept file or total beyond [`SOURCE_LIMITS`] before copying it.
pub fn publishable(before: &Files, after: &Files) -> Result<Files> {
    let ignored = Ignored::from_baseline(before);
    let mut total = 0usize;
    let mut kept = Files::new();
    for (path, file) in after {
        if !before.contains_key(path) && ignored.ignores(path) {
            continue;
        }
        total += file.bytes.len();
        if file.bytes.len() > SOURCE_LIMITS.file_bytes {
            return Err("publication file limit exceeded");
        }
        if total > SOURCE_LIMITS.total_bytes {
            return Err("publication byte limit exceeded");
        }
        kept.insert(path.clone(), file.clone());
    }
    Ok(kept)
}

/// Matchers of the snapshot's `.gitignore` files, deepest directory first.
struct Ignored(Vec<(String, Gitignore)>);

impl Ignored {
    fn from_baseline(baseline: &Files) -> Self {
        let mut matchers = Vec::new();
        for (path, file) in baseline {
            let Some(directory) = path.strip_suffix(".gitignore") else {
                continue;
            };
            if !(directory.is_empty() || directory.ends_with('/')) {
                continue;
            }
            // Paths are matched relative to the directory, so the matcher root stays empty.
            let mut builder = GitignoreBuilder::new("");
            let text = String::from_utf8_lossy(&file.bytes);
            // Like Git, a leading UTF-8 byte order mark is not part of the first rule.
            for line in text.strip_prefix('\u{feff}').unwrap_or(&text).lines() {
                // An unparsable pattern ignores nothing: its files stay publication candidates.
                builder.add_line(None, line).ok();
            }
            if let Ok(matcher) = builder.build() {
                matchers.push((directory.to_owned(), matcher));
            }
        }
        matchers.sort_by_key(|(directory, _)| std::cmp::Reverse(directory.len()));
        Self(matchers)
    }

    /// Git's precedence: the deepest `.gitignore` with a matching rule decides.
    fn decides(&self, path: &str, is_dir: bool) -> Option<bool> {
        self.0.iter().find_map(|(directory, matcher)| {
            let relative = path.strip_prefix(directory.as_str())?;
            let decision = matcher.matched(relative, is_dir);
            (!decision.is_none()).then(|| decision.is_ignore())
        })
    }

    /// Git never re-includes a file below an excluded directory.
    fn ignores(&self, path: &str) -> bool {
        path.match_indices('/')
            .filter_map(|(end, _)| path.get(..end))
            .any(|parent| self.decides(parent, true) == Some(true))
            || self.decides(path, false) == Some(true)
    }
}

fn checked_paths<'a>(
    before: &'a Files,
    after: &'a Files,
    guard: &OutputGuard,
    limits: Limits,
) -> Result<Vec<&'a String>> {
    bounded(before, limits)?;
    bounded(after, limits)?;
    let paths: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let mut changes = Vec::new();
    for path in paths {
        if before.get(path) == after.get(path) {
            continue;
        }
        if forbidden(path) {
            return Err("forbidden path changed; publication blocked");
        }
        guard.check(path)?;
        if let Some(file) = after.get(path) {
            check_bytes(&file.bytes, guard)?;
        }
        changes.push(path);
    }
    Ok(changes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn file(bytes: &[u8]) -> FileState {
        FileState {
            bytes: bytes.to_vec(),
            executable: false,
            link: false,
        }
    }

    fn link(target: &str) -> FileState {
        FileState {
            bytes: target.as_bytes().to_vec(),
            executable: false,
            link: true,
        }
    }

    #[test]
    fn forbidden_files_may_exist_but_never_change_or_disappear() {
        for path in [
            ".golems/task.md",
            ".Git/config",
            "a/.CLAUDE/settings.json",
            "a/.codex/config.toml",
            "a/.pi/agent",
            ".mcp.json",
            "nested/.mcp.json",
            "nested/.claude.json",
            "a/AGENTS.md",
            "CLAUDE.md",
            "a/.github/WORKFLOWS/ci.yml",
        ] {
            let before = Files::from([(path.into(), file(b"existing"))]);
            assert_eq!(
                diff(&before, &before, &OutputGuard::default()).unwrap(),
                vec![]
            );
            assert!(
                diff(&before, &Files::new(), &OutputGuard::default()).is_err(),
                "{path}"
            );
            assert!(
                diff(&Files::new(), &before, &OutputGuard::default()).is_err(),
                "{path}"
            );
            let modified = Files::from([(path.into(), file(b"changed"))]);
            assert!(
                diff(&before, &modified, &OutputGuard::default()).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn new_harness_state_is_skipped_but_snapshot_config_and_other_forbidden_paths_stay_guarded() {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, bytes: &[u8]| {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, bytes).unwrap();
        };
        write(".pi/settings.json", b"{}");
        write(".claude/skills/a/SKILL.md", b"skill");
        write("src.txt", b"code");
        let before = scan(root.path()).unwrap();
        // Harness installs and caches in new subtrees: many files, ignored without limits.
        for i in 0..50 {
            write(&format!(".pi/npm/node_modules/pkg{i}/index.js"), b"x");
        }
        write(".claude/cache/state.json", b"{}");
        write("nested/.agents/tmp/x", b"y");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            "index.js",
            root.path().join(".pi/npm/node_modules/pkg0/link"),
        )
        .unwrap();
        write("src.txt", b"changed");
        let after = scan_workspace(root.path(), &before).unwrap();
        assert!(after.keys().all(|path| !path.starts_with(".pi/npm")
            && !path.starts_with(".claude/cache")
            && !path.starts_with("nested/.agents")));
        let changes = diff(&before, &after, &OutputGuard::default()).unwrap();
        assert_eq!(
            changes.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
            ["src.txt"]
        );
        // New files beside existing config are harness state too: skipped, never published.
        write(".claude/skills/a/extra.md", b"new");
        let after = scan_workspace(root.path(), &before).unwrap();
        assert!(!after.contains_key(".claude/skills/a/extra.md"));
        assert!(
            after.contains_key(".claude/skills/a/SKILL.md"),
            "existing config is still compared"
        );
        // Existing project config still may not change or disappear.
        write(".pi/settings.json", b"{\"x\":1}");
        assert!(
            diff(
                &before,
                &scan_workspace(root.path(), &before).unwrap(),
                &OutputGuard::default()
            )
            .is_err(),
            "changed project setting"
        );
        write(".pi/settings.json", b"{}");
        fs::remove_file(root.path().join(".claude/skills/a/SKILL.md")).unwrap();
        assert!(
            diff(
                &before,
                &scan_workspace(root.path(), &before).unwrap(),
                &OutputGuard::default()
            )
            .is_err(),
            "deleted project skill"
        );
        write(".claude/skills/a/SKILL.md", b"skill");
        // New files outside harness directories that are forbidden still block.
        for path in [
            ".golems/new.md",
            ".github/workflows/x.yml",
            "AGENTS.md",
            ".mcp.json",
        ] {
            write(path, b"new");
            assert!(
                diff(
                    &before,
                    &scan_workspace(root.path(), &before).unwrap(),
                    &OutputGuard::default()
                )
                .is_err(),
                "{path}"
            );
            fs::remove_file(root.path().join(path)).unwrap();
        }
    }

    #[test]
    fn changes_include_deletions_modes_and_binary_content_without_credentials() {
        let before = Files::from([("old".into(), file(b"old")), ("mode".into(), file(b"same"))]);
        let after = Files::from([
            ("new".into(), file(&[0, 255])),
            (
                "mode".into(),
                FileState {
                    bytes: b"same".to_vec(),
                    executable: true,
                    link: false,
                },
            ),
        ]);
        let changes = diff(&before, &after, &OutputGuard::default()).unwrap();
        assert_eq!(changes.len(), 3);
        assert_eq!(changes.last().unwrap().file, None);
        let mut guard = OutputGuard::default();
        guard.add("sensitive-test-secret");
        for bytes in [
            b"\0sensitive-test-secret\xff".to_vec(),
            "sensitive-test-secret"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
            "sensitive-test-secret"
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect(),
        ] {
            assert!(
                diff(
                    &Files::new(),
                    &Files::from([("binary".into(), file(&bytes))]),
                    &guard
                )
                .is_err()
            );
        }
    }

    #[test]
    fn scanner_retains_forbidden_files_and_rejects_size_and_case_collisions() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("AGENTS.md"), b"existing").unwrap();
        assert_eq!(scan(root.path()).unwrap().len(), 1);
        let large = fs::File::create(root.path().join("large")).unwrap();
        large.set_len(SOURCE_LIMITS.file_bytes as u64 + 1).unwrap();
        assert!(scan(root.path()).is_err());
        let collision = Files::from([("A".into(), file(b"a")), ("a".into(), file(b"b"))]);
        assert!(diff(&Files::new(), &collision, &OutputGuard::default()).is_err());
    }

    #[test]
    fn workspace_byte_bounds_do_not_expand_source_or_publication_bounds() {
        let root = tempfile::tempdir().unwrap();
        let artifact = fs::File::create(root.path().join("generated.jar")).unwrap();
        artifact.set_len(64 * 1024 * 1024).unwrap();
        assert!(scan(root.path()).is_err());
        let scanned = scan_workspace(root.path(), &Files::new()).unwrap();
        assert_eq!(
            scanned.get("generated.jar").unwrap().bytes.len(),
            64 * 1024 * 1024
        );
        assert!(validate_workspace(&Files::new(), &scanned, &OutputGuard::default()).is_ok());
        assert!(diff(&Files::new(), &scanned, &OutputGuard::default()).is_err());
        assert_eq!(
            publishable(&Files::new(), &scanned),
            Err("publication file limit exceeded")
        );
        drop(scanned);
        artifact.set_len(64 * 1024 * 1024 + 1).unwrap();
        assert_eq!(
            scan_workspace(root.path(), &Files::new()),
            Err("snapshot file limit exceeded")
        );
        assert_eq!(WORKSPACE_LIMITS.total_bytes, 512 * 1024 * 1024);
    }

    fn ignore_rules(rules: &[(&str, &str)]) -> Files {
        rules
            .iter()
            .map(|(path, text)| ((*path).to_owned(), file(text.as_bytes())))
            .collect()
    }

    #[test]
    fn new_files_ignored_by_the_baseline_are_never_publication_candidates() {
        let mut before = ignore_rules(&[
            (".gitignore", "output/\n*.log\n!keep.log\n"),
            ("sub/.gitignore", "!trace.log\ngen/\n"),
            ("output/tracked.txt", "tracked despite ignore"),
        ]);
        before.insert("src.txt".into(), file(b"code"));
        let mut after = before.clone();
        for path in [
            "output/deps/big.jar",
            "output/nested/x",
            "run.log",
            "sub/gen/a.class",
            "keep.log",
            "sub/trace.log",
            "new.txt",
        ] {
            after.insert(path.into(), file(b"generated"));
        }
        after.insert("output/tracked.txt".into(), file(b"edited"));
        after.remove("src.txt");
        let kept = publishable(&before, &after).unwrap();
        let changed: Vec<_> = diff(&before, &kept, &OutputGuard::default())
            .unwrap()
            .into_iter()
            .map(|change| change.path)
            .collect();
        // Whitelists (root and deeper) are published; tracked ignored files and deletions stay.
        assert_eq!(
            changed,
            [
                "keep.log",
                "new.txt",
                "output/tracked.txt",
                "src.txt",
                "sub/trace.log"
            ]
        );
    }

    #[test]
    fn ignore_rules_come_from_the_baseline_and_cannot_reinclude_below_excluded_directories() {
        let before = ignore_rules(&[(".gitignore", "output/\n"), ("output/.keep", "")]);
        let mut after = before.clone();
        // Run-time edits neither hide a new file nor re-include ignored output.
        after.insert(
            ".gitignore".into(),
            file(b"output/\n!output/x\nsecret.txt\n"),
        );
        after.insert("secret.txt".into(), file(b"agent change"));
        after.insert("output/x".into(), file(b"generated"));
        let kept = publishable(&before, &after).unwrap();
        assert!(kept.contains_key("secret.txt") && kept.contains_key(".gitignore"));
        assert!(!kept.contains_key("output/x"));
        let marked = ignore_rules(&[(".gitignore", "\u{feff}output/\n")]);
        let mut after = marked.clone();
        after.insert("output/x".into(), file(b"generated"));
        assert_eq!(publishable(&marked, &after).unwrap(), marked);
        let nested = ignore_rules(&[(".gitignore", "build/\n"), ("build/.gitignore", "!y\n")]);
        let mut after = nested.clone();
        after.insert("build/y".into(), file(b"generated"));
        assert!(
            !publishable(&nested, &after)
                .unwrap()
                .contains_key("build/y")
        );
    }

    #[test]
    fn ignored_workspace_output_is_still_validated() {
        let before = ignore_rules(&[(".gitignore", "output/\n")]);
        let mut guard = OutputGuard::default();
        guard.add("sensitive-test-secret");
        for (path, bytes) in [
            ("output/report.txt", b"sensitive-test-secret".as_slice()),
            ("output/AGENTS.md", b"generated".as_slice()),
        ] {
            let mut after = before.clone();
            after.insert(path.into(), file(bytes));
            assert_eq!(publishable(&before, &after).unwrap(), before, "{path}");
            assert!(
                validate_workspace(&before, &after, &guard).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn publication_candidate_is_bounded_before_copy() {
        let before = Files::new();
        let mut after = Files::new();
        for index in 0..17 {
            after.insert(
                format!("new-{index}"),
                file(&vec![0; SOURCE_LIMITS.file_bytes]),
            );
        }
        assert_eq!(
            publishable(&before, &after),
            Err("publication byte limit exceeded")
        );
        after.retain(|path, _| path != "new-16");
        assert_eq!(publishable(&before, &after).unwrap().len(), 16);
    }

    #[test]
    fn scan_and_validation_enforce_inclusive_file_and_aggregate_bounds() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("one"), b"1234").unwrap();
        fs::write(root.path().join("two"), b"5678").unwrap();
        let limits = Limits {
            file_bytes: 4,
            total_bytes: 8,
        };
        let files = scan_with(root.path(), None, limits).unwrap();
        assert!(checked_paths(&Files::new(), &files, &OutputGuard::default(), limits).is_ok());
        for limits in [
            Limits {
                file_bytes: 3,
                total_bytes: 8,
            },
            Limits {
                file_bytes: 4,
                total_bytes: 7,
            },
        ] {
            assert!(scan_with(root.path(), None, limits).is_err());
            assert!(checked_paths(&Files::new(), &files, &OutputGuard::default(), limits).is_err());
        }
    }

    #[test]
    fn link_targets_resolve_lexically_inside_the_repository_only() {
        for (path, target, resolved) in [
            ("docs/readme", "../README.md", Some("README.md")),
            ("docs/readme", "./guide.md", Some("docs/guide.md")),
            ("a/b/c", "../../x/y", Some("x/y")),
            (
                "legal/java.logging/LICENSE",
                "../java.base/LICENSE",
                Some("legal/java.base/LICENSE"),
            ),
            ("top", "../outside", None),
            ("a/b", "../..", None),
            ("a/b", "..", None),
            ("link", "/etc/passwd", None),
            ("link", "C:/Windows", None),
            ("link", "sub\\file", None),
            ("link", "", None),
            ("a/link", "d/../../x", None),
            ("a/link", "d/../x", None),
            ("link", "con.txt", None),
        ] {
            assert_eq!(
                link_target_inside(path, target).as_deref(),
                resolved,
                "{path} -> {target}"
            );
        }
        assert_eq!(
            link_target_inside("link", &"x".repeat(LINK_TARGET_BYTES_MAX + 1)),
            None
        );
    }

    #[test]
    fn snapshots_never_carry_links_at_protected_paths() {
        for (path, target) in [
            (".claude", "harness-config"),
            (".claude/settings.json", "../conf.json"),
            ("sub/.codex", "../config"),
            ("CLAUDE.md", "docs/agents.md"),
            (".mcp.json", "mcp.json"),
            (".github/workflows/ci.yml", "../../ci.yml"),
            (".golems/hello.md", "../hello.md"),
        ] {
            assert!(!snapshot_link_allowed(path, target), "{path} -> {target}");
        }
        // Links into protected content are fine: writing through them changes the real,
        // still-guarded path.
        assert!(snapshot_link_allowed(
            "docs/settings",
            "../.claude/settings.json"
        ));
        assert!(snapshot_link_allowed("docs/readme", "../README.md"));
    }

    #[test]
    fn publication_accepts_only_links_that_stay_inside_and_avoid_protected_paths() {
        let before = Files::from([("README.md".into(), file(b"readme"))]);
        let guard = OutputGuard::default();
        let mut after = before.clone();
        after.insert("docs/readme".into(), link("../README.md"));
        let changes = diff(&before, &after, &guard).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes.first().unwrap().file.as_ref().unwrap().link);
        let longest = "x".repeat(PUBLISHED_LINK_TARGET_BYTES_MAX);
        let mut after = before.clone();
        after.insert("longest".into(), link(&longest));
        assert!(
            diff(&before, &after, &guard).is_ok(),
            "100-byte target round-trips"
        );
        let long = "x".repeat(PUBLISHED_LINK_TARGET_BYTES_MAX + 1);
        for (path, target) in [
            ("escape", "../outside"),
            ("absolute", "/etc/passwd"),
            ("climb", "sub/../../x"),
            ("workflow", ".github/workflows/ci.yml"),
            ("skills", ".claude/skills"),
            ("rules", ".golems/hello.md"),
            // Inside the repository, but too long for Git archives to round-trip without PAX.
            ("long", long.as_str()),
        ] {
            let mut after = before.clone();
            after.insert(path.into(), link(target));
            assert_eq!(
                diff(&before, &after, &guard),
                Err("unsafe link target; publication blocked"),
                "{path} -> {target}"
            );
        }
        // A forbidden link path stays forbidden however safe its target.
        let mut after = before.clone();
        after.insert(".golems/alias".into(), link("../README.md"));
        assert!(diff(&before, &after, &guard).is_err());
        // Unchanged snapshot links are not re-judged; their targets are still credential-checked.
        let baseline = Files::from([(".agents/skills".into(), link("../.claude/skills"))]);
        assert_eq!(diff(&baseline, &baseline, &guard).unwrap(), vec![]);
        let mut secret = OutputGuard::default();
        secret.add("sensitive-test-secret");
        let after = Files::from([("leak".into(), link("sensitive-test-secret"))]);
        assert!(diff(&Files::new(), &after, &secret).is_err());
    }

    #[test]
    fn ignored_links_are_disposable_but_still_validated_for_forbidden_paths() {
        let before = Files::from([(".gitignore".into(), file(b"output/\n"))]);
        let mut after = before.clone();
        after.insert(
            "output/runtime/legal/LICENSE".into(),
            link("/absolute/elsewhere"),
        );
        assert_eq!(publishable(&before, &after).unwrap(), before);
        assert!(validate_workspace(&before, &after, &OutputGuard::default()).is_ok());
        after.insert("output/AGENTS.md".into(), link("x"));
        assert!(validate_workspace(&before, &after, &OutputGuard::default()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn scanner_records_links_without_following_them() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.path().join("link")).unwrap();
        fs::create_dir(root.path().join("dir")).unwrap();
        std::os::unix::fs::symlink("/", root.path().join("dir/root")).unwrap();
        let scanned = scan(root.path()).unwrap();
        assert_eq!(scanned.get("link"), Some(&link("/etc/passwd")));
        assert_eq!(scanned.get("dir/root"), Some(&link("/")));
        assert_eq!(scanned.len(), 2, "never descends through a directory link");
        assert!(diff(&Files::new(), &scanned, &OutputGuard::default()).is_err());
        let protected = tempfile::tempdir().unwrap();
        fs::create_dir(protected.path().join("config")).unwrap();
        std::os::unix::fs::symlink("config", protected.path().join(".claude")).unwrap();
        assert_eq!(
            scan(protected.path()),
            Err("snapshot contains a protected link")
        );
        let fifo = root.path().join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(scan(root.path()), Err("snapshot contains special file"));
    }
}
