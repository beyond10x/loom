//! The one door every sync of a durable write on the tree path goes through
//! (`story:tree-run-tests-fit-the-budget`): the stores of this crate, the run log, the tree's store
//! and lock, a kept blob and a staged file.
//!
//! [`sync_file`] returns the sync's error, as each file sync did before the door; [`sync_dir`]
//! ignores a directory sync's error, as each directory sync did. Both count what they do:
//! [`counts`] for the process, [`counts_on_this_thread`] for the calling thread, so a test that
//! shares its process with other tests reads only its own writes.
//!
//! # The test switch
//!
//! Only a build with the feature `test-unsynced-store` can skip a sync, and only while the process's
//! own environment holds `B10X_LOOM_TEST_UNSYNCED_STORE=1`: then the door syncs nothing and counts
//! each sync it skipped. Without the feature the variable is never read and nothing is ever
//! skipped. The feature is enabled only from the `[dev-dependencies]` of `b10x-loom-cli`, so it
//! reaches a build only when that build has test targets, and a build without debug assertions
//! (a release build) that holds it fails to compile. A build that holds it carries a marker
//! `loom-xtask unsynced-store` refuses.

use std::cell::Cell;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(feature = "test-unsynced-store", not(debug_assertions)))]
compile_error!(
    "the feature `test-unsynced-store` of b10x-loom-governor is for test builds only: a build \
     without debug assertions (a release build) cannot hold it"
);

/// Syncs made, by every thread of the process.
static SYNCED: AtomicU64 = AtomicU64::new(0);
/// Syncs skipped under the test switch, by every thread of the process. Always 0 without the
/// feature.
static SKIPPED: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ON_THIS_THREAD: Cell<Counts> = const { Cell::new(Counts { synced: 0, skipped: 0 }) };
}

/// How many syncs the door made and skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// Syncs made.
    pub synced: u64,
    /// Syncs skipped under the test switch; 0 in a build without the feature.
    pub skipped: u64,
}

/// The counts of every thread of the process.
pub fn counts() -> Counts {
    Counts {
        synced: SYNCED.load(Ordering::Relaxed),
        skipped: SKIPPED.load(Ordering::Relaxed),
    }
}

/// The counts of the calling thread alone.
pub fn counts_on_this_thread() -> Counts {
    ON_THIS_THREAD.with(Cell::get)
}

/// Syncs `file`, returning the sync's error.
///
/// # Errors
/// The error of [`File::sync_all`].
pub fn sync_file(file: &File) -> io::Result<()> {
    if switched_off() {
        skipped();
        return Ok(());
    }
    synced();
    file.sync_all()
}

/// Syncs the directory `dir`, so a rename or removal in it is durable. A directory that cannot be
/// opened is not synced and nothing is counted; a failed sync is ignored: the rename or removal
/// has happened, and a failed directory sync only weakens how soon it is durable.
pub fn sync_dir(dir: &Path) {
    if switched_off() {
        skipped();
        return;
    }
    if let Ok(opened) = File::open(dir) {
        synced();
        let _ = opened.sync_all();
    }
}

fn synced() {
    SYNCED.fetch_add(1, Ordering::Relaxed);
    ON_THIS_THREAD.with(|counts| {
        let mut now = counts.get();
        now.synced += 1;
        counts.set(now);
    });
}

fn skipped() {
    SKIPPED.fetch_add(1, Ordering::Relaxed);
    ON_THIS_THREAD.with(|counts| {
        let mut now = counts.get();
        now.skipped += 1;
        counts.set(now);
    });
}

/// Writes the process's [`counts`] as one line `{"synced":n,"skipped":n}` appended to the file
/// `B10X_LOOM_TEST_SYNC_COUNTS` names, so a test reads the counts of a `b10x-loom` it started.
/// Does nothing without the feature, or without the variable; an error is ignored.
pub fn write_counts() {
    #[cfg(feature = "test-unsynced-store")]
    if let Some(path) = std::env::var_os(test_switch::COUNTS) {
        use std::io::Write as _;
        let now = counts();
        let line = format!(
            "{{\"synced\":{},\"skipped\":{}}}\n",
            now.synced, now.skipped
        );
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// Whether the test switch is on: never without the feature.
#[cfg(not(feature = "test-unsynced-store"))]
fn switched_off() -> bool {
    false
}

/// Whether the test switch is on: the variable is `1` in the process's own environment.
#[cfg(feature = "test-unsynced-store")]
fn switched_off() -> bool {
    std::hint::black_box(&test_switch::MARKER);
    std::env::var_os(test_switch::SWITCH).is_some_and(|value| value == "1")
}

#[cfg(feature = "test-unsynced-store")]
mod test_switch {
    /// The variable that turns the switch on, read only in a build with the feature.
    pub(super) const SWITCH: &str = "B10X_LOOM_TEST_UNSYNCED_STORE";
    /// The variable naming the file [`super::write_counts`] appends to.
    pub(super) const COUNTS: &str = "B10X_LOOM_TEST_SYNC_COUNTS";
    /// The bytes `loom-xtask unsynced-store` finds in a binary that holds the feature.
    #[used]
    pub(super) static MARKER: [u8; 36] = *b"b10x-loom-test-unsynced-store-marker";
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::OwnedFd;
    use std::path::{Path, PathBuf};

    use b10x_loom_commission::model::responsibility::CaseId;

    use super::{Counts, counts, counts_on_this_thread, sync_dir, sync_file};
    use crate::locked_dir::LockedDir;
    use crate::{CanonGovernor, CaseInputs, FileCaseStore};

    /// The repository root.
    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A fresh scratch directory under the repository's build directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = root().join("target/tmp/durable").join(format!(
            "{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The syncs the calling thread made and skipped while `write` ran.
    fn counted(write: impl FnOnce()) -> Counts {
        let before = counts_on_this_thread();
        write();
        let after = counts_on_this_thread();
        Counts {
            synced: after.synced - before.synced,
            skipped: after.skipped - before.skipped,
        }
    }

    const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: fixture
  revision: 1
artifacts:
  spec: {}
evidence_kinds:
  gate: {}
actions:
  spec.check:
    effect: read
    may_produce:
      - evidence: gate
";

    #[test]
    fn each_governor_write_syncs_its_file_and_directory() {
        let synced = |n| Counts {
            synced: n,
            skipped: 0,
        };

        let dir = scratch("locked");
        let mut opened = None;
        assert_eq!(
            counted(|| opened = Some(LockedDir::open(dir.clone()).unwrap())),
            synced(1),
            "an open syncs its probe"
        );
        let locked = opened.unwrap();
        let file = dir.join("document");
        assert_eq!(
            counted(|| locked
                .locked(|| locked.replace(&file, "text\n"))
                .unwrap()
                .unwrap()),
            synced(2),
            "a replace syncs its file and the directory"
        );
        assert_eq!(
            counted(|| locked.locked(|| locked.remove(&file)).unwrap().unwrap()),
            synced(1),
            "a remove syncs the directory"
        );

        let dir = scratch("case");
        let governor = CanonGovernor::new(FileCaseStore::open(&dir).unwrap())
            .with_protocol_yaml("fixture@1", PROTOCOL)
            .unwrap();
        let case = CaseId("case-1".to_owned());
        governor
            .open_case(
                case.clone(),
                "fixture@1",
                [("spec".to_owned(), "s1".to_owned())].into(),
                CaseInputs::none(),
            )
            .unwrap();
        let store = FileCaseStore::open(&dir).unwrap();
        assert_eq!(
            counted(|| {
                store
                    .update_until(&case, |_| (), &mut |_| std::ops::ControlFlow::Continue(()))
                    .unwrap()
                    .unwrap()
                    .unwrap();
            }),
            synced(2),
            "a FileStore write syncs its file and the directory"
        );
        assert_eq!(counts().skipped, 0, "{:?}", counts());
    }

    #[test]
    fn a_failing_file_sync_is_an_error_with_the_switch_off() {
        assert!(
            std::env::var_os(concat!("B10X_LOOM_TEST_", "UNSYNCED_STORE")).is_none(),
            "this test runs with the switch off"
        );
        let (_reader, writer) = std::io::pipe().unwrap();
        let pipe = fs::File::from(OwnedFd::from(writer));
        let error = sync_file(&pipe).expect_err("a pipe cannot be synced");
        assert_eq!(error.raw_os_error(), Some(22), "EINVAL: {error}");

        let missing = scratch("missing").join("no-such-directory");
        assert_eq!(
            counted(|| sync_dir(&missing)),
            Counts::default(),
            "a directory that cannot be opened is not synced"
        );
    }

    /// Every file under `dir`, recursively, `target` directories left out.
    fn files(dir: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                if entry.file_name() != "target" {
                    files(&entry.path(), found);
                }
            } else if kind.is_file() {
                found.push(entry.path());
            }
        }
    }

    /// The path of `file` under the repository root, with `/` between its parts.
    fn relative(file: &Path) -> String {
        file.strip_prefix(root())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// The crates on the tree path, whose `src/` syncs only through this module.
    const TREE_PATH: [&str; 5] = [
        "loom-governor",
        "loom-runlog",
        "loom-tree",
        "loom-bindings",
        "loom-cli",
    ];

    #[test]
    fn no_sync_outside_the_door() {
        let calls = [
            concat!("sync", "_all"),
            concat!("sync", "_data"),
            concat!("fs", "ync"),
            concat!("fdata", "sync"),
            concat!("sync", "fs"),
        ];
        let mut outside = Vec::new();
        let mut scanned = 0;
        for name in TREE_PATH {
            let mut found = Vec::new();
            files(&root().join("crates").join(name).join("src"), &mut found);
            for file in found {
                let path = relative(&file);
                if path == "crates/loom-governor/src/durable.rs" {
                    continue;
                }
                scanned += 1;
                let text = fs::read_to_string(&file).unwrap_or_default();
                for (number, line) in text.lines().enumerate() {
                    if calls.iter().any(|call| line.contains(call)) {
                        outside.push(format!("{path}:{}: {}", number + 1, line.trim()));
                    }
                }
            }
        }
        assert!(scanned > 20, "only {scanned} source files were scanned");
        assert!(
            outside.is_empty(),
            "a sync outside `loom_governor::durable`; call `durable::sync_file` or \
             `durable::sync_dir` instead:\n{}",
            outside.join("\n")
        );
    }

    /// The files that may name the test switch's variable.
    const MAY_NAME: [&str; 5] = [
        "crates/loom-governor/src/durable.rs",
        "crates/loom-xtask/src/test_step.rs",
        "crates/loom-xtask/src/unsynced_store.rs",
        "crates/loom-cli/tests/tree_run.rs",
        "crates/loom-cli/tests/support/loom.rs",
    ];

    /// The files that may set it on a process they start.
    const MAY_SET: [&str; 2] = [
        "crates/loom-cli/tests/tree_run.rs",
        "crates/loom-cli/tests/support/loom.rs",
    ];

    /// The durability tests: in process, never under the switch.
    const DURABILITY_TESTS: [(&str, &str); 8] = [
        (
            "crates/loom-runlog/tests/run_log.rs",
            "fn a_crash_at_each_step_of_an_append_leaves_n_or_n_plus_1_lines_that_verify",
        ),
        (
            "crates/loom-governor/tests/restart_durability.rs",
            "fn a_case_survives_a_process_restart",
        ),
        (
            "crates/loom-governor/tests/step_write.rs",
            "fn a_crash_at_the_store_write_of_a_step_leaves_neither_or_both",
        ),
        (
            "crates/loom-governor/tests/step_write.rs",
            "fn a_step_written_through_a_file_store_reads_back_after_a_restart",
        ),
        (
            "crates/loom-governor/tests/decision_store.rs",
            "fn a_request_survives_a_process_restart",
        ),
        (
            "crates/loom-governor/tests/decision_store.rs",
            "fn appended_records_survive_a_process_restart_in_order",
        ),
        (
            "crates/loom-governor/tests/child_cases.rs",
            "fn a_restart_between_the_child_opened_and_the_link_recorded_adopts_the_child_and_opens_no_second",
        ),
        (
            "crates/loom-governor/tests/child_cases.rs",
            "fn a_restart_between_the_child_terminated_and_the_outcome_written_is_completed_once",
        ),
    ];

    #[test]
    fn only_the_allowed_files_name_or_set_the_switch() {
        let switch = concat!("B10X_LOOM_TEST_", "UNSYNCED_STORE");
        let setters = [
            concat!(".en", "v("),
            concat!(".en", "vs("),
            concat!("set", "_var("),
            concat!("set", "env("),
        ];
        let mut found = Vec::new();
        files(&root().join("crates"), &mut found);
        files(&root().join(".github"), &mut found);
        for taskfile in ["Taskfile.yml", "Taskfile.commission.yml"] {
            found.push(root().join(taskfile));
        }
        assert!(found.len() > 100, "only {} files were scanned", found.len());

        let mut refused = Vec::new();
        for file in &found {
            let path = relative(file);
            let bytes = fs::read(file).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            let names = text.contains(switch);
            if names && !MAY_NAME.contains(&path.as_str()) {
                refused.push(format!("{path} names {switch}"));
            }
            if !names || MAY_SET.contains(&path.as_str()) {
                continue;
            }
            for (number, line) in text.lines().enumerate() {
                let exported = ["pub ", "pub(crate)"]
                    .iter()
                    .any(|export| line.trim_start().starts_with(export));
                if exported && line.contains(switch) {
                    refused.push(format!("{path}:{}: exports {switch}", number + 1));
                }
                for setter in setters {
                    let mut rest = line;
                    while let Some(at) = rest.find(setter) {
                        rest = &rest[at + setter.len()..];
                        let argument = rest.trim_start();
                        let literal = argument
                            .strip_prefix('"')
                            .and_then(|tail| tail.split('"').next());
                        if literal.is_none_or(|name| name == switch) {
                            refused.push(format!(
                                "{path}:{}: may set {switch}: {}",
                                number + 1,
                                line.trim()
                            ));
                        }
                    }
                }
            }
        }
        for (path, _) in DURABILITY_TESTS {
            let text = fs::read_to_string(root().join(path)).unwrap();
            assert!(!text.contains(switch), "{path} names {switch}");
        }
        for (path, test) in DURABILITY_TESTS {
            let text = fs::read_to_string(root().join(path)).unwrap();
            assert!(text.contains(test), "{path} no longer holds {test}");
        }
        assert!(refused.is_empty(), "{}", refused.join("\n"));
    }
}
