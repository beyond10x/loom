//! The revision Loom computes for each artifact a bindings document places at a path, and the
//! governor wrapper that computes them again before every evaluation
//! (`story:bound-artifact-revisions`).
//!
//! [`content_revision`] is `t` and the first 16 hex digits of a SHA-256, or [`ABSENT`] while
//! nothing is at the path:
//!
//! - a file: the SHA-256 over its bytes, so a file holding `abc` is `tba7816bf8f01cfea`;
//! - a symbolic link: the SHA-256 over its target string; it is never followed;
//! - a directory, a tree: the SHA-256 over [`TREE_HEADER`] and then, for every entry below it in
//!   path order (each directory's names in byte order, a directory's entries right after it), the
//!   entry's kind (`d`, `f` or `l`), its path relative to the placed path (segments joined by `/`),
//!   a NUL byte, the length of its bytes as eight big-endian bytes and the bytes: a file's content,
//!   a link's target string, none for a directory. Listing order changes nothing; one byte or one
//!   name changes the digest.
//!
//! Nothing outside the workspace is read. The workspace root is the host's; from it each segment
//! of the placed path is opened relative to the directory before it with `O_NOFOLLOW`, so a link
//! among the path's directories is refused ([`RevisionRefusalReason::OutsideWorkspace`]) wherever
//! it points, and an absolute path, an empty one or one with a `..` segment is refused before
//! anything is opened. An entry is examined without being followed before it is opened, and the
//! one opened must be the one examined. A file whose size, modification or change time moved while
//! it was read, or whose bytes were not its size, and a directory whose entries changed while its
//! tree was read, are [`RevisionRefusalReason::ChangedWhileRead`]. More than
//! [`MAX_ARTIFACT_BYTES`] bytes, [`MAX_TREE_ENTRIES`] entries or [`MAX_TREE_DEPTH`] levels under
//! one placed path is [`RevisionRefusalReason::TooLarge`], refused before the bytes past the bound
//! are read. An entry that is neither a file, a directory nor a link (a pipe, a socket, a device) is
//! never opened: [`RevisionRefusalReason::Unreadable`], as is one that cannot be opened.
//!
//! [`PlacedRevisions`] is the governor a host hands Commission's loop: before each `frontier` and
//! `completion` it computes every placement and writes the moves in one store write
//! ([`CanonGovernor::record_step`]), so a person's edit in the workspace or another case's write
//! reaches the case as a new revision before the frontier is read.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read as _;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use b10x_loom_commission::model::responsibility::{
    CaseId, CompletionDetermination, Frontier, GovernorError, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use loom::bindings::{
    ArtifactPlacement, BindingsDocument, RevisionMove, RevisionRefusal, RevisionRefusalReason,
};
use loom_governor::{
    CanonGovernor, EvaluationTime, FallibleCaseStore, NoEvaluationTime, StepError,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;
use sha2::{Digest as _, Sha256};

/// The revision of a placed path at which nothing is.
pub const ABSENT: &str = "none";
/// The most bytes (file contents and link targets) read under one placed path.
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
/// The most entries of one placed tree.
pub const MAX_TREE_ENTRIES: usize = 10_000;
/// The deepest directory level of one placed tree, the placed directory being level 0.
pub const MAX_TREE_DEPTH: usize = 64;
/// The bytes a tree's digest starts with, so it is never the digest of a file's bytes alone by
/// accident.
pub const TREE_HEADER: &[u8] = b"loom.bindings tree/1\n";

/// Why one placed path was not digested, before the artifact is named.
type Refused = (RevisionRefusalReason, String);

fn refused(reason: RevisionRefusalReason, detail: impl Into<String>) -> Refused {
    (reason, detail.into())
}

/// The revision of what is at `path` in `workspace` (see the module documentation).
///
/// # Errors
/// The reason and what is wrong, when the path is not digested; nothing outside the workspace is
/// read.
pub fn content_revision(
    workspace: &Path,
    path: &str,
) -> Result<String, (RevisionRefusalReason, String)> {
    let segments = segments(path)?;
    let mut dir = rustix::fs::open(
        workspace,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        refused(
            RevisionRefusalReason::Unreadable,
            format!(
                "the workspace {} cannot be opened: {error}",
                workspace.display()
            ),
        )
    })?;
    let mut budget = Budget::default();
    let Some((leaf, ancestors)) = segments.split_last() else {
        // The placed path is the workspace itself: a tree.
        let mut hasher = tree_hasher();
        walk(&dir, b"", 0, &mut budget, &mut hasher)?;
        return Ok(revision(hasher));
    };
    for segment in ancestors {
        match rustix::fs::openat(
            &dir,
            *segment,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(next) => dir = next,
            Err(Errno::NOENT) => return Ok(ABSENT.to_owned()),
            Err(error) => {
                // A link is refused wherever it points; anything else where a directory belongs
                // leaves nothing at the path.
                return match rustix::fs::statat(&dir, *segment, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink => {
                        Err(refused(
                            RevisionRefusalReason::OutsideWorkspace,
                            format!(
                                "{segment} is a symbolic link; a link among the directories of a placed path is never followed"
                            ),
                        ))
                    }
                    Ok(stat) if FileType::from_raw_mode(stat.st_mode) != FileType::Directory => {
                        Ok(ABSENT.to_owned())
                    }
                    Err(Errno::NOENT) => Ok(ABSENT.to_owned()),
                    _ => Err(refused(
                        RevisionRefusalReason::Unreadable,
                        format!("the directory {segment} cannot be opened: {error}"),
                    )),
                };
            }
        }
    }
    match entry(&dir, *leaf)? {
        Entry::Absent => Ok(ABSENT.to_owned()),
        Entry::Link(target) => {
            budget.take_bytes(target.len() as u64)?;
            Ok(revision(Sha256::new_with_prefix(&target)))
        }
        Entry::File(fd, stat) => {
            let mut hasher = Sha256::new();
            read_file(fd, &stat, &mut budget, &mut hasher)?;
            Ok(revision(hasher))
        }
        Entry::Directory(fd, stat) => {
            let mut hasher = tree_hasher();
            walk(&fd, b"", 0, &mut budget, &mut hasher)?;
            unchanged(&fd, &stat, leaf)?;
            Ok(revision(hasher))
        }
    }
}

/// The revision of every artifact `document` places, computed in `workspace`, in document order.
///
/// # Errors
/// The first placed path that is not digested, naming the artifact and the path.
pub fn placements(
    workspace: &Path,
    document: &BindingsDocument,
) -> Result<Vec<ArtifactPlacement>, RevisionRefusal> {
    document
        .artifacts
        .iter()
        .map(|placed| {
            content_revision(workspace, &placed.path)
                .map(|revision| ArtifactPlacement {
                    artifact: placed.artifact.clone(),
                    path: placed.path.clone(),
                    revision,
                })
                .map_err(|(reason, detail)| RevisionRefusal {
                    reason,
                    artifact: placed.artifact.clone(),
                    path: placed.path.clone(),
                    detail,
                })
        })
        .collect()
}

/// The revision each placement names, by artifact: the revisions a step writes.
pub fn revisions_of(placements: &[ArtifactPlacement]) -> BTreeMap<String, String> {
    placements
        .iter()
        .map(|placement| (placement.artifact.clone(), placement.revision.clone()))
        .collect()
}

/// `revision: <detail>` of a refusal, naming the artifact and its path.
pub(crate) fn describe(refusal: &RevisionRefusal) -> String {
    format!(
        "{:?}: the artifact {} at {}: {}",
        refusal.reason, refusal.artifact, refusal.path, refusal.detail
    )
}

/// The segments of `path` to open in turn, empty for the workspace itself.
fn segments(path: &str) -> Result<Vec<&str>, Refused> {
    crate::workspace_path(path).map_err(|why| {
        refused(
            RevisionRefusalReason::OutsideWorkspace,
            format!("the path {path:?} {why}"),
        )
    })?;
    Ok(path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect())
}

/// What one directory entry is, examined without following it.
enum Entry {
    Absent,
    Link(Vec<u8>),
    File(OwnedFd, Stat),
    Directory(OwnedFd, Stat),
}

/// The entry `name` of `dir`: examined first, never followed, and opened only when it is a file
/// or a directory, which must then be the entry examined.
fn entry<P: rustix::path::Arg + Copy + std::fmt::Debug>(
    dir: &OwnedFd,
    name: P,
) -> Result<Entry, Refused> {
    let unreadable = |error: Errno| {
        refused(
            RevisionRefusalReason::Unreadable,
            format!("{name:?} cannot be read: {error}"),
        )
    };
    let examined = match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(Entry::Absent),
        Err(error) => return Err(unreadable(error)),
    };
    let kind = FileType::from_raw_mode(examined.st_mode);
    let flags = match kind {
        FileType::Symlink => {
            return match rustix::fs::readlinkat(dir, name, Vec::new()) {
                Ok(target) => Ok(Entry::Link(target.into_bytes())),
                Err(Errno::NOENT) => Err(changed(name)),
                Err(error) => Err(unreadable(error)),
            };
        }
        FileType::RegularFile => OFlags::RDONLY,
        FileType::Directory => OFlags::RDONLY | OFlags::DIRECTORY,
        other => {
            return Err(refused(
                RevisionRefusalReason::Unreadable,
                format!(
                    "{name:?} is neither a file, a directory nor a link ({other:?}); it is not opened"
                ),
            ));
        }
    };
    let fd = match rustix::fs::openat(
        dir,
        name,
        flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        // Replaced since it was examined (by a link, a directory, nothing).
        Err(Errno::NOENT | Errno::LOOP | Errno::NOTDIR) => return Err(changed(name)),
        Err(error) => return Err(unreadable(error)),
    };
    let opened = rustix::fs::fstat(&fd).map_err(unreadable)?;
    if (opened.st_dev, opened.st_ino) != (examined.st_dev, examined.st_ino)
        || FileType::from_raw_mode(opened.st_mode) != kind
    {
        return Err(changed(name));
    }
    Ok(if kind == FileType::Directory {
        Entry::Directory(fd, opened)
    } else {
        Entry::File(fd, opened)
    })
}

fn changed(name: impl std::fmt::Debug) -> Refused {
    refused(
        RevisionRefusalReason::ChangedWhileRead,
        format!("{name:?} changed while it was read"),
    )
}

/// The bytes and entries one placed path has used of its bounds.
#[derive(Default)]
struct Budget {
    bytes: u64,
    entries: usize,
}

impl Budget {
    fn take_bytes(&mut self, bytes: u64) -> Result<(), Refused> {
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bytes > MAX_ARTIFACT_BYTES {
            return Err(refused(
                RevisionRefusalReason::TooLarge,
                format!("more than {MAX_ARTIFACT_BYTES} bytes"),
            ));
        }
        Ok(())
    }

    fn take_entry(&mut self) -> Result<(), Refused> {
        self.entries += 1;
        if self.entries > MAX_TREE_ENTRIES {
            return Err(refused(
                RevisionRefusalReason::TooLarge,
                format!("more than {MAX_TREE_ENTRIES} entries"),
            ));
        }
        Ok(())
    }
}

/// The members of a stat that move when a file or directory is changed.
fn moved(stat: &Stat) -> impl PartialEq + use<> {
    (
        stat.st_dev,
        stat.st_ino,
        stat.st_size,
        stat.st_mtime,
        stat.st_mtime_nsec,
        stat.st_ctime,
        stat.st_ctime_nsec,
    )
}

/// Whether `fd` is as `before` described it.
fn unchanged(fd: &OwnedFd, before: &Stat, name: impl std::fmt::Debug) -> Result<(), Refused> {
    let after = rustix::fs::fstat(fd).map_err(|error| {
        refused(
            RevisionRefusalReason::Unreadable,
            format!("{name:?} cannot be read: {error}"),
        )
    })?;
    if moved(&after) != moved(before) {
        return Err(changed(name));
    }
    Ok(())
}

/// Feeds the bytes of the file `fd`, described by `stat`, to `hasher`.
fn read_file(
    fd: OwnedFd,
    stat: &Stat,
    budget: &mut Budget,
    hasher: &mut Sha256,
) -> Result<(), Refused> {
    let size = u64::try_from(stat.st_size).unwrap_or(u64::MAX);
    budget.take_bytes(size)?;
    let mut file = File::from(fd);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut read: u64 = 0;
    loop {
        let count = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(refused(
                    RevisionRefusalReason::Unreadable,
                    format!("a file cannot be read: {error}"),
                ));
            }
        };
        read += count as u64;
        if read > size {
            return Err(changed("a file"));
        }
        hasher.update(&buffer[..count]);
    }
    if read != size {
        return Err(changed("a file"));
    }
    unchanged(&OwnedFd::from(file), stat, "a file")
}

fn tree_hasher() -> Sha256 {
    Sha256::new_with_prefix(TREE_HEADER)
}

/// One entry of a tree: its kind, its relative path, a NUL byte and its length.
fn entry_header(hasher: &mut Sha256, kind: u8, path: &[u8], length: u64) {
    hasher.update([kind]);
    hasher.update(path);
    hasher.update([0]);
    hasher.update(length.to_be_bytes());
}

/// Feeds every entry below the directory `dir`, whose path in the tree is `prefix`, to `hasher`.
fn walk(
    dir: &OwnedFd,
    prefix: &[u8],
    depth: usize,
    budget: &mut Budget,
    hasher: &mut Sha256,
) -> Result<(), Refused> {
    if depth >= MAX_TREE_DEPTH {
        return Err(refused(
            RevisionRefusalReason::TooLarge,
            format!("more than {MAX_TREE_DEPTH} levels"),
        ));
    }
    let listing = rustix::fs::Dir::read_from(dir).map_err(|error| {
        refused(
            RevisionRefusalReason::Unreadable,
            format!("a directory cannot be listed: {error}"),
        )
    })?;
    let mut names = Vec::new();
    for item in listing {
        let item = item.map_err(|error| {
            refused(
                RevisionRefusalReason::Unreadable,
                format!("a directory cannot be listed: {error}"),
            )
        })?;
        let name = item.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        budget.take_entry()?;
        names.push(item.file_name().to_owned());
    }
    names.sort();
    for name in names {
        let mut path = prefix.to_vec();
        if !path.is_empty() {
            path.push(b'/');
        }
        path.extend_from_slice(name.to_bytes());
        match entry(dir, name.as_c_str())? {
            Entry::Absent => return Err(changed(name)),
            Entry::Link(target) => {
                budget.take_bytes(target.len() as u64)?;
                entry_header(hasher, b'l', &path, target.len() as u64);
                hasher.update(&target);
            }
            Entry::File(fd, stat) => {
                entry_header(
                    hasher,
                    b'f',
                    &path,
                    u64::try_from(stat.st_size).unwrap_or(u64::MAX),
                );
                read_file(fd, &stat, budget, hasher)?;
            }
            Entry::Directory(fd, stat) => {
                entry_header(hasher, b'd', &path, 0);
                walk(&fd, &path, depth + 1, budget, hasher)?;
                unchanged(&fd, &stat, &name)?;
            }
        }
    }
    Ok(())
}

/// `t` and the first 16 hex digits of the digest.
fn revision(hasher: Sha256) -> String {
    let digest = hasher.finalize();
    digest[..8].iter().fold(String::from("t"), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// Why the placements of a case were not written ([`PlacedRevisions::refresh`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    /// The case is held under another protocol than the one the document binds.
    CaseOfAnotherProtocol {
        /// The protocol the case is held under.
        held: String,
    },
    /// A placed path was not digested; nothing was written.
    Refused(RevisionRefusal),
    /// The governor did not take the moves; nothing was written.
    Governor(StepError),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CaseOfAnotherProtocol { held } => write!(
                f,
                "the case is held under {held}, not the protocol the bindings document binds"
            ),
            Self::Refused(refusal) => f.write_str(&describe(refusal)),
            Self::Governor(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RefreshError {}

/// The governor a host hands Commission's loop for cases whose artifacts `document` places in
/// `workspace`: [`CanonGovernor`]'s answers, with every placement computed and its moves written
/// before each `frontier` and `completion` ([`PlacedRevisions::refresh`]). A placement that cannot
/// be computed or written makes the evaluation `GovernorUnavailable`, so nothing is decided on a
/// revision Loom could not compute.
pub struct PlacedRevisions<'a, S, T = NoEvaluationTime> {
    governor: &'a CanonGovernor<S, T>,
    document: &'a BindingsDocument,
    workspace: PathBuf,
}

impl<S, T> std::fmt::Debug for PlacedRevisions<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlacedRevisions")
            .field("protocol", &self.document.protocol)
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl<'a, S: FallibleCaseStore, T: EvaluationTime> PlacedRevisions<'a, S, T> {
    /// The governor over `governor` for `document`'s placements in `workspace`.
    pub fn new(
        governor: &'a CanonGovernor<S, T>,
        document: &'a BindingsDocument,
        workspace: &Path,
    ) -> Self {
        Self {
            governor,
            document,
            workspace: workspace.to_owned(),
        }
    }

    /// Computes every placement and writes, in one store write, each revision that differs from
    /// the one `case` holds; returns the moves written.
    ///
    /// # Errors
    /// [`RefreshError`]; nothing was written.
    pub fn refresh(&self, case: &CaseId) -> Result<Vec<RevisionMove>, RefreshError> {
        let held = self
            .governor
            .protocol(case)
            .map_err(|error| RefreshError::Governor(StepError::from(error)))?;
        if held != self.document.protocol {
            return Err(RefreshError::CaseOfAnotherProtocol { held });
        }
        let placed = placements(&self.workspace, self.document).map_err(RefreshError::Refused)?;
        self.governor
            .record_step(case, &revisions_of(&placed), Vec::new())
            .map(|written| written.moves)
            .map_err(RefreshError::Governor)
    }

    fn refreshed(&self, case: &CaseId) -> Result<(), GovernorError> {
        self.refresh(case).map(|_| ()).map_err(|error| match error {
            RefreshError::Governor(StepError::UnknownCase) => GovernorError::UnknownCase,
            _ => GovernorError::GovernorUnavailable,
        })
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> Governor for PlacedRevisions<'_, S, T> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.governor.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.refreshed(case)?;
        self.governor.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.refreshed(case)?;
        self.governor.completion(case)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_revision_of_abc_is_its_sha256_prefixed_with_t() {
        assert_eq!(
            revision(Sha256::new_with_prefix(b"abc")),
            "tba7816bf8f01cfea"
        );
    }

    #[test]
    fn a_path_leaving_the_workspace_is_refused_before_anything_is_opened() {
        for path in ["/etc/passwd", "../x", "a/../../x", ""] {
            let (reason, _) = segments(path).unwrap_err();
            assert_eq!(reason, RevisionRefusalReason::OutsideWorkspace, "{path}");
        }
        assert_eq!(segments("./a//b/").unwrap(), vec!["a", "b"]);
    }

    fn opened(dir: &Path, name: &str) -> Entry {
        let fd = rustix::fs::open(
            dir,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        entry(&fd, name).unwrap()
    }

    #[test]
    fn a_file_or_a_directory_that_changes_while_it_is_read_is_changed_while_read() {
        // Unit tests get no `CARGO_TARGET_TMPDIR`: the work tree's own `target/tmp`, as the
        // integration tests' scratch.
        let tmp = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        let scratch = tempfile::tempdir_in(tmp).unwrap();
        std::fs::write(scratch.path().join("file"), b"abc").unwrap();
        std::fs::create_dir(scratch.path().join("tree")).unwrap();

        let Entry::File(fd, stat) = opened(scratch.path(), "file") else {
            panic!("not a file");
        };
        // Written to between the examination and the read.
        std::fs::write(scratch.path().join("file"), b"abcdef").unwrap();
        let (reason, _) = read_file(fd, &stat, &mut Budget::default(), &mut Sha256::new())
            .expect_err("a file grown while read");
        assert_eq!(reason, RevisionRefusalReason::ChangedWhileRead);

        let Entry::Directory(fd, stat) = opened(scratch.path(), "tree") else {
            panic!("not a directory");
        };
        std::fs::write(scratch.path().join("tree/new"), b"x").unwrap();
        let (reason, _) = unchanged(&fd, &stat, "tree").expect_err("an entry added while read");
        assert_eq!(reason, RevisionRefusalReason::ChangedWhileRead);
    }
}
