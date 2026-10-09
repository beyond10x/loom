//! The slack-handler's configuration file: one JSON object, the generated `loom.slack`
//! `SlackConfig`, read and checked before anything starts.
//!
//! ```json
//! {
//!   "plugin": {
//!     "connectors": {"program": "connectors", "config": null, "state_dir": null, "timeout_seconds": 60},
//!     "sources": [{"name": "docs", "adapter": "docs", "connection": "docs-main", "search": "docs.search"}],
//!     "objectives": [{"name": "help people with cheap lookups", "weight": 0.6}],
//!     "classify_threshold": 0.5,
//!     "poll_interval_seconds": 300,
//!     "workspace_roots": [],
//!     "checkouts": []
//!   },
//!   "adapter": "slack",
//!   "connection": "slack-main",
//!   "list_channels": "conversations.list",
//!   "history": "conversations.history",
//!   "replies": "conversations.replies",
//!   "bot_user_id": "U0BOT",
//!   "min_age_minutes": 10,
//!   "seed": 7,
//!   "lookback_minutes": 1440,
//!   "channels": [{"channel": "C0FIXTURE1", "objectives": ["help people with cheap lookups"]}]
//! }
//! ```
//!
//! A member every object declares is required, except an optional one (`config`, `state_dir`,
//! `timeout_seconds`, `list`, `search`, `get`, `classify_threshold`, `lookback_minutes`), which
//! may be absent or `null`, and a list, which may be absent and is then empty. A decimal is a JSON
//! number or a string spelling one. A member no object declares is refused, so a misspelt key is
//! never silently ignored. [`check`] then refuses what decodes but cannot run (see there).

use std::path::Path;

use b10x_loom_plugin::PluginError;
use loom::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, DataSource, OperationId, SourceName,
};
use loom::json::{self, Value};
use loom::plugin::{Objective, PluginConfig};
use loom::primitives::Decimal;
use loom::slack::{ChannelObjectives, SlackConfig};

/// The largest configuration file read.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Reads the configuration file at `path`, decodes it ([`parse`]) and checks it ([`check`]).
///
/// # Errors
/// [`PluginError::Config`] naming the file, for a file that cannot be read, is larger than
/// [`MAX_CONFIG_BYTES`], or that [`parse`] refuses.
pub fn read(path: &Path) -> Result<SlackConfig, PluginError> {
    let refused = |why: String| PluginError::Config(format!("{}: {why}", path.display()));
    let metadata = std::fs::metadata(path).map_err(|error| refused(error.to_string()))?;
    if !metadata.is_file() {
        return Err(refused("is not a regular file".to_owned()));
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(refused(format!("is larger than {MAX_CONFIG_BYTES} bytes")));
    }
    let text = std::fs::read_to_string(path).map_err(|error| refused(error.to_string()))?;
    parse(&text).map_err(|error| match error {
        PluginError::Config(why) => refused(why),
        other => other,
    })
}

/// Decodes the configuration `text` and checks it ([`check`]).
///
/// # Errors
/// [`PluginError::Config`] naming the member, for text that is not JSON, a missing required
/// member, a member of the wrong type, an unknown member, or a configuration [`check`] refuses.
pub fn parse(text: &str) -> Result<SlackConfig, PluginError> {
    let value = json::parse(text)
        .map_err(|error| PluginError::Config(format!("the configuration is not JSON: {error}")))?;
    let config = slack(&value).map_err(PluginError::Config)?;
    check(&config)?;
    Ok(config)
}

/// Refuses a configuration that decodes but cannot run: no Slack adapter or connection, a read
/// operation that names nothing, no bot user id, a negative `min_age_minutes`, a
/// `lookback_minutes` that is not positive, or a channel entry naming no channel or an objective
/// the plugin configuration does not declare.
///
/// # Errors
/// [`PluginError::Config`] naming the member.
pub fn check(config: &SlackConfig) -> Result<(), PluginError> {
    let refused = |why: String| Err(PluginError::Config(why));
    for (member, value, what) in [
        (
            "adapter",
            &config.adapter.0,
            "the Slack adapter the poll reads through",
        ),
        (
            "connection",
            &config.connection.0,
            "the Slack connection the poll reads",
        ),
        (
            "list_channels",
            &config.list_channels.0,
            "the channel list read",
        ),
        ("history", &config.history.0, "the channel history read"),
        ("replies", &config.replies.0, "the thread replies read"),
        (
            "bot_user_id",
            &config.bot_user_id,
            "the bot's user id, which tells its own messages, reactions and mentions apart",
        ),
    ] {
        if value.trim().is_empty() {
            return refused(format!("`{member}` is empty; it names {what}"));
        }
    }
    if config.min_age_minutes < 0 {
        return refused(format!(
            "`min_age_minutes` {} is negative",
            config.min_age_minutes
        ));
    }
    if let Some(minutes) = config.lookback_minutes
        && minutes <= 0
    {
        return refused(format!("`lookback_minutes` {minutes} is not positive"));
    }
    for (at, entry) in config.channels.iter().enumerate() {
        if entry.channel.trim_start_matches('#').trim().is_empty() {
            return refused(format!("`channels[{at}].channel` names no channel"));
        }
        for objective in &entry.objectives {
            if !config
                .plugin
                .objectives
                .iter()
                .any(|declared| declared.name == *objective)
            {
                return refused(format!(
                    "`channels[{at}]` names the objective `{objective}`, which `plugin.objectives` \
                     does not declare"
                ));
            }
        }
    }
    Ok(())
}

/// One JSON object being decoded, at its path.
struct Object<'v> {
    at: String,
    members: &'v [(String, Value)],
}

impl<'v> Object<'v> {
    /// The object `value` at `at`, holding no member outside `known`.
    fn new(value: &'v Value, at: &str, known: &[&str]) -> Result<Self, String> {
        let members = json::members_at(value, display(at), "an object").map_err(decode)?;
        if let Some((unknown, _)) = members
            .iter()
            .find(|(name, _)| !known.contains(&name.as_str()))
        {
            return Err(format!(
                "`{}` is not a member the configuration declares",
                json::nested(at, unknown)
            ));
        }
        Ok(Self {
            at: at.to_owned(),
            members,
        })
    }

    fn path(&self, name: &str) -> String {
        json::nested(&self.at, name)
    }

    /// The member `name`, `None` when absent or `null`.
    fn optional(&self, name: &str) -> Option<&'v Value> {
        self.members
            .iter()
            .find(|(member, _)| member == name)
            .map(|(_, value)| value)
            .filter(|value| **value != Value::Null)
    }

    fn required(&self, name: &str) -> Result<&'v Value, String> {
        self.optional(name)
            .ok_or_else(|| format!("`{}` is missing", self.path(name)))
    }

    fn text(&self, name: &str) -> Result<String, String> {
        let at = self.path(name);
        json::text_at(self.required(name)?, &at, "a string")
            .map(str::to_owned)
            .map_err(decode)
    }

    fn optional_text(&self, name: &str) -> Result<Option<String>, String> {
        self.optional(name)
            .map(|value| {
                json::text_at(value, &self.path(name), "a string")
                    .map(str::to_owned)
                    .map_err(decode)
            })
            .transpose()
    }

    fn integer(&self, name: &str) -> Result<i64, String> {
        json::integer_at(self.required(name)?, &self.path(name), "an integer").map_err(decode)
    }

    fn optional_integer(&self, name: &str) -> Result<Option<i64>, String> {
        self.optional(name)
            .map(|value| json::integer_at(value, &self.path(name), "an integer").map_err(decode))
            .transpose()
    }

    fn decimal(&self, name: &str) -> Result<Decimal, String> {
        decimal(self.required(name)?, &self.path(name))
    }

    fn optional_decimal(&self, name: &str) -> Result<Option<Decimal>, String> {
        self.optional(name)
            .map(|value| decimal(value, &self.path(name)))
            .transpose()
    }

    /// The list `name`, empty when absent, each element decoded by `each` at its index.
    fn list<T>(
        &self,
        name: &str,
        each: impl Fn(&Value, &str) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let Some(value) = self.optional(name) else {
            return Ok(Vec::new());
        };
        let at = self.path(name);
        json::items_at(value, &at, "a list")
            .map_err(decode)?
            .iter()
            .enumerate()
            .map(|(index, item)| each(item, &format!("{at}[{index}]")))
            .collect()
    }
}

fn display(at: &str) -> &str {
    if at.is_empty() {
        "the configuration"
    } else {
        at
    }
}

fn decode(error: json::DecodeError) -> String {
    format!(
        "`{}` is {}: expected {}",
        error.at, error.found, error.expected
    )
}

/// A decimal: a JSON number, or a string spelling one.
fn decimal(value: &Value, at: &str) -> Result<Decimal, String> {
    match value {
        Value::Number(spelling) => Ok(Decimal(spelling.clone())),
        Value::Text(text) if text.trim().parse::<f64>().is_ok_and(f64::is_finite) => {
            Ok(Decimal(text.trim().to_owned()))
        }
        other => Err(format!(
            "`{at}` is {}: expected a decimal number",
            other.describes()
        )),
    }
}

fn slack(value: &Value) -> Result<SlackConfig, String> {
    let object = Object::new(
        value,
        "",
        &[
            "plugin",
            "adapter",
            "connection",
            "list_channels",
            "history",
            "replies",
            "bot_user_id",
            "min_age_minutes",
            "seed",
            "lookback_minutes",
            "channels",
        ],
    )?;
    Ok(SlackConfig {
        plugin: plugin(object.required("plugin")?, "plugin")?,
        adapter: AdapterAlias(object.text("adapter")?),
        connection: ConnectionId(object.text("connection")?),
        list_channels: OperationId(object.text("list_channels")?),
        history: OperationId(object.text("history")?),
        replies: OperationId(object.text("replies")?),
        bot_user_id: object.text("bot_user_id")?,
        min_age_minutes: object.integer("min_age_minutes")?,
        seed: object.integer("seed")?,
        lookback_minutes: object.optional_integer("lookback_minutes")?,
        channels: object.list("channels", |value, at| {
            let entry = Object::new(value, at, &["channel", "objectives"])?;
            Ok(ChannelObjectives {
                channel: entry.text("channel")?,
                objectives: entry.list("objectives", |value, at| {
                    json::text_at(value, at, "a string")
                        .map(str::to_owned)
                        .map_err(decode)
                })?,
            })
        })?,
    })
}

fn plugin(value: &Value, at: &str) -> Result<PluginConfig, String> {
    let object = Object::new(
        value,
        at,
        &[
            "connectors",
            "sources",
            "objectives",
            "classify_threshold",
            "poll_interval_seconds",
            "workspace_roots",
            "checkouts",
        ],
    )?;
    let texts = |value: &Value, at: &str| {
        json::text_at(value, at, "a string")
            .map(str::to_owned)
            .map_err(decode)
    };
    Ok(PluginConfig {
        connectors: connectors(object.required("connectors")?, &object.path("connectors"))?,
        sources: object.list("sources", source)?,
        objectives: object.list("objectives", |value, at| {
            let objective = Object::new(value, at, &["name", "weight"])?;
            Ok(Objective {
                name: objective.text("name")?,
                weight: objective.decimal("weight")?,
            })
        })?,
        classify_threshold: object.optional_decimal("classify_threshold")?,
        poll_interval_seconds: object.integer("poll_interval_seconds")?,
        workspace_roots: object.list("workspace_roots", texts)?,
        checkouts: object.list("checkouts", texts)?,
    })
}

fn connectors(value: &Value, at: &str) -> Result<ConnectorsCliConfig, String> {
    let object = Object::new(
        value,
        at,
        &["program", "config", "state_dir", "timeout_seconds"],
    )?;
    Ok(ConnectorsCliConfig {
        program: object.text("program")?,
        config: object.optional_text("config")?,
        state_dir: object.optional_text("state_dir")?,
        timeout_seconds: object.optional_integer("timeout_seconds")?,
    })
}

fn source(value: &Value, at: &str) -> Result<DataSource, String> {
    let object = Object::new(
        value,
        at,
        &["name", "adapter", "connection", "list", "search", "get"],
    )?;
    let operation = |name: &str| object.optional_text(name).map(|text| text.map(OperationId));
    Ok(DataSource {
        name: SourceName(object.text("name")?),
        adapter: AdapterAlias(object.text("adapter")?),
        connection: ConnectionId(object.text("connection")?),
        list: operation("list")?,
        search: operation("search")?,
        get: operation("get")?,
    })
}
