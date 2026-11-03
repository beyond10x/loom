// generated from loom v1
// model digest 687ce3d39bdb34d56d1fe0c221f558534fa9c3e8eda9e34862b5b742d61331e9
// contract digest 3a8aa766c8bb9ebca0e67279c2e64d75442a59c32dac9ff9524c320274ed5bed
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
