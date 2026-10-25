//! The A2A side of a served agent (`story:agent-a2a`): [`SpecPlugin`] answers each A2A message
//! with one governed turn: [`Answering`] is its [`b10x_loom_a2a::Answerer`].
//!
//! The turn is the host's turn on an item, as `run_plugin` runs it: the classifier decides the
//! intent, a confidence under the threshold ends it `unclassified`, and the state whose `when`
//! names the intent is the projection. Then it differs in two ways only, so that nothing is sent
//! through A2A:
//!
//! - the turn runs on `inbound-answer@1`, whose `proposed` outcome ends the turn on
//!   `reply.propose`: the proposal is the answer the A2A task returns;
//! - `reply.send` is taken out of the projection and the turn has no send policy, so the effect
//!   port has nothing to send with, whatever the authority grants.
//!
//! Each `RuntimeEvent` of the turn's run reaches the task as it happens. The case and its records
//! are kept in the state directory as every turn's are, under the commission of the item
//! `a2a:<task>` and the message id as its revision.
//!
//! The listener needs a bearer: [`bearer`] reads it from [`BEARER_ENV`], which the agent's Secret
//! sets; serving refuses to start without it.

use std::fmt;

use b10x_loom_a2a::{A2aServer, Answerer, Bearer};

pub use b10x_loom_a2a::admit::{
    A2aRefusalReason, A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome,
};
pub use b10x_loom_a2a::{a2a_refusal_spelling, admit_a2a_request};
use b10x_loom_plugin::project::SEND;
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, TURN_STEP_BUDGET, stopped};
use b10x_loom_plugin::{
    Classification, Host, InboundItem, PROTOCOL, Plugin, PluginError, Projection, RecordLine,
    RecordOutcome, TurnOptions, TurnResult, turn_with,
};
use loom_sdk::EventSink;
use loom_sdk::agent::A2aExposure;
use loom_sdk::governor::{FileCaseStore, FileSuspensionStore};

use super::SpecPlugin;

/// The environment variable holding the bearer every A2A request but a probe carries.
pub const BEARER_ENV: &str = "LOOM_A2A_BEARER";

/// The bearer of the A2A listener, from `value`, the content of [`BEARER_ENV`].
///
/// # Errors
/// [`PluginError::Config`] when it is unset, empty or blank; the message names the variable,
/// never a value.
pub fn bearer(value: Option<&str>) -> Result<Bearer, PluginError> {
    value
        .ok_or_else(|| {
            PluginError::Config(format!(
                "`expose.a2a` is declared and {BEARER_ENV} is unset; the listener needs a bearer"
            ))
        })
        .and_then(|value| {
            Bearer::new(value).map_err(|_| {
                PluginError::Config(format!(
                    "`expose.a2a` is declared and {BEARER_ENV} is empty; the listener needs a bearer"
                ))
            })
        })
}

/// The A2A listener of `exposure`, admitting requests that carry `bearer`.
///
/// # Errors
/// [`PluginError::Config`] for an `expose.a2a` the listener cannot serve: a port outside
/// 1..=65535, or a path that is not absolute or is one the listener answers itself.
pub fn listener(exposure: &A2aExposure, bearer: Bearer) -> Result<A2aServer, PluginError> {
    port(exposure)?;
    A2aServer::new(exposure.clone(), bearer)
        .map_err(|why| PluginError::Config(format!("the agent spec: {why}")))
}

/// The port of `exposure`.
///
/// # Errors
/// [`PluginError::Config`] for a port outside 1..=65535.
pub fn port(exposure: &A2aExposure) -> Result<u16, PluginError> {
    u16::try_from(exposure.port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| {
            PluginError::Config(format!(
                "the agent spec: `expose.a2a.port` {} is no port from 1 to 65535",
                exposure.port
            ))
        })
}

/// A served agent answering A2A messages: [`SpecPlugin`] under a host.
pub struct Answering<'a> {
    plugin: &'a SpecPlugin,
    host: Host<'a>,
}

impl<'a> Answering<'a> {
    /// `plugin` under the host of its configuration, as `run_plugin` builds it.
    pub fn new(plugin: &'a SpecPlugin) -> Self {
        Self {
            plugin,
            host: Host::new(&plugin.config),
        }
    }

    /// `plugin` under `host`.
    pub fn with_host(plugin: &'a SpecPlugin, host: Host<'a>) -> Self {
        Self { plugin, host }
    }
}

impl Answerer for Answering<'_> {
    fn answer(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        let error = |error: PluginError| error.to_string();
        let (plugin, host) = (self.plugin, &self.host);
        let threshold = host.threshold().map_err(error)?;
        let classification = plugin.classify(host, item).map_err(error)?;
        let sure = classification
            .confidence
            .0
            .trim()
            .parse::<f64>()
            .ok()
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
        let mut projection = plugin.project(host, item, &classification);
        projection.actions.retain(|action| action != SEND);
        let turn = plugin
            .answer_turn(host, item, &classification, &projection, events)
            .map_err(error)?;
        Ok(plugin.result(item, &classification, turn))
    }
}

impl SpecPlugin {
    /// One turn of an A2A message (see the module documentation).
    fn answer_turn(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
        projection: &Projection,
        events: &mut dyn EventSink,
    ) -> Result<TurnResult, PluginError> {
        if projection.actions.is_empty() {
            return Ok(stopped(
                Vec::new(),
                "no state of the spec admits this intent",
            ));
        }
        let state = |error: &dyn fmt::Display| PluginError::State(error.to_string());
        let store = FileCaseStore::open(&self.state).map_err(|e| state(&e))?;
        let suspensions = FileSuspensionStore::open(&self.state).map_err(|e| state(&e))?;
        let inner = self.spec.loops.as_ref().map(|loops| &loops.inner);
        let step_budget = inner
            .and_then(|inner| usize::try_from(inner.step_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(TURN_STEP_BUDGET);
        let model_budget = inner
            .and_then(|inner| u64::try_from(inner.model_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(TURN_MODEL_BUDGET);
        let turned = turn_with(
            host,
            self.turn_model()?,
            item,
            classification,
            projection,
            TurnOptions {
                protocol: PROTOCOL,
                authority: &self.authority,
                store,
                suspensions: Some(&suspensions),
                send: None,
                commission: Some(self.commission_of(item)),
                step_budget,
                model_budget,
                events: Some(events),
            },
        )?;
        Ok(turned.result)
    }
}
