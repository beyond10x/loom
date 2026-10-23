//! Adversary probes for `story:runtime-wrapper` (wave 2026-10-23-w1, unit 1): does
//! `loom_sdk::Runtime` hand the loop anything other than what the free functions get for the same
//! parts?
//!
//! `runtime_wrapper.rs` compares the two on a context that sets only ids, a clock and a step
//! budget, and a fresh `Runtime` per call. These probes set every other `LoopContext` field (a run
//! budget with a deadline, a stop policy that drops the approval gate and keeps a breached step, a
//! host stop, an event sink), call one `Runtime` twice, resume after a run that held nothing, and
//! share one `FileSuspensionStore` between two Runtimes, each against the free function on an
//! identical world. The fixtures are `runtime_wrapper.rs`'s, copied: the example's run on
//! `software-change@1`, deterministic effects, no model, no network, no git.

#[allow(dead_code)]
#[path = "../examples/software_change.rs"]
mod example;

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use loom_sdk::commission::model::json::{self, Value};
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, BreachStep, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId, HostStop, Observation,
    ObservationData, ObservationId, PrincipalId, RunBudget, RunId, RuntimeEvent, StopPolicy, Unit,
    commission_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use loom_sdk::commission::ports::events::EventSink;
use loom_sdk::commission::ports::evidence::{ObservationPort as _, submit_evidence};
use loom_sdk::commission::ports::governor::Governor as _;
use loom_sdk::commission::ports::suspension::SuspensionPort;
use loom_sdk::commission::runtime::{resume_after_grant, run_with_suspension};
use loom_sdk::governor::FileSuspensionStore;
use loom_sdk::intake::slice::case;
use loom_sdk::intake::slice::run::SliceReads;
use loom_sdk::loom::arguments::ArgumentContext;
use loom_sdk::loom::model::run::{CatalogueEntry, SelectionStrategy};
use loom_sdk::loom::selection::{Choice, SelectionContext, SelectorError};
use loom_sdk::{
    ActionSelector, ArgumentGenerator, CanonGovernor, CaseInputs, Loom, LoopContext,
    MemoryCaseStore, Runtime, run_until_blocked,
};

type Gov = CanonGovernor<MemoryCaseStore>;

const CASE: &str = "case-1";
const MERGE: &str = "repository.merge";
const WITHHELD: &str = "merging needs the release manager's approval";

/// Every `LoopContext` field set, the host stopping at its `stop_at`-th ask (never for `None`):
/// the free run and the `Runtime` run end alike, emit the same events, ask the host as often and
/// invoke the same effects.
fn every_context_field_reaches_the_loop_alike(stop_at: Option<usize>) {
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let free_loom = agent(&script());
    let mut free_runs = run_store(0x500);
    let mut free_context = Context::full(0x300, stop_at);
    let free_end = run_until_blocked(
        &SliceReads(&free.governor),
        &free_loom,
        &Authority::withholding(),
        &free_effects,
        &commission(),
        &mut free_runs,
        &mut free_context,
    );

    let wrapped = World::new();
    let wrapped_effects = Effects::new(&wrapped.governor);
    let (reads, loom, authority) = (
        SliceReads(&wrapped.governor),
        agent(&script()),
        Authority::withholding(),
    );
    let mut runtime = Runtime::new(
        &reads,
        &loom,
        &authority,
        &wrapped_effects,
        run_store(0x500),
        Context::full(0x300, stop_at),
    );
    let wrapped_end = runtime.run(&commission());

    assert_eq!(wrapped_end, free_end, "the same end");
    let shown = format!("{free_end:?}");
    match stop_at {
        Some(_) => assert!(shown.contains("HostStop"), "the host stopped it: {shown}"),
        None => assert!(!shown.contains("HostStop"), "nothing stopped it: {shown}"),
    }
    assert!(
        !runtime.context().events.is_empty(),
        "the sink the context returns received the run's events"
    );
    assert_eq!(
        runtime.context().events,
        free_context.events,
        "the same events"
    );
    assert_eq!(
        runtime.context().host_asked,
        free_context.host_asked,
        "the host asked as often"
    );
    assert_eq!(runtime.context().next, free_context.next, "the same ids");
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(wrapped.state(), free.state());
}

#[test]
fn adversary_w20261023w1_u1_every_context_field_reaches_the_loop_alike() {
    every_context_field_reaches_the_loop_alike(None);
}

#[test]
fn adversary_w20261023w1_u1_a_host_stop_reaches_the_loop_alike() {
    every_context_field_reaches_the_loop_alike(Some(2));
}

/// A deadline already past: the free function and the `Runtime` refuse or suspend alike.
#[test]
fn adversary_w20261023w1_u1_a_passed_deadline_reaches_the_loop_alike() {
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let mut free_context = Context::full(0x300, None);
    free_context.deadline = "2000-01-01T00:00:00Z".to_owned();
    let free_end = run_until_blocked(
        &SliceReads(&free.governor),
        &agent(&script()),
        &Authority::withholding(),
        &free_effects,
        &commission(),
        &mut run_store(0x500),
        &mut free_context,
    );

    let wrapped = World::new();
    let wrapped_effects = Effects::new(&wrapped.governor);
    let (reads, loom, authority) = (
        SliceReads(&wrapped.governor),
        agent(&script()),
        Authority::withholding(),
    );
    let mut context = Context::full(0x300, None);
    context.deadline = "2000-01-01T00:00:00Z".to_owned();
    let mut runtime = Runtime::new(
        &reads,
        &loom,
        &authority,
        &wrapped_effects,
        run_store(0x500),
        context,
    );
    assert!(
        format!("{free_end:?}").contains("Deadline"),
        "the deadline stopped the free run: {free_end:?}"
    );
    assert_eq!(runtime.run(&commission()), free_end);
    assert_eq!(runtime.context().events, free_context.events);
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
}

/// Two `run`s on one `Runtime` are two `run_until_blocked` calls on one run store and one
/// context: the second start sees what the first left, and nothing else.
#[test]
fn adversary_w20261023w1_u1_a_second_run_on_one_runtime_equals_reusing_the_parts() {
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let free_loom = agent(&script());
    let free_reads = SliceReads(&free.governor);
    let free_authority = Authority::withholding();
    let mut free_runs = run_store(0x500);
    let mut free_context = Context::full(0x300, None);
    let free_first = run_until_blocked(
        &free_reads,
        &free_loom,
        &free_authority,
        &free_effects,
        &commission(),
        &mut free_runs,
        &mut free_context,
    );
    let free_second = run_until_blocked(
        &free_reads,
        &free_loom,
        &free_authority,
        &free_effects,
        &commission(),
        &mut free_runs,
        &mut free_context,
    );

    let wrapped = World::new();
    let wrapped_effects = Effects::new(&wrapped.governor);
    let (reads, loom, authority) = (
        SliceReads(&wrapped.governor),
        agent(&script()),
        Authority::withholding(),
    );
    let mut runtime = Runtime::new(
        &reads,
        &loom,
        &authority,
        &wrapped_effects,
        run_store(0x500),
        Context::full(0x300, None),
    );
    let wrapped_first = runtime.run(&commission());
    let wrapped_second = runtime.run(&commission());

    assert_eq!(wrapped_first, free_first, "the first run");
    assert_eq!(wrapped_second, free_second, "the second run");
    assert_eq!(runtime.context().events, free_context.events);
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(wrapped.state(), free.state());
}

/// A `resume` after a `run` that held no Suspension is refused as `resume_after_grant` refuses it,
/// and invokes nothing.
#[test]
fn adversary_w20261023w1_u1_a_resume_after_a_run_that_held_nothing_is_refused_alike() {
    let free_dir = tempfile::tempdir().expect("a temporary directory");
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let free_suspensions = suspensions(free_dir.path());
    let free_reads = SliceReads(&free.governor);
    let free_loom = agent(&script());
    let free_authority = Authority::withholding();
    let mut free_runs = run_store(0x500);
    let mut free_context = Context::full(0x300, None);
    let free_ran = run_until_blocked(
        &free_reads,
        &free_loom,
        &free_authority,
        &free_effects,
        &commission(),
        &mut free_runs,
        &mut free_context,
    );
    let free_calls = free_effects.calls().len();
    let free_resumed = resume_after_grant(
        &free_reads,
        &free_loom,
        &free_authority,
        &free_effects,
        &commission(),
        &mut free_runs,
        &free_suspensions,
        &mut free_context,
    );
    assert!(free_resumed.is_err(), "{free_resumed:?}");
    assert_eq!(free_effects.calls().len(), free_calls);

    let wrapped_dir = tempfile::tempdir().expect("a temporary directory");
    let wrapped = World::new();
    let wrapped_effects = Effects::new(&wrapped.governor);
    let wrapped_suspensions = suspensions(wrapped_dir.path());
    let (reads, loom, authority) = (
        SliceReads(&wrapped.governor),
        agent(&script()),
        Authority::withholding(),
    );
    let mut runtime = Runtime::new(
        &reads,
        &loom,
        &authority,
        &wrapped_effects,
        run_store(0x500),
        Context::full(0x300, None),
    );
    assert_eq!(runtime.run(&commission()), free_ran);
    assert_eq!(
        runtime.resume(&commission(), &wrapped_suspensions),
        free_resumed
    );
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(runtime.context().events, free_context.events);
}

/// Two Runtimes over one `FileSuspensionStore`: the second may not start a Run for a commission
/// the first suspended, as `run_with_suspension` refuses it, and resumes it as
/// `resume_after_grant` does.
#[test]
fn adversary_w20261023w1_u1_two_runtimes_on_one_suspension_store_act_as_the_free_functions() {
    let free_dir = tempfile::tempdir().expect("a temporary directory");
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let free_suspensions = suspensions(free_dir.path());
    let free_reads = SliceReads(&free.governor);
    let free_suspended = run_with_suspension(
        &free_reads,
        &agent(&script()),
        &Authority::withholding(),
        &free_effects,
        &commission(),
        &mut run_store(0x500),
        &free_suspensions,
        &mut Context::full(0x300, None),
    );
    let mut free_second_runs = run_store(0x600);
    let mut free_second_context = Context::full(0x700, None);
    let free_second_loom = agent(&[]);
    let free_refused = run_with_suspension(
        &free_reads,
        &free_second_loom,
        &Authority::granting(),
        &free_effects,
        &commission(),
        &mut free_second_runs,
        &free_suspensions,
        &mut free_second_context,
    );
    assert!(
        format!("{free_refused:?}").contains("CommissionSuspended"),
        "{free_refused:?}"
    );
    let free_resumed = resume_after_grant(
        &free_reads,
        &free_second_loom,
        &Authority::granting(),
        &free_effects,
        &commission(),
        &mut free_second_runs,
        &free_suspensions,
        &mut free_second_context,
    );

    let wrapped_dir = tempfile::tempdir().expect("a temporary directory");
    let wrapped = World::new();
    let wrapped_effects = Effects::new(&wrapped.governor);
    let shared = suspensions(wrapped_dir.path());
    let reads = SliceReads(&wrapped.governor);
    let (first_loom, withholding) = (agent(&script()), Authority::withholding());
    let mut first = Runtime::new(
        &reads,
        &first_loom,
        &withholding,
        &wrapped_effects,
        run_store(0x500),
        Context::full(0x300, None),
    );
    assert_eq!(first.run_suspending(&commission(), &shared), free_suspended);

    let other = suspensions(wrapped_dir.path());
    let (second_loom, granting) = (agent(&[]), Authority::granting());
    let mut second = Runtime::new(
        &reads,
        &second_loom,
        &granting,
        &wrapped_effects,
        run_store(0x600),
        Context::full(0x700, None),
    );
    assert_eq!(second.run_suspending(&commission(), &other), free_refused);
    assert_eq!(second.resume(&commission(), &other), free_resumed);
    assert_eq!(second.context().events, free_second_context.events);
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(held(&other), held(&free_suspensions));
    assert_eq!(wrapped.state(), free.state());
}

/// One case on `software-change@1` at fixed revisions, with the command line's default inputs.
struct World {
    governor: Gov,
}

impl World {
    fn new() -> Self {
        let governor = Gov::new(MemoryCaseStore::default());
        let revisions: BTreeMap<String, String> = [
            ("intent", case::intent_revision(example::INTENT)),
            ("system_specification", case::INITIAL.to_owned()),
            ("plan", case::INITIAL.to_owned()),
            ("implementation", "R1".to_owned()),
            ("release", case::INITIAL.to_owned()),
            ("deployment", case::INITIAL.to_owned()),
        ]
        .into_iter()
        .map(|(artifact, revision)| (artifact.to_owned(), revision))
        .collect();
        let inputs = CaseInputs::try_from_pairs([
            ("risk", "standard"),
            ("change_kind", "feature"),
            ("affects_security_boundary", "false"),
        ])
        .expect("distinct inputs");
        governor
            .open_case(case_id(), example::PROTOCOL, revisions, inputs)
            .expect("the case opens on software-change@1");
        Self { governor }
    }

    /// What the governor holds for the case: its revision and its artifact revisions.
    fn state(&self) -> (i64, BTreeMap<String, String>) {
        (
            self.governor
                .current_revision(&case_id())
                .expect("the case revision"),
            self.governor.revisions(&case_id()).expect("the revisions"),
        )
    }
}

/// The example's script, its actions in order.
fn script() -> Vec<&'static str> {
    example::SCRIPT.iter().map(|(action, _)| *action).collect()
}

/// Loom over a selector picking `picks` in order and the example's arguments.
fn agent(picks: &[&'static str]) -> Loom<Selector, Arguments> {
    Loom::new(
        Selector(Mutex::new(picks.iter().copied().collect())),
        Arguments,
        example::INTENT,
    )
}

/// A fake selector: the next scripted action, else nothing.
struct Selector(Mutex<VecDeque<&'static str>>);

impl ActionSelector for Selector {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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

/// A fake argument generator: the arguments the example's script gives an action, else none.
struct Arguments;

impl ArgumentGenerator for Arguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        match example::SCRIPT
            .iter()
            .find(|(action, _)| *action == entry.action)
        {
            Some((_, arguments)) => json::parse(arguments).map_err(|error| format!("{error:?}")),
            None => Ok(Value::Object(Vec::new())),
        }
    }
}

/// An authority provider that withholds the merge, or grants it; it allows every other capability.
struct Authority {
    grants_merge: bool,
}

impl Authority {
    fn withholding() -> Self {
        Self {
            grants_merge: false,
        }
    }

    fn granting() -> Self {
        Self { grants_merge: true }
    }
}

impl AuthorityProvider for Authority {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(if capability == MERGE && !self.grants_merge {
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: WITHHELD.to_owned(),
            })
        } else {
            AuthorityVerdict::Allow(Unit(true))
        })
    }
}

/// The deterministic effects on one world: the edit moves the case's `implementation` revision to
/// `R<n>`, `n` the invocation's number, and the test run submits a passing `test_result` for the
/// revision the case holds. Each invocation is recorded with its request id.
struct Effects<'g> {
    governor: &'g Gov,
    calls: Mutex<Vec<(String, String)>>,
}

impl<'g> Effects<'g> {
    fn new(governor: &'g Gov) -> Self {
        Self {
            governor,
            calls: Mutex::default(),
        }
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Submits a passing `test_result` for the case's current `implementation` revision, observed
    /// first, as a host does.
    fn pass_tests(&self, number: usize) -> Result<(), EffectError> {
        let failed = |error: &dyn std::fmt::Debug| EffectError::new(format!("{error:?}"));
        let case = case_id();
        let revisions = self.governor.revisions(&case).map_err(|e| failed(&e))?;
        let implementation = revisions
            .get("implementation")
            .ok_or_else(|| EffectError::new("the case has no implementation revision"))?;
        let observed_at = "2026-01-01T00:00:00Z";
        let observation = ObservationId(uuid(0x20_0000 + number as u64));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: "runtime-wrapper/tests".to_owned(),
                subject: "implementation".to_owned(),
                observed_at: Timestamp(observed_at.to_owned()),
                payload: Value::Null,
            }))
            .map_err(|e| failed(&e))?;
        let facts = Value::Object(vec![
            ("format".to_owned(), Value::Text("canon-evidence/1".into())),
            (
                "id".to_owned(),
                Value::Text(format!("tests-{implementation}")),
            ),
            ("kind".to_owned(), Value::Text("test_result".into())),
            ("result".to_owned(), Value::Text("pass".into())),
            ("subject".to_owned(), Value::Text("implementation".into())),
            (
                "subject_revision".to_owned(),
                Value::Text(implementation.clone()),
            ),
            ("observed_at".to_owned(), Value::Text(observed_at.into())),
        ]);
        let evidence = EvidenceData {
            evidence_id: EvidenceId(uuid(0x30_0000 + number as u64)),
            case_id: case.clone(),
            kind: "test_result".to_owned(),
            subject_revision: self
                .governor
                .current_revision(&case)
                .map_err(|e| failed(&e))?,
            producer: String::new(),
            observation_ids: vec![observation],
            facts,
            provenance: Value::Null,
        };
        submit_evidence(self.governor, example::PRODUCER, evidence).map_err(|e| failed(&e))
    }
}

impl EffectPort for Effects<'_> {
    fn performs(&self, action: &str) -> bool {
        [
            "repository.inspect",
            "implementation.edit",
            "tests.run",
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
        let number = {
            let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
            calls.push((
                request.action.clone(),
                request.action_request_id.0.0.clone(),
            ));
            calls.len()
        };
        match request.action.as_str() {
            "implementation.edit" => {
                self.governor
                    .update_revision(&case_id(), "implementation", &format!("R{}", number + 1))
                    .map_err(|error| EffectError::new(format!("{error:?}")))?;
            }
            "tests.run" => self.pass_tests(number)?,
            _ => {}
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text(format!("performed {}", request.action)),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// The Suspension the store holds for the commission.
fn held(
    store: &FileSuspensionStore,
) -> Option<loom_sdk::commission::model::responsibility::SuspensionSnapshot> {
    store
        .get(&commission().data().commission_id)
        .expect("the suspension store reads")
}

/// The production suspension store over `dir`.
fn suspensions(dir: &Path) -> FileSuspensionStore {
    FileSuspensionStore::open(dir.join("state")).expect("the suspension store opens")
}

fn run_store(base: u64) -> RunStore {
    let next = AtomicU64::new(base);
    RunStore::new(move || RunId(uuid(next.fetch_add(1, Ordering::SeqCst))))
}

fn case_id() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: case_id(),
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// New ids from a counter, a fixed clock, a step budget of ten, and every other field set: a run
/// budget with `deadline`, a stop policy with no approval gate, an idle bound of three and
/// `KeepStep`, a host stop at its `stop_at`-th ask, and a sink keeping every event.
struct Context {
    next: u64,
    deadline: String,
    stop_at: Option<usize>,
    host_asked: usize,
    events: Vec<RuntimeEvent>,
}

impl Context {
    fn full(base: u64, stop_at: Option<usize>) -> Self {
        Self {
            next: base,
            deadline: "2030-01-01T00:00:00Z".to_owned(),
            stop_at,
            host_asked: 0,
            events: Vec::new(),
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.next += 1;
        ActionRequestId(uuid(self.next))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.next += 1;
        ObservationId(uuid(0x10_0000 + self.next))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-01-01T00:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(10)
    }

    fn run_budget(&self) -> Option<RunBudget> {
        Some(RunBudget {
            max_steps: Some(9),
            max_tokens_in: None,
            max_tokens_out: None,
            max_duration_ms: None,
            max_cost_micro_usd: None,
            max_tokens: Some(1_000_000),
            deadline: Some(Timestamp(self.deadline.clone())),
            per_action: Vec::new(),
        })
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: false,
            idle_bound: Some(3),
            on_breach: BreachStep::KeepStep,
            breach_by_limit: Vec::new(),
        }
    }

    fn host_stop(&mut self) -> Option<HostStop> {
        self.host_asked += 1;
        (Some(self.host_asked) == self.stop_at).then(|| HostStop {
            name: "disk".to_owned(),
            detail: Value::Text("under 20G".to_owned()),
        })
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.events
    }
}
