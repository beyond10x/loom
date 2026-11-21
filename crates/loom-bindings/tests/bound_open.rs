//! Acceptance for `story:child-case-runtime` in the bindings: an action bound to an `open`
//! executor, performed through Commission's own loop (`run_until_blocked`) and the open effect
//! port, opens one child case per name under the protocol its `case_open` names, with the
//! `independence` map of the child's bindings file, read relative to the directory of the parent's
//! bindings file; the load check holds `open` to `case_open`.
//!
//! The fixture protocols are the governor's (`crates/loom-governor/tests/fixtures/child-cases/`):
//! `parent/1` opens `child/1` cases. No model, no network, no process started; scratch under
//! `CARGO_TARGET_TMPDIR`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_bindings::{
    MAX_DOCUMENT_BYTES, OpenEffects, Refusal, check, check_case_open, decode,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, EvidenceId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Truth,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason};
use loom_governor::{CanonGovernor, CaseInputs, FileCaseStore};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::json;

const PARENT: &str = include_str!("../../loom-governor/tests/fixtures/child-cases/parent-1.yaml");
const CHILD: &str = include_str!("../../loom-governor/tests/fixtures/child-cases/child-1.yaml");
const OPEN: &str = "child.open";

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "bound-open-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog_of(entries: &[(&str, &str)]) -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    for (name, yaml) in entries {
        catalog.add_yaml(name, yaml, source()).unwrap();
    }
    catalog
}

fn catalog() -> ProtocolCatalog {
    catalog_of(&[("parent@1", PARENT), ("child@1", CHILD)])
}

fn parent_protocol() -> b10x_canon::model::Protocol {
    b10x_canon::model::parse(PARENT).unwrap()
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A scratch directory: `conf/` holds the bindings files, `store/` the cases.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "bound-open-{}-{}",
            std::process::id(),
            SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("conf/sub")).unwrap();
        std::fs::create_dir_all(root.join("store")).unwrap();
        Self { root }
    }

    fn write(&self, path: &str, text: &str) -> PathBuf {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn store(&self) -> PathBuf {
        self.root.join("store")
    }
}

fn child_bindings(producer: &str) -> String {
    format!(
        "format: loom.bindings/1\nprotocol: child@1\nindependence:\n  {producer}: [different_agent_run]\nactions: []\n"
    )
}

/// The parent's document: `child.open` bound to `open` with `executor`, and its `from: effect`
/// entry recording `child_opened`.
fn parent_bindings(executor: &str) -> String {
    format!(
        "format: loom.bindings/1\nprotocol: parent@1\nartifacts:\n  idea: idea.md\nactions:\n\
         \x20 - action: {OPEN}\n    executor: {executor}\n    evidence:\n\
         \x20     - {{kind: child_opened, subject: idea, producer: loom, from: effect, results: {{performed: opened}}}}\n"
    )
}

fn loaded(text: &str) -> BindingsDocument {
    let document = decode(text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(&document, "parent@1", &parent_protocol())
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

fn governor(scratch: &Scratch) -> (CanonGovernor<FileCaseStore>, CaseId) {
    let governor = CanonGovernor::new(FileCaseStore::open(scratch.store()).unwrap())
        .with_catalog(&catalog())
        .unwrap();
    let case = CaseId("case-1".into());
    governor
        .open_case(
            case.clone(),
            "parent@1",
            BTreeMap::from([("idea".to_owned(), "idea-1".to_owned())]),
            CaseInputs::none(),
        )
        .unwrap();
    (governor, case)
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Context;

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed)))
    }
    fn observation_id(&mut self) -> ObservationId {
        ObservationId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed)))
    }
    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T12:00:00Z".to_owned())
    }
    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// One step of `case` in which the executor proposes `child.open` with `arguments`.
fn drive(
    governor: &CanonGovernor<FileCaseStore>,
    effects: &OpenEffects<'_, FileCaseStore>,
    case: &CaseId,
    arguments: serde_json::Value,
) -> LoopEnd {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed))),
        agent_revision_id: AgentRevisionId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed))),
        case_id: case.clone(),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: OPEN.to_owned(),
            arguments: ProposedActionArguments(json::parse(&arguments.to_string()).unwrap()),
        },
    )]);
    let mut runs = Generated::new(RunStore::new(|| {
        RunId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed)))
    }));
    run_until_blocked(
        governor,
        &executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission,
        &mut runs,
        &mut Context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn performed(end: &LoopEnd) {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    assert!(
        matches!(end.effects[0], EffectOutcome::Performed(_)),
        "{:?}",
        end.effects
    );
}

fn kinds(governor: &CanonGovernor<FileCaseStore>, case: &CaseId) -> Vec<String> {
    governor
        .evidence(case)
        .unwrap()
        .into_iter()
        .map(|record| record.kind)
        .collect()
}

fn claim(governor: &CanonGovernor<FileCaseStore>, case: &CaseId, claim: &str) -> Truth {
    governor
        .frontier(case)
        .unwrap()
        .data()
        .claims
        .iter()
        .find(|entry| entry.claim == claim)
        .unwrap()
        .value
}

#[test]
fn an_each_argument_with_three_entries_opens_three_children_and_three_records() {
    let scratch = Scratch::new();
    scratch.write("conf/sub/child.yaml", &child_bindings("reviewer"));
    let text = parent_bindings("{kind: open, each: names, bindings: sub/child.yaml}");
    let file = scratch.write("conf/parent.yaml", &text);
    let document = loaded(&text);
    let (governor, case) = governor(&scratch);
    let effects = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog()).unwrap();
    performed(&drive(
        &governor,
        &effects,
        &case,
        json!({"names": ["a", "b", "c"]}),
    ));
    let tree: Vec<String> = governor
        .tree(&case)
        .unwrap()
        .into_iter()
        .map(|member| member.case.0)
        .collect();
    assert_eq!(tree, ["case-1", "case-1.a", "case-1.b", "case-1.c"]);
    for child in &tree[1..] {
        assert_eq!(
            governor.protocol(&CaseId(child.clone())).unwrap(),
            "child@1"
        );
    }
    let held = kinds(&governor, &case);
    assert_eq!(
        held.iter().filter(|kind| *kind == "case_outcome").count(),
        3,
        "{held:?}"
    );
    assert_eq!(
        held.iter().filter(|kind| *kind == "child_opened").count(),
        1,
        "{held:?}"
    );
    assert_eq!(claim(&governor, &case, "child.opened"), Truth::True);
    assert_eq!(claim(&governor, &case, "children.passed"), Truth::Unknown);
}

#[test]
fn a_named_child_opened_again_gets_the_next_numbered_id() {
    let scratch = Scratch::new();
    scratch.write("conf/sub/child.yaml", &child_bindings("reviewer"));
    let text = parent_bindings("{kind: open, name: review, bindings: sub/child.yaml}");
    let file = scratch.write("conf/parent.yaml", &text);
    let document = loaded(&text);
    let (governor, case) = governor(&scratch);
    let effects = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog()).unwrap();
    performed(&drive(&governor, &effects, &case, json!({})));
    performed(&drive(&governor, &effects, &case, json!({})));
    let tree: Vec<String> = governor
        .tree(&case)
        .unwrap()
        .into_iter()
        .map(|member| member.case.0)
        .collect();
    assert_eq!(tree, ["case-1", "case-1.review", "case-1.review.2"]);
}

/// `child/1` declares `work`, which `parent/1` does not: whether the child's bindings file places
/// it at a path that exists, at one that does not exist yet, or at no path, the child opens with
/// it at `r0` (`UNREVISED`); the placed path is not read.
#[test]
fn a_child_artifact_the_parent_does_not_hold_opens_at_r0_wherever_it_is_placed() {
    for placement in [
        "artifacts:\n  work: work.md\n",
        "artifacts:\n  work: missing.md\n",
        "",
    ] {
        let scratch = Scratch::new();
        scratch.write("conf/sub/work.md", "the work\n");
        let child = child_bindings("reviewer").replace("actions: []\n", "");
        scratch.write(
            "conf/sub/child.yaml",
            &format!("{child}{placement}actions: []\n"),
        );
        let text = parent_bindings("{kind: open, name: review, bindings: sub/child.yaml}");
        let file = scratch.write("conf/parent.yaml", &text);
        let document = loaded(&text);
        let (governor, case) = governor(&scratch);
        let effects =
            OpenEffects::new(&governor, case.clone(), &document, &file, &catalog()).unwrap();
        performed(&drive(&governor, &effects, &case, json!({})));
        let opened = governor
            .case_state(&CaseId("case-1.review".into()))
            .unwrap();
        assert_eq!(
            opened.artifacts,
            BTreeMap::from([("work".to_owned(), loom_governor::UNREVISED.to_owned())]),
            "{placement:?}"
        );
    }
}

/// An observation the governor received, for a submitted record to cite.
fn observed(governor: &CanonGovernor<FileCaseStore>) -> ObservationId {
    let observation = ObservationId(uuid(ISSUED.fetch_add(1, Ordering::Relaxed)));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:bound-open".into(),
            subject: "work".into(),
            observed_at: Timestamp("2026-11-21T12:00:00Z".into()),
            payload: Value::Null,
        }),
    )
    .unwrap();
    observation
}

/// Submits a passing review of the child's `work` by `producer`.
fn review(governor: &CanonGovernor<FileCaseStore>, case: &CaseId, producer: &str) {
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let revision = governor.revisions(case).unwrap()["work"].clone();
    submit_evidence(
        governor,
        producer,
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: "review".into(),
            subject_revision: governor.current_revision(case).unwrap(),
            producer: producer.into(),
            observation_ids: vec![observed(governor)],
            facts: json::parse(
                &json!({
                    "format": "canon-evidence/1",
                    "id": format!("review-{n}"),
                    "kind": "review",
                    "subject": "work",
                    "subject_revision": revision,
                    "result": "pass",
                })
                .to_string(),
            )
            .unwrap(),
            provenance: Value::Null,
        },
    )
    .unwrap();
}

#[test]
fn the_child_is_opened_with_the_independence_of_the_bindings_file_beside_the_parents() {
    let scratch = Scratch::new();
    // The file the parent names, beside it; a file of the same name in the workspace and in the
    // working directory, each with another map, are never read.
    scratch.write("conf/sub/child.yaml", &child_bindings("reviewer"));
    scratch.write("workspace/sub/child.yaml", &child_bindings("intruder"));
    scratch.write("elsewhere/sub/child.yaml", &child_bindings("intruder"));
    std::env::set_current_dir(scratch.root.join("elsewhere")).unwrap();
    let text = parent_bindings("{kind: open, name: review, bindings: sub/child.yaml}");
    let file = scratch.write("conf/parent.yaml", &text);
    let document = loaded(&text);
    let (governor, case) = governor(&scratch);
    let effects = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog()).unwrap();
    performed(&drive(&governor, &effects, &case, json!({})));
    let child = CaseId("case-1.review".into());
    let declared: Vec<String> = governor
        .independence(&child)
        .unwrap()
        .iter()
        .map(|(producer, _)| producer.to_owned())
        .collect();
    assert_eq!(declared, ["reviewer"]);
    review(&governor, &child, "intruder");
    assert_eq!(claim(&governor, &child, "work.reviewed"), Truth::Unknown);
    review(&governor, &child, "reviewer");
    assert_eq!(claim(&governor, &child, "work.reviewed"), Truth::True);
}

fn refused_by_case_open(text: &str) -> Refusal {
    let document = decode(text).unwrap();
    check(&document, "parent@1", &parent_protocol()).unwrap();
    check_case_open(&document, &parent_protocol()).expect_err("refused")
}

#[test]
fn open_is_held_to_case_open_at_load() {
    let on_plain = "format: loom.bindings/1\nprotocol: parent@1\nactions:\n\
                    \x20 - action: idea.note\n    executor: {kind: open, name: x, bindings: c.yaml}\n";
    let refusal = refused_by_case_open(on_plain);
    assert_eq!(refusal.0.reason, Reason::MemberNotForExecutor, "{refusal}");
    assert_eq!(refusal.0.action.as_deref(), Some("idea.note"));
    assert_eq!(refusal.0.member, "executor.kind");
    let to_person = "format: loom.bindings/1\nprotocol: parent@1\nactions:\n\
                     \x20 - action: child.open\n    executor: {kind: person, principal: p}\n";
    let refusal = refused_by_case_open(to_person);
    assert_eq!(refusal.0.reason, Reason::MemberNotForExecutor, "{refusal}");
    assert_eq!(refusal.0.action.as_deref(), Some(OPEN));
    assert_eq!(refusal.0.member, "executor.kind");
    let outcome_entry = "format: loom.bindings/1\nprotocol: parent@1\nactions:\n\
                         \x20 - action: idea.note\n    executor: {kind: person, principal: p}\n    evidence:\n\
                         \x20     - {kind: child_opened, subject: idea, producer: p, from: effect, results: {performed: x}}\n\
                         \x20     - {kind: case_outcome, subject: idea, producer: p, from: effect, results: {performed: passed}}\n";
    let document = decode(outcome_entry).unwrap();
    let refusal = check_case_open(&document, &parent_protocol()).expect_err("refused");
    assert_eq!(refusal.0.reason, Reason::KindNotProducible, "{refusal}");
    assert_eq!(refusal.0.member, "evidence[1].kind");
    let ok = parent_bindings("{kind: open, name: x, bindings: c.yaml}");
    check_case_open(&decode(&ok).unwrap(), &parent_protocol()).expect("open on case_open");
}

#[test]
fn a_child_protocol_the_catalog_does_not_hold_is_refused_before_anything_is_written() {
    let scratch = Scratch::new();
    scratch.write("conf/sub/child.yaml", &child_bindings("reviewer"));
    let text = parent_bindings("{kind: open, name: review, bindings: sub/child.yaml}");
    let file = scratch.write("conf/parent.yaml", &text);
    let document = loaded(&text);
    let (governor, case) = governor(&scratch);
    let listing = |dir: &Path| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let before = listing(&scratch.store());
    let without_child = catalog_of(&[("parent@1", PARENT)]);
    let refused = OpenEffects::new(&governor, case.clone(), &document, &file, &without_child)
        .map(|_| ())
        .expect_err("the child protocol is not in the catalog");
    assert_eq!(refused.0.action.as_deref(), Some(OPEN), "{refused}");
    // A child bindings file for another protocol is refused at load too.
    scratch.write(
        "conf/sub/child.yaml",
        "format: loom.bindings/1\nprotocol: parent@1\nactions: []\n",
    );
    let refused = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog())
        .map(|_| ())
        .expect_err("the child file binds another protocol");
    assert_eq!(refused.0.reason, Reason::ProtocolMismatch, "{refused}");
    assert_eq!(listing(&scratch.store()), before);
    assert_eq!(kinds(&governor, &case), Vec::<String>::new());
}

/// The store directory's file names, sorted.
fn store_listing(scratch: &Scratch) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(scratch.store())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The child bindings document padded with a comment line to exactly `size` bytes.
fn child_bindings_of_size(size: usize) -> String {
    let document = child_bindings("reviewer");
    let padding = size - document.len() - "# \n".len();
    let text = format!("{document}# {}\n", "x".repeat(padding));
    assert_eq!(text.len(), size);
    text
}

#[test]
fn a_child_bindings_file_at_the_size_bound_is_read_and_one_over_it_is_refused_before_any_write() {
    let scratch = Scratch::new();
    let text = parent_bindings("{kind: open, name: review, bindings: sub/child.yaml}");
    let file = scratch.write("conf/parent.yaml", &text);
    let document = loaded(&text);
    let (governor, case) = governor(&scratch);
    let before = store_listing(&scratch);

    scratch.write(
        "conf/sub/child.yaml",
        &child_bindings_of_size(MAX_DOCUMENT_BYTES + 1),
    );
    let refused = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog())
        .map(|_| ())
        .expect_err("a child bindings file over the bound is refused");
    assert_eq!(refused.0.reason, Reason::Malformed, "{refused}");
    assert_eq!(refused.0.member, "executor.bindings", "{refused}");
    assert!(refused.to_string().contains("larger than"), "{refused}");
    assert_eq!(store_listing(&scratch), before);
    assert_eq!(kinds(&governor, &case), Vec::<String>::new());

    scratch.write(
        "conf/sub/child.yaml",
        &child_bindings_of_size(MAX_DOCUMENT_BYTES),
    );
    let effects = OpenEffects::new(&governor, case.clone(), &document, &file, &catalog())
        .expect("a child bindings file at the bound is read");
    performed(&drive(&governor, &effects, &case, json!({})));
    assert_eq!(
        governor.protocol(&CaseId("case-1.review".into())).unwrap(),
        "child@1"
    );
}
