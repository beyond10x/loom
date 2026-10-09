// generated from loom v1
// model digest cee559ad7b98c0f74aa2bb607bd073e52902527033f7f7fd8d1292410f7f2e17
// contract digest 874974d029ad4c9d989fd20452dec290f5ffb7299a164b44350cf9fe1e61f737
// do not edit: regenerate with `ess synthesize --layout crate`

//! What the specification fully determines, generated: the behaviour of every command the plan
//! lists as generated, written against ports the implementor supplies.
//!
//! Storage is a port: one trait per entity, get, put and delete of a snapshot by identity. ess
//! preserves the trait; generated network entries supply an ephemeral store. `Context` carries the caller's attributes,
//! every identity and value the model says the implementation assigns, and the answer to each
//! `external:` branch. [`Generated`] implements every generated `…Behavior` trait over those ports
//! and forwards every behaviour and query the plan still owes to them, so it is a complete bundle
//! for every component port. To replace one generated behaviour, write a bundle of your own that
//! implements that trait and delegates the rest to a `Generated`.
//!
//! An `Err` from a generated behaviour is the typed refusal naming the command: the model declares
//! no outcome for the request (a guard is undecidable over it, or no declared branch answers it),
//! or — as `entity invariant` — the declared outcome would leave an entity breaking an invariant.

use crate::obligation::UnmetObligation;

/// Where `loom.run.ActionCatalogue` is stored — a port the implementor provides.
///
/// Keyed by the identity `catalogue_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait ActionCatalogueStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::CatalogueId) -> Option<crate::run::ActionCatalogueSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::ActionCatalogueSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::CatalogueId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::ActionCatalogueSnapshot>;
}

/// Where `loom.run.ArgumentRequest` is stored — a port the implementor provides.
///
/// Keyed by the identity `argument_request_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait ArgumentRequestStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::ArgumentRequestId) -> Option<crate::run::ArgumentRequestSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::ArgumentRequestSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::ArgumentRequestId);
}

/// Where `loom.run.Compaction` is stored — a port the implementor provides.
///
/// Keyed by the identity `compaction_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait CompactionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::CompactionId) -> Option<crate::run::CompactionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::CompactionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::CompactionId);
}

/// Where `loom.run.Selection` is stored — a port the implementor provides.
///
/// Keyed by the identity `selection_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SelectionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::SelectionId) -> Option<crate::run::SelectionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::SelectionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::SelectionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::SelectionSnapshot>;
}

/// Where `loom.run.Session` is stored — a port the implementor provides.
///
/// Keyed by the identity `session_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SessionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::SessionId) -> Option<crate::run::SessionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::SessionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::SessionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::SessionSnapshot>;
}

/// Where `loom.run.Turn` is stored — a port the implementor provides.
///
/// Keyed by the identity `turn_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait TurnStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::TurnId) -> Option<crate::run::TurnSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::TurnSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::TurnId);
}

/// The exact executing command input supplied to an external decision.
///
/// This supplies facts, not authority: the context must verify its request-bound proof.
#[derive(Debug, Clone, Copy)]
pub enum ExternalCommand<'a> {
    /// The executing `loom.run.RevalidateSelection` input.
    LoomRunRevalidateSelection(&'a crate::run::RevalidateSelection),
}

impl ExternalCommand<'_> {
    /// The canonical qualified identity of this command.
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoomRunRevalidateSelection(_) => "loom.run.RevalidateSelection",
        }
    }
}

/// What the specification leaves to the implementor's context — a port the implementor provides.
///
/// The caller's attributes, the values the model says the implementation assigns, and the answer
/// to each `external:` branch.
pub trait Context {
    /// Whether the external branch `outcome` of `command` is taken on this invocation.
    ///
    /// Asked in declaration order, before the branch's input guard is read; the first branch
    /// answered `true` whose guard holds is taken. A test forces a branch by answering `true`
    /// for it alone; a deployment asks whatever decides it.
    fn external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> bool;
}

/// Context answers that may be unavailable, without fabricated values.
/// Existing `Context` implementations receive the blanket adapter.
pub trait TryContext {
/// Decides the named external branch, or names the unavailable answer.
fn try_external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> Result<bool, UnmetObligation>;
}

impl<T: Context + ?Sized> TryContext for T {
fn try_external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> Result<bool, UnmetObligation> { Ok(Context::external(self, command, outcome)) }
}

/// An unavailable runtime context answer, rather than a new planned capability.
pub fn unmet_context(source: &'static str) -> UnmetObligation { UnmetObligation { capability: "context answer", source } }

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes,
/// `TryContext` (or its legacy `Context` blanket adapter) where one asks it anything, and every `…Behavior` and `…Query` trait the plan still
/// owes; `Generated<P>` forwards those to it.
pub struct Generated<P> {
    /// The storage and context ports, and every behaviour or query still owed.
    pub ports: P,
}

impl<P> Generated<P> {
    /// The generated behaviours, over `ports`.
    pub fn new(ports: P) -> Self {
        Self { ports }
    }
}

/// `loom.run.FileSession`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::FileSessionBehavior for Generated<P>
where
    P: SessionStorage,
{
    fn file_session(&mut self, input: crate::run::FileSession) -> Result<crate::run::FileSessionOutcome, UnmetObligation> {
        let _ = &input;
        // `filed`: the default.
        let Some(held) = SessionStorage::get(&self.ports, &input.session_id) else {
            return Ok(crate::run::FileSessionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySession::Active(instance) => crate::run::AnySession::Filed(instance.file()),
            _ => return Ok(crate::run::FileSessionOutcome::WrongState { error: crate::run::SessionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::FileSessionOutcome::Filed { session_filed: crate::run::SessionFiled { session_id: input.session_id.clone(), ending: input.ending.clone() } };
        SessionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.InterruptSession`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::InterruptSessionBehavior for Generated<P>
where
    P: SessionStorage,
{
    fn interrupt_session(&mut self, input: crate::run::InterruptSession) -> Result<crate::run::InterruptSessionOutcome, UnmetObligation> {
        let _ = &input;
        // `interrupted`: the default.
        let Some(held) = SessionStorage::get(&self.ports, &input.session_id) else {
            return Ok(crate::run::InterruptSessionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySession::Active(instance) => crate::run::AnySession::Interrupted(instance.interrupt()),
            _ => return Ok(crate::run::InterruptSessionOutcome::WrongState { error: crate::run::SessionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::InterruptSessionOutcome::Interrupted { session_interrupted: crate::run::SessionInterrupted { session_id: input.session_id.clone() } };
        SessionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.OpenSession`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::OpenSessionBehavior for Generated<P>
where
    P: SessionStorage,
{
    fn open_session(&mut self, input: crate::run::OpenSession) -> Result<crate::run::OpenSessionOutcome, UnmetObligation> {
        let _ = &input;
        // `session-exists`: an identity a record already carries, before any branch is taken.
        if SessionStorage::get(&self.ports, &input.session_id).is_some() {
            return Ok(crate::run::OpenSessionOutcome::SessionExists { error: crate::run::SessionExists { session_id: input.session_id.clone() } });
        }
        // `opened`: the default.
        let identity: crate::run::SessionId = input.session_id.clone();
        let data = crate::run::SessionData {
            session_id: identity.clone(),
            commission_run: input.commission_run.clone(),
            wire: input.wire.clone(),
        };
        let answer = crate::run::OpenSessionOutcome::Opened { session_opened: crate::run::SessionOpened { session_id: identity.clone(), commission_run: input.commission_run.clone(), wire: input.wire.clone() } };
        SessionStorage::put(&mut self.ports, crate::run::AnySession::Active(crate::run::Session::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.ProjectCatalogue`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::ProjectCatalogueBehavior for Generated<P>
where
    P: ActionCatalogueStorage,
{
    fn project_catalogue(&mut self, input: crate::run::ProjectCatalogue) -> Result<crate::run::ProjectCatalogueOutcome, UnmetObligation> {
        let _ = &input;
        // `catalogue-exists`: an identity a record already carries, before any branch is taken.
        if ActionCatalogueStorage::get(&self.ports, &input.catalogue_id).is_some() {
            return Ok(crate::run::ProjectCatalogueOutcome::CatalogueExists { error: crate::run::CatalogueExists { catalogue_id: input.catalogue_id.clone() } });
        }
        // `projected`: the default.
        let identity: crate::run::CatalogueId = input.catalogue_id.clone();
        let data = crate::run::ActionCatalogueData {
            catalogue_id: identity.clone(),
            turn_id: input.turn_id.clone(),
            frontier: input.frontier.clone(),
            case_revision: input.case_revision.clone(),
            entries: input.entries.clone(),
        };
        let answer = crate::run::ProjectCatalogueOutcome::Projected { catalogue_projected: crate::run::CatalogueProjected { catalogue_id: identity.clone(), turn_id: input.turn_id.clone(), case_revision: input.case_revision.clone() } };
        ActionCatalogueStorage::put(&mut self.ports, crate::run::AnyActionCatalogue::Projected(crate::run::ActionCatalogue::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.RecordCompaction`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RecordCompactionBehavior for Generated<P>
where
    P: CompactionStorage + SessionStorage,
{
    fn record_compaction(&mut self, input: crate::run::RecordCompaction) -> Result<crate::run::RecordCompactionOutcome, UnmetObligation> {
        let _ = &input;
        // `when_related:` reads the `loom.run.Session` row `input.session_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.session_id);
        let related = reference.and_then(|identity| SessionStorage::get(&self.ports, identity));
        // `session-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RecordCompactionOutcome::SessionUnknown { error: crate::run::SessionNotFound { session_id: input.session_id.clone() } });
        }
        let _ = &related;
        // `session-not-active`: selected by the present related row, in declaration order.
        if let Some(related) = &related {
        if decided(equal(Some(&related.state).map(|value| match value { crate::run::SessionState::Active => "Active", crate::run::SessionState::Filed => "Filed", crate::run::SessionState::Interrupted => "Interrupted" }.to_owned()), Some("Active".to_owned())).map(|value| !value), "loom.run.RecordCompaction")? {
            return Ok(crate::run::RecordCompactionOutcome::SessionNotActive { error: crate::run::SessionNotActive { session_id: input.session_id.clone() } });
        }
        }
        // `recorded`: the default.
        let identity: crate::run::CompactionId = input.compaction_id.clone();
        let data = crate::run::CompactionData {
            compaction_id: identity.clone(),
            session_id: input.session_id.clone(),
            usage: input.usage.clone(),
        };
        let answer = crate::run::RecordCompactionOutcome::Recorded { session_compacted: crate::run::SessionCompacted { compaction_id: identity.clone(), session_id: input.session_id.clone(), usage: input.usage.clone() } };
        CompactionStorage::put(&mut self.ports, crate::run::AnyCompaction::Recorded(crate::run::Compaction::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.RecordTurn`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RecordTurnBehavior for Generated<P>
where
    P: SessionStorage + TurnStorage,
{
    fn record_turn(&mut self, input: crate::run::RecordTurn) -> Result<crate::run::RecordTurnOutcome, UnmetObligation> {
        let _ = &input;
        // `when_related:` reads the `loom.run.Session` row `input.session_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.session_id);
        let related = reference.and_then(|identity| SessionStorage::get(&self.ports, identity));
        // `session-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RecordTurnOutcome::SessionUnknown { error: crate::run::SessionNotFound { session_id: input.session_id.clone() } });
        }
        let _ = &related;
        // `session-not-active`: selected by the present related row, in declaration order.
        if let Some(related) = &related {
        if decided(equal(Some(&related.state).map(|value| match value { crate::run::SessionState::Active => "Active", crate::run::SessionState::Filed => "Filed", crate::run::SessionState::Interrupted => "Interrupted" }.to_owned()), Some("Active".to_owned())).map(|value| !value), "loom.run.RecordTurn")? {
            return Ok(crate::run::RecordTurnOutcome::SessionNotActive { error: crate::run::SessionNotActive { session_id: input.session_id.clone() } });
        }
        }
        // `recorded`: the default.
        let identity: crate::run::TurnId = input.turn_id.clone();
        let data = crate::run::TurnData {
            turn_id: identity.clone(),
            session_id: input.session_id.clone(),
            index: input.index.clone(),
            items: input.items.clone(),
        };
        let answer = crate::run::RecordTurnOutcome::Recorded { turn_recorded: crate::run::TurnRecorded { turn_id: identity.clone(), session_id: input.session_id.clone(), index: input.index.clone() } };
        TurnStorage::put(&mut self.ports, crate::run::AnyTurn::Taken(crate::run::Turn::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.ReleaseSession`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::ReleaseSessionBehavior for Generated<P>
where
    P: SessionStorage,
{
    fn release_session(&mut self, input: crate::run::ReleaseSession) -> Result<crate::run::ReleaseSessionOutcome, UnmetObligation> {
        let _ = &input;
        // `released`: the default.
        let Some(held) = SessionStorage::get(&self.ports, &input.session_id) else {
            return Ok(crate::run::ReleaseSessionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySession::Active(instance) => crate::run::AnySession::Filed(instance.release()),
            _ => return Ok(crate::run::ReleaseSessionOutcome::WrongState { error: crate::run::SessionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::ReleaseSessionOutcome::Released { session_released: crate::run::SessionReleased { session_id: input.session_id.clone(), ending: crate::run::RunEnding::Failed } };
        SessionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.RequestArguments`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RequestArgumentsBehavior for Generated<P>
where
    P: ArgumentRequestStorage + SelectionStorage,
{
    fn request_arguments(&mut self, input: crate::run::RequestArguments) -> Result<crate::run::RequestArgumentsOutcome, UnmetObligation> {
        let _ = &input;
        // `when_related:` reads the `loom.run.Selection` row `input.selection_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.selection_id);
        let related = reference.and_then(|identity| SelectionStorage::get(&self.ports, identity));
        // `selection-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RequestArgumentsOutcome::SelectionUnknown { error: crate::run::SelectionNotFound { selection_id: input.selection_id.clone() } });
        }
        let _ = &related;
        // `selection-not-selected`: selected by the present related row, in declaration order.
        if let Some(related) = &related {
        if decided(equal(Some(&related.state).map(|value| match value { crate::run::SelectionState::Admitted => "Admitted", crate::run::SelectionState::Refused => "Refused", crate::run::SelectionState::Selected => "Selected" }.to_owned()), Some("Selected".to_owned())).map(|value| !value), "loom.run.RequestArguments")? {
            return Ok(crate::run::RequestArgumentsOutcome::SelectionNotSelected { error: crate::run::SelectionNotSelected { selection_id: input.selection_id.clone() } });
        }
        }
        // `requested`: the default.
        let identity: crate::run::ArgumentRequestId = input.argument_request_id.clone();
        let data = crate::run::ArgumentRequestData {
            argument_request_id: identity.clone(),
            selection_id: input.selection_id.clone(),
        };
        let answer = crate::run::RequestArgumentsOutcome::Requested { arguments_requested: crate::run::ArgumentsRequested { argument_request_id: identity.clone(), selection_id: input.selection_id.clone() } };
        ArgumentRequestStorage::put(&mut self.ports, crate::run::AnyArgumentRequest::Requested(crate::run::ArgumentRequest::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.ResumeSession`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::ResumeSessionBehavior for Generated<P>
where
    P: SessionStorage,
{
    fn resume_session(&mut self, input: crate::run::ResumeSession) -> Result<crate::run::ResumeSessionOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = SessionStorage::get(&self.ports, &input.session_id) else {
            return Ok(crate::run::ResumeSessionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        // `cross-wire`: selected by the addressed row.
        if decided(equal(Some(&held.data.wire).map(|value| value.clone()), Some(&input.wire).map(|value| value.clone())).map(|value| !value), "loom.run.ResumeSession")? {
            return Ok(crate::run::ResumeSessionOutcome::CrossWire { error: crate::run::SessionWireMismatch { session_id: input.session_id.clone(), session_wire: held.data.wire.clone(), wire: input.wire.clone() } });
        }
        // `resumed`: the default.
        let Some(held) = SessionStorage::get(&self.ports, &input.session_id) else {
            return Ok(crate::run::ResumeSessionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySession::Filed(instance) => crate::run::AnySession::Active(instance.resume()),
            crate::run::AnySession::Interrupted(instance) => crate::run::AnySession::Active(instance.resume()),
            _ => return Ok(crate::run::ResumeSessionOutcome::WrongState { error: crate::run::SessionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::ResumeSessionOutcome::Resumed { session_resumed: crate::run::SessionResumed { session_id: input.session_id.clone(), wire: input.wire.clone() } };
        SessionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.RevalidateSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RevalidateSelectionBehavior for Generated<P>
where
    P: TryContext + SelectionStorage,
{
    fn revalidate_selection(&mut self, input: crate::run::RevalidateSelection) -> Result<crate::run::RevalidateSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        // `stale-revision`: selected by the addressed row.
        if decided(compare_numbers(Some(&held.data.case_revision).map(|value| value.to_string()), Some(&input.case_revision).map(|value| value.to_string()), core::cmp::Ordering::is_ne), "loom.run.RevalidateSelection")? {
            let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
                return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
            };
            let _ = &held;
            let held_state = held.state;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Refused(instance.refuse()),
                _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
            };
            let next = moved.snapshot();
            let answer = crate::run::RevalidateSelectionOutcome::StaleRevision { selection_stale: crate::run::SelectionStale { selection_id: input.selection_id.clone(), catalogue_revision: before.case_revision.clone(), case_revision: input.case_revision.clone() } };
            SelectionStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `not-in-frontier`: an external branch, where the context takes it.
        if self.ports.try_external(ExternalCommand::LoomRunRevalidateSelection(&input), "not-in-frontier")? {
            let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
                return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
            };
            let _ = &held;
            let held_state = held.state;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Refused(instance.refuse()),
                _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
            };
            let next = moved.snapshot();
            let answer = crate::run::RevalidateSelectionOutcome::NotInFrontier { selection_not_in_frontier: crate::run::SelectionNotInFrontier { selection_id: input.selection_id.clone(), action: before.action.clone() } };
            SelectionStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `admitted`: the default.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Admitted(instance.admit()),
            _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::RevalidateSelectionOutcome::Admitted { selection_admitted: crate::run::SelectionAdmitted { selection_id: input.selection_id.clone() } };
        SelectionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

impl<P: crate::run::obligations::SelectActionBehavior> crate::run::obligations::SelectActionBehavior for Generated<P> {
    fn select_action(&mut self, input: crate::run::SelectAction) -> Result<crate::run::SelectActionOutcome, UnmetObligation> {
        crate::run::obligations::SelectActionBehavior::select_action(&mut self.ports, input)
    }
}

/// `loom.run.Catalogues`, generated: every row is one the specification fully determines from the stored `loom.run.ActionCatalogue`s.
impl<P> crate::run::obligations::CataloguesQuery for Generated<P>
where
    P: ActionCatalogueStorage,
{
    fn catalogues(&self) -> Result<Vec<crate::run::Catalogues>, UnmetObligation> {
        let admitted = ActionCatalogueStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::Catalogues {
                catalogue_id: held.data.catalogue_id,
                case_revision: held.data.case_revision,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.run.Selections`, generated: every row is one the specification fully determines from the stored `loom.run.Selection`s.
impl<P> crate::run::obligations::SelectionsQuery for Generated<P>
where
    P: SelectionStorage,
{
    fn selections(&self) -> Result<Vec<crate::run::Selections>, UnmetObligation> {
        let admitted = SelectionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::Selections {
                selection_id: held.data.selection_id,
                catalogue_id: held.data.catalogue_id,
                action: held.data.action,
                strategy: held.data.strategy,
                case_revision: held.data.case_revision,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.run.Sessions`, generated: every row is one the specification fully determines from the stored `loom.run.Session`s.
impl<P> crate::run::obligations::SessionsQuery for Generated<P>
where
    P: SessionStorage,
{
    fn sessions(&self) -> Result<Vec<crate::run::Sessions>, UnmetObligation> {
        let admitted = SessionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::Sessions {
                session_id: held.data.session_id,
                commission_run: held.data.commission_run,
                wire: held.data.wire,
                state: held.state,
            })
            .collect())
    }
}

/// The typed refusal of a request the model declares no outcome for.
fn undeclared(source: &'static str) -> UnmetObligation {
    let capability = "command behaviour";
    UnmetObligation { capability, source }
}

/// A guard's truth, where it has one: Unknown selects no branch, so the model declares no outcome.
fn decided(truth: Option<bool>, command: &'static str) -> Result<bool, UnmetObligation> {
    truth.ok_or_else(|| undeclared(command))
}

/// Equality of two read values; an unread one is Unknown.
fn equal<T: PartialEq>(left: Option<T>, right: Option<T>) -> Option<bool> {
    Some(left? == right?)
}

/// A decimal rendering as its sign, its whole digits and its fraction digits, without the zeros
/// that do not change its value; `None` where it is not a plain decimal.
fn number_parts(text: &str) -> Option<(bool, String, String)> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().chain(fraction.bytes()).all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole = whole.trim_start_matches('0').to_owned();
    let fraction = fraction.trim_end_matches('0').to_owned();
    let zero = whole.is_empty() && fraction.is_empty();
    Some((negative && !zero, whole, fraction))
}

/// Compares two decimal renderings exactly; an unread or unparsable one is Unknown.
fn compare_numbers(
    left: Option<String>,
    right: Option<String>,
    accepts: fn(core::cmp::Ordering) -> bool,
) -> Option<bool> {
    let (left, right) = (number_parts(&left?)?, number_parts(&right?)?);
    let magnitude = left
        .1
        .len()
        .cmp(&right.1.len())
        .then_with(|| left.1.cmp(&right.1))
        .then_with(|| left.2.cmp(&right.2));
    let ordering = match (left.0, right.0) {
        (false, false) => magnitude,
        (true, true) => magnitude.reverse(),
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
    };
    Some(accepts(ordering))
}
