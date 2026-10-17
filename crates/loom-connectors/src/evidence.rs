//! The host evidence adapter for governed writes (`story:connectors-governed-write`).
//!
//! A trace is not evidence (Atlas ADR 0074), and neither is what a model says it did. What a write
//! did is what Connectors reports for it: its `mutation` classification, its `request_id` and its
//! `source_audit`, as [`ConnectorsCli::invoke_write`](crate::cli::ConnectorsCli::invoke_write)
//! returns them in a [`WriteResult`]. [`WriteEvidenceAdapter`] turns an `applied` write of the one
//! action it is made for into the protocol's evidence for that action, naming the step: for
//! `support-triage@1`, a `routing_record` about the `ticket` for `ticket.route`
//! ([`WriteEvidenceAdapter::support_triage_route`]).
//!
//! [`WriteEvidenceAdapter::record`] delivers the write's observation through the
//! [`ObservationPort`] and then the evidence through Commission's [`submit_evidence`], citing that
//! observation and attributed to the trusted producer the host names. A write Connectors
//! classifies `not_attempted`, `refused` or `unknown` records nothing: no observation, no
//! evidence.
//!
//! The evidence's facts are the `canon-evidence/1` record the governor reads (`format`, `id`,
//! `kind`, `subject`, `subject_revision`, `observed_at`); its provenance names the step (`action`,
//! the admitted `action_request_id` and its `run_id`) and Connectors' own record of the write
//! (`request_id`, `idempotency_key`, `source_audit`).

use std::fmt;

use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::Timestamp;
use b10x_loom_commission::model::responsibility::{
    EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::effect::AdmittedRequest;
use b10x_loom_commission::ports::evidence::{
    EvidenceError, EvidencePort, ObservationPort, submit_evidence,
};
use loom::datasource::{MutationClassification, WriteResult};

/// The observation source the adapter names: what Connectors reported.
pub const SOURCE: &str = "connectors";

/// Turns an applied write of one action into the protocol's evidence for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvidenceAdapter {
    action: String,
    kind: String,
    subject: String,
}

/// Why a write's evidence was not recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteEvidenceError {
    /// The admitted request is for another action than the adapter's.
    OtherAction {
        /// The action the adapter records evidence for.
        expected: String,
        /// The action the request admitted.
        admitted: String,
    },
    /// Connectors' answer could not be carried into the evidence.
    Unreadable(String),
    /// The governor did not take the observation.
    Observation(GovernorError),
    /// The evidence was refused at [`submit_evidence`] or by the governor.
    Evidence(EvidenceError),
}

impl fmt::Display for WriteEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherAction { expected, admitted } => write!(
                f,
                "the admitted request is for `{admitted}`, the adapter records `{expected}`"
            ),
            Self::Unreadable(message) => f.write_str(message),
            Self::Observation(error) => {
                write!(f, "the governor did not take the observation: {error:?}")
            }
            Self::Evidence(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WriteEvidenceError {}

impl WriteEvidenceAdapter {
    /// An adapter recording an applied write of `action` as evidence of `kind` about the
    /// artifact `subject`.
    pub fn new(
        action: impl Into<String>,
        kind: impl Into<String>,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            action: action.into(),
            kind: kind.into(),
            subject: subject.into(),
        }
    }

    /// `support-triage@1`'s: an applied `ticket.route` is a `routing_record` about the `ticket`.
    pub fn support_triage_route() -> Self {
        Self::new("ticket.route", "routing_record", "ticket")
    }

    /// The action whose writes this adapter records.
    pub fn action(&self) -> &str {
        &self.action
    }

    /// The evidence kind it records.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Records `written`, the write Connectors reported for the admitted `request`: when it is
    /// `applied`, delivers its observation `observation` at the trusted time `observed_at` and
    /// then the evidence `evidence` about the subject at `subject_revision` (the artifact's
    /// revision, as the case holds it), through [`submit_evidence`] attributed to `producer`.
    /// Answers the evidence id recorded, or `None` for a write that was not applied, which
    /// delivers nothing.
    ///
    /// # Errors
    /// [`WriteEvidenceError::OtherAction`] for a request of another action, before anything is
    /// delivered; [`WriteEvidenceError::Observation`] and [`WriteEvidenceError::Evidence`] when the
    /// governor or `submit_evidence` refused.
    #[allow(clippy::too_many_arguments)]
    pub fn record<P>(
        &self,
        port: &P,
        producer: &str,
        request: &AdmittedRequest,
        written: &WriteResult,
        observation: ObservationId,
        observed_at: Timestamp,
        evidence: EvidenceId,
        subject_revision: &str,
    ) -> Result<Option<EvidenceId>, WriteEvidenceError>
    where
        P: ObservationPort + EvidencePort + ?Sized,
    {
        let admitted = request.data();
        if admitted.action != self.action {
            return Err(WriteEvidenceError::OtherAction {
                expected: self.action.clone(),
                admitted: admitted.action.clone(),
            });
        }
        if written.classification != MutationClassification::Applied {
            return Ok(None);
        }
        let text = |value: &str| Value::Text(value.to_owned());
        let optional = |value: &Option<String>| value.as_deref().map_or(Value::Null, text);
        let source_audit = match &written.source_audit {
            Some(audit) => {
                let mut document = String::new();
                loom::json::push_value(&mut document, audit);
                json::parse(&document).map_err(|error| {
                    WriteEvidenceError::Unreadable(format!(
                        "the write's source_audit is no JSON: {error:?}"
                    ))
                })?
            }
            None => Value::Null,
        };
        let reported = vec![
            ("action".to_owned(), text(&admitted.action)),
            (
                "action_request_id".to_owned(),
                text(&admitted.action_request_id.0.0),
            ),
            ("run_id".to_owned(), text(&admitted.run_id.0.0)),
            ("classification".to_owned(), text("applied")),
            ("request_id".to_owned(), optional(&written.request_id)),
            ("idempotency_key".to_owned(), text(&written.idempotency_key)),
            ("source_audit".to_owned(), source_audit),
        ];
        port.observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: SOURCE.to_owned(),
            subject: self.subject.clone(),
            observed_at: observed_at.clone(),
            payload: Value::Object(reported.clone()),
        }))
        .map_err(WriteEvidenceError::Observation)?;
        let facts = Value::Object(vec![
            ("format".to_owned(), text("canon-evidence/1")),
            ("id".to_owned(), text(&format!("write-{}", evidence.0.0))),
            ("kind".to_owned(), text(&self.kind)),
            ("subject".to_owned(), text(&self.subject)),
            ("subject_revision".to_owned(), text(subject_revision)),
            ("observed_at".to_owned(), text(&observed_at.0)),
        ]);
        let mut provenance = vec![("source".to_owned(), text(SOURCE))];
        provenance.extend(reported);
        submit_evidence(
            port,
            producer,
            EvidenceData {
                evidence_id: evidence.clone(),
                case_id: admitted.case_id.clone(),
                kind: self.kind.clone(),
                subject_revision: admitted.expected_case_revision,
                producer: producer.to_owned(),
                observation_ids: vec![observation],
                facts,
                provenance: Value::Object(provenance),
            },
        )
        .map_err(WriteEvidenceError::Evidence)?;
        Ok(Some(evidence))
    }
}
