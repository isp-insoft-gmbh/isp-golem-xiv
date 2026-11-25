#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use flate2::{Compression, write::GzEncoder};
use std::io::Cursor;

const FUTURE: &str = "2099.10.2";
const FUTURE_ASSET: &str = "mise-v2099.10.2-linux-x64.tar.gz";
const FIXTURE_MISE: &[u8] = br#"#!/bin/sh
set -eu
case "$1" in
  install)
    mkdir -p /target-tools/shadow
    printf '#!/bin/sh\necho wrong-mise\n' > /target-tools/shadow/mise
    chmod +x /target-tools/shadow/mise ;;
  bin-paths) echo /target-tools/shadow ;;
  env) echo '{}' ;;
  --version) echo fixture-mise ;;
  *) exit 1 ;;
esac
"#;

fn config(workspace: &Path, path: &str, text: &str) {
    let path = workspace.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn tarball(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (name, kind, bytes) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(*kind);
        header.set_mode(0o6777);
        header.set_size(bytes.len() as u64);
        tar.append_data(&mut header, name, *bytes).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

fn manifest(bytes: &[u8]) -> Vec<u8> {
    let mut hash = String::new();
    for byte in Sha256::digest(bytes) {
        write!(&mut hash, "{byte:02x}").unwrap();
    }
    format!("{hash}  ./{FUTURE_ASSET}\n").into_bytes()
}

pub(crate) async fn prepare_fixture(workspace: &Path) -> u64 {
    config(
        workspace,
        ".mise/conf.d/dev/mise.local.toml",
        &format!("[min_version]\nhard = '{FUTURE}'\n"),
    );
    config(workspace, "mise.toml", "min_version = '2099.9.30'");
    let archive = tarball(&[("mise/bin/mise", tar::EntryType::Regular, FIXTURE_MISE)]);
    prepare_with(workspace, 30, FETCH_MAX, async |version| {
        verified_archive(version, async |url, limit| {
            let base = "https://github.com/jdx/mise/releases/download/v2099.10.2/";
            match url.strip_prefix(base) {
                Some("SHASUMS256.txt") => {
                    assert_eq!(limit, 64 * 1024);
                    Ok(manifest(&archive))
                }
                Some(FUTURE_ASSET) => {
                    assert_eq!(limit, 96 * 1024 * 1024);
                    Ok(archive.clone())
                }
                _ => Err("unexpected fixture request"),
            }
        })
        .await
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn exact_verified_release_is_staged_privately_once() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let remaining = prepare_fixture(&workspace).await;
    assert!((1..30).contains(&remaining));
    let staged = mount_source(&workspace);
    assert_eq!(staged.parent(), Some(root.path()));
    assert_eq!(fs::read(staged).unwrap(), FIXTURE_MISE);
    assert!(
        stage(
            &workspace,
            &VerifiedArchive(tarball(&[(
                "mise/bin/mise",
                tar::EntryType::Regular,
                b"other"
            )]))
        )
        .is_err()
    );
    assert_eq!(fs::read(mount_source(&workspace)).unwrap(), FIXTURE_MISE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            fs::metadata(mount_source(&workspace))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o555
        );
    }
}

#[tokio::test]
async fn baseline_or_soft_and_inactive_requirements_never_fetch() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path();
    fs::create_dir(workspace.join("mise.toml")).unwrap();
    config(workspace, "mise/conf.d", "a file, not a fragment directory");
    assert_eq!(prepare(workspace, 30).await, Ok(30));
    fs::remove_dir(workspace.join("mise.toml")).unwrap();
    let no_fetch = async |_| Err("must not download when baseline suffices");
    assert_eq!(
        prepare_with(workspace, 30, FETCH_MAX, no_fetch).await,
        Ok(30)
    );
    config(
        workspace,
        ".config/mise/config.local.toml",
        &format!("min_version = '{}'", baseline().unwrap().release()),
    );
    config(
        workspace,
        "mise.toml",
        "min_version = '2025.12.31'\n[tasks]\nmin_version = '2099.1.0'",
    );
    config(
        workspace,
        ".mise/conf.d/soft.toml",
        "min_version.soft = '2099.1.0'",
    );
    config(workspace, ".miserc.toml", "env = ['fixture']");
    config(workspace, "mise.fixture.toml", "min_version = '2099.1.0'");
    config(
        workspace,
        "packages/app/mise.toml",
        "min_version = '2099.1.0'",
    );
    assert_eq!(
        prepare_with(workspace, 30, FETCH_MAX, no_fetch).await,
        Ok(30)
    );
}

#[test]
fn default_fragments_exclude_hidden_names_and_uppercase_extensions() {
    let root = tempfile::tempdir().unwrap();
    config(
        root.path(),
        ".mise/conf.d/dev.toml",
        "min_version = '2099.2.0'",
    );
    for ignored in [".toml", ".backup.toml", ".hidden/mise.toml", "extra.TOML"] {
        config(
            root.path(),
            &format!(".mise/conf.d/{ignored}"),
            "min_version = '9999.1.0'",
        );
    }
    assert_eq!(
        required_version(root.path()).unwrap().unwrap().release(),
        "2099.2.0"
    );
}

#[test]
fn invalid_root_config_cannot_choose_release_path() {
    let root = tempfile::tempdir().unwrap();
    for minimum in [
        "42",
        "'https://evil.test/mise'",
        "'2099.1.0/../../archive'",
        "'2099.01.0'",
        "'2099.1.0-rc1'",
        "{ unrelated = '2099.1.0' }",
        "'4294967296.1.1'",
        "'2099.1.0.1'",
    ] {
        config(
            root.path(),
            "mise.toml",
            &format!("min_version = {minimum}"),
        );
        assert!(required_version(root.path()).is_err(), "{minimum}");
    }
    config(root.path(), "mise.toml", "min_version = '");
    assert!(required_version(root.path()).is_err());
    fs::write(root.path().join("mise.toml"), [0xff]).unwrap();
    assert_eq!(
        required_version(root.path()).err(),
        Some("cannot read Mise config")
    );
    let oversized = format!(
        "{}\nmin_version = '2099.1.0'",
        "#".repeat(SOURCE_LIMITS.file_bytes)
    );
    config(root.path(), "mise.toml", &oversized);
    assert!(required_version(root.path()).is_err());
}

#[cfg(unix)]
#[test]
fn config_links_are_read_only_within_original_snapshot() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    config(
        &workspace,
        "metadata/toolchain.toml",
        "min_version = '2099.1.0'",
    );
    let path = workspace.join("mise.toml");
    symlink("metadata/toolchain.toml", &path).unwrap();
    assert_eq!(
        required_version(&workspace).unwrap().unwrap().release(),
        "2099.1.0"
    );
    fs::remove_file(&path).unwrap();
    config(root.path(), "outside.toml", "min_version = '2099.1.0'");
    symlink("../outside.toml", &path).unwrap();
    assert_eq!(
        required_version(&workspace).err(),
        Some("Mise config path leaves workspace")
    );
}

#[tokio::test]
async fn invalid_manifest_mismatch_or_invalid_archive_never_stages_mise() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    config(
        &workspace,
        "mise.toml",
        &format!("min_version = '{FUTURE}'"),
    );
    let archive = b"not even gzip";
    let valid = manifest(archive);
    for (manifest, expected) in [
        (b"".to_vec(), "Mise archive checksum missing"),
        (vec![0xff], "invalid Mise checksum manifest"),
        (
            [valid.clone(), valid.clone()].concat(),
            "invalid or ambiguous Mise archive checksum",
        ),
        (
            format!("bad  ./{FUTURE_ASSET}\n").into_bytes(),
            "invalid or ambiguous Mise archive checksum",
        ),
        (valid.clone(), "Mise archive SHA-256 mismatch"),
        (valid, "invalid Mise release archive"),
    ] {
        let result = prepare_with(&workspace, 30, FETCH_MAX, async |version| {
            verified_archive(version, async |url, _| {
                if url.ends_with("/SHASUMS256.txt") {
                    Ok(manifest.clone())
                } else {
                    match expected {
                        "Mise archive SHA-256 mismatch" => Ok(b"tampered archive".to_vec()),
                        "invalid Mise release archive" => Ok(archive.to_vec()),
                        _ => Err("must not fetch archive without exact checksum"),
                    }
                }
            })
            .await
        })
        .await;
        assert_eq!(result, Err(expected));
        assert!(!mount_source(&workspace).exists());
    }
}

#[test]
fn extraction_rejects_wrong_kind_duplicate_missing_truncated_and_oversize() {
    use tar::EntryType::{Regular, Symlink};
    let named = ("mise/bin/mise", Regular, FIXTURE_MISE);
    for (entries, expected) in [
        (
            vec![("unrelated", Regular, FIXTURE_MISE)],
            "Mise executable missing from archive",
        ),
        (
            vec![("mise/bin/mise", Symlink, &[][..])],
            "invalid Mise executable member",
        ),
        (vec![named, named], "invalid Mise executable member"),
    ] {
        assert_eq!(executable(&tarball(&entries), io::sink()), Err(expected));
    }
    let mut truncated = tarball(&[named]);
    truncated.pop();
    assert!(executable(&truncated, io::sink()).is_err());
    let mut header = tar::Header::new_ustar();
    header.set_path("mise/bin/mise").unwrap();
    header.set_size(EXECUTABLE_MAX + 1);
    header.set_entry_type(Regular);
    header.set_cksum();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(header.as_bytes()).unwrap();
    assert_eq!(
        executable(&gzip.finish().unwrap(), io::sink()),
        Err("Mise executable exceeds byte limit")
    );

    let mut output = Cursor::new(Vec::new());
    executable(
        &tarball(&[("mise/bin/not-mise", Regular, b"wrong"), named]),
        &mut output,
    )
    .unwrap();
    assert_eq!(output.into_inner(), FIXTURE_MISE);
}

// Drive real reqwest parsing and chunk reads without a network dependency.
async fn serve_once(reply: &'static [u8], limit: usize) -> Result<Vec<u8>> {
    use std::{io::BufRead as _, net::TcpListener};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/fixture", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut reader = io::BufReader::new(&socket);
        for _ in 0..16 {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        drop(socket.write_all(reply));
    });
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let result = download(&client, url, limit).await;
    server.join().unwrap();
    result
}

#[tokio::test]
async fn release_transport_rejects_unavailable_and_oversized_assets() {
    assert_eq!(
        download(
            &client().unwrap(),
            "http://127.0.0.1:1/fixture".into(),
            MANIFEST_MAX
        )
        .await,
        Err("Mise release download failed")
    );
    assert_eq!(
        serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort",
            MANIFEST_MAX
        )
        .await,
        Err("Mise release download failed")
    );
    assert_eq!(
        serve_once(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
            MANIFEST_MAX
        )
        .await,
        Err("Mise release asset unavailable")
    );
    assert_eq!(
        serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100663297\r\n\r\n",
            ARCHIVE_MAX
        )
        .await,
        Err("Mise release download exceeds byte limit")
    );
    let chunked =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n";
    assert_eq!(
        serve_once(chunked, 5).await,
        Err("Mise release download exceeds byte limit")
    );
    assert_eq!(serve_once(chunked, 6).await, Ok(b"abcdef".to_vec()));
}

#[tokio::test]
async fn release_fetch_is_cancelled_within_shared_budget_and_cannot_follow_foreign_hosts() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    config(
        &workspace,
        "mise.toml",
        &format!("min_version = '{FUTURE}'"),
    );
    let hanging = async |_| std::future::pending::<Result<VerifiedArchive>>().await;
    assert_eq!(
        prepare_with(&workspace, 30, Duration::from_millis(10), hanging).await,
        Err("Mise release download timed out")
    );
    assert_eq!(
        prepare_with(&workspace, 0, FETCH_MAX, hanging).await,
        Err("Mise release download timed out")
    );
    assert!(!mount_source(&workspace).exists());
    for (url, allowed) in [
        ("https://release-assets.githubusercontent.com/file", true),
        ("http://release-assets.githubusercontent.com/file", false),
        (
            "https://release-assets.githubusercontent.com.evil.test/file",
            false,
        ),
        (
            "https://release-assets.githubusercontent.com:444/file",
            false,
        ),
        (
            "https://user@release-assets.githubusercontent.com/file",
            false,
        ),
    ] {
        assert_eq!(release_redirect(&Url::parse(url).unwrap()), allowed);
    }
}
