//! Loom's action selector and argument generator over a model (story `selector-executor`).
//!
//! [`ModelSelector`] implements Loom's `ActionSelector` and [`ModelArguments`] its
//! `ArgumentGenerator`, each over a `&dyn llm_core::Model`. Each asks the model for one forced call
//! of one named tool (`ToolChoice::Named`):
//!
//! - the selector publishes `select_action`, `{"action": <id>}`, whose `action` is one of the
//!   catalogue entries it was handed. Loom refuses any other id; the selector does not check it.
//! - the generator publishes `action_arguments`, whose schema is the selected action's
//!   ([`crate::executor::arguments_schema`]), and returns the call's arguments as they are.
//!
//! Both are told the [`Briefing`]: the intent, its extracted references and the transcript so far,
//! which the caller extends with [`Briefing::record`] after each performed action and with
//! [`Briefing::record_refusal`] after each action that was not performed. Whatever else the
//! model says, text beside the call included, is dropped: a model answer is never evidence.
//!
//! The transcript keeps the last [`TRANSCRIPT_LIMIT`] entries, each cut at 16 KiB, numbered from the
//! first entry ever recorded. An entry quotes what the executor reported, and that includes file
//! contents the model itself wrote: inspected contents can imitate transcript lines (a forged
//! "tests.run ... exited with 0") and mislead the model's next choice. That is a limit on the
//! model's context, not on evidence: evidence comes only from the verifier (see
//! [`crate::verifier`]).
//!
//! The model's turn is driven to completion on a current-thread Tokio runtime made for the call,
//! so neither may be called from inside a Tokio runtime.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::responsibility::ExecutorOutcomeProposedAction;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext, SelectorError};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator};
use b10x_loom_intake_references::ExtractedReference;
use llm_core::{
    BoxFuture, Cancel, Error, Item, Model, StreamEvent, StreamSink, ToolChoice, ToolName, ToolSpec,
    TurnRequest,
};
use serde_json::Value;

use crate::executor::{Report, arguments_schema};

/// The tool the selector publishes.
pub const SELECT_TOOL: &str = "select_action";
/// The tool the generator publishes.
pub const ARGUMENTS_TOOL: &str = "action_arguments";

/// The selector's `Unavailable` message for a selection call that names no action: the model
/// answered, unusably, rather than failing to answer.
pub const NO_ACTION: &str = "the model's selection names no action";

/// The most bytes of one transcript entry the model is shown.
const ENTRY_LIMIT: usize = 16 * 1024;
/// The most transcript entries the model is shown: the last ones.
pub const TRANSCRIPT_LIMIT: usize = 64;

const SELECT_INSTRUCTIONS: &str = "You choose the next action of a software change in a local \
git workspace. Call `select_action` with exactly one of the candidate actions listed. Whatever \
you say besides the call is ignored; only running the tests shows whether they pass.";

const ARGUMENTS_INSTRUCTIONS: &str = "You write the arguments of the selected action of a \
software change in a local git workspace. Call `action_arguments` with arguments that match its \
schema. Paths are relative to the workspace root.";

/// What the model is told about the work: the intent, its references and the transcript so far.
/// Clones share one transcript.
#[derive(Debug, Clone)]
pub struct Briefing {
    inner: Arc<Mutex<Brief>>,
}

#[derive(Debug)]
struct Brief {
    intent: String,
    references: Vec<ExtractedReference>,
    transcript: VecDeque<String>,
    recorded: usize,
}

impl Briefing {
    /// A briefing on `intent` and `references`, with an empty transcript.
    pub fn new(intent: impl Into<String>, references: Vec<ExtractedReference>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Brief {
                intent: intent.into(),
                references,
                transcript: VecDeque::new(),
                recorded: 0,
            })),
        }
    }

    /// Adds a performed action and its report to the transcript.
    pub fn record(&self, proposal: &ExecutorOutcomeProposedAction, report: &Report) {
        self.push(format!(
            "{} {}\n{report}",
            proposal.action,
            text_of(&proposal.arguments.0)
        ));
    }

    /// Adds an action that was chosen or proposed and then not performed, with the reason, to the
    /// transcript: the entry reads `<action>` and then `refused: <reason>`.
    pub fn record_refusal(&self, action: &str, reason: &str) {
        self.push(format!("{action}\nrefused: {reason}"));
    }

    /// Adds one entry, cut at [`ENTRY_LIMIT`] bytes, keeping the last [`TRANSCRIPT_LIMIT`].
    fn push(&self, mut entry: String) {
        if entry.len() > ENTRY_LIMIT {
            let mut end = ENTRY_LIMIT;
            while !entry.is_char_boundary(end) {
                end -= 1;
            }
            entry.truncate(end);
            entry.push_str("\n[truncated]");
        }
        let mut brief = self.brief();
        brief.transcript.push_back(entry);
        brief.recorded += 1;
        while brief.transcript.len() > TRANSCRIPT_LIMIT {
            brief.transcript.pop_front();
        }
    }

    fn brief(&self) -> MutexGuard<'_, Brief> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The briefing as text for the model.
    fn text(&self) -> String {
        let brief = self.brief();
        let mut text = format!("Intent:\n{}\n\nReferences in the intent:\n", brief.intent);
        if brief.references.is_empty() {
            text.push_str("(none)\n");
        }
        for reference in &brief.references {
            let _ = writeln!(text, "- {:?}: {}", reference.kind, reference.value);
        }
        text.push_str("\nTranscript so far:\n");
        if brief.transcript.is_empty() {
            text.push_str("(nothing done yet)\n");
        }
        let first = brief.recorded - brief.transcript.len();
        if first > 0 {
            let _ = writeln!(text, "({first} earlier entries are not shown)");
        }
        for (n, entry) in brief.transcript.iter().enumerate() {
            let _ = writeln!(text, "{}. {entry}", first + n + 1);
        }
        text
    }
}

/// Loom's selector, choosing with a model.
pub struct ModelSelector<'m> {
    model: &'m dyn Model,
    briefing: Briefing,
}

impl<'m> ModelSelector<'m> {
    /// A selector that asks `model`, telling it `briefing`.
    pub fn new(model: &'m dyn Model, briefing: Briefing) -> Self {
        Self { model, briefing }
    }
}

impl ActionSelector for ModelSelector<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        if candidates.is_empty() {
            return Err(SelectorError::NothingAdmissible);
        }
        let mut text = self.briefing.text();
        text.push_str("\nCandidate actions:\n");
        for candidate in candidates {
            let status = match candidate.status {
                CatalogueEntryStatus::Admissible => "admissible",
                CatalogueEntryStatus::ApprovalRequired => "needs approval",
            };
            let _ = writeln!(text, "- {} ({status})", candidate.action);
        }
        let actions: Vec<&str> = candidates
            .iter()
            .map(|entry| entry.action.as_str())
            .collect();
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"action": {"type": "string", "enum": actions}},
            "required": ["action"],
            "additionalProperties": false
        });
        let answer = ask(
            self.model,
            SELECT_INSTRUCTIONS,
            text,
            SELECT_TOOL,
            "Choose the next action from the candidates.",
            schema,
        )
        .map_err(SelectorError::Unavailable)?;
        let action = answer
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| SelectorError::Unavailable(NO_ACTION.to_owned()))?;
        Ok(Choice {
            action: action.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// Loom's argument generator, writing arguments with a model.
pub struct ModelArguments<'m> {
    model: &'m dyn Model,
    briefing: Briefing,
}

impl<'m> ModelArguments<'m> {
    /// A generator that asks `model`, telling it `briefing`.
    pub fn new(model: &'m dyn Model, briefing: Briefing) -> Self {
        Self { model, briefing }
    }
}

impl ArgumentGenerator for ModelArguments<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        let mut text = self.briefing.text();
        let _ = writeln!(text, "\nSelected action: {}", entry.action);
        let answer = ask(
            self.model,
            ARGUMENTS_INSTRUCTIONS,
            text,
            ARGUMENTS_TOOL,
            &format!("The arguments of `{}`.", entry.action),
            arguments_schema(&entry.action),
        )?;
        json::parse(&answer.to_string())
            .map_err(|error| format!("the model's arguments are not JSON: {error:?}"))
    }
}

/// The arguments of the model's forced call of `tool`, asked with `instructions` and `text`.
fn ask(
    model: &dyn Model,
    instructions: &str,
    text: String,
    tool: &str,
    description: &str,
    schema: Value,
) -> Result<Value, String> {
    let name = ToolName::new(tool).map_err(|error| error.to_string())?;
    let mut request = TurnRequest::new(model.provenance().model.as_str(), vec![Item::user(text)]);
    instructions.clone_into(&mut request.instructions);
    request.tools = vec![ToolSpec {
        name: name.clone(),
        description: description.to_owned(),
        input_schema: schema,
    }];
    request.tool_choice = ToolChoice::Named(name.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("no runtime for the model turn: {error}"))?;
    let cancel = Cancel::new();
    let mut sink = Discard;
    let outcome = runtime
        .block_on(model.turn(&request, &mut sink, &cancel))
        .map_err(|error| error.to_string())?;
    outcome
        .validate_for(&request, model.provenance())
        .map_err(|error| error.to_string())?;
    outcome
        .tool_calls()
        .find(|call| call.name == name)
        .map(|call| call.arguments.clone())
        .ok_or_else(|| format!("the model did not call `{tool}`"))
}

/// The arguments as compact JSON text.
fn text_of(value: &json::Value) -> String {
    match value {
        json::Value::Null => "null".to_owned(),
        json::Value::Bool(value) => value.to_string(),
        json::Value::Number(number) => number.clone(),
        json::Value::Text(text) => Value::from(text.as_str()).to_string(),
        json::Value::Array(items) => {
            let items: Vec<String> = items.iter().map(text_of).collect();
            format!("[{}]", items.join(","))
        }
        json::Value::Object(members) => {
            let members: Vec<String> = members
                .iter()
                .map(|(name, value)| format!("{}:{}", Value::from(name.as_str()), text_of(value)))
                .collect();
            format!("{{{}}}", members.join(","))
        }
    }
}

/// A sink that drops the stream: only the turn's outcome is read.
struct Discard;

impl StreamSink for Discard {
    fn emit(&mut self, _event: StreamEvent) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_loom_commission::model::responsibility::ProposedActionArguments;

    #[test]
    fn the_transcript_keeps_its_last_entries() {
        let briefing = Briefing::new("intent", Vec::new());
        let proposal = ExecutorOutcomeProposedAction {
            action: "repository.edit".to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
        };
        for n in 1..=TRANSCRIPT_LIMIT + 6 {
            let report = Report::Edited {
                revision: format!("revision-{n}"),
            };
            briefing.record(&proposal, &report);
        }
        let text = briefing.text();
        assert_eq!(briefing.brief().transcript.len(), TRANSCRIPT_LIMIT);
        assert!(text.contains("(6 earlier entries are not shown)"), "{text}");
        assert!(!text.contains("revision-6\n"), "{text}");
        assert!(text.contains("7. repository.edit {}\ncommitted; HEAD is revision-7\n"));
        assert!(text.contains("70. repository.edit {}\ncommitted; HEAD is revision-70\n"));
    }
}
