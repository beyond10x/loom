// generated from loom v1
// model digest b774beb53c36941f484cfdcfb927b46287a34f934f9b43c3d7e49306ebe94f34
// contract digest 7b9385bd4805d85156d0039e947a53ea05aeda4435f2bebd60af8072091657ce
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

/// Where `loom.run.SelectionRecord` is stored — a port the implementor provides.
///
/// Keyed by the identity `selection_record_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SelectionRecordStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::SelectionRecordId) -> Option<crate::run::SelectionRecordSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::SelectionRecordSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::SelectionRecordId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::SelectionRecordSnapshot>;
}

/// Where `loom.session.AgentSession` is stored — a port the implementor provides.
///
/// Keyed by the identity `session_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait AgentSessionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::session::SessionId) -> Option<crate::session::AgentSessionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::session::AgentSessionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::session::SessionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::session::AgentSessionSnapshot>;
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

/// `loom.run.OverruleSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::OverruleSelectionBehavior for Generated<P>
where
    P: SelectionStorage,
{
    fn overrule_selection(&mut self, input: crate::run::OverruleSelection) -> Result<crate::run::OverruleSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // `overruled`: the default.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::OverruleSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Overruled(instance.overrule()),
            _ => return Ok(crate::run::OverruleSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
        };
        let mut next = moved.snapshot();
        next.data.replaced_by = Some(input.replacement_id.clone());
        let answer = crate::run::OverruleSelectionOutcome::Overruled { selection_overruled: crate::run::SelectionOverruled { selection_id: input.selection_id.clone(), replacement_id: input.replacement_id.clone() } };
        SelectionStorage::put(&mut self.ports, next);
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

/// `loom.run.RecordSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RecordSelectionBehavior for Generated<P>
where
    P: SelectionStorage + SelectionRecordStorage,
{
    fn record_selection(&mut self, input: crate::run::RecordSelection) -> Result<crate::run::RecordSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // `record-exists`: an identity a record already carries, before any branch is taken.
        if SelectionRecordStorage::get(&self.ports, &input.selection_record_id).is_some() {
            return Ok(crate::run::RecordSelectionOutcome::RecordExists { error: crate::run::SelectionRecordExists { selection_record_id: input.selection_record_id.clone() } });
        }
        // `when_related:` reads the `loom.run.Selection` row `input.selection_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.selection_id);
        let related = reference.and_then(|identity| SelectionStorage::get(&self.ports, identity));
        // `selection-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RecordSelectionOutcome::SelectionUnknown { error: crate::run::SelectionNotFound { selection_id: input.selection_id.clone() } });
        }
        let _ = &related;
        // `recorded`: the default.
        let identity: crate::run::SelectionRecordId = input.selection_record_id.clone();
        let data = crate::run::SelectionRecordData {
            selection_record_id: identity.clone(),
            selection_id: input.selection_id.clone(),
            strategy: input.strategy.clone(),
            candidate_count: input.candidate_count.clone(),
            chosen_action: input.chosen_action.clone(),
            confidence: input.confidence.clone(),
            fell_back_to: input.fell_back_to.clone(),
            latency_ms: input.latency_ms.clone(),
            input_tokens: input.input_tokens.clone(),
            output_tokens: input.output_tokens.clone(),
        };
        let answer = crate::run::RecordSelectionOutcome::Recorded { selection_recorded: crate::run::SelectionRecorded { selection_record_id: identity.clone(), selection_id: input.selection_id.clone(), strategy: input.strategy.clone(), fell_back_to: input.fell_back_to.clone() } };
        SelectionRecordStorage::put(&mut self.ports, crate::run::AnySelectionRecord::Recorded(crate::run::SelectionRecord::new(data)).snapshot());
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
        if decided(equal(Some(&related.state).map(|value| match value { crate::run::SelectionState::Admitted => "Admitted", crate::run::SelectionState::Overruled => "Overruled", crate::run::SelectionState::Refused => "Refused", crate::run::SelectionState::Selected => "Selected" }.to_owned()), Some("Selected".to_owned())).map(|value| !value), "loom.run.RequestArguments")? {
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

/// `loom.session.ProposeSessionChange`, generated: every outcome is one the specification fully determines.
impl<P> crate::session::obligations::ProposeSessionChangeBehavior for Generated<P>
where
    P: AgentSessionStorage,
{
    fn propose_session_change(&mut self, input: crate::session::ProposeSessionChange) -> Result<crate::session::ProposeSessionChangeOutcome, UnmetObligation> {
        let _ = &input;
        // `proposed`: the default.
        let identity: crate::session::SessionId = input.session_id.clone();
        let data = crate::session::AgentSessionData {
            session_id: identity.clone(),
            kind: input.kind.clone(),
            commission_id: input.commission_id.clone(),
            frontier_id: input.frontier_id.clone(),
            vendor_session_id: input.vendor_session_id.clone(),
            agent: input.agent.clone(),
            continues: input.continues.clone(),
            action: input.action.clone(),
            files: input.files.clone(),
        };
        let answer = crate::session::ProposeSessionChangeOutcome::Proposed { session_change_proposed: crate::session::SessionChangeProposed { session_id: identity.clone(), commission_id: input.commission_id.clone(), frontier_id: input.frontier_id.clone(), continues: input.continues.clone(), action: input.action.clone(), files: input.files.clone() } };
        AgentSessionStorage::put(&mut self.ports, crate::session::AnyAgentSession::Ended(crate::session::AgentSession::new(data)).snapshot());
        return Ok(answer);
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

/// `loom.run.SelectionRecords`, generated: every row is one the specification fully determines from the stored `loom.run.SelectionRecord`s.
impl<P> crate::run::obligations::SelectionRecordsQuery for Generated<P>
where
    P: SelectionRecordStorage,
{
    fn selection_records(&self) -> Result<Vec<crate::run::SelectionRecords>, UnmetObligation> {
        let admitted = SelectionRecordStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::SelectionRecords {
                selection_record_id: held.data.selection_record_id,
                selection_id: held.data.selection_id,
                strategy: held.data.strategy,
                candidate_count: held.data.candidate_count,
                chosen_action: held.data.chosen_action,
                fell_back_to: held.data.fell_back_to,
                latency_ms: held.data.latency_ms,
                input_tokens: held.data.input_tokens,
                output_tokens: held.data.output_tokens,
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
                replaced_by: held.data.replaced_by,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.session.AgentSessions`, generated: every row is one the specification fully determines from the stored `loom.session.AgentSession`s.
impl<P> crate::session::obligations::AgentSessionsQuery for Generated<P>
where
    P: AgentSessionStorage,
{
    fn agent_sessions(&self) -> Result<Vec<crate::session::AgentSessions>, UnmetObligation> {
        let admitted = AgentSessionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::session::AgentSessions {
                session_id: held.data.session_id,
                kind: held.data.kind,
                commission_id: held.data.commission_id,
                frontier_id: held.data.frontier_id,
                vendor_session_id: held.data.vendor_session_id,
                agent: held.data.agent,
                continues: held.data.continues,
                action: held.data.action,
                files: held.data.files,
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
