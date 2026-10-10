// generated from loom v1
// model digest 300dc2d9cea4ebe03da46be3740cd2be006c06099199c740b4db2b22e0ef540b
// contract digest 8d8c474b54c7be5a40b1ec49641cdc66e85f8f3e768698010015dd595b0fce5e
// do not edit: regenerate with `ess synthesize --layout crate`

//! Slack — `loom.slack`.
//!
//! The slack-handler plugin's configuration: the host's plugin configuration, the Slack adapter, connection and read operations its poll invokes, the bot's user id, the age rule, the walk's seed and lookback, and the objectives each channel serves. Nothing here writes to Slack.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ChannelObjectives — `loom.slack.ChannelObjectives`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelObjectives {
    /// `channel` — `String`.
    pub channel: String,
    /// `objectives` — `List<String>`.
    pub objectives: Vec<String>,
}

/// SlackConfig — `loom.slack.SlackConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackConfig {
    /// `plugin` — `loom.plugin.PluginConfig`.
    pub plugin: crate::plugin::PluginConfig,
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: crate::datasource::AdapterAlias,
    /// `connection` — `loom.datasource.ConnectionId`.
    pub connection: crate::datasource::ConnectionId,
    /// `list_channels` — `loom.datasource.OperationId`.
    pub list_channels: crate::datasource::OperationId,
    /// `history` — `loom.datasource.OperationId`.
    pub history: crate::datasource::OperationId,
    /// `replies` — `loom.datasource.OperationId`.
    pub replies: crate::datasource::OperationId,
    /// `bot_user_id` — `String`.
    pub bot_user_id: String,
    /// `min_age_minutes` — `Integer`.
    pub min_age_minutes: i64,
    /// `seed` — `Integer`.
    pub seed: i64,
    /// `lookback_minutes` — `Optional<Integer>`.
    pub lookback_minutes: Option<i64>,
    /// `channels` — `List<loom.slack.ChannelObjectives>`.
    pub channels: Vec<ChannelObjectives>,
}
