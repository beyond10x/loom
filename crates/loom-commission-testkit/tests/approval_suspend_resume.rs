//! I-003, approval suspend/resume across a process restart (`story:approval-suspend-resume-slice`):
//! the ELS `software.change/1` fixture case `chg-1842`, driven through Commission's runtime with
//! Loom as its executor, on the AEP governor (`CanonGovernor` over a `FileCaseStore`), with the
//! testkit's file-backed suspension store as the suspension port.
//!
//! The restart is real: [`approval_suspends_and_resumes_across_a_process_restart`] runs its own
//! test binary twice as a child process over one directory, which holds the case store and the
//! suspension store. The first child runs the commission until `repository.merge` needs authority
//! the provider withholds: the runtime suspends the Run (`SuspendRun`, reason `Authority`), writes
//! the commission's Suspension through the port, and the child exits without a merge. The second
//! child starts with a new governor, a new run store and a new suspension store over the same
//! directory, the provider now granting the merge, and resumes the same Run from the stored
//! Suspension alone: the merge is invoked exactly once, for the case and at the case revision
//! recorded at suspension.
//!
//! The other cases: without a suspension port the run ends as in 0.16.0 and nothing is suspended;
//! a resume after the case revision moved is refused and invokes nothing; a resume with nothing
//! stored is refused; a suspension the store cannot write fails the loop, naming the Run.
//!
//! The model is scripted (a selector and an argument generator), since tests make no model call.
//! The adapter performs every action the protocol declares, `repository.merge` included, so what
//! keeps the merge from it is the governor, Commission and the authority provider.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::behaviour::RunStorage as _;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId, ObservationId, PrincipalId,
    RunId, RunOutcome, RunOutcomeNeedsAuthority, RunState, SuspensionReason, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{
    LoopContext, LoopFailure, ResumeError, ResumeRefusal, resume_after_grant, run_until_blocked,
    run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};
use loom_governor::{CanonGovernor, FileCaseStore};
use serde_json::Value;

type Gov = CanonGovernor<FileCaseStore>;

const PROTOCOL: &str = "software-change@1";
const CASE: &str = "CHG-1842";
const PRODUCER: &str = "service:ci";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const WITHHELD: &str = "merging CHG-1842 needs the release manager's approval";

const ROLE: &str = "LOOM_APPROVAL_SUSPEND_RESUME_ROLE";
const DIR: &str = "LOOM_APPROVAL_SUSPEND_RESUME_DIR";
const TEST: &str = "approval_suspends_and_resumes_across_a_process_restart";

#[test]
fn approval_suspends_and_resumes_across_a_process_restart() {
    if let Ok(role) = std::env::var(ROLE) {
        let dir = PathBuf::from(std::env::var(DIR).expect("the parent names the directory"));
        match role.as_str() {
            "suspend" => suspend_child(&dir),
            "resume" => resume_child(&dir),
            other => panic!("unknown child role `{other}`"),
        }
        return;
    }

    let dir = store_dir("restart");
    run_child("suspend", &dir);
    run_child("resume", &dir);

    let suspended = Record::read(&dir.join("suspend.txt"));
    let resumed = Record::read(&dir.join("resume.txt"));
    assert_ne!(
        suspended.get("pid"),
        resumed.get("pid"),
        "the second life is another process"
    );

    // Execution suspended where repository.merge required authority, with no merge before it.
    assert_eq!(suspended.get("outcome"), "NeedsAuthority");
    assert_eq!(suspended.get("run_state"), "Suspended");
    assert_eq!(suspended.get("suspension_action"), MERGE);
    assert_eq!(suspended.get("suspension_reason"), "Authority");
    assert_eq!(
        suspended.get("invoked"),
        TESTS_RUN,
        "the first life ran the tests and never the merge"
    );

    // The case outlived the first process, at the same revision.
    assert_eq!(
        resumed.get("revision_before_resume"),
        suspended.get("revision_at_suspension"),
        "the case revision after the restart is the one before it"
    );

    // The second life resumed the same Run from durable state and merged exactly once.
    assert_eq!(
        resumed.get("run_id"),
        suspended.get("run_id"),
        "resuming continues the same run"
    );
    assert_eq!(
        resumed.get("invoked"),
        MERGE,
        "the merge was invoked exactly once, and only after the grant"
    );
    assert_eq!(resumed.get("merge_case"), CASE);
    assert_eq!(
        resumed.get("merge_revision"),
        suspended.get("revision_at_suspension"),
        "the merge carries the case revision recorded at suspension"
    );
    assert_eq!(
        resumed.get("merge_request"),
        suspended.get("suspension_request"),
        "the merge is the request the run was suspended on"
    );
    assert_eq!(
        resumed.get("suspension_after"),
        "none",
        "the suspension was consumed by the resume"
    );
}

/// The first life: the run reaches the merge, which the provider withholds, and is suspended.
fn suspend_child(dir: &Path) {
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
    let suspensions = suspensions(dir);
    let commission = commission(&case);
    let authority = withholding();
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([TESTS_RUN, MERGE]);
    let loom = loom(&model, &governor, "chg-1842-suspend");
    let mut runs = run_store(0x500);

    let end = run_with_suspension(
        &governor,
        &loom,
        &authority,
        &adapter,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));

    let stored = suspensions
        .get(&commission.data().commission_id)
        .expect("the suspension store reads")
        .expect("the run was suspended into the store")
        .into_data();
    let revision = governor.current_revision(&case).expect("current revision");
    let mut record = Record::default();
    record.set("pid", std::process::id());
    record.set("outcome", outcome_name(&end.outcome));
    record.set("run_id", &end.run_id.0.0);
    record.set("run_state", run_state(&runs, &end.run_id));
    record.set("revision_at_suspension", revision);
    record.set("suspension_action", &stored.action);
    record.set("suspension_request", &stored.action_request_id.0.0);
    record.set(
        "suspension_reason",
        match &stored.reason {
            SuspensionReason::Authority(_) => "Authority",
            _ => "other",
        },
    );
    assert_eq!(stored.run_id, end.run_id);
    assert_eq!(stored.expected_case_revision, revision);
    assert_eq!(stored.case_id, case);
    record.set("invoked", adapter.invoked().join(","));
    record.write(&dir.join("suspend.txt"));
}

/// The second life: a new governor, run store and suspension store over the same directory, and a
/// provider that now grants the merge.
fn resume_child(dir: &Path) {
    let ids = Ids::starting_at(0x2000);
    let governor = governor(dir);
    let case = CaseId(CASE.to_owned());
    let revision = governor
        .current_revision(&case)
        .expect("the case outlived its process");
    let suspensions = suspensions(dir);
    let commission = commission(&case);
    let authority =
        StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)));
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([]);
    let loom = loom(&model, &governor, "chg-1842-resume");
    let mut runs = run_store(0x600);

    let end = resume_after_grant(
        &governor,
        &loom,
        &authority,
        &adapter,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::starting_at(0x700),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    let calls = adapter.calls();
    let merge = calls
        .iter()
        .find(|call| call.action == MERGE)
        .expect("the merge was invoked");
    let mut record = Record::default();
    record.set("pid", std::process::id());
    record.set("run_id", &end.run_id.0.0);
    record.set("revision_before_resume", revision);
    record.set("invoked", adapter.invoked().join(","));
    record.set("merge_case", &merge.case.0);
    record.set("merge_revision", merge.expected_case_revision);
    record.set("merge_request", &merge.request);
    record.set(
        "suspension_after",
        match suspensions
            .get(&commission.data().commission_id)
            .expect("the suspension store reads")
        {
            Some(_) => "held",
            None => "none",
        },
    );
    record.write(&dir.join("resume.txt"));
}

/// Without a suspension port the run ends as in 0.16.0: `NeedsAuthority`, its Run left as the
/// loop left it, nothing written anywhere. With one, the caller sees the same outcome, the same
/// requests and the same effects; only the Run is suspended and its Suspension stored.
#[test]
fn without_a_suspension_port_the_run_ends_as_in_0_16_0() {
    let without_dir = store_dir("without-port");
    let (without, without_runs, without_calls) = {
        let (governor, case, ids) = opened_case(&without_dir);
        let adapter = Adapter::new(&governor, &case, &ids);
        let model = Model::picking([TESTS_RUN, MERGE]);
        let loom = loom(&model, &governor, "chg-1842-without-port");
        let mut runs = run_store(0x500);
        let end = run_until_blocked(
            &governor,
            &loom,
            &withholding(),
            &adapter,
            &commission(&case),
            &mut runs,
            &mut Context::starting_at(0x300),
        )
        .unwrap_or_else(|error| panic!("the loop failed: {error}"));
        let state = run_state(&runs, &end.run_id);
        (end, state, adapter.invoked())
    };
    assert_eq!(
        without.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: WITHHELD.to_owned(),
        }),
        "the 0.16.0 outcome"
    );
    assert_eq!(
        without_runs, "Running",
        "without a port the loop does not suspend the Run"
    );
    assert_eq!(without_calls, [TESTS_RUN]);

    let with_dir = store_dir("with-port");
    let (governor, case, ids) = opened_case(&with_dir);
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([TESTS_RUN, MERGE]);
    let loom = loom(&model, &governor, "chg-1842-without-port");
    let mut runs = run_store(0x500);
    let suspensions = suspensions(&with_dir);
    let with = run_with_suspension(
        &governor,
        &loom,
        &withholding(),
        &adapter,
        &commission(&case),
        &mut runs,
        &suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    assert_eq!(
        with, without,
        "the caller sees the same outcome, requests, effects and steps"
    );
    assert_eq!(adapter.invoked(), without_calls);
    assert_eq!(run_state(&runs, &with.run_id), "Suspended");
    assert!(
        suspensions
            .get(&commission(&case).data().commission_id)
            .expect("the store reads")
            .is_some(),
        "with a port the Suspension is stored"
    );
}

/// A resume finding the case at another revision than the one the request was suspended at is
/// refused: no effect is invoked, the provider is not asked, and the Suspension stays stored.
#[test]
fn a_resume_after_the_case_revision_moved_is_refused_and_invokes_nothing() {
    let dir = store_dir("moved");
    let (governor, case, ids) = opened_case(&dir);
    let suspensions = suspensions(&dir);
    let commission = commission(&case);
    {
        let adapter = Adapter::new(&governor, &case, &ids);
        let model = Model::picking([TESTS_RUN, MERGE]);
        let loom = loom(&model, &governor, "chg-1842-moved");
        run_with_suspension(
            &governor,
            &loom,
            &withholding(),
            &adapter,
            &commission,
            &mut run_store(0x500),
            &suspensions,
            &mut Context::starting_at(0x300),
        )
        .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    }
    let at_suspension = governor.current_revision(&case).expect("current revision");
    let moved = governor
        .update_revision(&case, "implementation", "R3")
        .expect("somebody pushes R3");
    assert_eq!(moved, at_suspension + 1);

    let granting =
        StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)));
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([MERGE]);
    let loom = loom(&model, &governor, "chg-1842-moved-resume");
    let resumed = resume_after_grant(
        &governor,
        &loom,
        &granting,
        &adapter,
        &commission,
        &mut run_store(0x600),
        &suspensions,
        &mut Context::starting_at(0x700),
    );
    assert_eq!(
        resumed.err(),
        Some(ResumeError::Refused(ResumeRefusal::CaseMoved {
            expected_case_revision: at_suspension,
            current_case_revision: moved,
        })),
        "a resume after the case moved is refused"
    );
    assert!(adapter.invoked().is_empty(), "nothing was invoked");
    assert!(granting.asked().is_empty(), "the provider was not asked");
    assert!(model.offered().is_empty(), "the executor was not run");
    assert!(
        suspensions
            .get(&commission.data().commission_id)
            .expect("the store reads")
            .is_some(),
        "the refused resume leaves the Suspension stored"
    );
}

/// A resume with no stored Suspension is refused before anything else is asked or done.
#[test]
fn a_resume_with_no_stored_suspension_is_refused() {
    let dir = store_dir("nothing-stored");
    let (governor, case, ids) = opened_case(&dir);
    let granting =
        StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)));
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([MERGE]);
    let loom = loom(&model, &governor, "chg-1842-nothing-stored");
    let resumed = resume_after_grant(
        &governor,
        &loom,
        &granting,
        &adapter,
        &commission(&case),
        &mut run_store(0x600),
        &suspensions(&dir),
        &mut Context::starting_at(0x700),
    );
    assert_eq!(
        resumed.err(),
        Some(ResumeError::Refused(ResumeRefusal::NothingSuspended)),
        "nothing stored is nothing to resume"
    );
    assert!(adapter.invoked().is_empty(), "nothing was invoked");
    assert!(granting.asked().is_empty(), "the provider was not asked");
    assert!(model.offered().is_empty(), "the executor was not run");
}

/// A Suspension the port cannot write fails the loop as the run store's failure to record the
/// suspension, naming the Run and the `Authority` reason it was suspended for; nothing is merged.
#[test]
fn a_suspension_the_store_cannot_write_fails_the_loop_naming_the_run() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = store_dir("unwritable");
    let (governor, case, ids) = opened_case(&dir);
    let store_dir = dir.join("suspensions");
    let suspensions = FileSuspensionStore::open(&store_dir).expect("the store opens");
    std::fs::set_permissions(&store_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    assert!(
        std::fs::write(store_dir.join("probe"), "x").is_err(),
        "this test needs a directory its user cannot write; run it as a user without \
         CAP_DAC_OVERRIDE"
    );
    let adapter = Adapter::new(&governor, &case, &ids);
    let model = Model::picking([TESTS_RUN, MERGE]);
    let loom = loom(&model, &governor, "chg-1842-unwritable");
    let mut runs = run_store(0x500);
    let failed = run_with_suspension(
        &governor,
        &loom,
        &withholding(),
        &adapter,
        &commission(&case),
        &mut runs,
        &suspensions,
        &mut Context::starting_at(0x300),
    );
    std::fs::set_permissions(&store_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    let error = failed.expect_err("a suspension the store cannot write fails the loop");
    let run_id = error.run_id.clone().expect("the error names the Run");
    assert!(
        matches!(error.failure, LoopFailure::RunStorage(_)),
        "the failure is the store's: {error}"
    );
    assert!(
        matches!(
            error.suspension_reason.as_deref(),
            Some(SuspensionReason::Authority(_))
        ),
        "the error carries the reason the Run was suspended for: {error}"
    );
    assert_eq!(run_state(&runs, &run_id), "Suspended");
    assert_eq!(adapter.invoked(), [TESTS_RUN], "nothing was merged");
    assert!(
        suspensions
            .get(&commission(&case).data().commission_id)
            .expect("the store reads")
            .is_none(),
        "nothing was stored"
    );
}

/// A governor over the case store in `dir`.
fn governor(dir: &Path) -> Gov {
    Gov::new(FileCaseStore::open(dir.join("cases")).expect("the case store opens"))
}

/// The suspension store in `dir`.
fn suspensions(dir: &Path) -> FileSuspensionStore {
    FileSuspensionStore::open(dir.join("suspensions")).expect("the suspension store opens")
}

/// The fixture case opened on a governor over `dir`, with its initial evidence submitted.
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

/// A provider that withholds the merge: approval required.
fn withholding() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: WITHHELD.to_owned(),
        }),
    )
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

/// The state the run store holds `run` in, by name.
fn run_state(runs: &RunStore, run: &RunId) -> &'static str {
    match runs.get(run).map(|held| held.state) {
        Some(RunState::Running) => "Running",
        Some(RunState::Suspended) => "Suspended",
        None => "absent",
    }
}

fn outcome_name(outcome: &RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Completed(_) => "Completed",
        RunOutcome::Suspended(_) => "Suspended",
        RunOutcome::NeedsAuthority(_) => "NeedsAuthority",
        RunOutcome::NeedsHumanJudgment(_) => "NeedsHumanJudgment",
        RunOutcome::NeedsExternalEvidence(_) => "NeedsExternalEvidence",
        RunOutcome::NoAdmissibleAction(_) => "NoAdmissibleAction",
        RunOutcome::AwaitingApproval(_) => "AwaitingApproval",
        RunOutcome::NoPerformableAction(_) => "NoPerformableAction",
        RunOutcome::CaseMovedOn(_) => "CaseMovedOn",
    }
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

/// Distinct UUIDs, in order, from a base each life chooses apart from the other's.
struct Ids(AtomicU64);

impl Ids {
    fn starting_at(base: u64) -> Self {
        Self(AtomicU64::new(base))
    }

    fn next(&self) -> Uuid {
        uuid(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

/// Submits `record` through Commission's `submit_evidence`, bound to the case's current revision.
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
            json::Value::Text("approval_suspend_resume".to_owned()),
        )]),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

/// The model's choices: one scripted pick per selection, recording how many times it was asked.
struct Model {
    picks: Mutex<VecDeque<&'static str>>,
    offered: Mutex<Vec<usize>>,
}

impl Model {
    fn picking<const N: usize>(picks: [&'static str; N]) -> Self {
        Self {
            picks: Mutex::new(picks.into_iter().collect()),
            offered: Mutex::default(),
        }
    }

    /// The number of candidates of every selection, in order.
    fn offered(&self) -> Vec<usize> {
        lock(&self.offered).clone()
    }
}

/// The model as Loom's selector. Once the script is used up it picks nothing.
struct Selector<'m>(&'m Model);

impl ActionSelector for Selector<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        lock(&self.0.offered).push(candidates.len());
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

/// The model's arguments: none.
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

/// One invocation of the adapter.
#[derive(Debug, Clone)]
struct Call {
    action: String,
    request: String,
    case: CaseId,
    expected_case_revision: i64,
}

/// The trusted action adapter: performs every action `software.change/1` declares and records each
/// invocation. A test run submits the passing result for the implementation revision the case
/// holds.
struct Adapter<'g> {
    governor: &'g Gov,
    case: &'g CaseId,
    ids: &'g Ids,
    calls: Mutex<Vec<Call>>,
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

    fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }

    fn invoked(&self) -> Vec<String> {
        self.calls().into_iter().map(|call| call.action).collect()
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
        lock(&self.calls).push(Call {
            action: request.action.clone(),
            request: request.action_request_id.0.0.clone(),
            case: request.case_id.clone(),
            expected_case_revision: request.expected_case_revision,
        });
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

/// New ids from a counter starting at a base each life chooses apart from the other's.
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

/// What one life records for the parent: `key=value` lines.
#[derive(Default)]
struct Record(BTreeMap<String, String>);

impl Record {
    fn set(&mut self, key: &str, value: impl ToString) {
        self.0.insert(key.to_owned(), value.to_string());
    }

    fn get(&self, key: &str) -> &str {
        self.0
            .get(key)
            .unwrap_or_else(|| panic!("the record holds `{key}`: {:?}", self.0))
    }

    fn write(&self, path: &Path) {
        let text: String = self
            .0
            .iter()
            .map(|(key, value)| format!("{key}={value}\n"))
            .collect();
        std::fs::write(path, text).expect("the record is written");
    }

    fn read(path: &Path) -> Self {
        Self(
            read(path)
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        )
    }
}

/// Runs this test's binary as a child in `role` over `dir` and requires it to exit 0.
fn run_child(role: &str, dir: &Path) {
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(DIR, dir)
        .output()
        .expect("the child starts");
    assert!(
        output.status.success(),
        "the {role} child failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 passed"),
        "the {role} child ran the one test: {stdout}"
    );
}

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("approval-suspend-resume")
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

/// The ELS `chg-1842` fixture, kept beside the governor's tests.
struct Fixture(Value);

impl Fixture {
    fn load() -> Self {
        let manifest =
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        let path =
            PathBuf::from(manifest).join("../loom-governor/tests/fixtures/chg-1842.fixture.yaml");
        let fixture: Value = serde_yaml_ng::from_str(&read(&path))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(fixture["format"], "els-fixture/1");
        assert_eq!(fixture["id"], "chg-1842");
        assert_eq!(fixture["case"]["id"], CASE);
        Self(fixture)
    }

    /// The case's artifact revisions, as the fixture gives them.
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

    /// The evidence records state `id` adds, each with its `observed_at`.
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
