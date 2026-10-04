//! The conformance kit for a `Governor` adapter.
//!
//! An adapter crate implements [`GovernorFixture`] for its governor and calls [`run`], usually as
//! `run(&MyFixture).assert_passed()` in one of its tests. The kit runs every [`Check`] on a fresh
//! governor from the fixture and reports each check it ran, naming each one that failed. A check
//! during which the governor or the fixture panics fails with the panic's message, and the other
//! checks still run:
//!
//! - [`Check::CurrentFrontier`]: before the case moves, the frontier is issued for the held case at
//!   its current revision and admits at least one action; after it moves, the frontier is issued
//!   for the held case at the new revision;
//! - [`Check::SupersededRevision`]: each time the case moves on, the governor reports a revision it
//!   has not reported before, so Commission's revalidation refuses as stale a request made at a
//!   superseded one, however many changes ago;
//! - [`Check::HeldCaseOpen`]: the held case, before it moves, is answered by `completion` as open;
//! - [`Check::UnknownCase`]: a case the governor does not hold is answered with
//!   `GovernorError::UnknownCase`, by every `Governor` call;
//! - [`Check::ObservationNotEvidence`]: executor outputs delivered as observations are received as
//!   exactly those observations, and never as evidence, also after the governor has since answered
//!   each `Governor` call for the case;
//! - [`Check::EvidenceObservationIds`]: evidence submitted through Commission arrives carrying
//!   exactly the observation ids it was submitted with, not every observation the governor holds.

use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use b10x_commission::action_request::{request, revalidate};
use b10x_commission::admission::admit;
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::{
    ActionRequestId, ActionRequestStale, Admission, CaseId, CompletionDetermination, EvidenceData,
    EvidenceId, ExecutorOutcomeProposedAction, Frontier, GovernorError, Observation,
    ObservationData, ObservationId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    frontier_state,
};
use b10x_commission::ports::evidence::{EvidencePort, ObservationPort, submit_evidence};
use b10x_commission::ports::governor::Governor;

/// What an adapter supplies to the governor kit.
pub trait GovernorFixture {
    /// The adapter's governor, with its observation and evidence ports.
    type Governor: Governor + ObservationPort + EvidencePort;

    /// A fresh governor that holds `case`, open, at a revision whose frontier admits at least one
    /// action, and holds no other case.
    fn hold(&self, case: &CaseId) -> Self::Governor;

    /// Moves `case` on `governor` to a later revision, as a change to the case would. The kit may
    /// call it more than once on one governor.
    ///
    /// After the move, the frontier the governor issues must be for `case` at the new revision. It
    /// need not admit any action.
    fn advance(&self, governor: &Self::Governor, case: &CaseId);

    /// Every observation `governor` has received, in the order received.
    fn observations(&self, governor: &Self::Governor) -> Vec<ObservationData>;

    /// Every evidence record `governor` has received, in the order received.
    fn evidence(&self, governor: &Self::Governor) -> Vec<EvidenceData>;
}

/// One check of the governor kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// The frontier is issued for the held case at its current revision, and admits an action
    /// before the case moves.
    CurrentFrontier,
    /// A revision change is reported, so a request made at a superseded revision is stale.
    SupersededRevision,
    /// The held case is answered by `completion` as open.
    HeldCaseOpen,
    /// An unknown case is answered with `GovernorError::UnknownCase`.
    UnknownCase,
    /// An executor's output arrives as an observation, never as evidence.
    ObservationNotEvidence,
    /// Evidence arrives with the observation ids it was submitted with.
    EvidenceObservationIds,
}

impl Check {
    /// Every check, in the order [`run`] runs them.
    pub const ALL: [Check; 6] = [
        Check::CurrentFrontier,
        Check::SupersededRevision,
        Check::HeldCaseOpen,
        Check::UnknownCase,
        Check::ObservationNotEvidence,
        Check::EvidenceObservationIds,
    ];

    /// The check's name, as a report shows it.
    pub fn name(&self) -> &'static str {
        match self {
            Check::CurrentFrontier => "current-frontier",
            Check::SupersededRevision => "superseded-revision",
            Check::HeldCaseOpen => "held-case-open",
            Check::UnknownCase => "unknown-case",
            Check::ObservationNotEvidence => "observation-not-evidence",
            Check::EvidenceObservationIds => "evidence-observation-ids",
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What one run of the kit found: every check it ran, each with its failure if it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    outcomes: Vec<(Check, Result<(), String>)>,
}

impl Report {
    /// Every check run, in the order run.
    pub fn ran(&self) -> Vec<Check> {
        self.outcomes.iter().map(|(check, _)| *check).collect()
    }

    /// Every check that failed, in the order run.
    pub fn failed(&self) -> Vec<Check> {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| outcome.is_err())
            .map(|(check, _)| *check)
            .collect()
    }

    /// Panics, with the report, unless every check passed.
    ///
    /// # Panics
    ///
    /// When any check failed.
    pub fn assert_passed(&self) {
        assert!(
            self.failed().is_empty(),
            "governor conformance failed:\n{self}"
        );
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (check, outcome) in &self.outcomes {
            match outcome {
                Ok(()) => writeln!(f, "pass {check}")?,
                Err(failure) => writeln!(f, "FAIL {check}: {failure}")?,
            }
        }
        Ok(())
    }
}

/// Runs every check against governors from `fixture`, each on a fresh one. A panic in one check is
/// that check's failure.
pub fn run<F: GovernorFixture>(fixture: &F) -> Report {
    Report {
        outcomes: Check::ALL
            .iter()
            .map(|check| (*check, unwound(|| run_check(fixture, *check))))
            .collect(),
    }
}

/// Runs `check`; a panic in it becomes its failure, carrying the panic's message.
pub(crate) fn unwound(check: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(check))
        .unwrap_or_else(|payload| Err(format!("panicked: {}", panic_message(payload.as_ref()))))
}

/// The message a panic carried, when it carried text.
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("a panic with no message")
}

/// The case every check holds.
const CASE: &str = "kit-case-held";

/// A case no check holds.
const UNKNOWN: &str = "kit-case-unknown";

fn run_check<F: GovernorFixture>(fixture: &F, check: Check) -> Result<(), String> {
    let case = CaseId(CASE.to_owned());
    let governor = fixture.hold(&case);
    match check {
        Check::CurrentFrontier => current_frontier(fixture, &governor, &case),
        Check::SupersededRevision => superseded_revision(fixture, &governor, &case),
        Check::HeldCaseOpen => held_case_open(&governor, &case),
        Check::UnknownCase => unknown_case(&governor),
        Check::ObservationNotEvidence => observation_not_evidence(fixture, &governor, &case),
        Check::EvidenceObservationIds => evidence_observation_ids(fixture, &governor, &case),
    }
}

/// `result`, with a governor error described as the failure of `call`.
fn answered<T>(call: &str, result: Result<T, GovernorError>) -> Result<T, String> {
    result.map_err(|error| format!("{call} on the held case failed: {error:?}"))
}

/// The frontier `governor` issues for `case` now, and the first action it admits outright.
fn admitted_action<G: Governor + ?Sized>(
    governor: &G,
    case: &CaseId,
) -> Result<(String, Frontier<frontier_state::Issued>), String> {
    let frontier = answered("frontier", governor.frontier(case))?;
    let action = frontier
        .data()
        .actions
        .iter()
        .map(|listed| listed.action.clone())
        .find(|action| matches!(admit(&frontier, action), Admission::Admissible(_)))
        .ok_or_else(|| {
            format!(
                "the frontier for the held case at revision {} admits no action",
                frontier.data().case_revision
            )
        })?;
    Ok((action, frontier))
}

fn current_frontier<F: GovernorFixture>(
    fixture: &F,
    governor: &F::Governor,
    case: &CaseId,
) -> Result<(), String> {
    let revision = answered("current_revision", governor.current_revision(case))?;
    let (_, frontier) = admitted_action(governor, case)
        .map_err(|failure| format!("before the case moved, {failure}"))?;
    issued_at(&frontier, case, revision, "before the case moved")?;

    fixture.advance(governor, case);

    let revision = answered("current_revision", governor.current_revision(case))?;
    let frontier = answered("frontier", governor.frontier(case))?;
    issued_at(&frontier, case, revision, "after the case moved")
}

/// `frontier` was issued for `case` at `revision`; `when` says at which point of the check it was
/// read.
fn issued_at(
    frontier: &Frontier<frontier_state::Issued>,
    case: &CaseId,
    revision: i64,
    when: &str,
) -> Result<(), String> {
    let issued = frontier.data();
    if issued.case_id != *case {
        return Err(format!(
            "{when}, the frontier for case `{}` was issued for case `{}`",
            case.0, issued.case_id.0
        ));
    }
    if issued.case_revision != revision {
        return Err(format!(
            "{when}, the current revision is {revision} but the frontier was issued for \
             revision {}",
            issued.case_revision
        ));
    }
    Ok(())
}

fn superseded_revision<F: GovernorFixture>(
    fixture: &F,
    governor: &F::Governor,
    case: &CaseId,
) -> Result<(), String> {
    let (action, frontier) = admitted_action(governor, case)?;
    let superseded = frontier.data().case_revision;
    let request = request(
        ActionRequestId(kit_uuid(0x10)),
        RunId(kit_uuid(0x11)),
        &frontier,
        ExecutorOutcomeProposedAction {
            action: action.clone(),
            arguments: ProposedActionArguments(Value::Null),
        },
    );
    let before = answered("revalidation", revalidate(governor, &request))?;
    if before != RevalidateActionRequestOutcome::Admitted {
        return Err(format!(
            "a request for `{action}` at revision {superseded}, revalidated before the case moved, \
             was not admitted: {before:?}"
        ));
    }

    // Two moves: a revision that comes back after a later change would let the request through.
    let mut reported = vec![superseded];
    for moves in 1..=2 {
        fixture.advance(governor, case);

        let current = answered("current_revision", governor.current_revision(case))?;
        if reported.contains(&current) {
            return Err(format!(
                "after the case moved {moves} time(s), the governor reports revision {current}, \
                 which it reported before (revisions so far: {reported:?})"
            ));
        }
        reported.push(current);
        let after = answered("revalidation", revalidate(governor, &request))?;
        let stale = RevalidateActionRequestOutcome::Stale {
            error: ActionRequestStale {
                expected_case_revision: superseded,
                current_case_revision: current,
            },
        };
        if after != stale {
            return Err(format!(
                "a request made at the superseded revision {superseded} was revalidated after \
                 {moves} move(s), at revision {current}, as {after:?}, not stale"
            ));
        }
    }
    Ok(())
}

fn held_case_open<G: Governor>(governor: &G, case: &CaseId) -> Result<(), String> {
    match answered("completion", governor.completion(case))? {
        CompletionDetermination::Open(_) => Ok(()),
        other => Err(format!(
            "the held case, which the fixture holds open, is answered by completion as {other:?}"
        )),
    }
}

fn unknown_case<G: Governor>(governor: &G) -> Result<(), String> {
    let unknown = CaseId(UNKNOWN.to_owned());
    let answers = [
        (
            "current_revision",
            governor.current_revision(&unknown).err(),
        ),
        ("frontier", governor.frontier(&unknown).err()),
        ("completion", governor.completion(&unknown).err()),
    ];
    let wrong: Vec<String> = answers
        .into_iter()
        .filter(|(_, error)| *error != Some(GovernorError::UnknownCase))
        .map(|(call, error)| match error {
            Some(error) => format!("{call} answered {error:?}"),
            None => format!("{call} answered Ok"),
        })
        .collect();
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "for case `{UNKNOWN}`, which it does not hold, {}; expected UnknownCase",
            wrong.join(", ")
        ))
    }
}

/// An executor's output for `case`, reported as observation `n`.
fn executor_output(n: u64, case: &CaseId) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(kit_uuid(0x100 + n)),
        source: "executor:conformance-kit".to_owned(),
        subject: case.0.clone(),
        observed_at: Timestamp(format!("2026-01-01T00:00:0{n}Z")),
        payload: Value::Object(vec![("tests".to_owned(), Value::Text("passed".to_owned()))]),
    }
}

/// Delivers `observations` to `governor` through its observation port.
fn observe_all<G: ObservationPort + ?Sized>(
    governor: &G,
    observations: &[ObservationData],
) -> Result<(), String> {
    for observation in observations {
        governor
            .observe(Observation::new(observation.clone()))
            .map_err(|error| {
                format!(
                    "the observation port refused observation `{}`: {error:?}",
                    observation.observation_id.0.0
                )
            })?;
    }
    Ok(())
}

fn observation_not_evidence<F: GovernorFixture>(
    fixture: &F,
    governor: &F::Governor,
    case: &CaseId,
) -> Result<(), String> {
    let outputs = [executor_output(1, case), executor_output(2, case)];
    observe_all(governor, &outputs)?;
    let observations = fixture.observations(governor);
    if observations != outputs {
        return Err(format!(
            "after two executor outputs were observed, the governor holds observations \
             {observations:?}, not exactly {outputs:?}"
        ));
    }
    // Every `Governor` call for the case, so a governor that promotes observations when it next
    // reports a revision, issues a frontier or determines completion is caught too.
    answered("current_revision", governor.current_revision(case))?;
    answered("frontier", governor.frontier(case))?;
    answered("completion", governor.completion(case))?;
    let evidence = fixture.evidence(governor);
    if !evidence.is_empty() {
        return Err(format!(
            "executor outputs delivered as observations became evidence: {evidence:?}"
        ));
    }
    Ok(())
}

fn evidence_observation_ids<F: GovernorFixture>(
    fixture: &F,
    governor: &F::Governor,
    case: &CaseId,
) -> Result<(), String> {
    let revision = answered("current_revision", governor.current_revision(case))?;
    // Three observed, two named: the first and the last, so the record names fewer than the
    // governor holds and not a prefix of them.
    let observed = [
        executor_output(3, case),
        executor_output(4, case),
        executor_output(5, case),
    ];
    observe_all(governor, &observed)?;
    let named: Vec<ObservationId> = [&observed[0], &observed[2]]
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect();
    let submitted = EvidenceData {
        evidence_id: EvidenceId(kit_uuid(0x200)),
        case_id: case.clone(),
        kind: "test_result".to_owned(),
        subject_revision: revision,
        producer: "verifier:conformance-kit".to_owned(),
        observation_ids: named.clone(),
        facts: Value::Object(vec![("tests.pass".to_owned(), Value::Bool(true))]),
        provenance: Value::Object(vec![(
            "source".to_owned(),
            Value::Text("conformance-kit".to_owned()),
        )]),
    };
    submit_evidence(governor, "verifier:conformance-kit", submitted.clone())
        .map_err(|error| format!("evidence for the held case was refused: {error}"))?;
    let arrived = fixture.evidence(governor);
    match arrived.as_slice() {
        [record]
            if record.evidence_id == submitted.evidence_id && record.observation_ids == named =>
        {
            Ok(())
        }
        _ => Err(format!(
            "evidence `{}` was submitted naming observations {named:?}; the governor holds \
             {arrived:?}",
            submitted.evidence_id.0.0
        )),
    }
}

/// A fixed uuid the kit uses, distinct for each `n`.
fn kit_uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}
