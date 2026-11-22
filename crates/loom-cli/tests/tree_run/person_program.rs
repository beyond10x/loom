//! Acceptance for `story:person-action-runs-a-program`: a `person` binding may name an `argv`;
//! `tree approve --action` records the approval, runs the program once through the runner a
//! program step starts through, and writes the person's records only after it exits 0. An argv
//! entry that is exactly `{principal}` is the approving principal, one argument.
//!
//! The fixture is the protocol `answer@1` (`fixtures/tree-two-level/extra/answer.yaml`): three
//! person actions, `answer.take` (writes, under `owner.answer`), `answer.free` (writes, no
//! capability) and `answer.look` (reads, under `owner.look`). Each test writes its bindings file
//! and its POSIX sh stubs into the workspace and runs every program through the `local` runner;
//! the library tests call `b10x_loom_tree::approve` on such a store. No test reaches the network,
//! a model or a credential, and each ends within 60 s.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use b10x_loom_commission::model::json::Value as CommissionJson;
use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_intake_slice::confinement::{
    Backend, ChildCommand, ConfinementError, ConfinementRefusal, TestExecution, TestRunner,
};
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_tree::model::{ApprovalDecision, TreeRefusalReason};
use b10x_loom_tree::{ApproveRequest, TreeTime, approve, person_program};
use loom_governor::{CanonGovernor, FileCaseStore};

use super::*;

/// The `source` of every observation of a bound program's run.
const PROGRAM_SOURCE: &str = "loom/bound-program";

/// The placed path of `subject`, below a directory a test may replace by a link.
const SUBJECT: &str = "answer/subject.txt";

/// The principal every binding of these tests names, unless a test names another.
pub(super) const OWNER: &str = "owner";

/// One person binding of `answer@1`: `action` taken by `principal`, `argv` the flow list of its
/// program when given, `extra` more members of the action, the evidence kind `kind` with the
/// result `result` for `performed`.
fn person(
    action: &str,
    principal: &str,
    argv: Option<&str>,
    extra: &str,
    kind: &str,
    result: &str,
) -> String {
    let argv = argv.map_or_else(String::new, |argv| format!("      argv: {argv}\n"));
    format!(
        "  - action: {action}\n    executor:\n      kind: person\n      principal: {principal:?}\n{argv}\
         {extra}    evidence:\n      - kind: {kind}\n        subject: subject\n        producer: {principal:?}\n        \
         from: effect\n        results: {{performed: {result}}}\n"
    )
}

/// The bindings file of `answer@1` with `principal` and the argv given for each action (`None`:
/// no program), `take_extra` more members of `answer.take`, and `top` more top-level members.
pub(super) fn answer_bindings(
    principal: &str,
    take: Option<&str>,
    free: Option<&str>,
    look: Option<&str>,
    take_extra: &str,
    top: &str,
) -> String {
    format!(
        "format: loom.bindings/1\nprotocol: answer@1\n{top}artifacts:\n  subject: {SUBJECT}\nactions:\n{}{}{}",
        person(
            "answer.take",
            principal,
            take,
            take_extra,
            "answer_taken",
            "taken"
        ),
        person("answer.free", principal, free, "", "answer_freed", "freed"),
        person(
            "answer.look",
            principal,
            look,
            "",
            "answer_looked",
            "looked"
        ),
    )
}

/// A tree of one `answer@1` case opened with `bindings`, the subject holding `question`, the
/// stubs `(name, text)` written into `stubs/`.
pub(super) fn answer_tree(label: &str, bindings: &str, stubs: &[(&str, &str)]) -> Tree {
    let tree = Tree::new(label);
    let workspace = tree.workspace();
    fs::create_dir_all(workspace.join("answer")).unwrap();
    fs::write(workspace.join(SUBJECT), "question\n").unwrap();
    for (name, text) in stubs {
        fs::write(workspace.join("stubs").join(name), text).unwrap();
    }
    fs::write(workspace.join("answer.bindings.yaml"), bindings).unwrap();
    open_with(&tree, "answer@1", "answer.bindings.yaml");
    tree
}

/// The decisions the first `tree run` of `tree` printed, waiting on a person.
pub(super) fn waiting(tree: &Tree) -> Vec<Value> {
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
    waited["decisions"].as_array().unwrap().clone()
}

/// The argv of `tree approve` of `action` of the root as `principal`, in the word order a refusal's
/// `command` has: `capability` and `more` (the result and reasons) where given, `--at`, then
/// `--confinement local`.
fn approve_words(
    tree: &Tree,
    action: &str,
    capability: Option<&str>,
    principal: &str,
    more: &[&str],
) -> Vec<String> {
    let mut words: Vec<String> = ["b10x-loom", "tree", "approve", "--store"]
        .map(str::to_owned)
        .to_vec();
    words.push(tree.store().display().to_string());
    words.extend(["--case".to_owned(), "root".to_owned()]);
    if let Some(capability) = capability {
        words.extend(["--capability".to_owned(), capability.to_owned()]);
    }
    words.extend([
        "--principal".to_owned(),
        principal.to_owned(),
        "--action".to_owned(),
        action.to_owned(),
    ]);
    words.extend(more.iter().map(|word| (*word).to_owned()));
    words.extend(
        ["--at", AT, "--confinement", "local"]
            .map(str::to_owned)
            .to_vec(),
    );
    words
}

/// Runs `words`, an argv starting with `b10x-loom`, as the test's `b10x-loom`.
fn run_words(tree: &Tree, words: &[String]) -> Output {
    assert_eq!(words[0], "b10x-loom", "{words:?}");
    tree.command().args(&words[1..]).output().unwrap()
}

/// `output` refused `PersonProgramFailed` with exit 1, saying each of `named`; its refusal
/// document.
pub(super) fn program_failed(output: &Output, named: &[&str]) -> Value {
    let said = stderr(output);
    assert_eq!(output.status.code(), Some(1), "{}{said}", stdout(output));
    assert!(said.contains("PersonProgramFailed"), "{said}");
    for name in named {
        assert!(said.contains(name), "{name}: {said}");
    }
    let refusal = json_line(output);
    assert_eq!(refusal["reason"], "PersonProgramFailed", "{refusal}");
    refusal
}

/// The `action finished` line among `lines`, the only one.
fn one_finished(lines: &[RunLogLine]) -> b10x_loom_tree::runlog::ActionFinished {
    let finished: Vec<_> = lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) => Some(finished.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1, "{lines:?}");
    finished[0].clone()
}

/// The governor over the test store's case files.
fn store_governor(tree: &Tree) -> CanonGovernor<FileCaseStore> {
    CanonGovernor::new(FileCaseStore::open(tree.store()).unwrap())
}

/// The payloads of the bound-program runs the store keeps, as JSON.
fn program_runs(tree: &Tree) -> Vec<Value> {
    store_governor(tree)
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .map(|observation| plain(&observation.payload))
        .collect()
}

/// `value` as `serde_json`'s.
fn plain(value: &CommissionJson) -> Value {
    match value {
        CommissionJson::Null => Value::Null,
        CommissionJson::Bool(value) => Value::Bool(*value),
        CommissionJson::Number(spelling) => serde_json::from_str(spelling).unwrap(),
        CommissionJson::Text(text) => Value::String(text.clone()),
        CommissionJson::Array(items) => Value::Array(items.iter().map(plain).collect()),
        CommissionJson::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// The revision the case holds for the subject.
fn subject_revision(tree: &Tree) -> String {
    store_governor(tree)
        .revisions(&CaseId("root".to_owned()))
        .unwrap()["subject"]
        .clone()
}

/// The revision of the subject's bytes now: `t` and the first 16 hex digits of their SHA-256.
fn digest_now(tree: &Tree) -> String {
    let bytes = fs::read(tree.workspace().join(SUBJECT)).unwrap();
    format!("t{}", &sha256_hex(&bytes)[..16])
}

/// A stub that appends each of its arguments to the subject file, one per line.
const APPEND: &str = "#!/bin/sh\nprintf '%s\\n' \"$@\" >> answer/subject.txt\n";

/// A stub that ends with exit 3 and prints a line that must never reach a refusal.
const EXIT_3: &str = "#!/bin/sh\necho 'secret program output 5d1e'\nexit 3\n";

#[test]
fn a_person_action_runs_its_program_with_the_principal() {
    let bindings = answer_bindings(
        OWNER,
        Some("[sh, stubs/answer.sh, \"{principal}\", --action, answer.take]"),
        None,
        None,
        "",
        "",
    );
    let tree = answer_tree("person-program-runs", &bindings, &[("answer.sh", APPEND)]);
    let decision = about(&waiting(&tree), "answer.take");
    assert_eq!(decision["kind"], "ApproveAction", "{decision}");
    let before = log_lines(&tree, "root").len();
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let subject = fs::read_to_string(tree.workspace().join(SUBJECT)).unwrap();
    assert_eq!(
        subject, "question\nowner\n--action\nanswer.take\n",
        "the principal is one argument"
    );
    let taken = records_of(&tree, "root", "answer_taken");
    assert_eq!(taken.len(), 1, "{taken:?}");
    assert_eq!(taken[0]["result"], "taken", "{taken:?}");
    assert_eq!(taken[0]["producer"], OWNER, "{taken:?}");
    assert_eq!(taken[0]["subject_revision"], digest_now(&tree), "{taken:?}");
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(is_authority(&written[0]), "{written:?}");
    let ended = one_finished(&written);
    assert_eq!(ended.action, "answer.take");
    assert_eq!(ended.exit, Some(0), "{ended:?}");
    assert_eq!(ended.refusal, None, "{ended:?}");
    assert_eq!(ended.evidence, [taken[0]["evidence_id"].as_str().unwrap()]);
    let runs = program_runs(&tree);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0]["principal"], OWNER, "{runs:?}");
    assert_eq!(runs[0]["exit"], 0, "{runs:?}");
    assert_eq!(
        ended.blob_digest.as_deref(),
        runs[0]["blob_sha256"].as_str(),
        "{ended:?} {runs:?}"
    );
}

#[test]
fn a_person_program_beside_the_bindings_runs_with_the_principal() {
    // Runbook's shape: the tool beside the bindings file, named relative to it.
    let bindings = answer_bindings(
        OWNER,
        Some("[tools/answer-tool.sh, scope-answer, \"{principal}\", --action, answer.take]"),
        None,
        None,
        "",
        "program_root: bindings\n",
    );
    let tree = Tree::new("person-program-beside");
    let workspace = tree.workspace();
    fs::create_dir_all(workspace.join("answer")).unwrap();
    fs::write(workspace.join(SUBJECT), "question\n").unwrap();
    let directory = workspace.join("bindings");
    fs::create_dir_all(directory.join("tools")).unwrap();
    fs::write(directory.join("answer.bindings.yaml"), &bindings).unwrap();
    let tool = directory.join("tools/answer-tool.sh");
    fs::write(&tool, APPEND).unwrap();
    let mut permissions = fs::metadata(&tool).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(&tool, permissions).unwrap();
    open_with(&tree, "answer@1", "bindings/answer.bindings.yaml");
    let decision = about(&waiting(&tree), "answer.take");
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let subject = fs::read_to_string(workspace.join(SUBJECT)).unwrap();
    assert_eq!(
        subject,
        "question\nscope-answer\nowner\n--action\nanswer.take\n"
    );
    assert_eq!(records_of(&tree, "root", "answer_taken").len(), 1);
}

#[test]
fn the_principal_is_one_argument_and_runs_nothing() {
    let principal = "owner a;$(touch pwned)";
    let bindings = answer_bindings(
        principal,
        Some("[sh, stubs/count.sh, \"{principal}\", --action, answer.take]"),
        None,
        None,
        "",
        "",
    );
    let count = "#!/bin/sh\nprintf '%s\\n' \"$#\" \"$@\" > counted.txt\n";
    let tree = answer_tree(
        "person-program-injection",
        &bindings,
        &[("count.sh", count)],
    );
    let decision = about(&waiting(&tree), "answer.take");
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let counted = fs::read_to_string(tree.workspace().join("counted.txt")).unwrap();
    assert_eq!(counted, format!("3\n{principal}\n--action\nanswer.take\n"));
    for place in [tree.workspace(), tree.root.clone(), tree.store()] {
        assert!(!place.join("pwned").exists(), "{}", place.display());
    }
}

#[test]
fn the_principal_is_never_in_the_childs_environment() {
    let principal = "owner-7c1e9a";
    let bindings = answer_bindings(
        principal,
        Some("[sh, stubs/env.sh, \"{principal}\"]"),
        None,
        None,
        "",
        "",
    );
    let env = "#!/bin/sh\n/usr/bin/env > printed-env.txt\n";
    let tree = answer_tree("person-program-env", &bindings, &[("env.sh", env)]);
    let decision = about(&waiting(&tree), "answer.take");
    let taken = tree.printed(&decision);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let printed = fs::read_to_string(tree.workspace().join("printed-env.txt")).unwrap();
    let kept: Vec<&str> = printed
        .lines()
        .filter(|line| {
            !["PWD=", "SHLVL=", "_="]
                .iter()
                .any(|own| line.starts_with(own))
        })
        .collect();
    for line in &kept {
        let value = line.split_once('=').map_or("", |(_, value)| value);
        assert!(!value.contains(principal), "{line}");
    }
    let names: BTreeSet<&str> = kept
        .iter()
        .map(|line| line.split_once('=').map_or(*line, |(name, _)| name))
        .collect();
    assert_eq!(
        names,
        BTreeSet::from(["LOOM_CASE", "LOOM_TIMEOUT_SECONDS", "PATH"]),
        "{printed}"
    );
}

#[test]
fn a_person_program_without_a_capability_runs_and_keeps_no_approval() {
    let bindings = answer_bindings(
        OWNER,
        None,
        Some("[sh, stubs/free.sh, \"{principal}\"]"),
        None,
        "",
        "",
    );
    let tree = answer_tree("person-program-free", &bindings, &[("free.sh", APPEND)]);
    let decision = about(&waiting(&tree), "answer.free");
    assert!(decision["capability"].is_null(), "{decision}");
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
    assert_eq!(written.len(), 1, "{written:?}");
    assert!(!written.iter().any(is_authority), "{written:?}");
    let ended = one_finished(&written);
    assert_eq!(ended.exit, Some(0), "{ended:?}");
    assert!(
        b10x_loom_tree::read_approvals(&tree.store(), "root")
            .unwrap()
            .is_empty()
    );
    assert_eq!(records_of(&tree, "root", "answer_freed").len(), 1);
    assert_eq!(
        fs::read_to_string(tree.workspace().join(SUBJECT)).unwrap(),
        format!("question\n{OWNER}\n")
    );
}

#[test]
fn a_failed_person_program_records_nothing_and_keeps_the_approval() {
    let bindings = answer_bindings(OWNER, Some("[sh, stubs/fails.sh]"), None, None, "", "");
    let tree = answer_tree("person-program-fails", &bindings, &[("fails.sh", EXIT_3)]);
    waiting(&tree);
    let before = log_lines(&tree, "root").len();
    let words = approve_words(
        &tree,
        "answer.take",
        Some("owner.answer"),
        OWNER,
        &[
            "--result",
            "performed",
            "--reasons",
            "Answered in the file.",
        ],
    );
    let refused = run_words(&tree, &words);
    let refusal = program_failed(&refused, &["answer.take", "exit 3"]);
    assert!(!stderr(&refused).contains("5d1e"), "{}", stderr(&refused));
    let runs = program_runs(&tree);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0]["exit"], 3, "{runs:?}");
    let blob = runs[0]["blob_sha256"].as_str().unwrap();
    assert!(stderr(&refused).contains(blob), "{}", stderr(&refused));
    assert_eq!(refusal["command"], json!(words), "{refusal}");
    assert_eq!(
        b10x_loom_tree::read_approvals(&tree.store(), "root")
            .unwrap()
            .len(),
        1
    );
    assert!(records_of(&tree, "root", "answer_taken").is_empty());
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(is_authority(&written[0]), "{written:?}");
    let ended = one_finished(&written);
    assert_eq!(ended.exit, Some(3), "{ended:?}");
    assert_eq!(ended.blob_digest.as_deref(), Some(blob), "{ended:?}");
    assert!(ended.evidence.is_empty(), "{ended:?}");

    // The principal fixes the cause and runs the refusal's command: it takes the action again.
    fs::write(
        tree.workspace().join("stubs/fails.sh"),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();
    let again: Vec<String> = refusal["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap().to_owned())
        .collect();
    let taken = run_words(&tree, &again);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    let taken = records_of(&tree, "root", "answer_taken");
    assert_eq!(taken.len(), 1, "{taken:?}");
    // The named result `performed`, read through the entry's table.
    assert_eq!(taken[0]["result"], "taken", "{taken:?}");
}

#[test]
fn a_failed_person_program_without_a_capability_leaves_no_approval() {
    let bindings = answer_bindings(OWNER, None, Some("[sh, stubs/fails.sh]"), None, "", "");
    let tree = answer_tree(
        "person-program-free-fails",
        &bindings,
        &[("fails.sh", EXIT_3)],
    );
    waiting(&tree);
    let before = log_lines(&tree, "root").len();
    let words = approve_words(&tree, "answer.free", None, OWNER, &[]);
    let refused = run_words(&tree, &words);
    let refusal = program_failed(&refused, &["answer.free", "exit 3"]);
    assert_eq!(refusal["command"], json!(words), "{refusal}");
    assert!(
        b10x_loom_tree::read_approvals(&tree.store(), "root")
            .unwrap()
            .is_empty()
    );
    assert!(records_of(&tree, "root", "answer_freed").is_empty());
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 1, "{written:?}");
    assert!(!written.iter().any(is_authority), "{written:?}");
    let ended = one_finished(&written);
    assert_eq!(ended.exit, Some(3), "{ended:?}");
    assert!(ended.evidence.is_empty(), "{ended:?}");
}

#[test]
fn a_person_program_killed_at_its_timeout_records_nothing() {
    let bindings = answer_bindings(
        OWNER,
        Some("[sh, stubs/sleeps.sh]"),
        None,
        None,
        "    timeout_seconds: 1\n",
        "",
    );
    let tree = answer_tree(
        "person-program-timeout",
        &bindings,
        &[("sleeps.sh", "#!/bin/sh\nexec /usr/bin/sleep 30\n")],
    );
    waiting(&tree);
    let started = Instant::now();
    let refused = run_words(
        &tree,
        &approve_words(&tree, "answer.take", Some("owner.answer"), OWNER, &[]),
    );
    let took = started.elapsed();
    program_failed(&refused, &["timed out"]);
    assert!(took < Duration::from_secs(20), "took {took:?}");
    assert!(records_of(&tree, "root", "answer_taken").is_empty());
    let ended = one_finished(&log_lines(&tree, "root"));
    assert_eq!(ended.refusal.as_deref(), Some("timed out"), "{ended:?}");
}

#[test]
fn a_person_program_of_a_reader_that_writes_records_nothing() {
    let bindings = answer_bindings(
        OWNER,
        None,
        None,
        Some("[sh, stubs/answer.sh, \"{principal}\"]"),
        "",
        "",
    );
    let tree = answer_tree("person-program-reader", &bindings, &[("answer.sh", APPEND)]);
    waiting(&tree);
    let before = log_lines(&tree, "root").len();
    let refused = run_words(
        &tree,
        &approve_words(&tree, "answer.look", Some("owner.look"), OWNER, &[]),
    );
    program_failed(&refused, &["ChangedByNonWriter"]);
    let written = lines_since(&tree, "root", before);
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(is_authority(&written[0]), "{written:?}");
    let ended = one_finished(&written);
    assert_eq!(ended.exit, Some(0), "{ended:?}");
    assert!(ended.evidence.is_empty(), "{ended:?}");
    let runs = program_runs(&tree);
    assert_eq!(
        ended.blob_digest.as_deref(),
        runs[0]["blob_sha256"].as_str(),
        "{runs:?}"
    );
    assert!(
        !written
            .iter()
            .any(|line| matches!(line.event, RunLogEvent::Discarded(_))),
        "{written:?}"
    );
    assert_eq!(
        b10x_loom_tree::read_approvals(&tree.store(), "root")
            .unwrap()
            .len(),
        1
    );
    assert!(records_of(&tree, "root", "answer_looked").is_empty());
    assert_eq!(subject_revision(&tree), digest_now(&tree));
}

/// A runner that refuses every child it is given with `detail`, as a runner does for a child it
/// started and lost.
struct Refusing {
    detail: String,
    children: Mutex<Vec<ChildCommand>>,
}

impl TestRunner for Refusing {
    fn run(&self, _: &TestCommand, _: &Path) -> Result<TestExecution, ConfinementError> {
        Err(ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: "runs no test command".to_owned(),
        })
    }

    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.children.lock().unwrap().push(child.clone());
        Err(ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: self.detail.clone(),
        })
    }

    fn preflight_child(&self, _: &Path) -> Result<(), ConfinementError> {
        Ok(())
    }
}

/// The library's `approve` of `action` of `tree`'s root as `principal` with `runner`.
fn library_approve(
    tree: &Tree,
    action: &str,
    capability: Option<&str>,
    principal: &str,
    runner: Option<Arc<dyn TestRunner>>,
) -> Result<(), b10x_loom_tree::Refused> {
    let catalog = tree.catalog();
    let store = tree.store();
    approve(&ApproveRequest {
        store: &store,
        case: "root",
        capability,
        principal,
        decision: ApprovalDecision::Allow,
        action: Some(action),
        result: None,
        reasons: None,
        time: TreeTime::At(AT.to_owned()),
        catalog: &catalog,
        runner,
    })
    .map(drop)
}

#[test]
fn a_runner_refusal_of_a_person_program_records_nothing() {
    let bindings = answer_bindings(
        OWNER,
        Some("[sh, stubs/answer.sh, \"{principal}\"]"),
        None,
        None,
        "",
        "",
    );
    let tree = answer_tree("person-program-runner", &bindings, &[("answer.sh", APPEND)]);
    waiting(&tree);
    let refusing = Arc::new(Refusing {
        detail: "the launch was lost 41c7".to_owned(),
        children: Mutex::new(Vec::new()),
    });
    let refused = library_approve(
        &tree,
        "answer.take",
        Some("owner.answer"),
        OWNER,
        Some(Arc::clone(&refusing) as Arc<dyn TestRunner>),
    )
    .expect_err("refused");
    assert_eq!(refused.reason(), TreeRefusalReason::PersonProgramFailed);
    assert!(refused.0.detail.contains("41c7"), "{refused}");
    assert_eq!(refused.0.command, None, "the library sets no command");
    assert_eq!(refusing.children.lock().unwrap().len(), 1);
    let runs = program_runs(&tree);
    assert_eq!(runs.len(), 1, "{runs:?}");
    let runner_refusal = runs[0]["runner_refusal"].as_str().unwrap().to_owned();
    assert!(runner_refusal.contains("41c7"), "{runs:?}");
    assert_eq!(runs[0]["principal"], OWNER, "{runs:?}");
    let ended = one_finished(&log_lines(&tree, "root"));
    assert_eq!(ended.refusal.as_deref(), Some(runner_refusal.as_str()));
    assert!(ended.evidence.is_empty(), "{ended:?}");
    assert!(records_of(&tree, "root", "answer_taken").is_empty());
    assert_eq!(
        fs::read_to_string(tree.workspace().join(SUBJECT)).unwrap(),
        "question\n"
    );
}

#[test]
fn an_unreadable_placed_artifact_refuses_the_person_program_before_any_write() {
    let marker = "#!/bin/sh\n: > marker.txt\n";
    let bindings = answer_bindings(OWNER, Some("[sh, stubs/marker.sh]"), None, None, "", "");
    let tree = answer_tree(
        "person-program-unreadable",
        &bindings,
        &[("marker.sh", marker)],
    );
    waiting(&tree);
    let workspace = tree.workspace();
    fs::remove_dir_all(workspace.join("answer")).unwrap();
    fs::create_dir_all(tree.root.join("away")).unwrap();
    fs::write(tree.root.join("away/subject.txt"), "away\n").unwrap();
    std::os::unix::fs::symlink(tree.root.join("away"), workspace.join("answer")).unwrap();
    let before = every_file(&tree.store());
    let refused = run_words(
        &tree,
        &approve_words(&tree, "answer.take", Some("owner.answer"), OWNER, &[]),
    );
    program_failed(&refused, &["ArtifactUnreadable", "OutsideWorkspace"]);
    assert_eq!(every_file(&tree.store()), before, "a refusal wrote");
    assert!(
        !workspace.join("marker.txt").exists(),
        "the program started"
    );
}

#[test]
fn a_person_program_is_retried_by_approving_again_within_max_runs() {
    let bindings = answer_bindings(
        OWNER,
        Some("[sh, stubs/marked.sh]"),
        None,
        None,
        "    budget: {max_runs: 2}\n",
        "",
    );
    let marked = "#!/bin/sh\ntest -f marker.txt || exit 4\n";
    let tree = answer_tree("person-program-retry", &bindings, &[("marked.sh", marked)]);
    waiting(&tree);
    let words = approve_words(&tree, "answer.take", Some("owner.answer"), OWNER, &[]);
    for _ in 0..2 {
        program_failed(&run_words(&tree, &words), &["exit 4"]);
    }
    let before = every_file(&tree.store());
    let spent = run_words(&tree, &words);
    let refusal = program_failed(&spent, &["RunsSpent"]);
    assert_eq!(refusal["command"], json!(words), "{refusal}");
    assert_eq!(every_file(&tree.store()), before, "a spent action wrote");
    assert_eq!(program_runs(&tree).len(), 2);

    let tree = answer_tree(
        "person-program-retried",
        &bindings,
        &[("marked.sh", marked)],
    );
    waiting(&tree);
    let words = approve_words(&tree, "answer.take", Some("owner.answer"), OWNER, &[]);
    program_failed(&run_words(&tree, &words), &["exit 4"]);
    fs::write(tree.workspace().join("marker.txt"), "").unwrap();
    let taken = run_words(&tree, &words);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    assert_eq!(records_of(&tree, "root", "answer_taken").len(), 1);
}

#[test]
fn a_person_program_without_a_runner_is_refused_before_anything_is_written() {
    let bindings = answer_bindings(OWNER, Some("[sh, stubs/answer.sh]"), None, None, "", "");
    let tree = answer_tree(
        "person-program-no-runner",
        &bindings,
        &[("answer.sh", APPEND)],
    );
    waiting(&tree);
    let catalog = tree.catalog();
    assert!(person_program(&tree.store(), "root", "answer.take", &catalog).unwrap());
    assert!(!person_program(&tree.store(), "root", "answer.free", &catalog).unwrap());
    let before = every_file(&tree.store());
    let refused = library_approve(&tree, "answer.take", Some("owner.answer"), OWNER, None)
        .expect_err("refused");
    assert_eq!(refused.reason(), TreeRefusalReason::ConfinementRefused);
    assert!(refused.0.detail.contains("NoRunner"), "{refused}");
    assert_eq!(every_file(&tree.store()), before, "a refusal wrote");
    assert_eq!(
        fs::read_to_string(tree.workspace().join(SUBJECT)).unwrap(),
        "question\n"
    );

    // A person action with no argv needs no runner and records as before.
    let (sign, _) = signpost("person-program-no-runner-sign");
    let catalog = sign.catalog();
    assert!(!person_program(&sign.store(), "root", "post.sign", &catalog).unwrap());
    library_approve(&sign, "post.sign", None, "walker", None).unwrap();
    assert_eq!(records_of(&sign, "root", "post_signed").len(), 1);
}

#[test]
fn the_printed_approve_action_command_carries_the_confinement() {
    let bindings = answer_bindings(OWNER, Some("[sh, stubs/answer.sh]"), None, None, "", "");
    let tree = answer_tree(
        "person-program-printed",
        &bindings,
        &[("answer.sh", APPEND)],
    );
    let decisions = waiting(&tree);
    let take = about(&decisions, "answer.take");
    let words: Vec<&str> = take["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap())
        .collect();
    assert!(
        words
            .windows(2)
            .any(|pair| pair == ["--confinement", "local"]),
        "{take}"
    );
    // A person action without an argv prints no confinement, and its `tree approve` never starts
    // itself again: run with no `systemd-run` on its `PATH`, it takes the action.
    let free = about(&decisions, "answer.free");
    assert!(
        !free["command"]
            .as_array()
            .unwrap()
            .iter()
            .any(|word| word == "--confinement"),
        "{free}"
    );
    let taken = tree.printed(&take);
    assert_eq!(
        taken.status.code(),
        Some(0),
        "{}{}",
        stdout(&taken),
        stderr(&taken)
    );
    assert_eq!(records_of(&tree, "root", "answer_taken").len(), 1);
    let empty = tree.root.join("empty-path");
    fs::create_dir_all(&empty).unwrap();
    let free_words: Vec<String> = free["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| word.as_str().unwrap().to_owned())
        .collect();
    let freed = tree
        .command()
        .env("PATH", &empty)
        .args(&free_words[1..])
        .output()
        .unwrap();
    assert_eq!(
        freed.status.code(),
        Some(0),
        "{}{}",
        stdout(&freed),
        stderr(&freed)
    );
    assert_eq!(records_of(&tree, "root", "answer_freed").len(), 1);
}
