//! Adversary pass 2 on `story:agent-executor-port`: the scripted fake when its script runs out.
//!
//! `ScriptedExecutor` documents that "a call after the script is used up panics"
//! (`crates/commission-testkit/src/fake_executor.rs`). No test calls it past its script, so a fake
//! that quietly returned a default outcome instead would pass the whole suite, and a caller such as
//! `story:run-outcomes` or `story:local-runtime-loop` that loops one time too many would be told
//! nothing. These cases pin the documented behaviour.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, Frontier, FrontierData, FrontierId, PrincipalId, Unit, commission_state,
    frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission_testkit::fake_executor::{ExecutorCall, ScriptedExecutor};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn uuid(n: u8) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-0000000000{n:02x}"))
}

fn commission(n: u8) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(n)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("principal-1".to_owned()),
        authority_context: AuthorityContext(Value::Object(Vec::new())),
    })
}

fn frontier(n: u8) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(n)),
        case_id: CaseId("case-1".to_owned()),
        case_revision: 7,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: Vec::new(),
    })
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "<non-text panic>".to_owned())
}

/// One scripted outcome, two calls: the second panics, naming the used-up script, and does not
/// return an outcome nobody scripted.
#[test]
fn adversary2_scripted_executor_panics_past_its_script() {
    let executor = ScriptedExecutor::new([ExecutorOutcome::CompletedLocalReasoning(Unit(true))]);
    assert_eq!(
        executor.run(&commission(1), &frontier(3)),
        ExecutorOutcome::CompletedLocalReasoning(Unit(true))
    );
    let second = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(1), &frontier(4))
    }));
    match second {
        Ok(outcome) => panic!("a call past the script returned {outcome:?} instead of panicking"),
        Err(payload) => {
            let text = panic_text(payload.as_ref());
            assert!(
                text.contains("script is used up"),
                "the panic does not say the script is used up: {text}"
            );
        }
    }
}

/// An empty script panics on the first call.
#[test]
fn adversary2_scripted_executor_with_an_empty_script_panics_on_the_first_call() {
    let executor = ScriptedExecutor::new(Vec::<ExecutorOutcome>::new());
    let first = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(1), &frontier(3))
    }));
    assert!(
        first.is_err(),
        "an empty script returned {:?} instead of panicking",
        first.ok()
    );
}

/// The call log after a used-up call, as built: the call that panicked was still made, so it is
/// logged (the push precedes the pop), and `calls()` keeps answering after the panic poisoned the
/// script's lock. This pins the observed order, which the fake's docs ("records each call") allow.
#[test]
fn adversary2_scripted_executor_logs_the_call_that_found_the_script_used_up() {
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    executor.run(&commission(1), &frontier(3));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(5), &frontier(4))
    }));
    let again = catch_unwind(AssertUnwindSafe(|| {
        executor.run(&commission(6), &frontier(4))
    }));
    let text = again
        .err()
        .map(|payload| panic_text(payload.as_ref()))
        .unwrap_or_default();
    assert!(
        text.contains("script is used up"),
        "a second call past the script did not panic with the script message (poisoned lock?): {text}"
    );
    assert_eq!(
        executor.calls(),
        vec![
            ExecutorCall {
                commission_id: CommissionId(uuid(1)),
                frontier_id: FrontierId(uuid(3)),
            },
            ExecutorCall {
                commission_id: CommissionId(uuid(5)),
                frontier_id: FrontierId(uuid(4)),
            },
            ExecutorCall {
                commission_id: CommissionId(uuid(6)),
                frontier_id: FrontierId(uuid(4)),
            },
        ]
    );
}
