// generated from loom v1
// contract digest 1b38c8556b2b1608e93e2b9aa51a96e33bcab4b509b8eed67d8d0ed794ddbbd1
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

/// ArtifactRevision — `loom.tree.ArtifactRevision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRevision {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `revision` — `String`.
    pub revision: String,
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

/// CaseSelector — `loom.tree.CaseSelector`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSelector {
    /// `path` — `Optional<String>`.
    pub path: Option<String>,
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `every` — `Boolean`.
    pub every: bool,
}

/// CaseSource — `loom.tree.CaseSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSource {
    /// `case` — `loom.tree.CaseSelector`.
    pub case: CaseSelector,
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
    /// `log` — `Optional<loom.tree.StatusLog>`.
    pub log: Option<StatusLog>,
    /// `budget` — `Optional<loom.tree.SpentBudget>`.
    pub budget: Option<SpentBudget>,
}

/// CasesSource — `loom.tree.CasesSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasesSource {
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
}

/// CheckDifference — `loom.tree.CheckDifference`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckDifference {
    /// `check` — `String`.
    pub check: String,
    /// `kind` — `String`.
    pub kind: String,
    /// `reason` — `loom.tree.DifferenceReason`.
    pub reason: DifferenceReason,
    /// `expected` — `String`.
    pub expected: String,
    /// `found` — `Optional<String>`.
    pub found: Option<String>,
}

/// CheckKind — `loom.tree.CheckKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// `exists`.
    Exists,
    /// `count`.
    Count,
    /// `equals`.
    Equals,
    /// `contains`.
    Contains,
    /// `lacks`.
    Lacks,
    /// `matches`.
    Matches,
    /// `not_matches`.
    NotMatches,
    /// `all`.
    All,
    /// `none`.
    None,
    /// `some`.
    Some,
    /// `sequence`.
    Sequence,
    /// `before`.
    Before,
    /// `compare`.
    Compare,
}

/// CompareOp — `loom.tree.CompareOp`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// `eq`.
    Eq,
    /// `ne`.
    Ne,
    /// `lt`.
    Lt,
    /// `le`.
    Le,
    /// `gt`.
    Gt,
    /// `ge`.
    Ge,
    /// `subset`.
    Subset,
}

/// Condition — `loom.tree.Condition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// `path` — `String`.
    pub path: String,
    /// `equals` — `Optional<Json>`.
    pub equals: Option<crate::json::Value>,
    /// `not_equals` — `Optional<Json>`.
    pub not_equals: Option<crate::json::Value>,
    /// `in` — `Optional<List<Json>>`.
    pub r#in: Option<Vec<crate::json::Value>>,
    /// `not_in` — `Optional<List<Json>>`.
    pub not_in: Option<Vec<crate::json::Value>>,
    /// `contains` — `Optional<Json>`.
    pub contains: Option<crate::json::Value>,
    /// `lacks` — `Optional<Json>`.
    pub lacks: Option<crate::json::Value>,
    /// `matches` — `Optional<String>`.
    pub matches: Option<String>,
    /// `not_matches` — `Optional<String>`.
    pub not_matches: Option<String>,
    /// `min` — `Optional<Json>`.
    pub min: Option<crate::json::Value>,
    /// `max` — `Optional<Json>`.
    pub max: Option<crate::json::Value>,
    /// `type` — `Optional<loom.tree.ValueType>`.
    pub r#type: Option<ValueType>,
    /// `length` — `Optional<loom.tree.LengthBound>`.
    pub length: Option<LengthBound>,
    /// `exists` — `Optional<Boolean>`.
    pub exists: Option<bool>,
}

/// ConditionGroup — `loom.tree.ConditionGroup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionGroup {
    /// `all` — `List<loom.tree.Condition>`.
    pub all: Vec<Condition>,
}

/// Conditions — `loom.tree.Conditions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conditions {
    /// `all` — `List<loom.tree.Condition>`.
    pub all: Vec<Condition>,
    /// `any_of` — `List<loom.tree.ConditionGroup>`.
    pub any_of: Vec<ConditionGroup>,
}

/// CycleArea — `loom.tree.CycleArea`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleArea {
    /// `work`.
    Work,
    /// `store`.
    Store,
}

/// CycleCondition — `loom.tree.CycleCondition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleCondition {
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
    /// `declares` — `Optional<loom.tree.DeclaredName>`.
    pub declares: Option<DeclaredName>,
    /// `suite` — `Optional<String>`.
    pub suite: Option<String>,
    /// `test` — `Optional<loom.tree.Condition>`.
    pub test: Option<Condition>,
}

/// CycleExpectation — `loom.tree.CycleExpectation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleExpectation {
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
    /// `stop` — `Optional<loom.tree.StopExpectation>`.
    pub stop: Option<StopExpectation>,
    /// `checks` — `List<loom.tree.SuiteCheck>`.
    pub checks: Vec<SuiteCheck>,
}

/// CycleResult — `loom.tree.CycleResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleResult {
    /// `name` — `String`.
    pub name: String,
    /// `pending` — `Boolean`.
    pub pending: bool,
    /// `waits_for` — `Optional<loom.tree.WaitingCycle>`.
    pub waits_for: Option<WaitingCycle>,
    /// `run` — `Boolean`.
    pub run: bool,
    /// `not_run_when` — `Optional<Integer>`.
    pub not_run_when: Option<i64>,
    /// `as_expected` — `Boolean`.
    pub as_expected: bool,
    /// `difference` — `Optional<loom.tree.CheckDifference>`.
    pub difference: Option<CheckDifference>,
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `seconds` — `Decimal`.
    pub seconds: crate::primitives::Decimal,
}

/// CycleStep — `loom.tree.CycleStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleStep {
    /// `do` — `loom.tree.StepKind`.
    pub r#do: StepKind,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `at` — `Optional<Integer>`.
    pub at: Option<i64>,
    /// `exit` — `Optional<loom.tree.ExitExpectation>`.
    pub exit: Option<ExitExpectation>,
    /// `refusal` — `Optional<String>`.
    pub refusal: Option<String>,
    /// `engine` — `Optional<String>`.
    pub engine: Option<String>,
    /// `rounds` — `Optional<Integer>`.
    pub rounds: Option<i64>,
    /// `checks` — `List<loom.tree.SuiteCheck>`.
    pub checks: Vec<SuiteCheck>,
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
    /// `capability` — `Optional<String>`.
    pub capability: Option<String>,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
    /// `result` — `Optional<String>`.
    pub result: Option<String>,
    /// `reasons` — `Optional<List<String>>`.
    pub reasons: Option<Vec<String>>,
    /// `deny` — `Optional<Boolean>`.
    pub deny: Option<bool>,
    /// `option` — `Optional<String>`.
    pub option: Option<String>,
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

/// DeclaredName — `loom.tree.DeclaredName`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredName {
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
}

/// DifferenceReason — `loom.tree.DifferenceReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifferenceReason {
    /// `CheckFailed`.
    CheckFailed,
    /// `ExitDiffers`.
    ExitDiffers,
    /// `RefusalDiffers`.
    RefusalDiffers,
    /// `OutcomeDiffers`.
    OutcomeDiffers,
    /// `StopDiffers`.
    StopDiffers,
    /// `VerifyFailed`.
    VerifyFailed,
    /// `ScriptUnused`.
    ScriptUnused,
    /// `RoundsSpent`.
    RoundsSpent,
    /// `OperandNotOne`.
    OperandNotOne,
    /// `BlobChanged`.
    BlobChanged,
    /// `PathOutsideCycle`.
    PathOutsideCycle,
    /// `SelectionTooLarge`.
    SelectionTooLarge,
    /// `SeedUnusable`.
    SeedUnusable,
    /// `StepFailed`.
    StepFailed,
}

/// DocSource — `loom.tree.DocSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSource {
    /// `in` — `loom.tree.CycleArea`.
    pub r#in: CycleArea,
    /// `path` — `String`.
    pub path: String,
}

/// ExitExpectation — `loom.tree.ExitExpectation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitExpectation {
    /// `status` — `Optional<Integer>`.
    pub status: Option<i64>,
    /// `not` — `Optional<Integer>`.
    pub not: Option<i64>,
}

/// FilesSource — `loom.tree.FilesSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesSource {
    /// `in` — `loom.tree.CycleArea`.
    pub r#in: CycleArea,
    /// `glob` — `String`.
    pub glob: String,
}

/// Follow — `loom.tree.Follow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Follow {
    /// `target` — `loom.tree.FollowTarget`.
    pub target: FollowTarget,
    /// `member` — `Optional<String>`.
    pub member: Option<String>,
    /// `below` — `Optional<String>`.
    pub below: Option<String>,
}

/// FollowTarget — `loom.tree.FollowTarget`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowTarget {
    /// `blob`.
    Blob,
    /// `evidence`.
    Evidence,
    /// `file`.
    File,
}

/// LengthBound — `loom.tree.LengthBound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LengthBound {
    /// `min` — `Optional<Integer>`.
    pub min: Option<i64>,
    /// `max` — `Optional<Integer>`.
    pub max: Option<i64>,
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

/// LogSource — `loom.tree.LogSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSource {
    /// `case` — `loom.tree.CaseSelector`.
    pub case: CaseSelector,
    /// `event` — `List<String>`.
    pub event: Vec<String>,
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
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
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

/// Operand — `loom.tree.Operand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    /// `value` — `Optional<Json>`.
    pub value: Option<crate::json::Value>,
    /// `select` — `Optional<List<loom.tree.SelectItem>>`.
    pub select: Option<Vec<SelectItem>>,
    /// `take` — `Optional<loom.tree.OperandTake>`.
    pub take: Option<OperandTake>,
    /// `plus` — `Optional<Integer>`.
    pub plus: Option<i64>,
    /// `after` — `Optional<String>`.
    pub after: Option<String>,
}

/// OperandTake — `loom.tree.OperandTake`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandTake {
    /// `value`.
    Value,
    /// `count`.
    Count,
    /// `size`.
    Size,
    /// `digest`.
    Digest,
    /// `length`.
    Length,
}

/// PickPosition — `loom.tree.PickPosition`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickPosition {
    /// `first`.
    First,
    /// `last`.
    Last,
    /// `index`.
    Index,
}

/// ReadAs — `loom.tree.ReadAs`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadAs {
    /// `text`.
    Text,
    /// `lines`.
    Lines,
    /// `json`.
    Json,
    /// `jsonl`.
    Jsonl,
    /// `yaml`.
    Yaml,
}

/// RecordsSource — `loom.tree.RecordsSource`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordsSource {
    /// `case` — `loom.tree.CaseSelector`.
    pub case: CaseSelector,
    /// `kind` — `List<String>`.
    pub kind: Vec<String>,
}

/// ScriptedApproval — `loom.tree.ScriptedApproval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedApproval {
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
    /// `capability` — `Optional<String>`.
    pub capability: Option<String>,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `result` — `Optional<String>`.
    pub result: Option<String>,
    /// `reasons` — `Optional<List<String>>`.
    pub reasons: Option<Vec<String>>,
    /// `deny` — `Optional<Boolean>`.
    pub deny: Option<bool>,
    /// `times` — `loom.tree.UseBound`.
    pub times: UseBound,
    /// `optional` — `Boolean`.
    pub optional: bool,
}

/// ScriptedChoice — `loom.tree.ScriptedChoice`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedChoice {
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
    /// `option` — `Optional<String>`.
    pub option: Option<String>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `times` — `loom.tree.UseBound`.
    pub times: UseBound,
    /// `optional` — `Boolean`.
    pub optional: bool,
}

/// ScriptedClose — `loom.tree.ScriptedClose`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedClose {
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
    /// `outcome` — `String`.
    pub outcome: String,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `times` — `loom.tree.UseBound`.
    pub times: UseBound,
    /// `optional` — `Boolean`.
    pub optional: bool,
}

/// ScriptedDecision — `loom.tree.ScriptedDecision`: one of a fixed set of shapes, tagged on the wire by `take`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedDecision {
    /// Tagged `approve` — `loom.tree.ScriptedApproval`.
    Approve(ScriptedApproval),
    /// Tagged `close` — `loom.tree.ScriptedClose`.
    Close(ScriptedClose),
    /// Tagged `decide` — `loom.tree.ScriptedChoice`.
    Decide(ScriptedChoice),
}

/// SeedEntry — `loom.tree.SeedEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedEntry {
    /// `from` — `String`.
    pub from: String,
    /// `to` — `String`.
    pub to: String,
}

/// SelectItem — `loom.tree.SelectItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    /// `log` — `Optional<loom.tree.LogSource>`.
    pub log: Option<LogSource>,
    /// `records` — `Optional<loom.tree.RecordsSource>`.
    pub records: Option<RecordsSource>,
    /// `runs` — `Optional<loom.tree.CaseSource>`.
    pub runs: Option<CaseSource>,
    /// `cases` — `Optional<loom.tree.CasesSource>`.
    pub cases: Option<CasesSource>,
    /// `doc` — `Optional<loom.tree.DocSource>`.
    pub doc: Option<DocSource>,
    /// `files` — `Optional<loom.tree.FilesSource>`.
    pub files: Option<FilesSource>,
    /// `step` — `Optional<String>`.
    pub step: Option<String>,
    /// `suite` — `Optional<String>`.
    pub suite: Option<String>,
    /// `requests` — `Optional<loom.tree.CaseSource>`.
    pub requests: Option<CaseSource>,
    /// `calls` — `Optional<loom.tree.CaseSource>`.
    pub calls: Option<CaseSource>,
    /// `where` — `Optional<loom.tree.Conditions>`.
    pub r#where: Option<Conditions>,
    /// `pick` — `Optional<loom.tree.SelectPick>`.
    pub pick: Option<SelectPick>,
    /// `path` — `Optional<String>`.
    pub path: Option<String>,
    /// `follow` — `Optional<loom.tree.Follow>`.
    pub follow: Option<Follow>,
    /// `as` — `Optional<loom.tree.ReadAs>`.
    pub r#as: Option<ReadAs>,
}

/// SelectPick — `loom.tree.SelectPick`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectPick {
    /// `position` — `loom.tree.PickPosition`.
    pub position: PickPosition,
    /// `index` — `Optional<Integer>`.
    pub index: Option<i64>,
}

/// SourceKind — `loom.tree.SourceKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// `log`.
    Log,
    /// `records`.
    Records,
    /// `runs`.
    Runs,
    /// `cases`.
    Cases,
    /// `doc`.
    Doc,
    /// `files`.
    Files,
    /// `step`.
    Step,
    /// `suite`.
    Suite,
    /// `requests`.
    Requests,
    /// `calls`.
    Calls,
}

/// SpentBudget — `loom.tree.SpentBudget`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpentBudget {
    /// `Cancel`.
    Cancel,
    /// `MaxSteps`.
    MaxSteps,
    /// `MaxWallSeconds`.
    MaxWallSeconds,
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
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
}

/// StatusLog — `loom.tree.StatusLog`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLog {
    /// `verified` — `Boolean`.
    pub verified: bool,
    /// `refusal` — `Optional<loom.runlog.RunLogRefusal>`.
    pub refusal: Option<crate::runlog::RunLogRefusal>,
    /// `line` — `Optional<Integer>`.
    pub line: Option<i64>,
    /// `detail` — `Optional<String>`.
    pub detail: Option<String>,
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

/// StepKind — `loom.tree.StepKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// `open`.
    Open,
    /// `run`.
    Run,
    /// `approve`.
    Approve,
    /// `close`.
    Close,
    /// `decide`.
    Decide,
    /// `stop`.
    Stop,
    /// `resume`.
    Resume,
    /// `allow`.
    Allow,
    /// `reopen`.
    Reopen,
    /// `replay`.
    Replay,
    /// `status`.
    Status,
    /// `verify`.
    Verify,
}

/// StopExpectation — `loom.tree.StopExpectation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopExpectation {
    /// `stop` — `loom.tree.TreeStop`.
    pub stop: TreeStop,
    /// `case` — `Optional<String>`.
    pub case: Option<String>,
}

/// SuiteCheck — `loom.tree.SuiteCheck`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteCheck {
    /// `check` — `loom.tree.CheckKind`.
    pub check: CheckKind,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `select` — `List<loom.tree.SelectItem>`.
    pub select: Vec<SelectItem>,
    /// `is` — `Optional<Json>`.
    pub is: Option<crate::json::Value>,
    /// `min` — `Optional<Integer>`.
    pub min: Option<i64>,
    /// `max` — `Optional<Integer>`.
    pub max: Option<i64>,
    /// `regex` — `Optional<String>`.
    pub regex: Option<String>,
    /// `where` — `Optional<loom.tree.Conditions>`.
    pub r#where: Option<Conditions>,
    /// `starts` — `Optional<List<Json>>`.
    pub starts: Option<Vec<crate::json::Value>>,
    /// `ends` — `Optional<List<Json>>`.
    pub ends: Option<Vec<crate::json::Value>>,
    /// `within` — `Optional<List<Json>>`.
    pub within: Option<Vec<crate::json::Value>>,
    /// `first` — `Optional<loom.tree.Conditions>`.
    pub first: Option<Conditions>,
    /// `then` — `Optional<loom.tree.Conditions>`.
    pub then: Option<Conditions>,
    /// `between_none` — `Optional<loom.tree.Conditions>`.
    pub between_none: Option<Conditions>,
    /// `left` — `Optional<loom.tree.Operand>`.
    pub left: Option<Operand>,
    /// `op` — `Optional<loom.tree.CompareOp>`.
    pub op: Option<CompareOp>,
    /// `right` — `Optional<loom.tree.Operand>`.
    pub right: Option<Operand>,
}

/// SuiteCycle — `loom.tree.SuiteCycle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteCycle {
    /// `name` — `String`.
    pub name: String,
    /// `pending` — `Boolean`.
    pub pending: bool,
    /// `when` — `List<loom.tree.CycleCondition>`.
    pub when: Vec<CycleCondition>,
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
    /// `bindings` — `Optional<String>`.
    pub bindings: Option<String>,
    /// `engine` — `Optional<String>`.
    pub engine: Option<String>,
    /// `fixtures` — `Optional<String>`.
    pub fixtures: Option<String>,
    /// `revisions` — `List<loom.tree.ArtifactRevision>`.
    pub revisions: Vec<ArtifactRevision>,
    /// `seed` — `List<loom.tree.SeedEntry>`.
    pub seed: Vec<SeedEntry>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `max_rounds` — `Integer`.
    pub max_rounds: i64,
    /// `decide` — `List<loom.tree.ScriptedDecision>`.
    pub decide: Vec<ScriptedDecision>,
    /// `steps` — `List<loom.tree.CycleStep>`.
    pub steps: Vec<CycleStep>,
    /// `expect` — `loom.tree.CycleExpectation`.
    pub expect: CycleExpectation,
}

/// SuiteDefaults — `loom.tree.SuiteDefaults`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteDefaults {
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
    /// `bindings` — `Optional<String>`.
    pub bindings: Option<String>,
    /// `engine` — `Optional<String>`.
    pub engine: Option<String>,
    /// `fixtures` — `Optional<String>`.
    pub fixtures: Option<String>,
    /// `revisions` — `Optional<List<loom.tree.ArtifactRevision>>`.
    pub revisions: Option<Vec<ArtifactRevision>>,
    /// `seed` — `Optional<List<loom.tree.SeedEntry>>`.
    pub seed: Option<Vec<SeedEntry>>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `max_rounds` — `Optional<Integer>`.
    pub max_rounds: Option<i64>,
    /// `decide` — `Optional<List<loom.tree.ScriptedDecision>>`.
    pub decide: Option<Vec<ScriptedDecision>>,
    /// `steps` — `Optional<List<loom.tree.CycleStep>>`.
    pub steps: Option<Vec<CycleStep>>,
}

/// SuiteRefusal — `loom.tree.SuiteRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteRefusal {
    /// `reason` — `loom.tree.SuiteRefusalReason`.
    pub reason: SuiteRefusalReason,
    /// `member` — `Optional<String>`.
    pub member: Option<String>,
    /// `detail` — `String`.
    pub detail: String,
}

/// SuiteRefusalReason — `loom.tree.SuiteRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteRefusalReason {
    /// `SuiteUnreadable`.
    SuiteUnreadable,
    /// `MemberUndeclared`.
    MemberUndeclared,
    /// `MemberMissing`.
    MemberMissing,
    /// `PathOutsideSuite`.
    PathOutsideSuite,
    /// `PathOutsideCycle`.
    PathOutsideCycle,
    /// `CycleNameUnusable`.
    CycleNameUnusable,
    /// `CycleTwice`.
    CycleTwice,
    /// `StepNameTwice`.
    StepNameTwice,
    /// `AfterUnknownStep`.
    AfterUnknownStep,
    /// `PathUnusable`.
    PathUnusable,
    /// `RegexUnusable`.
    RegexUnusable,
    /// `SelectUnusable`.
    SelectUnusable,
    /// `StepKindUnknown`.
    StepKindUnknown,
    /// `SourceUnknown`.
    SourceUnknown,
    /// `EventKindUnknown`.
    EventKindUnknown,
    /// `ProtocolsUnreadable`.
    ProtocolsUnreadable,
    /// `ScratchNotEmpty`.
    ScratchNotEmpty,
    /// `SeedUnusable`.
    SeedUnusable,
}

/// SuiteReport — `loom.tree.SuiteReport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteReport {
    /// `cycles` — `List<loom.tree.CycleResult>`.
    pub cycles: Vec<CycleResult>,
    /// `expected` — `Integer`.
    pub expected: i64,
    /// `as_expected` — `Integer`.
    pub as_expected: i64,
    /// `failed` — `List<String>`.
    pub failed: Vec<String>,
    /// `pending` — `Integer`.
    pub pending: i64,
    /// `pending_passing` — `List<String>`.
    pub pending_passing: Vec<String>,
    /// `waiting` — `List<loom.tree.WaitingCycle>`.
    pub waiting: Vec<WaitingCycle>,
    /// `not_run` — `List<String>`.
    pub not_run: Vec<String>,
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

/// TreeSuite — `loom.tree.TreeSuite`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSuite {
    /// `format` — `String`.
    pub format: String,
    /// `defaults` — `Optional<loom.tree.SuiteDefaults>`.
    pub defaults: Option<SuiteDefaults>,
    /// `cycles` — `List<loom.tree.SuiteCycle>`.
    pub cycles: Vec<SuiteCycle>,
}

/// TreeVerify — `loom.tree.TreeVerify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeVerify {
    /// `cases` — `Integer`.
    pub cases: i64,
    /// `failure` — `Optional<loom.tree.LogFailure>`.
    pub failure: Option<LogFailure>,
}

/// UseBound — `loom.tree.UseBound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseBound {
    /// `count` — `Optional<Integer>`.
    pub count: Option<i64>,
    /// `any` — `Boolean`.
    pub any: bool,
}

/// ValueType — `loom.tree.ValueType`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// `string`.
    String,
    /// `number`.
    Number,
    /// `bool`.
    Bool,
    /// `list`.
    List,
    /// `map`.
    Map,
    /// `null`.
    Null,
}

/// WaitingCycle — `loom.tree.WaitingCycle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingCycle {
    /// `name` — `String`.
    pub name: String,
    /// `story` — `String`.
    pub story: String,
    /// `kind` — `String`.
    pub kind: String,
}
