//! The host side of one step of Commission's loop in a tree run: its instants, its ids, its one
//! step and the wall-time stop, and the events it hands the run log.
//!
//! Every id is derived from the case, its revision, the step of the run and a counter
//! ([`derived_uuid`]), never from the process or the time, so two runs from the same store and the
//! same `--at` give the same ids.

use std::time::Instant;

use b10x_loom_commission::model::json::Value as CommissionJson;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, BreachStep, HostStop, ObservationId, RuntimeEvent, StopPolicy,
};
use b10x_loom_commission::ports::events::EventSink;
use b10x_loom_commission::runtime::{Clock, HostControl, Ids, Limits};
use sha2::{Digest as _, Sha256};

use crate::TreeTime;

/// A UUID-shaped id from the SHA-256 of `parts`, each ended by a NUL byte: the same parts give the
/// same id.
pub fn derived_uuid(parts: &[&str]) -> Uuid {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let hex: String = hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// The context of one step of one case.
pub(crate) struct StepContext {
    case: String,
    case_revision: i64,
    step: i64,
    counter: u64,
    time: TreeTime,
    instant: Timestamp,
    wall: Option<(Instant, u64)>,
    pub(crate) events: Vec<RuntimeEvent>,
}

impl StepContext {
    /// The context of step `step` of the run, for `case` at `case_revision`, reading the instant
    /// from `time`; `wall` is the run's start and its wall-time budget in seconds.
    pub(crate) fn new(
        case: &str,
        case_revision: i64,
        step: i64,
        time: TreeTime,
        instant: Timestamp,
        wall: Option<(Instant, u64)>,
    ) -> Self {
        Self {
            case: case.to_owned(),
            case_revision,
            step,
            counter: 0,
            time,
            instant,
            wall,
            events: Vec::new(),
        }
    }

    fn next(&mut self, kind: &str) -> Uuid {
        self.counter += 1;
        derived_uuid(&[
            kind,
            &self.case,
            &self.case_revision.to_string(),
            &self.step.to_string(),
            &self.counter.to_string(),
        ])
    }
}

impl Ids for StepContext {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(self.next("action-request"))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(self.next("observation"))
    }
}

impl Clock for StepContext {
    fn now(&mut self) -> Timestamp {
        match self.time.now() {
            Ok(at) => Timestamp(at.0),
            Err(_) => self.instant.clone(),
        }
    }
}

impl Limits for StepContext {
    /// One step: the tree chooses the case of every step itself.
    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }

    /// No approval gate: an action that awaits approval is proposed, and the tree's authority
    /// decides it from the approvals `tree approve` recorded.
    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: false,
            idle_bound: None,
            on_breach: BreachStep::DiscardStep,
            breach_by_limit: Vec::new(),
        }
    }
}

impl HostControl for StepContext {
    fn host_stop(&mut self) -> Option<HostStop> {
        let (started, seconds) = self.wall?;
        (started.elapsed().as_secs() >= seconds).then(|| HostStop {
            name: "max-wall-seconds".to_owned(),
            detail: CommissionJson::Text(format!("the run's {seconds} s of wall time are spent")),
        })
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.events
    }
}
