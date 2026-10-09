#![forbid(unsafe_code)]

//! The slack-handler plugin: a read-only walk over the Slack channels the bot is a member of,
//! handing the plugin host ([`b10x_loom_plugin`]) the recent messages nobody answered.
//!
//! [`SlackHandler`] implements [`Plugin`]. Its own hook is the poll; classification, the
//! projection, the governed turn and the record line are the host's defaults, so a question gets a
//! proposed reply read from the configured data sources, a task a proposed case, and nothing is
//! ever sent. Its configuration is the generated `loom.slack` [`SlackConfig`], which holds the
//! host's `PluginConfig` ([`config`] reads it from a file and refuses one that cannot run).
//!
//! # The poll
//!
//! Every read goes through `loom_connectors::cli::ConnectorsCli::invoke_read` on the configured
//! Slack adapter and connection, with the three read operations the configuration names:
//!
//! 1. `list_channels` (Slack's `conversations.list`), page by page up to [`MAX_LIST_PAGES`]; only
//!    channels whose `is_member` is true and that are not archived are walked.
//! 2. The walk orders them: the heaviest objective a channel serves first, then the
//!    channel whose newest message the plugin has held longest (one never read first of all), then
//!    the channel with more members, then an order drawn from the seed.
//! 3. `history` (`conversations.history`) of each channel in that order, with `oldest` set to the
//!    channel's stored `ts` (the cursor named by the channel id), or, for a channel never read,
//!    to `lookback_minutes` ago ([`DEFAULT_LOOKBACK_MINUTES`] when absent).
//! 4. `replies` (`conversations.replies`) of a thread, only for a message that would otherwise be
//!    an item and has a `reply_count`: a reply by anybody but the poster answers it.
//!
//! A message is an item when it has aged `min_age_minutes`, has no `subtype` (a join or any other
//! system message) other than `file_share`, is no bot's (no `bot_id`, not the bot's user id), has
//! no reaction by the bot, has text, and no reply in its thread from anybody but its poster. Items
//! naming the bot (`<@bot_user_id>`) come first, then the others, each group in walk order and,
//! within a channel, oldest first. An item's id is `<channel>:<ts>`, its revision the `ts` of its
//! last edit (its own `ts` when unedited), its text the message's text, and its details the
//! channel, the user, the `ts` and whether it names the bot.
//!
//! A channel's cursor moves to the newest message that has aged `min_age_minutes`, item or not; a
//! younger message is read again by the next poll. The host saves the cursors only after a cycle
//! in which nothing was unavailable, and handles an item id once.

pub mod config;
mod poll;
mod walk;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_plugin::{
    Host, Objective, Plugin, PluginError, PluginState, Poll, RecordLine, TurnModel, run_plugin,
    run_plugin_on,
};
use llm_core::Model;

pub use loom::slack::{ChannelObjectives, SlackConfig};

/// The name the plugin is registered under.
pub const NAME: &str = "slack-handler";

/// How far back a channel never read before is read, when the configuration names no
/// `lookback_minutes`: one day.
pub const DEFAULT_LOOKBACK_MINUTES: i64 = 24 * 60;

/// The most pages of the channel list one poll reads.
pub const MAX_LIST_PAGES: usize = 10;

/// The `limit` of every read: channels per page, messages per history, replies per thread.
pub const READ_LIMIT: i64 = 200;

/// Makes the model each turn runs on, one per turn.
pub type TurnModels = Box<dyn Fn() -> Result<TurnModel<'static>, PluginError>>;

/// The host's clock: microseconds since the Unix epoch.
pub type Clock = Box<dyn Fn() -> i64>;

/// The slack-handler plugin over one checked configuration.
pub struct SlackHandler {
    config: SlackConfig,
    classifier: Option<Box<dyn Model>>,
    turn_models: Option<TurnModels>,
    clock: Clock,
}

impl SlackHandler {
    /// The plugin over `config`, on the system clock, with no models yet.
    ///
    /// # Errors
    /// [`PluginError::Config`] for a configuration [`config::check`] refuses: no Slack adapter or
    /// connection, no bot user id, and the rest it names.
    pub fn new(config: SlackConfig) -> Result<Self, PluginError> {
        config::check(&config)?;
        Ok(Self {
            config,
            classifier: None,
            turn_models: None,
            clock: Box::new(system_clock),
        })
    }

    /// Classifies items, and picks a task's protocol, with `model`.
    #[must_use]
    pub fn with_classifier(mut self, model: Box<dyn Model>) -> Self {
        self.classifier = Some(model);
        self
    }

    /// Runs each turn on a model `models` makes.
    #[must_use]
    pub fn with_turn_models(mut self, models: TurnModels) -> Self {
        self.turn_models = Some(models);
        self
    }

    /// Reads the time from `clock` instead of the system clock.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// The configuration.
    pub fn config(&self) -> &SlackConfig {
        &self.config
    }

    /// Hosts the plugin with its configuration, keeping its state in `state`, until `stop` is set
    /// ([`run_plugin`]).
    ///
    /// # Errors
    /// As [`run_plugin`].
    pub fn run(&self, state: &Path, stop: &AtomicBool) -> Result<Vec<RecordLine>, PluginError> {
        run_plugin(self, &self.config.plugin, state, stop)
    }

    /// [`SlackHandler::run`] on `host`, which must hold this plugin's `PluginConfig`.
    ///
    /// # Errors
    /// As [`run_plugin`].
    pub fn run_on(
        &self,
        host: &Host<'_>,
        state: &Path,
        stop: &AtomicBool,
    ) -> Result<Vec<RecordLine>, PluginError> {
        run_plugin_on(self, host, state, stop)
    }
}

impl Plugin for SlackHandler {
    fn name(&self) -> &str {
        NAME
    }

    fn poll(
        &self,
        host: &Host<'_>,
        state: &PluginState,
        objectives: &[Objective],
    ) -> Result<Poll, PluginError> {
        poll::poll(&self.config, host, state, objectives, (self.clock)())
    }

    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        self.classifier.as_deref().ok_or_else(|| {
            PluginError::Unavailable(format!("the plugin `{NAME}` was given no classifier model"))
        })
    }

    fn turn_model(&self) -> Result<TurnModel<'_>, PluginError> {
        match &self.turn_models {
            Some(models) => models(),
            None => Err(PluginError::Unavailable(format!(
                "the plugin `{NAME}` was given no turn model"
            ))),
        }
    }
}

/// The system clock, in microseconds since the Unix epoch; 0 before it.
fn system_clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
