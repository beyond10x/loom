//! Acceptance for `story:dw-read-fanout` (architecture decision record `read-fan-out`): an
//! executor proposes several reads at once as `ExecutorOutcome::ProposedReads`, and Commission's
//! runtime revalidates each against the same frontier, invokes every admitted read concurrently,
//! joins them, and records each effect once, in the order proposed.
//!
//! The fan-out is admitted only when every listed action is one the effect port reads
//! ([`EffectPort::reads`]) and the frontier the executor was handed lists admissible, needing no
//! authority, and the list holds at least one and at most [`MAX_FAN_OUT`] reads. Otherwise the
//! whole proposal is refused, nothing is invoked, no request is made and the iteration is idle.
//! Each admitted read is its own request, revalidated before any is invoked; one that fails
//! revalidation is recorded with its answer and not invoked, and the others go on. A `Refused`
//! read cancels nothing; an `Err` fails the loop after every read joined, and nothing is retried.
//! A fan-out is one step.
//!
//! The binding half of the acceptance, three reads through `ConnectorsInvoker` against a fake
//! Connectors service, is `crates/loom-connectors/tests/read_fan_out.rs`
//! (`each_read_is_invoked_once_through_its_binding`): only `b10x-loom-connectors` links
//! Connectors.

use std::collections::BTreeMap;
use std::iter::repeat_n;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::sleep;
use std::time::{Duration, Instant};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, ConnectorAuditRef, EffectOutcome,
    EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RunOutcomeAwaitingApproval, RunOutcomeCompleted, SuspensionReason, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::runtime::{
    EFFECT_SOURCE, EXECUTOR_SOURCE, LoopContext, LoopEnd, LoopError, LoopFailure, MAX_FAN_OUT,
    run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-15T09:00:00Z";

/// How long a read waits for the others at the meeting point before it reports that it ran alone.
const MEETING: Duration = Duration::from_secs(10);

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-r".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn admissible(name: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

fn gated(name: &str, capability: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: Some(capability.to_owned()),
        reasons: Vec::new(),
    }
}

/// The case open at `revision`, its frontier listing exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

/// The arguments the executor proposes for `action`.
fn arguments(action: &str) -> Value {
    Value::Object(vec![("source".to_owned(), Value::Text(action.to_owned()))])
}

fn reads(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions
            .iter()
            .map(|action| ExecutorOutcomeProposedAction {
                action: (*action).to_owned(),
                arguments: ProposedActionArguments(arguments(action)),
            })
            .collect(),
    })
}

/// The audit record the fake port's read of `action` names.
fn audit(action: &str) -> ConnectorAuditRef {
    ConnectorAuditRef(format!("audit-{action}"))
}

fn performed(action: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text(format!("read {action}")),
        attempt: None,
        audit: Some(audit(action)),
        usage: None,
    })
}

/// New ids from counters and the one trusted time [`NOW`]; no step budget unless given one.
#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the fake port does when it is handed a request for one action.
#[derive(Debug, Clone, Copy)]
enum Plan {
    /// Waits this long, then answers `Performed`, naming the action's audit record.
    Perform(Duration),
    /// Answers `Refused` at once.
    Refuse,
    /// Answers `Err` at once.
    Fail,
}

/// Where reads that must run together meet: each arrives and waits until `expected` have
/// arrived, or until [`MEETING`] passes, when it reports that it ran alone.
#[derive(Debug)]
struct Meeting {
    expected: usize,
    arrived: Mutex<usize>,
    all: Condvar,
}

impl Meeting {
    fn new(expected: usize) -> Self {
        Self {
            expected,
            arrived: Mutex::new(0),
            all: Condvar::new(),
        }
    }

    /// Whether every expected read arrived before the meeting timed out.
    fn meet(&self) -> bool {
        let deadline = Instant::now() + MEETING;
        let mut arrived = lock(&self.arrived);
        *arrived += 1;
        self.all.notify_all();
        while *arrived < self.expected {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            arrived = self
                .all
                .wait_timeout(arrived, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}

/// A fake effect port that reads `reads` and performs `performs`, and is shared by the threads of
/// a fan-out. It records each request it was handed, in the order handed, and each action whose
/// invocation returned, in the order returned.
#[derive(Debug)]
struct Reads {
    reads: Vec<&'static str>,
    performs: Vec<&'static str>,
    plans: BTreeMap<&'static str, Plan>,
    meeting: Option<Meeting>,
    /// Whether the port invokes reads concurrently; a port that does not keeps the default.
    concurrent: bool,
    handed: Mutex<Vec<ActionRequestData>>,
    returned: Mutex<Vec<String>>,
}

impl Reads {
    /// A port that reads and performs `actions`, each performed at once.
    fn new(actions: &[&'static str]) -> Self {
        Self {
            reads: actions.to_vec(),
            performs: actions.to_vec(),
            plans: BTreeMap::new(),
            meeting: None,
            concurrent: true,
            handed: Mutex::new(Vec::new()),
            returned: Mutex::new(Vec::new()),
        }
    }

    /// The same port, also performing `writes`, which it does not read.
    fn writing(mut self, writes: &[&'static str]) -> Self {
        self.performs.extend_from_slice(writes);
        self
    }

    /// The same port, answering `action` as `plan` says.
    fn planning(mut self, action: &'static str, plan: Plan) -> Self {
        self.plans.insert(action, plan);
        self
    }

    /// The same port, every read waiting for `expected` reads to be under way together.
    fn meeting(mut self, expected: usize) -> Self {
        self.meeting = Some(Meeting::new(expected));
        self
    }

    /// The same port, reading nothing: the default `reads` and `invoke_reads`.
    fn not_reading(mut self) -> Self {
        self.reads.clear();
        self.concurrent = false;
        self
    }

    fn handed(&self) -> Vec<String> {
        lock(&self.handed)
            .iter()
            .map(|request| request.action.clone())
            .collect()
    }

    fn returned(&self) -> Vec<String> {
        lock(&self.returned).clone()
    }
}

impl EffectPort for Reads {
    fn performs(&self, action: &str) -> bool {
        self.performs.contains(&action)
    }

    fn reads(&self, action: &str) -> bool {
        self.reads.contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.clone();
        lock(&self.handed).push(request.data().clone());
        let met = self.meeting.as_ref().is_none_or(Meeting::meet);
        let plan = self
            .plans
            .get(action.as_str())
            .copied()
            .unwrap_or(Plan::Perform(Duration::ZERO));
        let answer = if met {
            match plan {
                Plan::Perform(delay) => {
                    sleep(delay);
                    Ok(performed(&action))
                }
                Plan::Refuse => Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                    reason: format!("{action} refused"),
                    ends_run: false,
                })),
                Plan::Fail => Err(EffectError::new(format!("{action} failed"))),
            }
        } else {
            Err(EffectError::new(format!(
                "{action} ran alone: the other reads were not under way with it"
            )))
        };
        lock(&self.returned).push(action);
        answer
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        self.concurrent
            .then(|| invoke_concurrently(self, commission, requests))
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

/// Runs one loop of the commission on `case` over the ports given.
fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Reads,
    case: &CaseId,
    mut context: Context,
) -> Result<LoopEnd, LoopError> {
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut store(),
        &mut context,
    )
}

fn ended(name: &str, result: Result<LoopEnd, LoopError>) -> LoopEnd {
    result.unwrap_or_else(|error| panic!("{name}: the loop failed: {error:?}"))
}

/// The case open at revision 1 with `actions` for `open` governor calls, then complete.
fn open_then_complete(governor: &FakeGovernor, case: &CaseId, actions: &[&str], open: usize) {
    let frontier = listing(1, actions.iter().map(|a| admissible(a)).collect());
    governor.script(
        case.clone(),
        repeat_n(frontier.clone(), open).chain([frontier.complete("collected")]),
    );
}

/// The governor calls one fan-out iteration of `n` reads makes, the Run's start included: the
/// revision at the start, the completion and the frontier, then each revalidation's revision and
/// frontier.
fn first_fan_out_calls(n: usize) -> usize {
    3 + 2 * n
}

/// The actions of the effect observations the governor received, in the order received.
fn observed_effects(governor: &FakeGovernor) -> Vec<String> {
    governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == EFFECT_SOURCE)
        .map(|observation| match observation.payload {
            Value::Object(members) => members
                .into_iter()
                .find(|(name, _)| name == "action")
                .and_then(|(_, value)| match value {
                    Value::Text(text) => Some(text),
                    _ => None,
                })
                .unwrap_or_default(),
            other => panic!("an effect observation's payload is no object: {other:?}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

/// Three reads that each wait until all three are under way complete: run one at a time, the
/// first would wait alone and report it. They are one step, one executor call, three admitted
/// requests at the frontier's revision and three `Performed` effects.
#[test]
fn admitted_reads_run_concurrently_and_join() {
    let name = "concurrent";
    let case = CaseId("case-fan-out".to_owned());
    let governor = FakeGovernor::new();
    let sources = ["logs.read", "metrics.read", "deploys.read"];
    open_then_complete(&governor, &case, &sources, first_fan_out_calls(3));
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let effects = Reads::new(&sources).meeting(3);
    let authority = StaticAuthorityProvider::new();

    let end = ended(
        name,
        run(
            &governor,
            &executor,
            &authority,
            &effects,
            &case,
            Context::default(),
        ),
    );

    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "collected".to_owned()
        }),
        "{name}"
    );
    assert_eq!(end.steps, 1, "{name}: a fan-out is one step");
    assert_eq!(executor.calls().len(), 1, "{name}: one executor call");
    assert_eq!(
        end.effects,
        sources.iter().map(|s| performed(s)).collect::<Vec<_>>(),
        "{name}: every read met the others and was performed"
    );
    assert_eq!(effects.handed().len(), 3, "{name}: each read handed once");
    assert_eq!(end.admitted.len(), 3, "{name}");
    for (n, (admitted, source)) in end.admitted.iter().zip(sources).enumerate() {
        assert_eq!(admitted.action, source, "{name}: request {n}");
        assert_eq!(admitted.expected_case_revision, 1, "{name}: request {n}");
        assert_eq!(admitted.run_id, end.run_id, "{name}: request {n}");
        assert_eq!(
            admitted.action_request_id,
            ActionRequestId(uuid(0x301 + n as u64)),
            "{name}: request {n} has its own id"
        );
    }
    assert!(
        end.requests
            .iter()
            .all(|r| r.outcome == RevalidateActionRequestOutcome::Admitted),
        "{name}: {:?}",
        end.requests
    );
}

/// Reads that finish in the reverse of the order proposed are recorded, admitted, observed and
/// answered in the order proposed.
#[test]
fn effects_are_recorded_in_proposal_order() {
    let name = "proposal order";
    let case = CaseId("case-order".to_owned());
    let governor = FakeGovernor::new();
    let sources = ["slow.read", "middle.read", "fast.read"];
    open_then_complete(&governor, &case, &sources, first_fan_out_calls(3));
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let effects = Reads::new(&sources)
        .planning("slow.read", Plan::Perform(Duration::from_millis(400)))
        .planning("middle.read", Plan::Perform(Duration::from_millis(200)));
    let authority = StaticAuthorityProvider::new();

    let end = ended(
        name,
        run(
            &governor,
            &executor,
            &authority,
            &effects,
            &case,
            Context::default(),
        ),
    );

    assert_eq!(
        effects.returned(),
        vec!["fast.read", "middle.read", "slow.read"],
        "{name}: the reads finished out of proposal order"
    );
    assert_eq!(
        end.effects,
        sources.iter().map(|s| performed(s)).collect::<Vec<_>>(),
        "{name}"
    );
    assert_eq!(
        end.admitted
            .iter()
            .map(|r| r.action.as_str())
            .collect::<Vec<_>>(),
        sources,
        "{name}"
    );
    assert_eq!(
        end.requests
            .iter()
            .map(|r| r.request.action.as_str())
            .collect::<Vec<_>>(),
        sources,
        "{name}"
    );
    assert_eq!(observed_effects(&governor), sources, "{name}");
    let step = governor
        .observations()
        .into_iter()
        .find(|observation| observation.source == EXECUTOR_SOURCE)
        .unwrap_or_else(|| panic!("{name}: the step was not observed"));
    let Value::Object(payload) = step.payload else {
        panic!("{name}: the step's payload is no object");
    };
    assert!(
        payload.contains(&(
            "outcome".to_owned(),
            Value::Text("ProposedReads".to_owned())
        )),
        "{name}: {payload:?}"
    );
    let listed = payload
        .iter()
        .find(|(member, _)| member == "reads")
        .map(|(_, value)| value.clone());
    assert_eq!(
        listed,
        Some(Value::Array(
            sources
                .iter()
                .map(|s| Value::Object(vec![
                    ("action".to_owned(), Value::Text((*s).to_owned())),
                    ("arguments".to_owned(), arguments(s)),
                ]))
                .collect()
        )),
        "{name}: the step's observation lists the reads in proposal order"
    );
}

/// A fan-out that lists a write, a read the frontier lists as needing approval, a read the port
/// does not read, an action the frontier does not list, or a port that keeps the default `reads`,
/// is refused whole: no request is made, nothing is invoked, no authority is asked, and the
/// iteration is idle.
#[test]
fn a_fan_out_with_a_write_or_an_approval_required_read_invokes_nothing() {
    struct Case {
        name: &'static str,
        frontier: Vec<FrontierAction>,
        proposed: Vec<&'static str>,
        effects: Reads,
        outcome: RunOutcome,
    }
    let idle_end = RunOutcome::NoAdmissibleAction(Unit(true));
    let cases = vec![
        Case {
            name: "a write among the reads",
            frontier: vec![admissible("logs.read"), admissible("ticket.write")],
            proposed: vec!["logs.read", "ticket.write"],
            effects: Reads::new(&["logs.read"]).writing(&["ticket.write"]),
            outcome: idle_end.clone(),
        },
        Case {
            name: "a read that needs approval",
            frontier: vec![
                admissible("logs.read"),
                gated("audit.read", "audit.read.grant"),
            ],
            proposed: vec!["logs.read", "audit.read"],
            effects: Reads::new(&["logs.read", "audit.read"]),
            outcome: RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
                actions: vec!["audit.read".to_owned()],
            }),
        },
        Case {
            name: "an action the frontier does not list",
            frontier: vec![admissible("logs.read")],
            proposed: vec!["logs.read", "metrics.read"],
            effects: Reads::new(&["logs.read", "metrics.read"]),
            outcome: idle_end.clone(),
        },
        Case {
            name: "a port that reads nothing",
            frontier: vec![admissible("logs.read"), admissible("metrics.read")],
            proposed: vec!["logs.read", "metrics.read"],
            effects: Reads::new(&["logs.read", "metrics.read"]).not_reading(),
            outcome: idle_end.clone(),
        },
    ];
    for Case {
        name,
        frontier,
        proposed,
        effects,
        outcome,
    } in cases
    {
        let case = CaseId(format!("case-refused-{}", name.replace(' ', "-")));
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [listing(1, frontier)]);
        let executor = ScriptedExecutor::new([reads(&proposed), reads(&proposed)]);
        let authority = StaticAuthorityProvider::new();

        let end = ended(
            name,
            run(
                &governor,
                &executor,
                &authority,
                &effects,
                &case,
                Context::default(),
            ),
        );

        assert_eq!(effects.handed(), Vec::<String>::new(), "{name}: invoked");
        assert!(end.requests.is_empty(), "{name}: {:?}", end.requests);
        assert!(end.admitted.is_empty(), "{name}");
        assert!(end.effects.is_empty(), "{name}");
        assert!(authority.asked().is_empty(), "{name}: authority asked");
        assert!(observed_effects(&governor).is_empty(), "{name}");
        assert_eq!(end.outcome, outcome, "{name}");
    }
}

/// An empty fan-out, and one of more than [`MAX_FAN_OUT`] reads, are refused whole.
#[test]
fn an_empty_or_oversized_fan_out_invokes_nothing() {
    let many: Vec<String> = (0..=MAX_FAN_OUT)
        .map(|n| format!("source-{n}.read"))
        .collect();
    let many: Vec<&'static str> = many
        .into_iter()
        .map(|s| &*Box::leak(s.into_boxed_str()))
        .collect();
    for (name, proposed) in [("empty", Vec::new()), ("oversized", many.clone())] {
        let case = CaseId(format!("case-{name}"));
        let governor = FakeGovernor::new();
        governor.script(
            case.clone(),
            [listing(1, many.iter().map(|a| admissible(a)).collect())],
        );
        let executor = ScriptedExecutor::new([reads(&proposed), reads(&proposed)]);
        let effects = Reads::new(&many);
        let authority = StaticAuthorityProvider::new();

        let end = ended(
            name,
            run(
                &governor,
                &executor,
                &authority,
                &effects,
                &case,
                Context::default(),
            ),
        );

        assert!(effects.handed().is_empty(), "{name}: invoked");
        assert!(end.requests.is_empty(), "{name}");
        assert_eq!(
            end.outcome,
            RunOutcome::NoAdmissibleAction(Unit(true)),
            "{name}"
        );
    }

    // At the bound itself the fan-out is admitted.
    let name = "at the bound";
    let at_bound = &many[..MAX_FAN_OUT];
    let case = CaseId("case-at-bound".to_owned());
    let governor = FakeGovernor::new();
    open_then_complete(&governor, &case, at_bound, first_fan_out_calls(MAX_FAN_OUT));
    let executor = ScriptedExecutor::new([reads(at_bound)]);
    let effects = Reads::new(at_bound);
    let end = ended(
        name,
        run(
            &governor,
            &executor,
            &StaticAuthorityProvider::new(),
            &effects,
            &case,
            Context::default(),
        ),
    );
    assert_eq!(end.effects.len(), MAX_FAN_OUT, "{name}");
}

/// The case moves while the second of three reads is revalidated: that read is recorded stale and
/// never invoked, and the first and third are performed.
#[test]
fn a_stale_read_is_refused_and_the_others_are_performed() {
    let name = "stale read";
    let case = CaseId("case-stale".to_owned());
    let governor = FakeGovernor::new();
    let sources = ["logs.read", "metrics.read", "deploys.read"];
    let open = listing(1, sources.iter().map(|a| admissible(a)).collect());
    // The start, completion, frontier and the first read's revalidation; the second read's
    // revision is 2; the third read's revalidation is at 1 again; then the case is complete.
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 5)
            .chain([listing(2, Vec::new())])
            .chain(repeat_n(open.clone(), 2))
            .chain([open.complete("collected")]),
    );
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let effects = Reads::new(&sources);
    let authority = StaticAuthorityProvider::new();

    let end = ended(
        name,
        run(
            &governor,
            &executor,
            &authority,
            &effects,
            &case,
            Context::default(),
        ),
    );

    assert_eq!(end.requests.len(), 3, "{name}");
    assert_eq!(
        end.requests[0].outcome,
        RevalidateActionRequestOutcome::Admitted,
        "{name}"
    );
    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "{name}: {:?}",
        end.requests[1].outcome
    );
    assert_eq!(
        end.requests[2].outcome,
        RevalidateActionRequestOutcome::Admitted,
        "{name}"
    );
    let mut handed = effects.handed();
    handed.sort();
    assert_eq!(handed, vec!["deploys.read", "logs.read"], "{name}");
    assert_eq!(
        end.admitted
            .iter()
            .map(|r| r.action.as_str())
            .collect::<Vec<_>>(),
        vec!["logs.read", "deploys.read"],
        "{name}"
    );
    assert_eq!(
        end.effects,
        vec![performed("logs.read"), performed("deploys.read")],
        "{name}"
    );
    assert_eq!(
        observed_effects(&governor),
        vec!["logs.read", "deploys.read"],
        "{name}"
    );
    assert_eq!(end.steps, 1, "{name}");
}

/// A `Refused` read cancels nothing; an `Err` from one read fails the loop only after every read
/// returned, each handed once, with nothing retried; the reads that answered are observed, in
/// proposal order, and the Run is suspended for `ExternalAvailability`.
#[test]
fn an_effect_error_fails_the_loop_after_every_read_joined() {
    let name = "effect error";
    let case = CaseId("case-error".to_owned());
    let governor = FakeGovernor::new();
    let sources = ["logs.read", "metrics.read", "deploys.read", "tickets.read"];
    open_then_complete(&governor, &case, &sources, first_fan_out_calls(4));
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let effects = Reads::new(&sources)
        .planning("logs.read", Plan::Perform(Duration::from_millis(300)))
        .planning("metrics.read", Plan::Fail)
        .planning("deploys.read", Plan::Refuse)
        .planning("tickets.read", Plan::Perform(Duration::from_millis(300)));
    let authority = StaticAuthorityProvider::new();

    let error = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::default(),
    )
    .expect_err("an effect error fails the loop");

    let mut returned = effects.returned();
    returned.sort();
    let mut all: Vec<String> = sources.iter().map(|s| (*s).to_owned()).collect();
    all.sort();
    assert_eq!(
        returned, all,
        "{name}: every read returned before the loop failed"
    );
    assert_eq!(effects.handed().len(), 4, "{name}: each handed once");
    assert_eq!(
        error.failure,
        LoopFailure::Effect(EffectError::new("metrics.read failed")),
        "{name}"
    );
    assert!(error.run_id.is_some(), "{name}: the error names the Run");
    assert!(
        matches!(
            error.suspension_reason.as_deref(),
            Some(SuspensionReason::ExternalAvailability(_))
        ),
        "{name}: {:?}",
        error.suspension_reason
    );
    assert_eq!(
        observed_effects(&governor),
        vec!["logs.read", "deploys.read", "tickets.read"],
        "{name}: the reads that answered are observed in proposal order"
    );
}
