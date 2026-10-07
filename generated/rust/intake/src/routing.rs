// generated from intake v1
// model digest b3281773ee88313834099ce04aab0b9af436baedbe62ff1a432c305c3ed756d4
// contract digest eba8d734a7767f27148ac6b7d3a96ad427989af6d293d3c3496fba405caae95a
// do not edit: regenerate with `ess synthesize --layout crate`

//! Routing — `intake.routing`.
//!
//! An intent as given, the references extracted from its text, the protocol the router proposes for it, and the slice run that works the intent until it is blocked. A pick is a proposal, never authority.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// The states of `intake.routing.ExtractedReference`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ExtractedReference<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractedReferenceState {
    /// `Extracted`.
    Extracted,
}

/// The states of `intake.routing.Intent`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Intent<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentState {
    /// `Received`.
    Received,
}

/// IntentId — `intake.routing.IntentId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentId(pub crate::primitives::Uuid);

/// The states of `intake.routing.ProtocolPick`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ProtocolPick<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolPickState {
    /// `Proposed`.
    Proposed,
}

/// ReferenceKind — `intake.routing.ReferenceKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    /// `JiraIssue`.
    JiraIssue,
    /// `SlackMessage`.
    SlackMessage,
    /// `GitlabIssue`.
    GitlabIssue,
    /// `GitlabMergeRequest`.
    GitlabMergeRequest,
    /// `GithubIssue`.
    GithubIssue,
    /// `GithubPullRequest`.
    GithubPullRequest,
    /// `Url`.
    Url,
}

/// The states of `intake.routing.SliceRun`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `SliceRun<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceRunState {
    /// `Ended`.
    Ended,
}

/// StopReason — `intake.routing.StopReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// `ApprovalRequired`.
    ApprovalRequired,
    /// `NothingAdmissible`.
    NothingAdmissible,
    /// `StepBudget`.
    StepBudget,
    /// `NoLocalExecutor`.
    NoLocalExecutor,
    /// `Refused`.
    Refused,
    /// `ConfinementUnavailable`.
    ConfinementUnavailable,
    /// `Completed`.
    Completed,
}

/// What ExtractedReference — `intake.routing.ExtractedReference` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ExtractedReference<S>`], and at a boundary by [`ExtractedReferenceSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedReferenceData {
    /// The identity: `reference_id` — `Uuid`.
    pub reference_id: crate::primitives::Uuid,
    /// `intent_id` — `intake.routing.IntentId`.
    ///
    /// Carries `references`: `intake.routing.Intent` owns many `intake.routing.ExtractedReference`.
    pub intent_id: IntentId,
    /// `kind` — `intake.routing.ReferenceKind`.
    pub kind: ReferenceKind,
    /// `value` — `String`.
    pub value: String,
}

/// The states of `intake.routing.ExtractedReference`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](extracted_reference_state::Marker), so [`ExtractedReference<S>`](ExtractedReference) can only ever rest in a real state.
pub mod extracted_reference_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Extracted {}
    }

    /// A declared state of `ExtractedReference`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ExtractedReferenceState;
    }

    /// `Extracted`. Where a new instance starts.
    pub struct Extracted;

    impl Marker for Extracted {
        const STATE: super::ExtractedReferenceState = super::ExtractedReferenceState::Extracted;
    }
}

/// ExtractedReference — `intake.routing.ExtractedReference` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Extracted`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ExtractedReferenceSnapshot`]
/// and [`ExtractedReferenceSnapshot::refine`].
pub struct ExtractedReference<S: extracted_reference_state::Marker> {
    data: ExtractedReferenceData,
    state: core::marker::PhantomData<S>,
}

impl<S: extracted_reference_state::Marker> ExtractedReference<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ExtractedReferenceState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ExtractedReferenceData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ExtractedReferenceData {
        self.data
    }
}

impl ExtractedReference<extracted_reference_state::Extracted> {
    /// A new instance, resting in `Extracted` — the only state the lifecycle starts one in.
    pub fn new(data: ExtractedReferenceData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.ExtractedReference` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ExtractedReferenceSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedReferenceSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ExtractedReferenceState,
    /// What it holds.
    pub data: ExtractedReferenceData,
}

/// An `ExtractedReference` in whichever declared state it was found.
pub enum AnyExtractedReference {
    /// Resting in `Extracted`.
    Extracted(ExtractedReference<extracted_reference_state::Extracted>),
}

impl ExtractedReferenceSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ExtractedReferenceState` cannot spell one.
    pub fn refine(self) -> AnyExtractedReference {
        match self.state {
            ExtractedReferenceState::Extracted => AnyExtractedReference::Extracted(ExtractedReference {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyExtractedReference {
    /// The state, as the runtime value.
    pub fn state(&self) -> ExtractedReferenceState {
        match self {
            Self::Extracted(_) => ExtractedReferenceState::Extracted,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ExtractedReferenceSnapshot {
        match self {
            Self::Extracted(instance) => ExtractedReferenceSnapshot {
                state: ExtractedReferenceState::Extracted,
                data: instance.into_data(),
            },
        }
    }
}

/// What Intent — `intake.routing.Intent` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Intent<S>`], and at a boundary by [`IntentSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentData {
    /// The identity: `intent_id` — `intake.routing.IntentId`.
    pub intent_id: IntentId,
    /// `text` — `String`.
    pub text: String,
}

/// The states of `intake.routing.Intent`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](intent_state::Marker), so [`Intent<S>`](Intent) can only ever rest in a real state.
pub mod intent_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Received {}
    }

    /// A declared state of `Intent`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::IntentState;
    }

    /// `Received`. Where a new instance starts.
    pub struct Received;

    impl Marker for Received {
        const STATE: super::IntentState = super::IntentState::Received;
    }
}

/// Intent — `intake.routing.Intent` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Received`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`IntentSnapshot`]
/// and [`IntentSnapshot::refine`].
pub struct Intent<S: intent_state::Marker> {
    data: IntentData,
    state: core::marker::PhantomData<S>,
}

impl<S: intent_state::Marker> Intent<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> IntentState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &IntentData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> IntentData {
        self.data
    }
}

impl Intent<intent_state::Received> {
    /// A new instance, resting in `Received` — the only state the lifecycle starts one in.
    pub fn new(data: IntentData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.Intent` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`IntentSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: IntentState,
    /// What it holds.
    pub data: IntentData,
}

/// An `Intent` in whichever declared state it was found.
pub enum AnyIntent {
    /// Resting in `Received`.
    Received(Intent<intent_state::Received>),
}

impl IntentSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `IntentState` cannot spell one.
    pub fn refine(self) -> AnyIntent {
        match self.state {
            IntentState::Received => AnyIntent::Received(Intent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyIntent {
    /// The state, as the runtime value.
    pub fn state(&self) -> IntentState {
        match self {
            Self::Received(_) => IntentState::Received,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> IntentSnapshot {
        match self {
            Self::Received(instance) => IntentSnapshot {
                state: IntentState::Received,
                data: instance.into_data(),
            },
        }
    }
}

/// What ProtocolPick — `intake.routing.ProtocolPick` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ProtocolPick<S>`], and at a boundary by [`ProtocolPickSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolPickData {
    /// The identity: `pick_id` — `Uuid`.
    pub pick_id: crate::primitives::Uuid,
    /// `intent_id` — `intake.routing.IntentId`.
    ///
    /// Carries `pick`: `intake.routing.Intent` owns one `intake.routing.ProtocolPick`.
    pub intent_id: IntentId,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `confidence` — `Decimal`.
    pub confidence: crate::primitives::Decimal,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// The states of `intake.routing.ProtocolPick`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](protocol_pick_state::Marker), so [`ProtocolPick<S>`](ProtocolPick) can only ever rest in a real state.
pub mod protocol_pick_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Proposed {}
    }

    /// A declared state of `ProtocolPick`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ProtocolPickState;
    }

    /// `Proposed`. Where a new instance starts.
    pub struct Proposed;

    impl Marker for Proposed {
        const STATE: super::ProtocolPickState = super::ProtocolPickState::Proposed;
    }
}

/// ProtocolPick — `intake.routing.ProtocolPick` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Proposed`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ProtocolPickSnapshot`]
/// and [`ProtocolPickSnapshot::refine`].
pub struct ProtocolPick<S: protocol_pick_state::Marker> {
    data: ProtocolPickData,
    state: core::marker::PhantomData<S>,
}

impl<S: protocol_pick_state::Marker> ProtocolPick<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ProtocolPickState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ProtocolPickData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ProtocolPickData {
        self.data
    }
}

impl ProtocolPick<protocol_pick_state::Proposed> {
    /// A new instance, resting in `Proposed` — the only state the lifecycle starts one in.
    pub fn new(data: ProtocolPickData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.ProtocolPick` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ProtocolPickSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolPickSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ProtocolPickState,
    /// What it holds.
    pub data: ProtocolPickData,
}

/// An `ProtocolPick` in whichever declared state it was found.
pub enum AnyProtocolPick {
    /// Resting in `Proposed`.
    Proposed(ProtocolPick<protocol_pick_state::Proposed>),
}

impl ProtocolPickSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ProtocolPickState` cannot spell one.
    pub fn refine(self) -> AnyProtocolPick {
        match self.state {
            ProtocolPickState::Proposed => AnyProtocolPick::Proposed(ProtocolPick {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyProtocolPick {
    /// The state, as the runtime value.
    pub fn state(&self) -> ProtocolPickState {
        match self {
            Self::Proposed(_) => ProtocolPickState::Proposed,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ProtocolPickSnapshot {
        match self {
            Self::Proposed(instance) => ProtocolPickSnapshot {
                state: ProtocolPickState::Proposed,
                data: instance.into_data(),
            },
        }
    }
}

/// What SliceRun — `intake.routing.SliceRun` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`SliceRun<S>`], and at a boundary by [`SliceRunSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRunData {
    /// The identity: `run_id` — `Uuid`.
    pub run_id: crate::primitives::Uuid,
    /// `intent_id` — `intake.routing.IntentId`.
    ///
    /// Carries `intent`: `intake.routing.SliceRun` references one `intake.routing.Intent`.
    pub intent_id: IntentId,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `steps` — `Integer`.
    pub steps: i64,
    /// `stop_reason` — `intake.routing.StopReason`.
    pub stop_reason: StopReason,
}

/// The states of `intake.routing.SliceRun`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](slice_run_state::Marker), so [`SliceRun<S>`](SliceRun) can only ever rest in a real state.
pub mod slice_run_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Ended {}
    }

    /// A declared state of `SliceRun`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SliceRunState;
    }

    /// `Ended`. Where a new instance starts.
    pub struct Ended;

    impl Marker for Ended {
        const STATE: super::SliceRunState = super::SliceRunState::Ended;
    }
}

/// SliceRun — `intake.routing.SliceRun` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Ended`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`SliceRunSnapshot`]
/// and [`SliceRunSnapshot::refine`].
pub struct SliceRun<S: slice_run_state::Marker> {
    data: SliceRunData,
    state: core::marker::PhantomData<S>,
}

impl<S: slice_run_state::Marker> SliceRun<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> SliceRunState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &SliceRunData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> SliceRunData {
        self.data
    }
}

impl SliceRun<slice_run_state::Ended> {
    /// A new instance, resting in `Ended` — the only state the lifecycle starts one in.
    pub fn new(data: SliceRunData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.SliceRun` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`SliceRunSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRunSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: SliceRunState,
    /// What it holds.
    pub data: SliceRunData,
}

/// An `SliceRun` in whichever declared state it was found.
pub enum AnySliceRun {
    /// Resting in `Ended`.
    Ended(SliceRun<slice_run_state::Ended>),
}

impl SliceRunSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `SliceRunState` cannot spell one.
    pub fn refine(self) -> AnySliceRun {
        match self.state {
            SliceRunState::Ended => AnySliceRun::Ended(SliceRun {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnySliceRun {
    /// The state, as the runtime value.
    pub fn state(&self) -> SliceRunState {
        match self {
            Self::Ended(_) => SliceRunState::Ended,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SliceRunSnapshot {
        match self {
            Self::Ended(instance) => SliceRunSnapshot {
                state: SliceRunState::Ended,
                data: instance.into_data(),
            },
        }
    }
}
