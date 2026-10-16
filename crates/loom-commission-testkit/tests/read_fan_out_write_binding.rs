//! The concurrent path never invokes a binding that is not a read (`story:dw-read-fanout`,
//! `architecture-decision-record:read-fan-out`).
//!
//! The invariant checked: a Connector binding whose effect is not `Read` is never invoked through
//! the concurrent path. `ConnectorEffects::invoke_reads` is that path; the runtime's
//! admission check (through `EffectPort::reads`) keeps a `Write` binding off it. A host port that
//! wraps `ConnectorEffects` and forwards `invoke_reads` (as a recording wrapper would) decides
//! `reads` itself, so the binding's own declared effect is the last line that can hold.

use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ConnectorAttemptId, ConnectorAuditRef, ConnectorInstanceId, ConnectorOperationEffect,
    ConnectorOperationId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, FrontierAction, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::{ConnectorEffects, ConnectorInvoker};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const WRITE: &str = "ticket.write";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x91)),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: case.clone(),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(
    commission: &CommissionId,
    action: &str,
    effect: ConnectorOperationEffect,
) -> ActionBinding<b10x_loom_commission::model::responsibility::action_binding_state::Declared> {
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: commission.clone(),
            action: action.to_owned(),
        },
        commission_id: commission.clone(),
        instance_id: ConnectorInstanceId("ticketing".to_owned()),
        operation_id: ConnectorOperationId("ticket.update".to_owned()),
        effect,
    })
}

/// A shareable invoker that records the declared effect of every binding it is handed, and
/// answers as that effect requires.
#[derive(Debug, Default)]
struct Recorder {
    effects: Mutex<Vec<ConnectorOperationEffect>>,
}

impl ConnectorInvoker for Recorder {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.effects
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(binding.effect);
        Ok(EffectOutcome::Performed(match binding.effect {
            ConnectorOperationEffect::Write => EffectOutcomePerformed {
                report: Value::Text("written".to_owned()),
                attempt: Some(ConnectorAttemptId("attempt-1".to_owned())),
                audit: None,
            },
            ConnectorOperationEffect::Read => EffectOutcomePerformed {
                report: Value::Text("read".to_owned()),
                attempt: None,
                audit: Some(ConnectorAuditRef("audit-1".to_owned())),
            },
        }))
    }

    fn concurrent(&self) -> Option<&(dyn ConnectorInvoker + Sync)> {
        Some(self)
    }
}

/// A host port over `ConnectorEffects` that forwards the fan-out path and answers `reads` itself.
struct Forwarding<'a> {
    inner: &'a ConnectorEffects<&'a Recorder>,
}

impl EffectPort for Forwarding<'_> {
    fn performs(&self, action: &str) -> bool {
        self.inner.performs(action)
    }

    fn reads(&self, action: &str) -> bool {
        self.inner.performs(action)
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        self.inner.invoke_reads(commission, requests)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.inner.invoke(commission, request)
    }
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x500 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x600 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-15T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// `ConnectorEffects::invoke_reads` must not invoke a binding whose declared effect is `Write`,
/// whoever hands it the request: the concurrent path is for `Read` bindings only.
#[test]
fn the_concurrent_path_never_invokes_a_write_binding() {
    let case = CaseId("case-security-fan-out".to_owned());
    let own = commission(&case);
    let id = own.data().commission_id.clone();
    let recorder = Recorder::default();
    let connector = ConnectorEffects::new(
        &own,
        [binding(&id, WRITE, ConnectorOperationEffect::Write)],
        &recorder,
    )
    .expect("the binding is the commission's");
    assert!(
        !connector.reads(WRITE),
        "precondition: ConnectorEffects itself does not read a write"
    );
    let effects = Forwarding { inner: &connector };

    let frontier = Answer::at(1).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: WRITE.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    );
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(frontier.clone(), 5).chain([frontier.complete("done")]),
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedReads(
        ExecutorOutcomeProposedReads {
            reads: vec![ExecutorOutcomeProposedAction {
                action: WRITE.to_owned(),
                arguments: ProposedActionArguments(Value::Null),
            }],
        },
    )]);
    let authority = StaticAuthorityProvider::new();
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x700 + issued))
    }));

    let _ = run_until_blocked(
        &governor,
        &executor,
        &authority,
        &effects,
        &own,
        &mut runs,
        &mut Context::default(),
    );

    let seen = recorder
        .effects
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert!(
        !seen.contains(&ConnectorOperationEffect::Write),
        "a Write binding was invoked through ConnectorEffects::invoke_reads: {seen:?}"
    );
}
