//! The slice opens its case through the governor (story `case-frontier`).
//!
//! The case a `software.change/1` run starts with holds a hash of the intent text as the `intent`
//! revision, the workspace's git `HEAD` as the `implementation` revision and `r0` for every other
//! artifact the protocol declares. The governor, not the slice, issues the frontier.
//!
//! The fixture workspace is a git repository created under `CARGO_TARGET_TMPDIR`. The test's own
//! git calls, which build the fixture, run with no system or global configuration. The git that
//! `case::open` and `case::report_head` run reads the operator's normal git configuration, as the
//! slice does in use.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::responsibility::{ActionStatus, FrontierAction};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_intake_slice::case;
use loom_governor::{CanonGovernor, MemoryCaseStore};

const PICK: &str = "software-change@1";
const INTENT: &str = "make the failing test pass";

/// Every artifact `software.change/1` declares besides `intent` and `implementation`
/// (ELS `protocols/software-change/1.yaml` at `ac7dd03`, the revision the governor pins). The
/// governor refuses a case that omits a declared artifact or names an undeclared one, so this list
/// cannot drift from the protocol without the case failing to open.
const OTHER_ARTIFACTS: [&str; 4] = ["system_specification", "plan", "release", "deployment"];

#[test]
fn the_slice_case_starts_where_the_workspace_is() {
    let workspace = Workspace::new("start");
    workspace.commit("README.md", "first\n");
    let head = workspace.head();

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, workspace.path())
        .expect("a software-change@1 case opens on a git workspace");

    let revisions = governor
        .revisions(&opened)
        .expect("the governor holds the case the slice opened");
    let intent_revision = revisions
        .get("intent")
        .cloned()
        .expect("the case holds an intent revision");
    let mut expected: BTreeMap<String, String> = OTHER_ARTIFACTS
        .iter()
        .map(|artifact| ((*artifact).to_owned(), "r0".to_owned()))
        .collect();
    expected.insert("implementation".to_owned(), head.clone());
    expected.insert("intent".to_owned(), intent_revision.clone());
    assert_eq!(
        revisions, expected,
        "the case starts at HEAD with r0 for every artifact but intent and implementation"
    );

    // The intent revision is a hash of the intent text: the same text gives the same revision, a
    // different text a different one, and it is neither `r0` nor the workspace's HEAD.
    assert_ne!(intent_revision, "r0");
    assert_ne!(intent_revision, head);
    let again = case::open(&governor, PICK, INTENT, workspace.path()).expect("a second case opens");
    assert_ne!(again, opened, "the governor opens a new case");
    assert_eq!(
        governor.revisions(&again).expect("held")["intent"],
        intent_revision,
        "the same intent text gives the same intent revision"
    );
    let other = case::open(&governor, PICK, "rename the config flag", workspace.path())
        .expect("a case for another intent opens");
    assert_ne!(
        governor.revisions(&other).expect("held")["intent"],
        intent_revision,
        "a different intent text gives a different intent revision"
    );

    // The first frontier, as the governor issues it.
    let before = governor
        .current_revision(&opened)
        .expect("the case has a revision");
    let frontier = governor
        .frontier(&opened)
        .expect("the governor issues a frontier")
        .into_data();
    assert_eq!(frontier.case_id, opened);
    assert_eq!(frontier.case_revision, before);
    let actions = by_name(&frontier.actions);
    assert_eq!(actions["repository.edit"].status, ActionStatus::Admissible);
    assert_eq!(actions["tests.run"].status, ActionStatus::Admissible);
    let merge = actions["repository.merge"];
    assert_eq!(merge.status, ActionStatus::Blocked);
    assert!(
        merge
            .reasons
            .iter()
            .any(|reason| reason.contains("implementation.verified")),
        "merge is blocked on `implementation.verified`: {:?}",
        merge.reasons
    );

    // A commit in the workspace, reported through the governor's `update_revision`, raises the
    // case revision by one and moves `implementation` to the new HEAD.
    workspace.commit("src.txt", "changed\n");
    let new_head = workspace.head();
    assert_ne!(new_head, head);
    let reported = case::report_head(&governor, &opened, workspace.path())
        .expect("the new HEAD is reported to the governor");
    let after = governor
        .current_revision(&opened)
        .expect("the case has a revision");
    assert_eq!(after, before + 1, "the case revision rises by one");
    assert_eq!(reported, after, "report_head returns the new case revision");
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        new_head,
        "the case's implementation is the new HEAD"
    );
    assert_eq!(
        governor
            .frontier(&opened)
            .expect("frontier")
            .into_data()
            .case_revision,
        after,
        "the next frontier is issued for the new revision"
    );
}

fn by_name(actions: &[FrontierAction]) -> BTreeMap<&str, &FrontierAction> {
    actions
        .iter()
        .map(|action| (action.action.as_str(), action))
        .collect()
}

/// A git repository under the test's scratch directory, with a local identity.
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("slice-case-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create the fixture workspace");
        let workspace = Self { root };
        workspace.git(&["init", "--quiet"]);
        workspace.git(&["config", "user.name", "Fixture Author"]);
        workspace.git(&["config", "user.email", "fixture@example.invalid"]);
        workspace.git(&["config", "commit.gpgsign", "false"]);
        workspace
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn commit(&self, file: &str, contents: &str) {
        std::fs::write(self.root.join(file), contents).expect("write a fixture file");
        self.git(&["add", "--", file]);
        self.git(&["commit", "--quiet", "--message", file]);
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
