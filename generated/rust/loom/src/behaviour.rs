// generated from loom v1
// model digest 1e5c1537dda3b2b7e22b162efd4278fee13385bc49abc57d7ce5100af963d5c1
// contract digest d23e825dcb7bfe03fbe20cea75b62e61bd0a45f586a2de0740780d02d5d2b8a1
// do not edit: regenerate with `ess synthesize --layout crate`

//! What the specification fully determines, generated: the behaviour of every command the plan
//! lists as generated, written against ports the implementor supplies.
//!
//! Storage is a port: one trait per entity, get, put and delete of a snapshot by identity. ess
//! generates the trait and never a store. `Context` is the other port: the caller's attributes,
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
/// Keyed by the identity `catalogue_id`. ess generates this trait and never an implementation of it.
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

/// Where `loom.run.Selection` is stored — a port the implementor provides.
///
/// Keyed by the identity `selection_id`. ess generates this trait and never an implementation of it.
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
/// Keyed by the identity `session_id`. ess generates this trait and never an implementation of it.
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
    fn external(&mut self, command: &'static str, outcome: &'static str) -> bool;
}

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes,
/// `Context` where one asks it anything, and every `…Behavior` and `…Query` trait the plan still
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
        SessionStorage::put(&mut self.ports, next);
        return Ok(crate::run::FileSessionOutcome::Filed { session_filed: crate::run::SessionFiled { session_id: input.session_id.clone(), ending: input.ending.clone() } });
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
        SessionStorage::put(&mut self.ports, crate::run::AnySession::Active(crate::run::Session::new(data)).snapshot());
        return Ok(crate::run::OpenSessionOutcome::Opened { session_opened: crate::run::SessionOpened { session_id: identity.clone(), commission_run: input.commission_run.clone(), wire: input.wire.clone() } });
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
        ActionCatalogueStorage::put(&mut self.ports, crate::run::AnyActionCatalogue::Projected(crate::run::ActionCatalogue::new(data)).snapshot());
        return Ok(crate::run::ProjectCatalogueOutcome::Projected { catalogue_projected: crate::run::CatalogueProjected { catalogue_id: identity.clone(), turn_id: input.turn_id.clone(), case_revision: input.case_revision.clone() } });
    }
}

impl<P: crate::run::obligations::RecordTurnBehavior> crate::run::obligations::RecordTurnBehavior for Generated<P> {
    fn record_turn(&mut self, input: crate::run::RecordTurn) -> Result<crate::run::RecordTurnOutcome, UnmetObligation> {
        crate::run::obligations::RecordTurnBehavior::record_turn(&mut self.ports, input)
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
        SessionStorage::put(&mut self.ports, next);
        return Ok(crate::run::ReleaseSessionOutcome::Released { session_released: crate::run::SessionReleased { session_id: input.session_id.clone(), ending: crate::run::RunEnding::Failed } });
    }
}

impl<P: crate::run::obligations::RequestArgumentsBehavior> crate::run::obligations::RequestArgumentsBehavior for Generated<P> {
    fn request_arguments(&mut self, input: crate::run::RequestArguments) -> Result<crate::run::RequestArgumentsOutcome, UnmetObligation> {
        crate::run::obligations::RequestArgumentsBehavior::request_arguments(&mut self.ports, input)
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
            _ => return Ok(crate::run::ResumeSessionOutcome::WrongState { error: crate::run::SessionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        SessionStorage::put(&mut self.ports, next);
        return Ok(crate::run::ResumeSessionOutcome::Resumed { session_resumed: crate::run::SessionResumed { session_id: input.session_id.clone(), wire: input.wire.clone() } });
    }
}

/// `loom.run.RevalidateSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RevalidateSelectionBehavior for Generated<P>
where
    P: Context + SelectionStorage,
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
            SelectionStorage::put(&mut self.ports, next);
            return Ok(crate::run::RevalidateSelectionOutcome::StaleRevision { selection_stale: crate::run::SelectionStale { selection_id: input.selection_id.clone(), catalogue_revision: before.case_revision.clone(), case_revision: input.case_revision.clone() } });
        }
        // `not-in-frontier`: an external branch, where the context takes it.
        if self.ports.external("loom.run.RevalidateSelection", "not-in-frontier") {
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
            SelectionStorage::put(&mut self.ports, next);
            return Ok(crate::run::RevalidateSelectionOutcome::NotInFrontier { selection_not_in_frontier: crate::run::SelectionNotInFrontier { selection_id: input.selection_id.clone(), action: before.action.clone() } });
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
        SelectionStorage::put(&mut self.ports, next);
        return Ok(crate::run::RevalidateSelectionOutcome::Admitted { selection_admitted: crate::run::SelectionAdmitted { selection_id: input.selection_id.clone() } });
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
