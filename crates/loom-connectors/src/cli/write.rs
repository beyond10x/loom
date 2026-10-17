//! Governed writes through the `connectors` command line (`story:connectors-governed-write`).
//!
//! [`ConnectorsCli::invoke_write`] performs one write the runtime admitted: the
//! [`AdmittedRequest`] is the proof the step was revalidated and its authority checked, and only
//! the runtime makes one. The person's confirmation is the proof `connectors approvals issue
//! --proof-output <path>` wrote; the [`WriteApproval`] names its path, which is passed as
//! `--approval-file` and never read, written or stored here. An empty path, or one that is not
//! absolute, is refused before any process starts.
//!
//! One write is two commands, each `connectors --output json [--config F] [--state-dir D]` and
//! then:
//!
//! 1. `operations describe --adapter A --operation O`, which must report the operation with a
//!    write profile ([`WRITE_PROFILES`]); any other is refused `not-a-write` and not invoked;
//! 2. `operations invoke --adapter A --connection C --operation O --schema S --revision R
//!    --input-stdin --approval-file P --idempotency-key K`, run once, with the input document on
//!    stdin, never in argv.
//!
//! `K` is [`idempotency_key`] of the admitted request: the same admitted request always carries
//! the same key, so calling again for it can only observe the first attempt, never apply twice
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

use b10x_loom_commission::ports::effect::AdmittedRequest;
use loom::datasource::{
    AdapterAlias, MutationClassification, OperationId, WriteApproval, WriteOperation, WriteRefusal,
    WriteResult,
};
use loom::json::{self, Value};

use super::{Bounded, CliError, ConnectorsCli, WRITE_PROFILES, argument, text};

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

/// The `--idempotency-key` of a write for `request`: its admitted request id, under a prefix
/// naming Loom. The same admitted request always yields the same key.
pub fn idempotency_key(request: &AdmittedRequest) -> String {
    format!(
        "loom-action-request-{}",
        request.data().action_request_id.0.0
    )
}

impl ConnectorsCli {
    /// Writes `operation` for the admitted `request` with `input` on stdin and the proof
    /// `approval` names: one describe, then exactly one `operations invoke`.
    ///
    /// # Errors
    /// [`WriteError::Refused`] with code `approval-missing` or `approval-not-absolute` before
    /// any process starts; with `not-a-write` when `describe` reports another profile than a
    /// write's, before the invoke; with Connectors' code when it refused the describe.
    /// [`WriteError::Failed`] when the describe failed, or an argument would read as a flag.
    pub fn invoke_write(
        &self,
        request: &AdmittedRequest,
        operation: &WriteOperation,
        input: &Value,
        approval: &WriteApproval,
    ) -> Result<WriteResult, WriteError> {
        let WriteOperation {
            adapter,
            connection,
            operation,
        } = operation;
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
        let (entity, profile) = self.described(None, adapter, operation)?;
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
        argument("schema", &entity.schema)?;
        argument("revision", &entity.revision)?;
        let mut document = String::new();
        json::push_value(&mut document, input);
        let (mut command, timeout, _) = self.command(
            &[
                "operations",
                "invoke",
                "--adapter",
                &adapter.0,
                "--connection",
                &connection.0,
                "--operation",
                &operation.0,
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
