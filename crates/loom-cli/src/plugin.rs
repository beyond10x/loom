//! What `b10x-loom plugin` prints and reads: one line per proposal, the record a plugin keeps in
//! its state directory, and the turn models of `plugin run`.
//!
//! A proposal line is the item id, the outcome, the intent, then for a proposed reply the reads
//! it made (`reads=<source>/<kind>,…`) and its text as a JSON string, and for a proposed case
//! `case=<protocol>` and the router's `confidence=<x>`. Every part a source supplied is escaped,
//! so a line is always one line.

use std::path::Path;
use std::sync::Arc;

use b10x_loom_intake_slice::run::printable;
use b10x_loom_plugin::datasource::ReadKind;
use b10x_loom_plugin::state::RECORD_FILE;
use b10x_loom_plugin::{
    Intent, PluginError, RecordLine, RecordOutcome, TurnModel, decode_record_line,
};
use b10x_loom_plugin_slack::TurnModels;
use llm_core::Model;

use crate::model_port::LlmModelPort;

/// Whether `line` is a proposal: a proposed reply or a proposed case.
pub fn is_proposal(line: &RecordLine) -> bool {
    matches!(
        line.outcome,
        RecordOutcome::Proposed | RecordOutcome::ProposedCase
    )
}

/// The line `plugin report` prints for `line`.
pub fn report_line(line: &RecordLine) -> String {
    let outcome = match line.outcome {
        RecordOutcome::Proposed => "proposed",
        RecordOutcome::Declined => "declined",
        RecordOutcome::ProposedCase => "proposed_case",
        RecordOutcome::Unclassified => "unclassified",
        RecordOutcome::Stopped => "stopped",
    };
    let intent = match line.intent {
        Some(Intent::Ask) => "ask",
        Some(Intent::Request) => "request",
        Some(Intent::Task) => "task",
        Some(Intent::Find) => "find",
        None => "-",
    };
    let mut text = format!("{} {outcome} {intent}", printable(&line.item.0));
    if !line.reads.is_empty() {
        let reads: Vec<String> = line
            .reads
            .iter()
            .map(|read| {
                let kind = match read.kind {
                    ReadKind::List => "list",
                    ReadKind::Search => "search",
                    ReadKind::Get => "get",
                };
                format!("{}/{kind}", printable(&read.source.0))
            })
            .collect();
        text.push_str(&format!(" reads={}", reads.join(",")));
    }
    if let Some(case) = &line.proposed_case {
        text.push_str(&format!(
            " case={} confidence={}",
            printable(&case.protocol),
            printable(&case.confidence.0)
        ));
    }
    if let Some(proposal) = &line.proposal {
        text.push(' ');
        text.push_str(&serde_json::Value::String(proposal.clone()).to_string());
    }
    text
}

/// Every line of the record in the state directory `state`, without taking its lock: none when
/// nothing is recorded, and the torn trace of an append still in progress (the bytes after the
/// last newline) left out.
///
/// # Errors
/// A record that cannot be read, or a whole line that is not a record line, naming the file.
pub fn recorded(state: &Path) -> Result<Vec<RecordLine>, String> {
    let path = state.join(RECORD_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "the record {} cannot be read: {error}",
                path.display()
            ));
        }
    };
    let whole = text.rsplit_once('\n').map_or("", |(whole, _)| whole);
    whole
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(at, line)| {
            decode_record_line(line)
                .map_err(|error| format!("{} line {}: {error}", path.display(), at + 1))
        })
        .collect()
}

/// Turn models over `model`: each turn gets its own [`LlmModelPort`], every request naming the
/// model's own name.
pub fn turn_models(model: Arc<dyn Model>) -> TurnModels {
    Box::new(move || {
        let port = LlmModelPort::new(Arc::clone(&model)).map_err(PluginError::Unavailable)?;
        Ok(TurnModel {
            model: port.model_name(),
            port: Box::new(port),
        })
    })
}
