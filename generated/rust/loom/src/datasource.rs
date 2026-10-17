// generated from loom v1
// model digest 748fceb21cf7ee6d4e3507a439a01eec6e7e9190c5bee46f896cc1ad49fa1326
// contract digest 46c4d4d570ca4756a9a8caf1cdfa2d49a60fe40d986d3e54b1dd2ba9a7b18d06
// do not edit: regenerate with `ess synthesize --layout crate`

//! Data source — `loom.datasource`.
//!
//! A data source read through the Connectors command line: the configured source (a Connectors adapter alias, a connection and the read operation it declares for each kind of read), how the command line is run, what `operations describe` reports for an operation and what one read answers. A read never carries a credential and never runs an operation Connectors describes as a mutation.
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
