// generated from loom v1
// model digest 4a99756f85a24fac97752e928c318c5c1b6ffbd3343706808e92c1fbf83dada5
// contract digest d1418824cb56c636461a5a81d92386592eac2357fd0ef3ac5cffc66165578878
// do not edit: regenerate with `ess synthesize --layout crate`

//! Session — `loom.session`.
//!
//! The agent session an AgentExecutor runs through metaharness (harness, Claude Code or Codex) in a scratch copy of the workspace, and the change it returns as a proposed action. Proposed and applied are states of Commission's ActionRequest, not of the session: a session is one record.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// The states of `loom.session.AgentSession`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `AgentSession<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSessionState {
    /// `Ended`.
    Ended,
}

/// CallDecision — `loom.session.CallDecision`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDecision {
    /// `Allow`.
    Allow,
    /// `Deny`.
    Deny,
    /// `Replace`.
    Replace,
    /// `Abstain`.
    Abstain,
}

/// ChangedFile — `loom.session.ChangedFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// `path` — `String`.
    pub path: String,
    /// `content` — `Optional<String>`.
    pub content: Option<String>,
}

/// CommissionId — `loom.session.CommissionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionId(pub crate::primitives::Uuid);

/// DecidedBy — `loom.session.DecidedBy`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecidedBy {
    /// `Embedder`.
    Embedder,
    /// `Frame`.
    Frame,
    /// `Deadline`.
    Deadline,
    /// `Adapter`.
    Adapter,
    /// `Observe`.
    Observe,
}

/// FrontierId — `loom.session.FrontierId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierId(pub crate::primitives::Uuid);

/// SessionId — `loom.session.SessionId`: a distinct wrapper around `Uuid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub crate::primitives::Uuid);

/// SessionKind — `loom.session.SessionKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// `ClaudeCode`.
    ClaudeCode,
    /// `Codex`.
    Codex,
    /// `Harness`.
    Harness,
}

/// ToolDecision — `loom.session.ToolDecision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDecision {
    /// `call_id` — `String`.
    pub call_id: String,
    /// `tool` — `Optional<String>`.
    pub tool: Option<String>,
    /// `decision` — `loom.session.CallDecision`.
    pub decision: CallDecision,
    /// `reason` — `Optional<String>`.
    pub reason: Option<String>,
    /// `decided_by` — `loom.session.DecidedBy`.
    pub decided_by: DecidedBy,
}

/// What AgentSession — `loom.session.AgentSession` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`AgentSession<S>`], and at a boundary by [`AgentSessionSnapshot::state`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionData {
    /// The identity: `session_id` — `loom.session.SessionId`.
    pub session_id: SessionId,
    /// `kind` — `loom.session.SessionKind`.
    pub kind: SessionKind,
    /// `commission_id` — `loom.session.CommissionId`.
    pub commission_id: CommissionId,
    /// `frontier_id` — `loom.session.FrontierId`.
    pub frontier_id: FrontierId,
    /// `vendor_session_id` — `Optional<String>`.
    pub vendor_session_id: Option<String>,
    /// `agent` — `Optional<loom.agent.AgentId>`.
    pub agent: Option<crate::agent::AgentId>,
    /// `continues` — `Optional<loom.session.SessionId>`.
    ///
    /// Carries `predecessor`: `loom.session.AgentSession` references one `loom.session.AgentSession`.
    pub continues: Option<SessionId>,
    /// `action` — `String`.
    pub action: String,
    /// `files` — `List<loom.session.ChangedFile>`.
    pub files: Vec<ChangedFile>,
}

/// The states of `loom.session.AgentSession`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](agent_session_state::Marker), so [`AgentSession<S>`](AgentSession) can only ever rest in a real state.
pub mod agent_session_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Ended {}
    }

    /// A declared state of `AgentSession`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::AgentSessionState;
    }

    /// `Ended`. Where a new instance starts.
    pub struct Ended;

    impl Marker for Ended {
        const STATE: super::AgentSessionState = super::AgentSessionState::Ended;
    }
}

/// AgentSession — `loom.session.AgentSession` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Ended`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`AgentSessionSnapshot`]
/// and [`AgentSessionSnapshot::refine`].
pub struct AgentSession<S: agent_session_state::Marker> {
    data: AgentSessionData,
    state: core::marker::PhantomData<S>,
}

impl<S: agent_session_state::Marker> AgentSession<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> AgentSessionState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &AgentSessionData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> AgentSessionData {
        self.data
    }
}

impl AgentSession<agent_session_state::Ended> {
    /// A new instance, resting in `Ended` — the only state the lifecycle starts one in.
    pub fn new(data: AgentSessionData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.session.AgentSession` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`AgentSessionSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: AgentSessionState,
    /// What it holds.
    pub data: AgentSessionData,
}

/// An `AgentSession` in whichever declared state it was found.
pub enum AnyAgentSession {
    /// Resting in `Ended`.
    Ended(AgentSession<agent_session_state::Ended>),
}

impl AgentSessionSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `AgentSessionState` cannot spell one.
    pub fn refine(self) -> AnyAgentSession {
        match self.state {
            AgentSessionState::Ended => AnyAgentSession::Ended(AgentSession {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyAgentSession {
    /// The state, as the runtime value.
    pub fn state(&self) -> AgentSessionState {
        match self {
            Self::Ended(_) => AgentSessionState::Ended,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> AgentSessionSnapshot {
        match self {
            Self::Ended(instance) => AgentSessionSnapshot {
                state: AgentSessionState::Ended,
                data: instance.into_data(),
            },
        }
    }
}

/// ProposeSessionChange — the input of `loom.session.ProposeSessionChange`.
///
/// Everything it can result in is [`ProposeSessionChangeOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposeSessionChange {
    /// `session_id` — `loom.session.SessionId`.
    pub session_id: SessionId,
    /// `kind` — `loom.session.SessionKind`.
    pub kind: SessionKind,
    /// `commission_id` — `loom.session.CommissionId`.
    pub commission_id: CommissionId,
    /// `frontier_id` — `loom.session.FrontierId`.
    pub frontier_id: FrontierId,
    /// `vendor_session_id` — `Optional<String>`.
    pub vendor_session_id: Option<String>,
    /// `agent` — `Optional<loom.agent.AgentId>`.
    pub agent: Option<crate::agent::AgentId>,
    /// `continues` — `Optional<loom.session.SessionId>`.
    pub continues: Option<SessionId>,
    /// `action` — `String`.
    pub action: String,
    /// `files` — `List<loom.session.ChangedFile>`.
    pub files: Vec<ChangedFile>,
}

/// Everything `loom.session.ProposeSessionChange` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposeSessionChangeOutcome {
    /// `proposed` — otherwise.
    Proposed {
        /// The `loom.session.SessionChangeProposed` this outcome publishes.
        session_change_proposed: SessionChangeProposed,
    },
}

/// SessionChangeProposed — the event `loom.session.SessionChangeProposed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChangeProposed {
    /// `session_id` — `loom.session.SessionId`.
    pub session_id: SessionId,
    /// `commission_id` — `loom.session.CommissionId`.
    pub commission_id: CommissionId,
    /// `frontier_id` — `loom.session.FrontierId`.
    pub frontier_id: FrontierId,
    /// `continues` — `Optional<loom.session.SessionId>`.
    pub continues: Option<SessionId>,
    /// `action` — `String`.
    pub action: String,
    /// `files` — `List<loom.session.ChangedFile>`.
    pub files: Vec<ChangedFile>,
}

/// AgentSessions — one row of the view `loom.session.AgentSessions`.
///
/// Projects `loom.session.AgentSession` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessions {
    /// `session_id` — `loom.session.SessionId`.
    pub session_id: SessionId,
    /// `kind` — `loom.session.SessionKind`.
    pub kind: SessionKind,
    /// `commission_id` — `loom.session.CommissionId`.
    pub commission_id: CommissionId,
    /// `frontier_id` — `loom.session.FrontierId`.
    pub frontier_id: FrontierId,
    /// `vendor_session_id` — `Optional<String>`.
    pub vendor_session_id: Option<String>,
    /// `agent` — `Optional<loom.agent.AgentId>`.
    pub agent: Option<crate::agent::AgentId>,
    /// `continues` — `Optional<loom.session.SessionId>`.
    pub continues: Option<SessionId>,
    /// `action` — `String`.
    pub action: String,
    /// `files` — `List<loom.session.ChangedFile>`.
    pub files: Vec<ChangedFile>,
    /// `state` — `loom.session.AgentSession.State`.
    pub state: AgentSessionState,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
pub mod obligations {
    /// The behaviour `loom.session.ProposeSessionChange` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait ProposeSessionChangeBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.session.ProposeSessionChange`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn propose_session_change(&mut self, input: super::ProposeSessionChange) -> Result<super::ProposeSessionChangeOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.session.AgentSessions` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait AgentSessionsQuery {
        /// Serves `loom.session.AgentSessions` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn agent_sessions(&self) -> Result<Vec<super::AgentSessions>, crate::obligation::UnmetObligation>;
    }

}
