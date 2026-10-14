// SPDX-License-Identifier: Apache-2.0

//! A Loom workflow definition run as Commission's [`AgentExecutor`]
//! (`story:dw-workflow-definition-executor`).
//!
//! A definition ([`WorkflowDefinition`], the `loom.workflow` domain of `ess/`) names its protocol
//! and an ordered list of steps. A step is either deterministic (one action, literal arguments)
//! or a judgement (the actions a model may choose among, and instructions), and may carry `when`,
//! a frontier claim and the truth it must have, so a step is conditional on what Canon reports,
//! never on a model. Canon still decides admission and completion: the workflow only sequences
//! actions the frontier admits, and Commission revalidates every proposal, a deterministic one
//! included, before any effect.
//!
//! [`WorkflowExecutor`] takes, on each call, the first step after the last performed one whose
//! `when` holds on the frontier it is handed:
//!
//! - a deterministic step whose action the frontier lists (`Admissible` or `ApprovalRequired`) is
//!   proposed with its arguments, and no model is asked;
//! - a judgement step runs the governed loop ([`LoopExecutor`], [`Loom::run_loop`]) narrowed to the
//!   step's actions that the host's own narrowing (`LoopConfig::admits`) admits, so a step narrows
//!   the host's narrowing further and never widens it, with the step's instructions as the run's
//!   standing instructions; a call naming any other action is refused to the model by the loop,
//!   and a proposal of an action outside that set is never returned; a judgement step none of
//!   whose actions the host admits ends the call `NeedsHumanJudgment`;
//! - a step whose actions the frontier does not list ends the call `NeedsHumanJudgment`, naming the
//!   step and the frontier's reasons. The workflow never skips to a later step on its own.
//!
//! When no step is left, the call is `CompletedLocalReasoning`: the executor's reasoning ended, and
//! completion is still the governor's.
//!
//! [`RecordingEffects`] wraps the host's effect port and records each admitted request it is
//! handed with its [`EffectOutcome`] in an [`EffectLog`] the executor reads: the workflow advances
//! past a step only once the request that step proposed was `Performed`. A refused or stale step
//! is proposed again on the next call, bounded by Commission's idle rule. Where the workflow stands
//! is kept in process (`decision-blocker:dw-run-state-across-restart`).
//!
//! A step may declare typed outputs (`story:dw-typed-step-outputs`): each a [`StepOutput`], a
//! name, a JSON pointer (RFC 6901) into the report of the step's `Performed` effect and an
//! [`OutputType`]. When the executor sees the step performed it takes every output from that
//! report and checks its type, and keeps them for the run, in process
//! ([`WorkflowExecutor::outputs`]). A later deterministic step's arguments may hold, at any
//! position, `{"$step": <step>, "$output": <name>}`; the reference is resolved when the step is
//! proposed, and the resolved arguments are the proposal's, revalidated by Commission like any
//! other. No model supplies an output. An output whose pointer is absent from the report, or whose
//! value has another type, ends that call and every later one `NeedsHumanJudgment` naming the step
//! and the output, and nothing more is proposed; a reference to an output its step did not produce
//! in this run (the step was skipped) ends the call `NeedsHumanJudgment` naming the referring step
//! and the output. The workflow fails toward less effect, never toward a guessed value.
//!
//! Metered ([`WorkflowExecutor::with_meter`], under a [`crate::metering::MeteredExecutor`] over the
//! same [`RunMeter`]), each call labels its step of the meter's report with the name of the
//! workflow step it took; a call that took no step leaves its step unlabelled.
//!
//! A definition is read from YAML into the generated types and checked before any run ([`load`]):
//! every step action is an action of its protocol, every step name is unique and neither begins
//! nor ends with whitespace, and a judgement step
//! names at least one action; every output a step declares has a unique name and a well-formed
//! pointer, and every argument reference names a declared output of an earlier step; a refusal
//! names the step. The protocol's actions come from the host
//! (`loom_protocols::ProtocolEntry::actions`), so this crate does not depend on the catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionStatus, Commission, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeNeedsHumanJudgment, ExecutorOutcomeProposedAction, Frontier,
    HumanDecisionRequest, ProposedActionArguments, Truth as FrontierTruth, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;

use crate::Loom;
use crate::harness::governed::{LoopExecutor, tool_name};
use crate::harness::turn_loop::LoopConfig;
use crate::harness::wire::{ModelPort, StreamSink, TurnOutcome, TurnRequest, WireError, WireId};
use crate::metering::RunMeter;
use crate::model::json::Value as LoomValue;
use crate::model::run::SessionData;
use crate::model::workflow::{
    DeterministicStep, JudgementStep, OutputType, StepCondition, StepKind, StepOutput, Truth,
    WorkflowDefinition, WorkflowStep,
};

/// The member of an argument reference naming the step whose output it takes.
pub const REFERENCE_STEP: &str = "$step";

/// The member of an argument reference naming the output it takes.
pub const REFERENCE_OUTPUT: &str = "$output";

/// The largest definition [`load`] reads, in bytes.
pub const MAX_DEFINITION_BYTES: usize = 1024 * 1024;

/// Why a definition was refused at load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionError {
    /// The text is larger than [`MAX_DEFINITION_BYTES`], is not YAML, or does not have the shape
    /// of a `loom.workflow.WorkflowDefinition`: a member missing, of the wrong type, or one no
    /// type declares. Names the member.
    Shape(String),
    /// The host's catalog holds no protocol of this name.
    Protocol(String),
    /// The definition names no step.
    NoSteps,
    /// A step the check refuses, by name, and why.
    Step {
        /// The step's name.
        step: String,
        /// Why it is refused.
        why: String,
    },
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(why) => write!(f, "the workflow definition is malformed: {why}"),
            Self::Protocol(name) => write!(f, "the host's catalog holds no protocol `{name}`"),
            Self::NoSteps => write!(f, "the workflow definition names no step"),
            Self::Step { step, why } => write!(f, "step `{step}`: {why}"),
        }
    }
}

impl std::error::Error for DefinitionError {}

/// Reads `text`, a YAML workflow definition, into the generated types ([`parse`]) and checks it
/// against the actions of the protocol it names, which `actions` answers from the host's catalog
/// ([`check`]).
///
/// # Errors
///
/// What [`parse`] refuses; [`DefinitionError::Protocol`] for a protocol `actions` does not know;
/// what [`check`] refuses.
pub fn load(
    text: &str,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<WorkflowDefinition, DefinitionError> {
    let definition = parse(text)?;
    let declared = actions(&definition.protocol)
        .ok_or_else(|| DefinitionError::Protocol(definition.protocol.clone()))?;
    check(&definition, &declared)?;
    Ok(definition)
}

/// Reads `text`, a YAML workflow definition, into the generated types. A member no type
/// declares is refused, so a misspelt key is never silently ignored; `when` may be absent or
/// `null`.
///
/// # Errors
///
/// [`DefinitionError::Shape`], naming the member.
pub fn parse(text: &str) -> Result<WorkflowDefinition, DefinitionError> {
    if text.len() > MAX_DEFINITION_BYTES {
        return Err(DefinitionError::Shape(format!(
            "the definition is larger than {MAX_DEFINITION_BYTES} bytes"
        )));
    }
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)
        .map_err(|error| DefinitionError::Shape(format!("not YAML: {error}")))?;
    decode::definition(&yaml).map_err(DefinitionError::Shape)
}

/// Checks `definition` against `actions`, the actions its protocol declares: it names a step;
/// every step name is non-empty, neither begins nor ends with whitespace, and is unique; every action a step names is one of `actions`; and a
/// judgement step names at least one action, none twice; every output a step declares has a
/// non-empty name without surrounding whitespace, unique within the step, and a well-formed JSON
/// pointer; and every argument reference (`{"$step": …, "$output": …}`, exactly those two string
/// members) names an output declared by an earlier step.
///
/// # Errors
///
/// [`DefinitionError::NoSteps`], or [`DefinitionError::Step`] naming the first step refused.
pub fn check(definition: &WorkflowDefinition, actions: &[String]) -> Result<(), DefinitionError> {
    if definition.steps.is_empty() {
        return Err(DefinitionError::NoSteps);
    }
    let mut names = BTreeSet::new();
    // The outputs each earlier step declares, by step name.
    let mut earlier: BTreeMap<&str, &[StepOutput]> = BTreeMap::new();
    for step in &definition.steps {
        let refused = |why: String| {
            Err(DefinitionError::Step {
                step: step.name.clone(),
                why,
            })
        };
        if step.name.trim().is_empty() {
            return refused("the step has no name".to_owned());
        }
        if step.name.trim() != step.name {
            // A name is compared as written, so `inspect` and `inspect ` would be two steps that
            // read as one: surrounding whitespace is refused rather than silently trimmed.
            return refused("the step's name begins or ends with whitespace".to_owned());
        }
        if !names.insert(step.name.as_str()) {
            return refused("another step has the same name".to_owned());
        }
        let named = step_actions(step);
        if named.is_empty() {
            return refused("a judgement step names no action".to_owned());
        }
        let mut seen = BTreeSet::new();
        for action in &named {
            if !actions.iter().any(|declared| declared == action) {
                return refused(format!(
                    "`{action}` is not an action of protocol `{}`",
                    definition.protocol
                ));
            }
            if !seen.insert(action.as_str()) {
                return refused(format!("`{action}` is named twice"));
            }
            if let Err(error) = tool_name(action) {
                return refused(format!("`{action}` cannot be offered as a tool: {error}"));
            }
        }
        if let Err(why) = check_outputs(&step.outputs) {
            return refused(why);
        }
        if let StepKind::Deterministic(DeterministicStep { arguments, .. }) = &step.kind
            && let Err(why) = check_references(arguments, definition, &earlier)
        {
            return refused(why);
        }
        earlier.insert(step.name.as_str(), &step.outputs);
    }
    Ok(())
}

/// Every output has a non-empty name without surrounding whitespace, unique among `outputs`, and a
/// well-formed JSON pointer.
fn check_outputs(outputs: &[StepOutput]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for output in outputs {
        let name = &output.name;
        if name.trim().is_empty() {
            return Err("an output has no name".to_owned());
        }
        if name.trim() != name {
            return Err(format!(
                "output `{name}`'s name begins or ends with whitespace"
            ));
        }
        if !seen.insert(name.as_str()) {
            return Err(format!("output `{name}` is declared twice"));
        }
        if pointer_tokens(&output.pointer).is_none() {
            return Err(format!(
                "output `{name}`'s pointer `{}` is not a JSON pointer (RFC 6901): it is empty or \
                 begins with `/`, and `~` is followed by `0` or `1`",
                output.pointer
            ));
        }
    }
    Ok(())
}

/// Every argument reference inside `arguments` is well formed and names an output `earlier`, the
/// steps before this one, declares.
fn check_references(
    arguments: &LoomValue,
    definition: &WorkflowDefinition,
    earlier: &BTreeMap<&str, &[StepOutput]>,
) -> Result<(), String> {
    match arguments {
        LoomValue::Array(items) => items
            .iter()
            .try_for_each(|item| check_references(item, definition, earlier)),
        LoomValue::Object(members) => match reference(members)? {
            Some((step, output)) => {
                let Some(outputs) = earlier.get(step) else {
                    return Err(if definition.steps.iter().any(|known| known.name == step) {
                        format!(
                            "an argument names an output of step `{step}`, which is not an \
                                 earlier step"
                        )
                    } else {
                        format!(
                            "an argument names an output of step `{step}`, and no step has that name"
                        )
                    });
                };
                if outputs.iter().any(|declared| declared.name == output) {
                    Ok(())
                } else {
                    Err(format!(
                        "an argument names output `{output}` of step `{step}`, which declares no \
                         such output"
                    ))
                }
            }
            None => members
                .iter()
                .try_for_each(|(_, item)| check_references(item, definition, earlier)),
        },
        _ => Ok(()),
    }
}

/// Whether `members`, an object of a step's arguments, is an argument reference: `Ok(None)` when
/// it has neither `$step` nor `$output`; the step and the output it names when it has exactly
/// those two members, each a string; refused otherwise, so a malformed reference is never taken as
/// a literal.
fn reference(members: &[(String, LoomValue)]) -> Result<Option<(&str, &str)>, String> {
    let marked = |key: &str| key == REFERENCE_STEP || key == REFERENCE_OUTPUT;
    if !members.iter().any(|(key, _)| marked(key)) {
        return Ok(None);
    }
    let named = |wanted: &str| {
        let mut found = members.iter().filter(|(key, _)| key == wanted);
        match (found.next(), found.next()) {
            (Some((_, LoomValue::Text(name))), None) => Some(name.as_str()),
            _ => None,
        }
    };
    match (named(REFERENCE_STEP), named(REFERENCE_OUTPUT)) {
        (Some(step), Some(output)) if members.len() == 2 => Ok(Some((step, output))),
        _ => Err(format!(
            "an argument reference has exactly the members `{REFERENCE_STEP}` and \
             `{REFERENCE_OUTPUT}`, each once and a string"
        )),
    }
}

/// The reference tokens of `pointer`, a JSON pointer (RFC 6901), unescaped: none for the empty
/// pointer, which names the whole document; `None` when it is not a JSON pointer.
fn pointer_tokens(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(Vec::new());
    }
    pointer
        .strip_prefix('/')?
        .split('/')
        .map(|token| {
            let mut unescaped = String::with_capacity(token.len());
            let mut chars = token.chars();
            while let Some(c) = chars.next() {
                match c {
                    '~' => match chars.next() {
                        Some('0') => unescaped.push('~'),
                        Some('1') => unescaped.push('/'),
                        _ => return None,
                    },
                    other => unescaped.push(other),
                }
            }
            Some(unescaped)
        })
        .collect()
}

/// The actions `step` names: its one action, or the actions a judgement may choose among.
fn step_actions(step: &WorkflowStep) -> Vec<String> {
    match &step.kind {
        StepKind::Deterministic(DeterministicStep { action, .. }) => vec![action.clone()],
        StepKind::Judgement(JudgementStep { actions, .. }) => actions.clone(),
    }
}

/// One admitted request an effect port was handed, and what it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEffect {
    /// The request, as the runtime admitted it.
    pub request: ActionRequestData,
    /// What the port answered.
    pub outcome: EffectOutcome,
}

/// The effects a [`RecordingEffects`] recorded, shared with the [`WorkflowExecutor`] that reads
/// them. Clones share one record.
#[derive(Debug, Clone, Default)]
pub struct EffectLog(Arc<Mutex<Vec<RecordedEffect>>>);

impl EffectLog {
    /// Every effect recorded so far, in the order recorded.
    pub fn records(&self) -> Vec<RecordedEffect> {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<RecordedEffect>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn len(&self) -> usize {
        self.lock().len()
    }

    /// The report of the first record from position `from` on that was `proposal`'s request,
    /// `Performed`; `None` when there is none.
    fn performed_since(
        &self,
        from: usize,
        proposal: &ExecutorOutcomeProposedAction,
    ) -> Option<CommissionValue> {
        self.lock()
            .iter()
            .skip(from)
            .find_map(|recorded| match &recorded.outcome {
                EffectOutcome::Performed(EffectOutcomePerformed { report, .. })
                    if recorded.request.action == proposal.action
                        && recorded.request.arguments == proposal.arguments =>
                {
                    Some(report.clone())
                }
                _ => None,
            })
    }
}

/// The host's effect port, recording each admitted request it is handed with its
/// [`EffectOutcome`]. A port that fails to answer records nothing.
#[derive(Debug)]
pub struct RecordingEffects<E> {
    inner: E,
    log: EffectLog,
}

impl<E: EffectPort> RecordingEffects<E> {
    /// Records the effects of `inner` into a new log.
    pub fn new(inner: E) -> Self {
        Self {
            inner,
            log: EffectLog::default(),
        }
    }

    /// The log this port records into, for the [`WorkflowExecutor`] of the same run.
    pub fn log(&self) -> EffectLog {
        self.log.clone()
    }

    /// The wrapped port.
    pub fn inner(&self) -> &E {
        &self.inner
    }
}

impl<E: EffectPort> EffectPort for RecordingEffects<E> {
    fn performs(&self, action: &str) -> bool {
        self.inner.performs(action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let outcome = self.inner.invoke(commission, request)?;
        self.log.lock().push(RecordedEffect {
            request: request.data().clone(),
            outcome: outcome.clone(),
        });
        Ok(outcome)
    }
}

/// Where a workflow stands in its run: the last step performed, the step proposed on the last
/// call that has not been seen performed yet, the outputs the performed steps produced (step,
/// output, value, in the order taken), and the output a performed step could not produce, which
/// ends every later call.
#[derive(Debug, Default)]
struct Position {
    performed: Option<usize>,
    outstanding: Option<Outstanding>,
    attempted: Vec<String>,
    outputs: Vec<(String, String, CommissionValue)>,
    unproduced: Option<Unproduced>,
}

/// An output a performed step declared and its report did not hold as declared.
#[derive(Debug, Clone)]
struct Unproduced {
    step: usize,
    output: String,
    why: String,
}

/// A step proposed and not yet seen performed: what was proposed, and how many effects were
/// recorded when it was.
#[derive(Debug)]
struct Outstanding {
    step: usize,
    proposal: ExecutorOutcomeProposedAction,
    recorded: usize,
}

/// A workflow definition as Commission's [`AgentExecutor`] (see the module docs). Judgement steps
/// run `loom`'s governed loop on `model`, recording into `session`; runs take the model one at a
/// time.
pub struct WorkflowExecutor<'l, S, G, V, M> {
    loom: &'l Loom<S, G, V>,
    model: Mutex<M>,
    config: LoopConfig,
    session: SessionData,
    definition: WorkflowDefinition,
    effects: EffectLog,
    position: Mutex<Position>,
    meter: Option<RunMeter>,
}

impl<'l, S, G, V, M> WorkflowExecutor<'l, S, G, V, M> {
    /// An executor running `definition`, a definition [`load`] checked, whose judgement steps run
    /// `loom`'s loop on `model` under `config` (the step's instructions replace its
    /// instructions, and its actions narrow it), recording into `session`. It advances past a step
    /// once `effects`, the log of the run's [`RecordingEffects`], records that step's request
    /// `Performed`.
    pub fn new(
        loom: &'l Loom<S, G, V>,
        model: M,
        config: LoopConfig,
        session: SessionData,
        definition: WorkflowDefinition,
        effects: EffectLog,
    ) -> Self {
        Self {
            loom,
            model: Mutex::new(model),
            config,
            session,
            definition,
            effects,
            position: Mutex::default(),
            meter: None,
        }
    }

    /// Labels each call's step of `meter` with the name of the workflow step the call took
    /// ([`RunMeter::label_step`]). The step is the one the run's [`crate::metering::MeteredExecutor`],
    /// wrapping this executor over the same meter, opened for the call.
    #[must_use]
    pub fn with_meter(mut self, meter: &RunMeter) -> Self {
        self.meter = Some(meter.clone());
        self
    }

    /// The definition this executor runs.
    pub fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }

    /// The name of the step each call took, in call order: one per call that took a step, a call
    /// that found none left adding nothing.
    pub fn attempted(&self) -> Vec<String> {
        self.position().attempted.clone()
    }

    /// Every output the run's performed steps produced, as `(step, output, value)`, in step order
    /// and, within a step, in the order the step declares them. A step whose outputs could not all
    /// be taken adds none.
    pub fn outputs(&self) -> Vec<(String, String, CommissionValue)> {
        self.position().outputs.clone()
    }

    /// The model, given back.
    pub fn into_model(self) -> M {
        self.model
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn position(&self) -> MutexGuard<'_, Position> {
        self.position.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<S, G, V, M> AgentExecutor for WorkflowExecutor<'_, S, G, V, M>
where
    V: Deref,
    V::Target: Governor,
    M: ModelPort,
{
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut position = self.position();
        if let Some(outstanding) = position.outstanding.take()
            && let Some(report) = self
                .effects
                .performed_since(outstanding.recorded, &outstanding.proposal)
        {
            position.performed = Some(outstanding.step);
            let step = &self.definition.steps[outstanding.step];
            match produce(step, &report) {
                Ok(outputs) => position.outputs.extend(outputs),
                Err(unproduced) => {
                    position.unproduced = Some(Unproduced {
                        step: outstanding.step,
                        ..unproduced
                    });
                }
            }
        }
        if frontier.data().case_id != commission.data().case_id {
            return ExecutorOutcome::NoUsefulAction(Unit(true));
        }
        if let Some(Unproduced { step, output, why }) = position.unproduced.clone() {
            // The step was performed, so it is never proposed again; nothing after it can be
            // taken without the value it did not produce.
            let step = &self.definition.steps[step];
            return self.needs_judgment(step, &step_actions(step), frontier, &why, Some(&output));
        }
        let from = position.performed.map_or(0, |at| at + 1);
        let Some((index, step)) = self
            .definition
            .steps
            .iter()
            .enumerate()
            .skip(from)
            .find(|(_, step)| holds(step.when.as_ref(), frontier))
        else {
            return ExecutorOutcome::CompletedLocalReasoning(Unit(true));
        };
        position.attempted.push(step.name.clone());
        if let Some(meter) = &self.meter {
            meter.label_step(&step.name);
        }
        let named = self.offered(step);
        if named.is_empty() {
            return self.needs_judgment(
                step,
                &step_actions(step),
                frontier,
                "the host's narrowing admits none of the step's actions",
                None,
            );
        }
        if !named.iter().any(|action| listed(frontier, action)) {
            return self.needs_judgment(
                step,
                &named,
                frontier,
                "the frontier does not list the step's action as admissible or approval-required",
                None,
            );
        }
        let recorded = self.effects.len();
        let outcome = match &step.kind {
            StepKind::Deterministic(DeterministicStep { action, arguments }) => {
                match resolve(arguments, &position.outputs) {
                    Ok(arguments) => {
                        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                            action: action.clone(),
                            arguments: ProposedActionArguments(arguments),
                        })
                    }
                    Err((from, output)) => {
                        return self.needs_judgment(
                            step,
                            &named,
                            frontier,
                            &format!(
                                "step `{from}` produced no output `{output}` in this run, and an \
                                 argument names it"
                            ),
                            Some(&output),
                        );
                    }
                }
            }
            StepKind::Judgement(judgement) => {
                // The position is not held while the model works: nothing else reads it then.
                drop(position);
                let outcome = self.judge(judgement, &named, commission, frontier);
                position = self.position();
                outcome
            }
        };
        match outcome {
            ExecutorOutcome::ProposedAction(proposal) => {
                if !named.contains(&proposal.action) {
                    // The loop's narrowing keeps this from happening; a proposal outside the step
                    // is never returned, whatever produced it.
                    return ExecutorOutcome::NoUsefulAction(Unit(true));
                }
                position.outstanding = Some(Outstanding {
                    step: index,
                    proposal: proposal.clone(),
                    recorded,
                });
                ExecutorOutcome::ProposedAction(proposal)
            }
            other => other,
        }
    }
}

impl<S, G, V, M> WorkflowExecutor<'_, S, G, V, M>
where
    V: Deref,
    V::Target: Governor,
    M: ModelPort,
{
    /// The governed loop on the model, narrowed to `offered` (the judgement's actions the host's
    /// narrowing admits, [`Self::offered`]), under the judgement's instructions.
    fn judge(
        &self,
        judgement: &JudgementStep,
        offered: &[String],
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        // `check` refused an action that is no tool name, so every action is offered.
        let admits = offered
            .iter()
            .filter_map(|action| tool_name(action).ok())
            .collect();
        let mut config = self.config.clone().with_admitted(Some(admits));
        config.instructions.clone_from(&judgement.instructions);
        let mut model = self.model.lock().unwrap_or_else(PoisonError::into_inner);
        LoopExecutor::new(self.loom, Lent(&mut *model), config, self.session.clone())
            .run(commission, frontier)
    }

    /// The actions `step` may propose: a deterministic step's one action; a judgement step's actions
    /// the host's narrowing (`LoopConfig::admits`) admits, in the step's order. A step only narrows
    /// the host's narrowing further, never widens it; with no narrowing, every action of the step.
    fn offered(&self, step: &WorkflowStep) -> Vec<String> {
        let named = step_actions(step);
        match (&step.kind, &self.config.admits) {
            (StepKind::Judgement(_), Some(host)) => named
                .into_iter()
                .filter(|action| tool_name(action).is_ok_and(|tool| host.contains(&tool)))
                .collect(),
            _ => named,
        }
    }

    /// `NeedsHumanJudgment` for `step`, whose `actions` cannot be taken, and `why`: the workflow,
    /// the step, its actions, why, and the frontier's reasons, with the output it concerns when an
    /// output is why.
    fn needs_judgment(
        &self,
        step: &WorkflowStep,
        actions: &[String],
        frontier: &Frontier<frontier_state::Issued>,
        why: &str,
        output: Option<&str>,
    ) -> ExecutorOutcome {
        let reasons: Vec<CommissionValue> = frontier
            .data()
            .actions
            .iter()
            .filter(|listed| actions.contains(&listed.action))
            .flat_map(|listed| listed.reasons.iter().cloned().map(CommissionValue::Text))
            .collect();
        let text = |value: &str| CommissionValue::Text(value.to_owned());
        let mut members = vec![
            ("workflow".to_owned(), text(&self.definition.name)),
            ("step".to_owned(), text(&step.name)),
            (
                "actions".to_owned(),
                CommissionValue::Array(actions.iter().map(|action| text(action)).collect()),
            ),
            ("why".to_owned(), text(why)),
            ("reasons".to_owned(), CommissionValue::Array(reasons)),
        ];
        if let Some(output) = output {
            members.push(("output".to_owned(), text(output)));
        }
        ExecutorOutcome::NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment {
            request: HumanDecisionRequest(CommissionValue::Object(members)),
        })
    }
}

/// Whether `frontier` lists `action` as `Admissible` or `ApprovalRequired`.
fn listed(frontier: &Frontier<frontier_state::Issued>, action: &str) -> bool {
    frontier.data().actions.iter().any(|listed| {
        listed.action == action
            && matches!(
                listed.status,
                ActionStatus::Admissible | ActionStatus::ApprovalRequired
            )
    })
}

/// Whether `when` holds on `frontier`: no condition, or the frontier reports the claim with the
/// truth it names. A claim the frontier does not report does not hold.
fn holds(when: Option<&StepCondition>, frontier: &Frontier<frontier_state::Issued>) -> bool {
    let Some(condition) = when else {
        return true;
    };
    let wanted = match condition.value {
        Truth::True => FrontierTruth::True,
        Truth::False => FrontierTruth::False,
        Truth::Unknown => FrontierTruth::Unknown,
    };
    frontier
        .data()
        .claims
        .iter()
        .any(|claim| claim.claim == condition.claim && claim.value == wanted)
}

/// A step's literal arguments as Commission's JSON, value for value.
fn commission_value(value: &LoomValue) -> CommissionValue {
    match value {
        LoomValue::Null => CommissionValue::Null,
        LoomValue::Bool(flag) => CommissionValue::Bool(*flag),
        LoomValue::Number(number) => CommissionValue::Number(number.clone()),
        LoomValue::Text(text) => CommissionValue::Text(text.clone()),
        LoomValue::Array(items) => {
            CommissionValue::Array(items.iter().map(commission_value).collect())
        }
        LoomValue::Object(members) => CommissionValue::Object(
            members
                .iter()
                .map(|(key, value)| (key.clone(), commission_value(value)))
                .collect(),
        ),
    }
}

/// Every output `step` declares, taken from `report`, its performed effect's report, at its
/// pointer and checked against its type: all of them, in declared order, or the first that is
/// absent or of another type (its `step` is filled in by the caller).
fn produce(
    step: &WorkflowStep,
    report: &CommissionValue,
) -> Result<Vec<(String, String, CommissionValue)>, Unproduced> {
    step.outputs
        .iter()
        .map(|output| {
            let unproduced = |why: String| Unproduced {
                step: 0,
                output: output.name.clone(),
                why,
            };
            // `check` refused a pointer that is not one.
            let tokens = pointer_tokens(&output.pointer).ok_or_else(|| {
                unproduced(format!(
                    "output `{}`'s pointer is not a JSON pointer",
                    output.name
                ))
            })?;
            let value = at_pointer(report, &tokens).ok_or_else(|| {
                unproduced(format!(
                    "the report of step `{}` holds nothing at `{}` for output `{}`",
                    step.name, output.pointer, output.name
                ))
            })?;
            if !has_type(value, output.r#type) {
                return Err(unproduced(format!(
                    "the report of step `{}` holds {} at `{}`, and output `{}` is declared {:?}",
                    step.name,
                    value.describes(),
                    output.pointer,
                    output.name,
                    output.r#type
                )));
            }
            Ok((step.name.clone(), output.name.clone(), value.clone()))
        })
        .collect()
}

/// The value `tokens` reach in `value`: an object member by name, an array item by an index
/// without leading zeros. A member named twice reaches nothing, so an ambiguous report is never
/// read as either of its values.
fn at_pointer<'v>(value: &'v CommissionValue, tokens: &[String]) -> Option<&'v CommissionValue> {
    tokens.iter().try_fold(value, |value, token| match value {
        CommissionValue::Object(members) => {
            let mut found = members.iter().filter(|(key, _)| key == token);
            match (found.next(), found.next()) {
                (Some((_, item)), None) => Some(item),
                _ => None,
            }
        }
        CommissionValue::Array(items) => {
            let canonical = token == "0"
                || (!token.starts_with('0') && token.chars().all(|c| c.is_ascii_digit()));
            if token.is_empty() || !canonical {
                return None;
            }
            items.get(token.parse::<usize>().ok()?)
        }
        _ => None,
    })
}

/// Whether `value` has the declared type: an `Integer` is a number written with digits only, an
/// optional leading `-` and no leading zero, within 64 bits; a list is an array every item of
/// which has the item type; `Json` is any value.
fn has_type(value: &CommissionValue, declared: OutputType) -> bool {
    let items = |item: OutputType| match value {
        CommissionValue::Array(items) => items.iter().all(|value| has_type(value, item)),
        _ => false,
    };
    match declared {
        OutputType::String => matches!(value, CommissionValue::Text(_)),
        OutputType::Integer => match value {
            CommissionValue::Number(spelling) => {
                let digits = spelling.strip_prefix('-').unwrap_or(spelling);
                !digits.is_empty()
                    && digits.chars().all(|c| c.is_ascii_digit())
                    && (digits == "0" || !digits.starts_with('0'))
                    && spelling.parse::<i64>().is_ok()
            }
            _ => false,
        },
        OutputType::Boolean => matches!(value, CommissionValue::Bool(_)),
        OutputType::Json => true,
        OutputType::StringList => items(OutputType::String),
        OutputType::IntegerList => items(OutputType::Integer),
        OutputType::BooleanList => items(OutputType::Boolean),
        OutputType::JsonList => items(OutputType::Json),
    }
}

/// A deterministic step's arguments as Commission's JSON, every argument reference replaced by the
/// value of the output it names, taken from `outputs`. The first reference to an output the run
/// has not produced is the error, as its step and output.
fn resolve(
    arguments: &LoomValue,
    outputs: &[(String, String, CommissionValue)],
) -> Result<CommissionValue, (String, String)> {
    Ok(match arguments {
        LoomValue::Array(items) => CommissionValue::Array(
            items
                .iter()
                .map(|item| resolve(item, outputs))
                .collect::<Result<_, _>>()?,
        ),
        LoomValue::Object(members) => match reference(members) {
            // `check` refused a malformed reference, so `Err` does not happen; were it to, the
            // object is not taken as a literal.
            Err(_) => {
                return Err((String::new(), "a malformed argument reference".to_owned()));
            }
            Ok(Some((step, output))) => outputs
                .iter()
                .find(|(from, name, _)| from == step && name == output)
                .map(|(_, _, value)| value.clone())
                .ok_or_else(|| (step.to_owned(), output.to_owned()))?,
            Ok(None) => CommissionValue::Object(
                members
                    .iter()
                    .map(|(key, item)| resolve(item, outputs).map(|value| (key.clone(), value)))
                    .collect::<Result<_, _>>()?,
            ),
        },
        literal => commission_value(literal),
    })
}

/// The executor's model, lent to one judgement step's loop.
struct Lent<'m, M>(&'m mut M);

impl<M: ModelPort> ModelPort for Lent<'_, M> {
    fn wire(&self) -> &WireId {
        self.0.wire()
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.0.turn(request, sink)
    }
}

/// The YAML document into the generated types, member order kept.
mod decode {
    use serde_yaml_ng::{Mapping, Value};

    use crate::model::json::Value as LoomValue;
    use crate::model::workflow::{
        DeterministicStep, JudgementStep, OutputType, StepCondition, StepKind, StepOutput, Truth,
        WorkflowDefinition, WorkflowStep,
    };

    pub(super) fn definition(value: &Value) -> Result<WorkflowDefinition, String> {
        let members = object(value, "the definition")?;
        only(members, "the definition", &["name", "protocol", "steps"])?;
        let steps = list(members, "steps", "the definition")?
            .iter()
            .enumerate()
            .map(|(at, step)| self::step(step, at))
            .collect::<Result<_, _>>()?;
        Ok(WorkflowDefinition {
            name: text(members, "name", "the definition")?,
            protocol: text(members, "protocol", "the definition")?,
            steps,
        })
    }

    fn step(value: &Value, at: usize) -> Result<WorkflowStep, String> {
        let place = format!("steps[{at}]");
        let members = object(value, &place)?;
        let kind = text(members, "kind", &place)?;
        let kind = match kind.as_str() {
            "Deterministic" => {
                only(
                    members,
                    &place,
                    &["name", "when", "kind", "action", "arguments", "outputs"],
                )?;
                StepKind::Deterministic(DeterministicStep {
                    action: text(members, "action", &place)?,
                    arguments: json(
                        member(members, "arguments", &place)?,
                        &format!("{place}.arguments"),
                    )?,
                })
            }
            "Judgement" => {
                only(
                    members,
                    &place,
                    &["name", "when", "kind", "actions", "instructions", "outputs"],
                )?;
                let actions = list(members, "actions", &place)?
                    .iter()
                    .enumerate()
                    .map(|(index, action)| {
                        action
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| format!("{place}.actions[{index}] is not a string"))
                    })
                    .collect::<Result<_, _>>()?;
                StepKind::Judgement(JudgementStep {
                    actions,
                    instructions: text(members, "instructions", &place)?,
                })
            }
            other => {
                return Err(format!(
                    "{place}.kind is `{other}`; a step is `Deterministic` or `Judgement`"
                ));
            }
        };
        let when = match members.get("when") {
            None | Some(Value::Null) => None,
            Some(value) => Some(condition(value, &format!("{place}.when"))?),
        };
        // A step that declares no outputs may leave `outputs` out.
        let outputs = match members.get("outputs") {
            None | Some(Value::Null) => Vec::new(),
            Some(_) => list(members, "outputs", &place)?
                .iter()
                .enumerate()
                .map(|(index, value)| output(value, &format!("{place}.outputs[{index}]")))
                .collect::<Result<_, _>>()?,
        };
        Ok(WorkflowStep {
            name: text(members, "name", &place)?,
            when,
            kind,
            outputs,
        })
    }

    fn output(value: &Value, place: &str) -> Result<StepOutput, String> {
        let members = object(value, place)?;
        only(members, place, &["name", "pointer", "type"])?;
        let declared = match text(members, "type", place)?.as_str() {
            "String" => OutputType::String,
            "Integer" => OutputType::Integer,
            "Boolean" => OutputType::Boolean,
            "Json" => OutputType::Json,
            "StringList" => OutputType::StringList,
            "IntegerList" => OutputType::IntegerList,
            "BooleanList" => OutputType::BooleanList,
            "JsonList" => OutputType::JsonList,
            other => {
                return Err(format!(
                    "{place}.type is `{other}`; an output is `String`, `Integer`, `Boolean`, \
                     `Json`, `StringList`, `IntegerList`, `BooleanList` or `JsonList`"
                ));
            }
        };
        Ok(StepOutput {
            name: text(members, "name", place)?,
            pointer: text(members, "pointer", place)?,
            r#type: declared,
        })
    }

    fn condition(value: &Value, place: &str) -> Result<StepCondition, String> {
        let members = object(value, place)?;
        only(members, place, &["claim", "value"])?;
        let truth = match member(members, "value", place)? {
            Value::Bool(true) => Truth::True,
            Value::Bool(false) => Truth::False,
            Value::String(word) if word == "True" => Truth::True,
            Value::String(word) if word == "False" => Truth::False,
            Value::String(word) if word == "Unknown" => Truth::Unknown,
            _ => {
                return Err(format!("{place}.value is not `True`, `False` or `Unknown`"));
            }
        };
        Ok(StepCondition {
            claim: text(members, "claim", place)?,
            value: truth,
        })
    }

    fn object<'v>(value: &'v Value, place: &str) -> Result<&'v Mapping, String> {
        let members = value
            .as_mapping()
            .ok_or_else(|| format!("{place} is not a mapping"))?;
        match members.keys().find(|key| !key.is_string()) {
            Some(_) => Err(format!("{place} has a key that is not a string")),
            None => Ok(members),
        }
    }

    fn only(members: &Mapping, place: &str, declared: &[&str]) -> Result<(), String> {
        match members
            .keys()
            .filter_map(Value::as_str)
            .find(|key| !declared.contains(key))
        {
            Some(unknown) => Err(format!("{place} has `{unknown}`, which no type declares")),
            None => Ok(()),
        }
    }

    fn member<'v>(members: &'v Mapping, name: &str, place: &str) -> Result<&'v Value, String> {
        members
            .get(name)
            .ok_or_else(|| format!("{place} has no `{name}`"))
    }

    fn text(members: &Mapping, name: &str, place: &str) -> Result<String, String> {
        member(members, name, place)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{place}.{name} is not a string"))
    }

    fn list<'v>(members: &'v Mapping, name: &str, place: &str) -> Result<&'v Vec<Value>, String> {
        member(members, name, place)?
            .as_sequence()
            .ok_or_else(|| format!("{place}.{name} is not a list"))
    }

    /// A YAML value as the generated model's JSON: numbers keep their spelling, members their
    /// order. A tagged value, a non-finite number or a key that is not a string is refused.
    fn json(value: &Value, place: &str) -> Result<LoomValue, String> {
        Ok(match value {
            Value::Null => LoomValue::Null,
            Value::Bool(flag) => LoomValue::Bool(*flag),
            Value::Number(number) => {
                if number.as_f64().is_some_and(|float| !float.is_finite()) {
                    return Err(format!("{place} holds a number JSON cannot carry"));
                }
                LoomValue::Number(number.to_string())
            }
            Value::String(text) => LoomValue::Text(text.clone()),
            Value::Sequence(items) => LoomValue::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(at, item)| json(item, &format!("{place}[{at}]")))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Mapping(_) => LoomValue::Object(
                object(value, place)?
                    .iter()
                    .map(|(key, item)| {
                        // `object` refused a key that is not a string.
                        let key = key.as_str().unwrap_or_default().to_owned();
                        let item = json(item, &format!("{place}.{key}"))?;
                        Ok((key, item))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            Value::Tagged(_) => return Err(format!("{place} holds a tagged value")),
        })
    }
}
