//! Loom's action selector and argument generator over an llm `Model` (story `selector-executor`;
//! `story:chooser-executor-for-plugin-turns` moved them here from the intake slice so every host
//! can use them).
//!
//! [`ModelSelector`] implements Loom's [`ActionSelector`] and [`ModelArguments`] its
//! [`ArgumentGenerator`], each over a `&dyn llm_core::Model` and a host's [`Briefing`], what the
//! model is told about the work. Each request asks for one forced call of one named tool
//! (`ToolChoice::Named`):
//!
//! - the selector publishes [`SELECT_TOOL`], `{"action": <id>}`, whose `action` is one of the
//!   catalogue entries it was handed. Loom refuses any other id; the selector does not check it.
//! - the generator publishes [`ARGUMENTS_TOOL`], whose schema is the selected action's
//!   ([`Briefing::arguments_schema`]), and returns the call's arguments as the briefing gives them
//!   ([`Briefing::arguments`]).
//!
//! Whatever else the model says, text beside the call included, is dropped: a model answer is
//! never evidence. Every request is sent through [`Briefing::send`]: one attempt, unless the host
//! retries. The intake slice's briefing (`b10x_loom_intake_slice::selector::Briefing`) tells the
//! model the intent and the transcript of a software change; a plugin turn's tells it the item and
//! what the turn read.
//!
//! The model's turn is driven to completion on a current-thread Tokio runtime made for the call,
//! so neither may be called from inside a Tokio runtime.

use std::fmt::{self, Write as _};

use b10x_loom_commission::model::json;
use llm_core::{
    BoxFuture, Cancel, Error, Item, Model, StreamEvent, StreamSink, ToolChoice, ToolName, ToolSpec,
    TurnOutcome, TurnRequest,
};
use serde_json::Value;

use crate::arguments::{ArgumentContext, ArgumentGenerator};
use crate::model::run::{CatalogueEntry, CatalogueEntryStatus, SelectionStrategy};
use crate::selection::{ActionSelector, Choice, SelectionContext, SelectorError};

/// The tool the selector publishes.
pub const SELECT_TOOL: &str = "select_action";
/// The tool the generator publishes.
pub const ARGUMENTS_TOOL: &str = "action_arguments";

/// The selector's `Unavailable` message for a selection call that names no action: the model
/// answered, unusably, rather than failing to answer.
pub const NO_ACTION: &str = "the model's selection names no action";

/// The description of [`SELECT_TOOL`] in a request [`select_once`] sends.
const SELECT_DESCRIPTION: &str = "Choose the next action from the candidates.";

/// What a [`ModelSelector`] and a [`ModelArguments`] tell the model, and how they ask it. A host
/// implements the first four; the provided methods send one request per selection and one per
/// argument generation, which a host overrides to ask otherwise.
pub trait Briefing {
    /// The text every request carries, before what the request itself adds (the candidate
    /// actions, or the selected action).
    fn text(&self) -> String;

    /// The standing instructions of a selection request.
    fn selection_instructions(&self) -> String;

    /// The standing instructions of an argument request.
    fn argument_instructions(&self) -> String;

    /// The input schema of [`ARGUMENTS_TOOL`] when `action` is selected.
    fn arguments_schema(&self, action: &str) -> Value;

    /// Sends `request` to `model`: one attempt. A host that retries a model call overrides it;
    /// every attempt it makes is a call of `model`'s `turn`.
    fn send<'a>(
        &'a self,
        model: &'a dyn Model,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        model.turn(request, sink, cancel)
    }

    /// The arguments of the model's call of [`SELECT_TOOL`], `candidates` being the text that
    /// lists the candidate actions and `schema` the tool's input schema: by default
    /// [`select_once`].
    ///
    /// # Errors
    /// Why the model gave no usable call.
    fn select(&self, model: &dyn Model, candidates: &str, schema: Value) -> Result<Value, String> {
        select_once(self, model, candidates, schema)
    }

    /// The arguments of `action`, as they are proposed: by default [`arguments_once`].
    ///
    /// # Errors
    /// [`ArgumentsError::Unavailable`] when the model cannot be asked or gives no call;
    /// [`ArgumentsError::Refused`] when its answer cannot become the action's arguments.
    fn arguments(&self, model: &dyn Model, action: &str) -> Result<Value, ArgumentsError> {
        arguments_once(self, model, action)
    }
}

/// One selection request: [`Briefing::selection_instructions`], then the briefing's text and
/// `candidates`, forcing [`SELECT_TOOL`] with `schema`.
///
/// # Errors
/// Why the model gave no usable call.
pub fn select_once<B: Briefing + ?Sized>(
    briefing: &B,
    model: &dyn Model,
    candidates: &str,
    schema: Value,
) -> Result<Value, String> {
    ask(
        model,
        briefing,
        &briefing.selection_instructions(),
        format!("{}{candidates}", briefing.text()),
        SELECT_TOOL,
        SELECT_DESCRIPTION,
        schema,
    )
}

/// One argument request: [`Briefing::argument_instructions`], then the briefing's text and the
/// line `Selected action: <action>`, forcing [`ARGUMENTS_TOOL`] with
/// [`Briefing::arguments_schema`]. The call's arguments are returned as they are.
///
/// # Errors
/// [`ArgumentsError::Unavailable`] when the model cannot be asked or gives no call.
pub fn arguments_once<B: Briefing + ?Sized>(
    briefing: &B,
    model: &dyn Model,
    action: &str,
) -> Result<Value, ArgumentsError> {
    let mut text = briefing.text();
    // Writing to a String cannot fail.
    let _ = writeln!(text, "\nSelected action: {action}");
    ask(
        model,
        briefing,
        &briefing.argument_instructions(),
        text,
        ARGUMENTS_TOOL,
        &format!("The arguments of `{action}`."),
        briefing.arguments_schema(action),
    )
    .map_err(ArgumentsError::Unavailable)
}

/// Loom's selector, choosing with a model.
pub struct ModelSelector<'m, B> {
    model: &'m dyn Model,
    briefing: B,
}

impl<'m, B> ModelSelector<'m, B> {
    /// A selector that asks `model`, telling it `briefing`.
    pub fn new(model: &'m dyn Model, briefing: B) -> Self {
        Self { model, briefing }
    }
}

impl<B: Briefing> ActionSelector for ModelSelector<'_, B> {
    /// The model's choice among `candidates`, each listed with its status. No candidate is
    /// [`SelectorError::NothingAdmissible`]; an answer that names no action is
    /// [`SelectorError::Unavailable`] with [`NO_ACTION`].
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        if candidates.is_empty() {
            return Err(SelectorError::NothingAdmissible);
        }
        let mut listed = "\nCandidate actions:\n".to_owned();
        for candidate in candidates {
            let status = match candidate.status {
                CatalogueEntryStatus::Admissible => "admissible",
                CatalogueEntryStatus::ApprovalRequired => "needs approval",
            };
            // Writing to a String cannot fail.
            let _ = writeln!(listed, "- {} ({status})", candidate.action);
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
        let answer = self
            .briefing
            .select(self.model, &listed, schema)
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
pub struct ModelArguments<'m, B> {
    model: &'m dyn Model,
    briefing: B,
}

impl<'m, B> ModelArguments<'m, B> {
    /// A generator that asks `model`, telling it `briefing`.
    pub fn new(model: &'m dyn Model, briefing: B) -> Self {
        Self { model, briefing }
    }
}

/// Why [`ModelArguments`] has no arguments for the selected action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentsError {
    /// The model could not be asked or gave no usable call: Loom suspends for external
    /// availability.
    Unavailable(String),
    /// The model answered, and its answer cannot become the action's arguments. Nothing is
    /// proposed; a host may refuse the step and tell the model why instead of suspending.
    Refused(String),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why) | Self::Refused(why) => f.write_str(why),
        }
    }
}

impl<B: Briefing> ModelArguments<'_, B> {
    /// The arguments of `entry`, as [`Briefing::arguments`] gives them, in Commission's JSON.
    ///
    /// # Errors
    /// [`Briefing::arguments`]'s, and [`ArgumentsError::Unavailable`] for arguments Commission's
    /// JSON cannot carry.
    pub fn arguments(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<json::Value, ArgumentsError> {
        let answer = self.briefing.arguments(self.model, &entry.action)?;
        json::parse(&answer.to_string()).map_err(|error| {
            ArgumentsError::Unavailable(format!("the model's arguments are not JSON: {error:?}"))
        })
    }
}

impl<B: Briefing> ArgumentGenerator for ModelArguments<'_, B> {
    /// [`ModelArguments::arguments`], its error as text: Loom suspends on either kind. A host
    /// that refuses the step on a refused answer calls `arguments` itself.
    fn generate(
        &self,
        context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        self.arguments(context, entry)
            .map_err(|error| error.to_string())
    }
}

/// The arguments of the model's forced call of `tool`, asked with `instructions` and `text` and
/// sent through `briefing` ([`request`], then [`answer`]).
///
/// # Errors
/// A tool name that is no valid id, or why the model gave no usable call.
pub fn ask<B: Briefing + ?Sized>(
    model: &dyn Model,
    briefing: &B,
    instructions: &str,
    text: String,
    tool: &str,
    description: &str,
    schema: Value,
) -> Result<Value, String> {
    answer(
        model,
        briefing,
        &request(model, instructions, text, tool, description, schema)?,
        tool,
    )
}

/// A request for one forced call of `tool`, described as `description` with input schema
/// `schema`: `instructions`, and `text` as the one user item, naming the model's own name.
///
/// # Errors
/// A tool name that is no valid id.
pub fn request(
    model: &dyn Model,
    instructions: &str,
    text: String,
    tool: &str,
    description: &str,
    schema: Value,
) -> Result<TurnRequest, String> {
    let name = ToolName::new(tool).map_err(|error| error.to_string())?;
    let mut request = TurnRequest::new(model.provenance().model.as_str(), vec![Item::user(text)]);
    instructions.clone_into(&mut request.instructions);
    request.tools = vec![ToolSpec {
        name: name.clone(),
        description: description.to_owned(),
        input_schema: schema,
    }];
    request.tool_choice = ToolChoice::Named(name);
    Ok(request)
}

/// The arguments of the model's call of `tool` in its answer to `request`, sent through
/// `briefing` on a current-thread runtime made for it. The stream is dropped.
///
/// # Errors
/// The model's error, an outcome llm refuses for `request`, or no call of `tool`.
pub fn answer<B: Briefing + ?Sized>(
    model: &dyn Model,
    briefing: &B,
    request: &TurnRequest,
    tool: &str,
) -> Result<Value, String> {
    let name = ToolName::new(tool).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("no runtime for the model turn: {error}"))?;
    let cancel = Cancel::new();
    let mut sink = Discard;
    let outcome = runtime
        .block_on(briefing.send(model, request, &mut sink, &cancel))
        .map_err(|error| error.to_string())?;
    outcome
        .validate_for(request, model.provenance())
        .map_err(|error| error.to_string())?;
    outcome
        .tool_calls()
        .find(|call| call.name == name)
        .map(|call| call.arguments.clone())
        .ok_or_else(|| format!("the model did not call `{tool}`"))
}

/// A sink that drops the stream: only the turn's outcome is read.
struct Discard;

impl StreamSink for Discard {
    fn emit(&mut self, _event: StreamEvent) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }
}
