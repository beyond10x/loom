//! The Rust ESS conformance target for `b10x-loom-executor`.
//!
//! [`LoomTarget`] is the `ess_conformance::ConformanceTarget` the suite synthesized from `ess/`
//! runs against, and [`run_suite`] runs one suite against it and returns the
//! `ess-conformance-report/2` document a caller reads the verdict from.
//!
//! Every `loom.run` command is answered by `b10x-loom-executor`, over two records:
//!
//! * `ProjectCatalogue` by the generated behaviour of
//!   `b10x_loom_executor::projection::CatalogueRecord`;
//! * `SelectAction` by `b10x_loom_executor::selection::select_action`, from the catalogues the
//!   `CatalogueRecord` holds into the selections of a `b10x_loom_executor::arguments::RequestRecord`;
//! * `RequestArguments`, `RevalidateSelection` and `OverruleSelection` by the behaviours of that
//!   `RequestRecord`; `OverruleSelection` by its generated behaviour, which overrules whatever
//!   replacement it is named, not by `RequestRecord::overrule`, the host's check that the
//!   replacement is held (ESS refuses that guard beside `wrong_state`, `ESS-COMMAND-004`);
//! * `RecordSelection` by the generated behaviour of that `RequestRecord`, over the selections it
//!   holds and the selection records it keeps;
//! * the views `Catalogues`, `Selections` and `SelectionRecords` from what the two records hold.
//!
//! The `loom.session` domain's `ProposeSessionChange` is answered by `b10x-loom-session-executor`,
//! which records a session only by running one: the target runs a scripted Claude Code session
//! through it, and the view `AgentSessions` reads the sessions it recorded (the `session` module
//! says how).
//!
//! The `loom.resource` domain's `ApplyLoomAgent`, `RecordCheck`, `ObserveDeployment` and
//! `DeleteLoomAgent`, and its view `LoomAgents`, are answered by `b10x-loom-resource`'s
//! `LoomAgentRecord`, which runs the generated behaviour of `loom.resource` over itself (the
//! `resource` module).
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the executor returned, and each event is the one it returned.
//! The `loom.agent` domain's `CheckAgentSpec` is answered by `b10x_loom_agent::check_text` over
//! the plugin catalog (`loom_protocols::ProtocolCatalog::plugins`): `serveable`, or the branch of
//! the refusal's reason with `SpecRefused` carrying its reason, member and detail. The target plays
//! the file system for its `path` (the `agent` module says how).
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the executor or the check returned, and each event is the one it
//! returned.
//!
//! # The external outcomes
//!
//! `CheckAgentSpec`'s eight refusals are `external:`; a scenario forcing one of them hands the
//! check the fixture holding that refusal's one defect (the `agent` module).
//!
//! `RevalidateSelection`'s `not-in-frontier` is `external:` in the specification, and the executor
//! answers it from the frontier action ids in the command's input, which a Loom reads from the
//! governor's current frontier. A scenario decides that answer with `configure_external_outcome`
//! instead, so this target plays the governor: the frontier action ids it hands the executor are the
//! input's, without the selection's action when the scenario forced `not-in-frontier` for this
//! invocation and with it otherwise. The executor decides the outcome from them, but over those
//! two lists only, so the suite does not exercise its membership rule; the comment in
//! `revalidate_selection` names the test that does.
//!
//! Each scenario starts from empty records, an empty event log and no forced outcome, so no
//! observation of one scenario can satisfy another. Consistency tokens come from a counter, so two
//! runs of one suite report the same thing.

mod agent;
pub mod codec;
mod resource;
mod session;

use std::cell::RefCell;

use b10x_loom_executor::arguments::RequestRecord;
use b10x_loom_executor::model::behaviour::SelectionStorage;
use b10x_loom_executor::model::run::obligations::{
    OverruleSelectionBehavior, ProjectCatalogueBehavior, RecordSelectionBehavior,
    RequestArgumentsBehavior, RevalidateSelectionBehavior,
};
use b10x_loom_executor::model::run::{
    ArgumentRequestId, CatalogueId, OverruleSelection, OverruleSelectionOutcome, ProjectCatalogue,
    ProjectCatalogueOutcome, RecordSelection, RecordSelectionOutcome, RequestArguments,
    RequestArgumentsOutcome, RevalidateSelection, RevalidateSelectionOutcome, SelectAction,
    SelectActionOutcome, SelectionId, SelectionRecordId, SelectionState, TurnId,
};
use b10x_loom_executor::projection::CatalogueRecord;
use b10x_loom_executor::selection::select_action;
use b10x_loom_resource::LoomAgentRecord;
use b10x_loom_session_executor::model::session::AgentSessions;
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

use crate::codec::Input;

/// The name this implementation is reported under.
pub const IMPLEMENTATION: &str = "b10x-loom-executor";

const PROJECT_CATALOGUE: &str = "loom.run.ProjectCatalogue";
const SELECT_ACTION: &str = "loom.run.SelectAction";
const REQUEST_ARGUMENTS: &str = "loom.run.RequestArguments";
const REVALIDATE_SELECTION: &str = "loom.run.RevalidateSelection";
const OVERRULE_SELECTION: &str = "loom.run.OverruleSelection";
const RECORD_SELECTION: &str = "loom.run.RecordSelection";

const CATALOGUE_PROJECTED: &str = "loom.run.CatalogueProjected";
const ACTION_SELECTED: &str = "loom.run.ActionSelected";
const ARGUMENTS_REQUESTED: &str = "loom.run.ArgumentsRequested";
const SELECTION_STALE: &str = "loom.run.SelectionStale";
const SELECTION_NOT_IN_FRONTIER: &str = "loom.run.SelectionNotInFrontier";
const SELECTION_ADMITTED: &str = "loom.run.SelectionAdmitted";
const SELECTION_OVERRULED: &str = "loom.run.SelectionOverruled";
const SELECTION_RECORDED: &str = "loom.run.SelectionRecorded";

const CATALOGUE_EXISTS: &str = "loom.run.CatalogueExists";
const CATALOGUE_NOT_FOUND: &str = "loom.run.CatalogueNotFound";
const ACTION_NOT_IN_CATALOGUE: &str = "loom.run.ActionNotInCatalogue";
const CATALOGUE_REVISION_MISMATCH: &str = "loom.run.CatalogueRevisionMismatch";
const SELECTION_EXISTS: &str = "loom.run.SelectionExists";
const SELECTION_NOT_FOUND: &str = "loom.run.SelectionNotFound";
const SELECTION_NOT_SELECTED: &str = "loom.run.SelectionNotSelected";
const SELECTION_STATE_CONFLICT: &str = "loom.run.SelectionStateConflict";
const SELECTION_RECORD_EXISTS: &str = "loom.run.SelectionRecordExists";

const CATALOGUES: &str = "loom.run.Catalogues";
const SELECTIONS: &str = "loom.run.Selections";
const SELECTION_RECORDS: &str = "loom.run.SelectionRecords";

/// The external outcome a scenario may force.
const NOT_IN_FRONTIER: &str = "not-in-frontier";

/// One scenario's state.
#[derive(Default)]
struct Live {
    /// Catalogues, behind the executor's behaviour.
    catalogues: CatalogueRecord,
    /// Selections and argument requests, behind the executor's behaviours.
    requests: RequestRecord,
    /// Every event published in this scenario, in order.
    log: Vec<ObservedEvent>,
    /// Whether the next `RevalidateSelection` is to find its action outside the frontier.
    not_in_frontier: bool,
    /// The refusal branch of `CheckAgentSpec` the scenario forced for the next check, whose
    /// fixture the target reads when the input's path names no file (the `agent` module).
    forced_check: Option<String>,
    /// The invocations performed, which consistency tokens are minted from.
    sequence: u64,
    /// The sessions the session executor recorded, one `AgentSessions` row each.
    sessions: Vec<AgentSessions>,
    /// `LoomAgent` resources, behind the generated behaviour of `loom.resource`.
    resources: LoomAgentRecord,
}

impl Live {
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
}

/// The conformance target over `b10x-loom-executor`.
#[derive(Default)]
pub struct LoomTarget {
    live: RefCell<Live>,
}

impl std::fmt::Debug for LoomTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoomTarget").finish_non_exhaustive()
    }
}

impl ConformanceTarget for LoomTarget {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        Ok(ImplementationIdentity::new(
            IMPLEMENTATION,
            env!("CARGO_PKG_VERSION"),
        ))
    }

    fn begin_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.live.borrow_mut() = Live::default();
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
        let mut live = self.live.borrow_mut();
        let command = request.command.to_string();
        let input = &request.input;
        let correlation = &request.correlation;
        let result = match command.as_str() {
            PROJECT_CATALOGUE => project_catalogue(&mut live, input, correlation),
            SELECT_ACTION => select(&mut live, input, correlation),
            REQUEST_ARGUMENTS => request_arguments(&mut live, input, correlation),
            REVALIDATE_SELECTION => revalidate_selection(&mut live, input, correlation),
            OVERRULE_SELECTION => overrule_selection(&mut live, input, correlation),
            RECORD_SELECTION => record_selection(&mut live, input, correlation),
            session::PROPOSE_SESSION_CHANGE => {
                session::propose_session_change(&mut live, input, correlation)?
            }
            resource::APPLY_LOOM_AGENT => resource::apply_loom_agent(&mut live, input, correlation),
            resource::RECORD_CHECK => resource::record_check(&mut live, input, correlation),
            resource::OBSERVE_DEPLOYMENT => {
                resource::observe_deployment(&mut live, input, correlation)
            }
            resource::DELETE_LOOM_AGENT => {
                resource::delete_loom_agent(&mut live, input, correlation)
            }
            agent::CHECK_AGENT_SPEC => agent::check_agent_spec(&mut live, input)?,
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
        let live = self.live.borrow();
        let rows: Vec<ViewRow> = match view.as_str() {
            CATALOGUES => live
                .catalogues
                .catalogues()
                .iter()
                .map(|held| {
                    ViewRow::from([
                        (
                            "catalogue_id".to_owned(),
                            codec::id(&held.data.catalogue_id.0),
                        ),
                        (
                            "case_revision".to_owned(),
                            codec::number(held.data.case_revision),
                        ),
                        ("state".to_owned(), codec::catalogue_state(held.state)),
                    ])
                })
                .collect(),
            SELECTIONS => live
                .requests
                .selections()
                .iter()
                .map(|held| {
                    ViewRow::from([
                        (
                            "selection_id".to_owned(),
                            codec::id(&held.data.selection_id.0),
                        ),
                        (
                            "catalogue_id".to_owned(),
                            codec::id(&held.data.catalogue_id.0),
                        ),
                        ("action".to_owned(), Node::Text(held.data.action.clone())),
                        (
                            "strategy".to_owned(),
                            codec::strategy_name(held.data.strategy),
                        ),
                        (
                            "case_revision".to_owned(),
                            codec::number(held.data.case_revision),
                        ),
                        (
                            "replaced_by".to_owned(),
                            held.data
                                .replaced_by
                                .as_ref()
                                .map_or(Node::Null, |id| codec::id(&id.0)),
                        ),
                        ("state".to_owned(), codec::selection_state(held.state)),
                    ])
                })
                .collect(),
            // The confidence is not published (`ess/domains/run.yaml`, `loom.run.SelectionRecords`).
            SELECTION_RECORDS => live
                .requests
                .selection_records()
                .iter()
                .map(|held| {
                    ViewRow::from([
                        (
                            "selection_record_id".to_owned(),
                            codec::id(&held.data.selection_record_id.0),
                        ),
                        (
                            "selection_id".to_owned(),
                            codec::id(&held.data.selection_id.0),
                        ),
                        (
                            "strategy".to_owned(),
                            codec::strategy_name(held.data.strategy),
                        ),
                        (
                            "candidate_count".to_owned(),
                            codec::number(held.data.candidate_count),
                        ),
                        (
                            "chosen_action".to_owned(),
                            Node::Text(held.data.chosen_action.clone()),
                        ),
                        (
                            "fell_back_to".to_owned(),
                            held.data
                                .fell_back_to
                                .map_or(Node::Null, codec::strategy_name),
                        ),
                        ("latency_ms".to_owned(), codec::number(held.data.latency_ms)),
                        (
                            "input_tokens".to_owned(),
                            codec::number(held.data.input_tokens),
                        ),
                        (
                            "output_tokens".to_owned(),
                            codec::number(held.data.output_tokens),
                        ),
                        (
                            "state".to_owned(),
                            codec::selection_record_state(held.state),
                        ),
                    ])
                })
                .collect(),
            session::AGENT_SESSIONS => live.sessions.iter().map(session::row).collect(),
            resource::LOOM_AGENTS => resource::rows(&live.resources).ok_or_else(|| {
                TargetError::unavailable(
                    format!("reading `{view}`"),
                    "the generated query refused a row its declared type cannot hold",
                )
            })?,
            _ => {
                return Err(TargetError::unavailable(
                    format!("reading `{view}`"),
                    "the specification declares no such view",
                ));
            }
        };
        Ok(SemanticViewResult::of(rows))
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        Ok(self
            .live
            .borrow()
            .log
            .iter()
            .filter(|published| published.event == request.event)
            .cloned()
            .collect())
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        let forced = request.force.to_string();
        if forced == format!("{REVALIDATE_SELECTION}/{NOT_IN_FRONTIER}") {
            self.live.borrow_mut().not_in_frontier = true;
            return Ok(());
        }
        if let Some(branch) = forced
            .strip_prefix(agent::CHECK_AGENT_SPEC)
            .and_then(|rest| rest.strip_prefix('/'))
            .filter(|branch| agent::is_branch(branch))
        {
            self.live.borrow_mut().forced_check = Some(branch.to_owned());
            return Ok(());
        }
        Err(TargetError::unavailable(
            format!("forcing `{forced}`"),
            "the specification declares no such external outcome",
        ))
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        Err(TargetError::unsupported(
            format!("redelivering `{}`", request.event),
            "the specification declares no binding that reacts to an event",
        ))
    }

    fn end_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.live.borrow_mut() = Live::default();
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

/// `wrong-state` with the selection's actual state, or with no field for a selection no record
/// holds.
fn selection_wrong_state(command: &str, state: Option<SelectionState>) -> SemanticCommandResult {
    let declared = error(SELECTION_STATE_CONFLICT);
    took(command, "wrong-state").with_error(match state {
        Some(state) => declared.with("state", codec::selection_state(state)),
        None => declared,
    })
}

/// A refusal of `command` naming `field`'s identity in `name`.
fn refused(
    command: &str,
    outcome: &str,
    name: &str,
    field: &str,
    value: Node,
) -> SemanticCommandResult {
    took(command, outcome).with_error(error(name).with(field, value))
}

fn project_catalogue(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ProjectCatalogue {
        catalogue_id: CatalogueId(codec::uuid(input, "catalogue_id")?),
        turn_id: TurnId(codec::uuid(input, "turn_id")?),
        frontier: codec::text(input, "frontier")?,
        case_revision: codec::integer(input, "case_revision")?,
        entries: codec::entries(input, "entries")?,
    };
    Some(match live.catalogues.project_catalogue(command).ok()? {
        ProjectCatalogueOutcome::CatalogueExists { error } => refused(
            PROJECT_CATALOGUE,
            "catalogue-exists",
            CATALOGUE_EXISTS,
            "catalogue_id",
            codec::id(&error.catalogue_id.0),
        ),
        ProjectCatalogueOutcome::Projected {
            catalogue_projected,
        } => {
            let published = live.publish(
                event(CATALOGUE_PROJECTED)
                    .with(
                        "catalogue_id",
                        codec::id(&catalogue_projected.catalogue_id.0),
                    )
                    .with("turn_id", codec::id(&catalogue_projected.turn_id.0))
                    .with(
                        "case_revision",
                        codec::number(catalogue_projected.case_revision),
                    ),
                correlation,
            );
            took(PROJECT_CATALOGUE, "projected").emitting(published)
        }
    })
}

fn select(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = SelectAction {
        selection_id: SelectionId(codec::uuid(input, "selection_id")?),
        catalogue_id: CatalogueId(codec::uuid(input, "catalogue_id")?),
        action: codec::text(input, "action")?,
        confidence: codec::optional_decimal(input, "confidence")?,
        strategy: codec::strategy(input, "strategy")?,
        case_revision: codec::integer(input, "case_revision")?,
    };
    Some(
        match select_action(&live.catalogues, &mut live.requests, command) {
            SelectActionOutcome::SelectionExists { error } => refused(
                SELECT_ACTION,
                "selection-exists",
                SELECTION_EXISTS,
                "selection_id",
                codec::id(&error.selection_id.0),
            ),
            SelectActionOutcome::CatalogueUnknown { error } => refused(
                SELECT_ACTION,
                "catalogue-unknown",
                CATALOGUE_NOT_FOUND,
                "catalogue_id",
                codec::id(&error.catalogue_id.0),
            ),
            SelectActionOutcome::NotInCatalogue { error } => refused(
                SELECT_ACTION,
                "not-in-catalogue",
                ACTION_NOT_IN_CATALOGUE,
                "action",
                Node::Text(error.action),
            ),
            SelectActionOutcome::RevisionMismatch { error: mismatch } => {
                took(SELECT_ACTION, "revision-mismatch").with_error(
                    error(CATALOGUE_REVISION_MISMATCH)
                        .with("catalogue_id", codec::id(&mismatch.catalogue_id.0))
                        .with("case_revision", codec::number(mismatch.case_revision))
                        .with(
                            "catalogue_revision",
                            codec::number(mismatch.catalogue_revision),
                        ),
                )
            }
            SelectActionOutcome::Selected { action_selected } => {
                let published = live.publish(
                    event(ACTION_SELECTED)
                        .with("selection_id", codec::id(&action_selected.selection_id.0))
                        .with("catalogue_id", codec::id(&action_selected.catalogue_id.0))
                        .with("action", Node::Text(action_selected.action)),
                    correlation,
                );
                took(SELECT_ACTION, "selected").emitting(published)
            }
        },
    )
}

fn request_arguments(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RequestArguments {
        argument_request_id: ArgumentRequestId(codec::uuid(input, "argument_request_id")?),
        selection_id: SelectionId(codec::uuid(input, "selection_id")?),
    };
    Some(match live.requests.request_arguments(command).ok()? {
        RequestArgumentsOutcome::SelectionUnknown { error } => refused(
            REQUEST_ARGUMENTS,
            "selection-unknown",
            SELECTION_NOT_FOUND,
            "selection_id",
            codec::id(&error.selection_id.0),
        ),
        RequestArgumentsOutcome::SelectionNotSelected { error } => refused(
            REQUEST_ARGUMENTS,
            "selection-not-selected",
            SELECTION_NOT_SELECTED,
            "selection_id",
            codec::id(&error.selection_id.0),
        ),
        RequestArgumentsOutcome::Requested {
            arguments_requested,
        } => {
            let published = live.publish(
                event(ARGUMENTS_REQUESTED)
                    .with(
                        "argument_request_id",
                        codec::id(&arguments_requested.argument_request_id.0),
                    )
                    .with(
                        "selection_id",
                        codec::id(&arguments_requested.selection_id.0),
                    ),
                correlation,
            );
            took(REQUEST_ARGUMENTS, "requested").emitting(published)
        }
    })
}

/// `RevalidateSelection`, with the frontier action ids the governor would report: the input's,
/// without the selection's action when the scenario forced `not-in-frontier` for this invocation,
/// and with it otherwise. The forced outcome lapses after this invocation.
fn revalidate_selection(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let forced = std::mem::take(&mut live.not_in_frontier);
    let selection_id = SelectionId(codec::uuid(input, "selection_id")?);
    let mut frontier_actions = codec::texts(input, "frontier_actions")?;
    // Membership is answered here from the forced flag, not from the input: the specification
    // marks `not-in-frontier` `external:` (`ess/domains/run.yaml`), and ESS 0.55 synthesis refuses
    // it as a membership guard over `frontier_actions` (ESS-SYNTH-003, ESS-SYNTH-004), so every
    // synthesized scenario sends an empty list and forces the branch instead. The executor
    // therefore only ever sees `[]` or `[action]`, and this suite does not exercise its
    // not-in-frontier rule (`crates/loom-executor/src/revalidation.rs`): that rule is held by
    // `crates/loom-executor/tests/adversary_run_revalidation.rs`,
    // `not_in_frontier_follows_the_frontier_actions`. A follow-up story takes it.
    if let Some(held) = SelectionStorage::get(&live.requests, &selection_id) {
        frontier_actions.retain(|listed| *listed != held.data.action);
        if !forced {
            frontier_actions.push(held.data.action);
        }
    }
    let command = RevalidateSelection {
        selection_id,
        case_revision: codec::integer(input, "case_revision")?,
        frontier_actions,
    };
    Some(match live.requests.revalidate_selection(command).ok()? {
        RevalidateSelectionOutcome::StaleRevision { selection_stale } => {
            let published = live.publish(
                event(SELECTION_STALE)
                    .with("selection_id", codec::id(&selection_stale.selection_id.0))
                    .with(
                        "catalogue_revision",
                        codec::number(selection_stale.catalogue_revision),
                    )
                    .with(
                        "case_revision",
                        codec::number(selection_stale.case_revision),
                    ),
                correlation,
            );
            took(REVALIDATE_SELECTION, "stale-revision").emitting(published)
        }
        RevalidateSelectionOutcome::NotInFrontier {
            selection_not_in_frontier,
        } => {
            let published = live.publish(
                event(SELECTION_NOT_IN_FRONTIER)
                    .with(
                        "selection_id",
                        codec::id(&selection_not_in_frontier.selection_id.0),
                    )
                    .with("action", Node::Text(selection_not_in_frontier.action)),
                correlation,
            );
            took(REVALIDATE_SELECTION, NOT_IN_FRONTIER).emitting(published)
        }
        RevalidateSelectionOutcome::Admitted { selection_admitted } => {
            let published = live.publish(
                event(SELECTION_ADMITTED).with(
                    "selection_id",
                    codec::id(&selection_admitted.selection_id.0),
                ),
                correlation,
            );
            took(REVALIDATE_SELECTION, "admitted").emitting(published)
        }
        RevalidateSelectionOutcome::WrongState { error } => {
            selection_wrong_state(REVALIDATE_SELECTION, Some(error.state))
        }
        RevalidateSelectionOutcome::WrongStateUnknownInstance => {
            selection_wrong_state(REVALIDATE_SELECTION, None)
        }
    })
}

/// `OverruleSelection`, by the generated behaviour of the `RequestRecord`.
fn overrule_selection(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = OverruleSelection {
        selection_id: SelectionId(codec::uuid(input, "selection_id")?),
        replacement_id: SelectionId(codec::uuid(input, "replacement_id")?),
    };
    Some(match live.requests.overrule_selection(command).ok()? {
        OverruleSelectionOutcome::Overruled {
            selection_overruled,
        } => {
            let published = live.publish(
                event(SELECTION_OVERRULED)
                    .with(
                        "selection_id",
                        codec::id(&selection_overruled.selection_id.0),
                    )
                    .with(
                        "replacement_id",
                        codec::id(&selection_overruled.replacement_id.0),
                    ),
                correlation,
            );
            took(OVERRULE_SELECTION, "overruled").emitting(published)
        }
        OverruleSelectionOutcome::WrongState { error } => {
            selection_wrong_state(OVERRULE_SELECTION, Some(error.state))
        }
        OverruleSelectionOutcome::WrongStateUnknownInstance => {
            selection_wrong_state(OVERRULE_SELECTION, None)
        }
    })
}

/// `RecordSelection`, by the generated behaviour of the `RequestRecord`.
fn record_selection(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RecordSelection {
        selection_record_id: SelectionRecordId(codec::uuid(input, "selection_record_id")?),
        selection_id: SelectionId(codec::uuid(input, "selection_id")?),
        strategy: codec::strategy(input, "strategy")?,
        candidate_count: codec::integer(input, "candidate_count")?,
        chosen_action: codec::text(input, "chosen_action")?,
        confidence: codec::optional_decimal(input, "confidence")?,
        fell_back_to: codec::optional_strategy(input, "fell_back_to")?,
        latency_ms: codec::integer(input, "latency_ms")?,
        input_tokens: codec::integer(input, "input_tokens")?,
        output_tokens: codec::integer(input, "output_tokens")?,
    };
    Some(match live.requests.record_selection(command).ok()? {
        RecordSelectionOutcome::RecordExists { error } => refused(
            RECORD_SELECTION,
            "record-exists",
            SELECTION_RECORD_EXISTS,
            "selection_record_id",
            codec::id(&error.selection_record_id.0),
        ),
        RecordSelectionOutcome::SelectionUnknown { error } => refused(
            RECORD_SELECTION,
            "selection-unknown",
            SELECTION_NOT_FOUND,
            "selection_id",
            codec::id(&error.selection_id.0),
        ),
        RecordSelectionOutcome::Recorded { selection_recorded } => {
            let published = live.publish(
                event(SELECTION_RECORDED)
                    .with(
                        "selection_record_id",
                        codec::id(&selection_recorded.selection_record_id.0),
                    )
                    .with(
                        "selection_id",
                        codec::id(&selection_recorded.selection_id.0),
                    )
                    .with(
                        "strategy",
                        codec::strategy_name(selection_recorded.strategy),
                    )
                    .with(
                        "fell_back_to",
                        selection_recorded
                            .fell_back_to
                            .map_or(Node::Null, codec::strategy_name),
                    ),
                correlation,
            );
            took(RECORD_SELECTION, "recorded").emitting(published)
        }
    })
}

/// One run of a suite against [`LoomTarget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executed {
    /// `ess-conformance-report/2`, canonical: the document the verdict is read from.
    pub report: String,
    /// The runner's per-scenario results with their diagnostics, for a reader of a red run.
    pub diagnostics: String,
}

/// Admit `suite` (the JSON `ess verify conform synthesize` writes), run it against [`LoomTarget`]
/// and return the report.
///
/// # Errors
///
/// Returns the admission failure when the bytes are not an admissible suite, and the report
/// failure when the run and the suite disagree.
pub fn run_suite(suite: &str) -> Result<Executed, String> {
    let admitted = AdmittedSuite::from_json(suite).map_err(|error| error.to_string())?;
    let target = LoomTarget::default();
    let executed = Runner::for_suite(admitted.suite()).run_admitted(&admitted, &target);
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
