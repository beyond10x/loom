//! The test-result verifier: the one place the slice turns something into evidence (story
//! `selector-executor`, story `change-evidence-order`; Atlas ADR 0074). What it submits is
//! `intake.routing.SliceEvidence` ([`SliceEvidenceData`]).
//!
//! # Trust boundary
//!
//! The verifier is the trusted integration, never the model. It reads only a [`TestRun`] or a
//! [`Change`], which only the [`crate::executor`] makes, from the test command it ran itself or the
//! edit it committed, and from them only:
//!
//! - a run's real exit status: exit code `0` is `pass`; any other exit code, or an end by a
//!   signal, is `fail`;
//! - the `HEAD` the command ran on, or the new `HEAD` the edit committed, as the `implementation`
//!   revision the record is about;
//! - the case revision, the observation the executor delivered and the host clock's instant of
//!   that observation (`observed_at`);
//! - the regression-test command's runs that followed a suite run ([`TestRun::regressions`]),
//!   which the executor makes only from the host's command and the test paths its own
//!   `tests.write` actions committed;
//!
//! and, from the governor, the case's `change_kind` input, which the host gave when the case
//! opened. Nothing a model says, in text or in arguments, reaches it.
//!
//! # What it submits
//!
//! - For a test run: one `test_result` from the suite, citing its observation; and, on a case
//!   whose `change_kind` is `bugfix`, one `regression_test_result` per regression-test run that
//!   followed it, from that run's exit status alone and citing that run's own observation. The
//!   suite's result is never a `regression_test_result`: without a regression-test command
//!   ([`crate::executor::TestCommand::with_regression`]) no run yields one. A regression run on
//!   uncommitted changes yields none.
//! - For an `implementation.edit` that committed: one `implementation_change` about the new
//!   `HEAD`, with no `result`, citing the edit's observation.
//!
//! Every other report is no evidence (`Ok(None)`), and a run on a work tree with uncommitted changes
//! is about no revision ([`VerifyError::NoRevision`]) and yields none. An observation is verified
//! once: a report whose observation this verifier already cited, or any record the governor holds
//! for the case cites, is refused with [`VerifyError::AlreadyVerified`] before anything is
//! submitted, so one observation yields the records above and no other. A timed-out run has no
//! exit code and is `fail`.
//!
//! Each record is one `canon-evidence/1` record about `implementation`, carried in the evidence's
//! facts with its `observed_at` and the verifier's producer, citing the observation and submitted
//! through Commission's `submit_evidence` under that producer, which is the one the governor gives
//! Canon. The governor decides whether it applies. [`TestResultVerifier::verify`] returns the id of
//! the report's first record: the `test_result` of a run, the `implementation_change` of an edit.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::GovernorError;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, ObservationId,
};
use b10x_loom_commission::ports::evidence::{EvidenceError, submit_evidence};
pub use intake_model::routing::{SliceEvidenceData, SliceEvidenceKind, SliceEvidenceResult};
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore, NoEvaluationTime};

use crate::case::IMPLEMENTATION;
use crate::executor::{
    Change, EDIT_SOURCE, REGRESSION_RUN_SOURCE, Report, TEST_RUN_SOURCE, TestRun, fresh_uuid,
};

/// The evidence kind of a test run.
pub const TEST_RESULT: &str = "test_result";
/// The evidence kind of a bugfix's test run after its first `tests.write` that committed.
pub const REGRESSION_TEST_RESULT: &str = "regression_test_result";
/// The evidence kind of an `implementation.edit` that committed.
pub const IMPLEMENTATION_CHANGE: &str = "implementation_change";

/// The case input that says what a software change is, and its value for a bugfix.
const CHANGE_KIND: &str = "change_kind";
const BUGFIX: &str = "bugfix";

/// `kind` as `software.change/1` names it.
pub fn kind_name(kind: SliceEvidenceKind) -> &'static str {
    match kind {
        SliceEvidenceKind::TestResult => TEST_RESULT,
        SliceEvidenceKind::RegressionTestResult => REGRESSION_TEST_RESULT,
        SliceEvidenceKind::ImplementationChange => IMPLEMENTATION_CHANGE,
    }
}

/// `result` as a `canon-evidence/1` record writes it.
pub fn result_name(result: SliceEvidenceResult) -> &'static str {
    match result {
        SliceEvidenceResult::Pass => "pass",
        SliceEvidenceResult::Fail => "fail",
    }
}

/// Why a report yielded no evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The report belongs to another case than the verifier's.
    OtherCase { case: CaseId },
    /// The work tree had uncommitted changes, so the run is about no committed revision.
    NoRevision,
    /// Evidence citing the report's observation was already submitted.
    AlreadyVerified { observation: ObservationId },
    /// `submit_evidence` or the governor refused the record.
    Evidence(EvidenceError),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherCase { case } => write!(f, "the report belongs to case `{}`", case.0),
            Self::NoRevision => {
                f.write_str("the tests ran on uncommitted changes, so they are about no revision")
            }
            Self::AlreadyVerified { observation } => write!(
                f,
                "evidence citing the observation `{}` was already submitted",
                observation.0.0
            ),
            Self::Evidence(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Submits the slice's evidence for test runs and edits of one case of `governor`.
pub struct TestResultVerifier<'g, S, T = NoEvaluationTime> {
    governor: &'g CanonGovernor<S, T>,
    case: CaseId,
    producer: String,
    cited: Mutex<HashSet<String>>,
}

impl<'g, S: FallibleCaseStore, T: EvaluationTime> TestResultVerifier<'g, S, T> {
    /// A verifier for `case` that attributes its evidence to `producer`.
    pub fn new(
        governor: &'g CanonGovernor<S, T>,
        case: CaseId,
        producer: impl Into<String>,
    ) -> Self {
        Self {
            governor,
            case,
            producer: producer.into(),
            cited: Mutex::default(),
        }
    }

    /// The id of the first record submitted for `report`; `None` when it is neither a test run nor
    /// an `implementation.edit` that committed. See the module documentation for every record.
    pub fn verify(&self, report: &Report) -> Result<Option<EvidenceId>, VerifyError> {
        match report {
            Report::TestsRun(run) => self.verify_run(run).map(Some),
            Report::Edited {
                change: Some(change),
                ..
            } => self.verify_change(change).map(Some),
            _ => Ok(None),
        }
    }

    fn verify_run(&self, run: &TestRun) -> Result<EvidenceId, VerifyError> {
        let first = self.submit_run(run, SliceEvidenceKind::TestResult, TEST_RUN_SOURCE)?;
        if !run.regressions().is_empty() && self.bugfix()? {
            for regression in run.regressions() {
                // A regression run on uncommitted changes is about no revision: no record.
                if regression.run().implementation().is_some() {
                    self.submit_run(
                        regression.run(),
                        SliceEvidenceKind::RegressionTestResult,
                        REGRESSION_RUN_SOURCE,
                    )?;
                }
            }
        }
        Ok(first)
    }

    /// Submits one record of `kind` from `run`'s exit status, citing its observation.
    fn submit_run(
        &self,
        run: &TestRun,
        kind: SliceEvidenceKind,
        source: &str,
    ) -> Result<EvidenceId, VerifyError> {
        let mut cited = self.uncited(run.case(), run.observation())?;
        let implementation = run.implementation().ok_or(VerifyError::NoRevision)?;
        let result = if run.exit_code() == Some(0) {
            SliceEvidenceResult::Pass
        } else {
            SliceEvidenceResult::Fail
        };
        let provenance = Value::Object(vec![
            ("source".to_owned(), Value::Text(source.to_owned())),
            (
                "exit_code".to_owned(),
                run.exit_code()
                    .map_or(Value::Null, |code| Value::Number(code.to_string())),
            ),
        ]);
        let evidence = self.evidence(
            kind,
            implementation,
            Some(result),
            run.observation(),
            run.observed_at(),
        );
        let id = self.submit(&evidence, run.case_revision(), provenance)?;
        cited.insert(run.observation().0.0.clone());
        Ok(id)
    }

    fn verify_change(&self, change: &Change) -> Result<EvidenceId, VerifyError> {
        let mut cited = self.uncited(change.case(), change.observation())?;
        let evidence = self.evidence(
            SliceEvidenceKind::ImplementationChange,
            change.revision(),
            None,
            change.observation(),
            change.observed_at(),
        );
        let provenance = Value::Object(vec![(
            "source".to_owned(),
            Value::Text(EDIT_SOURCE.to_owned()),
        )]);
        let id = self.submit(&evidence, change.case_revision(), provenance)?;
        cited.insert(change.observation().0.0.clone());
        Ok(id)
    }

    /// The cited set, locked, once `observation` of `case` is known to be cited by no record.
    fn uncited(
        &self,
        case: &CaseId,
        observation: &ObservationId,
    ) -> Result<std::sync::MutexGuard<'_, HashSet<String>>, VerifyError> {
        if case != &self.case {
            return Err(VerifyError::OtherCase { case: case.clone() });
        }
        let cited = self.cited.lock().unwrap_or_else(PoisonError::into_inner);
        let held = self
            .governor
            .evidence(&self.case)
            .map_err(governor_refused)?;
        if cited.contains(&observation.0.0)
            || held
                .iter()
                .any(|record| record.observation_ids.contains(observation))
        {
            return Err(VerifyError::AlreadyVerified {
                observation: observation.clone(),
            });
        }
        Ok(cited)
    }

    /// Whether the case is a bugfix, as the host gave its `change_kind` when it opened.
    fn bugfix(&self) -> Result<bool, VerifyError> {
        let inputs = self.governor.inputs(&self.case).map_err(governor_refused)?;
        Ok(inputs.get(CHANGE_KIND) == Some(BUGFIX))
    }

    fn evidence(
        &self,
        kind: SliceEvidenceKind,
        implementation: &str,
        result: Option<SliceEvidenceResult>,
        observation: &ObservationId,
        observed_at: &b10x_loom_commission::model::primitives::Timestamp,
    ) -> SliceEvidenceData {
        SliceEvidenceData {
            evidence_id: intake_model::primitives::Uuid(fresh_uuid("evidence", &self.case.0).0),
            kind,
            subject_revision: implementation.to_owned(),
            result,
            observation_id: intake_model::primitives::Uuid(observation.0.0.clone()),
            observed_at: intake_model::primitives::Timestamp(observed_at.0.clone()),
            producer: self.producer.clone(),
        }
    }

    /// Submits `evidence` through `submit_evidence`, at `case_revision`.
    fn submit(
        &self,
        evidence: &SliceEvidenceData,
        case_revision: i64,
        provenance: Value,
    ) -> Result<EvidenceId, VerifyError> {
        let evidence_id = EvidenceId(Uuid(evidence.evidence_id.0.clone()));
        let data = EvidenceData {
            evidence_id: evidence_id.clone(),
            case_id: self.case.clone(),
            kind: kind_name(evidence.kind).to_owned(),
            subject_revision: case_revision,
            producer: evidence.producer.clone(),
            observation_ids: vec![ObservationId(Uuid(evidence.observation_id.0.clone()))],
            facts: record(evidence),
            provenance,
        };
        submit_evidence(self.governor, &self.producer, data).map_err(VerifyError::Evidence)?;
        Ok(evidence_id)
    }
}

fn governor_refused(error: GovernorError) -> VerifyError {
    VerifyError::Evidence(EvidenceError::Governor(error))
}

/// The `canon-evidence/1` record of `evidence`, about `implementation`.
fn record(evidence: &SliceEvidenceData) -> Value {
    let text = |value: &str| Value::Text(value.to_owned());
    let kind = kind_name(evidence.kind);
    let mut members = vec![
        ("format".to_owned(), text("canon-evidence/1")),
        (
            "id".to_owned(),
            text(&format!(
                "{}-{}",
                kind.replace('_', "-"),
                evidence.evidence_id.0
            )),
        ),
        ("kind".to_owned(), text(kind)),
        ("subject".to_owned(), text(IMPLEMENTATION)),
        (
            "subject_revision".to_owned(),
            text(&evidence.subject_revision),
        ),
    ];
    if let Some(result) = evidence.result {
        members.push(("result".to_owned(), text(result_name(result))));
    }
    members.push(("observed_at".to_owned(), text(&evidence.observed_at.0)));
    members.push(("producer".to_owned(), text(&evidence.producer)));
    Value::Object(members)
}
