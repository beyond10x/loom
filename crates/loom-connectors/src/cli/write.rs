//! Governed writes through the `connectors` command line (`story:connectors-governed-write`).
//!
//! [`ConnectorsCli::invoke_write`] performs one write the runtime admitted: the
//! [`AdmittedRequest`] is the proof the step was revalidated and its authority checked, and only
//! the runtime makes one. The write is bound to what it admitted: the [`WriteOperation`]'s
//! `action` must be the admitted request's `action`, else it is refused `action-not-admitted`, and
//! the input must be the admitted `arguments` (compared as JSON values), else it is refused
//! `input-not-admitted`, both before any process starts. The person's confirmation is the proof
//! `connectors approvals issue --proof-output <path>` wrote; the [`WriteApproval`] names its path,
//! which is passed as `--approval-file` and never read, written or stored here. An empty path, one
//! holding a control character, or one that is not absolute, is refused before any process
//! starts.
//!
//! One write is two commands, each `connectors --output json [--config F] [--state-dir D]` and
//! then:
//!
//! 1. `operations describe --adapter A --operation O`, which must report the operation with a
//!    write profile ([`WRITE_PROFILES`]); any other is refused `not-a-write` and not invoked;
//! 2. `invoke N/I O --connection C --schema S --revision R --input-stdin --approval-file P
//!    --idempotency-key K`, where `N/I` is the `connector` and `instance` the describe names, as
//!    for a read (connectors `v0.60.0`, `story:connectors-cli-invoke-form`), run once, with the
//!    input document on stdin, never in argv. A describe naming no usable `connector` or
//!    `instance` fails naming the member, and nothing is invoked.
//!
//! `K` is [`idempotency_key`] of the admitted request, a digest of its case, run and request ids:
//! the same admitted request always carries the same key, and requests of two runs never share
//! one, so calling again for it can only observe the first attempt, never apply twice
//! (connectors `docs/local-gitlab-merge.md`: "Reuse the exact key for observation of this
//! invocation").
//!
//! Once the invoke started, the answer is a [`WriteResult`], never an error: the
//! `mutation.classification` Connectors states (`not_attempted`, `refused`, `applied`,
//! `unknown`), on success or inside `error.data`, with its `request_id`, `source_audit` and the
//! provider's answer. An answer that cannot be read as one, a lost or late one included, is
//! [`MutationClassification::Unknown`]: the write may have happened. Nothing is retried, an
//! unknown write least of all ("A lost response is unknown and never triggers another PUT").

use std::path::Path;

use b10x_loom_commission::model::responsibility::CommissionId;
use b10x_loom_commission::ports::effect::AdmittedRequest;
use connectors_core::v1alpha2::KEY_LIMIT;
use loom::datasource::{
    AdapterAlias, MutationClassification, OperationId, WriteApproval, WriteOperation, WriteRefusal,
    WriteResult,
};
use loom::json::{self, Value};
use sha2::{Digest as _, Sha256};

use super::{Bounded, CliError, ConnectorsCli, Described, WRITE_PROFILES, argument, text};

/// Why a governed write was not invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The write was refused before its invoke: by this client, or by Connectors at the
    /// describe.
    Refused(WriteRefusal),
    /// The describe could not be made or understood, or an argument would have read as a flag.
    Failed(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(refusal) => {
                write!(f, "refused ({}): {}", refusal.code, refusal.message)
            }
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for WriteError {}

/// The `--idempotency-key` of a write for `request`: [`key`] over its case, run and admitted
/// request id. The same admitted request always yields the same key, a resumed one included (the
/// runtime resumes it under the same three ids); requests of two runs or two cases never share
/// one.
pub fn idempotency_key(request: &AdmittedRequest) -> String {
    key(None, request)
}

/// The `idempotency_key` a [`ConnectorsInvoker`](crate::ConnectorsInvoker) sends for a write of
/// `request` admitted in `commission`: [`key`] over the commission id, then the request's case,
/// run and admitted request id. Two commissions admitting requests with the same ids never share
/// a key, and no command-line key ([`idempotency_key`]) equals one.
pub fn commission_idempotency_key(commission: &CommissionId, request: &AdmittedRequest) -> String {
    key(Some(commission), request)
}

/// Names the derivation; a key of another derivation never collides with these.
const KEY_DOMAIN: &str = "loom.connectors.idempotency-key/1";

/// `loom-action-request-<id>-<digest>`, or `loom-action-request-<digest>` when the admitted
/// request id would carry the key past Connectors' limit ([`KEY_LIMIT`] bytes). The digest is the
/// lowercase hex SHA-256 of [`KEY_DOMAIN`], then whether a commission scopes the key, then the
/// commission id when it does, the case id, the run id and the admitted request id, each preceded
/// by its length in bytes as a big-endian `u64`, so no two lists of parts share an encoding. The
/// id in the key is for the reader; the digest alone makes it unique.
fn key(commission: Option<&CommissionId>, request: &AdmittedRequest) -> String {
    let admitted = request.data();
    let mut hasher = Sha256::new();
    let mut part = |bytes: &[u8]| {
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        hasher.update(length.to_be_bytes());
        hasher.update(bytes);
    };
    part(KEY_DOMAIN.as_bytes());
    match commission {
        Some(commission) => {
            part(b"commission");
            part(commission.0.0.as_bytes());
        }
        None => part(b"no commission"),
    }
    part(admitted.case_id.0.as_bytes());
    part(admitted.run_id.0.0.as_bytes());
    part(admitted.action_request_id.0.0.as_bytes());
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let named = format!(
        "loom-action-request-{}-{digest}",
        admitted.action_request_id.0.0
    );
    if named.len() <= KEY_LIMIT {
        named
    } else {
        format!("loom-action-request-{digest}")
    }
}

impl ConnectorsCli {
    /// Writes `operation` for the admitted `request` with `input` on stdin and the proof
    /// `approval` names: one describe, then exactly one `invoke` of the `connector/instance` the
    /// describe names. The write is bound to the admitted request: `operation.action` must be its
    /// `action` and `input` its `arguments`.
    ///
    /// # Errors
    /// [`WriteError::Refused`] with code `action-not-admitted` when `operation.action` is not the
    /// admitted request's action, `input-not-admitted` when `input` is not its admitted arguments
    /// as a JSON value, `approval-missing` for an empty proof path, `approval-not-a-path` for one
    /// holding a control character or `approval-not-absolute` for a relative one, each before
    /// any process starts; with `not-a-write` when `describe` reports another profile than a
    /// write's, before the invoke; with Connectors' code when it refused the describe.
    /// [`WriteError::Failed`] when the describe failed or named no usable `connector` or
    /// `instance`, or an argument would read as a flag.
    pub fn invoke_write(
        &self,
        request: &AdmittedRequest,
        operation: &WriteOperation,
        input: &Value,
        approval: &WriteApproval,
    ) -> Result<WriteResult, WriteError> {
        let WriteOperation {
            action,
            adapter,
            connection,
            operation,
        } = operation;
        let admitted = request.data();
        if *action != admitted.action {
            return Err(refused(
                adapter,
                operation,
                "action-not-admitted",
                format!(
                    "a write of action `{action}` was asked for a request admitted for `{}`",
                    admitted.action
                ),
            ));
        }
        if !admits(&admitted.arguments.0, input) {
            return Err(refused(
                adapter,
                operation,
                "input-not-admitted",
                format!(
                    "the input of a write of adapter `{}` operation `{}` is not the arguments the \
                     runtime admitted for `{action}`",
                    adapter.0, operation.0
                ),
            ));
        }
        let proof = approval.proof_file.as_str();
        if proof.is_empty() {
            return Err(refused(
                adapter,
                operation,
                "approval-missing",
                format!(
                    "a write of adapter `{}` operation `{}` names no approval proof",
                    adapter.0, operation.0
                ),
            ));
        }
        if proof.chars().any(char::is_control) {
            return Err(refused(
                adapter,
                operation,
                "approval-not-a-path",
                format!("the approval proof path {proof:?} holds a control character"),
            ));
        }
        if !Path::new(proof).is_absolute() {
            return Err(refused(
                adapter,
                operation,
                "approval-not-absolute",
                format!("the approval proof path `{proof}` is not absolute"),
            ));
        }
        let key = idempotency_key(request);
        for (name, value) in [
            ("adapter", adapter.0.as_str()),
            ("connection", connection.0.as_str()),
            ("operation", operation.0.as_str()),
            ("idempotency key", key.as_str()),
        ] {
            argument(name, value).map_err(WriteError::from)?;
        }
        let Described {
            entity,
            profile,
            target,
        } = self.described(None, adapter, operation)?;
        if !WRITE_PROFILES.contains(&profile.as_str()) {
            return Err(refused(
                adapter,
                operation,
                "not-a-write",
                format!(
                    "adapter `{}` operation `{}` is described with the `{profile}` profile, not a \
                     write's, and is never invoked as one",
                    adapter.0, operation.0
                ),
            ));
        }
        let target = target?;
        argument("schema", &entity.schema)?;
        argument("revision", &entity.revision)?;
        let mut document = String::new();
        json::push_value(&mut document, input);
        let (mut command, timeout, _) = self.command(
            &[
                "invoke",
                &target,
                &operation.0,
                "--connection",
                &connection.0,
                "--schema",
                &entity.schema,
                "--revision",
                &entity.revision,
                "--input-stdin",
                "--approval-file",
                proof,
                "--idempotency-key",
                &key,
            ],
            true,
        )?;
        let output = match Self::bounded(&mut command, Some(&document), timeout) {
            Ok(output) => output,
            Err(Bounded::NotStarted(error)) => {
                return Err(WriteError::Failed(format!(
                    "`{}` did not start: {error}",
                    self.config.program
                )));
            }
            // Started, and its answer lost or late: the write may have happened.
            Err(Bounded::TimedOut | Bounded::Io(..)) => return Ok(unknown(key, None)),
        };
        let printed = if output.stdout.trim_ascii().is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        let Ok(answer) = json::parse(String::from_utf8_lossy(printed).trim()) else {
            return Ok(unknown(key, None));
        };
        Ok(classified(&answer, output.status.success(), key))
    }
}

impl From<CliError> for WriteError {
    fn from(error: CliError) -> Self {
        match error {
            CliError::Refused(refusal) => Self::Refused(WriteRefusal {
                adapter: refusal.adapter,
                operation: refusal.operation,
                code: refusal.code,
                message: refusal.message,
            }),
            CliError::Failed(message) => Self::Failed(message),
        }
    }
}

fn refused(
    adapter: &AdapterAlias,
    operation: &OperationId,
    code: &str,
    message: String,
) -> WriteError {
    WriteError::Refused(WriteRefusal {
        adapter: adapter.clone(),
        operation: Some(operation.clone()),
        code: code.to_owned(),
        message,
    })
}

/// Whether `input` is the JSON value of the `admitted` arguments: objects compared by their
/// members whatever their order, numbers by their spelling, everything else by equality. A
/// member named twice on either side is never the same value.
fn admits(admitted: &b10x_loom_commission::model::json::Value, input: &Value) -> bool {
    let mut document = String::new();
    b10x_loom_commission::model::json::push_value(&mut document, admitted);
    json::parse(&document).is_ok_and(|admitted| same(&admitted, input))
}

fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(l, r)| same(l, r))
        }
        (Value::Object(left), Value::Object(right)) => {
            let (left, right) = (sorted(left), sorted(right));
            left.len() == right.len()
                && left.windows(2).all(|pair| pair[0].0 != pair[1].0)
                && right.windows(2).all(|pair| pair[0].0 != pair[1].0)
                && left
                    .iter()
                    .zip(&right)
                    .all(|(l, r)| l.0 == r.0 && same(&l.1, &r.1))
        }
        _ => left == right,
    }
}

fn sorted(members: &[(String, Value)]) -> Vec<&(String, Value)> {
    let mut members: Vec<&(String, Value)> = members.iter().collect();
    members.sort_by(|a, b| a.0.cmp(&b.0));
    members
}

fn unknown(idempotency_key: String, code: Option<String>) -> WriteResult {
    WriteResult {
        classification: MutationClassification::Unknown,
        idempotency_key,
        request_id: None,
        body: None,
        source_audit: None,
        audit_ref: None,
        code,
    }
}

/// The write `answer` reports: on success its `result`, on failure its `error.data`, each with
/// `request_id`, `mutation` and `source_audit` beside it (connectors
/// `apps/connectors/src/local/operations.rs`, `project`). Without a classification Connectors
/// states, the write is unknown.
fn classified(answer: &Value, success: bool, key: String) -> WriteResult {
    let carrier = if success && answer.member("ok") == Some(&Value::Bool(true)) {
        answer.member("result")
    } else {
        answer
            .member("error")
            .and_then(|error| error.member("data"))
    };
    let code = (!success)
        .then(|| {
            carrier
                .and_then(|data| text(data, "code"))
                .or_else(|| answer.member("error").and_then(|error| text(error, "code")))
        })
        .flatten()
        .map(str::to_owned);
    let Some(carrier) = carrier else {
        return unknown(key, code);
    };
    let classification = match carrier
        .member("mutation")
        .and_then(|mutation| text(mutation, "classification"))
    {
        Some("not_attempted") => MutationClassification::NotAttempted,
        Some("refused") => MutationClassification::Refused,
        Some("applied") => MutationClassification::Applied,
        _ => MutationClassification::Unknown,
    };
    let source_audit = carrier.member("source_audit").cloned();
    let audit_ref = source_audit
        .as_ref()
        .and_then(|audit| text(audit, "audit_ref"))
        .map(str::to_owned);
    WriteResult {
        classification,
        idempotency_key: key,
        request_id: text(carrier, "request_id").map(str::to_owned),
        body: success.then(|| carrier.member("result").cloned()).flatten(),
        source_audit,
        audit_ref,
        code,
    }
}
