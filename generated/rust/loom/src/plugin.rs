// generated from loom v1
// model digest 300dc2d9cea4ebe03da46be3740cd2be006c06099199c740b4db2b22e0ef540b
// contract digest 8d8c474b54c7be5a40b1ec49641cdc66e85f8f3e768698010015dd595b0fce5e
// do not edit: regenerate with `ess synthesize --layout crate`

//! Plugin — `loom.plugin`.
//!
//! A plugin hosted by Loom: what its poll answers, how an item is classified and projected onto the actions and data sources a governed turn may use, what the turn ends with, the line the host records for each handled item and the state it keeps between polls. A turn proposes a reply or declines; it sends nothing.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// Classification — `loom.plugin.Classification`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    /// `intent` — `loom.plugin.Intent`.
    pub intent: Intent,
    /// `hints` — `List<String>`.
    pub hints: Vec<String>,
    /// `confidence` — `Decimal`.
    pub confidence: crate::primitives::Decimal,
}

/// Cursor — `loom.plugin.Cursor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// `name` — `String`.
    pub name: String,
    /// `value` — `String`.
    pub value: String,
}

/// InboundItem — `loom.plugin.InboundItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundItem {
    /// `id` — `loom.plugin.ItemId`.
    pub id: ItemId,
    /// `revision` — `String`.
    pub revision: String,
    /// `text` — `String`.
    pub text: String,
    /// `details` — `Json`.
    pub details: crate::json::Value,
}

/// Intent — `loom.plugin.Intent`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// `Ask`.
    Ask,
    /// `Request`.
    Request,
    /// `Task`.
    Task,
    /// `Find`.
    Find,
}

/// ItemFailures — `loom.plugin.ItemFailures`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFailures {
    /// `item` — `loom.plugin.InboundItem`.
    pub item: InboundItem,
    /// `failures` — `Integer`.
    pub failures: i64,
    /// `last_failure` — `String`.
    pub last_failure: String,
}

/// ItemId — `loom.plugin.ItemId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemId(pub String);

/// Objective — `loom.plugin.Objective`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objective {
    /// `name` — `String`.
    pub name: String,
    /// `weight` — `Decimal`.
    pub weight: crate::primitives::Decimal,
}

/// PluginConfig — `loom.plugin.PluginConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConfig {
    /// `connectors` — `loom.datasource.ConnectorsCliConfig`.
    pub connectors: crate::datasource::ConnectorsCliConfig,
    /// `sources` — `List<loom.datasource.DataSource>`.
    pub sources: Vec<crate::datasource::DataSource>,
    /// `objectives` — `List<loom.plugin.Objective>`.
    pub objectives: Vec<Objective>,
    /// `classify_threshold` — `Optional<Decimal>`.
    pub classify_threshold: Option<crate::primitives::Decimal>,
    /// `poll_interval_seconds` — `Integer`.
    pub poll_interval_seconds: i64,
    /// `workspace_roots` — `List<String>`.
    pub workspace_roots: Vec<String>,
    /// `checkouts` — `List<String>`.
    pub checkouts: Vec<String>,
}

/// PluginState — `loom.plugin.PluginState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginState {
    /// `cursors` — `List<loom.plugin.Cursor>`.
    pub cursors: Vec<Cursor>,
    /// `handled` — `List<loom.plugin.ItemId>`.
    pub handled: Vec<ItemId>,
    /// `failing` — `List<loom.plugin.ItemFailures>`.
    pub failing: Vec<ItemFailures>,
}

/// Poll — `loom.plugin.Poll`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    /// `items` — `List<loom.plugin.InboundItem>`.
    pub items: Vec<InboundItem>,
    /// `cursors` — `List<loom.plugin.Cursor>`.
    pub cursors: Vec<Cursor>,
}

/// Projection — `loom.plugin.Projection`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// `actions` — `List<String>`.
    pub actions: Vec<String>,
    /// `sources` — `List<loom.datasource.SourceName>`.
    pub sources: Vec<crate::datasource::SourceName>,
}

/// Proposal — `loom.plugin.Proposal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// `item` — `loom.plugin.ItemId`.
    pub item: ItemId,
    /// `text` — `String`.
    pub text: String,
}

/// ProposedCase — `loom.plugin.ProposedCase`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedCase {
    /// `protocol` — `String`.
    pub protocol: String,
    /// `confidence` — `Decimal`.
    pub confidence: crate::primitives::Decimal,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// RecordLine — `loom.plugin.RecordLine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLine {
    /// `item` — `loom.plugin.ItemId`.
    pub item: ItemId,
    /// `intent` — `Optional<loom.plugin.Intent>`.
    pub intent: Option<Intent>,
    /// `confidence` — `Optional<Decimal>`.
    pub confidence: Option<crate::primitives::Decimal>,
    /// `outcome` — `loom.plugin.RecordOutcome`.
    pub outcome: RecordOutcome,
    /// `reads` — `List<loom.plugin.SourceRead>`.
    pub reads: Vec<SourceRead>,
    /// `proposal` — `Optional<String>`.
    pub proposal: Option<String>,
    /// `proposed_case` — `Optional<loom.plugin.ProposedCase>`.
    pub proposed_case: Option<ProposedCase>,
    /// `detail` — `Optional<String>`.
    pub detail: Option<String>,
}

/// RecordOutcome — `loom.plugin.RecordOutcome`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    /// `Proposed`.
    Proposed,
    /// `Declined`.
    Declined,
    /// `ProposedCase`.
    ProposedCase,
    /// `Unclassified`.
    Unclassified,
    /// `Stopped`.
    Stopped,
}

/// SourceRead — `loom.plugin.SourceRead`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRead {
    /// `source` — `loom.datasource.SourceName`.
    pub source: crate::datasource::SourceName,
    /// `kind` — `loom.datasource.ReadKind`.
    pub kind: crate::datasource::ReadKind,
}

/// TurnResult — `loom.plugin.TurnResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    /// `outcome` — `loom.plugin.RecordOutcome`.
    pub outcome: RecordOutcome,
    /// `reads` — `List<loom.plugin.SourceRead>`.
    pub reads: Vec<SourceRead>,
    /// `proposal` — `Optional<String>`.
    pub proposal: Option<String>,
    /// `proposed_case` — `Optional<loom.plugin.ProposedCase>`.
    pub proposed_case: Option<ProposedCase>,
    /// `detail` — `Optional<String>`.
    pub detail: Option<String>,
}
