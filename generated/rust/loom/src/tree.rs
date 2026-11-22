// generated from loom v1
// contract digest d8d7d39dbecbd81471f7e6f22a7ef0473ff3fc40b31db137f4535fc29e93e708
// do not edit: regenerate with `ess synthesize --layout crate`

//! Tree — `loom.tree`.
//!
//! A tree of protocol cases run from a case store: how a run of the tree stops, the decisions a waiting tree names with the command that records each, the budget of one run, a person's approval of one capability for one case, the protocols a tree was opened from, the status of every case of the tree, the result of verifying its run logs, and the refusals of the tree commands.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ApprovalDecision — `loom.tree.ApprovalDecision`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// `Allow`.
    Allow,
    /// `Deny`.
    Deny,
}

/// CaseApproval — `loom.tree.CaseApproval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseApproval {
    /// `case` — `String`.
    pub case: String,
    /// `capability` — `String`.
    pub capability: String,
    /// `decision` — `loom.tree.ApprovalDecision`.
    pub decision: ApprovalDecision,
    /// `principal` — `String`.
    pub principal: String,
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `at` — `Timestamp`.
    pub at: crate::primitives::Timestamp,
}

/// CaseStatus — `loom.tree.CaseStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseStatus {
    /// `case` — `String`.
    pub case: String,
    /// `parent` — `Optional<String>`.
    pub parent: Option<String>,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `revision` — `Integer`.
    pub revision: i64,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
    /// `stop` — `Optional<String>`.
    pub stop: Option<String>,
    /// `open_obligations` — `List<String>`.
    pub open_obligations: Vec<String>,
    /// `case_file` — `String`.
    pub case_file: String,
    /// `run_log` — `String`.
    pub run_log: String,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `protocol_id` — `String`.
    pub protocol_id: String,
    /// `protocol_revision` — `String`.
    pub protocol_revision: String,
    /// `stop_next` — `List<String>`.
    pub stop_next: Vec<String>,
    /// `evidence` — `List<loom.tree.StatusEvidence>`.
    pub evidence: Vec<StatusEvidence>,
    /// `runs` — `List<loom.tree.StatusRun>`.
    pub runs: Vec<StatusRun>,
    /// `approvals` — `List<loom.tree.StatusApproval>`.
    pub approvals: Vec<StatusApproval>,
    /// `copies` — `List<loom.tree.StatusCopy>`.
    pub copies: Vec<StatusCopy>,
}

/// DecisionKind — `loom.tree.DecisionKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    /// `Approve`.
    Approve,
    /// `ApproveAction`.
    ApproveAction,
    /// `Close`.
    Close,
    /// `Handoff`.
    Handoff,
}

/// LogFailure — `loom.tree.LogFailure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFailure {
    /// `case` — `String`.
    pub case: String,
    /// `line` — `Integer`.
    pub line: i64,
    /// `detail` — `String`.
    pub detail: String,
}

/// NextDecision — `loom.tree.NextDecision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextDecision {
    /// `case` — `String`.
    pub case: String,
    /// `kind` — `loom.tree.DecisionKind`.
    pub kind: DecisionKind,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `capability` — `Optional<String>`.
    pub capability: Option<String>,
    /// `decides` — `String`.
    pub decides: String,
    /// `subject` — `Optional<String>`.
    pub subject: Option<String>,
    /// `revision` — `Optional<String>`.
    pub revision: Option<String>,
    /// `path` — `Optional<String>`.
    pub path: Option<String>,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
    /// `command` — `List<String>`.
    pub command: Vec<String>,
}

/// StatusApproval — `loom.tree.StatusApproval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusApproval {
    /// `capability` — `String`.
    pub capability: String,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `answer` — `loom.tree.ApprovalDecision`.
    pub answer: ApprovalDecision,
    /// `actor` — `String`.
    pub actor: String,
    /// `at` — `String`.
    pub at: String,
}

/// StatusCopy — `loom.tree.StatusCopy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCopy {
    /// `source_case` — `String`.
    pub source_case: String,
    /// `target_case` — `String`.
    pub target_case: String,
    /// `record_kind` — `String`.
    pub record_kind: String,
}

/// StatusEvidence — `loom.tree.StatusEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEvidence {
    /// `index` — `Integer`.
    pub index: i64,
    /// `evidence_id` — `String`.
    pub evidence_id: String,
    /// `kind` — `String`.
    pub kind: String,
    /// `result` — `Optional<String>`.
    pub result: Option<String>,
    /// `subject` — `String`.
    pub subject: String,
    /// `subject_revision` — `String`.
    pub subject_revision: String,
    /// `upstream_revisions` — `Json`.
    pub upstream_revisions: crate::json::Value,
    /// `producer` — `String`.
    pub producer: String,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
}

/// StatusRun — `loom.tree.StatusRun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRun {
    /// `action` — `String`.
    pub action: String,
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `attempt` — `Integer`.
    pub attempt: i64,
    /// `retried` — `Boolean`.
    pub retried: bool,
    /// `retry_of` — `List<String>`.
    pub retry_of: Vec<String>,
}

/// TreeBudget — `loom.tree.TreeBudget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBudget {
    /// `max_steps` — `Optional<Integer>`.
    pub max_steps: Option<i64>,
    /// `max_wall_seconds` — `Optional<Integer>`.
    pub max_wall_seconds: Option<i64>,
}

/// TreeCatalog — `loom.tree.TreeCatalog`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeCatalog {
    /// `Host`.
    Host,
    /// `Directory`.
    Directory,
}

/// TreeLogHead — `loom.tree.TreeLogHead`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLogHead {
    /// `case` — `String`.
    pub case: String,
    /// `seq` — `Integer`.
    pub seq: i64,
    /// `digest` — `String`.
    pub digest: String,
}

/// TreeManifest — `loom.tree.TreeManifest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeManifest {
    /// `format` — `String`.
    pub format: String,
    /// `root` — `String`.
    pub root: String,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `bindings` — `String`.
    pub bindings: String,
    /// `catalog` — `loom.tree.TreeCatalog`.
    pub catalog: TreeCatalog,
}

/// TreeProtocol — `loom.tree.TreeProtocol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeProtocol {
    /// `registration` — `String`.
    pub registration: String,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `revision` — `Integer`.
    pub revision: i64,
    /// `file` — `String`.
    pub file: String,
    /// `sha256` — `String`.
    pub sha256: String,
}

/// TreeRefusal — `loom.tree.TreeRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRefusal {
    /// `reason` — `loom.tree.TreeRefusalReason`.
    pub reason: TreeRefusalReason,
    /// `detail` — `String`.
    pub detail: String,
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
    /// `file` — `Optional<String>`.
    pub file: Option<String>,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `story` — `Optional<String>`.
    pub story: Option<String>,
    /// `holder` — `Optional<Integer>`.
    pub holder: Option<i64>,
}

/// TreeRefusalReason — `loom.tree.TreeRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeRefusalReason {
    /// `StoreInUse`.
    StoreInUse,
    /// `NotATree`.
    NotATree,
    /// `StoreNotEmpty`.
    StoreNotEmpty,
    /// `WorkspaceUnusable`.
    WorkspaceUnusable,
    /// `BindingsRefused`.
    BindingsRefused,
    /// `AgentNotRunYet`.
    AgentNotRunYet,
    /// `UnknownProtocol`.
    UnknownProtocol,
    /// `InvalidRevision`.
    InvalidRevision,
    /// `InvalidInstant`.
    InvalidInstant,
    /// `CaseNotInTree`.
    CaseNotInTree,
    /// `CaseTerminated`.
    CaseTerminated,
    /// `OutcomeNotLegitimate`.
    OutcomeNotLegitimate,
    /// `NotAPersonAction`.
    NotAPersonAction,
    /// `ConfinementRefused`.
    ConfinementRefused,
    /// `StoreUnreadable`.
    StoreUnreadable,
    /// `LogRefused`.
    LogRefused,
    /// `StoredProtocolChanged`.
    StoredProtocolChanged,
    /// `DuplicateProtocol`.
    DuplicateProtocol,
    /// `ProtocolRefused`.
    ProtocolRefused,
    /// `ProtocolLinkLeavesDirectory`.
    ProtocolLinkLeavesDirectory,
    /// `ProtocolNotInDirectory`.
    ProtocolNotInDirectory,
    /// `NoProtocolFiles`.
    NoProtocolFiles,
    /// `UnknownStatusFormat`.
    UnknownStatusFormat,
}

/// TreeRun — `loom.tree.TreeRun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRun {
    /// `stop` — `loom.tree.TreeStop`.
    pub stop: TreeStop,
    /// `case` — `String`.
    pub case: String,
    /// `steps` — `Integer`.
    pub steps: i64,
    /// `decisions` — `List<loom.tree.NextDecision>`.
    pub decisions: Vec<NextDecision>,
    /// `detail` — `Optional<String>`.
    pub detail: Option<String>,
}

/// TreeStatus — `loom.tree.TreeStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeStatus {
    /// `format` — `String`.
    pub format: String,
    /// `version` — `String`.
    pub version: String,
    /// `store` — `String`.
    pub store: String,
    /// `workspace` — `String`.
    pub workspace: String,
    /// `root` — `String`.
    pub root: String,
    /// `cases` — `List<loom.tree.CaseStatus>`.
    pub cases: Vec<CaseStatus>,
    /// `protocols` — `List<loom.tree.TreeProtocol>`.
    pub protocols: Vec<TreeProtocol>,
}

/// TreeStop — `loom.tree.TreeStop`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeStop {
    /// `Terminal`.
    Terminal,
    /// `AwaitingPerson`.
    AwaitingPerson,
    /// `AwaitingAuthority`.
    AwaitingAuthority,
    /// `Stuck`.
    Stuck,
    /// `BudgetSpent`.
    BudgetSpent,
}

/// TreeVerify — `loom.tree.TreeVerify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeVerify {
    /// `cases` — `Integer`.
    pub cases: i64,
    /// `failure` — `Optional<loom.tree.LogFailure>`.
    pub failure: Option<LogFailure>,
}
