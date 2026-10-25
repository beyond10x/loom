// generated from loom v1
// model digest 57efaae179a33626d37c8c198c74946595a31ed6f2ea28fcc78619f3cd766242
// contract digest 2ae84016be0474df15e9872f7fcbe97c0bc899337811922b75b31e7cda8dc5fb
// do not edit: regenerate with `ess synthesize --layout crate`

//! Data source — `loom.datasource`.
//!
//! A data source read through the Connectors command line: the configured source (a Connectors adapter alias, a connection and the read operation it declares for each kind of read), how the command line is run, what `operations describe` reports for an operation and what one read answers; and a governed write: the operation written, the person's approval proof it passes and what Connectors answers for it. A read never carries a credential and never runs an operation Connectors describes as a mutation. A write carries no credential either; it runs only for a request the runtime admitted, with the path of the approval proof, and is never retried.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AdapterAlias — `loom.datasource.AdapterAlias`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterAlias(pub String);

/// ConnectionId — `loom.datasource.ConnectionId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionId(pub String);

/// ConnectorsCliConfig — `loom.datasource.ConnectorsCliConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorsCliConfig {
    /// `program` — `String`.
    pub program: String,
    /// `config` — `Optional<String>`.
    pub config: Option<String>,
    /// `state_dir` — `Optional<String>`.
    pub state_dir: Option<String>,
    /// `timeout_seconds` — `Optional<Integer>`.
    pub timeout_seconds: Option<i64>,
}

/// DataSource — `loom.datasource.DataSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSource {
    /// `name` — `loom.datasource.SourceName`.
    pub name: SourceName,
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: AdapterAlias,
    /// `connection` — `loom.datasource.ConnectionId`.
    pub connection: ConnectionId,
    /// `list` — `Optional<loom.datasource.OperationId>`.
    pub list: Option<OperationId>,
    /// `search` — `Optional<loom.datasource.OperationId>`.
    pub search: Option<OperationId>,
    /// `get` — `Optional<loom.datasource.OperationId>`.
    pub get: Option<OperationId>,
}

/// MutationClassification — `loom.datasource.MutationClassification`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationClassification {
    /// `NotAttempted`.
    NotAttempted,
    /// `Refused`.
    Refused,
    /// `Applied`.
    Applied,
    /// `Unknown`.
    Unknown,
}

/// OperationId — `loom.datasource.OperationId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationId(pub String);

/// ReadKind — `loom.datasource.ReadKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadKind {
    /// `List`.
    List,
    /// `Search`.
    Search,
    /// `Get`.
    Get,
}

/// ReadRefusal — `loom.datasource.ReadRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadRefusal {
    /// `source` — `Optional<loom.datasource.SourceName>`.
    pub source: Option<SourceName>,
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: AdapterAlias,
    /// `operation` — `Optional<loom.datasource.OperationId>`.
    pub operation: Option<OperationId>,
    /// `code` — `String`.
    pub code: String,
    /// `message` — `String`.
    pub message: String,
}

/// ReadResult — `loom.datasource.ReadResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadResult {
    /// `body` — `Json`.
    pub body: crate::json::Value,
    /// `audit_ref` — `Optional<String>`.
    pub audit_ref: Option<String>,
}

/// SourceEntity — `loom.datasource.SourceEntity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntity {
    /// `operation` — `loom.datasource.OperationId`.
    pub operation: OperationId,
    /// `schema` — `String`.
    pub schema: String,
    /// `input_schema` — `Json`.
    pub input_schema: crate::json::Value,
    /// `revision` — `String`.
    pub revision: String,
}

/// SourceName — `loom.datasource.SourceName`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceName(pub String);

/// WriteApproval — `loom.datasource.WriteApproval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteApproval {
    /// `proof_file` — `String`.
    pub proof_file: String,
}

/// WriteOperation — `loom.datasource.WriteOperation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOperation {
    /// `action` — `String`.
    pub action: String,
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: AdapterAlias,
    /// `connection` — `loom.datasource.ConnectionId`.
    pub connection: ConnectionId,
    /// `operation` — `loom.datasource.OperationId`.
    pub operation: OperationId,
}

/// WriteRefusal — `loom.datasource.WriteRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteRefusal {
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: AdapterAlias,
    /// `operation` — `Optional<loom.datasource.OperationId>`.
    pub operation: Option<OperationId>,
    /// `code` — `String`.
    pub code: String,
    /// `message` — `String`.
    pub message: String,
}

/// WriteResult — `loom.datasource.WriteResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteResult {
    /// `classification` — `loom.datasource.MutationClassification`.
    pub classification: MutationClassification,
    /// `idempotency_key` — `String`.
    pub idempotency_key: String,
    /// `request_id` — `Optional<String>`.
    pub request_id: Option<String>,
    /// `body` — `Optional<Json>`.
    pub body: Option<crate::json::Value>,
    /// `source_audit` — `Optional<Json>`.
    pub source_audit: Option<crate::json::Value>,
    /// `audit_ref` — `Optional<String>`.
    pub audit_ref: Option<String>,
    /// `code` — `Optional<String>`.
    pub code: Option<String>,
}
