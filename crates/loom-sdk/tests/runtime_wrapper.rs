//! Acceptance for `story:runtime-wrapper`: `loom_sdk::Runtime` holds a run's governor, executor,
//! authority, effects, run store and context, and its `run`, `run_suspending` and `resume` return
//! what `run_until_blocked`, `run_with_suspension` and `resume_after_grant` return for the same
//! parts.
//!
//! The run is the example's (`examples/software_change.rs`, compiled in by `#[path]`): a case on
//! `software-change@1`, the scripted agent of its `SCRIPT` (edit the check, run the tests, merge),
//! the governor read as the slice reads it, and an authority provider that withholds the merge,
//! so the run stops at `ApprovalRequired (repository.merge)`. Here the effects are a deterministic
//! fake instead of the slice's git adapter, so two runs on two identical worlds are equal down to
//! their ids: the edit moves the case's `implementation` revision, and the test run submits a
//! passing `test_result` for it. Each comparison builds two such worlds, drives one through the
//! free function and the other through the `Runtime` method, and asserts equal `LoopEnd`s, equal
//! effects invoked and equal state left behind. The suspend and the resume run over a
//! `FileSuspensionStore` in a temporary directory per world. No model, no network, no git.

#[allow(dead_code)]
#[path = "../examples/software_change.rs"]
mod example;

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use loom_sdk::commission::model::behaviour::RunStorage as _;
use loom_sdk::commission::model::json::{self, Value};
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId, Observation, ObservationData,
    ObservationId, PrincipalId, RunId, RunOutcome, Unit, commission_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
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

#[test]
fn runtime_equals_the_free_functions() {
    // `Runtime::run` and `run_until_blocked`, on the example's run.
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let mut free_runs = run_store(0x500);
    let free_end = run_until_blocked(
        &SliceReads(&free.governor),
        &agent(&script()),
        &Authority::withholding(),
        &free_effects,
        &commission(),
        &mut free_runs,
        &mut Context::starting_at(0x300),
    )
    .expect("the free run ends with an outcome");
    assert_eq!(
        example::stop_reason(&free_end),
        "ApprovalRequired (repository.merge)",
        "the free run is the example's: {free_end:#?}"
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
        Context::starting_at(0x300),
    );
    let wrapped_end = runtime
        .run(&commission())
        .expect("the wrapped run ends with an outcome");
    assert_eq!(
        wrapped_end, free_end,
        "`Runtime::run` returns what `run_until_blocked` returns"
    );
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(wrapped.state(), free.state());
    assert_eq!(
        runtime.runs().get(&wrapped_end.run_id),
        free_runs.get(&free_end.run_id),
        "the run store holds the Run as the free function's does"
    );

    // `Runtime::run_suspending` and `run_with_suspension`, each world over its own
    // `FileSuspensionStore`.
    let free_dir = tempfile::tempdir().expect("a temporary directory");
    let free = World::new();
    let free_effects = Effects::new(&free.governor);
    let free_suspensions = suspensions(free_dir.path());
    let mut free_runs = run_store(0x500);
    let free_suspended = run_with_suspension(
        &SliceReads(&free.governor),
        &agent(&script()),
        &Authority::withholding(),
        &free_effects,
        &commission(),
        &mut free_runs,
        &free_suspensions,
        &mut Context::starting_at(0x300),
    )
    .expect("the free run suspends with an outcome");
    assert!(
        matches!(free_suspended.outcome, RunOutcome::NeedsAuthority(_)),
        "{free_suspended:#?}"
    );
    let free_held = held(&free_suspensions);
    assert!(free_held.is_some(), "the free run holds a Suspension");

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
        Context::starting_at(0x300),
    );
    let wrapped_suspended = runtime
        .run_suspending(&commission(), &wrapped_suspensions)
        .expect("the wrapped run suspends with an outcome");
    assert_eq!(
        wrapped_suspended, free_suspended,
        "`Runtime::run_suspending` returns what `run_with_suspension` returns"
    );
    assert_eq!(
        held(&wrapped_suspensions),
        free_held,
        "the same Suspension is held"
    );
    assert_eq!(
        runtime.runs().get(&wrapped_suspended.run_id),
        free_runs.get(&free_suspended.run_id),
        "the Run is suspended alike"
    );
    assert_eq!(wrapped_effects.calls(), free_effects.calls());

    // `Runtime::resume` and `resume_after_grant`, as a new process resumes: a new run store and
    // context, an agent with nothing left to pick, and a provider that now grants the merge.
    let free_resumed = resume_after_grant(
        &SliceReads(&free.governor),
        &agent(&[]),
        &Authority::granting(),
        &free_effects,
        &commission(),
        &mut run_store(0x600),
        &free_suspensions,
        &mut Context::starting_at(0x700),
    )
    .expect("the free resume ends with an outcome");
    assert_eq!(
        free_effects.invoked().last().map(String::as_str),
        Some(MERGE),
        "the free resume performed the merge: {free_resumed:#?}"
    );

    let (reads, loom, authority) = (
        SliceReads(&wrapped.governor),
        agent(&[]),
        Authority::granting(),
    );
    let mut runtime = Runtime::new(
        &reads,
        &loom,
        &authority,
        &wrapped_effects,
        run_store(0x600),
        Context::starting_at(0x700),
    );
    let wrapped_resumed = runtime
        .resume(&commission(), &wrapped_suspensions)
        .expect("the wrapped resume ends with an outcome");
    assert_eq!(
        wrapped_resumed, free_resumed,
        "`Runtime::resume` returns what `resume_after_grant` returns"
    );
    assert_eq!(wrapped_effects.calls(), free_effects.calls());
    assert_eq!(wrapped.state(), free.state());
    assert_eq!(
        held(&wrapped_suspensions),
        held(&free_suspensions),
        "the Suspension is left alike"
    );

    // A refusal comes back as the free function returns it: nothing is held any more.
    let refused_free = resume_after_grant(
        &SliceReads(&free.governor),
        &agent(&[]),
        &Authority::granting(),
        &free_effects,
        &commission(),
        &mut run_store(0x800),
        &free_suspensions,
        &mut Context::starting_at(0x900),
    );
    assert!(refused_free.is_err(), "{refused_free:?}");
    let refused_wrapped = runtime.resume(&commission(), &wrapped_suspensions);
    assert_eq!(refused_wrapped, refused_free);
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

    fn invoked(&self) -> Vec<String> {
        self.calls().into_iter().map(|(action, _)| action).collect()
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

/// New ids from a counter, a fixed clock and a step budget of ten.
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
        Timestamp("2026-01-01T00:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(10)
    }
}
