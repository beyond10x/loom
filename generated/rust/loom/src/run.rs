// generated from loom v1
// model digest 43dcb22fa4b8b8a3ab9e196467ad3d47be4a63873a6a7b541b59c4567d195883
// contract digest 976fca135fea82818eebad3e94b5d61c4dc9a9754c5ed5bcb1f30531c2984b69
// do not edit: regenerate with `ess synthesize --layout crate`

//! Run — `loom.run`.
//!
//! One model session executing a commission's run. Each catalogue is projected from one frontier; a selection names one action from that catalogue; arguments are generated for the selected action only. Loom never decides authority or completion.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// The states of `loom.run.ActionCatalogue`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ActionCatalogue<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCatalogueState {
    /// `Projected`.
    Projected,
}

/// The states of `loom.run.ArgumentRequest`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ArgumentRequest<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentRequestState {
    /// `Requested`.
    Requested,
}

/// ArgumentRequestId — `loom.run.ArgumentRequestId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentRequestId(pub crate::primitives::Uuid);

/// CatalogueId — `loom.run.CatalogueId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueId(pub crate::primitives::Uuid);

/// CommissionRunId — `loom.run.CommissionRunId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionRunId(pub crate::primitives::Uuid);

/// The states of `loom.run.Selection`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Selection<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// `Selected`.
    Selected,
}

/// SelectionId — `loom.run.SelectionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionId(pub crate::primitives::Uuid);

/// SelectionStrategy — `loom.run.SelectionStrategy`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// `ReasoningModel`.
    ReasoningModel,
    /// `FastTyped`.
    FastTyped,
    /// `Rule`.
    Rule,
    /// `Hybrid`.
    Hybrid,
}

/// The states of `loom.run.Session`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Session<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// `Active`.
    Active,
}

/// SessionId — `loom.run.SessionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub crate::primitives::Uuid);

/// The states of `loom.run.Turn`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Turn<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// `Taken`.
    Taken,
}

/// TurnId — `loom.run.TurnId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnId(pub crate::primitives::Uuid);

/// What ActionCatalogue — `loom.run.ActionCatalogue` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ActionCatalogue<S>`], and at a boundary by [`ActionCatalogueSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionCatalogueData {
    /// The identity: `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `turn_id` — `loom.run.TurnId`.
    ///
    /// Carries `catalogue`: `loom.run.Turn` owns one `loom.run.ActionCatalogue`.
    pub turn_id: TurnId,
    /// `frontier` — `String`.
    pub frontier: String,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// The states of `loom.run.ActionCatalogue`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](action_catalogue_state::Marker), so [`ActionCatalogue<S>`](ActionCatalogue) can only ever rest in a real state.
pub mod action_catalogue_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Projected {}
    }

    /// A declared state of `ActionCatalogue`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ActionCatalogueState;
    }

    /// `Projected`. Where a new instance starts.
    pub struct Projected;

    impl Marker for Projected {
        const STATE: super::ActionCatalogueState = super::ActionCatalogueState::Projected;
    }
}

/// ActionCatalogue — `loom.run.ActionCatalogue` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Projected`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ActionCatalogueSnapshot`]
/// and [`ActionCatalogueSnapshot::refine`].
pub struct ActionCatalogue<S: action_catalogue_state::Marker> {
    data: ActionCatalogueData,
    state: core::marker::PhantomData<S>,
}

impl<S: action_catalogue_state::Marker> ActionCatalogue<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ActionCatalogueState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ActionCatalogueData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ActionCatalogueData {
        self.data
    }
}

impl ActionCatalogue<action_catalogue_state::Projected> {
    /// A new instance, resting in `Projected` — the only state the lifecycle starts one in.
    pub fn new(data: ActionCatalogueData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.ActionCatalogue` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ActionCatalogueSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionCatalogueSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ActionCatalogueState,
    /// What it holds.
    pub data: ActionCatalogueData,
}

/// An `ActionCatalogue` in whichever declared state it was found.
pub enum AnyActionCatalogue {
    /// Resting in `Projected`.
    Projected(ActionCatalogue<action_catalogue_state::Projected>),
}

impl ActionCatalogueSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ActionCatalogueState` cannot spell one.
    pub fn refine(self) -> AnyActionCatalogue {
        match self.state {
            ActionCatalogueState::Projected => AnyActionCatalogue::Projected(ActionCatalogue {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyActionCatalogue {
    /// The state, as the runtime value.
    pub fn state(&self) -> ActionCatalogueState {
        match self {
            Self::Projected(_) => ActionCatalogueState::Projected,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ActionCatalogueSnapshot {
        match self {
            Self::Projected(instance) => ActionCatalogueSnapshot {
                state: ActionCatalogueState::Projected,
                data: instance.into_data(),
            },
        }
    }
}

/// What ArgumentRequest — `loom.run.ArgumentRequest` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ArgumentRequest<S>`], and at a boundary by [`ArgumentRequestSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentRequestData {
    /// The identity: `argument_request_id` — `loom.run.ArgumentRequestId`.
    pub argument_request_id: ArgumentRequestId,
    /// `selection_id` — `loom.run.SelectionId`.
    ///
    /// Carries `selection`: `loom.run.ArgumentRequest` references one `loom.run.Selection`.
    pub selection_id: SelectionId,
}

/// The states of `loom.run.ArgumentRequest`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](argument_request_state::Marker), so [`ArgumentRequest<S>`](ArgumentRequest) can only ever rest in a real state.
pub mod argument_request_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Requested {}
    }

    /// A declared state of `ArgumentRequest`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ArgumentRequestState;
    }

    /// `Requested`. Where a new instance starts.
    pub struct Requested;

    impl Marker for Requested {
        const STATE: super::ArgumentRequestState = super::ArgumentRequestState::Requested;
    }
}

/// ArgumentRequest — `loom.run.ArgumentRequest` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Requested`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ArgumentRequestSnapshot`]
/// and [`ArgumentRequestSnapshot::refine`].
pub struct ArgumentRequest<S: argument_request_state::Marker> {
    data: ArgumentRequestData,
    state: core::marker::PhantomData<S>,
}

impl<S: argument_request_state::Marker> ArgumentRequest<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ArgumentRequestState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ArgumentRequestData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ArgumentRequestData {
        self.data
    }
}

impl ArgumentRequest<argument_request_state::Requested> {
    /// A new instance, resting in `Requested` — the only state the lifecycle starts one in.
    pub fn new(data: ArgumentRequestData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.ArgumentRequest` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ArgumentRequestSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentRequestSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ArgumentRequestState,
    /// What it holds.
    pub data: ArgumentRequestData,
}

/// An `ArgumentRequest` in whichever declared state it was found.
pub enum AnyArgumentRequest {
    /// Resting in `Requested`.
    Requested(ArgumentRequest<argument_request_state::Requested>),
}

impl ArgumentRequestSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ArgumentRequestState` cannot spell one.
    pub fn refine(self) -> AnyArgumentRequest {
        match self.state {
            ArgumentRequestState::Requested => AnyArgumentRequest::Requested(ArgumentRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyArgumentRequest {
    /// The state, as the runtime value.
    pub fn state(&self) -> ArgumentRequestState {
        match self {
            Self::Requested(_) => ArgumentRequestState::Requested,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ArgumentRequestSnapshot {
        match self {
            Self::Requested(instance) => ArgumentRequestSnapshot {
                state: ArgumentRequestState::Requested,
                data: instance.into_data(),
            },
        }
    }
}

/// What Selection — `loom.run.Selection` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Selection<S>`], and at a boundary by [`SelectionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionData {
    /// The identity: `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `catalogue_id` — `loom.run.CatalogueId`.
    ///
    /// Carries `catalogue`: `loom.run.Selection` references one `loom.run.ActionCatalogue`.
    pub catalogue_id: CatalogueId,
    /// `action` — `String`.
    pub action: String,
    /// `confidence` — `Optional<Decimal>`.
    pub confidence: Option<crate::primitives::Decimal>,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
}

/// The states of `loom.run.Selection`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](selection_state::Marker), so [`Selection<S>`](Selection) can only ever rest in a real state.
pub mod selection_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Selected {}
    }

    /// A declared state of `Selection`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SelectionState;
    }

    /// `Selected`. Where a new instance starts.
    pub struct Selected;

    impl Marker for Selected {
        const STATE: super::SelectionState = super::SelectionState::Selected;
    }
}

/// Selection — `loom.run.Selection` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Selected`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`SelectionSnapshot`]
/// and [`SelectionSnapshot::refine`].
pub struct Selection<S: selection_state::Marker> {
    data: SelectionData,
    state: core::marker::PhantomData<S>,
}

impl<S: selection_state::Marker> Selection<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> SelectionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &SelectionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> SelectionData {
        self.data
    }
}

impl Selection<selection_state::Selected> {
    /// A new instance, resting in `Selected` — the only state the lifecycle starts one in.
    pub fn new(data: SelectionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.Selection` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`SelectionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: SelectionState,
    /// What it holds.
    pub data: SelectionData,
}

/// An `Selection` in whichever declared state it was found.
pub enum AnySelection {
    /// Resting in `Selected`.
    Selected(Selection<selection_state::Selected>),
}

impl SelectionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `SelectionState` cannot spell one.
    pub fn refine(self) -> AnySelection {
        match self.state {
            SelectionState::Selected => AnySelection::Selected(Selection {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnySelection {
    /// The state, as the runtime value.
    pub fn state(&self) -> SelectionState {
        match self {
            Self::Selected(_) => SelectionState::Selected,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SelectionSnapshot {
        match self {
            Self::Selected(instance) => SelectionSnapshot {
                state: SelectionState::Selected,
                data: instance.into_data(),
            },
        }
    }
}

/// What Session — `loom.run.Session` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Session<S>`], and at a boundary by [`SessionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionData {
    /// The identity: `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `commission_run` — `loom.run.CommissionRunId`.
    pub commission_run: CommissionRunId,
}

/// The states of `loom.run.Session`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](session_state::Marker), so [`Session<S>`](Session) can only ever rest in a real state.
pub mod session_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Active {}
    }

    /// A declared state of `Session`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SessionState;
    }

    /// `Active`. Where a new instance starts.
    pub struct Active;

    impl Marker for Active {
        const STATE: super::SessionState = super::SessionState::Active;
    }
}

/// Session — `loom.run.Session` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Active`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`SessionSnapshot`]
/// and [`SessionSnapshot::refine`].
pub struct Session<S: session_state::Marker> {
    data: SessionData,
    state: core::marker::PhantomData<S>,
}

impl<S: session_state::Marker> Session<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> SessionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &SessionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> SessionData {
        self.data
    }
}

impl Session<session_state::Active> {
    /// A new instance, resting in `Active` — the only state the lifecycle starts one in.
    pub fn new(data: SessionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.Session` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`SessionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: SessionState,
    /// What it holds.
    pub data: SessionData,
}

/// An `Session` in whichever declared state it was found.
pub enum AnySession {
    /// Resting in `Active`.
    Active(Session<session_state::Active>),
}

impl SessionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `SessionState` cannot spell one.
    pub fn refine(self) -> AnySession {
        match self.state {
            SessionState::Active => AnySession::Active(Session {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnySession {
    /// The state, as the runtime value.
    pub fn state(&self) -> SessionState {
        match self {
            Self::Active(_) => SessionState::Active,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SessionSnapshot {
        match self {
            Self::Active(instance) => SessionSnapshot {
                state: SessionState::Active,
                data: instance.into_data(),
            },
        }
    }
}

/// What Turn — `loom.run.Turn` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Turn<S>`], and at a boundary by [`TurnSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnData {
    /// The identity: `turn_id` — `loom.run.TurnId`.
    pub turn_id: TurnId,
    /// `session_id` — `loom.run.SessionId`.
    ///
    /// Carries `turns`: `loom.run.Session` owns many `loom.run.Turn`.
    pub session_id: SessionId,
    /// `index` — `Integer`.
    pub index: i64,
}

/// The states of `loom.run.Turn`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](turn_state::Marker), so [`Turn<S>`](Turn) can only ever rest in a real state.
pub mod turn_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Taken {}
    }

    /// A declared state of `Turn`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::TurnState;
    }

    /// `Taken`. Where a new instance starts.
    pub struct Taken;

    impl Marker for Taken {
        const STATE: super::TurnState = super::TurnState::Taken;
    }
}

/// Turn — `loom.run.Turn` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Taken`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`TurnSnapshot`]
/// and [`TurnSnapshot::refine`].
pub struct Turn<S: turn_state::Marker> {
    data: TurnData,
    state: core::marker::PhantomData<S>,
}

impl<S: turn_state::Marker> Turn<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> TurnState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &TurnData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> TurnData {
        self.data
    }
}

impl Turn<turn_state::Taken> {
    /// A new instance, resting in `Taken` — the only state the lifecycle starts one in.
    pub fn new(data: TurnData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.Turn` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`TurnSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: TurnState,
    /// What it holds.
    pub data: TurnData,
}

/// An `Turn` in whichever declared state it was found.
pub enum AnyTurn {
    /// Resting in `Taken`.
    Taken(Turn<turn_state::Taken>),
}

impl TurnSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `TurnState` cannot spell one.
    pub fn refine(self) -> AnyTurn {
        match self.state {
            TurnState::Taken => AnyTurn::Taken(Turn {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyTurn {
    /// The state, as the runtime value.
    pub fn state(&self) -> TurnState {
        match self {
            Self::Taken(_) => TurnState::Taken,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> TurnSnapshot {
        match self {
            Self::Taken(instance) => TurnSnapshot {
                state: TurnState::Taken,
                data: instance.into_data(),
            },
        }
    }
}
