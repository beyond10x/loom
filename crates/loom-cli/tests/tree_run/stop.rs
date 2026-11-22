//! Acceptance for `story:tree-suite-stop-resume-reopen-steps`: `tree hold` records a hold in the
//! root case's run log under the store lock, `tree run` on a held tree is refused `TreeStopped`
//! and writes nothing, and `tree reopen` lifts the hold.
//!
//! The fixture is the `review@1` case of `fixtures/tree-two-level/` at its first wait
//! ([`reviewed`]); each test works on a fresh copy. As in the rest of the file, the commands run
//! in process through `b10x_loom_tree` and answer as `crates/loom-cli/src/tree.rs` makes the
//! answer; `tree hold`, `tree run` and `tree reopen` start the binary where the binary is what is
//! tested (the printed hold, the printed refusal, the lift's output), three starts in all. No test
//! reaches the network, a model or a credential, and each ends within 60 s.

use super::*;

/// The principal of every hold and reopen of these tests.
const OWNER: &str = "person:owner";

/// `tree hold --at AT` of `store` by `principal` for `reason`, in process: what the command
/// prints, or its refusal.
fn hold_in(store: &Path, principal: &str, reason: &str) -> Output {
    answered(
        b10x_loom_tree::hold(&b10x_loom_tree::HoldRequest {
            store,
            principal,
            reason,
            time: at(),
        }),
        |held| {
            (
                0,
                json!({
                    "case": held.case,
                    "seq": held.seq,
                    "at": held.at.0,
                    "principal": held.principal,
                    "reason": held.reason,
                }),
            )
        },
    )
}

/// `tree hold` of the tree's store by [`OWNER`] for `reason`, in process.
fn hold(tree: &Tree, reason: &str) -> Output {
    hold_in(&tree.store(), OWNER, reason)
}

/// `tree hold --at AT --store <store>` with `flags` after, started as `b10x-loom`.
fn hold_started(tree: &Tree, store: &Path, flags: &[&str]) -> Output {
    tree.command()
        .args(["tree", "hold", "--at", AT, "--store"])
        .arg(store)
        .args(flags)
        .output()
        .unwrap()
}

/// `tree reopen --at AT` of the tree's store by [`OWNER`], in process.
fn reopen(tree: &Tree) -> Output {
    answered(
        b10x_loom_tree::reopen(&b10x_loom_tree::ReopenRequest {
            store: &tree.store(),
            principal: OWNER,
            time: at(),
        }),
        |lifted| {
            (
                0,
                json!({
                    "case": lifted.hold.case,
                    "seq": lifted.seq,
                    "at": lifted.at.0,
                    "principal": lifted.principal,
                    "stop": lifted.hold.seq,
                }),
            )
        },
    )
}

/// [`reopen`], started as `b10x-loom`.
fn reopen_started(tree: &Tree) -> Output {
    tree.command()
        .args(["tree", "reopen", "--at", AT, "--principal", OWNER])
        .arg("--store")
        .arg(tree.store())
        .output()
        .unwrap()
}

/// `output` ended with exit 0.
fn done(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}{}",
        stdout(output),
        stderr(output)
    );
}

/// `output` was refused `reason` with exit 1.
fn refused_with(output: &Output, reason: &str) -> String {
    let said = stderr(output);
    assert_eq!(output.status.code(), Some(1), "{}{said}", stdout(output));
    assert!(said.contains(reason), "{reason}: {said}");
    said
}

/// The lines of the root's run log as the JSON objects written: kind and members.
fn root_lines(tree: &Tree) -> Vec<Value> {
    fs::read_to_string(tree.store().join(log_file_name("root")))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The lines of the root's log of `kind`.
fn lines_of(tree: &Tree, kind: &str) -> Vec<Value> {
    root_lines(tree)
        .into_iter()
        .filter(|line| line["kind"] == kind)
        .collect()
}

/// The decisions a run printed, with the test's directory (the store's and the workspace's
/// parent) written as `<root>`, so that two copies of one tree compare.
fn decisions_of(tree: &Tree, ran: &Output) -> Value {
    let root = tree.root.canonicalize().unwrap();
    let text = json(ran)["decisions"]
        .to_string()
        .replace(&root.display().to_string(), "<root>")
        .replace(&tree.root.display().to_string(), "<root>");
    serde_json::from_str(&text).unwrap()
}

/// What `tree status` says of each case that two copies of one tree share: the case, its parent,
/// protocol and revision, its stop and the decisions it names, its outcome and its approvals.
fn status_of(tree: &Tree) -> Vec<Value> {
    tree.status()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            json!({
                "case": case["case"],
                "parent": case["parent"],
                "protocol": case["protocol"],
                "revision": case["revision"],
                "outcome": case["outcome"],
                "stop": case["stop"],
                "stop_next": case["stop_next"],
                "approvals": case["approvals"],
            })
        })
        .collect()
}

#[test]
fn a_held_tree_refuses_to_run() {
    let tree = reviewed("held-run");
    let held = hold_started(
        &tree,
        &tree.store(),
        &["--principal", OWNER, "--reason", "hold for review"],
    );
    done(&held);
    assert_eq!(json(&held)["principal"], OWNER, "{}", stdout(&held));
    let before = every_file(&tree.store());
    let ran = tree.run();
    let said = refused_with(&ran, "TreeStopped");
    for named in [OWNER, "hold for review", "tree reopen", AT] {
        assert!(said.contains(named), "{named}: {said}");
    }
    assert_eq!(every_file(&tree.store()), before, "the refused run wrote");
    // The binary prints the same refusal and exits 1.
    let started = tree.run_started();
    let printed = refused_with(&started, "TreeStopped");
    assert!(
        printed.contains("b10x-loom tree reopen --store"),
        "{printed}"
    );
    assert_eq!(every_file(&tree.store()), before, "the refused run wrote");
}

#[test]
fn a_reopened_tree_runs_on() {
    let tree = reviewed("reopened");
    let never = reviewed("never-held");
    done(&hold(&tree, "hold for review"));
    let lift = reopen_started(&tree);
    done(&lift);
    let ran = tree.run();
    let unheld = never.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    assert_eq!(unheld.status.code(), Some(3), "{}", stderr(&unheld));
    assert_eq!(decisions_of(&tree, &ran), decisions_of(&never, &unheld));
    let recorded = lines_of(&tree, "StopRecorded");
    let lifted = lines_of(&tree, "StopLifted");
    assert_eq!(
        (recorded.len(), lifted.len()),
        (1, 1),
        "{:?}",
        root_lines(&tree)
    );
    assert_eq!(recorded[0]["principal"], OWNER);
    assert_eq!(recorded[0]["reason"], "hold for review");
    assert_eq!(lifted[0]["principal"], OWNER);
    // `stop` names the line it lifts: neither its own `seq` nor 0.
    assert_eq!(lifted[0]["stop"], recorded[0]["seq"], "{lifted:?}");
    assert_ne!(lifted[0]["stop"], lifted[0]["seq"]);
    assert_ne!(lifted[0]["stop"], json!(0));
    // The binary printed the lift as the log holds it.
    let printed = json(&lift);
    assert_eq!(
        (&printed["seq"], &printed["stop"]),
        (&lifted[0]["seq"], &lifted[0]["stop"]),
        "{printed}"
    );
    // The lines read back through the run log's reader.
    let lines = log_lines(&tree, "root");
    assert_eq!(lines.len(), root_lines(&tree).len());
}

#[test]
fn a_hold_and_a_reopen_each_apply_once() {
    let tree = reviewed("twice");
    done(&hold(&tree, "hold for review"));
    let before = every_file(&tree.store());
    refused_with(&hold(&tree, "hold again"), "TreeAlreadyStopped");
    assert_eq!(every_file(&tree.store()), before, "the second hold wrote");
    let open = reviewed("not-held");
    let before = every_file(&open.store());
    refused_with(&reopen(&open), "TreeNotStopped");
    assert_eq!(every_file(&open.store()), before, "the reopen wrote");
}

#[test]
fn an_approval_does_not_lift_a_hold() {
    let tree = reviewed("approval-holds");
    done(&hold(&tree, "hold for review"));
    let taken = review(&tree, "draft.review", &[]);
    assert_eq!(taken.status.code(), Some(0), "{}", stderr(&taken));
    refused_with(&tree.run(), "TreeStopped");
}

#[test]
fn an_unusable_reason_or_principal_is_refused() {
    let tree = reviewed("unusable");
    let before = every_file(&tree.store());
    let long_reason = "r".repeat(1025);
    let long_principal = "p".repeat(201);
    // The member is named in the detail ("the reason ...", "the principal ..."), not only as the
    // refusal document's `reason` key.
    for (principal, reason, member) in [
        (OWNER, "two\nlines", "the reason"),
        (OWNER, long_reason.as_str(), "the reason"),
        (long_principal.as_str(), "why", "the principal"),
        ("bell\u{7}", "why", "the principal"),
    ] {
        let said = refused_with(&hold_in(&tree.store(), principal, reason), "StopUnusable");
        assert!(said.contains(member), "{member}: {said}");
        assert_eq!(
            every_file(&tree.store()),
            before,
            "{member}: the refusal wrote"
        );
    }
    // Before the store is opened: a store that does not exist is never read, nor made.
    let absent = tree.root.join("no-such-store");
    let said = refused_with(&hold_in(&absent, OWNER, "two\nlines"), "StopUnusable");
    assert!(said.contains("the reason"), "{said}");
    assert!(!said.contains("NotATree"), "{said}");
    assert!(
        !absent.exists(),
        "the refused hold made {}",
        absent.display()
    );
}

#[test]
fn the_other_commands_work_on_a_held_tree() {
    let tree = reviewed("others-held");
    let never = reviewed("others-unheld");
    done(&hold(&tree, "hold for review"));
    assert_eq!(status_of(&tree), status_of(&never));
    let closed = tree.closed("root", "done");
    let unheld = never.closed("root", "done");
    assert_eq!(closed.status.code(), unheld.status.code());
    refused_with(&closed, "OutcomeNotLegitimate");
    refused_with(&unheld, "OutcomeNotLegitimate");
    let verified = tree.verified();
    assert_eq!(verified.status.code(), Some(0), "{}", stdout(&verified));
    done(&reopen(&tree));
    let verified = tree.verified();
    assert_eq!(verified.status.code(), Some(0), "{}", stdout(&verified));
}

#[test]
fn hold_and_reopen_wait_for_no_writer() {
    let tree = reviewed("writer-hold");
    let before = every_file(&tree.store());
    let held = b10x_loom_tree::StoreLock::acquire(&tree.store()).unwrap();
    let during = every_file(&tree.store());
    let said = refused_with(&hold(&tree, "hold for review"), "StoreInUse");
    assert!(
        said.contains(&format!("held by process {}", std::process::id())),
        "{said}"
    );
    assert_eq!(every_file(&tree.store()), during, "the refused hold wrote");
    drop(held);
    assert_eq!(every_file(&tree.store()), before);

    let open = reviewed("writer-reopen");
    done(&hold(&open, "hold for review"));
    let before = every_file(&open.store());
    let held = b10x_loom_tree::StoreLock::acquire(&open.store()).unwrap();
    let during = every_file(&open.store());
    refused_with(&reopen(&open), "StoreInUse");
    assert_eq!(
        every_file(&open.store()),
        during,
        "the refused reopen wrote"
    );
    drop(held);
    assert_eq!(every_file(&open.store()), before);
}
