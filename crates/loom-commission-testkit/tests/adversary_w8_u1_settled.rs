//! Adversary cases for `story:handoff-holds-no-suspension` (wave 2026-10-13-w3, U1): what the
//! runtime does after a suspension port answers `HoldSuspension` `settled` and the `ResumeRun`
//! that puts the suspended Run back `Running` fails.
//!
//! The runtime documents it (`runtime.rs`, module docs and `LoopFailure::NotResumed`): a
//! `ResumeRun` that fails after a settled hold ends the loop with `LoopFailure::RunStorage`
//! (`storage-failed`) or `LoopFailure::NotResumed` (any other answer), the Run suspended, its
//! reason in `LoopError::suspension_reason`, and nothing held. The unit's own case
//! (`approval_suspend_resume.rs`) drives only a `ResumeRun` that succeeds, so dropping the failure
//! branch of the settled arm in `suspend_for_approval` leaves every case of the unit green; these
//! cases are the ones that see it.
//!
//! The fixtures are copied from `adversary_w6_suspend_resume_pass2.rs`, since an integration test
//! cannot share that file's helpers.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::behaviour::RunStorage as _;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::obligation::UnmetObligation;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::obligations::{
    ResumeRunBehavior, StartRunBehavior, SuspendRunBehavior,
};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId, HoldSuspension,
    HoldSuspensionOutcome, ObservationId, PrincipalId, ReleaseSuspension, ReleaseSuspensionOutcome,
    ResumeRun, ResumeRunOutcome, RunId, RunState, RunStateConflict, RunStorageFailed, StartRun,
    StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionReason, SuspensionSettled,
    SuspensionSnapshot, TakeSuspension, TakeSuspensionOutcome, Unit, commission_state,
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
const UNWRITABLE: &str = "the run store is full";

/// A settled hold whose `ResumeRun` answers `storage-failed` fails the loop with
/// `LoopFailure::RunStorage` carrying the store's reason; one answering `wrong-state` fails it with
/// `LoopFailure::NotResumed` carrying that answer. Either way: the Run stays `Suspended`, the
/// error names it and the `Authority` reason, nothing is held, nothing is merged, and a resume of
/// the commission finds nothing to resume.
#[test]
fn a_resume_run_that_fails_after_a_settled_hold_fails_the_loop_and_holds_nothing() {
    let refusals = [
        (
            "storage-failed",
            ResumeRunOutcome::StorageFailed {
                error: RunStorageFailed {
                    reason: UNWRITABLE.to_owned(),
                },
            },
            LoopFailure::RunStorage(RunStorageFailed {
                reason: UNWRITABLE.to_owned(),
            }),
        ),
        (
            "wrong-state",
            ResumeRunOutcome::WrongState {
                error: RunStateConflict {
                    state: RunState::Running,
                },
            },
            LoopFailure::NotResumed(Box::new(ResumeRunOutcome::WrongState {
                error: RunStateConflict {
                    state: RunState::Running,
                },
            })),
        ),
    ];
    for (label, answer, expected) in refusals {
        let dir = store_dir(label);
        let (governor, case, ids) = opened_case(&dir);
        let adapter = Adapter::new(&governor, &case, &ids);
        let model = Model::picking([TESTS_RUN, MERGE]);
        let executor = loom(&model, &governor, "chg-1842-settled-not-resumed");
        let store = suspensions(&dir);
        let settling = Settles {
            store: &store,
            asked: Mutex::default(),
        };
        let mut runs = RefusesResume {
            inner: run_store(0x500),
            answer,
            asked: Vec::new(),
        };
        let failed = run_with_suspension(
            &governor,
            &executor,
            &withholding(),
            &adapter,
            &commission(&case),
            &mut runs,
            &settling,
            &mut Context::starting_at(0x300),
        )
        .expect_err("a ResumeRun that fails after a settled hold fails the loop");

        assert_eq!(
            lock(&settling.asked).len(),
            1,
            "{label}: one hold was asked"
        );
        assert_eq!(failed.failure, expected, "{label}");
        let run_id = failed
            .run_id
            .clone()
            .unwrap_or_else(|| panic!("{label}: the error names the Run"));
        assert_eq!(
            runs.asked,
            std::slice::from_ref(&run_id),
            "{label}: ResumeRun was asked once, for the Run the loop suspended"
        );
        assert!(
            matches!(
                failed.suspension_reason.as_deref(),
                Some(SuspensionReason::Authority(_))
            ),
            "{label}: the error carries the Authority reason the Run was suspended for: {:?}",
            failed.suspension_reason
        );
        assert_eq!(
            runs.inner.get(&run_id).map(|held| held.state),
            Some(RunState::Suspended),
            "{label}: the Run stays Suspended"
        );
        assert_eq!(
            adapter.invoked(),
            [TESTS_RUN],
            "{label}: nothing was merged"
        );
        assert!(
            store
                .get(&commission(&case).data().commission_id)
                .expect("the store reads")
                .is_none(),
            "{label}: a settled hold holds nothing"
        );

        let resume_adapter = Adapter::new(&governor, &case, &ids);
        let resume_model = Model::picking([MERGE]);
        let resume_loom = loom(
            &resume_model,
            &governor,
            "chg-1842-settled-not-resumed-again",
        );
        let resumed = resume_after_grant(
            &governor,
            &resume_loom,
            &granting(),
            &resume_adapter,
            &commission(&case),
            &mut run_store(0x600),
            &store,
            &mut Context::starting_at(0x700),
        );
        assert_eq!(
            resumed.err(),
            Some(ResumeError::Refused(ResumeRefusal::NothingSuspended)),
            "{label}: nothing is left to resume"
        );
        assert!(
            resume_adapter.invoked().is_empty(),
            "{label}: nothing was invoked"
        );
    }
}

/// The run store of `RunStore`, whose `ResumeRun` answers `answer` and records whom it was asked
/// for; `StartRun` and `SuspendRun` are the store's.
struct RefusesResume {
    inner: RunStore,
    answer: ResumeRunOutcome,
    asked: Vec<RunId>,
}

impl StartRunBehavior for RefusesResume {
    fn start_run(&mut self, input: StartRun) -> Result<StartRunOutcome, UnmetObligation> {
        self.inner.start_run(input)
    }
}

impl SuspendRunBehavior for RefusesResume {
    fn suspend_run(&mut self, input: SuspendRun) -> Result<SuspendRunOutcome, UnmetObligation> {
        self.inner.suspend_run(input)
    }
}

impl ResumeRunBehavior for RefusesResume {
    fn resume_run(&mut self, input: ResumeRun) -> Result<ResumeRunOutcome, UnmetObligation> {
        self.asked.push(input.run_id);
        Ok(self.answer.clone())
    }
}

/// The file store behind a host that answers every `HoldSuspension` `settled`, and passes every
/// other call to the store.
struct Settles<'s> {
    store: &'s FileSuspensionStore,
    asked: Mutex<Vec<HoldSuspension>>,
}

impl SuspensionPort for Settles<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        lock(&self.asked).push(input.clone());
        HoldSuspensionOutcome::Settled {
            error: SuspensionSettled {
                suspension: input.suspension,
                action_request_id: input.action_request_id,
                reason: "the host already did what an approval would lead to".to_owned(),
            },
        }
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        self.store.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.store.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        self.store.get(commission)
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures, as `adversary_w6_suspend_resume_pass2.rs` has them.

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
            json::Value::Text("adversary_w8_u1_settled".to_owned()),
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

/// The action adapter of `approval_suspend_resume.rs`, recording each invocation.
struct Adapter<'g> {
    governor: &'g Gov,
    case: &'g CaseId,
    ids: &'g Ids,
    calls: Mutex<Vec<String>>,
}

impl<'g> Adapter<'g> {
    fn new(governor: &'g Gov, case: &'g CaseId, ids: &'g Ids) -> Self {
        Self {
            governor,
            case,
            ids,
            calls: Mutex::default(),
        }
    }

    fn invoked(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }
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
        lock(&self.calls).push(request.action.clone());
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
        .join("adversary-w8-u1-settled")
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
