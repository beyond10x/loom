#![forbid(unsafe_code)]

//! The plugin host: an unattended agent is a plugin of Loom (ADR `plugin-hooks`).
//!
//! A [`Plugin`] is a crate linked at build time. [`run_plugin`] hosts it in a loop, per item:
//!
//! | Hook | Default | What it answers |
//! |---|---|---|
//! | [`Plugin::poll`] | none: the plugin's own | new [`InboundItem`]s and the [`Cursor`]s after them |
//! | [`Plugin::classify`] | [`classify::classify`]: one forced `classify_item` call | the [`Intent`], hints and confidence |
//! | [`Plugin::project`] | [`project::project`] | the actions and data sources a turn may use |
//! | [`Plugin::turn`] | [`turn::turn`], or [`turn::propose_case`] for a task | one governed run on the item |
//! | [`Plugin::result`] | [`record_line`] | the item's [`RecordLine`] |
//! | [`Plugin::objectives`] | the configuration's | the weights of the next poll |
//!
//! A classification below `classify_threshold` (0.5 when absent) is recorded `unclassified` and
//! runs no turn. An attempt on an item is counted in the state before it starts; a classify or
//! turn hook that fails leaves the item unhandled with no line, the state keeps the item, and each
//! cycle retries the failing items from the state before it polls. The [`MAX_FAILURES`]th
//! attempt that does not succeed records the item `stopped` with the last reason. A failure that
//! is not about the item ([`PluginError::Unavailable`]) counts against none: the cycle ends there,
//! without saving the poll's cursors, and the next cycle tries again. A poll that fails is such a
//! failure for the whole host: the cycle records nothing for it and saves no cursor, and the host
//! polls again after its interval; only a host that is stopping returns the failure. Every handled
//! item adds one line to the record and its id to the state; an id already handled is skipped, so
//! polling the same items again adds no line.
//!
//! The turn is a thin caller of Commission's `run_until_blocked` through `loom_sdk`: a
//! `CanonGovernor` over [`ProtocolCatalog::plugins`](loom_sdk::ProtocolCatalog::plugins) on
//! [`PROTOCOL`], Loom's governed model loop selecting from the frontier, [`PluginAuthority`]
//! granting `datasource.read` and `reply.propose` and nothing else, and [`DataSourceEffects`]
//! reading the projection's sources through the Connectors command line. Nothing is sent: a reply
//! is a proposal in the record.
//!
//! The state directory ([`state::StateDir`]) holds `state.json`, written to a temporary file and
//! renamed, and `record.jsonl`, one line per handled item. It is refused inside a workspace root or
//! checkout the configuration names; the check compares paths and runs no git command.

pub mod classify;
mod codec;
pub mod effects;
pub mod project;
pub mod state;
pub mod turn;

mod authority;

use std::ffi::OsString;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use llm_core::Model;
use loom_sdk::connectors::cli::ConnectorsCli;
use loom_sdk::loom::harness::wire::ModelPort;

pub use authority::{GRANTED, PluginAuthority};
pub use codec::{decode_record_line, encode_record_line};
pub use effects::DataSourceEffects;
pub use loom::datasource;
pub use loom::plugin::{
    Classification, Cursor, InboundItem, Intent, ItemFailures, ItemId, Objective, PluginConfig,
    PluginState, Poll, Projection, Proposal, ProposedCase, RecordLine, RecordOutcome, SourceRead,
    TurnResult,
};
pub use loom::primitives::Decimal;

use loom::datasource::DataSource;

/// The protocol of every turn, from the plugin catalog.
pub const PROTOCOL: &str = "inbound-answer@1";

/// The failures of one item after which it is recorded `stopped` and handled.
pub const MAX_FAILURES: i64 = 3;

/// The classification threshold when the configuration names none, as `loom-intake-router`'s
/// `--threshold`.
pub const DEFAULT_THRESHOLD: f64 = 0.5;

/// Why a hook, the state directory or the host failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// The state directory lies inside a workspace root or checkout the configuration names.
    StateInsideCheckout {
        /// The state directory, as resolved.
        state: String,
        /// The root or checkout it lies inside, as resolved.
        inside: String,
    },
    /// Another host holds the state directory.
    StateInUse {
        /// The state directory, as given.
        state: String,
    },
    /// The state directory or a file in it could not be read or written.
    State(String),
    /// The configuration cannot be used: a threshold outside 0 to 1, a negative interval.
    Config(String),
    /// The plugin's poll failed.
    Poll(String),
    /// Classification gave no usable answer.
    Classify(String),
    /// A turn could not run.
    Turn(String),
    /// Something the host needs and that is not about the item is unavailable: the sources cannot
    /// be described, the model cannot be reached, the router's model or catalog failed. It counts
    /// against no item; the cycle ends and the next one tries again.
    Unavailable(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateInsideCheckout { state, inside } => write!(
                f,
                "the state directory {state} lies inside {inside}, a workspace root or checkout \
                 of the configuration"
            ),
            Self::StateInUse { state } => write!(
                f,
                "the state directory {state} is held by another host; one host runs on a state \
                 directory at a time"
            ),
            Self::State(why) => write!(f, "the plugin state: {why}"),
            Self::Config(why) => write!(f, "the plugin configuration: {why}"),
            Self::Poll(why) => write!(f, "the poll failed: {why}"),
            Self::Classify(why) => write!(f, "classification failed: {why}"),
            Self::Turn(why) => write!(f, "the turn failed: {why}"),
            Self::Unavailable(why) => write!(f, "unavailable to the host: {why}"),
        }
    }
}

impl std::error::Error for PluginError {}

/// The model a turn runs on: a port of Loom's model loop and the model name its requests carry.
pub struct TurnModel<'a> {
    /// The model port.
    pub port: Box<dyn ModelPort + 'a>,
    /// The model name every request of the turn carries.
    pub model: String,
}

/// What the host hands each hook: the configuration and a Connectors read client over its
/// sources.
pub struct Host<'a> {
    config: &'a PluginConfig,
    environment: Option<Vec<(OsString, OsString)>>,
}

impl<'a> Host<'a> {
    /// The host of `config`; the Connectors command line starts from this process's environment,
    /// narrowed to [`loom_sdk::connectors::cli::INHERITED`].
    pub fn new(config: &'a PluginConfig) -> Self {
        Self {
            config,
            environment: None,
        }
    }

    /// The host of `config`, starting the Connectors command line from `environment` instead,
    /// still narrowed.
    #[must_use]
    pub fn with_environment<K, V>(mut self, environment: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.environment = Some(
            environment
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        );
        self
    }

    /// The plugin's configuration.
    pub fn config(&self) -> &PluginConfig {
        self.config
    }

    /// A read client over every configured source.
    pub fn connectors(&self) -> ConnectorsCli {
        self.connectors_over(self.config.sources.clone())
    }

    /// A read client over `sources` only.
    pub fn connectors_over(&self, sources: Vec<DataSource>) -> ConnectorsCli {
        let cli = ConnectorsCli::new(self.config.connectors.clone(), sources);
        match &self.environment {
            Some(environment) => cli.with_environment(environment.iter().cloned()),
            None => cli,
        }
    }

    /// The classification threshold: the configuration's, or [`DEFAULT_THRESHOLD`].
    ///
    /// # Errors
    /// [`PluginError::Config`] for a threshold that is no number from 0 to 1.
    pub fn threshold(&self) -> Result<f64, PluginError> {
        let Some(threshold) = &self.config.classify_threshold else {
            return Ok(DEFAULT_THRESHOLD);
        };
        decimal(threshold)
            .filter(|value| (0.0..=1.0).contains(value))
            .ok_or_else(|| {
                PluginError::Config(format!(
                    "classify_threshold `{}` is not a number from 0 to 1",
                    threshold.0
                ))
            })
    }
}

/// A plugin of Loom: its hooks, each with the host's default where it has one.
pub trait Plugin {
    /// The plugin's name.
    fn name(&self) -> &str;

    /// New items, and the cursors after them. `state` holds the cursors of the last poll and the
    /// ids already handled; `objectives` weigh this poll.
    ///
    /// # Errors
    /// Any failure to poll. [`run_plugin`] takes it as an outage of the whole host: it records
    /// nothing for the poll, saves no cursor and polls again after its interval.
    fn poll(
        &self,
        host: &Host<'_>,
        state: &PluginState,
        objectives: &[Objective],
    ) -> Result<Poll, PluginError>;

    /// The item's intent, hints and confidence: by default one forced call on
    /// [`Plugin::classifier`] ([`classify::classify`]).
    ///
    /// # Errors
    /// No classifier, or no usable answer.
    fn classify(&self, host: &Host<'_>, item: &InboundItem) -> Result<Classification, PluginError> {
        classify::classify(self.classifier()?, host.config(), item)
    }

    /// The actions and sources a turn on `item` may use ([`project::project`]).
    fn project(
        &self,
        host: &Host<'_>,
        _item: &InboundItem,
        classification: &Classification,
    ) -> Projection {
        project::project(host.config(), classification)
    }

    /// One turn on `item`. By default, a projection that admits nothing proposes a case for a
    /// task ([`turn::propose_case`]) and stops otherwise; any other runs the governed turn on
    /// [`Plugin::turn_model`] ([`turn::turn`]).
    ///
    /// # Errors
    /// No model, or a turn that could not run.
    fn turn(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
        projection: &Projection,
    ) -> Result<TurnResult, PluginError> {
        if projection.actions.is_empty() {
            if classification.intent == Intent::Task {
                return turn::propose_case(host, self.classifier()?, item);
            }
            return Ok(turn::stopped(Vec::new(), "the projection admits no action"));
        }
        turn::turn(host, self.turn_model()?, item, classification, projection)
    }

    /// The record line of `item` ([`record_line`]).
    fn result(
        &self,
        item: &InboundItem,
        classification: &Classification,
        turn: TurnResult,
    ) -> RecordLine {
        record_line(item, Some(classification), turn)
    }

    /// The weights of the next poll: the configuration's.
    fn objectives(&self, config: &PluginConfig) -> Vec<Objective> {
        config.objectives.clone()
    }

    /// The model the default [`Plugin::classify`] and the task path ask.
    ///
    /// # Errors
    /// By default the plugin supplies none.
    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        Err(PluginError::Classify(format!(
            "the plugin `{}` supplies no classifier model",
            self.name()
        )))
    }

    /// The model the default [`Plugin::turn`] runs on.
    ///
    /// # Errors
    /// By default the plugin supplies none.
    fn turn_model(&self) -> Result<TurnModel<'_>, PluginError> {
        Err(PluginError::Turn(format!(
            "the plugin `{}` supplies no turn model",
            self.name()
        )))
    }
}

/// The record line of `item`: its classification's intent and confidence, when it has one, and
/// what `turn` ended with.
pub fn record_line(
    item: &InboundItem,
    classification: Option<&Classification>,
    turn: TurnResult,
) -> RecordLine {
    RecordLine {
        item: item.id.clone(),
        intent: classification.map(|c| c.intent),
        confidence: classification.map(|c| c.confidence.clone()),
        outcome: turn.outcome,
        reads: turn.reads,
        proposal: turn.proposal,
        proposed_case: turn.proposed_case,
        detail: turn.detail,
    }
}

/// Hosts `plugin` with `config`, keeping its state in the directory `state`, until `stop` is set.
///
/// The state directory is checked against the configuration and held
/// ([`state::StateDir::open`]) before anything else, until the call returns. Each cycle first
/// retries the failing items the state keeps, then polls and handles every item not handled or
/// tried yet (classify, then, at or above the threshold, project, turn and result), appending one
/// record line per handled item and saving the state after each attempt and the cursors after the
/// cycle. It then checks `stop`: set, the call returns; otherwise it waits
/// `poll_interval_seconds`, checking `stop` meanwhile, and starts the next cycle. A `stop` already
/// set runs exactly one cycle (`once`); a `stop` set while the host runs ends it after the item
/// being handled, leaving the rest of the poll's items and its cursors to the next host. A cycle
/// whose poll fails saves no cursor and handles no
/// polled item; the host waits and polls again as after any cycle.
///
/// # Errors
/// A state directory refused, held by another host or unusable, or an unusable configuration.
/// The failure of the poll of the last cycle, when `stop` ends the host after it (with `once`,
/// after its one cycle). A failing item or an unavailable service is no error (see the crate
/// docs).
pub fn run_plugin<P: Plugin + ?Sized>(
    plugin: &P,
    config: &PluginConfig,
    state: &Path,
    stop: &AtomicBool,
) -> Result<Vec<RecordLine>, PluginError> {
    run_plugin_on(plugin, &Host::new(config), state, stop)
}

/// [`run_plugin`] with the host given.
///
/// # Errors
/// As [`run_plugin`].
pub fn run_plugin_on<P: Plugin + ?Sized>(
    plugin: &P,
    host: &Host<'_>,
    state: &Path,
    stop: &AtomicBool,
) -> Result<Vec<RecordLine>, PluginError> {
    let config = host.config();
    let held = state::StateDir::open(state, config)?;
    let threshold = host.threshold()?;
    let interval = u64::try_from(config.poll_interval_seconds)
        .map(Duration::from_secs)
        .map_err(|_| {
            PluginError::Config(format!(
                "poll_interval_seconds {} is negative",
                config.poll_interval_seconds
            ))
        })?;
    let mut cycle = Cycle {
        plugin,
        host,
        held: &held,
        threshold,
        recorded: Vec::new(),
        // A stop already set is `once`: its one cycle runs whole. One set later ends the host
        // after the item being handled.
        stop: (!stop.load(Ordering::SeqCst)).then_some(stop),
    };
    loop {
        let outage = cycle.run()?;
        if stopped_within(stop, interval) {
            return match outage {
                Some(failed) => Err(failed),
                None => Ok(cycle.recorded),
            };
        }
    }
}

/// What one attempt on an item came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempted {
    /// The item was recorded, or its failure counted.
    Done,
    /// A service the host needs was unavailable: the cycle ends.
    Unavailable,
}

/// The cycles of one [`run_plugin_on`] call.
struct Cycle<'c, 'h, P: ?Sized> {
    plugin: &'c P,
    host: &'c Host<'h>,
    held: &'c state::StateDir,
    threshold: f64,
    recorded: Vec<RecordLine>,
    /// The stop that ends a cycle between two items; `None` for `once`.
    stop: Option<&'c AtomicBool>,
}

impl<P: Plugin + ?Sized> Cycle<'_, '_, P> {
    /// One cycle: the failing items of the state, then the poll's items, then the cursors. A
    /// poll that fails is an outage of the whole host: the cycle ends there, saving no cursor, and
    /// answers the failure as `Some`.
    fn run(&mut self) -> Result<Option<PluginError>, PluginError> {
        let mut current = self.held.load()?;
        let failing: Vec<InboundItem> = current
            .failing
            .iter()
            .map(|failing| failing.item.clone())
            .collect();
        let mut tried: Vec<ItemId> = Vec::new();
        for item in &failing {
            if self.stopping() {
                return Ok(None);
            }
            tried.push(item.id.clone());
            if self.attempt(&mut current, item)? == Attempted::Unavailable {
                return Ok(None);
            }
        }
        let objectives = self.plugin.objectives(self.host.config());
        let poll = match self.plugin.poll(self.host, &current, &objectives) {
            Ok(poll) => poll,
            Err(failed) => return Ok(Some(failed)),
        };
        for item in &poll.items {
            if current.handled.contains(&item.id) || tried.contains(&item.id) {
                continue;
            }
            if self.stopping() {
                // The poll's cursors are not saved, so the next poll answers the items left.
                return Ok(None);
            }
            tried.push(item.id.clone());
            if self.attempt(&mut current, item)? == Attempted::Unavailable {
                // The poll's cursors are not saved, so the next poll answers what this one did.
                return Ok(None);
            }
        }
        for cursor in poll.cursors {
            match current
                .cursors
                .iter_mut()
                .find(|kept| kept.name == cursor.name)
            {
                Some(kept) => kept.value = cursor.value,
                None => current.cursors.push(cursor),
            }
        }
        self.held.save(&current)?;
        Ok(None)
    }

    /// Whether a stop set after the host started ends the cycle before the next item.
    fn stopping(&self) -> bool {
        self.stop.is_some_and(|stop| stop.load(Ordering::SeqCst))
    }

    /// One attempt on `item`, counted in the state before it starts.
    fn attempt(
        &mut self,
        current: &mut PluginState,
        item: &InboundItem,
    ) -> Result<Attempted, PluginError> {
        let previous = current
            .failing
            .iter()
            .position(|failing| failing.item.id == item.id)
            .map(|at| current.failing.remove(at));
        let before = previous.as_ref().map_or(0, |failing| failing.failures);
        if before >= MAX_FAILURES {
            // Attempts that never returned, a crash among them: no further one is made.
            let last = previous
                .map(|failing| failing.last_failure)
                .unwrap_or_default();
            let turn = turn::stopped(
                Vec::new(),
                &format!("{before} attempts did not succeed; the last: {last}"),
            );
            self.finish(current, item, record_line(item, None, turn))?;
            return Ok(Attempted::Done);
        }
        let failures = before + 1;
        current.failing.push(ItemFailures {
            item: item.clone(),
            failures,
            last_failure: format!("attempt {failures} did not finish"),
        });
        self.held.save(current)?;
        match handle(self.plugin, self.host, item, self.threshold) {
            Ok(line) => self.finish(current, item, line)?,
            Err((_, PluginError::Unavailable(_))) => {
                current.failing.retain(|failing| failing.item.id != item.id);
                current.failing.extend(previous);
                self.held.save(current)?;
                return Ok(Attempted::Unavailable);
            }
            Err((classification, error)) => {
                if failures < MAX_FAILURES {
                    if let Some(failing) = current
                        .failing
                        .iter_mut()
                        .find(|failing| failing.item.id == item.id)
                    {
                        failing.last_failure = error.to_string();
                    }
                    self.held.save(current)?;
                    return Ok(Attempted::Done);
                }
                let turn = turn::stopped(
                    Vec::new(),
                    &format!("{failures} attempts failed; the last: {error}"),
                );
                let line = match &classification {
                    Some(classification) => self.plugin.result(item, classification, turn),
                    None => record_line(item, None, turn),
                };
                self.finish(current, item, line)?;
            }
        }
        Ok(Attempted::Done)
    }

    /// Records `line` for `item` and saves it as handled.
    fn finish(
        &mut self,
        current: &mut PluginState,
        item: &InboundItem,
        line: RecordLine,
    ) -> Result<(), PluginError> {
        self.held.append(&line)?;
        current.handled.push(item.id.clone());
        current.failing.retain(|failing| failing.item.id != item.id);
        self.held.save(current)?;
        self.recorded.push(line);
        Ok(())
    }
}

/// Whether `stop` is set now or becomes set within `interval`.
fn stopped_within(stop: &AtomicBool, interval: Duration) -> bool {
    let started = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return true;
        }
        let waited = started.elapsed();
        if waited >= interval {
            return false;
        }
        std::thread::sleep((interval - waited).min(Duration::from_millis(100)));
    }
}

/// One item, from classification to its record line; or the classify or turn failure, with the
/// classification when there is one.
fn handle<P: Plugin + ?Sized>(
    plugin: &P,
    host: &Host<'_>,
    item: &InboundItem,
    threshold: f64,
) -> Result<RecordLine, (Option<Classification>, PluginError)> {
    let classification = plugin.classify(host, item).map_err(|error| (None, error))?;
    let sure = decimal(&classification.confidence)
        .is_some_and(|confidence| (0.0..=1.0).contains(&confidence) && confidence >= threshold);
    if !sure {
        let turn = TurnResult {
            outcome: RecordOutcome::Unclassified,
            reads: Vec::new(),
            proposal: None,
            proposed_case: None,
            detail: Some(format!(
                "confidence {} is below the threshold {threshold}",
                classification.confidence.0
            )),
        };
        return Ok(plugin.result(item, &classification, turn));
    }
    let projection = plugin.project(host, item, &classification);
    match plugin.turn(host, item, &classification, &projection) {
        Ok(turn) => Ok(plugin.result(item, &classification, turn)),
        Err(error) => Err((Some(classification), error)),
    }
}

/// A decimal's value, when it is a finite number.
pub(crate) fn decimal(value: &Decimal) -> Option<f64> {
    value.0.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}
