//! Adversary cases for `story:cli-run-holds-suspension` (wave 2026-10-13-w2, U2): the resume
//! context store beside the governor's file stores over one directory, and its read-back.
//!
//! `ResumeContextStore` has its own copy of the lock-and-rename code `loom_governor`'s crate-private
//! `LockedDir` holds, and `LockedDir::open` removes every `.tmp-` file it finds. These cases drive
//! both over one directory at once and check no keep is lost, no temporary file is left, and a
//! kept context reads back exactly.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_intake_slice::resume::{
    AcceptedRoute, AgentRevisionId, CaseId, CatalogDigest, CommissionId, ResumeContextData,
    ResumeContextStore,
};
use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::primitives::{Decimal, Uuid};
use loom_governor::{FileCaseStore, FileSuspensionStore};

fn context(n: usize, intent: &str) -> ResumeContextData {
    ResumeContextData {
        commission_id: CommissionId(Uuid(format!("00000000-0000-4000-8000-{n:012}"))),
        case_id: CaseId(format!("case-{n}")),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
        intent: intent.to_owned(),
        route: AcceptedRoute {
            protocol: "software-change@1".into(),
            intent_revision: "intent-sha256-00".into(),
            catalog: vec![CatalogDigest {
                name: "software-change@1".into(),
                digest: "ab".repeat(32),
            }],
            context_policy: ContextPolicy::Legacy,
            threshold: Decimal("0.5".into()),
        },
        workspace: Some("/work/space".into()),
        test_command: vec![
            "grep".into(),
            "-qx".into(),
            "fixed".into(),
            "check.txt".into(),
        ],
        test_paths: vec!["tests/**".into()],
        max_steps: 6,
        confinement: Backend::None,
        cgroup_root: None,
        model: "lab-agent".into(),
        catalog: Some("/catalog.toml".into()),
    }
}

fn scratch(label: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("adv-u2-{label}-"))
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .expect("a scratch directory")
}

fn temporaries(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".tmp-"))
        .collect()
}

/// Keeps from two threads while two others open the governor's stores over the same directory
/// in a loop (each open removes every `.tmp-` file under the lock): every keep succeeds, every
/// context reads back exactly, and no temporary file is left.
#[test]
fn keeping_contexts_while_governor_stores_open_over_the_directory_loses_nothing() {
    let dir = scratch("interleave");
    let path = dir.path().to_owned();
    let store = Arc::new(ResumeContextStore::open(&path).unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let openers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut opened = 0_u64;
                while !stop.load(Ordering::SeqCst) {
                    FileCaseStore::open(&path).expect("the case store opens");
                    FileSuspensionStore::open(&path).expect("the suspension store opens");
                    opened += 1;
                }
                opened
            })
        })
        .collect();
    let keepers: Vec<_> = (0..2)
        .map(|half| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for n in (half * 1000)..(half * 1000 + 150) {
                    store
                        .keep(&context(n, "interleaved"))
                        .unwrap_or_else(|error| panic!("keep {n}: {error}"));
                }
            })
        })
        .collect();
    for keeper in keepers {
        keeper.join().expect("the keeper finished");
    }
    stop.store(true, Ordering::SeqCst);
    let opened: u64 = openers.into_iter().map(|o| o.join().unwrap()).sum();
    assert!(
        opened > 0,
        "the governor stores opened while contexts were kept"
    );
    let kept = store.list().unwrap();
    assert_eq!(kept.len(), 300);
    for half in 0..2 {
        for n in (half * 1000)..(half * 1000 + 150) {
            let expected = context(n, "interleaved");
            assert_eq!(store.get(&expected.commission_id).unwrap(), Some(expected));
        }
    }
    assert!(temporaries(&path).is_empty(), "{:?}", temporaries(&path));
}

/// A temporary file a dead resume write left (`.tmp-<pid>-resume-<n>`) is removed by the next
/// governor store that opens, as the resume store's documentation says, and does not hide a
/// context.
#[test]
fn a_resume_temporary_a_dead_write_left_is_removed_by_the_governor_stores() {
    let dir = scratch("stale");
    let store = ResumeContextStore::open(dir.path()).unwrap();
    std::fs::write(
        dir.path().join(".tmp-4194304-resume-0"),
        "{\"commission_id\":",
    )
    .unwrap();
    FileCaseStore::open(dir.path()).unwrap();
    assert!(temporaries(dir.path()).is_empty());
    store.keep(&context(7, "after a crash")).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
}

/// A context reads back exactly for text a model never supplies but an operator may: control
/// characters, a quote, a backslash, a non-BMP character, the empty string.
#[test]
fn a_context_reads_back_exactly_for_awkward_text() {
    let dir = scratch("exact");
    let store = ResumeContextStore::open(dir.path()).unwrap();
    for (n, intent) in [
        "",
        "\u{0}\u{1f}\"\\",
        "tab\there\r\nline",
        "\u{1F600} \u{2028}",
    ]
    .into_iter()
    .enumerate()
    {
        let mut kept = context(n, intent);
        kept.workspace = None;
        kept.catalog = None;
        kept.cgroup_root = Some("/sys/fs/cgroup/user.slice/\u{e9}".into());
        kept.max_steps = i64::MAX;
        kept.route.threshold = Decimal("1e-7".into());
        store.keep(&kept).unwrap();
        assert_eq!(store.get(&kept.commission_id).unwrap(), Some(kept));
    }
}
