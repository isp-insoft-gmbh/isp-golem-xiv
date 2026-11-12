use crate::{OutputGuard, Result};
use std::{collections::BTreeMap, fs, io::Read as _, path::Path};

const ENTRIES_MAX: usize = 20_000;
const FILE_BYTES_MAX: usize = 4 * 1024 * 1024;
const TOTAL_BYTES_MAX: usize = 64 * 1024 * 1024;
const DEPTH_MAX: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub bytes: Vec<u8>,
    pub executable: bool,
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
/// Rejects links, special files, nonportable paths, or snapshots exceeding explicit limits.
/// Existing forbidden files are retained so changes, including deletion, can be detected.
pub fn scan(root: &Path) -> Result<Files> {
    scan_with(root, None)
}

/// Scans after the agent against the snapshot: harness-created state under `.claude`, `.codex`,
/// `.pi` or `.agents` that did not exist before is skipped entirely (not counted, not published,
/// links allowed), while every file present in the snapshot is still compared.
///
/// # Errors
/// As [`scan`] for everything outside skipped harness state.
pub fn scan_after(root: &Path, baseline: &Files) -> Result<Files> {
    scan_with(root, Some(baseline))
}

fn scan_with(root: &Path, baseline: Option<&Files>) -> Result<Files> {
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
            if !metadata.is_file() {
                return Err("snapshot contains link or special file");
            }
            if metadata.len() > FILE_BYTES_MAX as u64 {
                return Err("snapshot file limit exceeded");
            }
            let mut bytes = Vec::new();
            fs::File::open(entry.path())
                .map_err(|_| "cannot open snapshot file")?
                .take(FILE_BYTES_MAX as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "cannot read snapshot file")?;
            total += bytes.len();
            if bytes.len() > FILE_BYTES_MAX || total > TOTAL_BYTES_MAX {
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
            files.insert(path, FileState { bytes, executable });
        }
    }
    Ok(files)
}

fn bounded(files: &Files) -> Result<()> {
    if files.len() > ENTRIES_MAX {
        return Err("snapshot entry limit exceeded");
    }
    let mut total = 0usize;
    let mut names = std::collections::BTreeSet::new();
    for (path, file) in files {
        if !valid_path(path) || !names.insert(path.to_ascii_lowercase()) {
            return Err("invalid or colliding snapshot path");
        }
        if file.bytes.len() > FILE_BYTES_MAX {
            return Err("snapshot file limit exceeded");
        }
        total += file.bytes.len();
        if total > TOTAL_BYTES_MAX {
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
    bounded(before)?;
    bounded(after)?;
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
        changes.push(Change {
            path: path.clone(),
            file: after.get(path).cloned(),
        });
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
        let after = scan_after(root.path(), &before).unwrap();
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
        let after = scan_after(root.path(), &before).unwrap();
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
                &scan_after(root.path(), &before).unwrap(),
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
                &scan_after(root.path(), &before).unwrap(),
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
                    &scan_after(root.path(), &before).unwrap(),
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
        large.set_len(FILE_BYTES_MAX as u64 + 1).unwrap();
        assert!(scan(root.path()).is_err());
        let collision = Files::from([("A".into(), file(b"a")), ("a".into(), file(b"b"))]);
        assert!(diff(&Files::new(), &collision, &OutputGuard::default()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn scanner_rejects_symlinks() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.path().join("link")).unwrap();
        assert!(scan(root.path()).is_err());
    }
}
