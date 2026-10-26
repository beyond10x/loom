// generated from loom v1
// model digest bd0680f8ac3534518a439d54a62dfd2a90bf5c733881ebeb4328aed4f2b64019
// contract digest 686d3e50c2ac373e8881a3960e68cf840d8e87f71ccb441f5efdc3580e3c3fa1
// do not edit: regenerate with `ess synthesize --layout crate`

//! Run — `loom.run`.
//!
//! The executor's record of a commission's run. Each catalogue is projected from one frontier; a selection names one action from that catalogue; arguments are generated for the selected action only. Loom never decides authority or completion.
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

/// CatalogueEntry — `loom.run.CatalogueEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEntry {
    /// `action` — `String`.
    pub action: String,
    /// `status` — `loom.run.CatalogueEntryStatus`.
    pub status: CatalogueEntryStatus,
}

/// CatalogueEntryStatus — `loom.run.CatalogueEntryStatus`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogueEntryStatus {
    /// `Admissible`.
    Admissible,
    /// `ApprovalRequired`.
    ApprovalRequired,
}

/// CatalogueId — `loom.run.CatalogueId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueId(pub crate::primitives::Uuid);

/// CommissionRunId — `loom.run.CommissionRunId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionRunId(pub crate::primitives::Uuid);

/// LoopStop — `loom.run.LoopStop`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopStop {
    /// Tagged `budget-unobservable` — `loom.run.LoopStopBudgetUnobservable`.
    BudgetUnobservable(LoopStopBudgetUnobservable),
    /// Tagged `max-input-tokens` — `loom.run.LoopStopMaxInputTokens`.
    MaxInputTokens(LoopStopMaxInputTokens),
}

/// LoopStopBudgetUnobservable — `loom.run.LoopStopBudgetUnobservable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopStopBudgetUnobservable {
    /// `name` — `String`.
    pub name: String,
    /// `reason` — `String`.
    pub reason: String,
}

/// LoopStopMaxInputTokens — `loom.run.LoopStopMaxInputTokens`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopStopMaxInputTokens {
    /// `limit` — `Integer`.
    pub limit: i64,
    /// `reported` — `Integer`.
    pub reported: i64,
}

/// The states of `loom.run.Selection`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Selection<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// `Admitted`.
    Admitted,
    /// `Overruled`.
    Overruled,
    /// `Refused`.
    Refused,
    /// `Selected`.
    Selected,
}

/// SelectionId — `loom.run.SelectionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionId(pub crate::primitives::Uuid);

/// The states of `loom.run.SelectionRecord`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `SelectionRecord<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionRecordState {
    /// `Recorded`.
    Recorded,
}

/// SelectionRecordId — `loom.run.SelectionRecordId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecordId(pub crate::primitives::Uuid);

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
    pub turn_id: TurnId,
    /// `frontier` — `String`.
    pub frontier: String,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `entries` — `List<loom.run.CatalogueEntry>`.
    pub entries: Vec<CatalogueEntry>,
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
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `replaced_by` — `Optional<loom.run.SelectionId>`.
    ///
    /// Carries `replacement`: `loom.run.Selection` references one `loom.run.Selection`.
    pub replaced_by: Option<SelectionId>,
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
        impl Sealed for super::Admitted {}
        impl Sealed for super::Overruled {}
        impl Sealed for super::Refused {}
        impl Sealed for super::Selected {}
    }

    /// A declared state of `Selection`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SelectionState;
    }

    /// `Admitted`. Terminal: an instance may rest here forever.
    pub struct Admitted;

    impl Marker for Admitted {
        const STATE: super::SelectionState = super::SelectionState::Admitted;
    }

    /// `Overruled`. Terminal: an instance may rest here forever.
    pub struct Overruled;

    impl Marker for Overruled {
        const STATE: super::SelectionState = super::SelectionState::Overruled;
    }

    /// `Refused`. Terminal: an instance may rest here forever.
    pub struct Refused;

    impl Marker for Refused {
        const STATE: super::SelectionState = super::SelectionState::Refused;
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

impl Selection<selection_state::Selected> {
    /// `admit` — `Selected` → `Admitted`. Taken by the `admitted` outcome of `loom.run.RevalidateSelection`.
    pub fn admit(self) -> Selection<selection_state::Admitted> {
        Selection {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `refuse` — `Selected` → `Refused`. Taken by the `stale-revision` outcome of `loom.run.RevalidateSelection`, the `not-in-frontier` outcome of `loom.run.RevalidateSelection`.
    pub fn refuse(self) -> Selection<selection_state::Refused> {
        Selection {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `overrule` — `Selected` → `Overruled`. Taken by the `overruled` outcome of `loom.run.OverruleSelection`.
    pub fn overrule(self) -> Selection<selection_state::Overruled> {
        Selection {
            data: self.data,
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
    /// Resting in `Admitted`.
    Admitted(Selection<selection_state::Admitted>),
    /// Resting in `Overruled`.
    Overruled(Selection<selection_state::Overruled>),
    /// Resting in `Refused`.
    Refused(Selection<selection_state::Refused>),
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
            SelectionState::Admitted => AnySelection::Admitted(Selection {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            SelectionState::Overruled => AnySelection::Overruled(Selection {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            SelectionState::Refused => AnySelection::Refused(Selection {
                data: self.data,
                state: core::marker::PhantomData,
            }),
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
            Self::Admitted(_) => SelectionState::Admitted,
            Self::Overruled(_) => SelectionState::Overruled,
            Self::Refused(_) => SelectionState::Refused,
            Self::Selected(_) => SelectionState::Selected,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SelectionSnapshot {
        match self {
            Self::Admitted(instance) => SelectionSnapshot {
                state: SelectionState::Admitted,
                data: instance.into_data(),
            },
            Self::Overruled(instance) => SelectionSnapshot {
                state: SelectionState::Overruled,
                data: instance.into_data(),
            },
            Self::Refused(instance) => SelectionSnapshot {
                state: SelectionState::Refused,
                data: instance.into_data(),
            },
            Self::Selected(instance) => SelectionSnapshot {
                state: SelectionState::Selected,
                data: instance.into_data(),
            },
        }
    }
}

/// What SelectionRecord — `loom.run.SelectionRecord` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`SelectionRecord<S>`], and at a boundary by [`SelectionRecordSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecordData {
    /// The identity: `selection_record_id` — `loom.run.SelectionRecordId`.
    pub selection_record_id: SelectionRecordId,
    /// `selection_id` — `loom.run.SelectionId`.
    ///
    /// Carries `telemetry`: `loom.run.Selection` owns one `loom.run.SelectionRecord`.
    pub selection_id: SelectionId,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `candidate_count` — `Integer`.
    pub candidate_count: i64,
    /// `chosen_action` — `String`.
    pub chosen_action: String,
    /// `confidence` — `Optional<Decimal>`.
    pub confidence: Option<crate::primitives::Decimal>,
    /// `fell_back_to` — `Optional<loom.run.SelectionStrategy>`.
    pub fell_back_to: Option<SelectionStrategy>,
    /// `latency_ms` — `Integer`.
    pub latency_ms: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
}

/// The states of `loom.run.SelectionRecord`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](selection_record_state::Marker), so [`SelectionRecord<S>`](SelectionRecord) can only ever rest in a real state.
pub mod selection_record_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Recorded {}
    }

    /// A declared state of `SelectionRecord`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SelectionRecordState;
    }

    /// `Recorded`. Where a new instance starts.
    pub struct Recorded;

    impl Marker for Recorded {
        const STATE: super::SelectionRecordState = super::SelectionRecordState::Recorded;
    }
}

/// SelectionRecord — `loom.run.SelectionRecord` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Recorded`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`SelectionRecordSnapshot`]
/// and [`SelectionRecordSnapshot::refine`].
pub struct SelectionRecord<S: selection_record_state::Marker> {
    data: SelectionRecordData,
    state: core::marker::PhantomData<S>,
}

impl<S: selection_record_state::Marker> SelectionRecord<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> SelectionRecordState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &SelectionRecordData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> SelectionRecordData {
        self.data
    }
}

impl SelectionRecord<selection_record_state::Recorded> {
    /// A new instance, resting in `Recorded` — the only state the lifecycle starts one in.
    pub fn new(data: SelectionRecordData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.SelectionRecord` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`SelectionRecordSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecordSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: SelectionRecordState,
    /// What it holds.
    pub data: SelectionRecordData,
}

/// An `SelectionRecord` in whichever declared state it was found.
pub enum AnySelectionRecord {
    /// Resting in `Recorded`.
    Recorded(SelectionRecord<selection_record_state::Recorded>),
}

impl SelectionRecordSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `SelectionRecordState` cannot spell one.
    pub fn refine(self) -> AnySelectionRecord {
        match self.state {
            SelectionRecordState::Recorded => AnySelectionRecord::Recorded(SelectionRecord {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnySelectionRecord {
    /// The state, as the runtime value.
    pub fn state(&self) -> SelectionRecordState {
        match self {
            Self::Recorded(_) => SelectionRecordState::Recorded,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SelectionRecordSnapshot {
        match self {
            Self::Recorded(instance) => SelectionRecordSnapshot {
                state: SelectionRecordState::Recorded,
                data: instance.into_data(),
            },
        }
    }
}

/// OverruleSelection — the input of `loom.run.OverruleSelection`.
///
/// Everything it can result in is [`OverruleSelectionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverruleSelection {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `replacement_id` — `loom.run.SelectionId`.
    pub replacement_id: SelectionId,
}

/// Everything `loom.run.OverruleSelection` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverruleSelectionOutcome {
    /// `overruled` — otherwise.
    Overruled {
        /// The `loom.run.SelectionOverruled` this outcome publishes.
        selection_overruled: SelectionOverruled,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SelectionStateConflict`.
        error: SelectionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// ProjectCatalogue — the input of `loom.run.ProjectCatalogue`.
///
/// Everything it can result in is [`ProjectCatalogueOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCatalogue {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `turn_id` — `loom.run.TurnId`.
    pub turn_id: TurnId,
    /// `frontier` — `String`.
    pub frontier: String,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `entries` — `List<loom.run.CatalogueEntry>`.
    pub entries: Vec<CatalogueEntry>,
}

/// Everything `loom.run.ProjectCatalogue` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectCatalogueOutcome {
    /// `catalogue-exists` — for an identity a record already carries.
    CatalogueExists {
        /// Why it was refused: `loom.run.CatalogueExists`.
        error: CatalogueExists,
    },
    /// `projected` — otherwise.
    Projected {
        /// The `loom.run.CatalogueProjected` this outcome publishes.
        catalogue_projected: CatalogueProjected,
    },
}

/// RecordSelection — the input of `loom.run.RecordSelection`.
///
/// Everything it can result in is [`RecordSelectionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSelection {
    /// `selection_record_id` — `loom.run.SelectionRecordId`.
    pub selection_record_id: SelectionRecordId,
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `candidate_count` — `Integer`.
    pub candidate_count: i64,
    /// `chosen_action` — `String`.
    pub chosen_action: String,
    /// `confidence` — `Optional<Decimal>`.
    pub confidence: Option<crate::primitives::Decimal>,
    /// `fell_back_to` — `Optional<loom.run.SelectionStrategy>`.
    pub fell_back_to: Option<SelectionStrategy>,
    /// `latency_ms` — `Integer`.
    pub latency_ms: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
}

/// Everything `loom.run.RecordSelection` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordSelectionOutcome {
    /// `record-exists` — for an identity a record already carries.
    RecordExists {
        /// Why it was refused: `loom.run.SelectionRecordExists`.
        error: SelectionRecordExists,
    },
    /// `selection-unknown` — when no `loom.run.Selection` carries the identity `input.selection_id` names.
    SelectionUnknown {
        /// Why it was refused: `loom.run.SelectionNotFound`.
        error: SelectionNotFound,
    },
    /// `recorded` — otherwise.
    Recorded {
        /// The `loom.run.SelectionRecorded` this outcome publishes.
        selection_recorded: SelectionRecorded,
    },
}

/// RequestArguments — the input of `loom.run.RequestArguments`.
///
/// Everything it can result in is [`RequestArgumentsOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestArguments {
    /// `argument_request_id` — `loom.run.ArgumentRequestId`.
    pub argument_request_id: ArgumentRequestId,
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// Everything `loom.run.RequestArguments` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestArgumentsOutcome {
    /// `selection-unknown` — when no `loom.run.Selection` carries the identity `input.selection_id` names.
    SelectionUnknown {
        /// Why it was refused: `loom.run.SelectionNotFound`.
        error: SelectionNotFound,
    },
    /// `selection-not-selected` — when the `loom.run.Selection` that `input.selection_id` names satisfies `state != Selected`.
    SelectionNotSelected {
        /// Why it was refused: `loom.run.SelectionNotSelected`.
        error: SelectionNotSelected,
    },
    /// `requested` — otherwise.
    Requested {
        /// The `loom.run.ArgumentsRequested` this outcome publishes.
        arguments_requested: ArgumentsRequested,
    },
}

/// RevalidateSelection — the input of `loom.run.RevalidateSelection`.
///
/// Everything it can result in is [`RevalidateSelectionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevalidateSelection {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `frontier_actions` — `List<String>`.
    pub frontier_actions: Vec<String>,
}

/// Everything `loom.run.RevalidateSelection` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevalidateSelectionOutcome {
    /// `stale-revision` — when the existing subject's stored fields satisfy `case_revision != input.case_revision`.
    StaleRevision {
        /// The `loom.run.SelectionStale` this outcome publishes.
        selection_stale: SelectionStale,
    },
    /// `not-in-frontier` — externally decided (the frontier action ids in the input do not list the selection's action).
    NotInFrontier {
        /// The `loom.run.SelectionNotInFrontier` this outcome publishes.
        selection_not_in_frontier: SelectionNotInFrontier,
    },
    /// `admitted` — otherwise.
    Admitted {
        /// The `loom.run.SelectionAdmitted` this outcome publishes.
        selection_admitted: SelectionAdmitted,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SelectionStateConflict`.
        error: SelectionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// SelectAction — the input of `loom.run.SelectAction`.
///
/// Everything it can result in is [`SelectActionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectAction {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `action` — `String`.
    pub action: String,
    /// `confidence` — `Optional<Decimal>`.
    pub confidence: Option<crate::primitives::Decimal>,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// Everything `loom.run.SelectAction` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectActionOutcome {
    /// `selection-exists` — for an identity a record already carries.
    SelectionExists {
        /// Why it was refused: `loom.run.SelectionExists`.
        error: SelectionExists,
    },
    /// `catalogue-unknown` — when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names.
    CatalogueUnknown {
        /// Why it was refused: `loom.run.CatalogueNotFound`.
        error: CatalogueNotFound,
    },
    /// `not-in-catalogue` — when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`.
    NotInCatalogue {
        /// Why it was refused: `loom.run.ActionNotInCatalogue`.
        error: ActionNotInCatalogue,
    },
    /// `revision-mismatch` — when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`.
    RevisionMismatch {
        /// Why it was refused: `loom.run.CatalogueRevisionMismatch`.
        error: CatalogueRevisionMismatch,
    },
    /// `selected` — otherwise.
    Selected {
        /// The `loom.run.ActionSelected` this outcome publishes.
        action_selected: ActionSelected,
    },
}

/// ActionSelected — the event `loom.run.ActionSelected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSelected {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `action` — `String`.
    pub action: String,
}

/// ArgumentsRequested — the event `loom.run.ArgumentsRequested`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentsRequested {
    /// `argument_request_id` — `loom.run.ArgumentRequestId`.
    pub argument_request_id: ArgumentRequestId,
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// CatalogueProjected — the event `loom.run.CatalogueProjected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueProjected {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `turn_id` — `loom.run.TurnId`.
    pub turn_id: TurnId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// SelectionAdmitted — the event `loom.run.SelectionAdmitted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionAdmitted {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// SelectionNotInFrontier — the event `loom.run.SelectionNotInFrontier`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionNotInFrontier {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `action` — `String`.
    pub action: String,
}

/// SelectionOverruled — the event `loom.run.SelectionOverruled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionOverruled {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `replacement_id` — `loom.run.SelectionId`.
    pub replacement_id: SelectionId,
}

/// SelectionRecorded — the event `loom.run.SelectionRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecorded {
    /// `selection_record_id` — `loom.run.SelectionRecordId`.
    pub selection_record_id: SelectionRecordId,
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `fell_back_to` — `Optional<loom.run.SelectionStrategy>`.
    pub fell_back_to: Option<SelectionStrategy>,
}

/// SelectionStale — the event `loom.run.SelectionStale`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStale {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `catalogue_revision` — `Integer`.
    pub catalogue_revision: i64,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// The declared error `loom.run.ActionNotInCatalogue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionNotInCatalogue {
    /// `action` — `String`.
    pub action: String,
}

/// The declared error `loom.run.CatalogueExists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueExists {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
}

/// The declared error `loom.run.CatalogueNotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueNotFound {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
}

/// The declared error `loom.run.CatalogueRevisionMismatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueRevisionMismatch {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `catalogue_revision` — `Integer`.
    pub catalogue_revision: i64,
}

/// The declared error `loom.run.SelectionExists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionExists {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// The declared error `loom.run.SelectionNotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionNotFound {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// The declared error `loom.run.SelectionNotSelected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionNotSelected {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
}

/// The declared error `loom.run.SelectionRecordExists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecordExists {
    /// `selection_record_id` — `loom.run.SelectionRecordId`.
    pub selection_record_id: SelectionRecordId,
}

/// The declared error `loom.run.SelectionStateConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStateConflict {
    /// `state` — `loom.run.Selection.State`.
    pub state: SelectionState,
}

/// Catalogues — one row of the view `loom.run.Catalogues`.
///
/// Projects `loom.run.ActionCatalogue` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogues {
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `state` — `loom.run.ActionCatalogue.State`.
    pub state: ActionCatalogueState,
}

/// SelectionRecords — one row of the view `loom.run.SelectionRecords`.
///
/// Projects `loom.run.SelectionRecord` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRecords {
    /// `selection_record_id` — `loom.run.SelectionRecordId`.
    pub selection_record_id: SelectionRecordId,
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `candidate_count` — `Integer`.
    pub candidate_count: i64,
    /// `chosen_action` — `String`.
    pub chosen_action: String,
    /// `fell_back_to` — `Optional<loom.run.SelectionStrategy>`.
    pub fell_back_to: Option<SelectionStrategy>,
    /// `latency_ms` — `Integer`.
    pub latency_ms: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `state` — `loom.run.SelectionRecord.State`.
    pub state: SelectionRecordState,
}

/// Selections — one row of the view `loom.run.Selections`.
///
/// Projects `loom.run.Selection` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selections {
    /// `selection_id` — `loom.run.SelectionId`.
    pub selection_id: SelectionId,
    /// `catalogue_id` — `loom.run.CatalogueId`.
    pub catalogue_id: CatalogueId,
    /// `action` — `String`.
    pub action: String,
    /// `strategy` — `loom.run.SelectionStrategy`.
    pub strategy: SelectionStrategy,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `replaced_by` — `Optional<loom.run.SelectionId>`.
    pub replaced_by: Option<SelectionId>,
    /// `state` — `loom.run.Selection.State`.
    pub state: SelectionState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `loom.run.OverruleSelection` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait OverruleSelectionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.OverruleSelection`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn overrule_selection(&mut self, input: super::OverruleSelection) -> Result<super::OverruleSelectionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.ProjectCatalogue` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ProjectCatalogueBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.ProjectCatalogue`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn project_catalogue(&mut self, input: super::ProjectCatalogue) -> Result<super::ProjectCatalogueOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.RecordSelection` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RecordSelectionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.RecordSelection`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn record_selection(&mut self, input: super::RecordSelection) -> Result<super::RecordSelectionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.RequestArguments` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RequestArgumentsBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.RequestArguments`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn request_arguments(&mut self, input: super::RequestArguments) -> Result<super::RequestArgumentsOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.RevalidateSelection` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RevalidateSelectionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.RevalidateSelection`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn revalidate_selection(&mut self, input: super::RevalidateSelection) -> Result<super::RevalidateSelectionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.SelectAction` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the guard `exists entry in entries: (entry.action == input.action)`, in `not-in-catalogue`.
    ///
    /// Contract: given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `selection-exists` for an identity a record already carries, error `loom.run.SelectionExists`; `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected`.
    pub trait SelectActionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.SelectAction`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn select_action(&mut self, input: super::SelectAction) -> Result<super::SelectActionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.run.Catalogues` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait CataloguesQuery {
        /// Serves `loom.run.Catalogues` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn catalogues(&self) -> Result<Vec<super::Catalogues>, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.run.SelectionRecords` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait SelectionRecordsQuery {
        /// Serves `loom.run.SelectionRecords` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn selection_records(&self) -> Result<Vec<super::SelectionRecords>, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.run.Selections` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait SelectionsQuery {
        /// Serves `loom.run.Selections` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn selections(&self) -> Result<Vec<super::Selections>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl SelectActionBehavior for Unimplemented {
        fn select_action(&mut self, _input: super::SelectAction) -> Result<super::SelectActionOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.run.SelectAction" })
        }
    }
}
