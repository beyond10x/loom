// generated from loom v1
// model digest 1b4649239db036c3c54fd1c6256d3b9d645bd435c2ca60b02f06c78c3984d805
// contract digest 1f9c399d657e6f3bf6b0a8cdbf769f9a23ce8204351cad8651327a3aec61a299
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
