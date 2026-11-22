//! Acceptance for `story:tree-suite-format`, library half: a `loom.tree-suite/1` document decodes
//! or is refused naming its member, its selections read what a cycle left and nothing else, and
//! its thirteen check kinds hold or name their difference. No tree runs and no program starts.
//!
//! The end state the checks read is made once per test run, not once per test: it is built in a
//! temporary directory under `CARGO_TARGET_TMPDIR` and renamed into place, keyed by the SHA-256 of
//! its sources (this file and the fixtures it reads), so concurrent test processes read only a
//! finished one. Building it takes milliseconds; each test asserts it ended within 5 s.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp as CommissionTimestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, ObservationData, ObservationId,
};
use b10x_loom_runlog::RunLog;
use b10x_loom_tree::model::{
    CheckDifference, DifferenceReason, SelectItem, SuiteCheck, SuiteRefusalReason, TreeSuite,
};
use b10x_loom_tree::suite::{
    Availability, AvailabilityEntry, Captured, CycleView, KindUse, SUITE_AVAILABILITY,
    capture_after, decode_suite, evaluate_checks, first_waiting, not_run_result, read_suite,
    select, summarize, summary_lines, waiting_result, when_holds,
};
use b10x_loom_tree::{OpenRequest, TreeTime, open_tree};
use loom::primitives::Timestamp;
use loom::runlog::{
    ActionFinished, ActionStarted, CaseOpened, ChildOpened, RunLogEvent, Stopped, Terminated,
};
use loom_governor::{
    CaseIndependence, CaseInputs, CaseState, FallibleCaseStore as _, FileCaseStore, HeldEvidence,
};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const HERE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/suite-format");
const TWO_LEVEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../loom-cli/tests/fixtures/tree-two-level"
);
const AT: &str = "2026-11-22T08:00:00Z";
const BOUND: Duration = Duration::from_secs(5);

/// The blob the root's first action left.
const BLOB: &str = "{\"summary\": \"3 traced to behaviour, 0 constraint, 0 dropped, 0 untraced\"}";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    for (name, file) in [("tour@1", "tour.yaml"), ("tour-leg@1", "tour-leg.yaml")] {
        let path = Path::new(TWO_LEVEL).join("protocols").join(file);
        catalog
            .add_yaml(
                name,
                &fs::read_to_string(&path).unwrap(),
                ProtocolSource {
                    kind: SourceKind::Local,
                    location: path.display().to_string(),
                    revision: "1".into(),
                    path: file.into(),
                },
            )
            .unwrap();
    }
    catalog
}

/// The directory `build` makes from the sources `inputs`, made once for every test process: in a
/// temporary directory renamed into place, keyed by the SHA-256 of the inputs.
fn made_once(label: &str, inputs: &[&[u8]], build: impl FnOnce(&Path)) -> PathBuf {
    let mut digest = Sha256::new();
    for input in inputs {
        digest.update((input.len() as u64).to_le_bytes());
        digest.update(input);
    }
    let key: String = digest
        .finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    let target = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{label}-{key}"));
    if target.is_dir() {
        return target;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let building = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!(".{label}-{key}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&building).unwrap();
    build(&building);
    if fs::rename(&building, &target).is_err() {
        let _ = fs::remove_dir_all(&building);
    }
    assert!(target.is_dir(), "the fixture {label} was not made");
    target
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn json_value(value: &Value) -> commission_json::Value {
    commission_json::parse(&value.to_string()).unwrap()
}

fn record(
    id: &str,
    kind: &str,
    result: &str,
    subject: &str,
    revision: &str,
    upstream: Value,
) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "result": result,
        "subject": subject,
        "subject_revision": revision,
        "upstream_revisions": upstream,
    })
}

/// Puts `case` with `records`, in that order, into the store `store` through the governor's
/// file store.
fn put_case(store: &Path, case: &str, protocol: &str, records: &[Value]) {
    let cases = FileCaseStore::open(store.to_path_buf()).unwrap();
    let evidence = records
        .iter()
        .enumerate()
        .map(|(index, facts)| HeldEvidence {
            data: EvidenceData {
                evidence_id: EvidenceId(Uuid(format!("00000000-0000-4000-8000-{:012}", index + 1))),
                case_id: CaseId(case.into()),
                kind: facts["kind"].as_str().unwrap().into(),
                subject_revision: 1,
                producer: "fixture".into(),
                observation_ids: Vec::new(),
                facts: json_value(facts),
                provenance: json_value(&json!({"source": "fixture"})),
            },
            applies: true,
        })
        .collect();
    cases
        .insert(CaseState {
            id: CaseId(case.into()),
            protocol: protocol.into(),
            revision: 1,
            artifacts: BTreeMap::new(),
            inputs: CaseInputs::none(),
            independence: CaseIndependence::none(),
            evidence,
            decisions: Vec::new(),
            parent: None,
            children: Vec::new(),
            termination: None,
        })
        .unwrap();
}

fn append(log: &RunLog, case: &str, event: RunLogEvent) {
    log.append(case, Timestamp(AT.into()), event).unwrap();
}

fn finished(action: &str, exit: i64, blob: Option<String>, evidence: &[&str]) -> RunLogEvent {
    RunLogEvent::ActionFinished(ActionFinished {
        action: action.into(),
        exit: Some(exit),
        refusal: None,
        blob_digest: blob,
        evidence: evidence.iter().map(|id| (*id).to_owned()).collect(),
        revisions: Vec::new(),
    })
}

fn started(action: &str) -> RunLogEvent {
    RunLogEvent::ActionStarted(ActionStarted {
        action: action.into(),
    })
}

/// The end state: a tree `trip` of `tour@1` opened by `tree open`, its log, two child cases
/// `trip.first` and `trip.north` of `tour-leg@1` with their logs, two records and two program
/// runs (the second retrying the first's record), two blobs (one whose bytes changed), and a
/// workspace with a `.yaml`, a `.jsonl`, a text file and a directory of files.
fn build_end_state(dir: &Path) {
    let work = dir.join("work");
    write(&work.join("route.txt"), "stop\nstop\n");
    write(
        &work.join("questions.yaml"),
        "questions:\n  - {id: q1, status: open}\n  - {id: q2, status: changed, answer: the billing clerk}\nlegs: [north, south]\n\"a.b\": {value: 1}\n",
    );
    write(
        &work.join("events.jsonl"),
        "{\"n\": 1, \"step\": \"plan\"}\n{\"n\": 2, \"step\": \"check\"}\n{\"n\": 3, \"step\": \"issue\"}\n",
    );
    write(&work.join("notes.txt"), "first line\nsecond line\n");
    write(&work.join("legs/north.yaml"), "leg: north\n");
    write(&work.join("legs/south.yaml"), "leg: south\n");
    write(
        &work.join("paths.json"),
        "{\"good\": \"notes.txt\", \"bad\": \"../../x\"}",
    );
    let later = dir.join("work-later");
    for leg in ["north", "south", "east"] {
        write(
            &later.join(format!("legs/{leg}.yaml")),
            &format!("leg: {leg}\n"),
        );
    }

    let catalog = catalog();
    let store = dir.join("store");
    open_tree(&OpenRequest {
        store: store.clone(),
        workspace: work.clone(),
        protocol: "tour@1".into(),
        bindings: Path::new(TWO_LEVEL).join("tour.bindings.yaml"),
        revisions: Vec::new(),
        case: "trip".into(),
        time: TreeTime::At(AT.into()),
        catalog: &catalog,
        protocols: Vec::new(),
    })
    .unwrap();

    let blob = sha256(BLOB.as_bytes());
    write(&store.join("blobs").join(&blob), BLOB);
    let changed = sha256(b"original");
    write(&store.join("blobs").join(&changed), "changed");

    let log = RunLog::open(store.clone()).unwrap();
    append(&log, "trip", started("first.open"));
    append(
        &log,
        "trip",
        finished("first.open", 0, Some(blob.clone()), &[]),
    );
    for child in ["trip.first", "trip.north"] {
        append(
            &log,
            "trip",
            RunLogEvent::ChildOpened(ChildOpened {
                child: child.into(),
                protocol: "tour-leg@1".into(),
                record: None,
                record_digest: None,
            }),
        );
        append(
            &log,
            child,
            RunLogEvent::CaseOpened(CaseOpened {
                case: child.into(),
                protocol: "tour-leg@1".into(),
            }),
        );
    }
    append(
        &log,
        "trip",
        RunLogEvent::Stopped(Stopped {
            stop: "AwaitingPerson".into(),
            next: Vec::new(),
            budget: None,
        }),
    );
    append(
        &log,
        "trip",
        RunLogEvent::Terminated(Terminated {
            reason: "arrived".into(),
        }),
    );
    append(&log, "trip.first", started("route.write"));
    append(&log, "trip.first", finished("route.write", 0, None, &[]));
    append(&log, "trip.first", started("route.check"));
    append(
        &log,
        "trip.first",
        finished("route.check", 1, None, &["ev-route-check-1"]),
    );
    append(
        &log,
        "trip.north",
        finished("leg.rest", 0, Some(changed), &[]),
    );

    put_case(
        &store,
        "trip.first",
        "tour-leg@1",
        &[
            record(
                "ev-route-check-1",
                "route_check",
                "invalid",
                "route",
                "r1",
                json!({}),
            ),
            record(
                "ev-route-shape-1",
                "route_shape",
                "short",
                "route",
                "r1",
                json!({}),
            ),
        ],
    );
    let cases = FileCaseStore::open(store.clone()).unwrap();
    for (index, (exit, retry_of)) in [(1, json!([])), (0, json!(["ev-route-check-1"]))]
        .into_iter()
        .enumerate()
    {
        cases
            .observe(ObservationData {
                observation_id: ObservationId(Uuid(format!(
                    "00000000-0000-4000-9000-{:012}",
                    index + 1
                ))),
                source: "loom/bound-program".into(),
                subject: "route.check".into(),
                observed_at: CommissionTimestamp(AT.into()),
                payload: json_value(&json!({
                    "case": "trip.first",
                    "action": "route.check",
                    "exit": exit,
                    "blob_sha256": null,
                    "timed_out": false,
                    "runner_refusal": null,
                    "retry_of": retry_of,
                })),
            })
            .unwrap();
    }
}

/// The four records of a reworked bug fix, and each variant that breaks one condition alone.
fn bug_fixes() -> Vec<(&'static str, Vec<Value>)> {
    let red = |change: &str| {
        record(
            "a",
            "repro_run",
            "red",
            "repro",
            "r1",
            json!({"change": change}),
        )
    };
    let failed = record(
        "t1",
        "test_run",
        "failed",
        "change",
        "c1",
        json!({"repro": "r1"}),
    );
    let change = |at: &str, repro: &str| {
        record(
            "b",
            "implementation_change",
            "applied",
            "change",
            at,
            json!({"repro": repro}),
        )
    };
    let green = |at: &str, repro: &str| {
        record(
            "c",
            "test_run",
            "passed",
            "change",
            at,
            json!({"repro": repro}),
        )
    };
    vec![
        (
            "reworked",
            vec![
                red("c1"),
                failed.clone(),
                change("c2", "r1"),
                green("c2", "r1"),
            ],
        ),
        (
            "green-before-change",
            vec![
                red("c1"),
                failed.clone(),
                green("c2", "r1"),
                change("c2", "r1"),
            ],
        ),
        (
            "green-tests-r0",
            vec![
                red("c1"),
                failed.clone(),
                change("c2", "r1"),
                green("c2", "r0"),
            ],
        ),
        (
            "change-answers-r0",
            vec![
                red("c1"),
                failed.clone(),
                change("c2", "r0"),
                green("c2", "r1"),
            ],
        ),
        (
            "change-at-c3",
            vec![
                red("c1"),
                failed.clone(),
                change("c3", "r1"),
                green("c2", "r1"),
            ],
        ),
        (
            "green-at-none",
            vec![
                red("c1"),
                failed.clone(),
                change("none", "r1"),
                green("none", "r1"),
            ],
        ),
        (
            "green-at-c1",
            vec![
                red("c1"),
                failed.clone(),
                change("c1", "r1"),
                green("c1", "r1"),
            ],
        ),
        (
            "no-red",
            vec![failed, change("c2", "r1"), green("c2", "r1")],
        ),
    ]
}

fn build_bug_fixes(dir: &Path) {
    for (name, records) in bug_fixes() {
        let store = dir.join(name).join("store");
        fs::create_dir_all(&store).unwrap();
        write(
            &store.join("tree.json"),
            &json!({
                "format": "loom.tree/1",
                "root": "fix",
                "workspace": dir.join(name).join("work").display().to_string(),
                "bindings": dir.join(name).join("fix.bindings.yaml").display().to_string(),
                "catalog": "Host",
            })
            .to_string(),
        );
        fs::create_dir_all(dir.join(name).join("work")).unwrap();
        put_case(&store, "fix", "fix@1", &records);
    }
}

const SOURCE: &[u8] = include_bytes!("mod.rs");

fn end_state() -> PathBuf {
    made_once("suite-format-end-state", &[SOURCE], build_end_state)
}

/// One test's reading of a directory the fixture made.
struct Reading {
    dir: PathBuf,
    catalog: ProtocolCatalog,
    steps: BTreeMap<String, Value>,
    started: Instant,
}

impl Reading {
    fn of(dir: PathBuf) -> Self {
        Self {
            dir,
            catalog: catalog(),
            steps: BTreeMap::new(),
            started: Instant::now(),
        }
    }

    fn view_of<'a>(&'a self, work: &'a Path, store: &'a Path) -> CycleView<'a> {
        CycleView {
            work,
            store,
            suite: Path::new(HERE),
            steps: &self.steps,
            catalog: &self.catalog,
        }
    }

    fn within_bound(&self) {
        assert!(
            self.started.elapsed() < BOUND,
            "took {:?}",
            self.started.elapsed()
        );
    }
}

fn kinds() -> TreeSuite {
    read_suite(&Path::new(HERE).join("kinds.suite.yaml")).unwrap()
}

/// The checks of the one cycle of a suite whose `expect.checks` is `checks`.
fn checks_of(checks: &str) -> Vec<SuiteCheck> {
    let suite = decode_suite(&format!(
        "format: loom.tree-suite/1\ndefaults: {{protocol: tour@1, bindings: b.yaml}}\ncycles:\n  - name: one\n    expect:\n      checks:\n{checks}"
    ))
    .unwrap();
    suite.cycles[0].expect.checks.clone()
}

/// The selection `items`, written as the `select` of a check.
fn selection(items: &str) -> Vec<SelectItem> {
    checks_of(&format!("        - {{check: exists, select: {items}}}\n"))[0]
        .select
        .clone()
}

fn first_difference(reading: &Reading, checks: &[SuiteCheck]) -> Option<CheckDifference> {
    let work = reading.dir.join("work");
    let store = reading.dir.join("store");
    evaluate_checks(
        checks,
        "expect",
        &reading.view_of(&work, &store),
        &Captured::new(),
    )
}

#[test]
fn each_check_kind_holds() {
    let reading = Reading::of(end_state());
    let suite = kinds();
    let mut held = 0;
    for cycle in suite.cycles.iter().filter(|c| c.name.ends_with("-holds")) {
        assert_eq!(
            first_difference(&reading, &cycle.expect.checks),
            None,
            "{}",
            cycle.name
        );
        held += 1;
    }
    assert_eq!(held, 13);
    reading.within_bound();
}

#[test]
fn each_check_kind_fails_by_name() {
    let reading = Reading::of(end_state());
    let suite = kinds();
    let expected = [
        ("exists", "leg-rested", "at least one value"),
        ("count", "three-children", "3"),
        ("equals", "q2-open", "open"),
        ("contains", "third-line", "third line"),
        ("lacks", "no-second-line", "second line"),
        ("matches", "answer-a", "^a "),
        ("not_matches", "answer-no-clerk", "clerk"),
        (
            "all",
            "every-case-a-leg",
            "every value holds where, at least one value",
        ),
        ("none", "no-route-check", "no value holds where"),
        ("some", "a-run-exited-2", "at least one value holds where"),
        ("sequence", "root-starts-stopped", "starts [\"Stopped\"]"),
        (
            "before",
            "child-then-started",
            "a value holding then after the last holding first",
        ),
        ("compare", "fewer-cases-than-legs", "lt 3"),
    ];
    for (kind, check, value) in expected {
        let cycle = suite
            .cycles
            .iter()
            .find(|c| c.name == format!("{kind}-fails"))
            .unwrap();
        let difference = first_difference(&reading, &cycle.expect.checks)
            .unwrap_or_else(|| panic!("{kind}-fails is as expected"));
        assert_eq!(
            (
                difference.check.as_str(),
                difference.kind.as_str(),
                difference.expected.as_str()
            ),
            (check, kind, value),
            "{kind}"
        );
        assert_eq!(difference.reason, DifferenceReason::CheckFailed, "{kind}");
    }
    reading.within_bound();
}

#[test]
fn order_and_quantifiers_mean_what_they_say() {
    let reading = Reading::of(end_state());
    let between = checks_of(
        "        - {check: before, name: no-finish-between, select: [{log: {case: \".\"}}], first: {kind: ActionStarted}, then: {kind: ChildOpened}, between_none: {kind: ActionFinished}}\n",
    );
    let difference = first_difference(&reading, &between).unwrap();
    assert_eq!(difference.check, "no-finish-between");
    assert_eq!(
        difference.expected,
        "no value holding between_none between first and then"
    );
    assert!(difference.found.unwrap().contains(" bytes, sha256 "));
    let without = checks_of(
        "        - {check: before, select: [{log: {case: \".\"}}], first: {kind: ActionStarted}, then: {kind: ChildOpened}, between_none: {kind: Stopped}}\n",
    );
    assert_eq!(first_difference(&reading, &without), None);
    let empty = checks_of(
        "        - {check: all, name: all-of-nothing, select: [{log: {case: first, event: Terminated}}], where: {kind: Terminated}}\n",
    );
    let difference = first_difference(&reading, &empty).unwrap();
    assert_eq!(
        (difference.check.as_str(), difference.found.as_deref()),
        ("all-of-nothing", Some("0 of 0 hold"))
    );
    reading.within_bound();
}

fn values(reading: &Reading, items: &str) -> Vec<Value> {
    let work = reading.dir.join("work");
    let store = reading.dir.join("store");
    select(&selection(items), &reading.view_of(&work, &store))
        .unwrap()
        .into_iter()
        .map(|selected| selected.value)
        .collect()
}

#[test]
fn paths_select_what_they_name() {
    let reading = Reading::of(end_state());
    let doc = "{doc: {in: work, path: questions.yaml}}";
    assert_eq!(
        values(
            &reading,
            &format!("[{doc}, {{path: \"questions[id=q2].status\"}}]")
        ),
        vec![json!("changed")]
    );
    assert_eq!(
        values(&reading, &format!("[{doc}, {{path: \"questions[*].id\"}}]")),
        vec![json!("q1"), json!("q2")]
    );
    assert_eq!(
        values(
            &reading,
            &format!("[{doc}, {{path: \"questions[-1].id\"}}]")
        ),
        vec![json!("q2")]
    );
    assert_eq!(
        values(&reading, &format!("[{doc}, {{path: '\"a.b\".value'}}]")),
        vec![json!(1)]
    );
    assert_eq!(
        values(
            &reading,
            &format!("[{doc}, {{path: \"questions[id=q9].status\"}}]")
        ),
        Vec::<Value>::new()
    );
    assert_eq!(
        values(
            &reading,
            "[{doc: {in: work, path: events.jsonl}}, {path: step}]"
        ),
        vec![json!("plan"), json!("check"), json!("issue")]
    );
    assert_eq!(
        values(&reading, "[{files: {in: work, glob: \"legs/*\"}}]"),
        vec![json!("legs/north.yaml"), json!("legs/south.yaml")]
    );
    reading.within_bound();
}

#[test]
fn a_run_names_the_records_it_retries() {
    let reading = Reading::of(end_state());
    assert_eq!(
        values(
            &reading,
            "[{runs: {case: first}}, {where: {action: route.check}}, {pick: last}, {path: \"retry_of[*]\"}, {follow: evidence}, {path: kind}]"
        ),
        vec![json!("route_check")]
    );
    assert_eq!(
        values(
            &reading,
            "[{runs: {case: first}}, {pick: first}, {path: \"retry_of[*]\"}]"
        ),
        Vec::<Value>::new()
    );
    reading.within_bound();
}

#[test]
fn a_green_names_the_red_it_follows() {
    let fixes = made_once("suite-format-bug-fixes", &[SOURCE], build_bug_fixes);
    let reading = Reading::of(fixes.clone());
    let suite = read_suite(&Path::new(HERE).join("red-then-green.suite.yaml")).unwrap();
    let checks = &suite.cycles[0].expect.checks;
    assert_eq!(checks.len(), 9);
    let fails = |variant: &str| {
        let work = fixes.join(variant).join("work");
        let store = fixes.join(variant).join("store");
        evaluate_checks(
            checks,
            "expect",
            &reading.view_of(&work, &store),
            &Captured::new(),
        )
        .map(|difference| difference.check)
    };
    assert_eq!(fails("reworked"), None);
    for (variant, check) in [
        ("green-before-change", "change-then-green"),
        ("green-tests-r0", "green-tests-the-red"),
        ("change-answers-r0", "change-answers-the-red"),
        ("change-at-c3", "green-tests-the-change"),
        ("green-at-none", "green-has-a-revision"),
        ("green-at-c1", "green-is-new-code"),
        ("no-red", "red-then-change"),
    ] {
        assert_eq!(fails(variant).as_deref(), Some(check), "{variant}");
    }
    let work = fixes.join("no-red/work");
    let store = fixes.join("no-red/store");
    let first_compare = checks
        .iter()
        .find(|check| check.name.as_deref() == Some("green-tests-the-red"))
        .unwrap();
    let difference = evaluate_checks(
        std::slice::from_ref(first_compare),
        "expect",
        &reading.view_of(&work, &store),
        &Captured::new(),
    )
    .unwrap();
    assert_eq!(
        (
            difference.reason,
            difference.expected.as_str(),
            difference.found.as_deref()
        ),
        (
            DifferenceReason::OperandNotOne,
            "right: one value",
            Some("0")
        )
    );
    reading.within_bound();
}

#[test]
fn follow_stays_in_the_cycle() {
    let reading = Reading::of(end_state());
    assert_eq!(
        values(
            &reading,
            "[{log: {case: \".\", event: ActionFinished}}, {follow: blob}, {path: summary}]"
        ),
        vec![json!(
            "3 traced to behaviour, 0 constraint, 0 dropped, 0 untraced"
        )]
    );
    let changed = checks_of(
        "        - {check: exists, name: rested, select: [{log: {case: north, event: ActionFinished}}, {follow: blob}]}\n",
    );
    let difference = first_difference(&reading, &changed).unwrap();
    assert_eq!(
        (difference.check.as_str(), difference.reason),
        ("rested", DifferenceReason::BlobChanged)
    );
    assert_eq!(
        values(
            &reading,
            "[{doc: {in: work, path: paths.json}}, {path: good}, {follow: file}]"
        ),
        vec![json!("first line\nsecond line\n")]
    );
    let outside = checks_of(
        "        - {check: exists, name: escapes, select: [{doc: {in: work, path: paths.json}}, {path: bad}, {follow: file}]}\n",
    );
    let difference = first_difference(&reading, &outside).unwrap();
    assert_eq!(
        (difference.check.as_str(), difference.reason),
        ("escapes", DifferenceReason::PathOutsideCycle)
    );
    // A link in the workspace to a file outside it is not read through.
    let linked = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "suite-format-link-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    write(&linked.join("outside.txt"), "outside the cycle");
    fs::create_dir_all(linked.join("work")).unwrap();
    std::os::unix::fs::symlink(linked.join("outside.txt"), linked.join("work/leak.txt")).unwrap();
    let through_link = checks_of(
        "        - {check: exists, name: leaks, select: [{doc: {in: work, path: leak.txt}}]}\n",
    );
    let difference = evaluate_checks(
        &through_link,
        "expect",
        &reading.view_of(&linked.join("work"), &reading.dir.join("store")),
        &Captured::new(),
    )
    .unwrap();
    assert_eq!(
        (difference.check.as_str(), difference.reason),
        ("leaks", DifferenceReason::PathOutsideCycle)
    );
    let _ = fs::remove_dir_all(&linked);
    reading.within_bound();
}

/// A named pipe a program left in `work/`, the store or the suite's directory, named by any
/// source that reads a file, is refused `StepFailed` as no regular file and never opened for a
/// read that waits for a writer: no cycle wall time bounds the checks.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "makes a named pipe")]
fn a_selection_never_blocks_on_a_named_pipe() {
    use std::sync::mpsc;

    let dir = super::scratch("suite-fifo");
    let (work, store, suite) = (dir.join("work"), dir.join("store"), dir.join("suite"));
    let digest = "a".repeat(64);
    write(
        &store.join("tree.json"),
        r#"{"format":"loom.tree/1","root":"root","workspace":"/w","bindings":"/b","catalog":"Host"}"#,
    );
    write(&work.join("names.json"), r#"{"file": "pipe.txt"}"#);
    write(
        &work.join("digest.json"),
        &format!(r#"{{"blob_digest": "{digest}"}}"#),
    );
    fs::create_dir_all(store.join("blobs")).unwrap();
    fs::create_dir_all(&suite).unwrap();
    let stem = loom_governor::case_file_stem("root");
    // Each source that reads a file, and the file it reads.
    let sources: [(&str, PathBuf); 8] = [
        ("[{doc: {in: work, path: pipe.txt}}]", work.join("pipe.txt")),
        (
            "[{doc: {in: store, path: pipe.txt}}]",
            store.join("pipe.txt"),
        ),
        ("[{suite: pipe.txt}]", suite.join("pipe.txt")),
        (
            "[{log: {case: \".\"}}]",
            store.join(b10x_loom_tree::log_file_name("root")),
        ),
        (
            "[{records: {case: \".\"}}]",
            store.join(format!("{stem}.json")),
        ),
        ("[{runs: {case: \".\"}}]", store.join("observations.json")),
        (
            "[{doc: {in: work, path: names.json}}, {path: file}, {follow: file}]",
            work.join("pipe.txt"),
        ),
        (
            "[{doc: {in: work, path: digest.json}}, {follow: blob}]",
            store.join("blobs").join(&digest),
        ),
    ];
    let mut blocked = Vec::new();
    for (items, pipe) in sources {
        let made = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap();
        assert!(made.success(), "mkfifo {}", pipe.display());
        let selected = selection(items);
        let (sender, receiver) = mpsc::channel();
        let (work_in, store_in, suite_in) = (work.clone(), store.clone(), suite.clone());
        let reader = std::thread::spawn(move || {
            let steps = BTreeMap::new();
            let catalog = ProtocolCatalog::default();
            let view = CycleView {
                work: &work_in,
                store: &store_in,
                suite: &suite_in,
                steps: &steps,
                catalog: &catalog,
            };
            let _ = sender.send(select(&selected, &view));
        });
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(answer) => {
                let refused = answer.expect_err(items);
                assert!(
                    refused.starts_with("StepFailed: ") && refused.contains("not a regular file"),
                    "{items}: {refused}"
                );
            }
            Err(_) => {
                // Release the reader: an open for writing pairs with its blocked open, and the
                // close gives it its end of file.
                drop(fs::OpenOptions::new().write(true).open(&pipe).unwrap());
                blocked.push(items);
            }
        }
        reader.join().unwrap();
        fs::remove_file(&pipe).unwrap();
    }
    let _ = fs::remove_dir_all(&dir);
    assert!(
        blocked.is_empty(),
        "these selections blocked over 5 s on a named pipe: {blocked:#?}"
    );
}

#[test]
fn operands_read_the_state_after_a_step() {
    let reading = Reading::of(end_state());
    let suite = decode_suite(
        "format: loom.tree-suite/1\ncycles:\n  - name: grows\n    protocol: tour@1\n    bindings: b.yaml\n    steps: [{do: open, name: s1}, {do: run, name: s2}]\n    expect:\n      checks:\n        - check: compare\n          name: one-leg-more\n          left: {select: [{files: {in: work, glob: \"legs/*.yaml\"}}], take: count, after: s1, plus: 1}\n          op: eq\n          right: {select: [{files: {in: work, glob: \"legs/*.yaml\"}}], take: count}\n        - check: compare\n          name: as-many-legs\n          left: {select: [{files: {in: work, glob: \"legs/*.yaml\"}}], take: count, after: s1}\n          op: eq\n          right: {select: [{files: {in: work, glob: \"legs/*.yaml\"}}], take: count}\n",
    )
    .unwrap();
    let cycle = &suite.cycles[0];
    let store = reading.dir.join("store");
    let before = reading.dir.join("work");
    let after = reading.dir.join("work-later");
    let mut captured = Captured::new();
    capture_after(
        "s1",
        cycle,
        &reading.view_of(&before, &store),
        &mut captured,
    )
    .unwrap();
    assert_eq!(captured.len(), 2);
    let checks = &cycle.expect.checks;
    let view = reading.view_of(&after, &store);
    assert_eq!(
        evaluate_checks(&checks[..1], "expect", &view, &captured),
        None
    );
    let difference = evaluate_checks(&checks[1..], "expect", &view, &captured).unwrap();
    assert_eq!(
        (
            difference.check.as_str(),
            difference.expected.as_str(),
            difference.found.as_deref()
        ),
        ("as-many-legs", "eq 3", Some("2"))
    );
    reading.within_bound();
}

#[test]
fn differences_never_print_program_text() {
    let reading = Reading::of(end_state());
    let text = checks_of(
        "        - {check: equals, name: notes, select: [{doc: {in: work, path: notes.txt}}], is: nothing}\n",
    );
    let difference = first_difference(&reading, &text).unwrap();
    let found = difference.found.unwrap();
    let notes = "first line\nsecond line\n";
    assert_eq!(
        found,
        format!(
            "{} bytes, sha256 {}",
            notes.len(),
            &sha256(notes.as_bytes())[..12]
        )
    );
    assert!(!found.contains("line"));
    let kind = checks_of(
        "        - {check: equals, name: last-kind, select: [{log: {case: \".\"}}, {pick: -2}, {path: kind}], is: Terminated}\n",
    );
    let difference = first_difference(&reading, &kind).unwrap();
    assert_eq!(difference.found.as_deref(), Some("Stopped"));
    reading.within_bound();
}

#[test]
fn cycle_members_default_from_the_suite() {
    let started = Instant::now();
    let suite = decode_suite(
        "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml, fixtures: f0, decide: [{take: close, outcome: d0}]}\ncycles:\n  - name: plain\n  - name: own-fixtures\n    fixtures: f1\n  - name: own-decide\n    decide: [{take: close, outcome: own}]\n",
    )
    .unwrap();
    let plain = &suite.cycles[0];
    assert_eq!(
        (
            plain.bindings.as_deref(),
            plain.fixtures.as_deref(),
            plain.max_rounds
        ),
        (Some("b.yaml"), Some("f0"), 40)
    );
    assert_eq!(plain.steps.len(), 2);
    let own = &suite.cycles[1];
    assert_eq!(
        (own.bindings.as_deref(), own.fixtures.as_deref()),
        (Some("b.yaml"), Some("f1"))
    );
    let outcomes: Vec<String> = suite.cycles[2]
        .decide
        .iter()
        .map(|decision| match decision {
            b10x_loom_tree::model::ScriptedDecision::Close(close) => close.outcome.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(outcomes, ["own", "d0"]);
    let missing =
        decode_suite("format: loom.tree-suite/1\ncycles:\n  - {name: a, protocol: tour@1}\n  - {name: b, protocol: tour@1}\n")
            .unwrap_err();
    assert_eq!(
        (missing.reason(), missing.member()),
        (
            SuiteRefusalReason::MemberMissing,
            Some("cycles[0].bindings")
        )
    );
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_refused_suite_names_its_member() {
    let started = Instant::now();
    let head = "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml}\n";
    let check = |select: &str| {
        format!(
            "{head}cycles:\n  - name: one\n    expect:\n      checks:\n        - {{check: exists, select: {select}}}\n"
        )
    };
    let cases: Vec<(String, SuiteRefusalReason, &str)> = vec![
        (
            format!("{head}cycles:\n  - {{name: one, colour: red}}\n"),
            SuiteRefusalReason::MemberUndeclared,
            "cycles[0].colour",
        ),
        (
            "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml, expect: {exit: 0}}\ncycles: []\n".into(),
            SuiteRefusalReason::MemberUndeclared,
            "defaults.expect",
        ),
        (
            check("[{log: {case: \".\"}}, {records: {case: \".\"}}]"),
            SuiteRefusalReason::SelectUnusable,
            "cycles[0].expect.checks[0].select[1].records",
        ),
        (
            format!("{head}cycles:\n  - name: one\n    expect:\n      checks:\n        - {{check: matches, select: [{{doc: {{in: work, path: a.txt}}}}], regex: \"(a)\\\\1\"}}\n"),
            SuiteRefusalReason::RegexUnusable,
            "cycles[0].expect.checks[0].regex",
        ),
        (
            format!("{head}cycles:\n  - name: one\n    steps:\n      - do: open\n        checks:\n          - check: compare\n            left: {{select: [{{files: {{in: work, glob: \"*\"}}}}], take: count, after: later}}\n            op: eq\n            right: {{value: 1}}\n      - {{do: run, name: later}}\n"),
            SuiteRefusalReason::AfterUnknownStep,
            "cycles[0].steps[0].checks[0].left.after",
        ),
        (
            check("[{doc: {in: work, path: a.yaml}}, {path: \"questions[id=q2\"}]"),
            SuiteRefusalReason::PathUnusable,
            "cycles[0].expect.checks[0].select[1].path",
        ),
        (
            format!("{head}cycles:\n  - name: one\n    steps: [{{do: rewind}}]\n"),
            SuiteRefusalReason::StepKindUnknown,
            "cycles[0].steps[0].do",
        ),
        (
            check("[{logs: {case: \".\"}}]"),
            SuiteRefusalReason::SourceUnknown,
            "cycles[0].expect.checks[0].select[0].logs",
        ),
        (
            check("[{log: {case: \".\", event: Rewound}}]"),
            SuiteRefusalReason::EventKindUnknown,
            "cycles[0].expect.checks[0].select[0].log.event[0]",
        ),
        (
            "format: loom.tree-suite/1\ncycles:\n  - {name: one, protocol: tour@1, bindings: ../x.yaml}\n".into(),
            SuiteRefusalReason::PathOutsideSuite,
            "cycles[0].bindings",
        ),
        (
            format!("{head}cycles:\n  - name: one\n    seed: [{{from: seeds/a, to: ../w}}]\n"),
            SuiteRefusalReason::PathOutsideCycle,
            "cycles[0].seed[0].to",
        ),
        (
            format!("{head}cycles:\n  - name: A b\n"),
            SuiteRefusalReason::CycleNameUnusable,
            "cycles[0].name",
        ),
        (
            format!("{head}cycles:\n  - name: one\n  - name: one\n"),
            SuiteRefusalReason::CycleTwice,
            "cycles[1].name",
        ),
    ];
    for (suite, reason, member) in cases {
        let refused = decode_suite(&suite).unwrap_err();
        assert_eq!(
            (refused.reason(), refused.member()),
            (reason, Some(member)),
            "{suite}"
        );
    }
    assert!(started.elapsed() < BOUND);
}

#[test]
fn a_waiting_kind_makes_the_cycle_pending() {
    let started = Instant::now();
    let suite = decode_suite(
        "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml}\ncycles:\n  - name: replays\n    steps: [{do: open}, {do: run}, {do: replay, name: again}]\n  - name: plain\n",
    )
    .unwrap();
    let table = [
        AvailabilityEntry {
            used: KindUse::Step,
            name: "replay",
            availability: Availability::Waits("story:tree-replay-command"),
        },
        AvailabilityEntry {
            used: KindUse::Step,
            name: "open",
            availability: Availability::Available,
        },
    ];
    let waiting = first_waiting(&suite.cycles[0], &table).unwrap();
    assert_eq!(
        (waiting.story.as_str(), waiting.kind.as_str()),
        ("story:tree-replay-command", "replay")
    );
    assert_eq!(first_waiting(&suite.cycles[1], &table), None);
    assert_eq!(
        first_waiting(&suite.cycles[0], SUITE_AVAILABILITY).map(|w| w.story),
        Some("story:tree-replay-command".to_owned())
    );
    let report = summarize(vec![waiting_result(&suite.cycles[0], waiting)]);
    assert_eq!(
        (
            report.expected,
            report.as_expected,
            report.failed.len(),
            report.pending
        ),
        (0, 0, 0, 1)
    );
    assert!(report.cycles[0].pending && !report.cycles[0].run);
    assert_eq!(
        summary_lines(&report),
        [
            "cycles: 0 expected, 0 as expected, 0 failed: none".to_owned(),
            "pending: 1 (0 still failing: none; passing, move to expected: none; 1 waiting: replays on story:tree-replay-command)".to_owned(),
        ]
    );
    assert!(started.elapsed() < BOUND);
}

/// `story:suite-runs-cycles-in-parallel`, test 13: with `only` set the first summary line says how
/// many cycles were selected of how many; without it the line is the one of a full run.
#[test]
fn the_selection_line_names_the_selected_cycles() {
    let started = Instant::now();
    let suite = decode_suite(
        "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml}\ncycles:\n  - name: a\n  - name: b\n  - name: c\n  - name: d\n  - name: e\n",
    )
    .unwrap();
    let not_run = |index: usize| not_run_result(&suite.cycles[index], 0);
    let full = summarize(vec![not_run(1), not_run(3)]);
    assert_eq!(full.only, None);
    assert_eq!(
        summary_lines(&full)[0],
        "cycles: 0 expected, 0 as expected, 0 failed: none"
    );
    let mut selected = full.clone();
    selected.only = Some(vec!["b".to_owned(), "d".to_owned()]);
    selected.suite_cycles = 5;
    let line = &summary_lines(&selected)[0];
    assert!(line.starts_with("cycles: selected 2 of 5,"), "{line}");
    assert_eq!(
        line,
        "cycles: selected 2 of 5, 0 expected, 0 as expected, 0 failed: none"
    );
    assert!(started.elapsed() < BOUND);
}

/// `story:tree-suite-approve-result-and-reasons`, test 6: the `approve` members `result`,
/// `reasons` and `deny` run, in a step and in a scripted entry alike.
#[test]
fn the_approve_members_are_available() {
    let started = Instant::now();
    for member in ["approve.result", "approve.reasons", "approve.deny"] {
        let held: Vec<Availability> = SUITE_AVAILABILITY
            .iter()
            .filter(|entry| entry.used == KindUse::Member && entry.name == member)
            .map(|entry| entry.availability)
            .collect();
        assert_eq!(held, [Availability::Available], "{member}");
    }
    let suite = decode_suite(
        "format: loom.tree-suite/1\ndefaults: {protocol: review@1, bindings: b.yaml}\ncycles:\n  - name: step\n    steps: [{do: open}, {do: approve, action: draft.review, result: rejected, reasons: [a, b]}, {do: approve, capability: c, deny: true}]\n  - name: entry\n    decide: [{take: approve, action: draft.review, result: rejected, reasons: [a]}, {take: approve, capability: c, deny: true}]\n",
    )
    .unwrap();
    for cycle in &suite.cycles {
        assert_eq!(
            first_waiting(cycle, SUITE_AVAILABILITY),
            None,
            "{}",
            cycle.name
        );
    }
    assert!(started.elapsed() < BOUND);
}

/// Thirteen of Runbook's cycles, written in this format by Loom (`crates/loom-cli/tests/fixtures/
/// tree-suite/runbook/`), decode; the ones that use a part waiting for a story name it.
#[test]
fn runbooks_cycle_shapes_decode() {
    let started = Instant::now();
    let suite = read_suite(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../loom-cli/tests/fixtures/tree-suite/runbook/cycles.suite.yaml"),
    )
    .unwrap();
    assert_eq!(suite.cycles.len(), 13);
    let waiting: Vec<(String, String)> = suite
        .cycles
        .iter()
        .filter_map(|cycle| first_waiting(cycle, SUITE_AVAILABILITY))
        .map(|waiting| (waiting.name, waiting.kind))
        .collect();
    assert_eq!(
        waiting,
        [("replay-unchanged".to_owned(), "replay".to_owned())]
    );
    assert!(started.elapsed() < BOUND);
}

#[test]
fn when_reads_the_catalog_and_the_suite() {
    let reading = Reading::of(end_state());
    let suite = decode_suite(
        "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml}\ncycles:\n  - name: one\n    when:\n      - {protocol: tour@1, declares: {action: legs.open}}\n      - {protocol: tour@1, declares: {action: legs.close}}\n      - {protocol: tour, declares: {outcome: turned_back}}\n      - {suite: flags.yaml, path: runbook.converted, equals: true}\n      - {suite: flags.yaml, path: runbook.cycles, min: 200}\n",
    )
    .unwrap();
    let held: Vec<bool> = suite.cycles[0]
        .when
        .iter()
        .map(|condition| when_holds(condition, &reading.catalog, Path::new(HERE)))
        .collect();
    assert_eq!(held, [true, false, true, true, false]);
    let declared: BTreeSet<&str> = ["when"].into();
    assert!(SUITE_AVAILABILITY.iter().any(
        |entry| declared.contains(entry.name) && entry.availability == Availability::Available
    ));
    reading.within_bound();
}

/// `story:tree-suite-stop-resume-reopen-steps`, test 11: the three hold steps and the two hold
/// events run; `allow` and `Labelled` still wait; a hold step takes no `case`.
#[test]
fn the_stop_kinds_are_available() {
    let started = Instant::now();
    let availability = |used: KindUse, name: &str| {
        SUITE_AVAILABILITY
            .iter()
            .find(|entry| entry.used == used && entry.name == name)
            .unwrap_or_else(|| panic!("{name} is not in the table"))
            .availability
    };
    for step in ["stop", "resume", "reopen"] {
        assert_eq!(
            availability(KindUse::Step, step),
            Availability::Available,
            "{step}"
        );
    }
    for event in ["StopRecorded", "StopLifted"] {
        assert_eq!(
            availability(KindUse::Event, event),
            Availability::Available,
            "{event}"
        );
    }
    let waits = Availability::Waits("story:tree-stop-record-and-labels");
    assert_eq!(availability(KindUse::Step, "allow"), waits);
    assert_eq!(availability(KindUse::Event, "Labelled"), waits);
    let head = "format: loom.tree-suite/1\ndefaults: {protocol: tour@1, bindings: b.yaml}\n";
    for kind in ["stop", "resume", "reopen"] {
        let refused = decode_suite(&format!(
            "{head}cycles:\n  - name: one\n    steps: [{{do: open}}, {{do: {kind}, case: x}}]\n"
        ))
        .unwrap_err();
        assert_eq!(
            (refused.reason(), refused.member()),
            (
                SuiteRefusalReason::MemberUndeclared,
                Some("cycles[0].steps[1].case")
            ),
            "{kind}: {refused}"
        );
    }
    let decoded = decode_suite(&format!(
        "{head}cycles:\n  - name: one\n    steps: [{{do: open}}, {{do: stop, principal: p, reason: why}}, {{do: resume}}, {{do: reopen, principal: p}}]\n"
    ))
    .unwrap();
    assert_eq!(decoded.cycles[0].steps.len(), 4);
    assert!(started.elapsed() < BOUND);
}
