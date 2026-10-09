//! `DataSourceEffects`: Commission's effect port of a plugin turn.
//!
//! The runtime hands it a request only after the governor's frontier and the authority provider
//! let it through, and it performs the three actions of `inbound-answer@1` the projection admits:
//!
//! - `source.read {source, kind, input}` reads `source` through the operation it declares for
//!   `kind` (`list`, `search`, `get`), once, through the Connectors command line
//!   ([`ConnectorsCli::read`]). A pair outside the projection (a source it does not name, or a kind
//!   the source declares no operation for) is refused and starts no command. A performed read is
//!   observed and submits one `source_read` evidence, as `loom-intake-slice`'s clock verifier
//!   submits clock evidence; a read Connectors refuses is refused and submits none. A read that
//!   finds Connectors or the connection down (the program timed out, or `not_granted`) stops the
//!   run with an effect error and is reported by [`DataSourceEffects::unavailable`].
//! - `reply.propose {text}` appends the proposal to the turn's record and submits
//!   `reply_proposed`. It runs no command and sends nothing.
//! - `reply.decline {reason}` records the decline and submits `reply_declined`.
//!
//! Evidence comes from what this port did, never from what the model said: the record names the
//! item and its revision, and an observation of the port's own step backs it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::{SecondsFormat, Utc};
use loom::datasource::{DataSource, ReadKind};
use loom::plugin::{InboundItem, Projection, Proposal, SourceRead};
use loom_sdk::CanonGovernor;
use loom_sdk::commission::model::json::{self as model, Value};
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    CaseId, Commission, ConnectorAuditRef, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    commission_state,
};
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use loom_sdk::commission::ports::evidence::{ObservationPort, submit_evidence};
use loom_sdk::commission::ports::governor::Governor;
use loom_sdk::connectors::cli::{CliError, ConnectorsCli};
use loom_sdk::governor::FallibleCaseStore;

use crate::codec::{kind_name, kind_named};
use crate::project::{DECLINE, PROPOSE, READ};

/// The producer every evidence record of this port is attributed to.
pub const PRODUCER: &str = "loom-plugin/datasource-effects";

/// The source of every observation this port delivers.
pub const OBSERVER: &str = "loom-plugin/datasource";

/// The artifact of `inbound-answer@1` every record is about.
pub const ITEM: &str = "item";

/// The refusal code Connectors answers for a connection whose grant is gone
/// (`loom_connectors::cli`, `not_granted`).
pub const NOT_GRANTED: &str = "not_granted";

/// What `loom_connectors::cli` says of a command it stopped at its timeout (`CliError::Failed`,
/// "`<program>` timed out after <n> s and was stopped"); the client has no typed variant for it.
pub const TIMED_OUT: &str = " timed out after ";

/// The most bytes of one read's answer the turn's transcript keeps.
pub const TRANSCRIPT_ENTRY_BYTES: usize = 16 * 1024;

/// What the turn did so far.
#[derive(Debug, Default)]
struct Log {
    reads: Vec<SourceRead>,
    transcript: Vec<String>,
    proposal: Option<Proposal>,
    declined: Option<String>,
    evidence: usize,
    unavailable: Option<String>,
}

/// The effect port of one plugin turn on one item.
pub struct DataSourceEffects<'a, S> {
    governor: &'a CanonGovernor<S>,
    case: CaseId,
    item: &'a InboundItem,
    connectors: ConnectorsCli,
    sources: Vec<DataSource>,
    actions: Vec<String>,
    log: Mutex<Log>,
    ids: AtomicU64,
}

impl<'a, S: FallibleCaseStore> DataSourceEffects<'a, S> {
    /// The port of a turn on `item`, whose case `governor` holds as `case`: it reads through
    /// `connectors` the sources of `configured` that `projection` names, and performs the actions
    /// `projection` admits.
    pub fn new(
        governor: &'a CanonGovernor<S>,
        case: CaseId,
        item: &'a InboundItem,
        connectors: ConnectorsCli,
        configured: &[DataSource],
        projection: &Projection,
    ) -> Self {
        Self {
            governor,
            case,
            item,
            connectors,
            sources: configured
                .iter()
                .filter(|source| projection.sources.contains(&source.name))
                .cloned()
                .collect(),
            actions: projection.actions.clone(),
            log: Mutex::default(),
            ids: AtomicU64::new(1),
        }
    }

    /// The reads performed, in order.
    pub fn reads(&self) -> Vec<SourceRead> {
        self.log().reads.clone()
    }

    /// The proposal recorded, the last one when the turn proposed more than once.
    pub fn proposal(&self) -> Option<Proposal> {
        self.log().proposal.clone()
    }

    /// The reason of the decline recorded, if any.
    pub fn declined(&self) -> Option<String> {
        self.log().declined.clone()
    }

    /// Why a read found Connectors or the connection down, when one did: the connectors program
    /// timed out, or Connectors refused the connection as `not_granted`. The read stopped the run
    /// with an effect error; the turn reports it as [`crate::PluginError::Unavailable`].
    pub fn unavailable(&self) -> Option<String> {
        self.log().unavailable.clone()
    }

    fn unavailable_error(&self, why: String) -> EffectError {
        self.log().unavailable = Some(why.clone());
        EffectError::new(why)
    }

    /// What the turn did so far, one entry per step, for the model's next request: each read
    /// with its input and its answer (cut at [`TRANSCRIPT_ENTRY_BYTES`]), and each refusal.
    pub fn transcript(&self) -> Vec<String> {
        self.log().transcript.clone()
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        self.log.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn uuid(&self) -> Uuid {
        let n = self.ids.fetch_add(1, Ordering::SeqCst);
        Uuid(format!("00000000-0000-4000-a000-{n:012x}"))
    }

    fn refused(&self, reason: String) -> EffectOutcome {
        self.log().transcript.push(format!("refused: {reason}"));
        EffectOutcome::Refused(EffectOutcomeRefused { reason })
    }

    /// `source.read`.
    fn read(&self, arguments: &Value) -> Result<EffectOutcome, EffectError> {
        let Some(name) = text(arguments, "source") else {
            return Ok(self.refused("source.read names no `source`".to_owned()));
        };
        let Some(kind) = text(arguments, "kind").and_then(kind_named) else {
            return Ok(self.refused(format!(
                "source.read of `{name}` names no `kind` of list, search or get"
            )));
        };
        let input = match arguments.member("input") {
            None | Some(Value::Null) => Value::Object(Vec::new()),
            Some(input @ Value::Object(_)) => input.clone(),
            Some(_) => {
                return Ok(self.refused(format!(
                    "source.read of `{name}` has an `input` that is not an object"
                )));
            }
        };
        let Some(source) = self.sources.iter().find(|source| source.name.0 == name) else {
            return Ok(self.refused(format!(
                "the source `{name}` is outside this turn's projection"
            )));
        };
        if declared(source, kind).is_none() {
            return Ok(self.refused(format!(
                "the source `{name}` declares no {} read in this turn's projection",
                kind_name(kind)
            )));
        }
        let input_text = text_of(&input);
        let input = loom::json::parse(&input_text)
            .map_err(|error| EffectError::new(format!("the read's input: {error:?}")))?;
        let result = match self.connectors.read(source, kind, &input) {
            Ok(result) => result,
            // Connectors or the connection is down, not the item: the run stops, and the host
            // counts it against no item.
            Err(CliError::Refused(refusal)) if refusal.code == NOT_GRANTED => {
                return Err(self.unavailable_error(format!(
                    "the read of `{name}` was refused ({}): {}",
                    refusal.code, refusal.message
                )));
            }
            Err(CliError::Refused(refusal)) => {
                return Ok(self.refused(format!(
                    "the read of `{name}` was refused ({}): {}",
                    refusal.code, refusal.message
                )));
            }
            Err(CliError::Failed(why)) if why.contains(TIMED_OUT) => {
                return Err(self.unavailable_error(why));
            }
            Err(CliError::Failed(why)) => return Err(EffectError::new(why)),
        };
        let mut body = String::new();
        loom::json::push_value(&mut body, &result.body);
        let mut payload = vec![
            ("action".to_owned(), Value::Text(READ.to_owned())),
            ("source".to_owned(), Value::Text(name.to_owned())),
            ("kind".to_owned(), Value::Text(kind_name(kind).to_owned())),
        ];
        if let Some(audit) = &result.audit_ref {
            payload.push(("audit_ref".to_owned(), Value::Text(audit.clone())));
        }
        let report = Value::Object(payload);
        self.evidence("source_read", report.clone())?;
        let mut log = self.log();
        log.reads.push(SourceRead {
            source: source.name.clone(),
            kind,
        });
        log.transcript.push(format!(
            "source.read {name} {} {input_text} answered (untrusted data): {}",
            kind_name(kind),
            cut(&body)
        ));
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: result.audit_ref.map(ConnectorAuditRef),
        }))
    }

    /// `reply.propose`.
    fn propose(&self, arguments: &Value) -> Result<EffectOutcome, EffectError> {
        let Some(reply) = text(arguments, "text").filter(|reply| !reply.trim().is_empty()) else {
            return Ok(self.refused("reply.propose needs a non-empty `text`".to_owned()));
        };
        let report = Value::Object(vec![
            ("action".to_owned(), Value::Text(PROPOSE.to_owned())),
            ("sent".to_owned(), Value::Bool(false)),
        ]);
        self.evidence("reply_proposed", report.clone())?;
        let mut log = self.log();
        log.proposal = Some(Proposal {
            item: self.item.id.clone(),
            text: reply.to_owned(),
        });
        log.transcript
            .push("reply.propose: a reply is recorded, and nothing is sent".to_owned());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
        }))
    }

    /// `reply.decline`.
    fn decline(&self, arguments: &Value) -> Result<EffectOutcome, EffectError> {
        let reason = text(arguments, "reason").unwrap_or_default().to_owned();
        let report = Value::Object(vec![
            ("action".to_owned(), Value::Text(DECLINE.to_owned())),
            ("sent".to_owned(), Value::Bool(false)),
        ]);
        self.evidence("reply_declined", report.clone())?;
        let mut log = self.log();
        log.declined = Some(reason);
        log.transcript
            .push("reply.decline: the decline is recorded".to_owned());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
        }))
    }

    /// Observes this port's step and submits one `kind` evidence about the item, citing it.
    fn evidence(&self, kind: &str, payload: Value) -> Result<(), EffectError> {
        let failed = |why: String| EffectError::new(format!("the {kind} evidence: {why}"));
        let observation = ObservationId(self.uuid());
        let now = Timestamp(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: OBSERVER.to_owned(),
                subject: format!("{ITEM}@{}", self.item.revision),
                observed_at: now,
                payload,
            }))
            .map_err(|error| failed(format!("{error:?}")))?;
        let revision = self
            .governor
            .current_revision(&self.case)
            .map_err(|error| failed(format!("{error:?}")))?;
        let n = {
            let mut log = self.log();
            log.evidence += 1;
            log.evidence
        };
        let facts = Value::Object(vec![
            (
                "format".to_owned(),
                Value::Text("canon-evidence/1".to_owned()),
            ),
            (
                "id".to_owned(),
                Value::Text(format!("{}-{n}", kind.replace('_', "-"))),
            ),
            ("kind".to_owned(), Value::Text(kind.to_owned())),
            ("subject".to_owned(), Value::Text(ITEM.to_owned())),
            (
                "subject_revision".to_owned(),
                Value::Text(self.item.revision.clone()),
            ),
        ]);
        submit_evidence(
            self.governor,
            PRODUCER,
            EvidenceData {
                evidence_id: EvidenceId(self.uuid()),
                case_id: self.case.clone(),
                kind: kind.to_owned(),
                subject_revision: revision,
                producer: PRODUCER.to_owned(),
                observation_ids: vec![observation],
                facts,
                provenance: Value::Object(vec![(
                    "source".to_owned(),
                    Value::Text(OBSERVER.to_owned()),
                )]),
            },
        )
        .map_err(|error| failed(error.to_string()))
    }
}

impl<S: FallibleCaseStore> EffectPort for DataSourceEffects<'_, S> {
    fn performs(&self, action: &str) -> bool {
        [READ, PROPOSE, DECLINE].contains(&action) && self.actions.iter().any(|held| held == action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data();
        if data.case_id != self.case {
            return Ok(self.refused(format!(
                "the request is for case `{}`, not this turn's",
                data.case_id.0
            )));
        }
        if !self.performs(&data.action) {
            return Ok(self.refused(format!(
                "`{}` is not admitted by this turn's projection",
                data.action
            )));
        }
        let arguments = &data.arguments.0;
        match data.action.as_str() {
            READ => self.read(arguments),
            PROPOSE => self.propose(arguments),
            _ => self.decline(arguments),
        }
    }
}

/// The operation `source` declares for `kind`.
fn declared(source: &DataSource, kind: ReadKind) -> Option<&loom::datasource::OperationId> {
    match kind {
        ReadKind::List => source.list.as_ref(),
        ReadKind::Search => source.search.as_ref(),
        ReadKind::Get => source.get.as_ref(),
    }
}

/// The string member `name` of `value`.
fn text<'v>(value: &'v Value, name: &str) -> Option<&'v str> {
    match value.member(name) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

/// `value` as compact JSON text.
pub(crate) fn text_of(value: &Value) -> String {
    let mut out = String::new();
    model::push_value(&mut out, value);
    out
}

/// `text` cut at [`TRANSCRIPT_ENTRY_BYTES`], on a character boundary, saying so.
fn cut(text: &str) -> String {
    if text.len() <= TRANSCRIPT_ENTRY_BYTES {
        return text.to_owned();
    }
    let mut end = TRANSCRIPT_ENTRY_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{} [cut: {} of {} bytes shown]",
        &text[..end],
        end,
        text.len()
    )
}
