//! Adversary cases for `story:loop-context-split`: the parts' defaults against `LoopContext`'s own,
//! a context assembled from parts that leaves every default against a single context that leaves
//! every default, and the shapes of `impl LoopContext` a host could already write that the blanket
//! impl must not reject (a boxed local type, a generic local wrapper, a type implementing some of
//! the parts beside `LoopContext` itself).

use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BreachStep, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomeRefused, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Frontier, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, StopPolicy, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{
    Clock, HostControl, Ids, Limits, LoopContext, LoopEnd, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const EVALUATE: &str = "evaluate";
const NOW: &str = "2026-10-23T09:00:00Z";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Ids from counters, shared by every context below so only the defaults differ.
#[derive(Default)]
struct Counters {
    requests: u64,
    observations: u64,
}

impl Counters {
    fn request(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }
}

/// A single `LoopContext` implementing only the required methods.
#[derive(Default)]
struct WholeDefaults {
    ids: Counters,
}

impl LoopContext for WholeDefaults {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation()
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The same context from the four parts, each implementing only its required methods.
#[derive(Default)]
struct PartsDefaults {
    ids: Counters,
}

impl Ids for PartsDefaults {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation()
    }
}

impl Clock for PartsDefaults {
    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }
}

impl Limits for PartsDefaults {
    fn step_budget(&self) -> Option<usize> {
        None
    }
}

impl HostControl for PartsDefaults {}

/// The default `LoopContext` documents, written out as a literal.
fn documented_default_policy() -> StopPolicy {
    StopPolicy {
        approval_gate: true,
        idle_bound: None,
        on_breach: BreachStep::DiscardStep,
        breach_by_limit: Vec::new(),
    }
}

#[test]
fn adversary_every_part_default_answers_as_the_loop_context_default() {
    let mut whole = WholeDefaults::default();
    let mut parts = PartsDefaults::default();

    assert_eq!(LoopContext::run_budget(&parts), None);
    assert_eq!(LoopContext::run_budget(&whole), None);
    assert_eq!(
        LoopContext::stop_policy(&parts),
        documented_default_policy(),
        "a parts context leaving Limits::stop_policy gets the documented default"
    );
    assert_eq!(
        LoopContext::stop_policy(&whole),
        documented_default_policy()
    );
    assert_eq!(LoopContext::host_stop(&mut parts), None);
    assert_eq!(LoopContext::host_stop(&mut whole), None);
    // Both forward the ids and clock the context holds, in order.
    assert_eq!(
        LoopContext::action_request_id(&mut parts),
        ActionRequestId(uuid(0x301))
    );
    assert_eq!(
        LoopContext::observation_id(&mut parts),
        ObservationId(uuid(0x401))
    );
    assert_eq!(LoopContext::now(&mut parts), Timestamp(NOW.to_owned()));
    assert_eq!(LoopContext::step_budget(&parts), None);
}

/// Refuses every request with an ordinary refusal, so the run ends on the default idle bound.
struct Refusing;

impl EffectPort for Refusing {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Refused(EffectOutcomeRefused {
            reason: "nothing changed".to_owned(),
            ends_run: false,
        }))
    }
}

/// Proposes `evaluate` with fresh arguments on every call.
#[derive(Default)]
struct Session {
    calls: AtomicUsize,
}

impl AgentExecutor for Session {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: EVALUATE.to_owned(),
            arguments: ProposedActionArguments(Value::Number(call.to_string())),
        })
    }
}

fn run<C: LoopContext>(name: &str, context: &mut C) -> LoopEnd {
    let case = CaseId(format!("case-adv-split-{name}"));
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(1).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: EVALUATE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )],
    );
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x71)),
        agent_revision_id: AgentRevisionId(uuid(0x72)),
        case_id: case,
        principal: PrincipalId("principal-adv-split".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        &governor,
        &Session::default(),
        &StaticAuthorityProvider::new(),
        &Refusing,
        &commission,
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

#[test]
fn adversary_a_parts_context_leaving_every_default_runs_as_a_whole_one_leaving_every_default() {
    let whole = run("defaults", &mut WholeDefaults::default());
    let parts = run("defaults", &mut PartsDefaults::default());
    assert_eq!(parts, whole);
}

/// Shapes of `impl LoopContext` a consumer could write before the split, which the blanket impl
/// must not reject: these compile or the file does not.
mod shapes {
    use super::*;

    /// A boxed local type: `Box` is fundamental, so `Box<Local>` is local.
    pub(super) struct Local(pub(super) Counters);

    impl LoopContext for Box<Local> {
        fn action_request_id(&mut self) -> ActionRequestId {
            self.0.request()
        }

        fn observation_id(&mut self) -> ObservationId {
            self.0.observation()
        }

        fn now(&mut self) -> Timestamp {
            Timestamp(NOW.to_owned())
        }

        fn step_budget(&self) -> Option<usize> {
            None
        }
    }

    /// A generic local wrapper over any type.
    pub(super) struct Wrapper<T>(pub(super) T, pub(super) Counters);

    impl<T> LoopContext for Wrapper<T> {
        fn action_request_id(&mut self) -> ActionRequestId {
            self.1.request()
        }

        fn observation_id(&mut self) -> ObservationId {
            self.1.observation()
        }

        fn now(&mut self) -> Timestamp {
            Timestamp(NOW.to_owned())
        }

        fn step_budget(&self) -> Option<usize> {
            None
        }
    }

    /// A type implementing three parts and `LoopContext` itself: without the fourth part the
    /// blanket impl does not apply, so the direct impl is the one the runtime reads.
    pub(super) struct ThreeParts(pub(super) Counters);

    impl Ids for ThreeParts {
        fn action_request_id(&mut self) -> ActionRequestId {
            ActionRequestId(uuid(0xdead))
        }

        fn observation_id(&mut self) -> ObservationId {
            ObservationId(uuid(0xdead))
        }
    }

    impl Clock for ThreeParts {
        fn now(&mut self) -> Timestamp {
            Timestamp("1970-01-01T00:00:00Z".to_owned())
        }
    }

    impl Limits for ThreeParts {
        fn step_budget(&self) -> Option<usize> {
            Some(0)
        }
    }

    impl LoopContext for ThreeParts {
        fn action_request_id(&mut self) -> ActionRequestId {
            self.0.request()
        }

        fn observation_id(&mut self) -> ObservationId {
            self.0.observation()
        }

        fn now(&mut self) -> Timestamp {
            Timestamp(NOW.to_owned())
        }

        fn step_budget(&self) -> Option<usize> {
            None
        }
    }
}

#[test]
fn adversary_the_shapes_a_consumer_could_already_write_still_run_as_before() {
    let whole = run("shape", &mut WholeDefaults::default());
    let boxed = run("shape", &mut Box::new(shapes::Local(Counters::default())));
    let wrapped = run("shape", &mut shapes::Wrapper((), Counters::default()));
    let three = run("shape", &mut shapes::ThreeParts(Counters::default()));
    assert_eq!(boxed, whole, "a boxed local context runs as before");
    assert_eq!(wrapped, whole, "a generic wrapper runs as before");
    assert_eq!(
        three, whole,
        "a type with three parts and its own LoopContext is read through its own impl"
    );
}
