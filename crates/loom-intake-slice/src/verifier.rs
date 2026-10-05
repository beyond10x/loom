//! The test-result verifier: the one place the slice turns something into evidence (story
//! `selector-executor`; Atlas ADR 0074).
//!
//! # Trust boundary
//!
//! The verifier is the trusted integration, never the model. It reads only a [`TestRun`], which only
//! the [`crate::executor`] makes, from the test command it ran itself, and from it only:
//!
//! - the command's real exit status: exit code `0` is `pass`; any other exit code, or an end by a
//!   signal, is `fail`;
//! - the `HEAD` the command ran on, as the `implementation` revision the result is about;
//! - the case revision when it ran, and the observation the executor delivered for the run.
//!
//! Nothing a model says, in text or in arguments, reaches it. Every other report is no test result
//! (`Ok(None)`), and a run on a work tree with uncommitted changes is about no revision
//! ([`VerifyError::NoRevision`]): neither yields evidence. A run yields at most one record: a run
//! whose observation this verifier already cited, or any record the governor holds for the case
//! cites, is refused with [`VerifyError::AlreadyVerified`]. A timed-out run has no exit code and is
//! `fail`.
//!
//! The evidence is one `canon-evidence/1` record of kind `test_result` about `implementation`,
//! carried in its facts, citing the run's observation and submitted through Commission's
//! `submit_evidence` under the verifier's producer. The governor decides whether it applies.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::responsibility::GovernorError;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, ObservationId,
};
use b10x_loom_commission::ports::evidence::{EvidenceError, submit_evidence};
use loom_governor::{CanonGovernor, CaseStore};

use crate::case::IMPLEMENTATION;
use crate::executor::{Report, TEST_RUN_SOURCE, TestRun, fresh_uuid};

/// The evidence kind the verifier submits.
pub const TEST_RESULT: &str = "test_result";

/// Why a test run yielded no evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The run belongs to another case than the verifier's.
    OtherCase { case: CaseId },
    /// The work tree had uncommitted changes, so the run is about no committed revision.
    NoRevision,
    /// Evidence citing the run's observation was already submitted.
    AlreadyVerified { observation: ObservationId },
    /// `submit_evidence` or the governor refused the record.
    Evidence(EvidenceError),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherCase { case } => write!(f, "the test run belongs to case `{}`", case.0),
            Self::NoRevision => {
                f.write_str("the tests ran on uncommitted changes, so they are about no revision")
            }
            Self::AlreadyVerified { observation } => write!(
                f,
                "evidence citing the test run's observation `{}` was already submitted",
                observation.0.0
            ),
            Self::Evidence(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Submits `test_result` evidence for test runs of one case of `governor`.
pub struct TestResultVerifier<'g, S> {
    governor: &'g CanonGovernor<S>,
    case: CaseId,
    producer: String,
    cited: Mutex<HashSet<String>>,
}

impl<'g, S: CaseStore> TestResultVerifier<'g, S> {
    /// A verifier for `case` that attributes its evidence to `producer`.
    pub fn new(governor: &'g CanonGovernor<S>, case: CaseId, producer: impl Into<String>) -> Self {
        Self {
            governor,
            case,
            producer: producer.into(),
            cited: Mutex::default(),
        }
    }

    /// The id of the evidence submitted for `report`; `None` when it is no test run.
    pub fn verify(&self, report: &Report) -> Result<Option<EvidenceId>, VerifyError> {
        let Report::TestsRun(run) = report else {
            return Ok(None);
        };
        if run.case() != &self.case {
            return Err(VerifyError::OtherCase {
                case: run.case().clone(),
            });
        }
        let observation = run.observation();
        let mut cited = self.cited.lock().unwrap_or_else(PoisonError::into_inner);
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
        let implementation = run.implementation().ok_or(VerifyError::NoRevision)?;
        let evidence_id = EvidenceId(fresh_uuid("evidence", &self.case.0));
        let evidence = EvidenceData {
            evidence_id: evidence_id.clone(),
            case_id: self.case.clone(),
            kind: TEST_RESULT.to_owned(),
            subject_revision: run.case_revision(),
            producer: self.producer.clone(),
            observation_ids: vec![run.observation().clone()],
            facts: record(&evidence_id, implementation, result(run)),
            provenance: Value::Object(vec![
                ("source".to_owned(), Value::Text(TEST_RUN_SOURCE.to_owned())),
                (
                    "exit_code".to_owned(),
                    run.exit_code()
                        .map_or(Value::Null, |code| Value::Number(code.to_string())),
                ),
            ]),
        };
        submit_evidence(self.governor, &self.producer, evidence).map_err(VerifyError::Evidence)?;
        cited.insert(observation.0.0.clone());
        Ok(Some(evidence_id))
    }
}

fn governor_refused(error: GovernorError) -> VerifyError {
    VerifyError::Evidence(EvidenceError::Governor(error))
}

/// `pass` for exit code 0, `fail` for anything else.
fn result(run: &TestRun) -> &'static str {
    if run.exit_code() == Some(0) {
        "pass"
    } else {
        "fail"
    }
}

/// The `canon-evidence/1` record of a test result about `implementation`.
fn record(evidence: &EvidenceId, implementation: &str, result: &str) -> Value {
    let text = |value: &str| Value::Text(value.to_owned());
    Value::Object(vec![
        ("format".to_owned(), text("canon-evidence/1")),
        (
            "id".to_owned(),
            text(&format!("test-result-{}", evidence.0.0)),
        ),
        ("kind".to_owned(), text(TEST_RESULT)),
        ("subject".to_owned(), text(IMPLEMENTATION)),
        ("subject_revision".to_owned(), text(implementation)),
        ("result".to_owned(), text(result)),
    ])
}
