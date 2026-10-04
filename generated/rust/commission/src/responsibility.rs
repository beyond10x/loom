// generated from commission v1
// model digest 8baad8a2a232f1823d8fce586ddd35c901af8923715a1eb1fbc3f62817da8e4f
// contract digest c27daebab1de8a70c1c4985e2a48176db7d5700784cedf99b11902b83a5dea7a
// do not edit: regenerate with `ess synthesize --layout crate`

//! Responsibility — `commission.responsibility`.
//!
//! An agent revision commissioned to a durable case. The governor owns the case's truth and returns a frontier; a run is one bounded period of execution against it. The executor proposes; it never completes a case.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// The states of `commission.responsibility.ActionRequest`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `ActionRequest<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRequestState {
    /// `Requested`.
    Requested,
}

/// ActionRequestId — `commission.responsibility.ActionRequestId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequestId(pub crate::primitives::Uuid);

/// ActionStatus — `commission.responsibility.ActionStatus`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    /// `Admissible`.
    Admissible,
    /// `ApprovalRequired`.
    ApprovalRequired,
    /// `Blocked`.
    Blocked,
}

/// Admission — `commission.responsibility.Admission`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Tagged `Admissible` — `commission.responsibility.Unit`.
    Admissible(Unit),
    /// Tagged `NeedsAuthority` — `commission.responsibility.AdmissionNeedsAuthority`.
    NeedsAuthority(AdmissionNeedsAuthority),
    /// Tagged `Refused` — `commission.responsibility.AdmissionRefused`.
    Refused(AdmissionRefused),
}

/// AdmissionNeedsAuthority — `commission.responsibility.AdmissionNeedsAuthority`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionNeedsAuthority {
    /// `capability` — `String`.
    pub capability: String,
}

/// AdmissionRefused — `commission.responsibility.AdmissionRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRefused {
    /// `action` — `String`.
    pub action: String,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// The states of `commission.responsibility.Agent`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Agent<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// `Registered`.
    Registered,
}

/// AgentId — `commission.responsibility.AgentId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentId(pub crate::primitives::Uuid);

/// The states of `commission.responsibility.AgentRevision`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `AgentRevision<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRevisionState {
    /// `Published`.
    Published,
}

/// AgentRevisionId — `commission.responsibility.AgentRevisionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRevisionId(pub crate::primitives::Uuid);

/// AuthorityContext — `commission.responsibility.AuthorityContext`: a distinct wrapper around `Json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityContext(pub crate::json::Value);

/// The states of `commission.responsibility.AuthorityDecision`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `AuthorityDecision<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityDecisionState {
    /// `Decided`.
    Decided,
}

/// AuthorityDecisionId — `commission.responsibility.AuthorityDecisionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityDecisionId(pub crate::primitives::Uuid);

/// AuthorityVerdict — `commission.responsibility.AuthorityVerdict`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityVerdict {
    /// Tagged `Allow` — `commission.responsibility.Unit`.
    Allow(Unit),
    /// Tagged `ApprovalRequired` — `commission.responsibility.AuthorityVerdictApprovalRequired`.
    ApprovalRequired(AuthorityVerdictApprovalRequired),
    /// Tagged `Deny` — `commission.responsibility.AuthorityVerdictDeny`.
    Deny(AuthorityVerdictDeny),
}

/// AuthorityVerdictApprovalRequired — `commission.responsibility.AuthorityVerdictApprovalRequired`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityVerdictApprovalRequired {
    /// `request` — `String`.
    pub request: String,
}

/// AuthorityVerdictDeny — `commission.responsibility.AuthorityVerdictDeny`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityVerdictDeny {
    /// `reason` — `String`.
    pub reason: String,
}

/// The states of `commission.responsibility.Case`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Case<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseState {
    /// `Open`.
    Open,
}

/// CaseId — `commission.responsibility.CaseId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseId(pub String);

/// The states of `commission.responsibility.Commission`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Commission<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommissionState {
    /// `Assigned`.
    Assigned,
}

/// CommissionId — `commission.responsibility.CommissionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionId(pub crate::primitives::Uuid);

/// CompletionDetermination — `commission.responsibility.CompletionDetermination`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionDetermination {
    /// Tagged `Complete` — `commission.responsibility.CompletionDeterminationComplete`.
    Complete(CompletionDeterminationComplete),
    /// Tagged `Open` — `commission.responsibility.Unit`.
    Open(Unit),
}

/// CompletionDeterminationComplete — `commission.responsibility.CompletionDeterminationComplete`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionDeterminationComplete {
    /// `outcome` — `String`.
    pub outcome: String,
}

/// The states of `commission.responsibility.Evidence`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Evidence<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceState {
    /// `Submitted`.
    Submitted,
}

/// EvidenceId — `commission.responsibility.EvidenceId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceId(pub crate::primitives::Uuid);

/// ExecutorOutcome — `commission.responsibility.ExecutorOutcome`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutorOutcome {
    /// Tagged `CompletedLocalReasoning` — `commission.responsibility.Unit`.
    CompletedLocalReasoning(Unit),
    /// Tagged `NeedsHumanJudgment` — `commission.responsibility.ExecutorOutcomeNeedsHumanJudgment`.
    NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment),
    /// Tagged `NoUsefulAction` — `commission.responsibility.Unit`.
    NoUsefulAction(Unit),
    /// Tagged `ProposedAction` — `commission.responsibility.ExecutorOutcomeProposedAction`.
    ProposedAction(ExecutorOutcomeProposedAction),
    /// Tagged `Suspended` — `commission.responsibility.ExecutorOutcomeSuspended`.
    Suspended(ExecutorOutcomeSuspended),
}

/// ExecutorOutcomeNeedsHumanJudgment — `commission.responsibility.ExecutorOutcomeNeedsHumanJudgment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorOutcomeNeedsHumanJudgment {
    /// `request` — `commission.responsibility.HumanDecisionRequest`.
    pub request: HumanDecisionRequest,
}

/// ExecutorOutcomeProposedAction — `commission.responsibility.ExecutorOutcomeProposedAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorOutcomeProposedAction {
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `commission.responsibility.ProposedActionArguments`.
    pub arguments: ProposedActionArguments,
}

/// ExecutorOutcomeSuspended — `commission.responsibility.ExecutorOutcomeSuspended`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorOutcomeSuspended {
    /// `reason` — `commission.responsibility.SuspensionReason`.
    pub reason: SuspensionReason,
}

/// The states of `commission.responsibility.Frontier`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Frontier<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontierState {
    /// `Issued`.
    Issued,
}

/// FrontierAction — `commission.responsibility.FrontierAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierAction {
    /// `action` — `String`.
    pub action: String,
    /// `status` — `commission.responsibility.ActionStatus`.
    pub status: ActionStatus,
    /// `capability` — `Optional<String>`.
    pub capability: Option<String>,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// FrontierClaim — `commission.responsibility.FrontierClaim`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierClaim {
    /// `claim` — `String`.
    pub claim: String,
    /// `value` — `commission.responsibility.Truth`.
    pub value: Truth,
}

/// FrontierId — `commission.responsibility.FrontierId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierId(pub crate::primitives::Uuid);

/// FrontierObligation — `commission.responsibility.FrontierObligation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierObligation {
    /// `obligation` — `String`.
    pub obligation: String,
    /// `open` — `Boolean`.
    pub open: bool,
}

/// GovernorError — `commission.responsibility.GovernorError`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernorError {
    /// `UnknownCase`.
    UnknownCase,
    /// `GovernorUnavailable`.
    GovernorUnavailable,
}

/// HumanDecisionRequest — `commission.responsibility.HumanDecisionRequest`: a distinct wrapper around `Json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanDecisionRequest(pub crate::json::Value);

/// The states of `commission.responsibility.Observation`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Observation<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationState {
    /// `Reported`.
    Reported,
}

/// ObservationId — `commission.responsibility.ObservationId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationId(pub crate::primitives::Uuid);

/// PrincipalId — `commission.responsibility.PrincipalId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalId(pub String);

/// ProposedActionArguments — `commission.responsibility.ProposedActionArguments`: a distinct wrapper around `Json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedActionArguments(pub crate::json::Value);

/// The states of `commission.responsibility.Run`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `Run<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// `Running`.
    Running,
    /// `Suspended`.
    Suspended,
}

/// RunId — `commission.responsibility.RunId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunId(pub crate::primitives::Uuid);

/// RunOutcome — `commission.responsibility.RunOutcome`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// Tagged `Completed` — `commission.responsibility.RunOutcomeCompleted`.
    Completed(RunOutcomeCompleted),
    /// Tagged `NeedsAuthority` — `commission.responsibility.RunOutcomeNeedsAuthority`.
    NeedsAuthority(RunOutcomeNeedsAuthority),
    /// Tagged `NeedsExternalEvidence` — `commission.responsibility.RunOutcomeNeedsExternalEvidence`.
    NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence),
    /// Tagged `NeedsHumanJudgment` — `commission.responsibility.RunOutcomeNeedsHumanJudgment`.
    NeedsHumanJudgment(RunOutcomeNeedsHumanJudgment),
    /// Tagged `NoAdmissibleAction` — `commission.responsibility.Unit`.
    NoAdmissibleAction(Unit),
    /// Tagged `Suspended` — `commission.responsibility.RunOutcomeSuspended`.
    Suspended(RunOutcomeSuspended),
}

/// RunOutcomeCompleted — `commission.responsibility.RunOutcomeCompleted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcomeCompleted {
    /// `outcome` — `String`.
    pub outcome: String,
}

/// RunOutcomeNeedsAuthority — `commission.responsibility.RunOutcomeNeedsAuthority`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcomeNeedsAuthority {
    /// `request` — `String`.
    pub request: String,
}

/// RunOutcomeNeedsExternalEvidence — `commission.responsibility.RunOutcomeNeedsExternalEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcomeNeedsExternalEvidence {
    /// `requirements` — `List<String>`.
    pub requirements: Vec<String>,
}

/// RunOutcomeNeedsHumanJudgment — `commission.responsibility.RunOutcomeNeedsHumanJudgment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcomeNeedsHumanJudgment {
    /// `request` — `commission.responsibility.HumanDecisionRequest`.
    pub request: HumanDecisionRequest,
}

/// RunOutcomeSuspended — `commission.responsibility.RunOutcomeSuspended`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcomeSuspended {
    /// `reason` — `commission.responsibility.SuspensionReason`.
    pub reason: SuspensionReason,
}

/// SuspensionReason — `commission.responsibility.SuspensionReason`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspensionReason {
    /// Tagged `Authority` — `Json`.
    Authority(crate::json::Value),
    /// Tagged `Budget` — `Json`.
    Budget(crate::json::Value),
    /// Tagged `Dependency` — `List<commission.responsibility.CaseId>`.
    Dependency(Vec<CaseId>),
    /// Tagged `Evidence` — `List<String>`.
    Evidence(Vec<String>),
    /// Tagged `ExternalAvailability` — `Json`.
    ExternalAvailability(crate::json::Value),
    /// Tagged `Human` — `commission.responsibility.HumanDecisionRequest`.
    Human(HumanDecisionRequest),
    /// Tagged `Time` — `Json`.
    Time(crate::json::Value),
}

/// Truth — `commission.responsibility.Truth`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    /// `True`.
    True,
    /// `False`.
    False,
    /// `Unknown`.
    Unknown,
}

/// Unit — `commission.responsibility.Unit`: a distinct wrapper around `Boolean`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit(pub bool);

/// What ActionRequest — `commission.responsibility.ActionRequest` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`ActionRequest<S>`], and at a boundary by [`ActionRequestSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequestData {
    /// The identity: `action_request_id` — `commission.responsibility.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `run_id` — `commission.responsibility.RunId`.
    ///
    /// Carries `requests`: `commission.responsibility.Run` owns many `commission.responsibility.ActionRequest`.
    pub run_id: RunId,
    /// `case_id` — `commission.responsibility.CaseId`.
    ///
    /// Carries `case`: `commission.responsibility.ActionRequest` references one `commission.responsibility.Case`.
    pub case_id: CaseId,
    /// `expected_case_revision` — `Integer`.
    pub expected_case_revision: i64,
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `commission.responsibility.ProposedActionArguments`.
    pub arguments: ProposedActionArguments,
}

/// The states of `commission.responsibility.ActionRequest`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](action_request_state::Marker), so [`ActionRequest<S>`](ActionRequest) can only ever rest in a real state.
pub mod action_request_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Requested {}
    }

    /// A declared state of `ActionRequest`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ActionRequestState;
    }

    /// `Requested`. Where a new instance starts.
    pub struct Requested;

    impl Marker for Requested {
        const STATE: super::ActionRequestState = super::ActionRequestState::Requested;
    }
}

/// ActionRequest — `commission.responsibility.ActionRequest` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Requested`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ActionRequestSnapshot`]
/// and [`ActionRequestSnapshot::refine`].
pub struct ActionRequest<S: action_request_state::Marker> {
    data: ActionRequestData,
    state: core::marker::PhantomData<S>,
}

impl<S: action_request_state::Marker> ActionRequest<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ActionRequestState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ActionRequestData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ActionRequestData {
        self.data
    }
}

impl ActionRequest<action_request_state::Requested> {
    /// A new instance, resting in `Requested` — the only state the lifecycle starts one in.
    pub fn new(data: ActionRequestData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.ActionRequest` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ActionRequestSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequestSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ActionRequestState,
    /// What it holds.
    pub data: ActionRequestData,
}

/// An `ActionRequest` in whichever declared state it was found.
pub enum AnyActionRequest {
    /// Resting in `Requested`.
    Requested(ActionRequest<action_request_state::Requested>),
}

impl ActionRequestSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ActionRequestState` cannot spell one.
    pub fn refine(self) -> AnyActionRequest {
        match self.state {
            ActionRequestState::Requested => AnyActionRequest::Requested(ActionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyActionRequest {
    /// The state, as the runtime value.
    pub fn state(&self) -> ActionRequestState {
        match self {
            Self::Requested(_) => ActionRequestState::Requested,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ActionRequestSnapshot {
        match self {
            Self::Requested(instance) => ActionRequestSnapshot {
                state: ActionRequestState::Requested,
                data: instance.into_data(),
            },
        }
    }
}

/// What Agent — `commission.responsibility.Agent` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Agent<S>`], and at a boundary by [`AgentSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentData {
    /// The identity: `agent_id` — `commission.responsibility.AgentId`.
    pub agent_id: AgentId,
    /// `name` — `String`.
    pub name: String,
}

/// The states of `commission.responsibility.Agent`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](agent_state::Marker), so [`Agent<S>`](Agent) can only ever rest in a real state.
pub mod agent_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Registered {}
    }

    /// A declared state of `Agent`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::AgentState;
    }

    /// `Registered`. Where a new instance starts.
    pub struct Registered;

    impl Marker for Registered {
        const STATE: super::AgentState = super::AgentState::Registered;
    }
}

/// Agent — `commission.responsibility.Agent` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Registered`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`AgentSnapshot`]
/// and [`AgentSnapshot::refine`].
pub struct Agent<S: agent_state::Marker> {
    data: AgentData,
    state: core::marker::PhantomData<S>,
}

impl<S: agent_state::Marker> Agent<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> AgentState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &AgentData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> AgentData {
        self.data
    }
}

impl Agent<agent_state::Registered> {
    /// A new instance, resting in `Registered` — the only state the lifecycle starts one in.
    pub fn new(data: AgentData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Agent` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`AgentSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: AgentState,
    /// What it holds.
    pub data: AgentData,
}

/// An `Agent` in whichever declared state it was found.
pub enum AnyAgent {
    /// Resting in `Registered`.
    Registered(Agent<agent_state::Registered>),
}

impl AgentSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `AgentState` cannot spell one.
    pub fn refine(self) -> AnyAgent {
        match self.state {
            AgentState::Registered => AnyAgent::Registered(Agent {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyAgent {
    /// The state, as the runtime value.
    pub fn state(&self) -> AgentState {
        match self {
            Self::Registered(_) => AgentState::Registered,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> AgentSnapshot {
        match self {
            Self::Registered(instance) => AgentSnapshot {
                state: AgentState::Registered,
                data: instance.into_data(),
            },
        }
    }
}

/// What AgentRevision — `commission.responsibility.AgentRevision` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`AgentRevision<S>`], and at a boundary by [`AgentRevisionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRevisionData {
    /// The identity: `agent_revision_id` — `commission.responsibility.AgentRevisionId`.
    pub agent_revision_id: AgentRevisionId,
    /// `agent_id` — `commission.responsibility.AgentId`.
    ///
    /// Carries `revisions`: `commission.responsibility.Agent` owns many `commission.responsibility.AgentRevision`.
    pub agent_id: AgentId,
    /// `revision` — `Integer`.
    pub revision: i64,
}

/// The states of `commission.responsibility.AgentRevision`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](agent_revision_state::Marker), so [`AgentRevision<S>`](AgentRevision) can only ever rest in a real state.
pub mod agent_revision_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Published {}
    }

    /// A declared state of `AgentRevision`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::AgentRevisionState;
    }

    /// `Published`. Where a new instance starts.
    pub struct Published;

    impl Marker for Published {
        const STATE: super::AgentRevisionState = super::AgentRevisionState::Published;
    }
}

/// AgentRevision — `commission.responsibility.AgentRevision` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Published`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`AgentRevisionSnapshot`]
/// and [`AgentRevisionSnapshot::refine`].
pub struct AgentRevision<S: agent_revision_state::Marker> {
    data: AgentRevisionData,
    state: core::marker::PhantomData<S>,
}

impl<S: agent_revision_state::Marker> AgentRevision<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> AgentRevisionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &AgentRevisionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> AgentRevisionData {
        self.data
    }
}

impl AgentRevision<agent_revision_state::Published> {
    /// A new instance, resting in `Published` — the only state the lifecycle starts one in.
    pub fn new(data: AgentRevisionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.AgentRevision` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`AgentRevisionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRevisionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: AgentRevisionState,
    /// What it holds.
    pub data: AgentRevisionData,
}

/// An `AgentRevision` in whichever declared state it was found.
pub enum AnyAgentRevision {
    /// Resting in `Published`.
    Published(AgentRevision<agent_revision_state::Published>),
}

impl AgentRevisionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `AgentRevisionState` cannot spell one.
    pub fn refine(self) -> AnyAgentRevision {
        match self.state {
            AgentRevisionState::Published => AnyAgentRevision::Published(AgentRevision {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyAgentRevision {
    /// The state, as the runtime value.
    pub fn state(&self) -> AgentRevisionState {
        match self {
            Self::Published(_) => AgentRevisionState::Published,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> AgentRevisionSnapshot {
        match self {
            Self::Published(instance) => AgentRevisionSnapshot {
                state: AgentRevisionState::Published,
                data: instance.into_data(),
            },
        }
    }
}

/// What AuthorityDecision — `commission.responsibility.AuthorityDecision` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`AuthorityDecision<S>`], and at a boundary by [`AuthorityDecisionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityDecisionData {
    /// The identity: `authority_decision_id` — `commission.responsibility.AuthorityDecisionId`.
    pub authority_decision_id: AuthorityDecisionId,
    /// `action` — `String`.
    pub action: String,
    /// `granted` — `Boolean`.
    pub granted: bool,
    /// `action_request_id` — `commission.responsibility.ActionRequestId`.
    ///
    /// Carries `action_request`: `commission.responsibility.AuthorityDecision` references one `commission.responsibility.ActionRequest`.
    pub action_request_id: ActionRequestId,
}

/// The states of `commission.responsibility.AuthorityDecision`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](authority_decision_state::Marker), so [`AuthorityDecision<S>`](AuthorityDecision) can only ever rest in a real state.
pub mod authority_decision_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Decided {}
    }

    /// A declared state of `AuthorityDecision`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::AuthorityDecisionState;
    }

    /// `Decided`. Where a new instance starts.
    pub struct Decided;

    impl Marker for Decided {
        const STATE: super::AuthorityDecisionState = super::AuthorityDecisionState::Decided;
    }
}

/// AuthorityDecision — `commission.responsibility.AuthorityDecision` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Decided`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`AuthorityDecisionSnapshot`]
/// and [`AuthorityDecisionSnapshot::refine`].
pub struct AuthorityDecision<S: authority_decision_state::Marker> {
    data: AuthorityDecisionData,
    state: core::marker::PhantomData<S>,
}

impl<S: authority_decision_state::Marker> AuthorityDecision<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> AuthorityDecisionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &AuthorityDecisionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> AuthorityDecisionData {
        self.data
    }
}

impl AuthorityDecision<authority_decision_state::Decided> {
    /// A new instance, resting in `Decided` — the only state the lifecycle starts one in.
    pub fn new(data: AuthorityDecisionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.AuthorityDecision` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`AuthorityDecisionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityDecisionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: AuthorityDecisionState,
    /// What it holds.
    pub data: AuthorityDecisionData,
}

/// An `AuthorityDecision` in whichever declared state it was found.
pub enum AnyAuthorityDecision {
    /// Resting in `Decided`.
    Decided(AuthorityDecision<authority_decision_state::Decided>),
}

impl AuthorityDecisionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `AuthorityDecisionState` cannot spell one.
    pub fn refine(self) -> AnyAuthorityDecision {
        match self.state {
            AuthorityDecisionState::Decided => AnyAuthorityDecision::Decided(AuthorityDecision {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyAuthorityDecision {
    /// The state, as the runtime value.
    pub fn state(&self) -> AuthorityDecisionState {
        match self {
            Self::Decided(_) => AuthorityDecisionState::Decided,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> AuthorityDecisionSnapshot {
        match self {
            Self::Decided(instance) => AuthorityDecisionSnapshot {
                state: AuthorityDecisionState::Decided,
                data: instance.into_data(),
            },
        }
    }
}

/// What Case — `commission.responsibility.Case` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Case<S>`], and at a boundary by [`CaseSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseData {
    /// The identity: `case_id` — `commission.responsibility.CaseId`.
    pub case_id: CaseId,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `revision` — `Integer`.
    pub revision: i64,
}

/// The states of `commission.responsibility.Case`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](case_state::Marker), so [`Case<S>`](Case) can only ever rest in a real state.
pub mod case_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Open {}
    }

    /// A declared state of `Case`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::CaseState;
    }

    /// `Open`. Where a new instance starts.
    pub struct Open;

    impl Marker for Open {
        const STATE: super::CaseState = super::CaseState::Open;
    }
}

/// Case — `commission.responsibility.Case` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Open`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`CaseSnapshot`]
/// and [`CaseSnapshot::refine`].
pub struct Case<S: case_state::Marker> {
    data: CaseData,
    state: core::marker::PhantomData<S>,
}

impl<S: case_state::Marker> Case<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> CaseState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &CaseData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> CaseData {
        self.data
    }
}

impl Case<case_state::Open> {
    /// A new instance, resting in `Open` — the only state the lifecycle starts one in.
    pub fn new(data: CaseData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Case` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`CaseSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: CaseState,
    /// What it holds.
    pub data: CaseData,
}

/// An `Case` in whichever declared state it was found.
pub enum AnyCase {
    /// Resting in `Open`.
    Open(Case<case_state::Open>),
}

impl CaseSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `CaseState` cannot spell one.
    pub fn refine(self) -> AnyCase {
        match self.state {
            CaseState::Open => AnyCase::Open(Case {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyCase {
    /// The state, as the runtime value.
    pub fn state(&self) -> CaseState {
        match self {
            Self::Open(_) => CaseState::Open,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> CaseSnapshot {
        match self {
            Self::Open(instance) => CaseSnapshot {
                state: CaseState::Open,
                data: instance.into_data(),
            },
        }
    }
}

/// What Commission — `commission.responsibility.Commission` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Commission<S>`], and at a boundary by [`CommissionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionData {
    /// The identity: `commission_id` — `commission.responsibility.CommissionId`.
    pub commission_id: CommissionId,
    /// `agent_revision_id` — `commission.responsibility.AgentRevisionId`.
    ///
    /// Carries `agent_revision`: `commission.responsibility.Commission` references one `commission.responsibility.AgentRevision`.
    pub agent_revision_id: AgentRevisionId,
    /// `case_id` — `commission.responsibility.CaseId`.
    ///
    /// Carries `case`: `commission.responsibility.Commission` references one `commission.responsibility.Case`.
    pub case_id: CaseId,
    /// `principal` — `commission.responsibility.PrincipalId`.
    pub principal: PrincipalId,
    /// `authority_context` — `commission.responsibility.AuthorityContext`.
    pub authority_context: AuthorityContext,
}

/// The states of `commission.responsibility.Commission`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](commission_state::Marker), so [`Commission<S>`](Commission) can only ever rest in a real state.
pub mod commission_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Assigned {}
    }

    /// A declared state of `Commission`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::CommissionState;
    }

    /// `Assigned`. Where a new instance starts.
    pub struct Assigned;

    impl Marker for Assigned {
        const STATE: super::CommissionState = super::CommissionState::Assigned;
    }
}

/// Commission — `commission.responsibility.Commission` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Assigned`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`CommissionSnapshot`]
/// and [`CommissionSnapshot::refine`].
pub struct Commission<S: commission_state::Marker> {
    data: CommissionData,
    state: core::marker::PhantomData<S>,
}

impl<S: commission_state::Marker> Commission<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> CommissionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &CommissionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> CommissionData {
        self.data
    }
}

impl Commission<commission_state::Assigned> {
    /// A new instance, resting in `Assigned` — the only state the lifecycle starts one in.
    pub fn new(data: CommissionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Commission` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`CommissionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: CommissionState,
    /// What it holds.
    pub data: CommissionData,
}

/// An `Commission` in whichever declared state it was found.
pub enum AnyCommission {
    /// Resting in `Assigned`.
    Assigned(Commission<commission_state::Assigned>),
}

impl CommissionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `CommissionState` cannot spell one.
    pub fn refine(self) -> AnyCommission {
        match self.state {
            CommissionState::Assigned => AnyCommission::Assigned(Commission {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyCommission {
    /// The state, as the runtime value.
    pub fn state(&self) -> CommissionState {
        match self {
            Self::Assigned(_) => CommissionState::Assigned,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> CommissionSnapshot {
        match self {
            Self::Assigned(instance) => CommissionSnapshot {
                state: CommissionState::Assigned,
                data: instance.into_data(),
            },
        }
    }
}

/// What Evidence — `commission.responsibility.Evidence` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Evidence<S>`], and at a boundary by [`EvidenceSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceData {
    /// The identity: `evidence_id` — `commission.responsibility.EvidenceId`.
    pub evidence_id: EvidenceId,
    /// `case_id` — `commission.responsibility.CaseId`.
    ///
    /// Carries `case`: `commission.responsibility.Evidence` references one `commission.responsibility.Case`.
    pub case_id: CaseId,
    /// `kind` — `String`.
    pub kind: String,
    /// `subject_revision` — `Integer`.
    pub subject_revision: i64,
    /// `producer` — `String`.
    pub producer: String,
    /// `observation_ids` — `List<commission.responsibility.ObservationId>`.
    ///
    /// Carries `observations`: `commission.responsibility.Evidence` references many `commission.responsibility.Observation`.
    pub observation_ids: Vec<ObservationId>,
    /// `facts` — `Json`.
    pub facts: crate::json::Value,
    /// `provenance` — `Json`.
    pub provenance: crate::json::Value,
}

/// The states of `commission.responsibility.Evidence`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](evidence_state::Marker), so [`Evidence<S>`](Evidence) can only ever rest in a real state.
pub mod evidence_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Submitted {}
    }

    /// A declared state of `Evidence`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::EvidenceState;
    }

    /// `Submitted`. Where a new instance starts.
    pub struct Submitted;

    impl Marker for Submitted {
        const STATE: super::EvidenceState = super::EvidenceState::Submitted;
    }
}

/// Evidence — `commission.responsibility.Evidence` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Submitted`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`EvidenceSnapshot`]
/// and [`EvidenceSnapshot::refine`].
pub struct Evidence<S: evidence_state::Marker> {
    data: EvidenceData,
    state: core::marker::PhantomData<S>,
}

impl<S: evidence_state::Marker> Evidence<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> EvidenceState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &EvidenceData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> EvidenceData {
        self.data
    }
}

impl Evidence<evidence_state::Submitted> {
    /// A new instance, resting in `Submitted` — the only state the lifecycle starts one in.
    pub fn new(data: EvidenceData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Evidence` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`EvidenceSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: EvidenceState,
    /// What it holds.
    pub data: EvidenceData,
}

/// An `Evidence` in whichever declared state it was found.
pub enum AnyEvidence {
    /// Resting in `Submitted`.
    Submitted(Evidence<evidence_state::Submitted>),
}

impl EvidenceSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `EvidenceState` cannot spell one.
    pub fn refine(self) -> AnyEvidence {
        match self.state {
            EvidenceState::Submitted => AnyEvidence::Submitted(Evidence {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyEvidence {
    /// The state, as the runtime value.
    pub fn state(&self) -> EvidenceState {
        match self {
            Self::Submitted(_) => EvidenceState::Submitted,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> EvidenceSnapshot {
        match self {
            Self::Submitted(instance) => EvidenceSnapshot {
                state: EvidenceState::Submitted,
                data: instance.into_data(),
            },
        }
    }
}

/// What Frontier — `commission.responsibility.Frontier` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Frontier<S>`], and at a boundary by [`FrontierSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierData {
    /// The identity: `frontier_id` — `commission.responsibility.FrontierId`.
    pub frontier_id: FrontierId,
    /// `case_id` — `commission.responsibility.CaseId`.
    ///
    /// Carries `case`: `commission.responsibility.Frontier` references one `commission.responsibility.Case`.
    pub case_id: CaseId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `claims` — `List<commission.responsibility.FrontierClaim>`.
    pub claims: Vec<FrontierClaim>,
    /// `obligations` — `List<commission.responsibility.FrontierObligation>`.
    pub obligations: Vec<FrontierObligation>,
    /// `actions` — `List<commission.responsibility.FrontierAction>`.
    pub actions: Vec<FrontierAction>,
}

/// The states of `commission.responsibility.Frontier`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](frontier_state::Marker), so [`Frontier<S>`](Frontier) can only ever rest in a real state.
pub mod frontier_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Issued {}
    }

    /// A declared state of `Frontier`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::FrontierState;
    }

    /// `Issued`. Where a new instance starts.
    pub struct Issued;

    impl Marker for Issued {
        const STATE: super::FrontierState = super::FrontierState::Issued;
    }
}

/// Frontier — `commission.responsibility.Frontier` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Issued`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`FrontierSnapshot`]
/// and [`FrontierSnapshot::refine`].
pub struct Frontier<S: frontier_state::Marker> {
    data: FrontierData,
    state: core::marker::PhantomData<S>,
}

impl<S: frontier_state::Marker> Frontier<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> FrontierState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &FrontierData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> FrontierData {
        self.data
    }
}

impl Frontier<frontier_state::Issued> {
    /// A new instance, resting in `Issued` — the only state the lifecycle starts one in.
    pub fn new(data: FrontierData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Frontier` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`FrontierSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: FrontierState,
    /// What it holds.
    pub data: FrontierData,
}

/// An `Frontier` in whichever declared state it was found.
pub enum AnyFrontier {
    /// Resting in `Issued`.
    Issued(Frontier<frontier_state::Issued>),
}

impl FrontierSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `FrontierState` cannot spell one.
    pub fn refine(self) -> AnyFrontier {
        match self.state {
            FrontierState::Issued => AnyFrontier::Issued(Frontier {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyFrontier {
    /// The state, as the runtime value.
    pub fn state(&self) -> FrontierState {
        match self {
            Self::Issued(_) => FrontierState::Issued,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> FrontierSnapshot {
        match self {
            Self::Issued(instance) => FrontierSnapshot {
                state: FrontierState::Issued,
                data: instance.into_data(),
            },
        }
    }
}

/// What Observation — `commission.responsibility.Observation` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Observation<S>`], and at a boundary by [`ObservationSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationData {
    /// The identity: `observation_id` — `commission.responsibility.ObservationId`.
    pub observation_id: ObservationId,
    /// `source` — `String`.
    pub source: String,
    /// `subject` — `String`.
    pub subject: String,
    /// `observed_at` — `Timestamp`.
    pub observed_at: crate::primitives::Timestamp,
    /// `payload` — `Json`.
    pub payload: crate::json::Value,
}

/// The states of `commission.responsibility.Observation`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](observation_state::Marker), so [`Observation<S>`](Observation) can only ever rest in a real state.
pub mod observation_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Reported {}
    }

    /// A declared state of `Observation`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::ObservationState;
    }

    /// `Reported`. Where a new instance starts.
    pub struct Reported;

    impl Marker for Reported {
        const STATE: super::ObservationState = super::ObservationState::Reported;
    }
}

/// Observation — `commission.responsibility.Observation` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Reported`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`ObservationSnapshot`]
/// and [`ObservationSnapshot::refine`].
pub struct Observation<S: observation_state::Marker> {
    data: ObservationData,
    state: core::marker::PhantomData<S>,
}

impl<S: observation_state::Marker> Observation<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> ObservationState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &ObservationData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> ObservationData {
        self.data
    }
}

impl Observation<observation_state::Reported> {
    /// A new instance, resting in `Reported` — the only state the lifecycle starts one in.
    pub fn new(data: ObservationData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Observation` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`ObservationSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: ObservationState,
    /// What it holds.
    pub data: ObservationData,
}

/// An `Observation` in whichever declared state it was found.
pub enum AnyObservation {
    /// Resting in `Reported`.
    Reported(Observation<observation_state::Reported>),
}

impl ObservationSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `ObservationState` cannot spell one.
    pub fn refine(self) -> AnyObservation {
        match self.state {
            ObservationState::Reported => AnyObservation::Reported(Observation {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyObservation {
    /// The state, as the runtime value.
    pub fn state(&self) -> ObservationState {
        match self {
            Self::Reported(_) => ObservationState::Reported,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> ObservationSnapshot {
        match self {
            Self::Reported(instance) => ObservationSnapshot {
                state: ObservationState::Reported,
                data: instance.into_data(),
            },
        }
    }
}

/// What Run — `commission.responsibility.Run` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`Run<S>`], and at a boundary by [`RunSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunData {
    /// The identity: `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `commission_id` — `commission.responsibility.CommissionId`.
    ///
    /// Carries `runs`: `commission.responsibility.Commission` owns many `commission.responsibility.Run`.
    pub commission_id: CommissionId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// The states of `commission.responsibility.Run`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](run_state::Marker), so [`Run<S>`](Run) can only ever rest in a real state.
pub mod run_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Running {}
        impl Sealed for super::Suspended {}
    }

    /// A declared state of `Run`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::RunState;
    }

    /// `Running`. Where a new instance starts.
    pub struct Running;

    impl Marker for Running {
        const STATE: super::RunState = super::RunState::Running;
    }

    /// `Suspended`.
    pub struct Suspended;

    impl Marker for Suspended {
        const STATE: super::RunState = super::RunState::Suspended;
    }
}

/// Run — `commission.responsibility.Run` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Running`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`RunSnapshot`]
/// and [`RunSnapshot::refine`].
pub struct Run<S: run_state::Marker> {
    data: RunData,
    state: core::marker::PhantomData<S>,
}

impl<S: run_state::Marker> Run<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> RunState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &RunData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> RunData {
        self.data
    }
}

impl Run<run_state::Running> {
    /// A new instance, resting in `Running` — the only state the lifecycle starts one in.
    pub fn new(data: RunData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

impl Run<run_state::Running> {
    /// `suspend` — `Running` → `Suspended`. Taken by the `suspended` outcome of `commission.responsibility.SuspendRun`.
    pub fn suspend(self) -> Run<run_state::Suspended> {
        Run {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl Run<run_state::Suspended> {
    /// `resume` — `Suspended` → `Running`. Taken by the `resumed` outcome of `commission.responsibility.ResumeRun`.
    pub fn resume(self) -> Run<run_state::Running> {
        Run {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

/// `commission.responsibility.Run` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`RunSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: RunState,
    /// What it holds.
    pub data: RunData,
}

/// An `Run` in whichever declared state it was found.
pub enum AnyRun {
    /// Resting in `Running`.
    Running(Run<run_state::Running>),
    /// Resting in `Suspended`.
    Suspended(Run<run_state::Suspended>),
}

impl RunSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `RunState` cannot spell one.
    pub fn refine(self) -> AnyRun {
        match self.state {
            RunState::Running => AnyRun::Running(Run {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            RunState::Suspended => AnyRun::Suspended(Run {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyRun {
    /// The state, as the runtime value.
    pub fn state(&self) -> RunState {
        match self {
            Self::Running(_) => RunState::Running,
            Self::Suspended(_) => RunState::Suspended,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> RunSnapshot {
        match self {
            Self::Running(instance) => RunSnapshot {
                state: RunState::Running,
                data: instance.into_data(),
            },
            Self::Suspended(instance) => RunSnapshot {
                state: RunState::Suspended,
                data: instance.into_data(),
            },
        }
    }
}

/// ResumeRun — the input of `commission.responsibility.ResumeRun`.
///
/// Everything it can result in is [`ResumeRunOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRun {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
}

/// Everything `commission.responsibility.ResumeRun` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeRunOutcome {
    /// `resumed` — otherwise.
    Resumed {
        /// The `commission.responsibility.RunResumed` this outcome publishes.
        run_resumed: RunResumed,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `commission.responsibility.RunStateConflict`.
        error: RunStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// RevalidateActionRequest — the input of `commission.responsibility.RevalidateActionRequest`.
///
/// Everything it can result in is [`RevalidateActionRequestOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevalidateActionRequest {
    /// `action_request_id` — `commission.responsibility.ActionRequestId`.
    pub action_request_id: ActionRequestId,
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `case_id` — `commission.responsibility.CaseId`.
    pub case_id: CaseId,
    /// `expected_case_revision` — `Integer`.
    pub expected_case_revision: i64,
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `commission.responsibility.ProposedActionArguments`.
    pub arguments: ProposedActionArguments,
}

/// Everything `commission.responsibility.RevalidateActionRequest` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevalidateActionRequestOutcome {
    /// `stale` — externally decided (the governor's current revision of the case is not the request's expected case revision).
    Stale {
        /// Why it was refused: `commission.responsibility.ActionRequestStale`.
        error: ActionRequestStale,
    },
    /// `not-admitted` — externally decided (the current frontier refuses the action).
    NotAdmitted {
        /// Why it was refused: `commission.responsibility.ActionNotAdmitted`.
        error: ActionNotAdmitted,
    },
    /// `needs-authority` — externally decided (the current frontier lists the action as ApprovalRequired, naming one capability).
    NeedsAuthority {
        /// Why it was refused: `commission.responsibility.ActionNeedsAuthority`.
        error: ActionNeedsAuthority,
    },
    /// `admitted` — otherwise.
    Admitted,
}

/// StartRun — the input of `commission.responsibility.StartRun`.
///
/// Everything it can result in is [`StartRunOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRun {
    /// `commission_id` — `commission.responsibility.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// Everything `commission.responsibility.StartRun` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartRunOutcome {
    /// `started` — otherwise.
    Started {
        /// The `commission.responsibility.RunStarted` this outcome publishes.
        run_started: RunStarted,
    },
}

/// SuspendRun — the input of `commission.responsibility.SuspendRun`.
///
/// Everything it can result in is [`SuspendRunOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendRun {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `reason` — `commission.responsibility.SuspensionReason`.
    pub reason: SuspensionReason,
}

/// Everything `commission.responsibility.SuspendRun` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspendRunOutcome {
    /// `suspended` — otherwise.
    Suspended {
        /// The `commission.responsibility.RunSuspended` this outcome publishes.
        run_suspended: RunSuspended,
    },
    /// `wrong-state` — from a state no declared move starts in.
    WrongState {
        /// Why it was refused: `commission.responsibility.RunStateConflict`.
        error: RunStateConflict,
    },
    /// `wrong-state` — for an instance no record carries.
    ///
    /// The same declared branch and error as [`Self::WrongState`], without the error's fields: an instance
    /// that does not exist has nothing for them to describe (`docs/design/unknown-instance-seams.md`).
    WrongStateUnknownInstance,
}

/// RunResumed — the event `commission.responsibility.RunResumed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResumed {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
}

/// RunStarted — the event `commission.responsibility.RunStarted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStarted {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `commission_id` — `commission.responsibility.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
}

/// RunSuspended — the event `commission.responsibility.RunSuspended`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSuspended {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `reason` — `commission.responsibility.SuspensionReason`.
    pub reason: SuspensionReason,
}

/// The declared error `commission.responsibility.ActionNeedsAuthority`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionNeedsAuthority {
    /// `action` — `String`.
    pub action: String,
    /// `capability` — `String`.
    pub capability: String,
}

/// The declared error `commission.responsibility.ActionNotAdmitted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionNotAdmitted {
    /// `action` — `String`.
    pub action: String,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// The declared error `commission.responsibility.ActionRequestStale`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequestStale {
    /// `expected_case_revision` — `Integer`.
    pub expected_case_revision: i64,
    /// `current_case_revision` — `Integer`.
    pub current_case_revision: i64,
}

/// The declared error `commission.responsibility.RunStateConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStateConflict {
    /// `state` — `commission.responsibility.Run.State`.
    pub state: RunState,
}

/// RunStates — one row of the view `commission.responsibility.RunStates`.
///
/// Projects `commission.responsibility.Run` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStates {
    /// `run_id` — `commission.responsibility.RunId`.
    pub run_id: RunId,
    /// `commission_id` — `commission.responsibility.CommissionId`.
    pub commission_id: CommissionId,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `state` — `commission.responsibility.Run.State`.
    pub state: RunState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `commission.responsibility.ResumeRun` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ResumeRunBehavior {
        /// Decides and enacts exactly one declared outcome of `commission.responsibility.ResumeRun`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn resume_run(&mut self, input: super::ResumeRun) -> Result<super::ResumeRunOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `commission.responsibility.RevalidateActionRequest` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `commission.responsibility.ActionRequestStale`, which the specification gives no source, in `stale`.
    ///
    /// Contract: given `commission.responsibility.RevalidateActionRequest` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `stale` externally decided (the governor's current revision of the case is not the request's expected case revision), error `commission.responsibility.ActionRequestStale`; `not-admitted` externally decided (the current frontier refuses the action), error `commission.responsibility.ActionNotAdmitted`; `needs-authority` externally decided (the current frontier lists the action as ApprovalRequired, naming one capability), error `commission.responsibility.ActionNeedsAuthority`; `admitted` otherwise.
    pub trait RevalidateActionRequestBehavior {
        /// Decides and enacts exactly one declared outcome of `commission.responsibility.RevalidateActionRequest`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn revalidate_action_request(&mut self, input: super::RevalidateActionRequest) -> Result<super::RevalidateActionRequestOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `commission.responsibility.StartRun` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait StartRunBehavior {
        /// Decides and enacts exactly one declared outcome of `commission.responsibility.StartRun`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn start_run(&mut self, input: super::StartRun) -> Result<super::StartRunOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `commission.responsibility.SuspendRun` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait SuspendRunBehavior {
        /// Decides and enacts exactly one declared outcome of `commission.responsibility.SuspendRun`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn suspend_run(&mut self, input: super::SuspendRun) -> Result<super::SuspendRunOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `commission.responsibility.RunStates` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait RunStatesQuery {
        /// Serves `commission.responsibility.RunStates` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn run_states(&self) -> Result<Vec<super::RunStates>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl RevalidateActionRequestBehavior for Unimplemented {
        fn revalidate_action_request(&mut self, _input: super::RevalidateActionRequest) -> Result<super::RevalidateActionRequestOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "commission.responsibility.RevalidateActionRequest" })
        }
    }
}
