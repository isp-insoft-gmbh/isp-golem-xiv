use flate2::read::MultiGzDecoder;
use golem_runner::{Result, changes::SOURCE_LIMITS};
use reqwest::{Client, Url, redirect::Policy};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::{
    collections::HashSet,
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read as _, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

const ARCHIVE_MAX: usize = 96 * 1024 * 1024;
const EXECUTABLE_MAX: u64 = 256 * 1024 * 1024;
const MANIFEST_MAX: usize = 64 * 1024;
const FETCH_MAX: Duration = Duration::from_secs(120);
const RELEASES: &str = "https://github.com/jdx/mise/releases/download";
const FIXED_CONFIGS: &[&str] = &[
    "mise.toml",
    ".mise.toml",
    "mise.local.toml",
    ".mise.local.toml",
    ".config/mise/config.toml",
    ".config/mise/mise.toml",
    ".config/mise.toml",
    ".config/mise/config.local.toml",
    ".config/mise/mise.local.toml",
    ".config/mise.local.toml",
    ".mise/config.toml",
    ".mise/config.local.toml",
    "mise/config.toml",
    "mise/config.local.toml",
];
const FRAGMENT_DIRS: &[&str] = &[".config/mise/conf.d", ".mise/conf.d", "mise/conf.d"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version(u32, u32, u32);

impl Version {
    fn parse(text: &str) -> Result<Self> {
        let invalid = "Mise minimum must be an exact numeric release version";
        if text.len() > 32 {
            return Err(invalid);
        }
        let mut parts = text.split('.');
        let mut number = || {
            let part = parts.next().ok_or(invalid)?;
            if !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return Err(invalid);
            }
            part.parse().map_err(|_| invalid)
        };
        let version = Self(number()?, number()?, number()?);
        if parts.next().is_some() {
            return Err(invalid);
        }
        Ok(version)
    }

    fn release(self) -> String {
        format!("{}.{}.{}", self.0, self.1, self.2)
    }
}

fn baseline() -> Result<Version> {
    let version = include_str!("../agent.Dockerfile")
        .split_once("RUN mise --version | grep '^")
        .and_then(|(_, rest)| rest.split_once(" '"))
        .map(|(version, _)| version)
        .ok_or("cannot read image Mise baseline")?;
    Version::parse(version)
}

#[derive(Deserialize)]
struct Config {
    min_version: Option<Minimum>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Minimum {
    Hard(String),
    Table(MinimumTable),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MinimumTable {
    hard: Option<String>,
    #[serde(rename = "soft")]
    _soft: Option<String>,
}

// The freshly extracted snapshot is private and has not been mounted in any container.
// Resolve links only inside that snapshot; do not read arbitrary host paths or special files.
fn in_snapshot(root: &Path, path: &Path) -> Result<Option<PathBuf>> {
    match path.canonicalize() {
        Ok(path) if path.starts_with(root) => Ok(Some(path)),
        Ok(_) => Err("Mise config path leaves workspace"),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(_) => Err("cannot resolve Mise config path"),
    }
}

fn config_minimum(path: &Path) -> Result<Option<Version>> {
    if !path
        .metadata()
        .map_err(|_| "cannot inspect Mise config")?
        .is_file()
    {
        return Ok(None);
    }
    let mut text = String::new();
    File::open(path)
        .and_then(|file| {
            file.take(SOURCE_LIMITS.file_bytes as u64 + 1)
                .read_to_string(&mut text)
        })
        .map_err(|_| "cannot read Mise config")?;
    if text.len() > SOURCE_LIMITS.file_bytes {
        return Err("Mise config exceeds snapshot file limit");
    }
    let config: Config =
        toml::from_str(&text).map_err(|_| "invalid Mise config for minimum selection")?;
    match config.min_version {
        Some(Minimum::Hard(hard)) => Version::parse(&hard).map(Some),
        Some(Minimum::Table(table)) => table.hard.as_deref().map(Version::parse).transpose(),
        None => Ok(None),
    }
}

fn required_version(workspace: &Path) -> Result<Option<Version>> {
    let root = workspace
        .canonicalize()
        .map_err(|_| "cannot resolve Mise workspace")?;
    let baseline = baseline()?;
    let mut selected = baseline;
    let mut seen = HashSet::new();
    let mut consider = |path: &Path| -> Result<()> {
        if let Some(path) = in_snapshot(&root, path)?
            && seen.insert(path.clone())
            && let Some(version) = config_minimum(&path)?
        {
            selected = selected.max(version);
        }
        Ok(())
    };
    for path in FIXED_CONFIGS {
        consider(&root.join(path))?;
    }
    for directory in FRAGMENT_DIRS {
        let Some(directory) = in_snapshot(&root, &root.join(directory))? else {
            continue;
        };
        if !directory.is_dir() {
            continue;
        }
        for (index, entry) in fs::read_dir(directory)
            .map_err(|_| "cannot list Mise config fragments")?
            .enumerate()
        {
            if index >= crate::snapshot::ENTRIES_MAX {
                return Err("too many Mise config fragments");
            }
            let entry = entry.map_err(|_| "cannot list Mise config fragments")?;
            // Mise filters hidden fragments and fragment folders after globbing.
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                consider(&path)?;
            }
            for name in ["mise.toml", "mise.local.toml"] {
                consider(&path.join(name))?;
            }
        }
    }
    Ok((selected > baseline).then_some(selected))
}

fn release_redirect(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str() == Some("release-assets.githubusercontent.com")
}

fn client() -> Result<Client> {
    Client::builder()
        .no_proxy()
        .https_only(true)
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() == 1 && release_redirect(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("unexpected Mise release redirect")
            }
        }))
        .build()
        .map_err(|_| "cannot initialize Mise release download")
}

async fn download(client: &Client, url: String, limit: usize) -> Result<Vec<u8>> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Mise release download failed")?;
    if response.status() != reqwest::StatusCode::OK {
        return Err("Mise release asset unavailable");
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("Mise release download exceeds byte limit");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Mise release download failed")?
    {
        if chunk.len() > limit - bytes.len() {
            return Err("Mise release download exceeds byte limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn checksum<'a>(manifest: &'a [u8], asset: &str) -> Result<&'a str> {
    let mut digest = None;
    for line in std::str::from_utf8(manifest)
        .map_err(|_| "invalid Mise checksum manifest")?
        .lines()
    {
        let Some((hash, name)) = line.split_once(' ') else {
            continue;
        };
        let name = name.strip_prefix([' ', '*']).unwrap_or(name);
        if name.strip_prefix("./").unwrap_or(name) == asset {
            if digest.is_some()
                || hash.len() != 64
                || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("invalid or ambiguous Mise archive checksum");
            }
            digest = Some(hash);
        }
    }
    digest.ok_or("Mise archive checksum missing")
}

async fn verified_archive(
    version: Version,
    fetch: impl AsyncFn(String, usize) -> Result<Vec<u8>>,
) -> Result<VerifiedArchive> {
    let release = version.release();
    let base = format!("{RELEASES}/v{release}");
    let asset = format!("mise-v{release}-linux-x64.tar.gz");
    let manifest = fetch(format!("{base}/SHASUMS256.txt"), MANIFEST_MAX).await?;
    let expected = checksum(&manifest, &asset)?;
    let archive = fetch(format!("{base}/{asset}"), ARCHIVE_MAX).await?;
    verify_checksum(archive, expected)
}

struct VerifiedArchive(Vec<u8>);

fn verify_checksum(archive: Vec<u8>, expected: &str) -> Result<VerifiedArchive> {
    let mut actual = String::with_capacity(64);
    for byte in Sha256::digest(&archive) {
        write!(&mut actual, "{byte:02x}").map_err(|_| "cannot format Mise archive checksum")?;
    }
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("Mise archive SHA-256 mismatch");
    }
    Ok(VerifiedArchive(archive))
}

fn executable(bytes: &[u8], mut output: impl Write) -> Result<()> {
    // Bound inflation including non-executable members, headers and padding.
    let expanded_max = EXECUTABLE_MAX + ARCHIVE_MAX as u64;
    let mut archive = tar::Archive::new(MultiGzDecoder::new(bytes).take(expanded_max + 1));
    let mut found = false;
    let invalid = "invalid Mise release archive";
    for entry in archive.entries().map_err(|_| invalid)?.raw(true) {
        let mut entry = entry.map_err(|_| invalid)?;
        // Never unpack paths or honor archive extensions, links, modes or ownership.
        if entry.path_bytes() != b"mise/bin/mise".as_slice() {
            continue;
        }
        if found || !entry.header().entry_type().is_file() || entry.size() == 0 {
            return Err("invalid Mise executable member");
        }
        if entry.size() > EXECUTABLE_MAX {
            return Err("Mise executable exceeds byte limit");
        }
        if io::copy(&mut entry, &mut output).map_err(|_| invalid)? != entry.size() {
            return Err(invalid);
        }
        found = true;
    }
    let mut decoder = archive.into_inner();
    io::copy(&mut decoder, &mut io::sink()).map_err(|_| invalid)?;
    if decoder.limit() == 0 {
        return Err("expanded Mise archive exceeds byte limit");
    }
    if !found {
        return Err("Mise executable missing from archive");
    }
    Ok(())
}

pub fn mount_source(workspace: &Path) -> PathBuf {
    workspace.with_extension("golem-mise")
}

fn stage(workspace: &Path, archive: &VerifiedArchive) -> Result<()> {
    let parent = workspace
        .parent()
        .ok_or("missing private Mise staging directory")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "cannot stage Mise executable")?;
    executable(&archive.0, &mut file)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o555))
            .map_err(|_| "cannot set Mise executable permissions")?;
    }
    file.persist_noclobber(mount_source(workspace))
        .map_err(|_| "cannot stage Mise executable")?;
    Ok(())
}

pub async fn prepare(workspace: &Path, seconds: u64) -> Result<u64> {
    prepare_with(workspace, seconds, FETCH_MAX, async |version| {
        let client = client()?;
        verified_archive(version, async |url, limit| {
            download(&client, url, limit).await
        })
        .await
    })
    .await
}

async fn prepare_with(
    workspace: &Path,
    seconds: u64,
    fetch_max: Duration,
    fetch: impl AsyncFnOnce(Version) -> Result<VerifiedArchive>,
) -> Result<u64> {
    let start = Instant::now();
    let Some(version) = required_version(workspace)? else {
        return Ok(seconds);
    };
    let budget = Duration::from_secs(seconds);
    let archive = timeout_at(start + budget.min(fetch_max), fetch(version))
        .await
        .map_err(|_| "Mise release download timed out")??;
    stage(workspace, &archive)?;
    let remaining = budget.saturating_sub(start.elapsed()).as_secs();
    if remaining == 0 {
        return Err("Mise selection exhausted agent budget");
    }
    println!("golem mise: using verified {}", version.release());
    Ok(remaining)
}

#[cfg(test)]
pub(crate) mod tests;
