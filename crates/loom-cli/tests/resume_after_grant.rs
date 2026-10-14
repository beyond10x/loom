//! Acceptance for `story:cli-resume-after-grant`: after `run --state <dir>` left a commission's
//! Suspension in `<dir>` (`story:cli-run-holds-suspension`), `b10x-loom approve <commission>
//! <capability> --state <dir>` writes a grant record (`intake.routing.Grant`) and performs
//! nothing, and `b10x-loom resume <commission> --state <dir>` performs the suspended
//! `repository.merge` once, as a merge hand-off record (`intake.routing.MergeHandoff`) that moves
//! no Git ref, releases the Suspension and runs on. Each refusal of a resume has its own exit
//! status.
//!
//! Every command runs the binary, a separate process. The stores are read back through the stores
//! themselves where they exist (`FileSuspensionStore`, `ResumeContextStore`); the hand-off is read
//! as the JSON document `resume` leaves in `<dir>`, one `merge-*.json` file per handed-off request.
//! The models are the anonymous Chat Completions routes of the fixture catalog
//! (`fixtures/catalog-route/catalog.toml`) at a loopback socket this file opens, scripted in the
//! recorded stream format `run_state.rs` replays; no case reaches the network, a credential or a
//! Codex login (`HOME` and `CODEX_HOME` are an empty scratch directory). Before each command the
//! catalog file the resume context names is rewritten to point at that command's socket.
//!
//! The workspace is a git repository with one failing check (`grep -qx fixed check.txt`, no
//! shell). The agent edits the check, runs the tests, then proposes `repository.merge`, which the
//! frontier lists as needing approval: the run stops `ApprovalRequired` and the commission holds
//! its Suspension.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{CommissionId, SuspensionState};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use loom_governor::FileSuspensionStore;
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const INTENT: &str = "Make the failing check pass.";
const TEST_CMD: &str = "grep -qx fixed check.txt";

/// The exit status of a resume refused `NothingSuspended`.
const NOTHING_SUSPENDED: i32 = 10;
/// The exit status of a resume refused `NotGranted`.
const NOT_GRANTED: i32 = 16;
/// The exit status of a resume refused `CaseMoved`.
const CASE_MOVED: i32 = 14;
/// The exit status of a run or a resume with a state directory that stopped `ApprovalRequired`
/// with nothing suspended.
const NOTHING_HELD: i32 = 4;

#[test]
fn resume_after_approve_merges_once() {
    let scratch = Scratch::new("merges-once");
    let held = scratch.suspend();

    // Approval writes the grant and performs nothing.
    let approved = scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    assert_eq!(scratch.files(&held.state, "grant-"), 1, "one grant record");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "approve merges nothing"
    );
    assert_eq!(
        held.suspension_state(),
        Some(SuspensionState::Pending),
        "approve leaves the Suspension as it was"
    );

    // The resume performs the merge as a hand-off, releases the Suspension and runs on: the agent
    // is asked again after the merge.
    let refs = scratch.refs();
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    let stdout = String::from_utf8_lossy(&resumed.stdout);
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert_eq!(
        calls,
        ["select_action", "action_arguments"],
        "the run went on after the merge, and classified nothing\n{}",
        shown(&resumed)
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("step 1: repository.merge")),
        "the first step is the merge\n{stdout}"
    );
    // The run stopped at the merge's gate again, at the HEAD it handed off: nothing is left to
    // approve, and the stop line says so instead of reading as an approvable gate.
    assert_eq!(
        stdout.lines().last(),
        Some(
            format!(
                "stopped: MergeHandedOff ({}, HEAD {})",
                held.action_request_id,
                scratch.head()
            )
            .as_str()
        ),
        "{}",
        shown(&resumed)
    );
    assert_eq!(held.suspension_state(), None, "the Suspension is released");

    // Exactly one hand-off, for the granted request, naming the workspace and its HEAD; no Git ref
    // moved.
    let handoffs = scratch.documents(&held.state, "merge-");
    assert_eq!(handoffs.len(), 1, "one merge hand-off: {handoffs:?}");
    let handoff = &handoffs[0];
    assert_eq!(handoff["commission_id"], json!(held.commission));
    assert_eq!(handoff["action_request_id"], json!(held.action_request_id));
    assert_eq!(handoff["workspace"], json!(held.workspace));
    assert_eq!(handoff["head"], json!(scratch.head()));
    assert_eq!(scratch.refs(), refs, "the resume moved no Git ref");

    // A second resume finds nothing suspended and performs nothing again.
    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let again = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        again.status.code(),
        Some(NOTHING_SUSPENDED),
        "{}",
        shown(&again)
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("NothingSuspended"),
        "{}",
        shown(&again)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        1,
        "still one hand-off"
    );
}

#[test]
fn resume_without_approve_is_not_granted_and_merges_nothing() {
    let scratch = Scratch::new("not-granted");
    let held = scratch.suspend();
    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("NotGranted"),
        "{}",
        shown(&resumed)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

#[test]
fn a_grant_for_another_request_does_not_allow_this_one() {
    let scratch = Scratch::new("other-request");
    let held = scratch.suspend();
    let approved = scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    // The grant names another request than the one the Suspension holds.
    let grant = scratch.paths(&held.state, "grant-");
    assert_eq!(grant.len(), 1, "one grant record");
    let mut document: Value =
        serde_json::from_str(&std::fs::read_to_string(&grant[0]).unwrap()).unwrap();
    assert_eq!(document["action_request_id"], json!(held.action_request_id));
    document["action_request_id"] = json!("00000000-0000-4000-8000-0000000000ff");
    std::fs::write(&grant[0], format!("{document:#}\n")).unwrap();

    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

#[test]
fn approve_refuses_another_capability_and_a_commission_with_nothing_suspended() {
    let scratch = Scratch::new("approve-refusals");
    let held = scratch.suspend();

    let other = scratch.command(&[
        "approve",
        &held.commission,
        "implementation.edit",
        "--state",
        held.state_text(),
    ]);
    assert_ne!(other.status.code(), Some(0), "{}", shown(&other));
    assert_ne!(other.status.code(), None, "{}", shown(&other));

    let nobody = scratch.command(&[
        "approve",
        "00000000-0000-4000-8000-0000000000aa",
        "repository.merge",
        "--state",
        held.state_text(),
    ]);
    assert_eq!(
        nobody.status.code(),
        Some(NOTHING_SUSPENDED),
        "{}",
        shown(&nobody)
    );
    assert_eq!(
        scratch.files(&held.state, "grant-"),
        0,
        "no grant is written"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

fn approve(scratch: &Scratch, held: &Held) -> Output {
    scratch.command(&[
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ])
}

/// A commit made in the workspace after the approval moves the case: `resume` reports the
/// workspace's `HEAD` to the governor first, so the resume is refused `CaseMoved`, calls no model
/// and hands nothing off.
#[test]
fn a_commit_after_approve_is_refused_case_moved_and_hands_off_nothing() {
    let scratch = Scratch::new("case-moved");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    std::fs::write(scratch.workspace.join("later.txt"), "not reviewed\n").unwrap();
    scratch.git(&["add", "--all"]);
    scratch.git(&["commit", "--quiet", "--message", "an unreviewed change"]);

    let (base_url, server) = Script::serve(vec![]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    let calls = server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(CASE_MOVED),
        "{}",
        shown(&resumed)
    );
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("CaseMoved"),
        "{}",
        shown(&resumed)
    );
    assert!(calls.is_empty(), "no model is called: {calls:?}");
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
}

/// Acceptance for `story:handoff-holds-no-suspension`: after `approve` and `resume` handed off a
/// merge at `HEAD` `h`, a merge the resumed agent proposes again at `h` holds no Suspension. The
/// run ends `stopped: MergeHandedOff (<request>, HEAD h)`, `<DIR>` holds no Suspension and no
/// resume context for the commission, and `approve` is refused `NothingSuspended`, writing
/// nothing.
#[test]
fn a_merge_proposed_again_at_a_handed_off_head_holds_no_suspension_and_leaves_nothing_to_approve() {
    let scratch = Scratch::new("already-handed-off");
    let held = scratch.suspend();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "repository.merge"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert_eq!(
        String::from_utf8_lossy(&resumed.stdout).lines().last(),
        Some(
            format!(
                "stopped: MergeHandedOff ({}, HEAD {})",
                held.action_request_id,
                scratch.head()
            )
            .as_str()
        ),
        "{}",
        shown(&resumed)
    );
    assert_eq!(scratch.files(&held.state, "merge-"), 1);
    assert_eq!(
        held.suspension_state(),
        None,
        "no Suspension is held for the re-proposed merge\n{}",
        shown(&resumed)
    );
    assert!(
        ResumeContextStore::open(&held.state)
            .unwrap()
            .list()
            .unwrap()
            .contexts
            .is_empty(),
        "no resume context is left for the commission\n{}",
        shown(&resumed)
    );

    let grant = scratch.paths(&held.state, "grant-");
    let before = std::fs::read(&grant[0]).unwrap();
    let again = approve(&scratch, &held);
    assert_eq!(
        again.status.code(),
        Some(NOTHING_SUSPENDED),
        "{}",
        shown(&again)
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("NothingSuspended"),
        "{}",
        shown(&again)
    );
    assert_eq!(
        std::fs::read(&grant[0]).unwrap(),
        before,
        "the grant is not replaced"
    );
    assert_eq!(scratch.files(&held.state, "grant-"), 1);
    assert_eq!(scratch.files(&held.state, "merge-"), 1);
}

/// New work after a hand-off, a new `HEAD`, is held at its own gate, approved and handed off at
/// that `HEAD`.
#[test]
fn new_work_after_a_hand_off_is_approved_and_handed_off_at_its_own_head() {
    let scratch = Scratch::new("new-work");
    let held = scratch.suspend_with(8);
    let first_head = scratch.head();
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));

    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "implementation.edit"})),
        agent(
            "action_arguments",
            json!({
                "files": [{"path": "notes.txt", "contents": "more work\n"}],
                "message": "more work"
            }),
        ),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
        agent("select_action", json!({"action": "repository.merge"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let first = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(first.status.code(), Some(0), "{}", shown(&first));
    let second_head = scratch.head();
    assert_ne!(first_head, second_head, "{}", shown(&first));
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{}",
        shown(&first)
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));

    let again = approve(&scratch, &held);
    assert_eq!(again.status.code(), Some(0), "{}", shown(&again));
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let second = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(second.status.code(), Some(0), "{}", shown(&second));
    let mut heads: Vec<Value> = scratch
        .documents(&held.state, "merge-")
        .iter()
        .map(|handoff| handoff["head"].clone())
        .collect();
    heads.sort_by_key(|head| head.as_str().map(str::to_owned));
    let mut expected = vec![json!(first_head), json!(second_head)];
    expected.sort_by_key(|head| head.as_str().map(str::to_owned));
    assert_eq!(heads, expected, "{}", shown(&second));
}

/// A run with a state directory that stops at the merge's gate without holding a Suspension (the
/// agent ran the tests again instead of proposing the merge) does not read as approvable: its stop
/// line says nothing is suspended, and it exits with its own status. Without `--state` the line
/// and the status are unchanged.
#[test]
fn a_run_at_the_gate_with_nothing_suspended_says_so() {
    let scratch = Scratch::new("nothing-held");
    let plain = scratch.run(5, false, "tests.run");
    assert_eq!(plain.status.code(), Some(0), "{}", shown(&plain));
    assert_eq!(
        String::from_utf8_lossy(&plain.stdout).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge)"),
        "{}",
        shown(&plain)
    );

    let scratch = Scratch::new("nothing-held-state");
    let output = scratch.run(5, true, "tests.run");
    assert_eq!(
        output.status.code(),
        Some(NOTHING_HELD),
        "{}",
        shown(&output)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge), nothing suspended"),
        "{}",
        shown(&output)
    );
    let state = scratch.root.join("state");
    assert!(
        ResumeContextStore::open(&state)
            .unwrap()
            .list()
            .unwrap()
            .contexts
            .is_empty(),
        "no context is kept"
    );
}

/// The same for a resume: after the hand-off the agent commits new work and runs the tests, but
/// proposes no merge; the new `HEAD` was not handed off and nothing is suspended.
#[test]
fn a_resume_at_the_gate_with_nothing_suspended_says_so() {
    let scratch = Scratch::new("resume-nothing-held");
    let held = scratch.suspend_with(8);
    let approved = approve(&scratch, &held);
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let (base_url, server) = Script::serve(vec![
        agent("select_action", json!({"action": "implementation.edit"})),
        agent(
            "action_arguments",
            json!({
                "files": [{"path": "notes.txt", "contents": "more work\n"}],
                "message": "more work"
            }),
        ),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]);
    scratch.catalog(&base_url);
    let resumed = scratch.command(&["resume", &held.commission, "--state", held.state_text()]);
    server.finish();
    assert_eq!(
        resumed.status.code(),
        Some(NOTHING_HELD),
        "{}",
        shown(&resumed)
    );
    assert_eq!(
        String::from_utf8_lossy(&resumed.stdout).lines().last(),
        Some("stopped: ApprovalRequired (repository.merge), nothing suspended"),
        "{}",
        shown(&resumed)
    );
    assert_eq!(held.suspension_state(), None);
}

// ---------------------------------------------------------------------------------------------
// Acceptance for `story:mandate-approval-on-resume`: with `--mandate <FILE>`, `approve` records a
// Mandate approval of the held request in the state directory's approval log
// (`mandate-approvals.jsonl`) and the grant names it, and `resume` performs the request only when
// Mandate's `check_with_approval` clears and consumes it, keeping the consumption in the log first.

/// The exit status of `approve` or `resume` that could not use its input or state directory.
const FAILED: i32 = 1;

/// A recorded, unexpired approval clears the held request once: the resume hands the merge off,
/// and the log keeps the approval and, after it, the event that consumed it.
#[test]
fn a_recorded_approval_clears_the_held_request_once() {
    let scratch = Scratch::new("mandate-clears");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);

    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let log = scratch.mandate_log(&held.state);
    assert_eq!(log.len(), 1, "one approval recorded: {log:?}");
    assert_eq!(log[0]["kind"], json!("ApprovalRecorded"));
    assert_eq!(log[0]["action"], json!("repository.merge"));
    assert_eq!(log[0]["action_request_id"], json!(held.action_request_id));
    assert_eq!(log[0]["approver"], json!(APPROVER));
    assert_eq!(log[0]["subject"], json!(SUBJECT));
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "approve merges nothing"
    );

    let refs = scratch.refs();
    let resumed = resume_with(&scratch, &held, Some(&host), script_after_merge());
    assert_eq!(resumed.status.code(), Some(0), "{}", shown(&resumed));
    assert!(
        String::from_utf8_lossy(&resumed.stdout)
            .lines()
            .any(|line| line.starts_with("step 1: repository.merge")),
        "{}",
        shown(&resumed)
    );
    let handoffs = scratch.documents(&held.state, "merge-");
    assert_eq!(handoffs.len(), 1, "one merge hand-off: {handoffs:?}");
    assert_eq!(
        handoffs[0]["action_request_id"],
        json!(held.action_request_id)
    );
    assert_eq!(scratch.refs(), refs, "the resume moved no Git ref");
    assert_eq!(held.suspension_state(), None, "the Suspension is released");

    let log = scratch.mandate_log(&held.state);
    assert_eq!(log.len(), 2, "the consumption is kept: {log:?}");
    assert_eq!(log[1]["kind"], json!("ApprovedDecisionRecorded"));
    assert_eq!(log[1]["approval"], log[0]["approval"]);
    assert_eq!(log[1]["action_request_id"], json!(held.action_request_id));
    assert_eq!(log[1]["decision"]["allowed"], json!(true));
}

/// A second resume with the same approval is refused by Mandate and performs nothing, across
/// separate processes. The first resume consumed the approval and performed the merge; the second
/// runs over the state directory as it was before the first performed anything, with the first
/// one's log: what a crash after the consumption was kept leaves. Mandate refuses the consumed
/// approval `wrong-state`: no hand-off, no commit, no model call, and the Suspension stays held.
#[test]
fn a_second_resume_with_the_same_approval_is_refused_and_performs_nothing() {
    let scratch = Scratch::new("mandate-second");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let before = scratch.root.join("state-before-resume");
    copy_dir(&held.state, &before);

    let first = resume_with(&scratch, &held, Some(&host), script_after_merge());
    assert_eq!(first.status.code(), Some(0), "{}", shown(&first));
    assert_eq!(scratch.files(&held.state, "merge-"), 1);

    // The state as the first resume left it before it performed anything, with its log.
    std::fs::copy(held.state.join(MANDATE_LOG), before.join(MANDATE_LOG)).unwrap();
    let crashed = Held {
        state: before,
        commission: held.commission.clone(),
        action_request_id: held.action_request_id.clone(),
        workspace: held.workspace.clone(),
    };
    assert_eq!(crashed.suspension_state(), Some(SuspensionState::Pending));
    let head = scratch.head();
    let refs = scratch.refs();
    let second = resume_with(&scratch, &crashed, Some(&host), Vec::new());
    assert_eq!(
        second.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&second)
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("NotGranted"), "{}", shown(&second));
    assert!(stderr.contains("wrong-state"), "{}", shown(&second));
    assert_eq!(scratch.files(&crashed.state, "merge-"), 0, "no hand-off");
    assert_eq!(scratch.head(), head, "no commit");
    assert_eq!(scratch.refs(), refs, "no Git ref moved");
    assert_eq!(crashed.suspension_state(), Some(SuspensionState::Pending));
    assert_eq!(
        scratch.mandate_log(&crashed.state).len(),
        2,
        "nothing more is consumed"
    );
}

/// An expired approval is refused and performs nothing: a host whose approvals last no time
/// records one that has expired when the resume presents it.
#[test]
fn an_expired_approval_is_refused_and_performs_nothing() {
    let scratch = Scratch::new("mandate-expired");
    let held = scratch.suspend();
    let host = scratch.mandate_host(0);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let log = scratch.mandate_log(&held.state);
    assert_eq!(log[0]["expires_at"], log[0]["recorded_at"], "{log:?}");

    let resumed = resume_with(&scratch, &held, Some(&host), Vec::new());
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("Mandate refused"),
        "{}",
        shown(&resumed)
    );
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
    assert_eq!(
        scratch.mandate_log(&held.state).len(),
        1,
        "nothing consumed"
    );
}

/// The grant names its Mandate approval; a grant without one (an `approve` without `--mandate`
/// replaced it) is refused when `resume` is given `--mandate`, and nothing is performed.
#[test]
fn the_grant_names_its_mandate_approval() {
    let scratch = Scratch::new("mandate-grant");
    let held = scratch.suspend();
    let host = scratch.mandate_host(3600);
    let approved = approve_with(&scratch, &held, Some(&host));
    assert_eq!(approved.status.code(), Some(0), "{}", shown(&approved));
    let grants = scratch.documents(&held.state, "grant-");
    let log = scratch.mandate_log(&held.state);
    assert_eq!(grants.len(), 1);
    assert_eq!(
        grants[0]["mandate_approval"], log[0]["approval"],
        "{grants:?}"
    );

    let plain = approve_with(&scratch, &held, None);
    assert_eq!(plain.status.code(), Some(0), "{}", shown(&plain));
    let grants = scratch.documents(&held.state, "grant-");
    assert_eq!(grants[0].get("mandate_approval"), None, "{grants:?}");

    let resumed = resume_with(&scratch, &held, Some(&host), Vec::new());
    assert_eq!(
        resumed.status.code(),
        Some(NOT_GRANTED),
        "{}",
        shown(&resumed)
    );
    assert!(
        String::from_utf8_lossy(&resumed.stderr).contains("names no Mandate approval"),
        "{}",
        shown(&resumed)
    );
    assert_eq!(
        scratch.files(&held.state, "merge-"),
        0,
        "nothing handed off"
    );
    assert_eq!(held.suspension_state(), Some(SuspensionState::Pending));
    assert_eq!(
        scratch.mandate_log(&held.state).len(),
        1,
        "nothing consumed"
    );
}

/// A Mandate file that cannot configure Mandate is refused before anything is approved.
#[test]
fn approve_refuses_a_mandate_file_it_cannot_use_and_writes_nothing() {
    let scratch = Scratch::new("mandate-bad-file");
    let held = scratch.suspend();
    let host = scratch.root.join("mandate-bad.json");
    std::fs::write(&host, "{\"verified\": {}}\n").unwrap();
    let refused = approve_with(&scratch, &held, Some(&host));
    assert_eq!(refused.status.code(), Some(FAILED), "{}", shown(&refused));
    assert_eq!(
        scratch.files(&held.state, "grant-"),
        0,
        "no grant is written"
    );
    assert!(
        !held.state.join(MANDATE_LOG).exists(),
        "no approval is recorded"
    );
}

const MANDATE_LOG: &str = "mandate-approvals.jsonl";
const SUBJECT: &str = "22222222-2222-4222-8222-222222222222";
const APPROVER: &str = "99999999-9999-4999-8999-999999999999";

/// The resumed agent's answers after the merge: it runs the tests, and the run stops at the
/// merge's gate at the HEAD it handed off.
fn script_after_merge() -> Vec<Vec<u8>> {
    vec![
        agent("select_action", json!({"action": "tests.run"})),
        agent("action_arguments", json!({})),
    ]
}

fn approve_with(scratch: &Scratch, held: &Held, mandate: Option<&Path>) -> Output {
    let mut args = vec![
        "approve",
        &held.commission,
        "repository.merge",
        "--state",
        held.state_text(),
    ];
    if let Some(mandate) = mandate {
        args.extend(["--mandate", mandate.to_str().unwrap()]);
    }
    scratch.command(&args)
}

/// Runs `resume`, with `--mandate` when given, answering the agent with `script`.
fn resume_with(
    scratch: &Scratch,
    held: &Held,
    mandate: Option<&Path>,
    script: Vec<Vec<u8>>,
) -> Output {
    let (base_url, server) = Script::serve(script);
    scratch.catalog(&base_url);
    let mut args = vec!["resume", &held.commission, "--state", held.state_text()];
    if let Some(mandate) = mandate {
        args.extend(["--mandate", mandate.to_str().unwrap()]);
    }
    let output = scratch.command(&args);
    server.finish();
    output
}

/// Copies the regular files of `from` into `to`, created.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// A commission `run --state` left suspended on the merge.
struct Held {
    state: PathBuf,
    commission: String,
    action_request_id: String,
    workspace: String,
}

impl Held {
    fn state_text(&self) -> &str {
        self.state.to_str().unwrap()
    }

    /// The state of the commission's Suspension, read through the store, or `None`.
    fn suspension_state(&self) -> Option<SuspensionState> {
        FileSuspensionStore::open(&self.state)
            .expect("the suspension store opens")
            .get(&CommissionId(Uuid(self.commission.clone())))
            .expect("the suspension store reads")
            .map(|held| held.state)
    }
}

fn shown(output: &Output) -> String {
    format!(
        "status {:?}\n--- stdout\n{}--- stderr\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A scratch directory under `CARGO_TARGET_TMPDIR`: the home, the Codex home, the protocol store
/// and a git workspace with one failing check; removed on drop.
struct Scratch {
    root: PathBuf,
    workspace: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("resume-{label}-{}-{nanos}", std::process::id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let scratch = Self { root, workspace };
        scratch.git(&["init", "--quiet", "--initial-branch=main"]);
        scratch.git(&["config", "user.name", "Fixture Author"]);
        scratch.git(&["config", "user.email", "fixture@example.invalid"]);
        scratch.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(scratch.workspace.join("check.txt"), "broken\n").unwrap();
        scratch.git(&["add", "--all"]);
        scratch.git(&["commit", "--quiet", "--message", "a failing check"]);
        scratch
    }

    /// Runs `run --state` to a Suspension on the merge, and reads what it left.
    fn suspend(&self) -> Held {
        self.suspend_with(3)
    }

    /// Runs `run`, with `--state <root>/state` when `state`, for `--max-steps max_steps`: the
    /// agent edits the check and runs the tests, then answers `last` (the merge, or the tests
    /// again).
    fn run(&self, max_steps: usize, state: bool, last: &str) -> Output {
        let (base_url, server) = Script::serve(vec![
            classify("software-change@1"),
            agent("select_action", json!({"action": "implementation.edit"})),
            agent(
                "action_arguments",
                json!({
                    "files": [{"path": "check.txt", "contents": "fixed\n"}],
                    "message": "fix the check"
                }),
            ),
            agent("select_action", json!({"action": "tests.run"})),
            agent("action_arguments", json!({})),
            agent("select_action", json!({"action": last})),
            agent("action_arguments", json!({})),
        ]);
        let catalog = self.catalog(&base_url);
        let state_dir = self.root.join("state");
        let max_steps = max_steps.to_string();
        let mut args = vec!["run", "--confinement", "none"];
        if state {
            args.extend(["--state", state_dir.to_str().unwrap()]);
        }
        args.extend([
            "--workspace",
            self.workspace.to_str().unwrap(),
            "--test-cmd",
            TEST_CMD,
            "--max-steps",
            &max_steps,
            "--catalog",
            catalog.to_str().unwrap(),
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            INTENT,
        ]);
        let output = self.command(&args);
        server.finish();
        output
    }

    /// Runs `run --state` to a Suspension on the merge, with a step budget of `max_steps`, and
    /// reads what it left.
    fn suspend_with(&self, max_steps: usize) -> Held {
        let state = self.root.join("state");
        let output = self.run(max_steps, true, "repository.merge");
        assert_eq!(output.status.code(), Some(0), "{}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("stopped: ApprovalRequired (repository.merge)"),
            "{}",
            shown(&output)
        );
        let contexts = ResumeContextStore::open(&state)
            .expect("the resume context store opens")
            .list()
            .expect("the resume contexts read back");
        assert_eq!(contexts.len(), 1, "{contexts:?}");
        let commission = contexts[0].commission_id.0.0.clone();
        let workspace = contexts[0].workspace.clone().expect("a workspace is kept");
        let suspension = FileSuspensionStore::open(&state)
            .expect("the suspension store opens")
            .get(&CommissionId(Uuid(commission.clone())))
            .expect("the suspension store reads")
            .expect("the commission holds a Suspension");
        assert_eq!(suspension.state, SuspensionState::Pending);
        assert_eq!(suspension.data.action, "repository.merge");
        Held {
            state,
            commission,
            action_request_id: suspension.data.action_request_id.0.0,
            workspace,
        }
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    /// The workspace's `HEAD` commit.
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Every ref of the workspace and what it points at, and what `HEAD` names.
    fn refs(&self) -> String {
        format!(
            "{}{}",
            self.git(&["for-each-ref", "--format=%(refname) %(objectname)"]),
            self.git(&["symbolic-ref", "HEAD"])
        )
    }

    /// The fixture catalog, at the one path every command names, with its endpoints at
    /// `base_url`.
    fn catalog(&self, base_url: &str) -> PathBuf {
        let path = self.root.join("catalog.toml");
        std::fs::write(&path, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        path
    }

    /// The files in `dir` whose name starts with `prefix` and ends `.json`, in name order.
    fn paths(&self, dir: &Path, prefix: &str) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with(prefix) && name.ends_with(".json")
            })
            .map(|entry| entry.path())
            .collect();
        paths.sort();
        paths
    }

    /// A host-trusted Mandate file (`loom.authority.MandateHost`) whose approvals last `seconds`:
    /// the subject holds `maintainer` on the case for `repository.merge` and `tests.run`, and
    /// `repository.merge` needs an approval.
    fn mandate_host(&self, seconds: i64) -> PathBuf {
        let path = self.root.join(format!("mandate-{seconds}.json"));
        let host = json!({
            "verified": {
                "subject": SUBJECT,
                "organization": "11111111-1111-4111-8111-111111111111",
                "audience": "loom",
                "credential": "77777777-7777-4777-8777-777777777777",
                "correlation": "correlation-resume"
            },
            "approver": APPROVER,
            "approver_policy": "40404040-4040-4040-8040-404040404040",
            "relation": "maintainer",
            "capabilities": ["repository.merge", "tests.run"],
            "approval_required": ["repository.merge"],
            "approval_seconds": seconds
        });
        std::fs::write(&path, format!("{host:#}\n")).unwrap();
        path
    }

    /// The entries of `dir`'s Mandate approval log, in order.
    fn mandate_log(&self, dir: &Path) -> Vec<Value> {
        match std::fs::read_to_string(dir.join(MANDATE_LOG)) {
            Ok(text) => text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn files(&self, dir: &Path, prefix: &str) -> usize {
        self.paths(dir, prefix).len()
    }

    fn documents(&self, dir: &Path, prefix: &str) -> Vec<Value> {
        self.paths(dir, prefix)
            .iter()
            .map(|path| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
            .collect()
    }

    fn command<A: AsRef<std::ffi::OsStr>>(&self, args: &[A]) -> Output {
        let home = self.root.join("home");
        Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
            .args(args)
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
            .env_remove("RUST_LOG")
            .output()
            .expect("run b10x-loom")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// One scripted model answer: a forced tool call in the Chat Completions stream format the
/// recorded fixtures of `catalog_route.rs` use.
fn tool_call(model: &str, name: &str, arguments: &Value) -> Vec<u8> {
    let chunk = |delta: Value, finish: Value| {
        json!({
            "id": format!("chatcmpl-call-{name}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
        })
    };
    let events = [
        chunk(
            json!({"role": "assistant", "tool_calls": [{"index": 0, "id": format!("call-{name}"),
                "type": "function", "function": {"name": name, "arguments": ""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls": [{"index": 0, "function": {"arguments": arguments.to_string()}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        json!({
            "id": format!("chatcmpl-call-{name}"),
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [],
            "usage": {"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127}
        }),
    ];
    let mut stream = String::new();
    for event in events {
        stream.push_str(&format!("data: {event}\n\n"));
    }
    stream.push_str("data: [DONE]\n\n");
    stream.into_bytes()
}

fn classify(protocol: &str) -> Vec<u8> {
    tool_call(
        "example/Classifier-Model",
        "pick_protocol",
        &json!({"protocol": protocol, "confidence": 0.99, "reasons": ["changes the repository"]}),
    )
}

fn agent(name: &str, arguments: Value) -> Vec<u8> {
    tool_call("example/Agent-Model", name, &arguments)
}

/// Answers one request per scripted response, in order, on a loopback socket, until told to
/// finish; records the forced tool each request named. A request past the script is answered
/// with an error status and recorded, so the case sees it.
struct Script {
    done: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Script {
    fn serve(responses: Vec<Vec<u8>>) -> (String, Self) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let finished = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            let mut sent = Vec::new();
            let mut responses = responses.into_iter();
            while !finished.load(Ordering::SeqCst) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                let request = read_request(&mut socket);
                sent.push(tool_named(&request));
                let answer = match responses.next() {
                    Some(response) => {
                        let mut answer = Vec::from(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                        );
                        answer.extend_from_slice(&response);
                        answer
                    }
                    None => Vec::from(
                        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
                let _ = socket.write_all(&answer);
                let _ = socket.shutdown(std::net::Shutdown::Both);
            }
            sent
        });
        (base_url, Self { done, handle })
    }

    /// Stops serving and returns the tool each request forced, in order.
    fn finish(self) -> Vec<String> {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().expect("the script finished")
    }
}

fn read_request(socket: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = socket.read(&mut buffer).expect("read");
        request.extend_from_slice(&buffer[..read]);
        if read == 0 || complete(&request) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

/// Whether `request` holds its whole body, by its `Content-Length`.
fn complete(request: &[u8]) -> bool {
    let text = String::from_utf8_lossy(request);
    text.split_once("\r\n\r\n").is_some_and(|(head, rest)| {
        head.to_ascii_lowercase()
            .split_once("content-length:")
            .and_then(|(_, tail)| tail.split('\r').next())
            .and_then(|value| value.trim().parse::<usize>().ok())
            .is_some_and(|length| rest.len() >= length)
    })
}

/// The forced tool a request names.
fn tool_named(request: &str) -> String {
    let body = request.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    serde_json::from_str::<Value>(body).map_or_else(
        |_| String::new(),
        |body| {
            body["tool_choice"]["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        },
    )
}
