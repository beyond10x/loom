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
//! evidence. Neither does a write of another admitted request: the write's idempotency key must be
//! [`idempotency_key`] of the request it is recorded for.
//!
//! The evidence's facts are the `canon-evidence/1` record the governor reads (`format`, `id`,
//! `kind`, `subject`, `subject_revision`, `observed_at`) and, for a route, `target`: the queue, or
//! for an owner route the owner, the admitted arguments name (`queue` or `owner`, exactly one of
//! them), never what a model said. The governor removes `target` before Canon reads the record.
//! A route whose target cannot be read (absent, not text, empty, or named twice), or whose answer
//! from Connectors names another queue or owner, records nothing. Its provenance names the step
//! (`action`, the admitted `action_request_id` and its `run_id`) and Connectors' own record of the
//! write (`request_id`, `idempotency_key`, `source_audit`).

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

use crate::cli::idempotency_key;

/// The observation source the adapter names: what Connectors reported.
pub const SOURCE: &str = "connectors";

/// The arguments a route names its target under: the queue, or for an owner route the owner.
const ROUTE_TARGETS: &[&str] = &["queue", "owner"];

/// Turns an applied write of one action into the protocol's evidence for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvidenceAdapter {
    action: String,
    kind: String,
    subject: String,
    /// The arguments the target is named under, exactly one of them; none when the evidence names
    /// no target.
    targets: &'static [&'static str],
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
    /// The write is not the one Connectors reported for this admitted request: its idempotency
    /// key is not [`idempotency_key`] of the request.
    OtherRequest {
        /// The key of the admitted request.
        expected: String,
        /// The key the write was invoked with.
        written: String,
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
            Self::OtherRequest { expected, written } => write!(
                f,
                "the write was invoked with idempotency key `{written}`, the admitted request's \
                 is `{expected}`"
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
            targets: &[],
        }
    }

    /// `support-triage@1`'s: an applied `ticket.route` is a `routing_record` about the `ticket`,
    /// naming the route target, the `queue` or `owner` the admitted arguments name.
    pub fn support_triage_route() -> Self {
        Self {
            targets: ROUTE_TARGETS,
            ..Self::new("ticket.route", "routing_record", "ticket")
        }
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
    /// [`WriteEvidenceError::OtherAction`] for a request of another action, and
    /// [`WriteEvidenceError::OtherRequest`] for a write whose idempotency key is not
    /// [`idempotency_key`] of `request` (the write of another admitted request), each before
    /// anything is delivered; [`WriteEvidenceError::Unreadable`] for an applied route whose target
    /// the admitted arguments do not name readably (absent, not text, empty, or both `queue` and
    /// `owner`) or whose answer names another one, saying why, before anything is delivered, and
    /// for a `source_audit` that is no JSON; [`WriteEvidenceError::Observation`] and
    /// [`WriteEvidenceError::Evidence`] when the governor or `submit_evidence` refused.
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
        let expected = idempotency_key(request);
        if written.idempotency_key != expected {
            return Err(WriteEvidenceError::OtherRequest {
                expected,
                written: written.idempotency_key.clone(),
            });
        }
        if written.classification != MutationClassification::Applied {
            return Ok(None);
        }
        let target = self
            .target(&admitted.arguments.0, written.body.as_ref())
            .map_err(WriteEvidenceError::Unreadable)?;
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
        let mut facts = vec![
            ("format".to_owned(), text("canon-evidence/1")),
            ("id".to_owned(), text(&format!("write-{}", evidence.0.0))),
            ("kind".to_owned(), text(&self.kind)),
            ("subject".to_owned(), text(&self.subject)),
            ("subject_revision".to_owned(), text(subject_revision)),
            ("observed_at".to_owned(), text(&observed_at.0)),
        ];
        if let Some((_, target)) = &target {
            facts.push(("target".to_owned(), text(target)));
        }
        let facts = Value::Object(facts);
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

    /// The target the admitted `arguments` name, as `(argument, target)`: the one of
    /// [`Self::targets`] they name, as non-empty text. `answer`, Connectors' answer for the write,
    /// may name it too, under the same argument; naming another one, or the same one otherwise,
    /// differs. A `null` member names nothing. `None` for an adapter that names no target; the
    /// error says why the target cannot be read.
    fn target(
        &self,
        arguments: &Value,
        answer: Option<&loom::json::Value>,
    ) -> Result<Option<(&'static str, String)>, String> {
        if self.targets.is_empty() {
            return Ok(None);
        }
        let either = self
            .targets
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(" or ");
        let Value::Object(members) = arguments else {
            return Err(format!(
                "the admitted `{}` arguments are {}, so they name no target ({either})",
                self.action,
                arguments.describes()
            ));
        };
        let named: Vec<(&'static str, &Value)> = members
            .iter()
            .filter(|(_, value)| *value != Value::Null)
            .filter_map(|(key, value)| {
                self.targets
                    .iter()
                    .find(|name| **name == key.as_str())
                    .map(|name| (*name, value))
            })
            .collect();
        let (name, value) = match named.as_slice() {
            [] => {
                return Err(format!(
                    "the admitted `{}` arguments name no target ({either})",
                    self.action
                ));
            }
            [one] => *one,
            [first, second, ..] => {
                return Err(format!(
                    "the admitted `{}` arguments name the target twice, both `{}` and `{}`",
                    self.action, first.0, second.0
                ));
            }
        };
        let target = match value {
            Value::Text(target) if target.is_empty() => {
                return Err(format!("the admitted `{name}` target is empty"));
            }
            Value::Text(target) => target.clone(),
            other => {
                return Err(format!(
                    "the admitted `{name}` target is not text but {}",
                    other.describes()
                ));
            }
        };
        if let Some(loom::json::Value::Object(answered)) = answer {
            for (key, value) in answered {
                let Some(other) = self.targets.iter().find(|other| **other == key.as_str()) else {
                    continue;
                };
                if *value == loom::json::Value::Null {
                    continue;
                }
                let same = *other == name
                    && matches!(value, loom::json::Value::Text(answered) if *answered == target);
                if !same {
                    return Err(format!(
                        "Connectors' answer names `{other}` {}, which differs from the admitted \
                         `{name}` `{target}`",
                        match value {
                            loom::json::Value::Text(answered) => format!("`{answered}`"),
                            value => value.describes().to_owned(),
                        }
                    ));
                }
            }
        }
        Ok(Some((name, target)))
    }
}
