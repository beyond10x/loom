//! The Rust ESS conformance target for `b10x-loom-executor`.
//!
//! [`LoomTarget`] is the `ess_conformance::ConformanceTarget` the suite synthesized from `ess/`
//! runs against, and [`run_suite`] runs one suite against it and returns the
//! `ess-conformance-report/2` document a caller reads the verdict from.
//!
//! Every command is answered by `b10x-loom-executor`, over the two records a Loom keeps:
//!
//! * `OpenSession`, `RecordTurn`, `RecordCompaction`, `FileSession`, `InterruptSession`,
//!   `ResumeSession`, `ReleaseSession` and `ProjectCatalogue` by the behaviours of
//!   `b10x_loom_executor::session::TurnRecord`;
//! * `SelectAction` by `b10x_loom_executor::selection::select_action`, from the catalogues the
//!   `TurnRecord` holds into the selections of a `b10x_loom_executor::arguments::RequestRecord`;
//! * `RequestArguments` and `RevalidateSelection` by the behaviours of that `RequestRecord`;
//! * the views `Sessions`, `Catalogues` and `Selections` from what the two records hold.
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the executor returned, and each event is the one it returned.
//!
//! # The one external outcome
//!
//! `RevalidateSelection`'s `not-in-frontier` is `external:` in the specification, and the executor
//! answers it from the frontier action ids in the command's input, which a Loom reads from the
//! governor's current frontier. A scenario decides that answer with `configure_external_outcome`
//! instead, so this target plays the governor: the frontier action ids it hands the executor are the
//! input's, without the selection's action when the scenario forced `not-in-frontier` for this
//! invocation and with it otherwise. The executor still decides the outcome from them.
//!
//! Each scenario starts from empty records, an empty event log and no forced outcome, so no
//! observation of one scenario can satisfy another. Consistency tokens come from a counter, so two
//! runs of one suite report the same thing.

pub mod codec;

use std::cell::RefCell;

use b10x_loom_executor::arguments::RequestRecord;
use b10x_loom_executor::model::behaviour::SelectionStorage;
use b10x_loom_executor::model::run::obligations::{
    FileSessionBehavior, InterruptSessionBehavior, OpenSessionBehavior, ProjectCatalogueBehavior,
    RecordCompactionBehavior, RecordTurnBehavior, ReleaseSessionBehavior, RequestArgumentsBehavior,
    ResumeSessionBehavior, RevalidateSelectionBehavior,
};
use b10x_loom_executor::model::run::{
    ArgumentRequestId, CatalogueId, CommissionRunId, CompactionId, FileSession, FileSessionOutcome,
    InterruptSession, InterruptSessionOutcome, OpenSession, OpenSessionOutcome, ProjectCatalogue,
    ProjectCatalogueOutcome, RecordCompaction, RecordCompactionOutcome, RecordTurn,
    RecordTurnOutcome, ReleaseSession, ReleaseSessionOutcome, RequestArguments,
    RequestArgumentsOutcome, ResumeSession, ResumeSessionOutcome, RevalidateSelection,
    RevalidateSelectionOutcome, SelectAction, SelectActionOutcome, SelectionId, SelectionState,
    SessionId, SessionState, TurnId,
};
use b10x_loom_executor::selection::select_action;
use b10x_loom_executor::session::TurnRecord;
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

const OPEN_SESSION: &str = "loom.run.OpenSession";
const RECORD_TURN: &str = "loom.run.RecordTurn";
const RECORD_COMPACTION: &str = "loom.run.RecordCompaction";
const FILE_SESSION: &str = "loom.run.FileSession";
const INTERRUPT_SESSION: &str = "loom.run.InterruptSession";
const RESUME_SESSION: &str = "loom.run.ResumeSession";
const RELEASE_SESSION: &str = "loom.run.ReleaseSession";
const PROJECT_CATALOGUE: &str = "loom.run.ProjectCatalogue";
const SELECT_ACTION: &str = "loom.run.SelectAction";
const REQUEST_ARGUMENTS: &str = "loom.run.RequestArguments";
const REVALIDATE_SELECTION: &str = "loom.run.RevalidateSelection";

const SESSION_OPENED: &str = "loom.run.SessionOpened";
const TURN_RECORDED: &str = "loom.run.TurnRecorded";
const SESSION_COMPACTED: &str = "loom.run.SessionCompacted";
const SESSION_FILED: &str = "loom.run.SessionFiled";
const SESSION_INTERRUPTED: &str = "loom.run.SessionInterrupted";
const SESSION_RESUMED: &str = "loom.run.SessionResumed";
const SESSION_RELEASED: &str = "loom.run.SessionReleased";
const CATALOGUE_PROJECTED: &str = "loom.run.CatalogueProjected";
const ACTION_SELECTED: &str = "loom.run.ActionSelected";
const ARGUMENTS_REQUESTED: &str = "loom.run.ArgumentsRequested";
const SELECTION_STALE: &str = "loom.run.SelectionStale";
const SELECTION_NOT_IN_FRONTIER: &str = "loom.run.SelectionNotInFrontier";
const SELECTION_ADMITTED: &str = "loom.run.SelectionAdmitted";

const SESSION_EXISTS: &str = "loom.run.SessionExists";
const SESSION_NOT_FOUND: &str = "loom.run.SessionNotFound";
const SESSION_NOT_ACTIVE: &str = "loom.run.SessionNotActive";
const SESSION_STATE_CONFLICT: &str = "loom.run.SessionStateConflict";
const SESSION_WIRE_MISMATCH: &str = "loom.run.SessionWireMismatch";
const CATALOGUE_EXISTS: &str = "loom.run.CatalogueExists";
const CATALOGUE_NOT_FOUND: &str = "loom.run.CatalogueNotFound";
const ACTION_NOT_IN_CATALOGUE: &str = "loom.run.ActionNotInCatalogue";
const CATALOGUE_REVISION_MISMATCH: &str = "loom.run.CatalogueRevisionMismatch";
const SELECTION_NOT_FOUND: &str = "loom.run.SelectionNotFound";
const SELECTION_NOT_SELECTED: &str = "loom.run.SelectionNotSelected";
const SELECTION_STATE_CONFLICT: &str = "loom.run.SelectionStateConflict";

const SESSIONS: &str = "loom.run.Sessions";
const CATALOGUES: &str = "loom.run.Catalogues";
const SELECTIONS: &str = "loom.run.Selections";

/// The external outcome a scenario may force.
const NOT_IN_FRONTIER: &str = "not-in-frontier";

/// One scenario's state.
#[derive(Default)]
struct Live {
    /// Sessions, turns, compactions and catalogues, behind the executor's behaviours.
    turns: TurnRecord,
    /// Selections and argument requests, behind the executor's behaviours.
    requests: RequestRecord,
    /// Every event published in this scenario, in order.
    log: Vec<ObservedEvent>,
    /// Whether the next `RevalidateSelection` is to find its action outside the frontier.
    not_in_frontier: bool,
    /// The invocations performed, which consistency tokens are minted from.
    sequence: u64,
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
            OPEN_SESSION => open_session(&mut live, input, correlation),
            RECORD_TURN => record_turn(&mut live, input, correlation),
            RECORD_COMPACTION => record_compaction(&mut live, input, correlation),
            FILE_SESSION => file_session(&mut live, input, correlation),
            INTERRUPT_SESSION => interrupt_session(&mut live, input, correlation),
            RESUME_SESSION => resume_session(&mut live, input, correlation),
            RELEASE_SESSION => release_session(&mut live, input, correlation),
            PROJECT_CATALOGUE => project_catalogue(&mut live, input, correlation),
            SELECT_ACTION => select(&mut live, input, correlation),
            REQUEST_ARGUMENTS => request_arguments(&mut live, input, correlation),
            REVALIDATE_SELECTION => revalidate_selection(&mut live, input, correlation),
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
            SESSIONS => live
                .turns
                .sessions()
                .iter()
                .map(|held| {
                    ViewRow::from([
                        ("session_id".to_owned(), codec::id(&held.data.session_id.0)),
                        (
                            "commission_run".to_owned(),
                            codec::id(&held.data.commission_run.0),
                        ),
                        ("wire".to_owned(), Node::Text(held.data.wire.clone())),
                        ("state".to_owned(), codec::session_state(held.state)),
                    ])
                })
                .collect(),
            CATALOGUES => live
                .turns
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
                        ("state".to_owned(), codec::selection_state(held.state)),
                    ])
                })
                .collect(),
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
        if forced != format!("{REVALIDATE_SELECTION}/{NOT_IN_FRONTIER}") {
            return Err(TargetError::unavailable(
                format!("forcing `{forced}`"),
                "the specification declares no such external outcome",
            ));
        }
        self.live.borrow_mut().not_in_frontier = true;
        Ok(())
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

/// `wrong-state` with the session's actual state, or with no field for a session no record holds.
fn session_wrong_state(command: &str, state: Option<SessionState>) -> SemanticCommandResult {
    let declared = error(SESSION_STATE_CONFLICT);
    took(command, "wrong-state").with_error(match state {
        Some(state) => declared.with("state", codec::session_state(state)),
        None => declared,
    })
}

/// `wrong-state` with the selection's actual state, or with no field for a selection no record
/// holds.
fn selection_wrong_state(state: Option<SelectionState>) -> SemanticCommandResult {
    let declared = error(SELECTION_STATE_CONFLICT);
    took(REVALIDATE_SELECTION, "wrong-state").with_error(match state {
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

fn open_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = OpenSession {
        session_id: SessionId(codec::uuid(input, "session_id")?),
        commission_run: CommissionRunId(codec::uuid(input, "commission_run")?),
        wire: codec::text(input, "wire")?,
    };
    Some(match live.turns.open_session(command).ok()? {
        OpenSessionOutcome::SessionExists { error } => refused(
            OPEN_SESSION,
            "session-exists",
            SESSION_EXISTS,
            "session_id",
            codec::id(&error.session_id.0),
        ),
        OpenSessionOutcome::Opened { session_opened } => {
            let published = live.publish(
                event(SESSION_OPENED)
                    .with("session_id", codec::id(&session_opened.session_id.0))
                    .with(
                        "commission_run",
                        codec::id(&session_opened.commission_run.0),
                    )
                    .with("wire", Node::Text(session_opened.wire)),
                correlation,
            );
            took(OPEN_SESSION, "opened").emitting(published)
        }
    })
}

fn record_turn(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RecordTurn {
        turn_id: TurnId(codec::uuid(input, "turn_id")?),
        session_id: SessionId(codec::uuid(input, "session_id")?),
        index: codec::integer(input, "index")?,
        items: codec::texts(input, "items")?,
    };
    Some(match live.turns.record_turn(command).ok()? {
        RecordTurnOutcome::SessionUnknown { error } => refused(
            RECORD_TURN,
            "session-unknown",
            SESSION_NOT_FOUND,
            "session_id",
            codec::id(&error.session_id.0),
        ),
        RecordTurnOutcome::SessionNotActive { error } => refused(
            RECORD_TURN,
            "session-not-active",
            SESSION_NOT_ACTIVE,
            "session_id",
            codec::id(&error.session_id.0),
        ),
        RecordTurnOutcome::Recorded { turn_recorded } => {
            let published = live.publish(
                event(TURN_RECORDED)
                    .with("turn_id", codec::id(&turn_recorded.turn_id.0))
                    .with("session_id", codec::id(&turn_recorded.session_id.0))
                    .with("index", codec::number(turn_recorded.index)),
                correlation,
            );
            took(RECORD_TURN, "recorded").emitting(published)
        }
    })
}

fn record_compaction(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RecordCompaction {
        compaction_id: CompactionId(codec::uuid(input, "compaction_id")?),
        session_id: SessionId(codec::uuid(input, "session_id")?),
        usage: codec::optional_usage(input, "usage")?,
    };
    Some(match live.turns.record_compaction(command).ok()? {
        RecordCompactionOutcome::SessionUnknown { error } => refused(
            RECORD_COMPACTION,
            "session-unknown",
            SESSION_NOT_FOUND,
            "session_id",
            codec::id(&error.session_id.0),
        ),
        RecordCompactionOutcome::SessionNotActive { error } => refused(
            RECORD_COMPACTION,
            "session-not-active",
            SESSION_NOT_ACTIVE,
            "session_id",
            codec::id(&error.session_id.0),
        ),
        RecordCompactionOutcome::Recorded { session_compacted } => {
            let published = live.publish(
                event(SESSION_COMPACTED)
                    .with(
                        "compaction_id",
                        codec::id(&session_compacted.compaction_id.0),
                    )
                    .with("session_id", codec::id(&session_compacted.session_id.0))
                    .with("usage", codec::usage(session_compacted.usage.as_ref())),
                correlation,
            );
            took(RECORD_COMPACTION, "recorded").emitting(published)
        }
    })
}

fn file_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = FileSession {
        session_id: SessionId(codec::uuid(input, "session_id")?),
        ending: codec::run_ending(input, "ending")?,
    };
    Some(match live.turns.file_session(command).ok()? {
        FileSessionOutcome::Filed { session_filed } => {
            let published = live.publish(
                event(SESSION_FILED)
                    .with("session_id", codec::id(&session_filed.session_id.0))
                    .with("ending", codec::run_ending_name(session_filed.ending)),
                correlation,
            );
            took(FILE_SESSION, "filed").emitting(published)
        }
        FileSessionOutcome::WrongState { error } => {
            session_wrong_state(FILE_SESSION, Some(error.state))
        }
        FileSessionOutcome::WrongStateUnknownInstance => session_wrong_state(FILE_SESSION, None),
    })
}

fn interrupt_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = InterruptSession {
        session_id: SessionId(codec::uuid(input, "session_id")?),
    };
    Some(match live.turns.interrupt_session(command).ok()? {
        InterruptSessionOutcome::Interrupted {
            session_interrupted,
        } => {
            let published = live.publish(
                event(SESSION_INTERRUPTED)
                    .with("session_id", codec::id(&session_interrupted.session_id.0)),
                correlation,
            );
            took(INTERRUPT_SESSION, "interrupted").emitting(published)
        }
        InterruptSessionOutcome::WrongState { error } => {
            session_wrong_state(INTERRUPT_SESSION, Some(error.state))
        }
        InterruptSessionOutcome::WrongStateUnknownInstance => {
            session_wrong_state(INTERRUPT_SESSION, None)
        }
    })
}

fn resume_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ResumeSession {
        session_id: SessionId(codec::uuid(input, "session_id")?),
        wire: codec::text(input, "wire")?,
    };
    Some(match live.turns.resume_session(command).ok()? {
        ResumeSessionOutcome::CrossWire { error: mismatch } => took(RESUME_SESSION, "cross-wire")
            .with_error(
                error(SESSION_WIRE_MISMATCH)
                    .with("session_id", codec::id(&mismatch.session_id.0))
                    .with("session_wire", Node::Text(mismatch.session_wire))
                    .with("wire", Node::Text(mismatch.wire)),
            ),
        ResumeSessionOutcome::Resumed { session_resumed } => {
            let published = live.publish(
                event(SESSION_RESUMED)
                    .with("session_id", codec::id(&session_resumed.session_id.0))
                    .with("wire", Node::Text(session_resumed.wire)),
                correlation,
            );
            took(RESUME_SESSION, "resumed").emitting(published)
        }
        ResumeSessionOutcome::WrongState { error } => {
            session_wrong_state(RESUME_SESSION, Some(error.state))
        }
        ResumeSessionOutcome::WrongStateUnknownInstance => {
            session_wrong_state(RESUME_SESSION, None)
        }
    })
}

fn release_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ReleaseSession {
        session_id: SessionId(codec::uuid(input, "session_id")?),
    };
    Some(match live.turns.release_session(command).ok()? {
        ReleaseSessionOutcome::Released { session_released } => {
            let published = live.publish(
                event(SESSION_RELEASED)
                    .with("session_id", codec::id(&session_released.session_id.0))
                    .with("ending", codec::run_ending_name(session_released.ending)),
                correlation,
            );
            took(RELEASE_SESSION, "released").emitting(published)
        }
        ReleaseSessionOutcome::WrongState { error } => {
            session_wrong_state(RELEASE_SESSION, Some(error.state))
        }
        ReleaseSessionOutcome::WrongStateUnknownInstance => {
            session_wrong_state(RELEASE_SESSION, None)
        }
    })
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
    Some(match live.turns.project_catalogue(command).ok()? {
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
        match select_action(&live.turns, &mut live.requests, command) {
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
            selection_wrong_state(Some(error.state))
        }
        RevalidateSelectionOutcome::WrongStateUnknownInstance => selection_wrong_state(None),
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
