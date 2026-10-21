//! Adversary pass 1 on `story:pending-effect` (wave 2026-10-20-w3, U2).
//!
//! A pending read of a fan-out is invoked again only as a read of a fan-out may be invoked at all.
//! Item 8a of the runtime's module doc admits a read only when the handed frontier lists its action
//! with every entry `Admissible` and naming no capability, and
//! `read_fan_out_boundaries.rs::an_admissible_entry_naming_a_capability_refuses_the_fan_out` holds
//! that an `Admissible` entry naming a capability needs authority. Item 8b says each pending
//! request is revalidated as before its first invocation and that "a pending read is never invoked
//! on an authority (8a)". This case moves the read's entry to `Admissible` naming a capability
//! between the read's first answer, `Pending`, and the next iteration, the case at the same
//! revision.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePending,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const SEARCH: &str = "search";
const FETCH: &str = "fetch";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xb1)),
        agent_revision_id: AgentRevisionId(uuid(0xb2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-u2".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn entry(name: &str, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::Admissible,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposed(name: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    }
}

fn pending(seen: &str) -> EffectOutcome {
    EffectOutcome::Pending(EffectOutcomePending {
        report: Value::Text(seen.to_owned()),
        ends_run: false,
    })
}

fn performed(action: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text(format!("done {action}")),
        attempt: None,
        audit: None,
        usage: None,
    })
}

struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-20T15:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// Performs and reads both actions; each invocation takes the next scripted answer for its action,
/// the last one answering again, and every request handed over is recorded as it is invoked. The
/// reads of a fan-out run concurrently, so the record has no fixed order between them.
struct Reads {
    answers: Mutex<BTreeMap<&'static str, VecDeque<EffectOutcome>>>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Reads {
    /// How many times each action was handed to the port, whatever the order of the invocations.
    fn invocations(&self) -> BTreeMap<String, usize> {
        let mut counted = BTreeMap::new();
        for request in self
            .invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            *counted.entry(request.action.clone()).or_insert(0) += 1;
        }
        counted
    }
}

impl EffectPort for Reads {
    fn performs(&self, action: &str) -> bool {
        action == SEARCH || action == FETCH
    }

    fn reads(&self, action: &str) -> bool {
        self.performs(action)
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        Some(invoke_concurrently(self, commission, requests))
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        let answer = {
            let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
            match answers.get_mut(data.action.as_str()) {
                Some(script) if script.len() > 1 => script.pop_front(),
                Some(script) => script.front().cloned(),
                None => None,
            }
        }
        .unwrap_or_else(|| performed(&data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(answer)
    }
}

/// The fan-out is admitted on a frontier listing both reads `Admissible` with no capability:
/// `search` answers `Pending`, `fetch` `Performed`. From the next iteration on, at the same
/// revision, the frontier lists `search` `Admissible` naming the capability `index.grant`, the
/// entry on which item 8a refuses a fan-out whole as needing authority. Item 8b revalidates the
/// pending read as before its first invocation and never invokes a pending read on an authority,
/// so `search` is handed to the port once; the runtime hands it over again through `invoke_reads`.
#[test]
fn adversary_a_pending_read_whose_entry_now_names_a_capability_is_not_invoked_again() {
    let case = CaseId("case-adversary-pending-read-capability".to_owned());
    let governor = FakeGovernor::new();
    // The start's load, the first iteration's completion and frontier, and the two reads'
    // revalidations (two calls each): seven calls on the frontier the fan-out was admitted on.
    // Every call after it lists `search` naming a capability, at the same revision.
    governor.script(
        case.clone(),
        std::iter::repeat_n(listing(4, vec![entry(SEARCH, None), entry(FETCH, None)]), 7).chain([
            listing(
                4,
                vec![entry(SEARCH, Some("index.grant")), entry(FETCH, None)],
            ),
        ]),
    );
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
            reads: vec![proposed(SEARCH), proposed(FETCH)],
        }),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let effects = Reads {
        answers: Mutex::new(BTreeMap::from([
            (
                SEARCH,
                VecDeque::from([pending("index building"), performed(SEARCH)]),
            ),
            (FETCH, VecDeque::from([performed(FETCH)])),
        ])),
        invoked: Mutex::new(Vec::new()),
    };
    let mut issued = 0;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));

    let end = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut runs,
        &mut Context {
            requests: 0x300,
            observations: 0x400,
        },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    // Each read is handed over exactly once: `search` is never invoked again on the entry that now
    // names a capability. The two first invocations run concurrently, so their order is not held.
    assert_eq!(
        effects.invocations(),
        BTreeMap::from([(FETCH.to_owned(), 1), (SEARCH.to_owned(), 1)]),
        "a pending read whose frontier entry now names a capability was invoked again; \
         effects {:?}, outcome {:?}",
        end.effects,
        end.outcome
    );
}
