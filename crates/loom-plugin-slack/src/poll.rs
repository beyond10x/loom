//! The poll (see the crate documentation).

use b10x_loom_plugin::effects::{NOT_GRANTED, TIMED_OUT};
use b10x_loom_plugin::{
    Cursor, Host, InboundItem, ItemId, Objective, PluginError, PluginState, Poll,
};
use loom::json::Value;
use loom::slack::SlackConfig;
use loom_sdk::connectors::cli::{CliError, ConnectorsCli};

use crate::walk::{self, Member};
use crate::{DEFAULT_LOOKBACK_MINUTES, MAX_LIST_PAGES, READ_LIMIT};

/// The one `subtype` a person's message carries: a file shared with text.
const PERSON_SUBTYPES: &[&str] = &["file_share"];

/// One poll at `now` (microseconds since the Unix epoch).
pub(crate) fn poll(
    config: &SlackConfig,
    host: &Host<'_>,
    state: &PluginState,
    objectives: &[Objective],
    now: i64,
) -> Result<Poll, PluginError> {
    let reader = Reader {
        config,
        cli: host.connectors_over(Vec::new()),
    };
    let channels = walk::order(
        reader.member_channels()?,
        config,
        objectives,
        &state.cursors,
        now,
    );
    let aged_by = now.saturating_sub(config.min_age_minutes.saturating_mul(60_000_000));
    let lookback = config.lookback_minutes.unwrap_or(DEFAULT_LOOKBACK_MINUTES);
    let mut mentions = Vec::new();
    let mut others = Vec::new();
    let mut cursors = Vec::new();
    for channel in &channels {
        let stored = state
            .cursors
            .iter()
            .find(|cursor| cursor.name == channel.id)
            .map(|cursor| cursor.value.clone());
        let oldest = stored
            .unwrap_or_else(|| ts_text(now.saturating_sub(lookback.saturating_mul(60_000_000))));
        let mut messages: Vec<(i64, &Value)> = Vec::new();
        let history = reader.history(&channel.id, &oldest)?;
        for message in items(&history, "messages") {
            if let Some(at) = text(message, "ts").and_then(ts_micros) {
                messages.push((at, message));
            }
        }
        messages.sort_by_key(|(at, _)| *at);
        let mut newest: Option<&str> = None;
        for (at, message) in messages {
            if at > aged_by {
                // Too young to be an item yet; the cursor stops before it, so it is read again.
                break;
            }
            newest = text(message, "ts");
            let Some(item) = candidate(config, channel, message) else {
                continue;
            };
            if reader.answered(&channel.id, message)? {
                continue;
            }
            if mentions_bot(&item.text, &config.bot_user_id) {
                mentions.push(item);
            } else {
                others.push(item);
            }
        }
        if let Some(newest) = newest {
            cursors.push(Cursor {
                name: channel.id.clone(),
                value: newest.to_owned(),
            });
        }
    }
    mentions.extend(others);
    Ok(Poll {
        items: mentions,
        cursors,
    })
}

/// `message` of `channel` as an item, when it is a person's message with text that the bot has
/// neither written nor reacted to; whether its thread answers it is decided after.
fn candidate(config: &SlackConfig, channel: &Member, message: &Value) -> Option<InboundItem> {
    let bot = config.bot_user_id.as_str();
    if text(message, "subtype").is_some_and(|subtype| !PERSON_SUBTYPES.contains(&subtype))
        || message
            .member("bot_id")
            .is_some_and(|id| *id != Value::Null)
        || message.member("bot_profile").is_some()
    {
        return None;
    }
    let user = text(message, "user").filter(|user| !user.is_empty() && *user != bot)?;
    let body = text(message, "text").filter(|body| !body.trim().is_empty())?;
    let reacted = items(message, "reactions").iter().any(|reaction| {
        items(reaction, "users")
            .iter()
            .any(|by| *by == Value::Text(bot.to_owned()))
    });
    if reacted {
        return None;
    }
    let ts = text(message, "ts")?;
    let revision = message
        .member("edited")
        .and_then(|edited| text(edited, "ts"))
        .unwrap_or(ts);
    let mention = mentions_bot(body, bot);
    let mut details = vec![
        ("channel".to_owned(), Value::Text(channel.id.clone())),
        ("user".to_owned(), Value::Text(user.to_owned())),
        ("ts".to_owned(), Value::Text(ts.to_owned())),
        ("mention".to_owned(), Value::Bool(mention)),
    ];
    if let Some(name) = &channel.name {
        details.insert(1, ("channel_name".to_owned(), Value::Text(name.clone())));
    }
    Some(InboundItem {
        id: ItemId(format!("{}:{ts}", channel.id)),
        revision: revision.to_owned(),
        text: body.to_owned(),
        details: Value::Object(details),
    })
}

/// Whether `body` names the user `bot`: `<@U…>` or `<@U…|name>`.
fn mentions_bot(body: &str, bot: &str) -> bool {
    body.contains(&format!("<@{bot}>")) || body.contains(&format!("<@{bot}|"))
}

/// The reads of one poll.
struct Reader<'c> {
    config: &'c SlackConfig,
    cli: ConnectorsCli,
}

impl Reader<'_> {
    /// Every channel the list reports the bot a member of and not archived.
    fn member_channels(&self) -> Result<Vec<Member>, PluginError> {
        let mut channels = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let mut input = vec![
                (
                    "types".to_owned(),
                    Value::Text("public_channel,private_channel".to_owned()),
                ),
                ("exclude_archived".to_owned(), Value::Bool(true)),
                ("limit".to_owned(), number(READ_LIMIT)),
            ];
            if let Some(cursor) = &cursor {
                input.push(("cursor".to_owned(), Value::Text(cursor.clone())));
            }
            let page = self.read(&self.config.list_channels.0, Value::Object(input))?;
            for channel in items(&page, "channels") {
                let Some(id) = text(channel, "id") else {
                    continue;
                };
                if flag(channel, "is_member") && !flag(channel, "is_archived") {
                    channels.push(Member {
                        id: id.to_owned(),
                        name: text(channel, "name").map(str::to_owned),
                        members: channel
                            .member("num_members")
                            .and_then(|count| match count {
                                Value::Number(spelling) => spelling.parse::<i64>().ok(),
                                _ => None,
                            })
                            .unwrap_or(0),
                    });
                }
            }
            cursor = page
                .member("response_metadata")
                .and_then(|metadata| text(metadata, "next_cursor"))
                .filter(|next| !next.is_empty())
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        Ok(channels)
    }

    /// The history of `channel` after `oldest`.
    fn history(&self, channel: &str, oldest: &str) -> Result<Value, PluginError> {
        self.read(
            &self.config.history.0,
            Value::Object(vec![
                ("channel".to_owned(), Value::Text(channel.to_owned())),
                ("oldest".to_owned(), Value::Text(oldest.to_owned())),
                ("limit".to_owned(), number(READ_LIMIT)),
            ]),
        )
    }

    /// Whether `message` of `channel` has a reply in its thread from anybody but its poster. Only
    /// a message with a `reply_count` above 0 has its thread read.
    fn answered(&self, channel: &str, message: &Value) -> Result<bool, PluginError> {
        let replies = message
            .member("reply_count")
            .and_then(|count| match count {
                Value::Number(spelling) => spelling.parse::<i64>().ok(),
                _ => None,
            })
            .unwrap_or(0);
        if replies <= 0 {
            return Ok(false);
        }
        let (Some(ts), poster) = (text(message, "ts"), text(message, "user")) else {
            return Ok(false);
        };
        let thread = text(message, "thread_ts").unwrap_or(ts);
        let read = self.read(
            &self.config.replies.0,
            Value::Object(vec![
                ("channel".to_owned(), Value::Text(channel.to_owned())),
                ("ts".to_owned(), Value::Text(thread.to_owned())),
                ("limit".to_owned(), number(READ_LIMIT)),
            ]),
        )?;
        Ok(items(&read, "messages").iter().any(|reply| {
            text(reply, "ts") != Some(thread)
                && (text(reply, "user") != poster
                    || reply.member("bot_id").is_some_and(|id| *id != Value::Null))
        }))
    }

    /// One read of `operation` on the Slack adapter and connection: Slack's answer, refused when
    /// it says `"ok": false`.
    fn read(&self, operation: &str, input: Value) -> Result<Value, PluginError> {
        let read = self
            .cli
            .invoke_read(
                &self.config.adapter,
                &self.config.connection,
                &loom::datasource::OperationId(operation.to_owned()),
                &input,
            )
            .map_err(|error| failed(operation, &error))?;
        if read.body.member("ok") == Some(&Value::Bool(false)) {
            let why = text(&read.body, "error").unwrap_or("no error named");
            return Err(PluginError::Poll(format!(
                "Slack answered `{operation}` with an error: {why}"
            )));
        }
        Ok(read.body)
    }
}

/// A read that failed: unavailable when Connectors or the connection is down (the program timed
/// out, or `not_granted`), as the host counts it; a failed poll otherwise.
fn failed(operation: &str, error: &CliError) -> PluginError {
    let why = format!("the read `{operation}`: {error}");
    match error {
        CliError::Refused(refusal) if refusal.code == NOT_GRANTED => PluginError::Unavailable(why),
        CliError::Failed(message) if message.contains(TIMED_OUT) => PluginError::Unavailable(why),
        _ => PluginError::Poll(why),
    }
}

fn number(value: i64) -> Value {
    Value::Number(value.to_string())
}

/// The string member `name` of `value`.
fn text<'v>(value: &'v Value, name: &str) -> Option<&'v str> {
    match value.member(name) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

/// The array member `name` of `value`; empty when it is none.
fn items<'v>(value: &'v Value, name: &str) -> &'v [Value] {
    match value.member(name) {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

/// Whether the member `name` of `value` is `true`.
fn flag(value: &Value, name: &str) -> bool {
    value.member(name) == Some(&Value::Bool(true))
}

/// A Slack `ts` (`<seconds>.<microseconds>`) in microseconds since the Unix epoch.
pub(crate) fn ts_micros(ts: &str) -> Option<i64> {
    let (seconds, fraction) = ts.split_once('.').unwrap_or((ts, ""));
    if seconds.is_empty()
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let micros = format!("{fraction:0<6}").parse::<i64>().ok()?;
    seconds
        .parse::<i64>()
        .ok()?
        .checked_mul(1_000_000)?
        .checked_add(micros)
}

/// Microseconds since the Unix epoch as a Slack `ts`.
fn ts_text(micros: i64) -> String {
    let micros = micros.max(0);
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}
