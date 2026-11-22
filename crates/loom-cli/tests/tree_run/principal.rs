//! Acceptance for `story:next-decision-names-the-principal`: a printed decision a person answers
//! names the principal its command records, the one `tree approve --action` checks; a capability
//! approval names the default principal it records; a close and a handoff name none. The status
//! and the run log of a waiting tree keep their bytes.
//!
//! The fixture is the `sign@1` case of `fixtures/tree-two-level/` at its first wait (one `tree
//! open` and one `tree run`, no program started), with `sign.bindings.yaml` or
//! `sign-decider.bindings.yaml`, whose signing person is `decider:model`. No test reaches the
//! network, a model or a credential, and each ends within 60 s.

use super::*;

/// A `sign@1` case opened with `bindings` and brought to its first wait, and the run's decisions.
fn waiting_sign(label: &str, bindings: &str) -> (Tree, Vec<Value>) {
    let tree = Tree::new(label);
    open_with(&tree, "sign@1", bindings);
    let ran = tree.run();
    assert_eq!(
        ran.status.code(),
        Some(3),
        "{}{}",
        stdout(&ran),
        stderr(&ran)
    );
    let waited = json(&ran);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decisions = waited["decisions"].as_array().unwrap().clone();
    assert_eq!(decisions.len(), 3, "{waited}");
    (tree, decisions)
}

/// The value after `--principal` in `decision`'s command.
fn command_principal(decision: &Value) -> String {
    let words: Vec<&str> = decision["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap())
        .collect();
    let at = words
        .iter()
        .position(|word| *word == "--principal")
        .unwrap_or_else(|| panic!("no --principal: {decision}"));
    words[at + 1].to_owned()
}

/// `decision`'s command with the value after `--principal` replaced by `principal`.
fn with_principal(decision: &Value, principal: &str) -> Value {
    let mut changed = decision.clone();
    let words = changed["command"].as_array_mut().unwrap();
    let at = words
        .iter()
        .position(|word| word == "--principal")
        .unwrap_or_else(|| panic!("no --principal: {decision}"));
    words[at + 1] = json!(principal);
    changed
}

/// The `executor.principal` the bindings file `bindings` of the fixture gives `action`.
fn bound_principal(bindings: &str, action: &str) -> String {
    let text = fs::read_to_string(Path::new(FIXTURE).join(bindings)).unwrap();
    let document: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    document["actions"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|bound| bound["action"].as_str() == Some(action))
        .and_then(|bound| bound["executor"]["principal"].as_str())
        .unwrap_or_else(|| panic!("{bindings} binds no person to {action}"))
        .to_owned()
}

/// The `AuthorityRecorded` lines of `lines`, as `(capability, principal)`.
fn authorities(lines: &[RunLogLine]) -> Vec<(String, String)> {
    lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::AuthorityRecorded(recorded) => {
                Some((recorded.capability.clone(), recorded.principal.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_person_decision_names_its_principal() {
    let (_tree, decisions) = waiting_sign("principal-person", "sign.bindings.yaml");
    for action in ["post.sign", "post.seal"] {
        let decision = about(&decisions, action);
        assert_eq!(decision["kind"], "ApproveAction", "{decision}");
        let bound = bound_principal("sign.bindings.yaml", action);
        assert_eq!(decision["principal"], bound.as_str(), "{decision}");
        assert_eq!(command_principal(&decision), bound, "{decision}");
    }
}

#[test]
fn the_printed_principal_is_the_one_recorded_and_no_other_is_accepted() {
    let (tree, decisions) = waiting_sign("principal-round-trip", "sign.bindings.yaml");
    let decision = about(&decisions, "post.seal");
    assert_eq!(decision["kind"], "ApproveAction", "{decision}");
    assert_eq!(decision["capability"], "clerk.seal", "{decision}");
    let principal = decision["principal"]
        .as_str()
        .unwrap_or_else(|| panic!("no principal: {decision}"))
        .to_owned();

    // Another principal, the other person of the case or the default, is refused, writing nothing.
    for other in ["walker", DEFAULT_PRINCIPAL_TEXT] {
        let before = every_file(&tree.store());
        let refused = tree.printed(&with_principal(&decision, other));
        refused_naming(&refused, &["post.seal", &principal]);
        assert_eq!(
            every_file(&tree.store()),
            before,
            "a refused principal wrote into the store"
        );
    }

    // The command as printed is taken, and what it records names the printed principal.
    let before = log_lines(&tree, "root").len();
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let written = lines_since(&tree, "root", before);
    assert_eq!(
        authorities(&written),
        [("clerk.seal".to_owned(), principal.clone())],
        "{written:?}"
    );
    let sealed = records_of(&tree, "root", "post_sealed");
    assert_eq!(sealed.len(), 1, "{sealed:?}");
    assert_eq!(sealed[0]["producer"], principal.as_str(), "{sealed:?}");
}

/// The principal a printed `tree approve` of a program action records.
const DEFAULT_PRINCIPAL_TEXT: &str = "operator";

#[test]
fn an_approval_names_the_principal_it_records() {
    let (tree, decisions) = waiting_sign("principal-approve", "sign.bindings.yaml");
    let decision = about(&decisions, "post.stamp");
    assert_eq!(decision["kind"], "Approve", "{decision}");
    assert_eq!(decision["principal"], DEFAULT_PRINCIPAL_TEXT, "{decision}");
    assert_eq!(command_principal(&decision), DEFAULT_PRINCIPAL_TEXT);
    // The default restricts nothing: the command with another principal records that principal,
    // and the command as printed records the default.
    for principal in ["clerk", DEFAULT_PRINCIPAL_TEXT] {
        let before = log_lines(&tree, "root").len();
        let granted = tree.printed(&with_principal(&decision, principal));
        assert_eq!(granted.status.code(), Some(0), "{}", stderr(&granted));
        let written = lines_since(&tree, "root", before);
        assert_eq!(
            authorities(&written),
            [("clerk.stamp".to_owned(), principal.to_owned())],
            "{written:?}"
        );
    }
}

#[test]
fn close_and_handoff_name_no_principal() {
    let closing = Tree::new("principal-close");
    open_with(&closing, "fork@1", "fork.bindings.yaml");
    let waited = closing.run();
    assert_eq!(waited.status.code(), Some(3), "{}", stderr(&waited));
    let waited = json(&waited);
    let closes = waited["decisions"].as_array().unwrap();
    assert_eq!(closes.len(), 2, "{waited}");
    for decision in closes {
        assert_eq!(decision["kind"], "Close", "{decision}");
        assert!(
            !decision.as_object().unwrap().contains_key("principal"),
            "{decision}"
        );
    }

    let handing = Tree::new("principal-handoff");
    let opened = handing.open_leg_with("tour-leg-handoff.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    let handoff = one_decision(&handing.run(), "Handoff");
    assert!(
        !handoff.as_object().unwrap().contains_key("principal"),
        "{handoff}"
    );
}

#[test]
fn a_decider_principal_is_named_and_accepted() {
    let bindings = "sign-decider.bindings.yaml";
    assert_eq!(bound_principal(bindings, "post.sign"), "decider:model");
    let (tree, decisions) = waiting_sign("principal-decider", bindings);
    let decision = about(&decisions, "post.sign");
    assert_eq!(decision["kind"], "ApproveAction", "{decision}");
    assert_eq!(decision["principal"], "decider:model", "{decision}");
    assert_eq!(command_principal(&decision), "decider:model");
    let taken = tree
        .command()
        .args(["tree", "approve", "--at", AT, "--store"])
        .arg(tree.store())
        .args([
            "--case",
            "root",
            "--principal",
            "decider:model",
            "--action",
            "post.sign",
        ])
        .output()
        .unwrap();
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let signed = records_of(&tree, "root", "post_signed");
    assert_eq!(signed.len(), 1, "{signed:?}");
    assert_eq!(signed[0]["producer"], "decider:model", "{signed:?}");
}

/// `text` with the scratch directory of `tree` as `<root>` and this release as `<version>`.
fn placeholders(tree: &Tree, text: &str) -> String {
    let canonical = fs::canonicalize(&tree.root).unwrap();
    text.replace(&canonical.display().to_string(), "<root>")
        .replace(&tree.root.display().to_string(), "<root>")
        .replace(
            &format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION")),
            r#""version":"<version>""#,
        )
}

/// `tree status` of the `sign@1` case at its first wait, before this story, paths and version
/// replaced as by [`placeholders`].
const WAITING_STATUS: &str = concat!(
    r#"{"cases":[{"approvals":[],"case":"root","case_file":"<root>/store/case-726f6f74.json","#,
    r#""copies":[],"evidence":[],"log":{"verified":true},"open_obligations":[],"protocol":"sign@1","#,
    r#""protocol_id":"sign","#,
    r#""protocol_revision":"1","revision":1,"run_log":"<root>/store/runlog-case-726f6f74.jsonl","#,
    r#""runs":[],"stop":"AwaitingPerson","stop_next":["post.seal of case root at revision 1","#,
    r#""post.sign of case root at revision 1","post.stamp of case root at revision 1"]}],"#,
    r#""format":"loom.tree-status/1","protocols":[{"file":"","protocol":"sign","#,
    r#""registration":"sign@1","revision":1,"#,
    r#""sha256":"67185f0c4ae58bcf9c3fd10d0453e1398a47e9b28add9bae25502d9e3b5e4fe8"}],"#,
    r#""root":"root","store":"<root>/store","#,
    r#""version":"<version>","workspace":"<root>/workspace"}"#,
    "\n"
);

/// The run log of the `sign@1` case at its first wait, before this story.
const WAITING_LOG: &str = concat!(
    r#"{"seq":0,"at":"2026-11-22T08:00:00Z","kind":"CaseOpened","case":"root","protocol":"sign@1"}"#,
    "\n",
    r#"{"seq":1,"prev":"281157692553ceb756ed4f697f567a3c0111c49044efd6d20ae100f3b4923f67","#,
    r#""at":"2026-11-22T08:00:00Z","kind":"Evaluated","case_revision":1,"#,
    r#""frontier":"0cb27c3f-727b-8eef-8af9-82e30f52fd7a","#,
    r#""decision_digest":"b1a90b2b4d272737707376d540f0c75589ca1ceeac3eaf9d220d40cacc6fc60e"}"#,
    "\n",
    r#"{"seq":2,"prev":"5529a195d37a712719b82deee9cd0c2f3aee945f2e969be40be2e16ed49bece0","#,
    r#""at":"2026-11-22T08:00:00Z","kind":"Stopped","stop":"AwaitingPerson","#,
    r#""next":["post.seal of case root at revision 1","post.sign of case root at revision 1","#,
    r#""post.stamp of case root at revision 1"]}"#,
    "\n"
);

#[test]
fn status_and_run_log_do_not_change() {
    let (tree, _) = waiting_sign("principal-unchanged", "sign.bindings.yaml");
    let status = tree.tree("status");
    assert_eq!(status.status.code(), Some(0), "{}", stderr(&status));
    assert_eq!(placeholders(&tree, &stdout(&status)), WAITING_STATUS);
    let log = fs::read_to_string(tree.store().join(log_file_name("root"))).unwrap();
    assert_eq!(log, WAITING_LOG);
}
