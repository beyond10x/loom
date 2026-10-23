// generated from intake v1
// model digest c5457c9e4735af8938031585724e656e9eaa6bc6ff00f6761d8287a591db7f8c
// contract digest f815c76fa97e91e076dd53cce6f3557b05973d54a2494e2d06a54f7355fd9c76
// do not edit: regenerate with `ess synthesize --layout crate`

//! Routing — `intake.routing`.
//!
//! An intent as given, the references extracted from its text, the protocol the router proposes for it, the slice run that works the intent until it is blocked, and the evidence that run submits from what it did itself. A pick is a proposal, never authority.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// AcceptedRoute — `intake.routing.AcceptedRoute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedRoute {
    /// `protocol` — `String`.
    pub protocol: String,
    /// `intent_revision` — `String`.
    pub intent_revision: String,
    /// `catalog` — `List<intake.routing.CatalogDigest>`.
    pub catalog: Vec<CatalogDigest>,
    /// `context_policy` — `intake.context.ContextPolicy`.
    pub context_policy: crate::context::ContextPolicy,
    /// `threshold` — `Decimal`.
    pub threshold: crate::primitives::Decimal,
}

/// ActionRequestId — `intake.routing.ActionRequestId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequestId(pub crate::primitives::Uuid);

/// AgentRevisionId — `intake.routing.AgentRevisionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRevisionId(pub crate::primitives::Uuid);

/// CaseId — `intake.routing.CaseId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseId(pub String);

/// CaseInputs — `intake.routing.CaseInputs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseInputs {
    /// `risk` — `intake.routing.Risk`.
    pub risk: Risk,
    /// `change_kind` — `intake.routing.ChangeKind`.
    pub change_kind: ChangeKind,
    /// `affects_security_boundary` — `Boolean`.
    pub affects_security_boundary: bool,
}

/// CatalogDigest — `intake.routing.CatalogDigest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDigest {
    /// `name` — `String`.
    pub name: String,
    /// `digest` — `String`.
    pub digest: String,
}

/// ChangeKind — `intake.routing.ChangeKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// `Feature`.
    Feature,
    /// `Bugfix`.
    Bugfix,
    /// `Refactor`.
    Refactor,
}

/// CommissionId — `intake.routing.CommissionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionId(pub crate::primitives::Uuid);

/// The states of `intake.routing.ExtractedReference`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ExtractedReference<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractedReferenceState {
    /// `Extracted`.
    Extracted,
}

/// The states of `intake.routing.Grant`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Grant<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantState {
    /// `Granted`.
    Granted,
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

/// MandateApprovalEntry — `intake.routing.MandateApprovalEntry`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MandateApprovalEntry {
    /// Tagged `ApprovalRecorded` — `intake.routing.MandateApprovalRecorded`.
    ApprovalRecorded(MandateApprovalRecorded),
    /// Tagged `ApprovedDecisionRecorded` — `intake.routing.MandateApprovedDecision`.
    ApprovedDecisionRecorded(MandateApprovedDecision),
}

/// MandateApprovalId — `intake.routing.MandateApprovalId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandateApprovalId(pub crate::primitives::Uuid);

/// MandateApprovalRecorded — `intake.routing.MandateApprovalRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandateApprovalRecorded {
    /// `approval` — `intake.routing.MandateApprovalId`.
    pub approval: MandateApprovalId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `organization` — `Uuid`.
    pub organization: crate::primitives::Uuid,
    /// `subject` — `Uuid`.
    pub subject: crate::primitives::Uuid,
    /// `actor` — `Optional<Uuid>`.
    pub actor: Option<crate::primitives::Uuid>,
    /// `action` — `String`.
    pub action: String,
    /// `resource_type` — `String`.
    pub resource_type: String,
    /// `resource_id` — `Uuid`.
    pub resource_id: crate::primitives::Uuid,
    /// `approver` — `Uuid`.
    pub approver: crate::primitives::Uuid,
    /// `recorded_at` — `Timestamp`.
    pub recorded_at: crate::primitives::Timestamp,
    /// `expires_at` — `Timestamp`.
    pub expires_at: crate::primitives::Timestamp,
}

/// MandateApprovedDecision — `intake.routing.MandateApprovedDecision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandateApprovedDecision {
    /// `approval` — `intake.routing.MandateApprovalId`.
    pub approval: MandateApprovalId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `context` — `Json`.
    pub context: crate::json::Value,
    /// `decision` — `Json`.
    pub decision: crate::json::Value,
    /// `recorded_at` — `Timestamp`.
    pub recorded_at: crate::primitives::Timestamp,
}

/// The states of `intake.routing.MergeHandoff`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `MergeHandoff<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeHandoffState {
    /// `Recorded`.
    Recorded,
}

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

/// The states of `intake.routing.ResumeContext`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ResumeContext<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeContextState {
    /// `Kept`.
    Kept,
}

/// Risk — `intake.routing.Risk`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// `Trivial`.
    Trivial,
    /// `Standard`.
    Standard,
    /// `Elevated`.
    Elevated,
    /// `Critical`.
    Critical,
}

/// The states of `intake.routing.SliceEvidence`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `SliceEvidence<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceEvidenceState {
    /// `Submitted`.
    Submitted,
}

/// SliceEvidenceKind — `intake.routing.SliceEvidenceKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceEvidenceKind {
    /// `TestResult`.
    TestResult,
    /// `RegressionTestResult`.
    RegressionTestResult,
    /// `ImplementationChange`.
    ImplementationChange,
}

/// SliceEvidenceResult — `intake.routing.SliceEvidenceResult`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceEvidenceResult {
    /// `Pass`.
    Pass,
    /// `Fail`.
    Fail,
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

/// What Grant — `intake.routing.Grant` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Grant<S>`], and at a boundary by [`GrantSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantData {
    /// The identity: `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `action` — `String`.
    pub action: String,
    /// `capability` — `String`.
    pub capability: String,
    /// `granted` — `Boolean`.
    pub granted: bool,
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `granted_at` — `Timestamp`.
    pub granted_at: crate::primitives::Timestamp,
    /// `mandate_approval` — `Optional<intake.routing.MandateApprovalId>`.
    pub mandate_approval: Option<MandateApprovalId>,
}

/// The states of `intake.routing.Grant`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](grant_state::Marker), so [`Grant<S>`](Grant) can only ever rest in a real state.
pub mod grant_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Granted {}
    }

    /// A declared state of `Grant`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::GrantState;
    }

    /// `Granted`. Where a new instance starts.
    pub struct Granted;

    impl Marker for Granted {
        const STATE: super::GrantState = super::GrantState::Granted;
    }
}

/// Grant — `intake.routing.Grant` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Granted`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`GrantSnapshot`]
/// and [`GrantSnapshot::refine`].
pub struct Grant<S: grant_state::Marker> {
    data: GrantData,
    state: core::marker::PhantomData<S>,
}

impl<S: grant_state::Marker> Grant<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> GrantState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &GrantData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> GrantData {
        self.data
    }
}

impl Grant<grant_state::Granted> {
    /// A new instance, resting in `Granted` — the only state the lifecycle starts one in.
    pub fn new(data: GrantData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.Grant` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`GrantSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: GrantState,
    /// What it holds.
    pub data: GrantData,
}

/// An `Grant` in whichever declared state it was found.
pub enum AnyGrant {
    /// Resting in `Granted`.
    Granted(Grant<grant_state::Granted>),
}

impl GrantSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `GrantState` cannot spell one.
    pub fn refine(self) -> AnyGrant {
        match self.state {
            GrantState::Granted => AnyGrant::Granted(Grant {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyGrant {
    /// The state, as the runtime value.
    pub fn state(&self) -> GrantState {
        match self {
            Self::Granted(_) => GrantState::Granted,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> GrantSnapshot {
        match self {
            Self::Granted(instance) => GrantSnapshot {
                state: GrantState::Granted,
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

/// What MergeHandoff — `intake.routing.MergeHandoff` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`MergeHandoff<S>`], and at a boundary by [`MergeHandoffSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandoffData {
    /// The identity: `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `head` — `String`.
    pub head: String,
}

/// The states of `intake.routing.MergeHandoff`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](merge_handoff_state::Marker), so [`MergeHandoff<S>`](MergeHandoff) can only ever rest in a real state.
pub mod merge_handoff_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Recorded {}
    }

    /// A declared state of `MergeHandoff`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::MergeHandoffState;
    }

    /// `Recorded`. Where a new instance starts.
    pub struct Recorded;

    impl Marker for Recorded {
        const STATE: super::MergeHandoffState = super::MergeHandoffState::Recorded;
    }
}

/// MergeHandoff — `intake.routing.MergeHandoff` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Recorded`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`MergeHandoffSnapshot`]
/// and [`MergeHandoffSnapshot::refine`].
pub struct MergeHandoff<S: merge_handoff_state::Marker> {
    data: MergeHandoffData,
    state: core::marker::PhantomData<S>,
}

impl<S: merge_handoff_state::Marker> MergeHandoff<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> MergeHandoffState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &MergeHandoffData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> MergeHandoffData {
        self.data
    }
}

impl MergeHandoff<merge_handoff_state::Recorded> {
    /// A new instance, resting in `Recorded` — the only state the lifecycle starts one in.
    pub fn new(data: MergeHandoffData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.MergeHandoff` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`MergeHandoffSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandoffSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: MergeHandoffState,
    /// What it holds.
    pub data: MergeHandoffData,
}

/// An `MergeHandoff` in whichever declared state it was found.
pub enum AnyMergeHandoff {
    /// Resting in `Recorded`.
    Recorded(MergeHandoff<merge_handoff_state::Recorded>),
}

impl MergeHandoffSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `MergeHandoffState` cannot spell one.
    pub fn refine(self) -> AnyMergeHandoff {
        match self.state {
            MergeHandoffState::Recorded => AnyMergeHandoff::Recorded(MergeHandoff {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyMergeHandoff {
    /// The state, as the runtime value.
    pub fn state(&self) -> MergeHandoffState {
        match self {
            Self::Recorded(_) => MergeHandoffState::Recorded,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> MergeHandoffSnapshot {
        match self {
            Self::Recorded(instance) => MergeHandoffSnapshot {
                state: MergeHandoffState::Recorded,
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

/// What ResumeContext — `intake.routing.ResumeContext` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ResumeContext<S>`], and at a boundary by [`ResumeContextSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextData {
    /// The identity: `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_id` — `intake.routing.CaseId`.
    pub case_id: CaseId,
    /// `agent_revision_id` — `intake.routing.AgentRevisionId`.
    pub agent_revision_id: AgentRevisionId,
    /// `intent` — `String`.
    pub intent: String,
    /// `route` — `intake.routing.AcceptedRoute`.
    pub route: AcceptedRoute,
    /// `workspace` — `Optional<String>`.
    pub workspace: Option<String>,
    /// `test_command` — `List<String>`.
    pub test_command: Vec<String>,
    /// `test_paths` — `List<String>`.
    pub test_paths: Vec<String>,
    /// `regression_test_command` — `Optional<String>`.
    pub regression_test_command: Option<String>,
    /// `max_steps` — `Integer`.
    pub max_steps: i64,
    /// `confinement` — `intake.confinement.Backend`.
    pub confinement: crate::confinement::Backend,
    /// `cgroup_root` — `Optional<String>`.
    pub cgroup_root: Option<String>,
    /// `model` — `String`.
    pub model: String,
    /// `catalog` — `Optional<String>`.
    pub catalog: Option<String>,
    /// `config` — `Optional<String>`.
    pub config: Option<String>,
}

/// The states of `intake.routing.ResumeContext`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](resume_context_state::Marker), so [`ResumeContext<S>`](ResumeContext) can only ever rest in a real state.
pub mod resume_context_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Kept {}
    }

    /// A declared state of `ResumeContext`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ResumeContextState;
    }

    /// `Kept`. Where a new instance starts.
    pub struct Kept;

    impl Marker for Kept {
        const STATE: super::ResumeContextState = super::ResumeContextState::Kept;
    }
}

/// ResumeContext — `intake.routing.ResumeContext` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Kept`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ResumeContextSnapshot`]
/// and [`ResumeContextSnapshot::refine`].
pub struct ResumeContext<S: resume_context_state::Marker> {
    data: ResumeContextData,
    state: core::marker::PhantomData<S>,
}

impl<S: resume_context_state::Marker> ResumeContext<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ResumeContextState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ResumeContextData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ResumeContextData {
        self.data
    }
}

impl ResumeContext<resume_context_state::Kept> {
    /// A new instance, resting in `Kept` — the only state the lifecycle starts one in.
    pub fn new(data: ResumeContextData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.ResumeContext` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ResumeContextSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ResumeContextState,
    /// What it holds.
    pub data: ResumeContextData,
}

/// An `ResumeContext` in whichever declared state it was found.
pub enum AnyResumeContext {
    /// Resting in `Kept`.
    Kept(ResumeContext<resume_context_state::Kept>),
}

impl ResumeContextSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ResumeContextState` cannot spell one.
    pub fn refine(self) -> AnyResumeContext {
        match self.state {
            ResumeContextState::Kept => AnyResumeContext::Kept(ResumeContext {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyResumeContext {
    /// The state, as the runtime value.
    pub fn state(&self) -> ResumeContextState {
        match self {
            Self::Kept(_) => ResumeContextState::Kept,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ResumeContextSnapshot {
        match self {
            Self::Kept(instance) => ResumeContextSnapshot {
                state: ResumeContextState::Kept,
                data: instance.into_data(),
            },
        }
    }
}

/// What SliceEvidence — `intake.routing.SliceEvidence` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`SliceEvidence<S>`], and at a boundary by [`SliceEvidenceSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceEvidenceData {
    /// The identity: `evidence_id` — `Uuid`.
    pub evidence_id: crate::primitives::Uuid,
    /// `kind` — `intake.routing.SliceEvidenceKind`.
    pub kind: SliceEvidenceKind,
    /// `subject_revision` — `String`.
    pub subject_revision: String,
    /// `result` — `Optional<intake.routing.SliceEvidenceResult>`.
    pub result: Option<SliceEvidenceResult>,
    /// `observation_id` — `Uuid`.
    pub observation_id: crate::primitives::Uuid,
    /// `observed_at` — `Timestamp`.
    pub observed_at: crate::primitives::Timestamp,
    /// `producer` — `String`.
    pub producer: String,
}

/// The states of `intake.routing.SliceEvidence`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](slice_evidence_state::Marker), so [`SliceEvidence<S>`](SliceEvidence) can only ever rest in a real state.
pub mod slice_evidence_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Submitted {}
    }

    /// A declared state of `SliceEvidence`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::SliceEvidenceState;
    }

    /// `Submitted`. Where a new instance starts.
    pub struct Submitted;

    impl Marker for Submitted {
        const STATE: super::SliceEvidenceState = super::SliceEvidenceState::Submitted;
    }
}

/// SliceEvidence — `intake.routing.SliceEvidence` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Submitted`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`SliceEvidenceSnapshot`]
/// and [`SliceEvidenceSnapshot::refine`].
pub struct SliceEvidence<S: slice_evidence_state::Marker> {
    data: SliceEvidenceData,
    state: core::marker::PhantomData<S>,
}

impl<S: slice_evidence_state::Marker> SliceEvidence<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> SliceEvidenceState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &SliceEvidenceData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> SliceEvidenceData {
        self.data
    }
}

impl SliceEvidence<slice_evidence_state::Submitted> {
    /// A new instance, resting in `Submitted` — the only state the lifecycle starts one in.
    pub fn new(data: SliceEvidenceData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `intake.routing.SliceEvidence` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`SliceEvidenceSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceEvidenceSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: SliceEvidenceState,
    /// What it holds.
    pub data: SliceEvidenceData,
}

/// An `SliceEvidence` in whichever declared state it was found.
pub enum AnySliceEvidence {
    /// Resting in `Submitted`.
    Submitted(SliceEvidence<slice_evidence_state::Submitted>),
}

impl SliceEvidenceSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `SliceEvidenceState` cannot spell one.
    pub fn refine(self) -> AnySliceEvidence {
        match self.state {
            SliceEvidenceState::Submitted => AnySliceEvidence::Submitted(SliceEvidence {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnySliceEvidence {
    /// The state, as the runtime value.
    pub fn state(&self) -> SliceEvidenceState {
        match self {
            Self::Submitted(_) => SliceEvidenceState::Submitted,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> SliceEvidenceSnapshot {
        match self {
            Self::Submitted(instance) => SliceEvidenceSnapshot {
                state: SliceEvidenceState::Submitted,
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
    /// `approvals` — `List<String>`.
    pub approvals: Vec<String>,
    /// `risk` — `intake.routing.Risk`.
    pub risk: Risk,
    /// `change_kind` — `intake.routing.ChangeKind`.
    pub change_kind: ChangeKind,
    /// `affects_security_boundary` — `Boolean`.
    pub affects_security_boundary: bool,
    /// `test_paths` — `List<String>`.
    pub test_paths: Vec<String>,
    /// `regression_test_command` — `Optional<String>`.
    pub regression_test_command: Option<String>,
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

/// DropResumeContext — the input of `intake.routing.DropResumeContext`.
///
/// Everything it can result in is [`DropResumeContextOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropResumeContext {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
}

/// Everything `intake.routing.DropResumeContext` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropResumeContextOutcome {
    /// `dropped` — otherwise.
    Dropped,
    /// `not-found` — for an identity no record carries.
    NotFound {
        /// Why it was refused: `intake.routing.ResumeContextNotFound`.
        error: ResumeContextNotFound,
    },
    /// `storage-failed` — externally decided (the resume context store cannot remove the context).
    StorageFailed {
        /// Why it was refused: `intake.routing.ResumeContextStorageFailed`.
        error: ResumeContextStorageFailed,
    },
}

/// KeepResumeContext — the input of `intake.routing.KeepResumeContext`.
///
/// Everything it can result in is [`KeepResumeContextOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeepResumeContext {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_id` — `intake.routing.CaseId`.
    pub case_id: CaseId,
    /// `agent_revision_id` — `intake.routing.AgentRevisionId`.
    pub agent_revision_id: AgentRevisionId,
    /// `intent` — `String`.
    pub intent: String,
    /// `route` — `intake.routing.AcceptedRoute`.
    pub route: AcceptedRoute,
    /// `workspace` — `Optional<String>`.
    pub workspace: Option<String>,
    /// `test_command` — `List<String>`.
    pub test_command: Vec<String>,
    /// `test_paths` — `List<String>`.
    pub test_paths: Vec<String>,
    /// `regression_test_command` — `Optional<String>`.
    pub regression_test_command: Option<String>,
    /// `max_steps` — `Integer`.
    pub max_steps: i64,
    /// `confinement` — `intake.confinement.Backend`.
    pub confinement: crate::confinement::Backend,
    /// `cgroup_root` — `Optional<String>`.
    pub cgroup_root: Option<String>,
    /// `model` — `String`.
    pub model: String,
    /// `catalog` — `Optional<String>`.
    pub catalog: Option<String>,
    /// `config` — `Optional<String>`.
    pub config: Option<String>,
}

/// Everything `intake.routing.KeepResumeContext` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeepResumeContextOutcome {
    /// `config-and-catalog` — when `(defined(config) and defined(catalog))`.
    ConfigAndCatalog {
        /// Why it was refused: `intake.routing.ResumeContextConfigAndCatalog`.
        error: ResumeContextConfigAndCatalog,
    },
    /// `already-kept` — for an identity a record already carries.
    AlreadyKept {
        /// Why it was refused: `intake.routing.ResumeContextAlreadyKept`.
        error: ResumeContextAlreadyKept,
    },
    /// `kept` — otherwise.
    Kept {
        /// The `intake.routing.ResumeContextKept` this outcome publishes.
        resume_context_kept: ResumeContextKept,
    },
    /// `storage-failed` — externally decided (the resume context store cannot keep the context).
    StorageFailed {
        /// Why it was refused: `intake.routing.ResumeContextStorageFailed`.
        error: ResumeContextStorageFailed,
    },
}

/// RecordMergeHandoff — the input of `intake.routing.RecordMergeHandoff`.
///
/// Everything it can result in is [`RecordMergeHandoffOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordMergeHandoff {
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `head` — `String`.
    pub head: String,
}

/// Everything `intake.routing.RecordMergeHandoff` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordMergeHandoffOutcome {
    /// `already-recorded` — for an identity a record already carries.
    AlreadyRecorded {
        /// Why it was refused: `intake.routing.MergeHandoffAlreadyRecorded`.
        error: MergeHandoffAlreadyRecorded,
    },
    /// `head-already-handed-off` — when, of the `intake.routing.MergeHandoff` rows satisfying `(commission_id == input.commission_id and head == input.head)`, at least one row is selected.
    HeadAlreadyHandedOff {
        /// Why it was refused: `intake.routing.HeadAlreadyHandedOff`.
        error: HeadAlreadyHandedOff,
    },
    /// `recorded` — otherwise.
    Recorded {
        /// The `intake.routing.MergeHandedOff` this outcome publishes.
        merge_handed_off: MergeHandedOff,
    },
    /// `storage-failed` — externally decided (the state directory cannot record the hand-off).
    StorageFailed {
        /// Why it was refused: `intake.routing.MergeHandoffStorageFailed`.
        error: MergeHandoffStorageFailed,
    },
}

/// MergeHandedOff — the event `intake.routing.MergeHandedOff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandedOff {
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `head` — `String`.
    pub head: String,
}

/// ResumeContextKept — the event `intake.routing.ResumeContextKept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextKept {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_id` — `intake.routing.CaseId`.
    pub case_id: CaseId,
}

/// The declared error `intake.routing.HeadAlreadyHandedOff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadAlreadyHandedOff {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `head` — `String`.
    pub head: String,
}

/// The declared error `intake.routing.MergeHandoffAlreadyRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandoffAlreadyRecorded {
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
}

/// The declared error `intake.routing.MergeHandoffStorageFailed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandoffStorageFailed {
    /// `reason` — `String`.
    pub reason: String,
}

/// The declared error `intake.routing.ResumeContextAlreadyKept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextAlreadyKept {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
}

/// The declared error `intake.routing.ResumeContextConfigAndCatalog`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextConfigAndCatalog {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
}

/// The declared error `intake.routing.ResumeContextNotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextNotFound {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
}

/// The declared error `intake.routing.ResumeContextStorageFailed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContextStorageFailed {
    /// `reason` — `String`.
    pub reason: String,
}

/// MergeHandoffs — one row of the view `intake.routing.MergeHandoffs`.
///
/// Projects `intake.routing.MergeHandoff` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeHandoffs {
    /// `action_request_id` — `intake.routing.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `head` — `String`.
    pub head: String,
    /// `state` — `intake.routing.MergeHandoff.State`.
    pub state: MergeHandoffState,
}

/// ResumeContexts — one row of the view `intake.routing.ResumeContexts`.
///
/// Projects `intake.routing.ResumeContext` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeContexts {
    /// `commission_id` — `intake.routing.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_id` — `intake.routing.CaseId`.
    pub case_id: CaseId,
    /// `agent_revision_id` — `intake.routing.AgentRevisionId`.
    pub agent_revision_id: AgentRevisionId,
    /// `intent` — `String`.
    pub intent: String,
    /// `workspace` — `Optional<String>`.
    pub workspace: Option<String>,
    /// `test_command` — `List<String>`.
    pub test_command: Vec<String>,
    /// `test_paths` — `List<String>`.
    pub test_paths: Vec<String>,
    /// `regression_test_command` — `Optional<String>`.
    pub regression_test_command: Option<String>,
    /// `max_steps` — `Integer`.
    pub max_steps: i64,
    /// `confinement` — `intake.confinement.Backend`.
    pub confinement: crate::confinement::Backend,
    /// `cgroup_root` — `Optional<String>`.
    pub cgroup_root: Option<String>,
    /// `model` — `String`.
    pub model: String,
    /// `catalog` — `Optional<String>`.
    pub catalog: Option<String>,
    /// `config` — `Optional<String>`.
    pub config: Option<String>,
    /// `state` — `intake.routing.ResumeContext.State`.
    pub state: ResumeContextState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `intake.routing.DropResumeContext` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed`.
    ///
    /// Contract: given `intake.routing.DropResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `dropped` otherwise, deletes `intake.routing.ResumeContext`; `not-found` for an identity no record carries, error `intake.routing.ResumeContextNotFound`; `storage-failed` externally decided (the resume context store cannot remove the context), error `intake.routing.ResumeContextStorageFailed`.
    pub trait DropResumeContextBehavior {
        /// Decides and enacts exactly one declared outcome of `intake.routing.DropResumeContext`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn drop_resume_context(&mut self, input: super::DropResumeContext) -> Result<super::DropResumeContextOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `intake.routing.KeepResumeContext` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed`.
    ///
    /// Contract: given `intake.routing.KeepResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `config-and-catalog` when `(defined(config) and defined(catalog))`, error `intake.routing.ResumeContextConfigAndCatalog`; `already-kept` for an identity a record already carries, error `intake.routing.ResumeContextAlreadyKept`; `kept` otherwise, creates `intake.routing.ResumeContext`, emits `intake.routing.ResumeContextKept`; `storage-failed` externally decided (the resume context store cannot keep the context), error `intake.routing.ResumeContextStorageFailed`.
    pub trait KeepResumeContextBehavior {
        /// Decides and enacts exactly one declared outcome of `intake.routing.KeepResumeContext`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn keep_resume_context(&mut self, input: super::KeepResumeContext) -> Result<super::KeepResumeContextOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `intake.routing.RecordMergeHandoff` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by a guard over the rows a selector selects (`when_related: {entity, where, exists | count | forall}`), in `head-already-handed-off`.
    ///
    /// Contract: given `intake.routing.RecordMergeHandoff` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `already-recorded` for an identity a record already carries, error `intake.routing.MergeHandoffAlreadyRecorded`; `head-already-handed-off` when, of the `intake.routing.MergeHandoff` rows satisfying `(commission_id == input.commission_id and head == input.head)`, at least one row is selected, error `intake.routing.HeadAlreadyHandedOff`; `recorded` otherwise, creates `intake.routing.MergeHandoff`, emits `intake.routing.MergeHandedOff`; `storage-failed` externally decided (the state directory cannot record the hand-off), error `intake.routing.MergeHandoffStorageFailed`.
    pub trait RecordMergeHandoffBehavior {
        /// Decides and enacts exactly one declared outcome of `intake.routing.RecordMergeHandoff`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn record_merge_handoff(&mut self, input: super::RecordMergeHandoff) -> Result<super::RecordMergeHandoffOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `intake.routing.MergeHandoffs` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait MergeHandoffsQuery {
        /// Serves `intake.routing.MergeHandoffs` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn merge_handoffs(&self) -> Result<Vec<super::MergeHandoffs>, crate::obligation::UnmetObligation>;
    }

    /// The query `intake.routing.ResumeContexts` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait ResumeContextsQuery {
        /// Serves `intake.routing.ResumeContexts` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn resume_contexts(&self) -> Result<Vec<super::ResumeContexts>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl DropResumeContextBehavior for Unimplemented {
        fn drop_resume_context(&mut self, _input: super::DropResumeContext) -> Result<super::DropResumeContextOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "intake.routing.DropResumeContext" })
        }
    }

    impl KeepResumeContextBehavior for Unimplemented {
        fn keep_resume_context(&mut self, _input: super::KeepResumeContext) -> Result<super::KeepResumeContextOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "intake.routing.KeepResumeContext" })
        }
    }

    impl RecordMergeHandoffBehavior for Unimplemented {
        fn record_merge_handoff(&mut self, _input: super::RecordMergeHandoff) -> Result<super::RecordMergeHandoffOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "intake.routing.RecordMergeHandoff" })
        }
    }
}
