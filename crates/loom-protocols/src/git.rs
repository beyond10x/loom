//! Fetch pinned data with a private bare object store; never check out repository contents.
use crate::{MAX_PROTOCOL_BYTES, ProtocolSource, SourceKind};
use std::{path::Path, process::Command};

pub(crate) fn validate_source(source: &str, path: &str) -> Result<(String, String), String> {
    if source.len() > 8192 || source.chars().any(char::is_control) {
        return Err("invalid Git source locator length or control characters".into());
    }
    let locator = source
        .strip_prefix("git+")
        .ok_or("Git source must begin with git+")?;
    let (location, revision) = locator
        .rsplit_once('#')
        .ok_or("Git source requires a full pinned commit after #")?;
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Git source requires a full 40-hex commit, not a branch or tag".into());
    }
    let url = url::Url::parse(location).map_err(|_| "invalid Git source URL")?;
    if !matches!(url.scheme(), "https" | "ssh" | "file") {
        return Err("Git transport must be HTTPS, SSH or file".into());
    }
    if url.password().is_some()
        || (url.scheme() != "ssh" && !url.username().is_empty())
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Git sources cannot contain credentials, query strings or extra fragments".into(),
        );
    }
    if url.scheme() == "file"
        && (url.host_str().is_some_and(|host| host != "localhost") || url.to_file_path().is_err())
    {
        return Err("Git file source must be a local absolute file URL".into());
    }
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Git protocol path must be a contained relative regular-file path".into());
    }
    Ok((location.into(), revision.to_ascii_lowercase()))
}
pub(crate) fn fetch(source: &str, path: &str) -> Result<(String, ProtocolSource), String> {
    let (location, revision) = validate_source(source, path)?;
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let bare = scratch.path().join("objects.git");
    git(
        &bare,
        &[
            "init",
            "--quiet",
            "--bare",
            "--template=",
            bare.to_str().ok_or("temporary Git path is not UTF-8")?,
        ],
    )?;
    git(
        &bare,
        &[
            "fetch",
            "--quiet",
            "--depth=1",
            "--no-tags",
            "--no-recurse-submodules",
            "--",
            &location,
            &revision,
        ],
    )?;
    let actual = git(&bare, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"])?;
    if actual.trim() != revision {
        return Err("fetched Git object does not match the pinned commit".into());
    }
    // Literal pathspec prevents magic/glob expansion; ls-tree reports mode, so symlinks and
    // gitlinks are refused before reading any bytes. Ancestor symlinks cannot be traversed.
    let listing = git(
        &bare,
        &["ls-tree", "-z", "--full-tree", &revision, "--", path],
    )?;
    let records: Vec<_> = listing.split('\0').filter(|v| !v.is_empty()).collect();
    if records.len() != 1 {
        return Err("pinned Git protocol path is missing or ambiguous".into());
    }
    let (metadata, filename) = records[0]
        .split_once('\t')
        .ok_or("invalid Git tree response")?;
    let fields: Vec<_> = metadata.split_whitespace().collect();
    if filename != path
        || fields.len() != 3
        || !matches!(fields[0], "100644" | "100755")
        || fields[1] != "blob"
    {
        return Err(
            "pinned Git protocol path must name a regular blob, not a symlink or submodule".into(),
        );
    }
    let oid = fields[2];
    let length = git(&bare, &["cat-file", "-s", oid])?
        .trim()
        .parse::<usize>()
        .map_err(|_| "invalid Git blob length")?;
    if length > MAX_PROTOCOL_BYTES {
        return Err("protocol document exceeds 1 MiB".into());
    }
    let yaml = git(&bare, &["cat-file", "blob", oid])?;
    if yaml.len() != length {
        return Err("Git blob length changed while reading".into());
    }
    Ok((
        yaml,
        ProtocolSource {
            kind: SourceKind::Git,
            location: source.into(),
            revision,
            path: path.into(),
        },
    ))
}
fn git(bare: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.env_clear();
    // SSH uses the operator's transport authentication; neither protocol data nor repository
    // configuration can introduce a helper or an executable command.
    for key in ["PATH", "HOME", "SSH_AUTH_SOCK", "SYSTEMROOT"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .arg(format!("--git-dir={}", bare.display()))
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "credential.helper=",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "http.followRedirects=false",
            "-c",
            "fetch.recurseSubmodules=false",
        ])
        .args(args);
    let output = command
        .output()
        .map_err(|e| format!("cannot run Git source fetch: {e}"))?;
    // Do not copy remote stderr: an authentication endpoint may echo credentials in diagnostics.
    if !output.status.success() {
        return Err(format!(
            "Git protocol source operation {} failed ({})",
            args.first().copied().unwrap_or("unknown"),
            output.status
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "Git protocol source is not UTF-8".into())
}
