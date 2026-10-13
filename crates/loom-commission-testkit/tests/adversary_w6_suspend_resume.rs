//! Adversary, wave 2026-10-13-w1 U1 (`story:approval-suspend-resume-slice`), pass 1.
//!
//! Each case states a property of the suspend/resume path on the `chg-1842` fixture case, over the
//! AEP governor and the governor's production `FileSuspensionStore`. The fixtures are copied from
//! `approval_suspend_resume.rs`, since an integration test cannot share that file's helpers.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId, HoldSuspension,
    HoldSuspensionOutcome, ObservationId, PrincipalId, ReleaseSuspension, ReleaseSuspensionOutcome,
    RunId, SuspensionSnapshot, SuspensionStorageFailed, TakeSuspension, TakeSuspensionOutcome,
    Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStoreError};
use b10x_loom_commission::runtime::{
    LoopContext, LoopFailure, ResumeError, ResumeRefusal, resume_after_grant, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};
use loom_governor::{CanonGovernor, FileCaseStore, FileSuspensionStore};
use serde_json::Value;

type Gov = CanonGovernor<FileCaseStore>;

const PROTOCOL: &str = "software-change@1";
const CASE: &str = "CHG-1842";
const PRODUCER: &str = "service:ci";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const WITHHELD: &str = "merging CHG-1842 needs the release manager's approval";

// ---------------------------------------------------------------------------------------------
// Red candidates
// ---------------------------------------------------------------------------------------------

/// Two resumes of one suspended commission, racing as two processes over one directory, merge at
/// most once. Process A reads the Suspension, revalidates, is granted and resumes the Run; before
/// its `take` lands, process B does the whole resume, taking and releasing the Suspension. A's
/// `take` then answers `not-found`, and A must not perform the request.
#[test]
fn two_racing_resumes_merge_at_most_once() {
    let dir = store_dir("race");
    suspend_on_merge(&dir);

    // Every merge either process invokes.
    let merges: Mutex<Vec<String>> = Mutex::default();

    // Process B, run in full between A's checks and A's `take`.
    let process_b = || {
        let governor = governor(&dir);
        let case = CaseId(CASE.to_owned());
        let ids = Ids::starting_at(0x3000);
        let adapter = Adapter::new(&governor, &case, &ids);
        let model = Model::picking([]);
        let loom = loom(&model, &governor, "chg-1842-race-b");
        let resumed = resume_after_grant(
            &governor,
            &loom,
            &granting(),
            &adapter,
            &commission(&case),
            &mut run_store(0x800),
            &suspensions(&dir),
            &mut Context::starting_at(0x900),
        );
        assert!(resumed.is_ok(), "process B resumes: {resumed:?}");
        lock(&merges).extend(merges_of(&adapter));
    };

    // Process A.
    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-race-a");
    let port = Racing {
        inner: suspensions(&dir),
        before_take: Mutex::new(Some(Box::new(process_b))),
    };
    let resumed_a = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &port,
        &mut Context::starting_at(0x700),
    );
    lock(&merges).extend(merges_of(&adapter));

    let merges = lock(&merges).clone();
    assert_eq!(
        merges.len(),
        1,
        "two racing resumes merged {} times (requests {merges:?}); process A, whose `take` \
         found the Suspension gone, ended {resumed_a:?}",
        merges.len()
    );
}

/// A commission whose Run is suspended on the merge, then run again from the start (a host that
/// restarts the commission rather than resuming it) with the merge now granted. The new Run is
/// refused and merges nothing, since the commission holds a Suspension (decided for the unit,
/// 2026-10-13: F2); the later resume merges once. The commission merges at most once.
#[test]
fn a_new_run_of_a_suspended_commission_leaves_no_suspension_to_merge_again() {
    let dir = store_dir("rerun");
    suspend_on_merge(&dir);

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let suspensions = suspensions(&dir);

    let rerun = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([MERGE]);
    let loom_rerun = loom(&model, &governor, "chg-1842-rerun");
    let ended = run_with_suspension(
        &governor,
        &loom_rerun,
        &granting(),
        &rerun,
        &commission(&case),
        &mut run_store(0x600),
        &suspensions,
        &mut Context::starting_at(0x700),
    );
    assert!(
        matches!(
            &ended,
            Err(error) if error.run_id.is_none()
                && matches!(error.failure, LoopFailure::CommissionSuspended(_))
        ),
        "a new Run of a commission holding a Suspension is refused: {ended:?}"
    );
    assert!(
        merges_of(&rerun).is_empty(),
        "the refused new Run merges nothing: {ended:?}"
    );

    let resume = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom_resume = loom(&model, &governor, "chg-1842-rerun-resume");
    let resumed = resume_after_grant(
        &governor,
        &loom_resume,
        &granting(),
        &resume,
        &commission(&case),
        &mut run_store(0x800),
        &suspensions,
        &mut Context::starting_at(0x900),
    );
    assert_eq!(
        merges_of(&rerun).len() + merges_of(&resume).len(),
        1,
        "the commission merged twice: once in its new Run, and again when the Suspension of its \
         first Run, left stored, was resumed ({resumed:?})"
    );
}

/// A process that dies after the resume took the Suspension and before the merge was performed
/// (simulated by an effect port that panics on the merge) leaves the request in doubt: the merge
/// may or may not have happened, so the retry performs nothing and is refused, naming the request,
/// and the Suspension stays stored as the durable trace for the host to settle. The merge is never
/// performed twice and never lost without a trace (decided for the unit, 2026-10-13: F3).
#[test]
fn a_crash_between_taking_the_suspension_and_the_merge_leaves_the_request_in_doubt() {
    let dir = store_dir("crash");
    suspend_on_merge(&dir);
    let commission_id = commission(&CaseId(CASE.to_owned()))
        .data()
        .commission_id
        .clone();
    let request = suspensions(&dir)
        .get(&commission_id)
        .expect("the store reads")
        .expect("the Suspension is stored")
        .data
        .action_request_id;

    // The first resume dies at the merge, before the effect port performed it.
    let crashed = std::panic::catch_unwind(|| {
        let governor = governor(&dir);
        let case = CaseId(CASE.to_owned());
        let ids = Ids::starting_at(0x2000);
        let adapter = Adapter::new(&governor, &case, &ids).dying_on(MERGE);
        let model = Model::picking([]);
        let loom = loom(&model, &governor, "chg-1842-crash");
        let _ = resume_after_grant(
            &governor,
            &loom,
            &granting(),
            &adapter,
            &commission(&case),
            &mut run_store(0x600),
            &suspensions(&dir),
            &mut Context::starting_at(0x700),
        );
    });
    assert!(crashed.is_err(), "the first resume died at the merge");

    // The retry, in a new life.
    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x3000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-crash-retry");
    let retried = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x800),
        &suspensions(&dir),
        &mut Context::starting_at(0x900),
    );
    assert!(
        merges_of(&adapter).is_empty(),
        "the retry merged after a crash that may have merged already: {retried:?}"
    );
    assert_eq!(
        retried.err(),
        Some(ResumeError::Refused(ResumeRefusal::InDoubt {
            action_request_id: request.clone(),
        })),
        "the retry is refused as in doubt, naming the request"
    );
    let held = suspensions(&dir)
        .get(&commission_id)
        .expect("the store reads")
        .expect("the Suspension stays stored as the request's durable trace");
    assert_eq!(held.data.action_request_id, request);
}

// ---------------------------------------------------------------------------------------------
// Probes expected to hold
// ---------------------------------------------------------------------------------------------

/// A port that cannot take the Suspension: the resume performs nothing and the Suspension stays.
#[test]
fn a_resume_the_port_cannot_consume_performs_nothing() {
    let dir = store_dir("unremovable");
    suspend_on_merge(&dir);

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-unremovable");
    let port = Unremovable(suspensions(&dir));
    let resumed = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &port,
        &mut Context::starting_at(0x700),
    );
    assert!(
        matches!(resumed, Err(ResumeError::Loop(_))),
        "a Suspension that cannot be taken fails the resume: {resumed:?}"
    );
    assert!(adapter.invoked().is_empty(), "nothing was invoked");
    assert!(
        port.0
            .get(&commission(&case).data().commission_id)
            .expect("the store reads")
            .is_some(),
        "the Suspension stays stored"
    );
}

/// A Suspension file cut short (a torn write by something other than the store) is a store error
/// on resume, and nothing is performed.
#[test]
fn a_torn_suspension_file_is_refused_and_performs_nothing() {
    let dir = store_dir("torn");
    suspend_on_merge(&dir);
    let file = std::fs::read_dir(dir.join("suspensions"))
        .expect("the store directory")
        .map(|entry| entry.expect("entry").path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("suspension-"))
        })
        .expect("the Suspension file");
    let text = std::fs::read_to_string(&file).expect("the Suspension reads");
    std::fs::write(&file, &text[..text.len() / 2]).expect("cut it short");

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-torn");
    let resumed = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &suspensions(&dir),
        &mut Context::starting_at(0x700),
    );
    assert!(
        matches!(resumed, Err(ResumeError::Store(_))),
        "a torn Suspension is a store error: {resumed:?}"
    );
    assert!(adapter.invoked().is_empty(), "nothing was invoked");
    // The refusal names no other refusal.
    assert!(!matches!(
        resumed,
        Err(ResumeError::Refused(ResumeRefusal::NothingSuspended))
    ));
}

// ---------------------------------------------------------------------------------------------
// Ports standing in for broken or racing stores
// ---------------------------------------------------------------------------------------------

/// The file store, with a hook run once just before the first `take`: the other process's turn.
struct Racing<'a> {
    inner: FileSuspensionStore,
    before_take: Mutex<Option<Box<dyn FnOnce() + 'a>>>,
}

impl SuspensionPort for Racing<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.inner.hold(input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        let hook = lock(&self.before_take).take();
        if let Some(hook) = hook {
            hook();
        }
        self.inner.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.inner.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        self.inner.get(commission)
    }
}

/// The file store, whose `take` always fails.
struct Unremovable(FileSuspensionStore);

impl SuspensionPort for Unremovable {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.0.hold(input)
    }

    fn take(&self, _input: TakeSuspension) -> TakeSuspensionOutcome {
        TakeSuspensionOutcome::StorageFailed {
            error: SuspensionStorageFailed {
                reason: "the disk is read-only".to_owned(),
            },
        }
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.0.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        self.0.get(commission)
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures, after approval_suspend_resume.rs
// ---------------------------------------------------------------------------------------------

/// Opens the fixture case in `dir` and runs the commission until the merge is withheld, leaving
/// the Run's Suspension stored.
fn suspend_on_merge(dir: &Path) {
    let (governor, case, ids) = opened_case(dir);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([TESTS_RUN, MERGE]);
    let loom = loom(&model, &governor, "chg-1842-suspend");
    let suspensions = suspensions(dir);
    run_with_suspension(
        &governor,
        &loom,
        &withholding(),
        &adapter,
        &commission(&case),
        &mut run_store(0x500),
        &suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    assert_eq!(adapter.invoked(), [TESTS_RUN], "suspended before the merge");
    assert!(
        suspensions
            .get(&commission(&case).data().commission_id)
            .expect("the store reads")
            .is_some(),
        "the Suspension is stored"
    );
}

fn governor(dir: &Path) -> Gov {
    Gov::new(FileCaseStore::open(dir.join("cases")).expect("the case store opens"))
}

fn suspensions(dir: &Path) -> FileSuspensionStore {
    FileSuspensionStore::open(dir.join("suspensions")).expect("the suspension store opens")
}

fn opened_case(dir: &Path) -> (Gov, CaseId, Ids) {
    let fixture = Fixture::load();
    let ids = Ids::starting_at(0x1000);
    let governor = governor(dir);
    let case = CaseId(CASE.to_owned());
    governor
        .open_case(case.clone(), PROTOCOL, fixture.revisions())
        .expect("the fixture case opens on software.change/1");
    for record in fixture.evidence("initial") {
        submit(&governor, &case, &ids, &record);
    }
    (governor, case, ids)
}

fn withholding() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: WITHHELD.to_owned(),
        }),
    )
}

fn granting() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)))
}

fn loom<'m, 'g>(
    model: &'m Model,
    governor: &'g Gov,
    instance: &str,
) -> Loom<Selector<'m>, NoArguments, &'g Gov> {
    Loom::new(Selector(model), NoArguments, "fix CHG-1842 and merge it")
        .with_governor(governor)
        .with_instance(instance)
}

fn run_store(base: u64) -> RunStore {
    let next = AtomicU64::new(base);
    RunStore::new(move || RunId(uuid(next.fetch_add(1, Ordering::SeqCst))))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    let context = json::parse(r#"{"tenant":"tenant-a","delegation":"change-team"}"#)
        .expect("authority context is JSON");
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: case.clone(),
        principal: PrincipalId("principal:change-agent".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Ids(AtomicU64);

impl Ids {
    fn starting_at(base: u64) -> Self {
        Self(AtomicU64::new(base))
    }

    fn next(&self) -> Uuid {
        uuid(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

fn submit(governor: &Gov, case: &CaseId, ids: &Ids, record: &Value) {
    let facts = json::parse(&record.to_string()).expect("facts are JSON");
    let evidence = EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts,
        provenance: json::Value::Object(vec![(
            "source".to_owned(),
            json::Value::Text("adversary_w6_suspend_resume".to_owned()),
        )]),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

struct Model {
    picks: Mutex<VecDeque<&'static str>>,
}

impl Model {
    fn picking<const N: usize>(picks: [&'static str; N]) -> Self {
        Self {
            picks: Mutex::new(picks.into_iter().collect()),
        }
    }
}

struct Selector<'m>(&'m Model);

impl ActionSelector for Selector<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        lock(&self.0.picks)
            .pop_front()
            .map(|action| Choice {
                action: action.to_owned(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

struct NoArguments;

impl ArgumentGenerator for NoArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        Ok(json::Value::Object(Vec::new()))
    }
}

/// The action adapter of `approval_suspend_resume.rs`, recording each invocation as
/// `<action>@<request id>`; optionally panicking on one action before performing it.
struct Adapter<'g> {
    governor: &'g Gov,
    case: &'g CaseId,
    ids: &'g Ids,
    calls: Mutex<Vec<(String, String)>>,
    dies_on: Option<&'static str>,
}

impl<'g> Adapter<'g> {
    fn new(governor: &'g Gov, case: &'g CaseId, ids: &'g Ids) -> Self {
        Self {
            governor,
            case,
            ids,
            calls: Mutex::default(),
            dies_on: None,
        }
    }

    fn dying_on(mut self, action: &'static str) -> Self {
        self.dies_on = Some(action);
        self
    }

    fn invoked(&self) -> Vec<String> {
        lock(&self.calls)
            .iter()
            .map(|(action, _)| action.clone())
            .collect()
    }
}

fn merges_of(adapter: &Adapter<'_>) -> Vec<String> {
    lock(&adapter.calls)
        .iter()
        .filter(|(action, _)| action == MERGE)
        .map(|(action, request)| format!("{action}@{request}"))
        .collect()
}

impl EffectPort for Adapter<'_> {
    fn performs(&self, action: &str) -> bool {
        ["repository.inspect", "repository.edit", TESTS_RUN, MERGE].contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let request = request.data();
        if self.dies_on == Some(request.action.as_str()) {
            panic!("the process dies before performing {}", request.action);
        }
        lock(&self.calls).push((
            request.action.clone(),
            request.action_request_id.0.0.clone(),
        ));
        if request.action == TESTS_RUN {
            let revisions = self
                .governor
                .revisions(self.case)
                .map_err(|error| EffectError::new(format!("{error:?}")))?;
            let record = serde_json::json!({
                "format": "canon-evidence/1",
                "id": format!("tests-{}", revisions["implementation"]),
                "kind": "test_result",
                "result": "pass",
                "subject": "implementation",
                "subject_revision": revisions["implementation"],
                "observed_at": "2026-10-04T11:00:00Z",
            });
            submit(self.governor, self.case, self.ids, &record);
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: json::Value::Text(format!("performed {}", request.action)),
            attempt: None,
            audit: None,
        }))
    }
}

struct Context(u64);

impl Context {
    fn starting_at(base: u64) -> Self {
        Self(base)
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.0 += 1;
        ActionRequestId(uuid(self.0))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.0 += 1;
        ObservationId(uuid(0x10_0000 + self.0))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-04T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-w6-suspend-resume")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

struct Fixture(Value);

impl Fixture {
    fn load() -> Self {
        let manifest =
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        let path =
            PathBuf::from(manifest).join("../loom-governor/tests/fixtures/chg-1842.fixture.yaml");
        let fixture: Value = serde_yaml_ng::from_str(&read(&path))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        Self(fixture)
    }

    fn revisions(&self) -> BTreeMap<String, String> {
        self.0["case"]["artifacts"]
            .as_object()
            .expect("case artifacts")
            .iter()
            .map(|(artifact, entry)| {
                let revision = entry["revision"].as_str().expect("revision");
                (artifact.clone(), revision.to_owned())
            })
            .collect()
    }

    fn evidence(&self, id: &str) -> Vec<Value> {
        let state = self.0["states"]
            .as_array()
            .expect("states")
            .iter()
            .find(|state| state["id"] == id)
            .unwrap_or_else(|| panic!("the fixture has state {id}"));
        state["add_evidence"]
            .as_array()
            .expect("add_evidence")
            .iter()
            .map(|entry| {
                let mut record = entry["record"].clone();
                record["observed_at"] = entry["observed_at"].clone();
                record
            })
            .collect()
    }
}
