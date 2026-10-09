//! The default projection: what a turn on an item may use.
//!
//! For `ask`, `find` and `request` it admits `source.read`, `reply.propose` and `reply.decline` of
//! `inbound-answer@1`, over the configured sources the classification hints at, or over every
//! configured source when it hints at none of them. A hint that names no configured source is
//! ignored, so the sources are always a subset of the configuration's. For `task` it admits
//! nothing: a task needs a case of its own, which the turn proposes instead.

use loom::datasource::SourceName;
use loom::plugin::{Classification, Intent, PluginConfig, Projection};

/// Reads one configured source.
pub const READ: &str = "source.read";
/// Records a proposed reply.
pub const PROPOSE: &str = "reply.propose";
/// Records that the item gets no proposed reply.
pub const DECLINE: &str = "reply.decline";

/// The projection of `classification` onto `config`.
pub fn project(config: &PluginConfig, classification: &Classification) -> Projection {
    if classification.intent == Intent::Task {
        return Projection {
            actions: Vec::new(),
            sources: Vec::new(),
        };
    }
    let named = |name: &SourceName| classification.hints.contains(&name.0);
    let hinted: Vec<SourceName> = config
        .sources
        .iter()
        .map(|source| source.name.clone())
        .filter(|name| named(name))
        .collect();
    let sources = if hinted.is_empty() {
        config
            .sources
            .iter()
            .map(|source| source.name.clone())
            .collect()
    } else {
        hinted
    };
    Projection {
        actions: [READ, PROPOSE, DECLINE].map(str::to_owned).to_vec(),
        sources,
    }
}
