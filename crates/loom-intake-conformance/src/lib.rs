//! The Rust ESS conformance target for `b10x-loom-intake-slice`.
//!
//! [`IntakeTarget`] is the `ess_conformance::ConformanceTarget` the suite synthesized from
//! `ess/intake/` runs against, and [`run_suite`] runs one suite against it and returns the
//! `ess-conformance-report/2` document a caller reads the verdict from.
//!
//! Every command is answered by the slice's own stores over a real state directory, through the
//! obligations the generated model leaves to them:
//!
//! * `KeepResumeContext` and `DropResumeContext` by
//!   `b10x_loom_intake_slice::resume::ResumeContextStore` (`KeepResumeContextBehavior`,
//!   `DropResumeContextBehavior`);
//! * `RecordMergeHandoff` by `b10x_loom_intake_slice::handoff::MergeHandoffStore`
//!   (`RecordMergeHandoffBehavior`);
//! * the view `ResumeContexts` by `ResumeContextStore::list`, which must read every context file it
//!   finds;
//! * the view `MergeHandoffs` by `MergeHandoffStore::get`, for every request the scenario asked to
//!   record: the target is the only writer of its directory, so no other hand-off can be there.
//!
//! Each row is the generated view type (`intake.routing.ResumeContexts`, `MergeHandoffs`), its
//! state the one the generated lifecycle gives a stored record.
//!
//! An external `storage-failed` is forced by taking the scenario's state directory away for the
//! one invocation it is forced for: the store cannot take the directory's lock, and answers with
//! its own reason. The directory is put back afterwards, unchanged.
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the slice returned, and each event is the one it returned.
//!
//! Each scenario starts from a fresh state directory, an empty event log and no forced outcome, so
//! no observation of one scenario can satisfy another.

pub mod codec;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use b10x_loom_intake_slice::handoff::MergeHandoffStore;
use b10x_loom_intake_slice::resume::ResumeContextStore;
use ess_conformance::scenario::{CommandRef, ErrorRef, EventRef, OutcomeRef};
use ess_conformance::target::{
    ConformanceTarget, DeclaredErrorValue, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, ObservedEvent, RedeliveryRequest, ScenarioContext,
    SemanticCommandRequest, SemanticCommandResult, SemanticViewRequest, SemanticViewResult,
    TargetError, ViewRow,
};
use ess_conformance::{AdmittedSuite, CountReport, Runner};
use ess_primitives::consistency::ConsistencyToken;
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;
use intake_model::primitives::Uuid;
use intake_model::routing::obligations::{
    DropResumeContextBehavior, KeepResumeContextBehavior, RecordMergeHandoffBehavior,
};
use intake_model::routing::{
    ActionRequestId, AgentRevisionId, CaseId, CommissionId, DropResumeContext,
    DropResumeContextOutcome, KeepResumeContext, KeepResumeContextOutcome, MergeHandoff,
    MergeHandoffs, RecordMergeHandoff, RecordMergeHandoffOutcome, ResumeContext, ResumeContexts,
};

use crate::codec::Input;

/// The name this implementation is reported under.
pub const IMPLEMENTATION: &str = "b10x-loom-intake-slice";

const KEEP_RESUME_CONTEXT: &str = "intake.routing.KeepResumeContext";
const RESUME_CONTEXT_KEPT: &str = "intake.routing.ResumeContextKept";
const RESUME_CONTEXT_ALREADY_KEPT: &str = "intake.routing.ResumeContextAlreadyKept";
const RESUME_CONTEXT_CONFIG_AND_CATALOG: &str = "intake.routing.ResumeContextConfigAndCatalog";
const RESUME_CONTEXTS: &str = "intake.routing.ResumeContexts";
const DROP_RESUME_CONTEXT: &str = "intake.routing.DropResumeContext";
const RESUME_CONTEXT_NOT_FOUND: &str = "intake.routing.ResumeContextNotFound";
const RESUME_CONTEXT_STORAGE_FAILED: &str = "intake.routing.ResumeContextStorageFailed";
const RECORD_MERGE_HANDOFF: &str = "intake.routing.RecordMergeHandoff";
const MERGE_HANDED_OFF: &str = "intake.routing.MergeHandedOff";
const MERGE_HANDOFF_ALREADY_RECORDED: &str = "intake.routing.MergeHandoffAlreadyRecorded";
const HEAD_ALREADY_HANDED_OFF: &str = "intake.routing.HeadAlreadyHandedOff";
const MERGE_HANDOFF_STORAGE_FAILED: &str = "intake.routing.MergeHandoffStorageFailed";
const MERGE_HANDOFFS: &str = "intake.routing.MergeHandoffs";
const STORAGE_FAILED: &str = "storage-failed";

/// The commands whose `storage-failed` a scenario can force.
const STORAGE_COMMANDS: [&str; 3] = [
    KEEP_RESUME_CONTEXT,
    DROP_RESUME_CONTEXT,
    RECORD_MERGE_HANDOFF,
];

/// One scenario's state.
struct Live {
    /// The scenario's state directory, which both stores keep their files in.
    dir: PathBuf,
    /// The resume contexts, behind the obligation the specification leaves to the store.
    contexts: ResumeContextStore,
    /// The merge hand-offs, behind the obligation the specification leaves to the store.
    handoffs: MergeHandoffStore,
    /// Every action request id the scenario asked to record a hand-off for, which the view
    /// `MergeHandoffs` reads back.
    requests: BTreeSet<String>,
    /// Every event published in this scenario, in order.
    log: Vec<ObservedEvent>,
    /// The command whose next invocation finds the state directory gone.
    forced: Option<&'static str>,
    /// The invocations performed, which consistency tokens are minted from.
    sequence: u64,
}

impl Live {
    fn open(dir: PathBuf) -> Result<Self, TargetError> {
        let unavailable = |error: &dyn std::fmt::Display| {
            TargetError::unavailable(
                format!("opening the state directory {}", dir.display()),
                error.to_string(),
            )
        };
        let contexts = ResumeContextStore::open(&dir).map_err(|error| unavailable(&error))?;
        let handoffs = MergeHandoffStore::open(&dir).map_err(|error| unavailable(&error))?;
        Ok(Self {
            dir,
            contexts,
            handoffs,
            requests: BTreeSet::new(),
            log: Vec::new(),
            forced: None,
            sequence: 0,
        })
    }

    fn token(&mut self) -> ConsistencyToken {
        self.sequence += 1;
        ConsistencyToken::new(format!("seq:{}", self.sequence))
            .expect("`seq:` and decimal digits is a well-formed token")
    }

    /// Publishes `event`, returning it for the command's own result.
    fn publish(&mut self, event: ObservedEvent, correlation: &CorrelationId) -> ObservedEvent {
        let sequence = u64::try_from(self.log.len()).expect("the log fits") + 1;
        let event = event.in_activity(correlation.clone()).at(sequence);
        self.log.push(event.clone());
        event
    }

    /// Runs `invoke` on the stores, with the state directory taken away first when `command`'s
    /// `storage-failed` was forced, and put back after.
    fn invoke<T>(
        &mut self,
        command: &str,
        invoke: impl FnOnce(&mut Self) -> T,
    ) -> Result<T, TargetError> {
        if self.forced != Some(command) {
            return Ok(invoke(self));
        }
        self.forced = None;
        let away = self.dir.with_extension("away");
        let moved = |from: &Path, to: &Path| {
            fs::rename(from, to).map_err(|error| {
                TargetError::unavailable(
                    format!("forcing `{command}/{STORAGE_FAILED}`"),
                    format!("rename {} to {}: {error}", from.display(), to.display()),
                )
            })
        };
        moved(&self.dir, &away)?;
        let answer = invoke(self);
        moved(&away, &self.dir)?;
        Ok(answer)
    }
}

/// The conformance target over `b10x-loom-intake-slice`.
pub struct IntakeTarget {
    /// The directory each scenario's state directory is made under.
    root: PathBuf,
    /// The scenarios begun, which name their directories.
    begun: RefCell<u64>,
    live: RefCell<Option<Live>>,
}

impl IntakeTarget {
    /// A target that makes each scenario's state directory under `root`, and removes it when the
    /// scenario ends.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            begun: RefCell::new(0),
            live: RefCell::new(None),
        }
    }

    fn close(&self) {
        if let Some(live) = self.live.borrow_mut().take() {
            let _ = fs::remove_dir_all(&live.dir);
        }
    }
}

impl std::fmt::Debug for IntakeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntakeTarget")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// The scenario's state, or the error a call outside a scenario gets.
fn current<'a>(slot: &'a mut Option<Live>, operation: &str) -> Result<&'a mut Live, TargetError> {
    slot.as_mut().ok_or_else(|| {
        TargetError::unavailable(operation.to_owned(), "no scenario is open".to_owned())
    })
}

impl ConformanceTarget for IntakeTarget {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        Ok(ImplementationIdentity::new(
            IMPLEMENTATION,
            env!("CARGO_PKG_VERSION"),
        ))
    }

    fn begin_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        self.close();
        let number = {
            let mut begun = self.begun.borrow_mut();
            *begun += 1;
            *begun
        };
        let dir = self.root.join(format!("scenario-{number}"));
        *self.live.borrow_mut() = Some(Live::open(dir)?);
        Ok(())
    }

    fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        if request.caller.is_some() {
            return Err(TargetError::unsupported(
                format!("sending `{}` as a caller", request.command),
                "the specification declares no caller attribute, so this target holds no credential",
            ));
        }
        let command = request.command.to_string();
        let mut slot = self.live.borrow_mut();
        let live = current(&mut slot, &format!("invoking `{command}`"))?;
        let input = &request.input;
        let correlation = &request.correlation;
        let result = match command.as_str() {
            KEEP_RESUME_CONTEXT => keep_resume_context(live, input, correlation)?,
            DROP_RESUME_CONTEXT => drop_resume_context(live, input)?,
            RECORD_MERGE_HANDOFF => record_merge_handoff(live, input, correlation)?,
            other => {
                return Err(TargetError::unavailable(
                    format!("invoking `{other}`"),
                    "the specification declares no such command",
                ));
            }
        };
        Ok(match result {
            Some(result) => result.with_consistency(live.token()),
            None => SemanticCommandResult::undeclared(),
        })
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        let view = request.view.to_string();
        let mut slot = self.live.borrow_mut();
        let live = current(&mut slot, &format!("reading `{view}`"))?;
        match view.as_str() {
            RESUME_CONTEXTS => resume_contexts(live, &view),
            MERGE_HANDOFFS => merge_handoffs(live, &view),
            other => Err(TargetError::unavailable(
                format!("reading `{other}`"),
                "the specification declares no such view",
            )),
        }
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        let slot = self.live.borrow();
        Ok(slot
            .iter()
            .flat_map(|live| live.log.iter())
            .filter(|published| published.event == request.event)
            .cloned()
            .collect())
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        let forced = request.force.to_string();
        let undeclared = || {
            TargetError::unavailable(
                format!("forcing `{forced}`"),
                "the specification declares no such external outcome",
            )
        };
        let (command, outcome) = forced.split_once('/').ok_or_else(undeclared)?;
        let command = STORAGE_COMMANDS
            .into_iter()
            .find(|declared| *declared == command && outcome == STORAGE_FAILED)
            .ok_or_else(undeclared)?;
        let mut slot = self.live.borrow_mut();
        current(&mut slot, &format!("forcing `{forced}`"))?.forced = Some(command);
        Ok(())
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        Err(TargetError::unsupported(
            format!("redelivering `{}`", request.event),
            "the specification declares no binding that reacts to an event",
        ))
    }

    fn end_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        self.close();
        Ok(())
    }
}

fn parse<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Display,
{
    text.parse()
        .unwrap_or_else(|error| panic!("`{text}` is a well-formed reference: {error}"))
}

fn took(command: &str, outcome: &str) -> SemanticCommandResult {
    SemanticCommandResult::took(OutcomeRef::new(
        parse::<CommandRef>(command),
        parse(outcome),
    ))
}

fn error(name: &str) -> DeclaredErrorValue {
    DeclaredErrorValue::new(parse::<ErrorRef>(name))
}

fn event(name: &str) -> ObservedEvent {
    ObservedEvent::new(parse::<EventRef>(name))
}

/// The input of `KeepResumeContext`, or `None` when a value cannot be held by its declared type.
fn keep_input(input: &Input) -> Option<KeepResumeContext> {
    Some(KeepResumeContext {
        commission_id: CommissionId(codec::uuid(input, "commission_id")?),
        case_id: CaseId(codec::text(input, "case_id")?),
        agent_revision_id: AgentRevisionId(codec::uuid(input, "agent_revision_id")?),
        intent: codec::text(input, "intent")?,
        route: codec::route(input, "route")?,
        workspace: codec::optional_text(input, "workspace")?,
        test_command: codec::text_list(input, "test_command")?,
        test_paths: codec::text_list(input, "test_paths")?,
        regression_test_command: codec::optional_text(input, "regression_test_command")?,
        max_steps: codec::integer(input, "max_steps")?,
        confinement: codec::backend(input, "confinement")?,
        cgroup_root: codec::optional_text(input, "cgroup_root")?,
        model: codec::text(input, "model")?,
        catalog: codec::optional_text(input, "catalog")?,
        config: codec::optional_text(input, "config")?,
    })
}

fn keep_resume_context(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(command) = keep_input(input) else {
        return Ok(None);
    };
    let outcome = live
        .invoke(KEEP_RESUME_CONTEXT, |live| {
            live.contexts.keep_resume_context(command)
        })?
        .map_err(|unmet| {
            TargetError::unavailable(
                format!("invoking `{KEEP_RESUME_CONTEXT}`"),
                format!("{} is owed: {}", unmet.capability, unmet.source),
            )
        })?;
    Ok(Some(match outcome {
        KeepResumeContextOutcome::Kept {
            resume_context_kept,
        } => {
            let published = live.publish(
                event(RESUME_CONTEXT_KEPT)
                    .with(
                        "commission_id",
                        Node::Text(resume_context_kept.commission_id.0.0),
                    )
                    .with("case_id", Node::Text(resume_context_kept.case_id.0)),
                correlation,
            );
            took(KEEP_RESUME_CONTEXT, "kept").emitting(published)
        }
        KeepResumeContextOutcome::ConfigAndCatalog { error: refused } => {
            took(KEEP_RESUME_CONTEXT, "config-and-catalog").with_error(
                error(RESUME_CONTEXT_CONFIG_AND_CATALOG)
                    .with("commission_id", Node::Text(refused.commission_id.0.0)),
            )
        }
        KeepResumeContextOutcome::AlreadyKept { error: kept } => {
            took(KEEP_RESUME_CONTEXT, "already-kept").with_error(
                error(RESUME_CONTEXT_ALREADY_KEPT)
                    .with("commission_id", Node::Text(kept.commission_id.0.0)),
            )
        }
        KeepResumeContextOutcome::StorageFailed { error: failed } => {
            took(KEEP_RESUME_CONTEXT, STORAGE_FAILED).with_error(
                error(RESUME_CONTEXT_STORAGE_FAILED).with("reason", Node::Text(failed.reason)),
            )
        }
    }))
}

fn drop_resume_context(
    live: &mut Live,
    input: &Input,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(commission_id) = codec::uuid(input, "commission_id") else {
        return Ok(None);
    };
    let command = DropResumeContext {
        commission_id: CommissionId(commission_id),
    };
    let outcome = live
        .invoke(DROP_RESUME_CONTEXT, |live| {
            live.contexts.drop_resume_context(command)
        })?
        .map_err(|unmet| {
            TargetError::unavailable(
                format!("invoking `{DROP_RESUME_CONTEXT}`"),
                format!("{} is owed: {}", unmet.capability, unmet.source),
            )
        })?;
    Ok(Some(match outcome {
        DropResumeContextOutcome::Dropped => took(DROP_RESUME_CONTEXT, "dropped"),
        DropResumeContextOutcome::NotFound { error: missing } => {
            took(DROP_RESUME_CONTEXT, "not-found").with_error(
                error(RESUME_CONTEXT_NOT_FOUND)
                    .with("commission_id", Node::Text(missing.commission_id.0.0)),
            )
        }
        DropResumeContextOutcome::StorageFailed { error: failed } => {
            took(DROP_RESUME_CONTEXT, STORAGE_FAILED).with_error(
                error(RESUME_CONTEXT_STORAGE_FAILED).with("reason", Node::Text(failed.reason)),
            )
        }
    }))
}

fn record_merge_handoff(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let (Some(action_request_id), Some(commission_id), Some(workspace), Some(head)) = (
        codec::uuid(input, "action_request_id"),
        codec::uuid(input, "commission_id"),
        codec::text(input, "workspace"),
        codec::text(input, "head"),
    ) else {
        return Ok(None);
    };
    live.requests.insert(action_request_id.0.clone());
    let command = RecordMergeHandoff {
        action_request_id: ActionRequestId(action_request_id),
        commission_id: CommissionId(commission_id),
        workspace,
        head,
    };
    let outcome = live
        .invoke(RECORD_MERGE_HANDOFF, |live| {
            live.handoffs.record_merge_handoff(command)
        })?
        .map_err(|unmet| {
            TargetError::unavailable(
                format!("invoking `{RECORD_MERGE_HANDOFF}`"),
                format!("{} is owed: {}", unmet.capability, unmet.source),
            )
        })?;
    Ok(Some(match outcome {
        RecordMergeHandoffOutcome::Recorded { merge_handed_off } => {
            let published = live.publish(
                event(MERGE_HANDED_OFF)
                    .with(
                        "action_request_id",
                        Node::Text(merge_handed_off.action_request_id.0.0),
                    )
                    .with(
                        "commission_id",
                        Node::Text(merge_handed_off.commission_id.0.0),
                    )
                    .with("workspace", Node::Text(merge_handed_off.workspace))
                    .with("head", Node::Text(merge_handed_off.head)),
                correlation,
            );
            took(RECORD_MERGE_HANDOFF, "recorded").emitting(published)
        }
        RecordMergeHandoffOutcome::AlreadyRecorded { error: recorded } => {
            took(RECORD_MERGE_HANDOFF, "already-recorded").with_error(
                error(MERGE_HANDOFF_ALREADY_RECORDED).with(
                    "action_request_id",
                    Node::Text(recorded.action_request_id.0.0),
                ),
            )
        }
        RecordMergeHandoffOutcome::HeadAlreadyHandedOff { error: handed_off } => {
            took(RECORD_MERGE_HANDOFF, "head-already-handed-off").with_error(
                error(HEAD_ALREADY_HANDED_OFF)
                    .with("commission_id", Node::Text(handed_off.commission_id.0.0))
                    .with("head", Node::Text(handed_off.head)),
            )
        }
        RecordMergeHandoffOutcome::StorageFailed { error: failed } => {
            took(RECORD_MERGE_HANDOFF, STORAGE_FAILED).with_error(
                error(MERGE_HANDOFF_STORAGE_FAILED).with("reason", Node::Text(failed.reason)),
            )
        }
    }))
}

/// The view `ResumeContexts`: every context the store lists, in file name order, its state the one
/// the generated lifecycle gives a kept context. A context file the store cannot read fails the
/// read, naming it, rather than leaving a row out.
fn resume_contexts(live: &Live, view: &str) -> Result<SemanticViewResult, TargetError> {
    let listed = live.contexts.list().map_err(|error| {
        TargetError::unavailable(format!("reading `{view}`"), error.to_string())
    })?;
    if let Some(unreadable) = listed.unreadable.first() {
        return Err(TargetError::unavailable(
            format!("reading `{view}`"),
            unreadable.error.to_string(),
        ));
    }
    let rows = listed.contexts.into_iter().map(|data| {
        let context = ResumeContext::new(data);
        let state = context.state();
        let data = context.into_data();
        let row = ResumeContexts {
            commission_id: data.commission_id,
            case_id: data.case_id,
            agent_revision_id: data.agent_revision_id,
            intent: data.intent,
            workspace: data.workspace,
            test_command: data.test_command,
            test_paths: data.test_paths,
            regression_test_command: data.regression_test_command,
            max_steps: data.max_steps,
            confinement: data.confinement,
            cgroup_root: data.cgroup_root,
            model: data.model,
            catalog: data.catalog,
            config: data.config,
            state,
        };
        ViewRow::from([
            (
                "commission_id".to_owned(),
                Node::Text(row.commission_id.0.0),
            ),
            ("case_id".to_owned(), Node::Text(row.case_id.0)),
            (
                "agent_revision_id".to_owned(),
                Node::Text(row.agent_revision_id.0.0),
            ),
            ("intent".to_owned(), Node::Text(row.intent)),
            ("workspace".to_owned(), codec::optional(row.workspace)),
            ("test_command".to_owned(), codec::texts(&row.test_command)),
            ("test_paths".to_owned(), codec::texts(&row.test_paths)),
            (
                "regression_test_command".to_owned(),
                codec::optional(row.regression_test_command),
            ),
            ("max_steps".to_owned(), codec::number(row.max_steps)),
            (
                "confinement".to_owned(),
                codec::backend_name(row.confinement),
            ),
            ("cgroup_root".to_owned(), codec::optional(row.cgroup_root)),
            ("model".to_owned(), Node::Text(row.model)),
            ("catalog".to_owned(), codec::optional(row.catalog)),
            ("config".to_owned(), codec::optional(row.config)),
            ("state".to_owned(), codec::context_state(row.state)),
        ])
    });
    Ok(SemanticViewResult::of(rows))
}

/// The view `MergeHandoffs`: every hand-off the store reads back for a request the scenario asked
/// to record, in request order, its state the one the generated lifecycle gives a recorded
/// hand-off.
fn merge_handoffs(live: &Live, view: &str) -> Result<SemanticViewResult, TargetError> {
    let mut rows = Vec::new();
    for request in &live.requests {
        let read = live
            .handoffs
            .get(&ActionRequestId(Uuid(request.clone())))
            .map_err(|error| {
                TargetError::unavailable(format!("reading `{view}`"), error.to_string())
            })?;
        let Some(data) = read else {
            continue;
        };
        let handoff = MergeHandoff::new(data);
        let state = handoff.state();
        let data = handoff.into_data();
        let row = MergeHandoffs {
            action_request_id: data.action_request_id,
            commission_id: data.commission_id,
            workspace: data.workspace,
            head: data.head,
            state,
        };
        rows.push(ViewRow::from([
            (
                "action_request_id".to_owned(),
                Node::Text(row.action_request_id.0.0),
            ),
            (
                "commission_id".to_owned(),
                Node::Text(row.commission_id.0.0),
            ),
            ("workspace".to_owned(), Node::Text(row.workspace)),
            ("head".to_owned(), Node::Text(row.head)),
            ("state".to_owned(), codec::handoff_state(row.state)),
        ]));
    }
    Ok(SemanticViewResult::of(rows))
}

/// One run of a suite against [`IntakeTarget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executed {
    /// `ess-conformance-report/2`, canonical: the document the verdict is read from.
    pub report: String,
    /// The runner's per-scenario results with their diagnostics, for a reader of a red run.
    pub diagnostics: String,
}

/// Admit `suite` (the JSON `ess verify conform synthesize` writes), run it against an
/// [`IntakeTarget`] whose scenarios keep their state under `scratch`, and return the report.
///
/// # Errors
///
/// Returns the admission failure when the bytes are not an admissible suite, the failure to
/// create `scratch`, and the report failure when the run and the suite disagree.
pub fn run_suite(suite: &str, scratch: &Path) -> Result<Executed, String> {
    let admitted = AdmittedSuite::from_json(suite).map_err(|error| error.to_string())?;
    fs::create_dir_all(scratch)
        .map_err(|error| format!("create {}: {error}", scratch.display()))?;
    let target = IntakeTarget::new(scratch);
    let executed = Runner::for_suite(admitted.suite()).run_admitted(&admitted, &target);
    target.close();
    let report = CountReport::from_run(&executed, &admitted)
        .and_then(|report| report.to_canonical_json())
        .map_err(|error| error.to_string())?;
    let diagnostics =
        serde_json::to_string_pretty(&*executed).map_err(|error| error.to_string())?;
    Ok(Executed {
        report,
        diagnostics,
    })
}
