//! `protocols-vendor --checkout <dir> --tag <tag>`: rewrites `protocols/engineering/` and its
//! `lock.json` from a local Git checkout of engineering-protocols at a tag.
//!
//! The command reads Git objects only. It resolves `refs/tags/<tag>` to its commit, lists the
//! commit's `protocols/` tree, and reads every `protocols/<name>/<major>.yaml` blob of it as bytes;
//! a file directly in `protocols/` (`vocabulary.yaml`) is not vendored. It never fetches, so a tag,
//! or a blob of a partial clone, that the checkout lacks is refused. The lock is written by
//! [`loom_protocols::lock`]: [`lock::locked_file`] per file and [`lock::encode`] for the bytes,
//! with [`lock::REPOSITORY`] as the repository whatever the checkout's remote is, so the command
//! and the catalog's load share one encoder and one decoder.
//!
//! Everything is validated before the first write, so a refusal changes nothing under the root:
//! every path below `protocols/` must be a regular file (a link or a submodule is refused) and every
//! vendored path must be `<name>/<major>.yaml` by the lock's own rule ([`lock::decode`]), which
//! leaves no room for `..`, an absolute path or a further directory. `protocols/` and
//! `protocols/engineering/` under the root must be directories, not links. The new tree is then
//! written beside the old one and swapped in by rename, so a file the tag no longer holds is gone.

use loom_protocols::lock::{self, LockError, ProtocolLock};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The vendored directory, relative to the repository root.
const VENDORED: &str = "protocols/engineering";
/// The directory of engineering-protocols whose `<name>/<major>.yaml` files are vendored.
const UPSTREAM: &str = "protocols/";

/// Rewrites `<root>/protocols/engineering/` from `checkout` at `tag`.
pub(crate) fn protocols_vendor(root: &Path, checkout: &Path, tag: &str) -> Result<(), String> {
    if !checkout.is_dir() {
        return Err(format!(
            "--checkout {}: not a directory; give a local checkout of engineering-protocols \
             (this command never fetches)",
            checkout.display()
        ));
    }
    // Before any other Git call: an older Git ignores `GIT_NO_LAZY_FETCH`.
    let version = git(checkout, &["--version"]).map_err(|e| format!("git --version: {e}"))?;
    supported_git(&String::from_utf8_lossy(&version))?;
    // A revision expression (`t1^0`, `t1~1`) would resolve to a commit the tag does not name.
    let tag_ref = format!("refs/tags/{tag}");
    if tag.is_empty() || git(checkout, &["check-ref-format", &tag_ref]).is_err() {
        return Err(format!("--tag {tag:?}: not a tag name"));
    }
    let target = destination(root)?;
    git(checkout, &["rev-parse", "--git-dir"]).map_err(|error| {
        format!(
            "--checkout {}: not a Git repository: {error}",
            checkout.display()
        )
    })?;
    let commit = commit(checkout, tag, &tag_ref)?;
    let paths = protocol_paths(checkout, tag, &commit)?;

    // Every path against the lock's own rule, before a blob is read.
    let unread = paths
        .iter()
        .map(|path| lock::locked_file(path, &[]))
        .collect();
    lock::decode(&lock::encode(&lock_of(tag, &commit, unread))).map_err(|e| refusal(tag, &e))?;

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let object = format!("{commit}:{UPSTREAM}{path}");
        let bytes = git(checkout, &["cat-file", "blob", &object])
            .map_err(|error| format!("tag {tag}: {UPSTREAM}{path} is not readable: {error}"))?;
        files.push((path, bytes));
    }
    let locked = files
        .iter()
        .map(|(path, bytes)| lock::locked_file(path, bytes))
        .collect();
    let encoded = lock::encode(&lock_of(tag, &commit, locked));
    lock::decode(&encoded).map_err(|e| refusal(tag, &e))?;

    write(&target, &files, &encoded)?;
    println!("{VENDORED}: {} files from {tag} ({commit})", files.len());
    Ok(())
}

fn lock_of(tag: &str, commit: &str, files: Vec<lock::LockedFile>) -> ProtocolLock {
    ProtocolLock {
        format: lock::FORMAT.to_owned(),
        repository: lock::REPOSITORY.to_owned(),
        tag: tag.to_owned(),
        commit: commit.to_owned(),
        files,
    }
}

/// A lock refusal as the command reports it: the tag and the path in the checkout it names.
fn refusal(tag: &str, error: &LockError) -> String {
    match &error.problem {
        Some(problem) if error.path != "lock.json" => {
            format!("tag {tag}: {UPSTREAM}{}: {problem}", error.path)
        }
        _ => format!("tag {tag}: the lock would be refused: {error}"),
    }
}

/// `<root>/protocols/engineering`, refusing a root that is not a directory and a `protocols/` or
/// `protocols/engineering/` that is a link or a file: writing through a link would leave the root.
fn destination(root: &Path) -> Result<PathBuf, String> {
    if !root.is_dir() {
        return Err(format!("--root {}: not a directory", root.display()));
    }
    let target = root.join(VENDORED);
    for dir in [root.join("protocols"), target.clone()] {
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(format!(
                    "{}: not a directory of the repository but a link or a file; nothing is written",
                    dir.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", dir.display())),
        }
    }
    Ok(target)
}

/// The oldest Git whose `GIT_NO_LAZY_FETCH` the command relies on.
const MINIMUM_GIT: (u64, u64, u64) = (2, 44, 0);

/// Accepts the output of `git --version` when it names Git 2.44.0 or newer, read by the first three
/// numbers of its version (`git version 2.44.0.windows.1`, `git version 2.45.2 (Apple Git-154)`);
/// refuses an older Git, naming the version found, and an output it cannot read.
fn supported_git(printed: &str) -> Result<(), String> {
    let (major, minor, patch) = MINIMUM_GIT;
    let found = printed.trim();
    let refusal = |why: &str| {
        format!(
            "git: {why} {found:?}; this command needs Git {major}.{minor}.{patch} or newer, whose \
             GIT_NO_LAZY_FETCH keeps a checkout's promisor remote from running; nothing is written"
        )
    };
    let numbers: Option<Vec<u64>> = found
        .strip_prefix("git version ")
        .and_then(|rest| rest.split_whitespace().next())
        .map(|version| {
            version
                .split('.')
                .take(3)
                .map_while(|part| {
                    let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                    digits.parse().ok()
                })
                .collect()
        });
    match numbers.as_deref() {
        Some(&[a, b, c]) if (a, b, c) >= MINIMUM_GIT => Ok(()),
        Some(&[_, _, _]) => Err(refusal("older Git")),
        _ => Err(refusal("unreadable version")),
    }
}

/// The commit `refs/tags/<tag>` names in the checkout, as 40 lowercase hex digits. The exact ref
/// is resolved first: `rev-parse refs/tags/<tag>` would also find a branch of that name.
fn commit(checkout: &Path, tag: &str, tag_ref: &str) -> Result<String, String> {
    let missing = || {
        format!(
            "tag {tag}: not in the checkout {}; fetch it there first (this command never fetches)",
            checkout.display()
        )
    };
    let object =
        git(checkout, &["show-ref", "--verify", "--hash", tag_ref]).map_err(|_| missing())?;
    let reference = format!("{}^{{commit}}", String::from_utf8_lossy(&object).trim());
    let out =
        git(checkout, &["rev-parse", "--verify", "--quiet", &reference]).map_err(|_| missing())?;
    let commit = String::from_utf8_lossy(&out).trim().to_owned();
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!(
            "tag {tag}: resolves to {commit:?}, not a commit of 40 lowercase hex digits"
        ));
    }
    Ok(commit)
}

/// The paths below `protocols/` of the commit that are vendored, `<name>/…` without the prefix,
/// in the tree's order. Every entry below `protocols/` must be a regular file; one directly in
/// `protocols/` is not vendored.
fn protocol_paths(checkout: &Path, tag: &str, commit: &str) -> Result<Vec<String>, String> {
    let listing = git(
        checkout,
        &["ls-tree", "-r", "-z", "--full-tree", commit, "--", UPSTREAM],
    )
    .map_err(|error| format!("tag {tag}: list {UPSTREAM}: {error}"))?;
    let mut paths = Vec::new();
    for record in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let (meta, path) = match record.iter().position(|b| *b == b'\t') {
            Some(tab) => (&record[..tab], &record[tab + 1..]),
            None => {
                return Err(format!(
                    "tag {tag}: unreadable tree entry {:?}",
                    String::from_utf8_lossy(record)
                ));
            }
        };
        let path = std::str::from_utf8(path).map_err(|_| {
            format!(
                "tag {tag}: {} is not a UTF-8 path",
                String::from_utf8_lossy(path)
            )
        })?;
        let Some(relative) = path.strip_prefix(UPSTREAM) else {
            continue;
        };
        let mode = meta.split(|b| *b == b' ').next().unwrap_or_default();
        if mode != b"100644" && mode != b"100755" {
            return Err(format!(
                "tag {tag}: {path} is a link or a submodule (mode {}), not a file",
                String::from_utf8_lossy(mode)
            ));
        }
        if relative.contains('/') {
            paths.push(relative.to_owned());
        }
    }
    if paths.is_empty() {
        return Err(format!(
            "tag {tag}: holds no {UPSTREAM}<name>/<major>.yaml file"
        ));
    }
    Ok(paths)
}

/// Writes the files and the lock into a staging directory beside `target`, then swaps it in by
/// rename: the old tree is moved aside, the new one moved in, and the old one removed.
fn write(target: &Path, files: &[(String, Vec<u8>)], lock: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent", target.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let id = std::process::id();
    let staging = crate::Staging::new(parent.join(format!(".engineering.staging-{id}")))?;
    fs::create_dir(&staging.dir).map_err(|e| format!("create {}: {e}", staging.dir.display()))?;
    for (path, bytes) in files {
        let to = staging.dir.join(path);
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        fs::write(&to, bytes).map_err(|e| format!("write {}: {e}", to.display()))?;
    }
    let lock_file = staging.dir.join("lock.json");
    fs::write(&lock_file, lock).map_err(|e| format!("write {}: {e}", lock_file.display()))?;

    if !target.exists() {
        return fs::rename(&staging.dir, target)
            .map_err(|e| format!("move the new tree to {}: {e}", target.display()));
    }
    let old = parent.join(format!(".engineering.old-{id}"));
    if old.exists() {
        fs::remove_dir_all(&old).map_err(|e| format!("remove {}: {e}", old.display()))?;
    }
    fs::rename(target, &old).map_err(|e| format!("move {} aside: {e}", target.display()))?;
    if let Err(error) = fs::rename(&staging.dir, target) {
        return Err(match fs::rename(&old, target) {
            Ok(()) => format!(
                "move the new tree to {}: {error}; previous tree restored",
                target.display()
            ),
            Err(e) => format!(
                "move the new tree to {}: {error}; previous tree left at {}: {e}",
                target.display(),
                old.display()
            ),
        });
    }
    fs::remove_dir_all(&old).map_err(|e| {
        format!(
            "{VENDORED} rewritten, but the previous tree at {} could not be removed: {e}",
            old.display()
        )
    })
}

/// Runs `git -C <checkout> <args>` and returns its standard output as bytes, never decoded.
///
/// Every `GIT_*` variable of the caller is removed (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
/// object directories and the rest), so the repository read is the checkout's; neither system
/// nor global configuration is read, hooks and the file-system monitor are off, replace refs are
/// ignored and pathspecs are literal; standard input is null. A missing object is an error, never a
/// fetch: `GIT_NO_LAZY_FETCH` stops a lazy fetch (a Git older than 2.44.0, which ignores it, is
/// refused before the first other call), `protocol.allow=never` is the default for every transport
/// the checkout's configuration does not name, and `protocol.ext.allow=never` overrides the
/// checkout's own setting for `ext`, the transport that starts a program.
fn git(checkout: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    let out = command
        .arg("-C")
        .arg(checkout)
        .args([
            "--no-replace-objects",
            "--literal-pathspecs",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "credential.helper=",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("run git {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::supported_git;

    #[test]
    fn a_git_older_than_2_44_0_is_refused_naming_the_version_found() {
        for printed in ["git version 2.43.9\n", "git version 2.39.3 (Apple Git-146)"] {
            let refusal = supported_git(printed).expect_err(printed);
            let version = printed.trim().trim_start_matches("git version ");
            assert!(refusal.contains(version), "{refusal}");
            assert!(refusal.contains("2.44.0"), "{refusal}");
        }
    }

    #[test]
    fn git_2_44_0_and_newer_are_accepted() {
        for printed in [
            "git version 2.44.0\n",
            "git version 2.56.1",
            "git version 3.0.0",
            "git version 2.44.0.windows.1",
            "git version 2.45.2 (Apple Git-154)",
            "git version 2.44.1-rc0",
        ] {
            assert_eq!(supported_git(printed), Ok(()), "{printed}");
        }
    }

    #[test]
    fn a_version_that_cannot_be_read_is_refused() {
        for printed in [
            "",
            "git version",
            "git version 2.44",
            "git version two",
            "2.44.0",
        ] {
            let refusal = supported_git(printed).expect_err(printed);
            assert!(refusal.contains("2.44.0"), "{refusal}");
        }
    }
}
