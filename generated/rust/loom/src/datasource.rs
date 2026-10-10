// generated from loom v1
// model digest 661401844e582bd43becacc2018c2e17e03abd6db5247a31d68efc412baadd64
// contract digest 6085f971d79bcae6071aef5103a63bf925d279df28713c5dd4a816f29b50f01e
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
