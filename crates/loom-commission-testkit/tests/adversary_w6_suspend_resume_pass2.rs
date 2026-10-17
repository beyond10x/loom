//! Adversary, wave 2026-10-13-w1 U1 (`story:approval-suspend-resume-slice`), pass 2.
//!
//! Attacks the fix of pass 1: the Suspension is held, taken once (`Pending` -> `Resuming`) and
//! released after the effect. Each case states a property of the suspend/resume path on the
//! `chg-1842` fixture case, over the AEP governor and the governor's production
//! `FileSuspensionStore`. The fixtures are copied from `adversary_w6_suspend_resume.rs`, since an
//! integration test cannot share that file's helpers.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId,
    Frontier, GovernorError, HoldSuspension, HoldSuspensionOutcome, Observation, ObservationId,
    PrincipalId, ReleaseSuspension, ReleaseSuspensionOutcome, RunId, SuspensionKey,
    SuspensionReason, SuspensionSnapshot, SuspensionState, SuspensionStorageFailed, TakeSuspension,
    TakeSuspensionOutcome, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
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

/// `TakeSuspension` and `ReleaseSuspension` name the commission only, not the request the resume
/// read and revalidated. Process B reads the Suspension S1 (merge request R1), checks the case
/// revision, revalidates and resumes the Run; before its `take` lands, process A resumes in full
/// (takes S1, merges R1, releases S1), and the commission, now holding no Suspension, is run again
/// and suspended on a new withheld request R2, holding S2 `Pending`. B's `take` then finds S2
/// `Pending` and takes it, so B merges R1 a second time, and releases S2, whose request R2 was
/// never performed: R2's Suspension is gone with no trace.
#[test]
fn a_stale_resume_takes_a_later_suspension_and_merges_its_old_request_again() {
    let dir = store_dir("stale-take");
    suspend_on_merge(&dir);
    let commission_id = commission(&CaseId(CASE.to_owned()))
        .data()
        .commission_id
        .clone();
    let first = suspensions(&dir)
        .get(&commission_id)
        .expect("the store reads")
        .expect("S1 is stored");

    let merges: Mutex<Vec<String>> = Mutex::default();
    let second_request = ActionRequestId(uuid(0xB00));

    // Process A's whole resume, then the commission's next Run suspending on R2, all between B's
    // checks and B's `take`.
    let process_a = || {
        let governor = governor(&dir);
        let case = CaseId(CASE.to_owned());
        let ids = Ids::starting_at(0x3000);
        let adapter = Adapter::new(&governor, &case, &ids);
        let model = Model::picking([]);
        let loom = loom(&model, &governor, "chg-1842-stale-a");
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
        assert!(resumed.is_ok(), "process A resumes: {resumed:?}");
        lock(&merges).extend(merges_of(&adapter));
        assert!(
            suspensions(&dir)
                .get(&commission_id)
                .expect("the store reads")
                .is_none(),
            "A released S1"
        );
        // The commission's next Run is suspended on R2: S2 is held `Pending`.
        let held = suspensions(&dir).hold(hold_input(
            &first,
            RunId(uuid(0xA00)),
            second_request.clone(),
        ));
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "S2 is held: {held:?}"
        );
    };

    // Process B.
    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-stale-b");
    let port = Racing {
        inner: suspensions(&dir),
        before_take: Mutex::new(Some(Box::new(process_a))),
    };
    let resumed_b = resume_after_grant(
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
    let left = suspensions(&dir)
        .get(&commission_id)
        .expect("the store reads");
    assert_eq!(
        (
            merges.len(),
            left.as_ref()
                .map(|s| (s.state, s.data.action_request_id.clone()))
        ),
        (1, Some((SuspensionState::Pending, second_request.clone()))),
        "R1 was merged {} times ({merges:?}) and S2 (R2, never performed) is left as {left:?}; \
         process B, which read S1, ended {resumed_b:?}",
        merges.len()
    );
}

// ---------------------------------------------------------------------------------------------
// Probes expected to hold
// ---------------------------------------------------------------------------------------------

/// A governor that fails to observe the merge after the effect port performed it: the resume
/// fails, the merge was performed once, the Suspension stays `Resuming`, and a later resume is
/// refused `InDoubt` naming the request, and merges nothing.
#[test]
fn a_governor_failing_after_the_merge_leaves_the_request_in_doubt_and_merged_once() {
    let dir = store_dir("observe-fails");
    suspend_on_merge(&dir);
    let commission_id = commission(&CaseId(CASE.to_owned()))
        .data()
        .commission_id
        .clone();
    let request = stored(&dir, &commission_id).data.action_request_id;

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-observe-fails");
    let failing = BlindGovernor(&governor);
    let resumed = resume_after_grant(
        &failing,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &suspensions(&dir),
        &mut Context::starting_at(0x700),
    );
    assert!(
        matches!(
            &resumed,
            Err(ResumeError::Loop(error)) if matches!(error.failure, LoopFailure::Governor(_))
        ),
        "the resume fails on the governor: {resumed:?}"
    );
    assert_eq!(merges_of(&adapter).len(), 1, "the merge was performed once");
    let held = stored(&dir, &commission_id);
    assert_eq!(held.state, SuspensionState::Resuming, "S1 stays Resuming");

    let retry = Adapter::new(&governor, &case, &ids);
    let retried = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &retry,
        &commission(&case),
        &mut run_store(0x800),
        &suspensions(&dir),
        &mut Context::starting_at(0x900),
    );
    assert_eq!(
        retried.err(),
        Some(ResumeError::Refused(ResumeRefusal::InDoubt {
            action_request_id: request
        }))
    );
    assert!(merges_of(&retry).is_empty(), "the retry merges nothing");
}

/// A port that refuses the release after the merge: the resume ends `SuspensionNotReleased`, the
/// merge was performed once, the Suspension stays `Resuming`, and a later resume is refused
/// `InDoubt` and merges nothing.
#[test]
fn a_refused_release_leaves_the_request_in_doubt_and_merged_once() {
    let dir = store_dir("release-refused");
    suspend_on_merge(&dir);
    let commission_id = commission(&CaseId(CASE.to_owned()))
        .data()
        .commission_id
        .clone();
    let request = stored(&dir, &commission_id).data.action_request_id;

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-release-refused");
    let port = Unreleasable(suspensions(&dir));
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
        matches!(
            &resumed,
            Err(ResumeError::Loop(error))
                if matches!(error.failure, LoopFailure::SuspensionNotReleased(_))
        ),
        "the resume ends SuspensionNotReleased: {resumed:?}"
    );
    assert_eq!(merges_of(&adapter).len(), 1, "the merge was performed once");
    assert_eq!(
        stored(&dir, &commission_id).state,
        SuspensionState::Resuming
    );

    let retry = Adapter::new(&governor, &case, &ids);
    let retried = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &retry,
        &commission(&case),
        &mut run_store(0x800),
        &suspensions(&dir),
        &mut Context::starting_at(0x900),
    );
    assert_eq!(
        retried.err(),
        Some(ResumeError::Refused(ResumeRefusal::InDoubt {
            action_request_id: request
        }))
    );
    assert!(merges_of(&retry).is_empty(), "the retry merges nothing");
}

/// A resume whose step budget is used up by the resumed merge: the merge is performed once and
/// the Suspension released before the Run is suspended on its budget; a second resume finds
/// nothing suspended and merges nothing; and the commission, holding no Suspension, may start a
/// new Run.
#[test]
fn a_resume_out_of_budget_after_the_merge_releases_the_suspension_and_merges_once() {
    let dir = store_dir("budget");
    suspend_on_merge(&dir);
    let commission_id = commission(&CaseId(CASE.to_owned()))
        .data()
        .commission_id
        .clone();

    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-budget");
    let resumed = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &suspensions(&dir),
        &mut Budgeted(Context::starting_at(0x700), 1),
    );
    assert!(resumed.is_ok(), "the budgeted resume ends: {resumed:?}");
    assert_eq!(merges_of(&adapter).len(), 1, "merged once");
    assert!(
        suspensions(&dir)
            .get(&commission_id)
            .expect("the store reads")
            .is_none(),
        "the Suspension was released"
    );

    let again = Adapter::new(&governor, &case, &ids);
    let retried = resume_after_grant(
        &governor,
        &loom,
        &granting(),
        &again,
        &commission(&case),
        &mut run_store(0x800),
        &suspensions(&dir),
        &mut Context::starting_at(0x900),
    );
    assert_eq!(
        retried.err(),
        Some(ResumeError::Refused(ResumeRefusal::NothingSuspended))
    );
    assert!(merges_of(&again).is_empty());

    let rerun = Adapter::new(&governor, &case, &ids);
    let ended = run_with_suspension(
        &governor,
        &loom,
        &granting(),
        &rerun,
        &commission(&case),
        &mut run_store(0xA00),
        &suspensions(&dir),
        &mut Context::starting_at(0xB00),
    );
    assert!(
        !matches!(
            &ended,
            Err(error) if matches!(
                error.failure,
                LoopFailure::CommissionSuspended(_) | LoopFailure::SuspensionStore(_)
            )
        ),
        "a commission whose Suspension was released starts again: {ended:?}"
    );
}

/// A Suspension of another commission in the same store does not keep this commission from
/// starting a Run.
#[test]
fn another_commissions_suspension_does_not_refuse_a_new_run() {
    let dir = store_dir("other-commission");
    suspend_on_merge(&dir);
    let governor = governor(&dir);
    let case = CaseId(CASE.to_owned());
    let ids = Ids::starting_at(0x2000);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-other");
    let mut other = commission(&case).into_data();
    other.commission_id = CommissionId(uuid(0x77));
    let ended = run_with_suspension(
        &governor,
        &loom,
        &granting(),
        &adapter,
        &Commission::new(other),
        &mut run_store(0x600),
        &suspensions(&dir),
        &mut Context::starting_at(0x700),
    );
    assert!(
        !matches!(
            &ended,
            Err(error) if matches!(
                error.failure,
                LoopFailure::CommissionSuspended(_) | LoopFailure::SuspensionStore(_)
            )
        ),
        "another commission starts: {ended:?}"
    );
}

/// Eight stores over one directory, each with its own lock handle, take one `Pending` Suspension
/// at once, two hundred times: exactly one takes it each time.
#[test]
fn concurrent_takes_over_one_directory_take_once() {
    let dir = store_dir("concurrent-take");
    let root = dir.join("suspensions");
    let snapshot = template();
    for round in 0..200u64 {
        let key = SuspensionKey {
            commission_id: CommissionId(uuid(0x1_0000 + round)),
        };
        let mut data = snapshot.clone();
        data.data.commission_id = key.commission_id.clone();
        data.data.suspension = key.clone();
        let held = FileSuspensionStore::open(&root)
            .expect("opens")
            .hold(hold_input(
                &data,
                data.data.run_id.clone(),
                data.data.action_request_id.clone(),
            ));
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "{held:?}"
        );
        let barrier = Arc::new(Barrier::new(8));
        let taken: Vec<bool> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                let root = root.clone();
                let key = key.clone();
                let request = data.data.action_request_id.clone();
                std::thread::spawn(move || {
                    let store = FileSuspensionStore::open(&root).expect("opens");
                    barrier.wait();
                    matches!(
                        store.take(TakeSuspension {
                            suspension: key,
                            action_request_id: request,
                        }),
                        TakeSuspensionOutcome::Taken { .. }
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("joins"))
            .collect();
        assert_eq!(
            taken.iter().filter(|t| **t).count(),
            1,
            "round {round}: {taken:?}"
        );
    }
}

/// Over the file store: holding while `Resuming` is refused and keeps it; releasing a `Pending`
/// one is `not-taken` and keeps it; taking a `Resuming` one is `wrong-state`.
#[test]
fn the_file_store_answers_hold_take_and_release_from_each_state() {
    let dir = store_dir("states");
    let store = FileSuspensionStore::open(dir.join("suspensions")).expect("opens");
    let held = template();
    let key = held.data.suspension.clone();
    let id = held.data.commission_id.clone();
    let request = held.data.action_request_id.clone();
    let input = || {
        hold_input(
            &held,
            held.data.run_id.clone(),
            held.data.action_request_id.clone(),
        )
    };

    assert!(matches!(
        store.hold(input()),
        HoldSuspensionOutcome::Held { .. }
    ));
    assert!(matches!(
        store.release(ReleaseSuspension {
            suspension: key.clone(),
            action_request_id: request.clone()
        }),
        ReleaseSuspensionOutcome::NotTaken { .. }
    ));
    assert_eq!(
        store.get(&id).unwrap().unwrap().state,
        SuspensionState::Pending
    );
    assert!(matches!(
        store.take(TakeSuspension {
            suspension: key.clone(),
            action_request_id: request.clone()
        }),
        TakeSuspensionOutcome::Taken { .. }
    ));
    assert!(matches!(
        store.hold(input()),
        HoldSuspensionOutcome::AlreadyHeld { .. }
    ));
    assert!(matches!(
        store.take(TakeSuspension {
            suspension: key.clone(),
            action_request_id: request.clone()
        }),
        TakeSuspensionOutcome::WrongState { .. }
    ));
    assert_eq!(
        store.get(&id).unwrap().unwrap().state,
        SuspensionState::Resuming
    );
    assert!(matches!(
        store.release(ReleaseSuspension {
            suspension: key.clone(),
            action_request_id: request.clone()
        }),
        ReleaseSuspensionOutcome::Released { .. }
    ));
    assert!(matches!(
        store.release(ReleaseSuspension {
            suspension: key,
            action_request_id: request
        }),
        ReleaseSuspensionOutcome::NotFound { .. }
    ));
    assert!(store.get(&id).unwrap().is_none());
}

// ---------------------------------------------------------------------------------------------
// Helpers of this pass
// ---------------------------------------------------------------------------------------------

fn stored(dir: &Path, commission_id: &CommissionId) -> SuspensionSnapshot {
    suspensions(dir)
        .get(commission_id)
        .expect("the store reads")
        .expect("the Suspension is stored")
}

fn hold_input(
    from: &SuspensionSnapshot,
    run_id: RunId,
    request: ActionRequestId,
) -> HoldSuspension {
    let data = &from.data;
    HoldSuspension {
        suspension: data.suspension.clone(),
        commission_id: data.commission_id.clone(),
        run_id,
        run_case_revision: data.run_case_revision,
        action_request_id: request,
        case_id: data.case_id.clone(),
        expected_case_revision: data.expected_case_revision,
        action: data.action.clone(),
        arguments: data.arguments.clone(),
        reason: data.reason.clone(),
    }
}

/// A Suspension snapshot built by hand, for the store-only cases.
fn template() -> SuspensionSnapshot {
    let commission_id = CommissionId(uuid(0x55));
    SuspensionSnapshot {
        state: SuspensionState::Pending,
        data: b10x_loom_commission::model::responsibility::SuspensionData {
            suspension: SuspensionKey {
                commission_id: commission_id.clone(),
            },
            commission_id,
            run_id: RunId(uuid(0x56)),
            run_case_revision: 1,
            action_request_id: ActionRequestId(uuid(0x57)),
            case_id: CaseId(CASE.to_owned()),
            expected_case_revision: 1,
            action: MERGE.to_owned(),
            arguments: b10x_loom_commission::model::responsibility::ProposedActionArguments(
                json::Value::Object(Vec::new()),
            ),
            reason: SuspensionReason::Authority(json::Value::Text(WITHHELD.to_owned())),
        },
    }
}

/// The governor, failing every observation.
struct BlindGovernor<'g>(&'g Gov);

impl Governor for BlindGovernor<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.0.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.0.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.0.completion(case)
    }
}

impl ObservationPort for BlindGovernor<'_> {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Err(GovernorError::GovernorUnavailable)
    }
}

/// A context with a step budget.
struct Budgeted(Context, usize);

impl LoopContext for Budgeted {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.0.action_request_id()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.0.observation_id()
    }

    fn now(&mut self) -> Timestamp {
        self.0.now()
    }

    fn step_budget(&self) -> Option<usize> {
        Some(self.1)
    }
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

/// The file store, whose `release` always fails.
struct Unreleasable(FileSuspensionStore);

impl SuspensionPort for Unreleasable {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.0.hold(input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        self.0.take(input)
    }

    fn release(&self, _input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        ReleaseSuspensionOutcome::StorageFailed {
            error: SuspensionStorageFailed {
                reason: "the disk is read-only".to_owned(),
            },
        }
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
        .open_case(
            case.clone(),
            PROTOCOL,
            fixture.revisions(),
            inputs_of(&fixture.0),
        )
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
    let evidence_id = EvidenceId(ids.next());
    let observation = ObservationId(ids.next());
    observe_at(governor, &observation, record);
    let evidence = EvidenceData {
        evidence_id,
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![observation],
        facts,
        provenance: json::Value::Object(vec![(
            "source".to_owned(),
            json::Value::Text("adversary_w6_suspend_resume_pass2".to_owned()),
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

    // Copied with the fixtures; no case of this pass dies before an action.
    #[allow(dead_code)]
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
        [
            "repository.inspect",
            "implementation.edit",
            TESTS_RUN,
            MERGE,
        ]
        .contains(&action)
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
            usage: None,
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
        .join("adversary-w6-suspend-resume-pass2")
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

/// The case inputs a fixture's case gives; a value YAML reads as a boolean is its text.
fn inputs_of(fixture: &serde_json::Value) -> loom_governor::CaseInputs {
    let pairs: Vec<(String, String)> = fixture["case"]["inputs"]
        .as_object()
        .expect("case inputs")
        .iter()
        .map(|(input, value)| {
            let value = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (input.clone(), value)
        })
        .collect();
    loom_governor::CaseInputs::try_from_pairs(pairs).expect("distinct inputs")
}

/// Delivers `observation` at `record`'s `observed_at`, as a host does before it submits the record:
/// the governor takes a record's `observed_at` only as the instant of the observation it cites.
fn observe_at(
    governor: &impl b10x_loom_commission::ports::evidence::ObservationPort,
    observation: &ObservationId,
    record: &Value,
) {
    // The governor refuses a record citing an observation it never received, with or
    // without an `observed_at`; a record without one is observed at a fixed instant.
    let at = record["observed_at"]
        .as_str()
        .unwrap_or("2026-01-01T00:00:00Z");
    governor
        .observe(
            b10x_loom_commission::model::responsibility::Observation::new(
                b10x_loom_commission::model::responsibility::ObservationData {
                    observation_id: observation.clone(),
                    source: "host/fixture".to_owned(),
                    subject: record["subject"]
                        .as_str()
                        .unwrap_or("implementation")
                        .to_owned(),
                    observed_at: b10x_loom_commission::model::primitives::Timestamp(at.to_owned()),
                    payload: json::Value::Null,
                },
            ),
        )
        .expect("the governor keeps the observation");
}
