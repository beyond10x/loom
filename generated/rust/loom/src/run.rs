// generated from loom v1
// model digest f3af5c9e850163e564232367b8200696d02f52deb9b2fe97676b33525d970bd0
// contract digest b22a63cd15b4b14521a04844fc21145ce4639632cccce11fd691fe08e65f0bec
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

/// The states of `loom.run.Compaction`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Compaction<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionState {
    /// `Recorded`.
    Recorded,
}

/// CompactionId — `loom.run.CompactionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionId(pub crate::primitives::Uuid);

/// CredentialKind — `loom.run.CredentialKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    /// `ApiKey`.
    ApiKey,
    /// `Oauth`.
    Oauth,
}

/// CredentialReference — `loom.run.CredentialReference`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialReference(pub String);

/// ReportedUsage — `loom.run.ReportedUsage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedUsage {
    /// `model` — `String`.
    pub model: String,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `cached_input_tokens` — `Integer`.
    pub cached_input_tokens: i64,
    /// `cache_creation_input_tokens` — `Optional<Integer>`.
    pub cache_creation_input_tokens: Option<i64>,
}

/// RunEnding — `loom.run.RunEnding`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnding {
    /// `Answered`.
    Answered,
    /// `Stopped`.
    Stopped,
    /// `Failed`.
    Failed,
}

/// The states of `loom.run.Selection`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Selection<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// `Admitted`.
    Admitted,
    /// `Refused`.
    Refused,
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
    /// `Filed`.
    Filed,
    /// `Interrupted`.
    Interrupted,
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

/// WireCredential — `loom.run.WireCredential`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCredential {
    /// `reference` — `loom.run.CredentialReference`.
    pub reference: CredentialReference,
    /// `kind` — `loom.run.CredentialKind`.
    pub kind: CredentialKind,
}

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

/// What Compaction — `loom.run.Compaction` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Compaction<S>`], and at a boundary by [`CompactionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionData {
    /// The identity: `compaction_id` — `loom.run.CompactionId`.
    pub compaction_id: CompactionId,
    /// `session_id` — `loom.run.SessionId`.
    ///
    /// Carries `compactions`: `loom.run.Session` owns many `loom.run.Compaction`.
    pub session_id: SessionId,
    /// `usage` — `Optional<loom.run.ReportedUsage>`.
    pub usage: Option<ReportedUsage>,
}

/// The states of `loom.run.Compaction`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](compaction_state::Marker), so [`Compaction<S>`](Compaction) can only ever rest in a real state.
pub mod compaction_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Recorded {}
    }

    /// A declared state of `Compaction`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::CompactionState;
    }

    /// `Recorded`. Where a new instance starts.
    pub struct Recorded;

    impl Marker for Recorded {
        const STATE: super::CompactionState = super::CompactionState::Recorded;
    }
}

/// Compaction — `loom.run.Compaction` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Recorded`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`CompactionSnapshot`]
/// and [`CompactionSnapshot::refine`].
pub struct Compaction<S: compaction_state::Marker> {
    data: CompactionData,
    state: core::marker::PhantomData<S>,
}

impl<S: compaction_state::Marker> Compaction<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> CompactionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &CompactionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> CompactionData {
        self.data
    }
}

impl Compaction<compaction_state::Recorded> {
    /// A new instance, resting in `Recorded` — the only state the lifecycle starts one in.
    pub fn new(data: CompactionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.run.Compaction` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`CompactionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: CompactionState,
    /// What it holds.
    pub data: CompactionData,
}

/// An `Compaction` in whichever declared state it was found.
pub enum AnyCompaction {
    /// Resting in `Recorded`.
    Recorded(Compaction<compaction_state::Recorded>),
}

impl CompactionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `CompactionState` cannot spell one.
    pub fn refine(self) -> AnyCompaction {
        match self.state {
            CompactionState::Recorded => AnyCompaction::Recorded(Compaction {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyCompaction {
    /// The state, as the runtime value.
    pub fn state(&self) -> CompactionState {
        match self {
            Self::Recorded(_) => CompactionState::Recorded,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> CompactionSnapshot {
        match self {
            Self::Recorded(instance) => CompactionSnapshot {
                state: CompactionState::Recorded,
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
    /// `wire` — `String`.
    pub wire: String,
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
        impl Sealed for super::Filed {}
        impl Sealed for super::Interrupted {}
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

    /// `Filed`.
    pub struct Filed;

    impl Marker for Filed {
        const STATE: super::SessionState = super::SessionState::Filed;
    }

    /// `Interrupted`.
    pub struct Interrupted;

    impl Marker for Interrupted {
        const STATE: super::SessionState = super::SessionState::Interrupted;
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

impl Session<session_state::Active> {
    /// `file` — `Active` → `Filed`. Taken by the `filed` outcome of `loom.run.FileSession`.
    pub fn file(self) -> Session<session_state::Filed> {
        Session {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `release` — `Active` → `Filed`. Taken by the `released` outcome of `loom.run.ReleaseSession`.
    pub fn release(self) -> Session<session_state::Filed> {
        Session {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `interrupt` — `Active` → `Interrupted`. Taken by the `interrupted` outcome of `loom.run.InterruptSession`.
    pub fn interrupt(self) -> Session<session_state::Interrupted> {
        Session {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl Session<session_state::Filed> {
    /// `resume` — `Filed` → `Active`. Taken by the `resumed` outcome of `loom.run.ResumeSession`.
    pub fn resume(self) -> Session<session_state::Active> {
        Session {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl Session<session_state::Interrupted> {
    /// `resume` — `Interrupted` → `Active`. Taken by the `resumed` outcome of `loom.run.ResumeSession`.
    pub fn resume(self) -> Session<session_state::Active> {
        Session {
            data: self.data,
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
    /// Resting in `Filed`.
    Filed(Session<session_state::Filed>),
    /// Resting in `Interrupted`.
    Interrupted(Session<session_state::Interrupted>),
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
            SessionState::Filed => AnySession::Filed(Session {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            SessionState::Interrupted => AnySession::Interrupted(Session {
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
            Self::Filed(_) => SessionState::Filed,
            Self::Interrupted(_) => SessionState::Interrupted,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SessionSnapshot {
        match self {
            Self::Active(instance) => SessionSnapshot {
                state: SessionState::Active,
                data: instance.into_data(),
            },
            Self::Filed(instance) => SessionSnapshot {
                state: SessionState::Filed,
                data: instance.into_data(),
            },
            Self::Interrupted(instance) => SessionSnapshot {
                state: SessionState::Interrupted,
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
    /// `items` — `List<String>`.
    pub items: Vec<String>,
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

/// FileSession — the input of `loom.run.FileSession`.
///
/// Everything it can result in is [`FileSessionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSession {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `ending` — `loom.run.RunEnding`.
    pub ending: RunEnding,
}

/// Everything `loom.run.FileSession` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSessionOutcome {
    /// `filed` — otherwise.
    Filed {
        /// The `loom.run.SessionFiled` this outcome publishes.
        session_filed: SessionFiled,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SessionStateConflict`.
        error: SessionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// InterruptSession — the input of `loom.run.InterruptSession`.
///
/// Everything it can result in is [`InterruptSessionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptSession {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// Everything `loom.run.InterruptSession` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptSessionOutcome {
    /// `interrupted` — otherwise.
    Interrupted {
        /// The `loom.run.SessionInterrupted` this outcome publishes.
        session_interrupted: SessionInterrupted,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SessionStateConflict`.
        error: SessionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// OpenSession — the input of `loom.run.OpenSession`.
///
/// Everything it can result in is [`OpenSessionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSession {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `commission_run` — `loom.run.CommissionRunId`.
    pub commission_run: CommissionRunId,
    /// `wire` — `String`.
    pub wire: String,
}

/// Everything `loom.run.OpenSession` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenSessionOutcome {
    /// `session-exists` — for an identity a record already carries.
    SessionExists {
        /// Why it was refused: `loom.run.SessionExists`.
        error: SessionExists,
    },
    /// `opened` — otherwise.
    Opened {
        /// The `loom.run.SessionOpened` this outcome publishes.
        session_opened: SessionOpened,
    },
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

/// RecordCompaction — the input of `loom.run.RecordCompaction`.
///
/// Everything it can result in is [`RecordCompactionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCompaction {
    /// `compaction_id` — `loom.run.CompactionId`.
    pub compaction_id: CompactionId,
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `usage` — `Optional<loom.run.ReportedUsage>`.
    pub usage: Option<ReportedUsage>,
}

/// Everything `loom.run.RecordCompaction` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordCompactionOutcome {
    /// `session-unknown` — when no `loom.run.Session` carries the identity `input.session_id` names.
    SessionUnknown {
        /// Why it was refused: `loom.run.SessionNotFound`.
        error: SessionNotFound,
    },
    /// `session-not-active` — when the `loom.run.Session` that `input.session_id` names satisfies `state != Active`.
    SessionNotActive {
        /// Why it was refused: `loom.run.SessionNotActive`.
        error: SessionNotActive,
    },
    /// `recorded` — otherwise.
    Recorded {
        /// The `loom.run.SessionCompacted` this outcome publishes.
        session_compacted: SessionCompacted,
    },
}

/// RecordTurn — the input of `loom.run.RecordTurn`.
///
/// Everything it can result in is [`RecordTurnOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTurn {
    /// `turn_id` — `loom.run.TurnId`.
    pub turn_id: TurnId,
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `index` — `Integer`.
    pub index: i64,
    /// `items` — `List<String>`.
    pub items: Vec<String>,
}

/// Everything `loom.run.RecordTurn` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordTurnOutcome {
    /// `session-unknown` — when no `loom.run.Session` carries the identity `input.session_id` names.
    SessionUnknown {
        /// Why it was refused: `loom.run.SessionNotFound`.
        error: SessionNotFound,
    },
    /// `session-not-active` — when the `loom.run.Session` that `input.session_id` names satisfies `state != Active`.
    SessionNotActive {
        /// Why it was refused: `loom.run.SessionNotActive`.
        error: SessionNotActive,
    },
    /// `recorded` — otherwise.
    Recorded {
        /// The `loom.run.TurnRecorded` this outcome publishes.
        turn_recorded: TurnRecorded,
    },
}

/// ReleaseSession — the input of `loom.run.ReleaseSession`.
///
/// Everything it can result in is [`ReleaseSessionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseSession {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// Everything `loom.run.ReleaseSession` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseSessionOutcome {
    /// `released` — otherwise.
    Released {
        /// The `loom.run.SessionReleased` this outcome publishes.
        session_released: SessionReleased,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SessionStateConflict`.
        error: SessionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
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

/// ResumeSession — the input of `loom.run.ResumeSession`.
///
/// Everything it can result in is [`ResumeSessionOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeSession {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `wire` — `String`.
    pub wire: String,
}

/// Everything `loom.run.ResumeSession` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeSessionOutcome {
    /// `cross-wire` — when the existing subject's stored fields satisfy `wire != input.wire`.
    CrossWire {
        /// Why it was refused: `loom.run.SessionWireMismatch`.
        error: SessionWireMismatch,
    },
    /// `resumed` — otherwise.
    Resumed {
        /// The `loom.run.SessionResumed` this outcome publishes.
        session_resumed: SessionResumed,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `loom.run.SessionStateConflict`.
        error: SessionStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
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

/// SessionCompacted — the event `loom.run.SessionCompacted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCompacted {
    /// `compaction_id` — `loom.run.CompactionId`.
    pub compaction_id: CompactionId,
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `usage` — `Optional<loom.run.ReportedUsage>`.
    pub usage: Option<ReportedUsage>,
}

/// SessionFiled — the event `loom.run.SessionFiled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFiled {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `ending` — `loom.run.RunEnding`.
    pub ending: RunEnding,
}

/// SessionInterrupted — the event `loom.run.SessionInterrupted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInterrupted {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// SessionOpened — the event `loom.run.SessionOpened`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOpened {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `commission_run` — `loom.run.CommissionRunId`.
    pub commission_run: CommissionRunId,
    /// `wire` — `String`.
    pub wire: String,
}

/// SessionReleased — the event `loom.run.SessionReleased`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReleased {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `ending` — `loom.run.RunEnding`.
    pub ending: RunEnding,
}

/// SessionResumed — the event `loom.run.SessionResumed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResumed {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `wire` — `String`.
    pub wire: String,
}

/// TurnRecorded — the event `loom.run.TurnRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecorded {
    /// `turn_id` — `loom.run.TurnId`.
    pub turn_id: TurnId,
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `index` — `Integer`.
    pub index: i64,
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

/// The declared error `loom.run.SelectionStateConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStateConflict {
    /// `state` — `loom.run.Selection.State`.
    pub state: SelectionState,
}

/// The declared error `loom.run.SessionExists`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionExists {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// The declared error `loom.run.SessionNotActive`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNotActive {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// The declared error `loom.run.SessionNotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNotFound {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
}

/// The declared error `loom.run.SessionStateConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStateConflict {
    /// `state` — `loom.run.Session.State`.
    pub state: SessionState,
}

/// The declared error `loom.run.SessionWireMismatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionWireMismatch {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `session_wire` — `String`.
    pub session_wire: String,
    /// `wire` — `String`.
    pub wire: String,
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
    /// `state` — `loom.run.Selection.State`.
    pub state: SelectionState,
}

/// Sessions — one row of the view `loom.run.Sessions`.
///
/// Projects `loom.run.Session` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessions {
    /// `session_id` — `loom.run.SessionId`.
    pub session_id: SessionId,
    /// `commission_run` — `loom.run.CommissionRunId`.
    pub commission_run: CommissionRunId,
    /// `wire` — `String`.
    pub wire: String,
    /// `state` — `loom.run.Session.State`.
    pub state: SessionState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `loom.run.FileSession` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait FileSessionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.FileSession`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn file_session(&mut self, input: super::FileSession) -> Result<super::FileSessionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.InterruptSession` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait InterruptSessionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.InterruptSession`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn interrupt_session(&mut self, input: super::InterruptSession) -> Result<super::InterruptSessionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.OpenSession` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait OpenSessionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.OpenSession`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn open_session(&mut self, input: super::OpenSession) -> Result<super::OpenSessionOutcome, crate::obligation::UnmetObligation>;
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

    /// The behaviour `loom.run.RecordCompaction` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RecordCompactionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.RecordCompaction`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn record_compaction(&mut self, input: super::RecordCompaction) -> Result<super::RecordCompactionOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.RecordTurn` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait RecordTurnBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.RecordTurn`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn record_turn(&mut self, input: super::RecordTurn) -> Result<super::RecordTurnOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.run.ReleaseSession` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ReleaseSessionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.ReleaseSession`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn release_session(&mut self, input: super::ReleaseSession) -> Result<super::ReleaseSessionOutcome, crate::obligation::UnmetObligation>;
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

    /// The behaviour `loom.run.ResumeSession` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ResumeSessionBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.run.ResumeSession`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn resume_session(&mut self, input: super::ResumeSession) -> Result<super::ResumeSessionOutcome, crate::obligation::UnmetObligation>;
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
    /// Contract: given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected`.
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

    /// The query `loom.run.Sessions` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait SessionsQuery {
        /// Serves `loom.run.Sessions` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn sessions(&self) -> Result<Vec<super::Sessions>, crate::obligation::UnmetObligation>;
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
