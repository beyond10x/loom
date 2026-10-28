//! An [`AgentExecutor`] of Commission over a metaharness session: the session works in a scratch
//! copy of a workspace, never in the workspace itself, and the executor returns what it changed
//! there as a proposal ([`Proposal`]); it performs nothing. Commission revalidates the proposal
//! against the case's revision, and the embedder's edit effect applies it, or refuses it, under
//! its own guard.
//!
//! One executor, the harness its [`Kind`]: the same brief, the same scratch copy, the same
//! proposal for each. The copy is the session's working directory, declared as the embedder's
//! scratch copy (`with_cwd` and `with_scratch_cwd`); the child's environment is metaharness's,
//! constructed from an allowlist and never inherited, so no variable of the embedder's reaches
//! it but those metaharness names.
//!
//! | kind | harness | hermetic | calls |
//! |---|---|---|---|
//! | `Claude` | Claude Code | `Strict` | decided here (`Ask`): the file tools of [`ToolPolicyDefaults::claude_default`] on paths inside the copy, and no other offered (`--tools`); restricted, auto-memory and the cross-session inbox off ([`CLAUDE_ARGUMENTS`]) |
//! | `Codex` | Codex (`codex exec`) | `On`, a gap holds the proposal | decided here (`Ask`): `Bash` and `apply_patch` ([`ToolPolicyDefaults::codex_default`]) naming no path outside the copy; its own sandbox (`workspace-write` on the copy, no network) confines what a command writes; its home is its own ([`HOME_VARIABLES`]) |
//! | `B10x` | b10x-harness | `On`, a gap holds the proposal | observed, never decided: metaharness's b10x adapter decides no call; the loop's toolset is what substrate confines to the copy |
//!
//! # One call, one session
//!
//! Each [`AgentExecutor::run_with_usage`] call is one session of the `loom.session` domain
//! (`ess/domains/session.yaml`): the executor mints a new [`SessionId`](model::session::SessionId)
//! for it and, when the session proposes a change, records it as one `AgentSession` through the
//! generated `ProposeSessionChange` behaviour, naming the commission's and the frontier's real ids
//! and the vendor's own session id where the harness stated one ([`SessionExecutor::sessions`],
//! [`SessionExecutor::proposed`]). The proposal's arguments are `{files}` only; the session's
//! summary and the SHA-256 digest of its transcript travel on the call's `ExecutorStep`, which the
//! runtime delivers as the step's observation and never as evidence, so two sessions that propose
//! the same files are the same request. The step's `usage` is what the session reported it spent,
//! charged to the run and its budget: the total its terminal record states, or for a
//! b10x-harness session that states none the sum of its per-call usage, never both.
//!
//! The session's record is also projected to `trace-ir/1` by metaharness and summarized
//! (`trace-ir-summary/1`) while it is still in the run's scratch, as the report's
//! [`SessionTrace`] ([`SessionReport::trace`]): the vendor record through its kind's production
//! reader when metaharness kept one, else the events the executor received. Its digest is the
//! step's. A record metaharness does not project keeps its digest and says why, and changes
//! nothing about the call's outcome; a session that never started has no trace.
//!
//! Metaharness decides each tool call of a session and records it (`tool.decided`). The report
//! reads those decisions back ([`SessionReport::tool_decisions`], `loom.session.ToolDecision`):
//! each call's id, its tool, the decision, a denial's reason and who decided it, in record order,
//! from the `trace-ir/1` document of the events the executor received, never from a vendor
//! record, which holds none of metaharness's decisions. They are known only when that document
//! carries metaharness's decision census and they add up to it; otherwise the report says they
//! are unknown, never that there were none. Loom makes none of these decisions beyond its answer
//! to `tool.decide`, and the decisions are never evidence and never in a proposal.
//!
//! A session may continue an earlier one of the same commission
//! ([`SessionExecutor::continuing`]): the next session the executor records names it as
//! `loom.session.AgentSession.continues`. The lineage is recorded only: the request to
//! metaharness is the one a fresh session gets, and the harness session itself is not resumed.
//! A predecessor of another commission than the call's is refused before any copy or session.
//!
//! A session that fails proposes nothing and is no record: the call returns `Suspended` with
//! `ExternalAvailability` naming why. A session that changed nothing proposes an empty list of
//! files.
//!
//! The change is what the session made in its copy against the bytes the copy was made from, the
//! workspace as the frontier was handed: a file written in the workspace while the session runs
//! is never proposed back. When the session ends, each file it changed is read again in the
//! workspace, and if any is no longer what the copy was made from (its bytes differ, it is gone,
//! or a file the session added now exists there), or a link of the workspace now reaches a named
//! path that none reached when the copy was made, the call proposes nothing and returns
//! `Suspended` with `ExternalAvailability` naming those paths, so a stale change never overwrites
//! a concurrent write. A write to such a file between the session's end and the edit effect
//! applying the proposal is not seen here; that window is the effect's own guard to close.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended,
    ExecutorStep, ExecutorUsage, Frontier, ProposedActionArguments, SuspensionReason,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use loom::agent::{AgentId, AgentSpec, DecisionMode as PolicyDecisions, ToolPolicy};
use loom::behaviour::{AgentSessionStorage, Generated};
use loom::models::ModelRole;
use loom::obligation::UnmetObligation;
use loom::primitives::Uuid;
use loom::session::obligations::{AgentSessionsQuery as _, ProposeSessionChangeBehavior as _};
use loom::session::{
    AgentSessionSnapshot, AgentSessions, CallDecision, ChangedFile, CommissionId, DecidedBy,
    FrontierId, ProposeSessionChange, ProposeSessionChangeOutcome, SessionChangeProposed,
    SessionId, SessionKind, ToolDecision,
};
use metaharness::protocol::{
    Command, Decision, DecisionMode, Event, HermeticMode, HermeticRow, RunSpec, SeamFactory,
    Severity, Usage, Verdict,
};
use metaharness::{ClaudeSeams, CodexSeams, EventPoll, Input, Metaharness};
use sha2::{Digest as _, Sha256};

/// Loom's generated model: the `loom.session` domain this executor records its sessions in.
pub use loom as model;
pub use metaharness;
pub use metaharness::ProcessRunner;
pub use metaharness::protocol::{CredentialSource, Kind};

/// The native tools a Claude Code session may use, on paths inside its copy: read, edit, write,
/// list and search the files, and nothing else.
#[deprecated(
    since = "0.36.0",
    note = "use `ToolPolicy::claude_default().allow` (`ToolPolicyDefaults`); removed in the release after 0.36.0"
)]
pub const TOOLS: [&str; 5] = CLAUDE_FILE_TOOLS;

/// The tools a Codex session may use, as its `PreToolUse` hook names them: the shell, to read
/// the copy, and `apply_patch`, to change it.
#[deprecated(
    since = "0.36.0",
    note = "use `ToolPolicy::codex_default().allow` (`ToolPolicyDefaults`); removed in the release after 0.36.0"
)]
pub const CODEX_TOOLS: [&str; 2] = CODEX_SHELL_TOOLS;

/// The file tools of [`ToolPolicyDefaults::claude_default`]: the only Claude Code tools whose
/// calls this executor can check ([`decide`]).
const CLAUDE_FILE_TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Glob", "Grep"];

/// The tools of [`ToolPolicyDefaults::codex_default`]: the only Codex tools whose calls this
/// executor can check ([`decide_codex`]).
const CODEX_SHELL_TOOLS: [&str; 2] = ["Bash", "apply_patch"];

/// The tool policies a session runs under when its agent spec does not say otherwise, as
/// associated functions of the generated `loom.agent.ToolPolicy` (`ToolPolicy::claude_default()`
/// with this trait in scope).
pub trait ToolPolicyDefaults {
    /// A Claude Code session's: the file tools `Read`, `Edit`, `Write`, `Glob` and `Grep` on
    /// paths inside the copy, nothing denied, no egress, every call decided here (`ask`).
    fn claude_default() -> Self;
    /// A Codex session's: `Bash`, to read the copy, and `apply_patch`, to change it, nothing
    /// denied, no egress, every call decided here (`ask`).
    fn codex_default() -> Self;
}

impl ToolPolicyDefaults for ToolPolicy {
    fn claude_default() -> Self {
        Self {
            allow: CLAUDE_FILE_TOOLS
                .iter()
                .map(|tool| (*tool).to_owned())
                .collect(),
            deny: Vec::new(),
            egress: Vec::new(),
            decisions: PolicyDecisions::Ask,
        }
    }

    fn codex_default() -> Self {
        Self {
            allow: CODEX_SHELL_TOOLS
                .iter()
                .map(|tool| (*tool).to_owned())
                .collect(),
            deny: Vec::new(),
            egress: Vec::new(),
            decisions: PolicyDecisions::Ask,
        }
    }
}

/// Why [`SessionExecutor::from_spec`] refused an agent spec; nothing was built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// The spec names no `session`, so it runs no agent session.
    NoSession {
        /// The spec's `agent.id`.
        agent: String,
    },
    /// The spec names a decision mode its session kind does not run under a policy Loom
    /// enforces: Claude Code and Codex only under `ask`, where this executor decides every call
    /// (under `frame` Loom supplies no frame, so every call is abstained on; under `observe`
    /// none is decided), and b10x-harness only under `observe`. metaharness 0.19.0 also runs
    /// b10x under `frame` and `ask` (ADR 0104 point 3), but Loom decides b10x calls with no
    /// rules of its own yet, so a spec that asks for either is refused here.
    DecisionModeUnenforced {
        /// The spec's `agent.id`.
        agent: String,
        /// The session kind, as the spec names it.
        kind: SessionKind,
        /// The decision mode the spec names.
        mode: PolicyDecisions,
    },
    /// The spec denies tools its session kind cannot withhold: a b10x-harness session's calls
    /// are observed and never decided, so no tool can be denied it.
    DenyUnenforced {
        /// The spec's `agent.id`.
        agent: String,
        /// The session kind, as the spec names it.
        kind: SessionKind,
        /// The tools denied.
        deny: Vec<String>,
    },
    /// The spec allows tools other than its session kind's default, which the kind cannot be
    /// narrowed or widened to: a b10x-harness session runs its loop's own toolset, observed.
    AllowUnenforced {
        /// The spec's `agent.id`.
        agent: String,
        /// The session kind, as the spec names it.
        kind: SessionKind,
        /// The tools allowed.
        allow: Vec<String>,
    },
    /// An `allow` or `deny` entry holds a comma or whitespace, so it is no one tool name: the
    /// offered tools are handed to the session joined by commas.
    ToolNameInvalid {
        /// The spec's `agent.id`.
        agent: String,
        /// The entry, as the spec writes it.
        tool: String,
    },
    /// The spec names a session override ([`SessionOverrides`]) its kind's launcher does not
    /// apply, so it is refused rather than dropped: `max_budget_micro_usd` on Codex (metaharness
    /// refuses `--max-budget-usd` there) and on b10x-harness, and `effort` on b10x-harness
    /// (nothing in its launch reads either).
    OverrideUnapplied {
        /// The spec's `agent.id`.
        agent: String,
        /// The session kind, as the spec names it.
        kind: SessionKind,
        /// The member, as `loom.agent.SessionSpec` names it.
        member: String,
    },
    /// The spec's `max_budget_micro_usd` is negative.
    BudgetNegative {
        /// The spec's `agent.id`.
        agent: String,
        /// The budget, as the spec writes it.
        budget: i64,
    },
    /// A `context` entry is not one of the files the executor copies, spelled as one plain
    /// relative path ([`plain_relative`]).
    ContextNotCopied {
        /// The spec's `agent.id`.
        agent: String,
        /// The entry, as the spec writes it.
        path: String,
    },
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSession { agent } => {
                write!(f, "the agent spec {agent} names no session to run")
            }
            Self::DecisionModeUnenforced { agent, kind, mode } => {
                let runs = match kind {
                    SessionKind::Harness => "Observe",
                    SessionKind::ClaudeCode | SessionKind::Codex => "Ask",
                };
                write!(
                    f,
                    "the agent spec {agent} runs a {kind:?} session under `decisions: {mode:?}`, \
                     which Loom does not enforce; a {kind:?} session runs only under \
                     `decisions: {runs}`"
                )
            }
            Self::DenyUnenforced { agent, kind, deny } => write!(
                f,
                "the agent spec {agent} denies {deny:?}, which a {kind:?} session cannot \
                 withhold: its calls are observed and never decided; leave them out of `deny`"
            ),
            Self::AllowUnenforced { agent, kind, allow } => write!(
                f,
                "the agent spec {agent} allows {allow:?}, but a {kind:?} session runs its own \
                 toolset and cannot be narrowed to a list; leave `allow` empty"
            ),
            Self::ToolNameInvalid { agent, tool } => write!(
                f,
                "the agent spec {agent} names the tool {tool:?}, which holds a comma or \
                 whitespace and so is no one tool name"
            ),
            Self::OverrideUnapplied {
                agent,
                kind,
                member,
            } => write!(
                f,
                "the agent spec {agent} names the session's `{member}`, which a {kind:?} \
                 session's launcher does not apply; leave it unset"
            ),
            Self::BudgetNegative { agent, budget } => write!(
                f,
                "the agent spec {agent} names `max_budget_micro_usd: {budget}`, which is \
                 negative"
            ),
            Self::ContextNotCopied { agent, path } => write!(
                f,
                "the agent spec {agent} names the context file {path:?}, which is not one of \
                 the files the session's copy holds, spelled as one plain relative path"
            ),
        }
    }
}

/// What a caller asks of a session's harness over its kind's defaults
/// (`story:session-executor-partial-runspec`): each member it names wins, each it leaves unset
/// keeps the kind's value. It has no member for what Loom sets for its own guarantees
/// (credentials, working directory, scratch copy, hermetic mode, decision mode, b10x-harness's
/// endpoint and secret route), so a caller cannot name those. A member the kind's launcher does
/// not apply is refused before the copy is made, never dropped: `max_budget_micro_usd` on Codex
/// and b10x-harness, `effort` on b10x-harness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOverrides {
    /// The reasoning effort, in the vendor's vocabulary: Claude Code's `--effort`, Codex's
    /// `model_reasoning_effort` in its scratch config.
    pub effort: Option<String>,
    /// A ceiling on what the session may spend, in micro-USD, which the vendor enforces during
    /// the run: Claude Code's `--max-budget-usd`, rendered exactly as a decimal of dollars
    /// (`12500000` is `12.5`).
    pub max_budget_micro_usd: Option<u64>,
    /// Whether metaharness refuses the session before it starts when the harness program is
    /// outside its adapter's pinned versions; none for the default, which warns.
    pub strict_version: Option<bool>,
    /// The workspace files handed to the harness as context, in order: each one of the files the
    /// executor copies, spelled as one plain relative path ([`plain_relative`]), and handed from
    /// the copy. Any other path is refused before the copy, naming it.
    pub context: Vec<String>,
}

impl SessionOverrides {
    /// The member `kind`'s launcher does not apply, by its `loom.agent.SessionSpec` name, when
    /// these name one.
    fn unapplied(&self, kind: Kind) -> Option<&'static str> {
        match kind {
            Kind::Claude => None,
            Kind::Codex => self
                .max_budget_micro_usd
                .is_some()
                .then_some("max_budget_micro_usd"),
            Kind::B10x if self.max_budget_micro_usd.is_some() => Some("max_budget_micro_usd"),
            Kind::B10x => self.effort.is_some().then_some("effort"),
        }
    }

    /// The first context path that is not one of `files`, spelled as one plain relative path.
    fn uncopied<'a>(&'a self, files: &[String]) -> Option<&'a String> {
        self.context
            .iter()
            .find(|path| !plain_relative(path) || !files.contains(path))
    }
}

/// `micro_usd` as the decimal of US dollars metaharness passes to the vendor, exactly: no
/// trailing zero in the fraction, and no fraction for whole dollars (`12500000` is `12.5`, `1`
/// is `0.000001`, `5000000` is `5`).
fn usd(micro_usd: u64) -> String {
    let (dollars, fraction) = (micro_usd / 1_000_000, micro_usd % 1_000_000);
    if fraction == 0 {
        return dollars.to_string();
    }
    let fraction = format!("{fraction:06}");
    format!("{dollars}.{}", fraction.trim_end_matches('0'))
}

/// The harness `kind` as a reader names it.
fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Claude => "Claude Code",
        Kind::Codex => "Codex",
        Kind::B10x => "b10x-harness",
    }
}

impl std::error::Error for SpecError {}

/// The model endpoint a `Kind::B10x` session's loop is pointed at, and the subscription token
/// it reads there: the loop reads the file itself, and metaharness copies nothing. On the
/// secret route ([`SessionExecutor::api_key_secret`]) it names no token: `token_file` empty,
/// `token_pointer` none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The endpoint origin plus its API prefix.
    pub base_url: String,
    /// The model API it speaks: `anthropic-messages` or `openai-responses`.
    pub wire: String,
    /// The model the endpoint serves.
    pub model: String,
    /// The file holding the subscription token.
    pub token_file: PathBuf,
    /// A JSON pointer to the token inside that file.
    pub token_pointer: Option<String>,
}

/// One file a proposal changes: its path relative to the workspace, and its whole content after
/// the change, absent when the change removes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedFile {
    pub path: String,
    pub content: Option<String>,
}

/// What a session proposes: the one action it names, one line on what it did, and every file
/// of its copy it changed, added or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub action: String,
    /// The session's own line on what it did: on the call's `ExecutorStep`, never in the
    /// outcome's arguments.
    pub summary: String,
    pub files: Vec<ProposedFile>,
}

impl Proposal {
    /// The proposal as the executor outcome that carries it: `ProposedAction` with the action
    /// and, as its arguments, `{"files": [{"path": …, "content": …}]}` only, a removed file's
    /// `content` null. The summary is not in them, so two proposals of the same files are the
    /// same request whatever each session said about it.
    #[must_use]
    pub fn to_outcome(&self) -> ExecutorOutcome {
        let files = self
            .files
            .iter()
            .map(|file| {
                Json::Object(vec![
                    ("path".to_owned(), Json::Text(file.path.clone())),
                    (
                        "content".to_owned(),
                        file.content.clone().map_or(Json::Null, Json::Text),
                    ),
                ])
            })
            .collect();
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: self.action.clone(),
            arguments: ProposedActionArguments(Json::Object(vec![(
                "files".to_owned(),
                Json::Array(files),
            )])),
        })
    }

    /// The proposal an outcome carries, its summary empty because the arguments do not carry
    /// one: none for an outcome that proposes no action; an error naming what does not read for
    /// a `ProposedAction` whose arguments are not a proposal's.
    pub fn from_outcome(outcome: &ExecutorOutcome) -> Option<Result<Self, String>> {
        let ExecutorOutcome::ProposedAction(proposed) = outcome else {
            return None;
        };
        let arguments = &proposed.arguments.0;
        let read = || -> Result<Self, String> {
            let Some(Json::Array(items)) = arguments.member("files") else {
                return Err("the proposal names no list of files".to_owned());
            };
            let mut files = Vec::new();
            for item in items {
                let Some(Json::Text(path)) = item.member("path") else {
                    return Err("a proposed file names no path".to_owned());
                };
                let content = match item.member("content") {
                    Some(Json::Text(text)) => Some(text.clone()),
                    None | Some(Json::Null) => None,
                    Some(other) => {
                        return Err(format!("the content of {path} is {}", other.describes()));
                    }
                };
                files.push(ProposedFile {
                    path: path.clone(),
                    content,
                });
            }
            Ok(Self {
                action: proposed.action.clone(),
                summary: String::new(),
                files,
            })
        };
        Some(read())
    }
}

/// What the last session reported, each field absent when it did not state it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionReport {
    /// The harness program's version, as its opening record stated it.
    pub version: Option<String>,
    /// The models its terminal record named, joined with `+`, else the opening record's.
    pub model: Option<String>,
    /// The vendor's own id of the session, as its opening record stated it.
    pub vendor_session_id: Option<String>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub api_ms: Option<u64>,
    pub turns: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_creation: Option<u64>,
    /// The directory the session worked in, the scratch copy; removed when the call returns.
    pub cwd: Option<PathBuf>,
    /// Every event metaharness delivered, one JSON line each.
    pub transcript: Vec<u8>,
    /// Each gating row of metaharness's hermetic floor that was not `ok`, as
    /// `<row> <verdict>: <detail>`.
    pub floor: Vec<String>,
    /// Every row of the floor, gating or advisory, as `<row> <verdict>`, in metaharness's
    /// order: the hermetic verdict whole.
    pub rows: Vec<String>,
    /// Why the call proposed nothing, when it did not.
    pub error: Option<String>,
    /// Whether metaharness started the harness program. A call that failed before it did (the
    /// copy, the spec, or a program that could not be started) made no model call.
    pub started: bool,
    /// The session's record, projected to `trace-ir/1` and summarized ([`SessionTrace`]); none
    /// when no session started.
    pub trace: Option<SessionTrace>,
    /// Each tool call's decision and who made it, in the order the session's record holds them
    /// (`loom.session.ToolDecision`), read from the `trace-ir/1` document of the events
    /// metaharness delivered ([`tool_decisions`]). None when they are unknown: the record holds
    /// no decision census, its decisions do not add up to the census, or no session started. An
    /// empty list is a census of no decision. Never evidence.
    pub tool_decisions: Option<Vec<ToolDecision>>,
}

/// A session's record as metaharness projects it: the `trace-ir/1` document named by its digest
/// and source, and its `trace-ir-summary/1` (`metaharness::protocol::summarize`), or why the
/// record does not project. The summary is a projection of what the session's record says, never
/// evidence, and a record that does not project fails nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionTrace {
    /// The SHA-256 digest of the session's record, lowercase hex: the `trace-ir/1` document's
    /// `transcript_digest`, and the step's. None when the session left no record.
    pub transcript_digest: Option<String>,
    /// The format the record was read as: the vendor record's (`claude-code-stream-json`,
    /// `codex-rollout`, `harness-json`) when metaharness kept one, else `metaharness.event/1`, the
    /// events the executor received. None when the session left no record.
    pub source: Option<String>,
    /// The record's summary, or why metaharness did not project it.
    pub summary: Result<metaharness::protocol::TraceIrSummary, String>,
}

/// The executor: one call runs one session over a scratch copy of `workspace` holding the
/// files `files` names, with `brief` as its prompt, and proposes what it changed there.
pub struct SessionExecutor {
    /// The harness the session runs: `Kind::Claude` by default.
    pub kind: Kind,
    /// The workspace the copy is made from; never written.
    pub workspace: PathBuf,
    /// The files copied, by their paths relative to the workspace, each spelled as one plain
    /// relative path ([`plain_relative`]); a call naming any other spelling fails before the copy
    /// is made.
    pub files: Vec<String>,
    /// The prompt the session is given, whole.
    pub brief: String,
    /// The model, when given.
    pub model: Option<String>,
    /// The most turns the session may take.
    pub max_turns: Option<u32>,
    /// Where the session's credential comes from: the operator's login by default.
    pub credentials: CredentialSource,
    /// The directory the scratch copy is made in, outside the workspace.
    pub scratch: PathBuf,
    /// How long the session may run; it is halted past it.
    pub timeout: Option<Duration>,
    /// The harness program the session runs (`claude`, `codex`, `b10x-harness`), by path; none
    /// for the one metaharness finds on the `PATH` it gives its child.
    pub program: Option<PathBuf>,
    /// For `Kind::B10x`, the endpoint its loop is pointed at; the session is refused without one.
    pub endpoint: Option<Endpoint>,
    /// The tool policy the session runs under (`loom.agent.ToolPolicy`): the tools it is offered
    /// (`allow` less `deny`), and who decides each call (`decisions`), handed to metaharness
    /// as the session's decision mode. None for its kind's default: Claude Code's
    /// [`ToolPolicyDefaults::claude_default`], Codex's [`ToolPolicyDefaults::codex_default`],
    /// and for b10x-harness none offered by name and every call observed. Its `egress` is not
    /// applied here.
    pub tools: Option<ToolPolicy>,
    /// The agent whose spec the executor was built from ([`Self::from_spec`]), recorded on each
    /// session as `loom.session.AgentSession.agent`; none for an executor built without one.
    pub agent: Option<AgentId>,
    /// For `Kind::B10x`, the name of [`Self::endpoint`]'s API key in the operator's secret store,
    /// as `<namespace>/<name>`: metaharness runs the session under `CredentialSource::ApiKey`
    /// and launches the loop with `--api-key-secret <name>`, and the loop opens the store itself,
    /// so the key never enters this process, metaharness or the child's environment. The endpoint
    /// then names no subscription token (`token_file` empty, `token_pointer` none). A call with a
    /// name on another kind, beside a token, or not of that shape fails before the copy is made,
    /// as does a b10x-harness call naming neither a name nor a token file.
    pub api_key_secret: Option<String>,
    /// What the caller asks of the session's harness over its kind's defaults
    /// ([`SessionOverrides`], [`Self::with_overrides`]); none named by default. A call naming a
    /// member its kind's launcher does not apply, or a context file the copy does not hold,
    /// fails before the copy is made.
    pub overrides: SessionOverrides,
    runner: RefCell<Option<Box<dyn ProcessRunner>>>,
    last: RefCell<Option<SessionReport>>,
    ids: RefCell<Box<dyn FnMut() -> SessionId>>,
    record: RefCell<SessionRecord>,
    predecessor: RefCell<Option<Predecessor>>,
}

/// The earlier session the next recorded session continues ([`SessionExecutor::continuing`]):
/// its id and the commission it ran for.
#[derive(Debug, Clone)]
struct Predecessor {
    session_id: SessionId,
    commission_id: CommissionId,
}

impl std::fmt::Debug for SessionExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionExecutor")
            .field("kind", &self.kind)
            .field("workspace", &self.workspace)
            .field("files", &self.files.len())
            .field("model", &self.model)
            .field("scratch", &self.scratch)
            .finish_non_exhaustive()
    }
}

impl SessionExecutor {
    /// An executor over `workspace`'s `files` with `brief` as the prompt, its copy made under
    /// the system's temporary directory, on the operator's login.
    #[must_use]
    pub fn new(
        workspace: impl Into<PathBuf>,
        files: Vec<String>,
        brief: impl Into<String>,
    ) -> Self {
        Self {
            kind: Kind::Claude,
            workspace: workspace.into(),
            files,
            brief: brief.into(),
            model: None,
            max_turns: None,
            credentials: CredentialSource::OperatorLogin,
            scratch: std::env::temp_dir(),
            timeout: None,
            program: None,
            endpoint: None,
            tools: None,
            agent: None,
            api_key_secret: None,
            overrides: SessionOverrides::default(),
            runner: RefCell::new(None),
            last: RefCell::new(None),
            ids: RefCell::new(Box::new(fresh_session_id)),
            record: RefCell::new(SessionRecord::default()),
            predecessor: RefCell::new(None),
        }
    }

    /// An executor over `workspace`'s `files` built from an agent's data (`loom.agent/1`):
    /// the session kind, the tool policy and the prompt (`agent.brief`) of `spec`, and as its
    /// model the name `spec.model` gives the `agent` role, as written (routes are the host's to
    /// resolve). Otherwise as [`Self::new`]: the copy under the system's temporary directory, on
    /// the operator's login. Each session it runs records the spec's `agent.id`. The session's
    /// `effort`, `max_budget_micro_usd`, `strict_version` and `context` become
    /// [`Self::overrides`]; a spec naming none builds the executor it built before them. The
    /// spec's run budget, stop policy and egress apertures are not applied here; a host bounds
    /// the run through Commission.
    ///
    /// # Errors
    ///
    /// It builds an executor only for a policy Loom enforces itself, and refuses any other spec
    /// before any session: [`SpecError::NoSession`] for a spec that runs no session;
    /// [`SpecError::ToolNameInvalid`] for an `allow` or `deny` entry holding a comma or
    /// whitespace; [`SpecError::DecisionModeUnenforced`] for Claude Code or Codex under any mode
    /// but `ask`, and b10x-harness under any but `observe`; and for b10x-harness,
    /// [`SpecError::AllowUnenforced`] for an `allow` other than its default (none) and
    /// [`SpecError::DenyUnenforced`] for any `deny`. Then the overrides:
    /// [`SpecError::BudgetNegative`] for a negative `max_budget_micro_usd`;
    /// [`SpecError::OverrideUnapplied`] for a member the kind's launcher does not apply (a budget
    /// on Codex or b10x-harness, an effort on b10x-harness); and [`SpecError::ContextNotCopied`]
    /// for a `context` entry that is not one of `files` as one plain relative path.
    pub fn from_spec(
        spec: &AgentSpec,
        workspace: impl Into<PathBuf>,
        files: Vec<String>,
    ) -> Result<Self, SpecError> {
        let agent = spec.agent.id.clone();
        let Some(session) = &spec.session else {
            return Err(SpecError::NoSession { agent: agent.0 });
        };
        let tools = &session.tools;
        if let Some(tool) = tools
            .allow
            .iter()
            .chain(&tools.deny)
            .find(|tool| tool.contains(|c: char| c == ',' || c.is_whitespace()))
        {
            return Err(SpecError::ToolNameInvalid {
                agent: agent.0,
                tool: tool.clone(),
            });
        }
        let (kind, enforced) = match session.kind {
            SessionKind::ClaudeCode => (Kind::Claude, PolicyDecisions::Ask),
            SessionKind::Codex => (Kind::Codex, PolicyDecisions::Ask),
            SessionKind::Harness => (Kind::B10x, PolicyDecisions::Observe),
        };
        if tools.decisions != enforced {
            return Err(SpecError::DecisionModeUnenforced {
                agent: agent.0,
                kind: session.kind,
                mode: tools.decisions,
            });
        }
        if kind == Kind::B10x {
            if !tools.allow.is_empty() {
                return Err(SpecError::AllowUnenforced {
                    agent: agent.0,
                    kind: session.kind,
                    allow: tools.allow.clone(),
                });
            }
            if !tools.deny.is_empty() {
                return Err(SpecError::DenyUnenforced {
                    agent: agent.0,
                    kind: session.kind,
                    deny: tools.deny.clone(),
                });
            }
        }
        let max_budget_micro_usd = session
            .max_budget_micro_usd
            .map(|budget| {
                u64::try_from(budget).map_err(|_| SpecError::BudgetNegative {
                    agent: agent.0.clone(),
                    budget,
                })
            })
            .transpose()?;
        let overrides = SessionOverrides {
            effort: session.effort.clone(),
            max_budget_micro_usd,
            strict_version: session.strict_version,
            context: session.context.clone(),
        };
        if let Some(member) = overrides.unapplied(kind) {
            return Err(SpecError::OverrideUnapplied {
                agent: agent.0,
                kind: session.kind,
                member: member.to_owned(),
            });
        }
        if let Some(path) = overrides.uncopied(&files) {
            return Err(SpecError::ContextNotCopied {
                agent: agent.0,
                path: path.clone(),
            });
        }
        let mut executor = Self::new(workspace, files, spec.agent.brief.clone())
            .of_kind(kind)
            .with_overrides(overrides);
        executor.model = spec
            .model
            .roles
            .iter()
            .find(|role| role.role == ModelRole::Agent)
            .map(|role| role.model.clone());
        executor.tools = Some(session.tools.clone());
        executor.agent = Some(agent);
        Ok(executor)
    }

    /// The tool policy the session runs under: [`Self::tools`], else its kind's default.
    #[must_use]
    pub fn policy(&self) -> ToolPolicy {
        self.tools.clone().unwrap_or_else(|| match self.kind {
            Kind::Claude => ToolPolicy::claude_default(),
            Kind::Codex => ToolPolicy::codex_default(),
            Kind::B10x => ToolPolicy {
                allow: Vec::new(),
                deny: Vec::new(),
                egress: Vec::new(),
                decisions: PolicyDecisions::Observe,
            },
        })
    }

    /// The tools the session is offered: the policy's `allow` less its `deny`, in `allow`'s
    /// order, each once. A Claude Code session is launched with these and no other; a call of a
    /// tool outside them is denied wherever this executor decides it.
    #[must_use]
    pub fn offered(&self) -> Vec<String> {
        let policy = self.policy();
        let mut offered: Vec<String> = Vec::new();
        for tool in policy.allow {
            if !policy.deny.contains(&tool) && !offered.contains(&tool) {
                offered.push(tool);
            }
        }
        offered
    }

    /// The same executor, each call's [`SessionId`] taken from `ids` instead of a fresh one
    /// ([`fresh_session_id`]): for a caller that assigns the ids itself, as a conformance run
    /// assigns the scenario's.
    #[must_use]
    pub fn with_session_ids(self, ids: impl FnMut() -> SessionId + 'static) -> Self {
        *self.ids.borrow_mut() = Box::new(ids);
        self
    }

    /// The same executor, its next recorded session continuing `previous`, a session this or
    /// another executor recorded ([`Self::sessions`]): that session names `previous.session_id`
    /// as `loom.session.AgentSession.continues`. The predecessor is the next recorded session's
    /// only; a later one continues none unless built again. A call that records no session (it
    /// failed or was refused) leaves the predecessor to the next call. The request to
    /// metaharness is unchanged and the harness session is not resumed: Loom records the
    /// lineage only.
    ///
    /// A call for another commission than `previous.commission_id` is refused before any copy is
    /// made or session started: it returns `Suspended` with `ExternalAvailability` naming the
    /// predecessor's session id and both commissions (compared as UUIDs, whatever their case),
    /// and records nothing. So is a call whose session id is the predecessor's own or one this
    /// executor already recorded, naming the id.
    #[must_use]
    pub fn continuing(self, previous: &AgentSessions) -> Self {
        *self.predecessor.borrow_mut() = Some(Predecessor {
            session_id: previous.session_id.clone(),
            commission_id: previous.commission_id.clone(),
        });
        self
    }

    /// The predecessor of the next session ([`Self::continuing`]), refused when it ran for
    /// another commission than `commission`. The two are compared as UUIDs, so a commission
    /// written in capitals is the same commission; the refusal names each as written.
    fn predecessor_for(&self, commission: &CommissionId) -> Result<Option<SessionId>, String> {
        match self.predecessor.borrow().as_ref() {
            None => Ok(None),
            Some(previous) if same_uuid(&previous.commission_id.0.0, &commission.0.0) => {
                Ok(Some(previous.session_id.clone()))
            }
            Some(previous) => Err(format!(
                "the session {} this one continues ran for the commission {}, not for this \
                 call's commission {}",
                previous.session_id.0.0, previous.commission_id.0.0, commission.0.0
            )),
        }
    }

    /// The id of the call's session, taken before any copy or session
    /// ([`Self::with_session_ids`]): refused, naming it, when it is the predecessor's own or this
    /// executor already recorded a session under it, so no session continues itself, no lineage
    /// of this executor loops and no recorded row is replaced. Ids are compared as UUIDs.
    fn session_id_for(&self, continues: Option<&SessionId>) -> Result<SessionId, String> {
        let session_id = (self.ids.borrow_mut())();
        let id = &session_id.0.0;
        if continues.is_some_and(|previous| same_uuid(&previous.0.0, id)) {
            return Err(format!(
                "the session id {id} is the id of the session this one continues"
            ));
        }
        if self
            .sessions()
            .iter()
            .any(|held| same_uuid(&held.session_id.0.0, id) || held.session_id.0.0 == *id)
        {
            return Err(format!(
                "the session id {id} names a session this executor already recorded"
            ));
        }
        Ok(session_id)
    }

    /// Every session this executor recorded, one row of the `loom.session.AgentSessions` view
    /// each, in the order they ran: a session that failed is not among them.
    #[must_use]
    pub fn sessions(&self) -> Vec<AgentSessions> {
        self.record.borrow().sessions()
    }

    /// The `loom.session.SessionChangeProposed` each recorded session published, in the order
    /// they ran.
    #[must_use]
    pub fn proposed(&self) -> Vec<SessionChangeProposed> {
        self.record.borrow().proposed.clone()
    }

    /// Records `proposal`, the change of the session `report` describes, as one `AgentSession`
    /// for `commission` over `frontier`, under `session_id` ([`Self::session_id_for`]), continuing `continues` when
    /// it names an earlier session.
    fn record(
        &self,
        commission: &CommissionId,
        frontier: &Frontier<frontier_state::Issued>,
        report: &SessionReport,
        proposal: &Proposal,
        session_id: SessionId,
        continues: Option<SessionId>,
    ) -> Result<SessionChangeProposed, String> {
        let kind = match self.kind {
            Kind::Claude => SessionKind::ClaudeCode,
            Kind::Codex => SessionKind::Codex,
            Kind::B10x => SessionKind::Harness,
        };
        self.record
            .borrow_mut()
            .propose(ProposeSessionChange {
                session_id,
                kind,
                commission_id: commission.clone(),
                frontier_id: FrontierId(Uuid(frontier.data().frontier_id.0.0.clone())),
                vendor_session_id: report.vendor_session_id.clone(),
                agent: self.agent.clone(),
                continues,
                action: proposal.action.clone(),
                files: proposal
                    .files
                    .iter()
                    .map(|file| ChangedFile {
                        path: file.path.clone(),
                        content: file.content.clone(),
                    })
                    .collect(),
            })
            .map_err(|unmet| format!("the session could not be recorded: {unmet}"))
    }

    /// The same executor, its session the harness `kind`.
    #[must_use]
    pub fn of_kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }

    /// The same executor, its sessions asking `overrides` of the harness over its kind's
    /// defaults ([`SessionOverrides`]): each member named wins, each left unset keeps the kind's
    /// value. Nothing else of the request is taken from the caller.
    #[must_use]
    pub fn with_overrides(mut self, overrides: SessionOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// [`Self::overrides`] are ones this executor applies: every member is one its kind's
    /// launcher applies, and every context file one of [`Self::files`], spelled as one plain
    /// relative path. Each refusal names the member and kind, or the path.
    fn check_overrides(&self) -> Result<(), String> {
        if let Some(member) = self.overrides.unapplied(self.kind) {
            return Err(format!(
                "the session override `{member}` is not applied by a {} session's launcher, so \
                 it is refused rather than dropped; leave it unset",
                kind_name(self.kind)
            ));
        }
        if let Some(path) = self.overrides.uncopied(&self.files) {
            return Err(format!(
                "the context file {path:?} is not one of the files the session's copy holds, \
                 spelled as one plain relative path"
            ));
        }
        Ok(())
    }

    /// The same executor, whose session is started by `runner` instead of the real program
    /// (metaharness's `ScriptedRunner`, or one that wraps it).
    #[must_use]
    pub fn with_runner(self, runner: Box<dyn ProcessRunner>) -> Self {
        *self.runner.borrow_mut() = Some(runner);
        self
    }

    /// What the last call's session reported; none before the first call.
    #[must_use]
    pub fn last_report(&self) -> Option<SessionReport> {
        self.last.borrow().clone()
    }

    /// The run metaharness is asked for, its decision mode the tool policy's ([`Self::policy`];
    /// by default every Claude Code and Codex call decided here, every b10x-harness call
    /// observed), and by kind: Claude Code `Strict`; Codex `On` (its strict floor fails on four rows its
    /// opening record does not carry, H1a, H1b, H4 and H5, which metaharness cannot decide for
    /// it yet); b10x-harness `On`, pointed at [`Self::endpoint`] with its subscription token, or
    /// with [`Self::api_key_secret`] under `ApiKey` when one is named, confined by substrate in the
    /// loop's own process so its write tools reach the copy
    /// and nothing else. Each with the copy as its declared scratch working directory. Then
    /// [`Self::overrides`], each member named over the kind's default: the effort, the budget as
    /// a decimal of dollars, strict version, and each context file by its path in the copy
    /// (relative, as metaharness reads it beneath the working directory).
    fn spec(&self, copy: &Path) -> Result<RunSpec, String> {
        let mut spec = RunSpec::new(self.kind);
        spec.credentials = self.credentials;
        spec.cwd = Some(copy.to_path_buf());
        spec.scratch_cwd = true;
        spec.prompt = Some(self.brief.clone());
        spec.model.clone_from(&self.model);
        let overrides = &self.overrides;
        spec.effort.clone_from(&overrides.effort);
        spec.max_budget_usd = overrides.max_budget_micro_usd.map(usd);
        if let Some(strict) = overrides.strict_version {
            spec.strict_version = strict;
        }
        spec.context = overrides.context.iter().map(PathBuf::from).collect();
        let decisions = match self.policy().decisions {
            PolicyDecisions::Frame => DecisionMode::Frame,
            PolicyDecisions::Ask => DecisionMode::Ask,
            PolicyDecisions::Observe => DecisionMode::Observe,
        };
        spec.decisions = decisions;
        match self.kind {
            Kind::Claude => {
                spec.hermetic = HermeticMode::Strict;
                spec.max_turns = self.max_turns;
            }
            Kind::Codex => {
                spec.hermetic = HermeticMode::On;
            }
            Kind::B10x => {
                let endpoint = self.endpoint.as_ref().ok_or(
                    "a b10x-harness session needs the endpoint its loop is pointed at; none given",
                )?;
                spec.hermetic = HermeticMode::On;
                spec.max_turns = self.max_turns;
                spec.model_endpoint = Some(endpoint.base_url.clone());
                spec.model_wire = Some(endpoint.wire.clone());
                spec.model = Some(self.model.clone().unwrap_or(endpoint.model.clone()));
                if let Some(name) = &self.api_key_secret {
                    // The loop opens the secret store by this name; metaharness holds the name
                    // and never the key.
                    spec.credentials = CredentialSource::ApiKey;
                    spec.api_key_secret = Some(name.clone());
                } else {
                    spec.credentials = CredentialSource::None;
                    spec.subscription_token_file = Some(endpoint.token_file.display().to_string());
                    spec.subscription_token_pointer
                        .clone_from(&endpoint.token_pointer);
                }
                spec.substrate_embedded = true;
            }
        }
        Ok(spec)
    }

    /// [`Self::api_key_secret`], when named, is for a b10x-harness session, is the endpoint's only
    /// credential, and has the shape `<namespace>/<name>` (metaharness's check, so the two cannot
    /// disagree); the value is quoted escaped. Without one, a b10x-harness endpoint names its
    /// token file: an empty one points the loop at no credential.
    fn check_api_key_secret(&self) -> Result<(), String> {
        let Some(name) = &self.api_key_secret else {
            if self.kind == Kind::B10x
                && let Some(endpoint) = &self.endpoint
                && endpoint.token_file.as_os_str().is_empty()
            {
                return Err(
                    "a b10x-harness session's endpoint names no token file and no API \
                            key secret is named; name one of the two"
                        .to_owned(),
                );
            }
            return Ok(());
        };
        let quoted = name.escape_debug();
        if self.kind != Kind::B10x {
            return Err(format!(
                "the API key secret `{quoted}` is for a b10x-harness session; a {:?} session has \
                 no secret route",
                self.kind
            ));
        }
        if let Some(endpoint) = &self.endpoint
            && (!endpoint.token_file.as_os_str().is_empty() || endpoint.token_pointer.is_some())
        {
            return Err(format!(
                "the API key secret `{quoted}` and the endpoint's subscription token are two \
                 credentials for one request; leave the endpoint's token_file empty and its \
                 token_pointer unset"
            ));
        }
        // No exec carries a NUL byte, and the loop reads no other control character in a name.
        if name.chars().any(char::is_control) {
            return Err(format!(
                "the API key secret `{quoted}` holds a control character"
            ));
        }
        metaharness_b10x::validate_api_key_secret(name)
    }

    /// One session: the copy made, the session run in it to its end, the proposal read.
    fn session(&self, report: &mut SessionReport) -> Result<Proposal, String> {
        // One spelling per file: the copy's base and the walk of the copy key a file by the
        // same string only when the embedder named it plainly.
        let unplain: Vec<&str> = self
            .files
            .iter()
            .map(String::as_str)
            .filter(|file| !plain_relative(file))
            .collect();
        if !unplain.is_empty() {
            return Err(format!(
                "the named files {unplain:?} are not plain paths relative to the workspace"
            ));
        }
        self.check_api_key_secret()?;
        self.check_overrides()?;
        let offered = self.offered();
        let copy = ScratchCopy::make(&self.scratch, &self.workspace, &self.files)?;
        report.cwd = Some(copy.dir.clone());
        let builder = Metaharness::from_spec(self.spec(&copy.dir)?);
        let mut seams: Box<dyn SeamFactory> = match self.kind {
            Kind::Claude => Box::new(ClaudeSeams),
            Kind::Codex => Box::new(CodexSeams),
            Kind::B10x => Box::new(metaharness_b10x::B10xSeams::new(None, None)),
        };
        // A Codex session's home is its own, in the scratch root ([`Homed`]).
        let home = match self.kind {
            Kind::Codex => Some(copy.home()?),
            _ => None,
        };
        // The program: by path when the embedder names one; Codex's otherwise by name, found on
        // the `PATH` metaharness gives its child (`$HOME/.local/bin` first) when it starts
        // ([`Named`]), and b10x-harness's by metaharness itself on that same `PATH`, because the
        // version H9 reads is that file's.
        let mut supplied = self.runner.borrow_mut();
        let mut named = match (supplied.is_some(), &self.program, self.kind) {
            (true, _, _) | (false, _, Kind::B10x) | (false, None, Kind::Claude) => None,
            (false, Some(program), kind) => Some(Named::new(kind, program.display().to_string())),
            (false, None, Kind::Codex) => Some(Named::new(Kind::Codex, "codex".to_owned())),
        };
        // Claude Code's with no program named: metaharness's own spawn, which `start` would use,
        // so that every Claude Code session starts through [`Restricted`].
        let mut spawn = metaharness::SpawnRunner::new();
        let runner: Option<&mut dyn ProcessRunner> = match (supplied.as_mut(), named.as_mut()) {
            (Some(runner), _) => Some(runner.as_mut()),
            (None, Some(named)) => Some(named),
            (None, None) if self.kind == Kind::Claude => Some(&mut spawn),
            (None, None) => None,
        };
        let started = match (runner, &home) {
            (Some(inner), Some(home)) => {
                let mut homed = Homed { home, inner };
                builder.start_with(Input::FromSpec, &mut homed, &mut *seams)
            }
            (Some(inner), None) if self.kind == Kind::Claude => {
                let mut restricted = Restricted {
                    inner,
                    offered: &offered,
                };
                builder.start_with(Input::FromSpec, &mut restricted, &mut *seams)
            }
            (Some(runner), None) => builder.start_with(Input::FromSpec, runner, &mut *seams),
            (None, _) => builder.start(Input::FromSpec),
        };
        drop(named);
        drop(supplied);
        let mut run =
            started.map_err(|refusal| format!("metaharness refused the session: {refusal}"))?;
        report.started = true;
        let deadline = self
            .timeout
            .and_then(|left| Instant::now().checked_add(left));
        let mut said = Vec::new();
        // The final answer the terminal record states, where it states one (Codex's).
        let mut final_text: Option<String> = None;
        let mut halted = false;
        let mut ended_in_error = None;
        // Whether every plugin the opening record names ships inside the vendor program
        // (`<name>@builtin`): none from a directory, a marketplace or a managed setting.
        let mut builtin_only = false;
        // The usage total the terminal record states, and, for b10x-harness, each call's
        // `usage` event: what the session is charged ([`charged_usage`]).
        let mut ended_usage: Option<Usage> = None;
        let mut usage_events: Vec<Usage> = Vec::new();
        let mut stream_error = None;
        loop {
            if !halted && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                halted = true;
                let _ = run.send(Command::Halt {
                    reason: "the step's time ran out".to_owned(),
                });
            }
            let line = match run.poll_event() {
                Ok(EventPoll::Event(line)) => line,
                Ok(EventPoll::Ended) => break,
                Ok(EventPoll::Idle | EventPoll::Progress | EventPoll::DecisionPending) => continue,
                Err(error) => {
                    stream_error = Some(format!("the session's stream: {error}"));
                    break;
                }
            };
            if let Ok(text) = serde_json::to_string(&*line) {
                report.transcript.extend_from_slice(text.as_bytes());
                report.transcript.push(b'\n');
            }
            match &line.event {
                Event::SessionStarted {
                    session_id,
                    harness_version,
                    model,
                    plugins,
                    ..
                } => {
                    report.vendor_session_id.clone_from(session_id);
                    report.version.clone_from(harness_version);
                    report.model.clone_from(model);
                    builtin_only = plugins.as_ref().is_some_and(|plugins| {
                        plugins.iter().all(|plugin| {
                            plugin
                                .source
                                .as_deref()
                                .is_some_and(|source| source.ends_with("@builtin"))
                        })
                    });
                }
                // b10x-harness streams a turn's text in fragments (`text-delta`): they join with
                // no separator, keeping the line breaks they carry, and each turn starts a text of
                // its own. Claude Code's and Codex's text events are whole messages.
                Event::TurnStarted { .. } if self.kind == Kind::B10x => said.push(String::new()),
                // A retried turn discards what its failed attempt streamed; metaharness passes the
                // loop's `turn-retried` on as an opaque event.
                Event::Opaque { vendor_subtype, .. }
                    if self.kind == Kind::B10x
                        && vendor_subtype.as_deref() == Some("turn-retried") =>
                {
                    if let Some(turn) = said.last_mut() {
                        turn.clear();
                    }
                }
                Event::Text { text, .. } if self.kind == Kind::B10x => match said.last_mut() {
                    Some(turn) => turn.push_str(text),
                    None => said.push(text.clone()),
                },
                Event::Text { text, .. } => said.push(text.clone()),
                // b10x-harness states each call's tokens only here. Codex's `usage` events are
                // running totals and Claude Code's are per message beside a total, so neither
                // is summed.
                Event::Usage { usage, .. } if self.kind == Kind::B10x => {
                    usage_events.push(usage.clone());
                }
                Event::ToolRequested {
                    call_id,
                    name,
                    input,
                    decision_required: true,
                    ..
                } => {
                    let decided = match self.kind {
                        Kind::Codex => decide_codex_among(&offered, name, input, &copy.dir),
                        _ => decide_among(&offered, name, input, &copy.dir),
                    };
                    let decision = match decided {
                        Ok(()) => Decision::Allow,
                        Err(reason) => Decision::Deny { reason },
                    };
                    let _ = run.send(Command::ToolDecide {
                        call_id: call_id.clone(),
                        decision,
                    });
                }
                Event::SessionEnded {
                    final_answer,
                    observed_models,
                    is_error,
                    subtype,
                    num_turns,
                    duration_ms,
                    duration_api_ms,
                    total_cost_usd,
                    usage,
                    model_usage,
                    ..
                } => {
                    if *is_error == Some(true) {
                        ended_in_error = Some(subtype.clone().unwrap_or_default());
                    }
                    report.turns = *num_turns;
                    report.duration_ms = *duration_ms;
                    report.api_ms = *duration_api_ms;
                    report.cost_usd = *total_cost_usd;
                    ended_usage.clone_from(usage);
                    if let Some(models) = model_usage
                        && !models.is_empty()
                    {
                        report.model = Some(models.keys().cloned().collect::<Vec<_>>().join("+"));
                    } else if let Some(models) = observed_models
                        && !models.is_empty()
                    {
                        let mut names: Vec<String> = models
                            .iter()
                            .filter_map(|seen| seen.model.clone())
                            .collect();
                        names.dedup();
                        report.model = Some(names.join("+"));
                    }
                    if let Some(answer) = final_answer {
                        final_text = Some(answer.text.clone());
                    }
                }
                _ => {}
            }
        }
        // Read while the transcript is still the events metaharness delivered: a vendor record
        // kept in its place holds no decision of metaharness's.
        report.tool_decisions = tool_decisions(&report.transcript);
        if let Some(usage) = charged_usage(ended_usage.as_ref(), &usage_events) {
            report.input_tokens = usage.input_tokens;
            report.output_tokens = usage.output_tokens;
            report.cache_read = usage.cache_read_input_tokens;
            report.cache_creation = usage.cache_creation_input_tokens;
            // Codex's input figure holds the cached tokens it also states apart, and
            // b10x-harness's holds, on every wire, both the cached tokens and the cache-write
            // tokens it states apart (`harness_wire::Usage`: "`input_tokens` is the whole, and
            // the cache figures are parts of it"): the input charged is the rest, so no token
            // is counted twice.
            let parts = match self.kind {
                Kind::Codex => usage.cache_read_input_tokens,
                Kind::B10x => usage
                    .cache_read_input_tokens
                    .into_iter()
                    .chain(usage.cache_creation_input_tokens)
                    .reduce(u64::saturating_add),
                Kind::Claude => None,
            };
            if let (Some(input), Some(parts)) = (usage.input_tokens, parts) {
                report.input_tokens = Some(input.saturating_sub(parts));
            }
        }
        if let Some(error) = stream_error {
            report.trace = Some(trace(self.kind, &copy, None, &report.transcript));
            return Err(error);
        }
        // The floor: every gating row that is not `ok` is reported, and every row's verdict is
        // kept. Under Claude Code's `Strict` a `gap` holds the proposal back, but not one about
        // the program installed rather than what the session was given: plugins the session
        // loaded that metaharness did not declare (H1a) when every one of them ships inside the
        // vendor program (`<name>@builtin`). Under Codex's and b10x-harness's `On` any `gap`
        // holds it back. An `unk` is recorded and holds nothing back: Codex's opening record
        // carries no plugins, output style, credential source or MCP servers (H1a, H1b, H4, H5).
        // The vendor's version against the adapter's pin (H9) is an advisory row since
        // metaharness 0.20.0, so a version off the pin is kept in `rows` and holds nothing back
        // (story:metaharness-0-20-pin).
        let mut held = Vec::new();
        for row in run.hermetic_floor() {
            report.rows.push(format!(
                "{} {}",
                row.row.id(),
                format!("{:?}", row.verdict).to_lowercase()
            ));
            if row.row.severity() != Severity::Gating || row.verdict == Verdict::Ok {
                continue;
            }
            let said = format!("{} {:?}: {}", row.row.id(), row.verdict, row.detail);
            let holds = match self.kind {
                Kind::Claude => {
                    let installed = row.row == HermeticRow::H1a && builtin_only;
                    row.verdict == Verdict::Gap && !installed
                }
                Kind::Codex => row.verdict == Verdict::Gap,
                Kind::B10x => row.verdict == Verdict::Gap,
            };
            if holds {
                held.push(said.clone());
            }
            report.floor.push(said);
        }
        // What the program printed, as metaharness kept it: the transcript, in place of the
        // events, and the reply, the result record's `result`, where there is one.
        let record = run.transcript().path.as_ref().map(PathBuf::from);
        let raw = record
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .filter(|raw| !raw.is_empty());
        let mut replied = None;
        let mut kept = None;
        if let Some(raw) = raw {
            replied = result_of(&String::from_utf8_lossy(&raw));
            report.transcript = raw;
            kept = record.as_deref();
        }
        // Projected while the record is still there: it lives in the run's scratch, which goes
        // with the run.
        report.trace = Some(trace(self.kind, &copy, kept, &report.transcript));
        drop(run);
        if halted {
            return Err("the session was halted: the step's time ran out".to_owned());
        }
        if let Some(subtype) = ended_in_error {
            return Err(format!("the session ended in an error ({subtype})"));
        }
        if !held.is_empty() {
            return Err(format!(
                "the session's hermetic floor has a gating gap: {}",
                held.join("; ")
            ));
        }
        let replied = replied.or(final_text);
        let (action, summary) = reply(&replied.unwrap_or_else(|| said.join("\n")))?;
        let files = copy.changes()?;
        let stale = copy.stale(&files);
        if !stale.is_empty() {
            return Err(format!(
                "the workspace changed under the session: {} no longer as the copy was made from",
                stale.join(", ")
            ));
        }
        Ok(Proposal {
            action,
            summary,
            files,
        })
    }
}

impl AgentExecutor for SessionExecutor {
    /// The outcome of [`Self::run_with_usage`].
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    /// One session over the frontier the brief describes, recorded as the module documents: the
    /// proposal as `ProposedAction`, or `Suspended` with `ExternalAvailability` naming why there
    /// is none; what the session reported it spent ([`usage_of`]); the SHA-256 digest of its
    /// transcript, lowercase hex, when it left one; and its summary, when it gave one.
    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let started = Instant::now();
        let mut report = SessionReport::default();
        let commission = CommissionId(Uuid(commission.data().commission_id.0.0.clone()));
        // A predecessor of another commission is refused before any copy or session.
        let proposed = self.predecessor_for(&commission).and_then(|continues| {
            // So is an id that is the predecessor's or one this executor already recorded.
            let session_id = self.session_id_for(continues.as_ref())?;
            let proposal = self.session(&mut report)?;
            self.record(
                &commission,
                frontier,
                &report,
                &proposal,
                session_id,
                continues,
            )?;
            // The predecessor is the recorded session's only.
            self.predecessor.borrow_mut().take();
            Ok(proposal)
        });
        let (outcome, summary) = match proposed {
            Ok(proposal) => {
                let summary = Some(proposal.summary.clone()).filter(|said| !said.is_empty());
                (proposal.to_outcome(), summary)
            }
            Err(why) => {
                report.error = Some(why.clone());
                let suspended = ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                    reason: SuspensionReason::ExternalAvailability(Json::Text(why)),
                });
                (suspended, None)
            }
        };
        let usage = usage_of(&report, started.elapsed());
        let transcript_digest = (!report.transcript.is_empty()).then(|| digest(&report.transcript));
        *self.last.borrow_mut() = Some(report);
        ExecutorStep {
            outcome,
            usage: Some(usage),
            transcript_digest,
            summary,
        }
    }
}

/// Whether `a` and `b` are one UUID, each read in the canonical `8-4-4-4-12` form whatever the
/// case of its hexadecimal digits; text that is no UUID equals nothing.
fn same_uuid(a: &str, b: &str) -> bool {
    matches!((parse_uuid(a), parse_uuid(b)), (Some(a), Some(b)) if a == b)
}

/// The 128 bits of a UUID written `8-4-4-4-12` in hexadecimal digits of either case.
fn parse_uuid(text: &str) -> Option<u128> {
    let groups: Vec<&str> = text.split('-').collect();
    let lengths: Vec<usize> = groups.iter().map(|group| group.len()).collect();
    if lengths != [8, 4, 4, 4, 12] {
        return None;
    }
    let digits = groups.concat();
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(&digits, 16).ok()
}

/// What a session spent, as its report states it: its input and output tokens (0 where it stated
/// none), its duration, else the call's own wall time `took`, and its cost in micro-USD where it
/// stated one. A session that failed spent too, and is charged. A session that never started
/// ([`SessionReport::started`]) made no call: its tokens and its cost are a known zero, so a run
/// under a cost limit is not left with a cost it cannot know.
#[must_use]
pub fn usage_of(report: &SessionReport, took: Duration) -> ExecutorUsage {
    let count = |figure: Option<u64>| figure.map_or(0, |n| i64::try_from(n).unwrap_or(i64::MAX));
    let duration = report
        .duration_ms
        .unwrap_or_else(|| u64::try_from(took.as_millis()).unwrap_or(u64::MAX));
    let cost = if report.started {
        report.cost_usd.map(micro_usd)
    } else {
        Some(0)
    };
    ExecutorUsage {
        input_tokens: count(report.input_tokens),
        output_tokens: count(report.output_tokens),
        duration_ms: count(Some(duration)),
        cost_micro_usd: cost,
    }
}

/// The tokens a session is charged, counted once: the total its terminal record states when it
/// states one, else the sum of the per-call `usage` events it emitted (b10x-harness's, whose
/// terminal record states none in metaharness 0.13.0), else none. Each figure of the sum is
/// absent only when no event stated it.
fn charged_usage(total: Option<&Usage>, events: &[Usage]) -> Option<Usage> {
    if let Some(total) = total {
        return Some(total.clone());
    }
    if events.is_empty() {
        return None;
    }
    let sum = |figure: fn(&Usage) -> Option<u64>| {
        events.iter().filter_map(figure).reduce(u64::saturating_add)
    };
    Some(Usage {
        input_tokens: sum(|usage| usage.input_tokens),
        output_tokens: sum(|usage| usage.output_tokens),
        cache_read_input_tokens: sum(|usage| usage.cache_read_input_tokens),
        cache_creation_input_tokens: sum(|usage| usage.cache_creation_input_tokens),
        ..Usage::default()
    })
}

/// `usd` in micro-USD, rounded to the nearest; a figure that is not a finite amount of at least
/// zero is none spent.
fn micro_usd(usd: f64) -> i64 {
    let micro = (usd * 1_000_000.0).round();
    if micro.is_finite() && micro >= 0.0 {
        // `as` saturates at the largest an i64 holds.
        micro as i64
    } else {
        0
    }
}

/// The SHA-256 digest of `bytes`, lowercase hex.
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The session's record, `bytes`, projected to `trace-ir/1` by metaharness and summarized
/// ([`SessionTrace`]). `record` names the vendor record `bytes` were read from, when metaharness
/// kept one, and its kind's production reader reads it again
/// (`metaharness::project_vendor_record`); without one, `bytes` are the events the executor
/// received, one `metaharness.event/1` line each (`metaharness::protocol::project_document`).
/// A record read again with other bytes than `bytes` is not summarized, so the summary always
/// describes the record the digest names. Why a record was not summarized names no directory of
/// the machine ([`unrooted`]).
fn trace(kind: Kind, copy: &ScratchCopy, record: Option<&Path>, bytes: &[u8]) -> SessionTrace {
    if bytes.is_empty() {
        return SessionTrace {
            transcript_digest: None,
            source: None,
            summary: Err("the session left no record".to_owned()),
        };
    }
    let named = digest(bytes);
    let (source, document) = match record {
        Some(path) => {
            let source = vendor_source(kind);
            let document = metaharness::project_vendor_record(path, source).map_err(|refusal| {
                format!(
                    "metaharness did not project the {} record: {refusal}",
                    source.as_str()
                )
            });
            (source.as_str(), document)
        }
        None => (metaharness::protocol::EVENT_FORMAT, events_document(bytes)),
    };
    let summary = document.and_then(|document| {
        if document.transcript_digest == named {
            Ok(metaharness::protocol::summarize(&document))
        } else {
            Err("the session's record changed after it was read".to_owned())
        }
    });
    SessionTrace {
        transcript_digest: Some(named),
        source: Some(source.to_owned()),
        summary: summary.map_err(|why| unrooted(&why, copy, record)),
    }
}

/// `why` with every host path the session's call knows written as what it is: the record by its
/// file name, and its directory, the copy, the scratch root and directory and the workspace by
/// name. A refusal metaharness states names the record it read by its absolute path, and a run
/// record is read beside the run, where the machine's directories are nothing a reader can open.
/// Longer paths are replaced first, so a path under another is never left half written.
fn unrooted(why: &str, copy: &ScratchCopy, record: Option<&Path>) -> String {
    let mut known: Vec<(String, String)> = Vec::new();
    let mut know = |path: &Path, name: String| {
        if path.is_absolute() && path.parent().is_some() {
            known.push((path.display().to_string(), name));
        }
    };
    if let Some(record) = record {
        let name = record.file_name().map_or_else(
            || "the session's record".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        know(record, name);
        if let Some(directory) = record.parent() {
            know(directory, "the session's record directory".to_owned());
        }
    }
    know(&copy.dir, "the session's copy".to_owned());
    know(&copy.root, "the session's scratch".to_owned());
    if let Some(scratch) = copy.root.parent() {
        know(scratch, "the scratch directory".to_owned());
    }
    know(&copy.workspace, "the workspace".to_owned());
    known.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
    known.into_iter().fold(why.to_owned(), |why, (path, name)| {
        why.replace(&path, &name)
    })
}

/// The vendor record format a session of `kind` leaves, as metaharness names it.
fn vendor_source(kind: Kind) -> metaharness::VendorSource {
    match kind {
        Kind::Claude => metaharness::VendorSource::ClaudeCodeStreamJson,
        Kind::Codex => metaharness::VendorSource::CodexRollout,
        Kind::B10x => metaharness::VendorSource::HarnessJson,
    }
}

/// The `trace-ir/1` document of the events a session delivered, one `metaharness.event/1` line
/// each in `bytes`, or why a line does not read as one.
fn events_document(bytes: &[u8]) -> Result<metaharness::protocol::TraceIrDocument, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("the session's events are not UTF-8: {error}"))?;
    let mut lines = Vec::new();
    for (at, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = metaharness::protocol::parse_event_line(line).map_err(|error| {
            format!(
                "line {} of the session's events does not read: {error}",
                at + 1
            )
        })?;
        lines.push(event);
    }
    Ok(metaharness::protocol::project_document(&lines, bytes))
}

/// Each tool call's decision the session's events hold, read from their `trace-ir/1` document
/// (metaharness 0.13.0): every `tool.decided` node in order, each with the tool its `tool_call`
/// node names (none for an empty name). The decisions are known only when the document carries
/// metaharness's decision census (`metaharness.census`, from the terminal record), every call
/// the record requested was decided, and the decisions agree with the census in each count:
/// allowed, denied, replaced, abstained, and per decider (`by_decider`). Otherwise, or when the
/// events do not project or a decision does not read, they are unknown (none), never an empty or
/// partial list. Loom decides nothing here: it reads what metaharness recorded.
#[must_use]
pub fn tool_decisions(events: &[u8]) -> Option<Vec<ToolDecision>> {
    if events.is_empty() {
        return None;
    }
    let document = events_document(events).ok()?;
    let census = document.metaharness.census.as_ref()?;
    let decided_kind = Event::ToolDecided {
        call_id: String::new(),
        decision: Decision::Abstain,
        decided_by: metaharness::protocol::DecidedBy::Adapter,
        seam: metaharness::protocol::Seam::None,
        latency_ms: None,
    }
    .name();
    let tool_call = metaharness::protocol::IrFamily::ToolCall.as_str();
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    // Every call the record requested, with the tool it names (none for an empty name).
    let mut tools: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut decided = Vec::new();
    for node in &document.events {
        let family = text(&node.kind, "event");
        if family.as_deref() == Some(tool_call) {
            let call = text(&node.kind, "call_id")?;
            let name = text(&node.kind, "name").filter(|name| !name.is_empty());
            tools.entry(call).or_insert(name);
        } else if family.as_deref() == Some(metaharness::protocol::UNK_FAMILY)
            && text(&node.kind, "event_kind").as_deref() == Some(decided_kind)
        {
            let mut payload = node.kind.get("payload")?.clone();
            payload
                .as_object_mut()?
                .insert("event".to_owned(), decided_kind.into());
            let Event::ToolDecided {
                call_id,
                decision,
                decided_by,
                ..
            } = serde_json::from_value(payload).ok()?
            else {
                return None;
            };
            decided.push((call_id, decision, decided_by));
        }
    }
    // A call requested and never decided (an observed b10x-harness call, which metaharness does
    // not decide) leaves the decisions unknown, whatever the census says.
    let decided_calls: BTreeSet<&str> = decided.iter().map(|(call, ..)| call.as_str()).collect();
    if tools
        .keys()
        .any(|call| !decided_calls.contains(call.as_str()))
    {
        return None;
    }
    // The decisions add up to the census only when every count agrees: each kind, and who
    // decided, kind by kind.
    let mut kinds = [0_u64; 4];
    let mut deciders: BTreeMap<String, u64> = BTreeMap::new();
    for (_, decision, decided_by) in &decided {
        let kind = match decision {
            Decision::Allow => 0,
            Decision::Deny { .. } => 1,
            Decision::Replace { .. } => 2,
            Decision::Abstain => 3,
        };
        kinds[kind] += 1;
        let decider = serde_json::to_value(decided_by).ok()?.as_str()?.to_owned();
        *deciders.entry(decider).or_default() += 1;
    }
    let census_kinds = [
        census.allowed,
        census.denied,
        census.replaced,
        census.abstained,
    ]
    .map(u64::from);
    let census_deciders: BTreeMap<String, u64> = census
        .by_decider
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(decider, count)| (decider.clone(), u64::from(*count)))
        .collect();
    if kinds != census_kinds || deciders != census_deciders {
        return None;
    }
    Some(
        decided
            .into_iter()
            .map(|(call_id, decision, decided_by)| {
                let (decision, reason) = match decision {
                    Decision::Allow => (CallDecision::Allow, None),
                    Decision::Deny { reason } => (CallDecision::Deny, Some(reason)),
                    Decision::Replace { .. } => (CallDecision::Replace, None),
                    Decision::Abstain => (CallDecision::Abstain, None),
                };
                let decided_by = match decided_by {
                    metaharness::protocol::DecidedBy::Embedder => DecidedBy::Embedder,
                    metaharness::protocol::DecidedBy::Frame => DecidedBy::Frame,
                    metaharness::protocol::DecidedBy::Deadline => DecidedBy::Deadline,
                    metaharness::protocol::DecidedBy::Adapter => DecidedBy::Adapter,
                    metaharness::protocol::DecidedBy::Observe => DecidedBy::Observe,
                };
                ToolDecision {
                    tool: tools.get(&call_id).cloned().flatten(),
                    call_id,
                    decision,
                    reason,
                    decided_by,
                }
            })
            .collect(),
    )
}

/// A new [`SessionId`]: a version-8 UUID from the SHA-256 digest of the process, the time and a
/// counter, so no two calls of one process, and no two processes, mint the same.
#[must_use]
pub fn fresh_session_id() -> SessionId {
    static ISSUED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let issued = ISSUED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let hashed = Sha256::digest(
        format!("loom.session\n{}\n{nanos}\n{issued}", std::process::id()).as_bytes(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hashed[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    SessionId(Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )))
}

/// The sessions an executor recorded (`loom.session.AgentSession`, through the generated
/// behaviour over this store) and the `SessionChangeProposed` each published, in the order run.
#[derive(Debug, Default)]
struct SessionRecord {
    sessions: Vec<AgentSessionSnapshot>,
    proposed: Vec<SessionChangeProposed>,
}

impl AgentSessionStorage for SessionRecord {
    fn get(&self, identity: &SessionId) -> Option<AgentSessionSnapshot> {
        self.sessions
            .iter()
            .find(|held| &held.data.session_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: AgentSessionSnapshot) {
        match self
            .sessions
            .iter_mut()
            .find(|held| held.data.session_id == snapshot.data.session_id)
        {
            Some(held) => *held = snapshot,
            None => self.sessions.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &SessionId) {
        self.sessions
            .retain(|held| &held.data.session_id != identity);
    }

    fn list(&self) -> Vec<AgentSessionSnapshot> {
        self.sessions.clone()
    }
}

impl SessionRecord {
    /// `ProposeSessionChange`, by its generated behaviour; the event it published, kept.
    fn propose(
        &mut self,
        input: ProposeSessionChange,
    ) -> Result<SessionChangeProposed, UnmetObligation> {
        let mut generated = Generated::new(std::mem::take(self));
        let answered = generated.propose_session_change(input);
        *self = generated.ports;
        let ProposeSessionChangeOutcome::Proposed {
            session_change_proposed,
        } = answered?;
        self.proposed.push(session_change_proposed.clone());
        Ok(session_change_proposed)
    }

    /// The `loom.session.AgentSessions` view, by its generated query.
    fn sessions(&self) -> Vec<AgentSessions> {
        let generated = Generated::new(Self {
            sessions: self.sessions.clone(),
            proposed: Vec::new(),
        });
        generated.agent_sessions().unwrap_or_default()
    }
}

/// metaharness's own spawn of the vendor program, the program named by path (the one the
/// embedder resolved, never another found first on the `PATH` metaharness gives its child) or,
/// named without a `/`, the first executable file of that name on that `PATH`, found when the
/// child starts ([`on_path`]): the file that runs is the one whose version the session's opening
/// record states and H9 reads. Claude Code as planned (its offered tools are [`Restricted`]'s);
/// Codex with its sandbox's network stated off and `/tmp` left out of what it may write
/// ([`CODEX_SANDBOX`]).
struct Named {
    kind: Kind,
    program: String,
    inner: Box<dyn ProcessRunner>,
}

/// The variables a Codex session's child is given that say where its home is, each as a path
/// under the session's own home ([`Homed`]), the empty one the home itself.
pub const HOME_VARIABLES: [(&str, &str); 5] = [
    ("HOME", ""),
    ("XDG_CONFIG_HOME", ".config"),
    ("XDG_DATA_HOME", ".local/share"),
    ("XDG_STATE_HOME", ".local/state"),
    ("XDG_CACHE_HOME", ".cache"),
];

/// The runner a Codex session starts through: its child's home is `home`, an empty directory in
/// the session's scratch root beside the copy, and the XDG base directories lie under it
/// ([`HOME_VARIABLES`]), so `~`, `$HOME` or a lookup of the user's configuration in the child
/// reaches nothing of the user's. `PATH` stays the one metaharness built from the user's home,
/// so the codex installed there is still the one found; `CODEX_HOME` stays metaharness's, which
/// holds what Codex needs to log in. An absolute path into the user's home is still read where
/// Codex's sandbox grants reading; what the session proposes is scanned before the embedder
/// writes it.
struct Homed<'a> {
    home: &'a Path,
    inner: &'a mut dyn ProcessRunner,
}

impl ProcessRunner for Homed<'_> {
    fn requires_executable(&self) -> bool {
        self.inner.requires_executable()
    }

    fn start(
        &mut self,
        plan: &metaharness::LaunchPlanView,
    ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
        let mut env = plan.env.clone();
        for (name, under) in HOME_VARIABLES {
            let at = if under.is_empty() {
                self.home.to_path_buf()
            } else {
                self.home.join(under)
            };
            env.insert(name.to_owned(), at.display().to_string());
        }
        let homed = metaharness::LaunchPlanView { env: &env, ..*plan };
        self.inner.start(&homed)
    }
}

/// The arguments a Claude Code session's child is given after those metaharness plans: a
/// restricted session, as the print-mode step's was. Claude Code (2.1.304) starts a restricted
/// session with auto-memory off, so its opening record names no memory directory
/// (`memory_paths`) and its first request carries no auto-memory section, and with no
/// cross-session inbox (`messaging_socket_path`); it still reads the `--settings` metaharness
/// names, its hook among them, and confines the file tools to the working directory, the copy.
/// metaharness writes the settings file itself and takes no setting for either, so the
/// executor gives the child the argument ([`Restricted`]).
pub const CLAUDE_ARGUMENTS: [&str; 1] = ["--restricted"];

/// The runner a Claude Code session starts through: its child's arguments are the ones
/// metaharness planned, less the native `--tools <pinned set>` pair metaharness 0.19.0 plans
/// (amendment a53), then [`CLAUDE_ARGUMENTS`], then `--tools` naming the tools the session is
/// offered ([`SessionExecutor::offered`]) and no other, so the child sees one `--tools`, a tool
/// outside them, a denied one included, is never offered and costs no schema in any call;
/// `--tools ""` offers none. Each call of an offered tool is still decided where the policy says.
struct Restricted<'a> {
    inner: &'a mut dyn ProcessRunner,
    offered: &'a [String],
}

/// `args` without a `--tools <list>` pair whose list is not empty. The owned surface's
/// `--tools ""` (no built-in tool) stays.
fn without_native_tools(args: &[String]) -> Vec<String> {
    let mut kept = Vec::with_capacity(args.len());
    let mut at = 0;
    while at < args.len() {
        if args[at] == "--tools" && args.get(at + 1).is_some_and(|list| !list.is_empty()) {
            at += 2;
            continue;
        }
        kept.push(args[at].clone());
        at += 1;
    }
    kept
}

impl ProcessRunner for Restricted<'_> {
    fn requires_executable(&self) -> bool {
        self.inner.requires_executable()
    }

    fn start(
        &mut self,
        plan: &metaharness::LaunchPlanView,
    ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
        let mut args = without_native_tools(plan.args);
        args.extend(CLAUDE_ARGUMENTS.iter().map(|arg| (*arg).to_owned()));
        args.extend(["--tools".to_owned(), self.offered.join(",")]);
        let restricted = metaharness::LaunchPlanView {
            args: &args,
            ..*plan
        };
        self.inner.start(&restricted)
    }
}

/// The first executable regular file named `name` in a directory of `path`, a `PATH` value.
fn on_path(name: &str, path: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}

/// What a Codex session's own sandbox is told beside metaharness's `sandbox_mode =
/// "workspace-write"` on the copy: no network, stated rather than left to the vendor's default,
/// and `/tmp` not writable, so what a command writes lands in the copy or in the child's own
/// temporary directory under metaharness's scratch root, and nowhere else.
pub const CODEX_SANDBOX: [&str; 4] = [
    "-c",
    "sandbox_workspace_write.network_access=false",
    "-c",
    "sandbox_workspace_write.exclude_slash_tmp=true",
];

impl Named {
    fn new(kind: Kind, program: String) -> Self {
        let inner: Box<dyn ProcessRunner> = match kind {
            Kind::Codex => Box::new(metaharness::CodexSpawnRunner::new()),
            _ => Box::new(metaharness::SpawnRunner::new()),
        };
        Self {
            kind,
            program,
            inner,
        }
    }
}

impl ProcessRunner for Named {
    fn start(
        &mut self,
        plan: &metaharness::LaunchPlanView,
    ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
        // In a session of its own (`setsid`, which execs the program in place, waiting when
        // it had to fork), so whatever the program leaves running is in that session and an
        // embedder that sweeps what left its own session finds it; the program as is where
        // there is no `setsid`.
        let resolved = if self.program.contains('/') {
            self.program.clone()
        } else {
            let path = plan.env.get("PATH").map_or("", String::as_str);
            on_path(&self.program, path)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!(
                            "{} is not on the PATH metaharness gives its child ({path})",
                            self.program
                        ),
                    )
                })?
                .display()
                .to_string()
        };
        let setsid = ["/usr/bin/setsid", "/bin/setsid"]
            .into_iter()
            .find(|path| Path::new(path).is_file());
        let mut args = Vec::with_capacity(plan.args.len() + 4);
        let program = match setsid {
            Some(setsid) => {
                args.push("--wait".to_owned());
                args.push(resolved.clone());
                setsid
            }
            None => resolved.as_str(),
        };
        match self.kind {
            // `codex exec`, then the sandbox's settings, then the rest as metaharness planned it.
            Kind::Codex => {
                let mut planned = plan.args.iter().cloned();
                args.extend(planned.next());
                args.extend(CODEX_SANDBOX.iter().map(|arg| (*arg).to_owned()));
                args.extend(planned);
            }
            // Claude Code's offered tools are [`Restricted`]'s to name, on every start.
            _ => args.extend(plan.args.iter().cloned()),
        }
        let named = metaharness::LaunchPlanView {
            program,
            args: &args,
            ..*plan
        };
        self.inner.start(&named)
    }
}

/// The reply in what the program printed: the `result` of its last line that reads as a JSON
/// object whose `type` is `result`, else of its last line that is not blank, a reply printed
/// as one document; none when that names no string `result`.
fn result_of(printed: &str) -> Option<String> {
    let lines: Vec<&str> = printed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let docs = |line: &&str| serde_json::from_str::<serde_json::Value>(line).ok();
    lines
        .iter()
        .rev()
        .filter_map(docs)
        .find(|doc| doc["type"] == "result")
        .or_else(|| lines.last().and_then(docs))
        .and_then(|doc| doc["result"].as_str().map(str::to_owned))
}

/// [`decide_among`] the default Claude Code tools.
#[cfg(test)]
fn decide(name: &str, input: &serde_json::Value, copy: &Path) -> Result<(), String> {
    decide_among(&ToolPolicy::claude_default().allow, name, input, copy)
}

/// Whether a Claude Code tool call is allowed: one of the tools `offered`
/// ([`SessionExecutor::offered`]) that is a file tool this executor checks
/// ([`ToolPolicyDefaults::claude_default`]'s), every path it names inside `copy`, and an Edit or
/// Write on no path with a part named `.git` (the copy's git state, or a git directory planted in
/// it); else the reason the model is told. An offered tool this executor cannot check is denied,
/// never allowed unchecked.
fn decide_among(
    offered: &[String],
    name: &str,
    input: &serde_json::Value,
    copy: &Path,
) -> Result<(), String> {
    if !offered.iter().any(|tool| tool == name) {
        return Err(format!(
            "{name} is not offered here: use only the file tools {} on the files of this directory",
            offered.join(", ")
        ));
    }
    if !CLAUDE_FILE_TOOLS.contains(&name) {
        return Err(format!(
            "{name} is offered, but its calls cannot be checked here: use only the file tools \
             {} on the files of this directory",
            offered
                .iter()
                .filter(|tool| CLAUDE_FILE_TOOLS.contains(&tool.as_str()))
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(path) = input.get(key).and_then(serde_json::Value::as_str)
            && !inside(copy, path)
        {
            return Err(format!(
                "{path} is outside this directory; work only on its files"
            ));
        }
    }
    if matches!(name, "Edit" | "Write")
        && let Some(path) = input.get("file_path").and_then(serde_json::Value::as_str)
        && Path::new(path)
            .strip_prefix(copy)
            .unwrap_or(Path::new(path))
            .components()
            .any(|part| part == Component::Normal(".git".as_ref()))
    {
        return Err(format!(
            "{path} is git's; work only on the files of this directory"
        ));
    }
    if let Some(pattern) = input.get("pattern").and_then(serde_json::Value::as_str)
        && name == "Glob"
        && (pattern.starts_with('/') || pattern.starts_with('~'))
        && !inside(copy, pattern)
    {
        return Err(format!(
            "{pattern} is outside this directory; work only on its files"
        ));
    }
    Ok(())
}

/// [`decide_codex_among`] the default Codex tools.
#[cfg(test)]
fn decide_codex(name: &str, input: &serde_json::Value, copy: &Path) -> Result<(), String> {
    decide_codex_among(&ToolPolicy::codex_default().allow, name, input, copy)
}

/// Whether a Codex tool call is allowed: one of the tools `offered`
/// ([`SessionExecutor::offered`]), and of those only the two this executor checks
/// ([`ToolPolicyDefaults::codex_default`]'s): a `Bash` command naming no place outside `copy`
/// ([`outside_word`]); an `apply_patch` whose every file is a path inside `copy` with no part
/// named `.git`; else the reason the model is told. What a command writes is confined by Codex's
/// own sandbox (`workspace-write` on the copy, no network, [`CODEX_SANDBOX`]); this check keeps a
/// command from reading or naming a place outside the copy, read lexically and so a floor, not a
/// wall: a command that builds a path at run time is not read here.
fn decide_codex_among(
    offered: &[String],
    name: &str,
    input: &serde_json::Value,
    copy: &Path,
) -> Result<(), String> {
    if !offered.iter().any(|tool| tool == name) {
        return Err(format!(
            "{name} is not offered here: use only {} on the files of this directory",
            offered.join(", ")
        ));
    }
    let text = |key: &str| match input.get(key) {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(serde_json::Value::Array(words)) => Some(
            words
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    };
    match name {
        "Bash" => {
            let command = text("command").unwrap_or_default();
            match outside_word(&command, copy) {
                Some(word) => Err(format!(
                    "`{word}` names a place outside this directory; read and change only its files"
                )),
                None => Ok(()),
            }
        }
        "apply_patch" => {
            let patch = text("patch")
                .or_else(|| text("command"))
                .or_else(|| text("input"))
                .unwrap_or_else(|| input.to_string());
            for line in patch.lines() {
                let named = [
                    "*** Add File: ",
                    "*** Update File: ",
                    "*** Delete File: ",
                    "*** Move to: ",
                ]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix));
                let Some(path) = named.map(str::trim) else {
                    continue;
                };
                if !inside(copy, path) {
                    return Err(format!(
                        "{path} is outside this directory; change only its files"
                    ));
                }
                if Path::new(path)
                    .strip_prefix(copy)
                    .unwrap_or(Path::new(path))
                    .components()
                    .any(|part| part == Component::Normal(".git".as_ref()))
                {
                    return Err(format!(
                        "{path} is git's; change only the files of this directory"
                    ));
                }
            }
            Ok(())
        }
        _ => Err(format!(
            "{name} is not offered here: use the shell to read and apply_patch to change the \
             files of this directory"
        )),
    }
}

/// The first word of a shell `command` that names a place outside `copy`, read lexically: a
/// word that starts with `~` or expands a variable (`$HOME`, `${CODEX_HOME}`); an absolute path
/// that is not inside the copy, a device stream (`/dev/null`, `/dev/stdout`, `/dev/stderr`,
/// `/dev/stdin`) or under the system's program directories, and that is there or has two parts
/// or more (a one-part word that is not there, such as a `sed` pattern `/^fn`, names no place);
/// a relative path whose `..` leaves the copy. None when no word does.
fn outside_word(command: &str, copy: &Path) -> Option<String> {
    const STREAMS: [&str; 4] = ["/dev/null", "/dev/stdout", "/dev/stderr", "/dev/stdin"];
    const PROGRAMS: [&str; 5] = ["/usr/", "/bin/", "/sbin/", "/lib/", "/lib64/"];
    let words = command
        .split(|c: char| {
            c.is_whitespace() || matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')' | '`' | '=')
        })
        .map(|word| word.trim_matches(|c| c == '\'' || c == '"'))
        .filter(|word| !word.is_empty());
    for word in words {
        let expands = word.match_indices('$').any(|(at, _)| {
            word[at + 1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '{' || c == '_')
        });
        if word.starts_with('~') || expands {
            return Some(word.to_owned());
        }
        if word.starts_with('/') {
            if inside(copy, word)
                || STREAMS.contains(&word)
                || PROGRAMS.iter().any(|dir| word.starts_with(dir))
            {
                continue;
            }
            let parts = Path::new(word)
                .components()
                .filter(|part| matches!(part, Component::Normal(_)))
                .count();
            if parts >= 2 || std::fs::symlink_metadata(word).is_ok() {
                return Some(word.to_owned());
            }
            continue;
        }
        if word.contains("..") && !inside(copy, word) {
            return Some(word.to_owned());
        }
    }
    None
}

/// Whether `path`, relative to `copy` or absolute, names a place inside it, read lexically:
/// a `..` that leaves it, a `~`, or an absolute path elsewhere does not.
fn inside(copy: &Path, path: &str) -> bool {
    if path.starts_with('~') {
        return false;
    }
    let path = Path::new(path);
    let rest = if path.is_absolute() {
        match path.strip_prefix(copy) {
            Ok(rest) => rest,
            Err(_) => return false,
        }
    } else {
        path
    };
    let mut depth = 0usize;
    for part in rest.components() {
        match part {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// The action and summary of a reply: its last line that opens a JSON object naming `action`.
fn reply(text: &str) -> Result<(String, String), String> {
    for line in text.lines().rev() {
        let line = line.trim();
        if line.starts_with('{') && line.contains("\"action\"") {
            let doc: serde_json::Value = serde_json::from_str(line)
                .map_err(|error| format!("the session's JSON line does not read: {error}"))?;
            let action = doc["action"]
                .as_str()
                .ok_or("the session's JSON line names no action")?
                .to_owned();
            let summary = doc["summary"].as_str().unwrap_or("").to_owned();
            return Ok((action, summary));
        }
    }
    Err("the session ended without the JSON line naming its action".to_owned())
}

/// Whether `path` is one plain path relative to a root: no root, no `.` or `..` part, no part
/// named `.git`, no empty part (`a//b`, a trailing `/`), not empty. Read by the string's own
/// `/`-separated parts, so a file has one plain spelling only.
#[must_use]
pub fn plain_relative(path: &str) -> bool {
    !path.contains('\0')
        && path
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".." | ".git"))
}

/// Whether `path`, a plain relative path ([`plain_relative`]), reaches through a link inside
/// `workspace`: any of its leading parts, or the whole of it, is a symbolic link there. Such a
/// path is neither copied nor proposed, since reading or writing it would reach wherever the
/// link points, outside the workspace or onto another of its files.
fn through_link(workspace: &Path, path: &str) -> bool {
    let mut at = workspace.to_path_buf();
    Path::new(path).components().any(|part| {
        at.push(part);
        std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_symlink())
    })
}

/// The scratch copy a session works in: `<scratch>/session-<pid>-<n>/copy`, and beside it
/// `git`, the git directory the copy's `.git` names when the workspace has one; both removed
/// when dropped. It keeps the digest of each file it copied, as copied: the base a session's
/// change is read against; and each named path that no link of the workspace reached when the
/// copy was made, whether there was a file at it or not.
struct ScratchCopy {
    root: PathBuf,
    dir: PathBuf,
    workspace: PathBuf,
    base: BTreeMap<String, String>,
    unlinked: Vec<String>,
}

impl ScratchCopy {
    /// Makes the copy of `files` of `workspace` under `scratch`: each regular file with its
    /// bytes and its executable bit; a link, a directory, a path that is not plain, or one
    /// reached through a link in the workspace ([`through_link`]) is left out; then the
    /// workspace's git state ([`Self::carry_git`]).
    fn make(scratch: &Path, workspace: &Path, files: &[String]) -> Result<Self, String> {
        use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let root = scratch.join(format!("session-{}-{stamp}-{n}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|error| format!("make the scratch copy {}: {error}", root.display()))?;
        let mut copy = Self {
            dir: root.join("copy"),
            root,
            workspace: workspace.to_path_buf(),
            base: BTreeMap::new(),
            unlinked: Vec::new(),
        };
        std::fs::create_dir(&copy.dir)
            .map_err(|error| format!("make {}: {error}", copy.dir.display()))?;
        // Resolved, as metaharness resolves the working directory it is given, so a path the
        // session names under it reads as inside it.
        copy.dir = std::fs::canonicalize(&copy.dir)
            .map_err(|error| format!("resolve {}: {error}", copy.dir.display()))?;
        for file in files.iter().filter(|file| plain_relative(file)) {
            if through_link(workspace, file) {
                continue;
            }
            copy.unlinked.push(file.clone());
            let from = workspace.join(file);
            let Ok(meta) = std::fs::symlink_metadata(&from) else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let to = copy.dir.join(file);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("make {}: {error}", parent.display()))?;
            }
            std::fs::copy(&from, &to).map_err(|error| {
                format!("copy {} into the scratch copy: {error}", from.display())
            })?;
            let mode = if meta.permissions().mode() & 0o111 != 0 {
                0o755
            } else {
                0o644
            };
            std::fs::set_permissions(&to, std::fs::Permissions::from_mode(mode))
                .map_err(|error| format!("set the mode of {}: {error}", to.display()))?;
            let copied = std::fs::read(&to)
                .map_err(|error| format!("read {file} in the scratch copy: {error}"))?;
            copy.base.insert(file.clone(), digest(&copied));
        }
        copy.carry_git(workspace)?;
        Ok(copy)
    }

    /// The session's home ([`Homed`]): `home` beside the copy, made empty and `0700`, removed
    /// with the copy.
    fn home(&self) -> Result<PathBuf, String> {
        use std::os::unix::fs::DirBuilderExt as _;
        let home = self.dir.with_file_name("home");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&home)
            .map_err(|error| format!("make the session's home {}: {error}", home.display()))?;
        Ok(home)
    }

    /// Gives the copy `workspace`'s git state with no git directory inside it, so a session's
    /// Glob and Grep list one `.git` file there and never git's own files: the copy's `.git`
    /// names `git` beside it, which holds the workspace's `HEAD`, index, refs, `packed-refs`,
    /// `shallow` and `info/exclude`, reads its objects through `objects/info/alternates` (git
    /// reads an alternate and writes new objects to its own directory), has an empty `hooks/`,
    /// and a `config` of the repository format, `bare = false` and the workspace's object and
    /// ref formats: no remote, include, hook path or monitor. Nothing of the workspace's git
    /// directory is written. A workspace with no git directory, no `HEAD` or no objects there,
    /// or a path git would not read back from one line, gives the copy no git state.
    fn carry_git(&self, workspace: &Path) -> Result<(), String> {
        use std::os::unix::ffi::OsStrExt as _;
        let Some((own, common)) = git_dirs(workspace) else {
            return Ok(());
        };
        let Ok(objects) = std::fs::canonicalize(common.join("objects")) else {
            return Ok(());
        };
        let git = self.dir.with_file_name("git");
        let one_line = |path: &Path| {
            let bytes = path.as_os_str().as_bytes();
            !bytes.contains(&b'\n') && !bytes.starts_with(b"\"")
        };
        let file = |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file());
        if !file(&own.join("HEAD")) || !one_line(&objects) || !one_line(&git) {
            return Ok(());
        }
        for dir in ["objects/info", "refs", "hooks", "info"] {
            let dir = git.join(dir);
            std::fs::create_dir_all(&dir).map_err(|error| git_state_failed(&dir, &error))?;
        }
        for (from, name) in [
            (&own, "HEAD"),
            (&own, "index"),
            (&common, "packed-refs"),
            (&common, "shallow"),
            (&common, "info/exclude"),
        ] {
            let from = from.join(name);
            if file(&from) {
                std::fs::copy(&from, git.join(name))
                    .map_err(|error| git_state_failed(&from, &error))?;
            }
        }
        let formats = formats(&std::fs::read_to_string(common.join("config")).unwrap_or_default());
        copy_tree(&common.join("refs"), &git.join("refs"))?;
        if formats.contains(&("refstorage", "reftable")) {
            copy_tree(&common.join("reftable"), &git.join("reftable"))?;
        }
        let mut alternates = objects.as_os_str().as_bytes().to_vec();
        alternates.push(b'\n');
        let mut config = format!(
            "[core]\n\trepositoryformatversion = {}\n\tbare = false\n",
            u8::from(!formats.is_empty())
        );
        if !formats.is_empty() {
            config.push_str("[extensions]\n");
            for (key, value) in &formats {
                config.push_str(&format!("\t{key} = {value}\n"));
            }
        }
        let mut link = b"gitdir: ".to_vec();
        link.extend_from_slice(git.as_os_str().as_bytes());
        link.push(b'\n');
        for (path, bytes) in [
            (git.join("objects/info/alternates"), alternates),
            (git.join("config"), config.into_bytes()),
            (self.dir.join(".git"), link),
        ] {
            std::fs::write(&path, bytes).map_err(|error| git_state_failed(&path, &error))?;
        }
        Ok(())
    }

    /// Every file of the copy that differs from what was copied into it: a copied file whose
    /// bytes differ or that is gone, and any file the session added; never a path with a part
    /// named `.git` ([`plain_relative`]), the copy's git state or a git directory a session
    /// planted anywhere in the copy, which no proposal carries; and never a path that reaches
    /// through a link in the workspace ([`through_link`]), the one thing of the workspace read
    /// here. A copied file is read as gone only when the copy no longer holds it, never because
    /// a link of the workspace now reaches it ([`Self::stale`] names that one). A file written
    /// there since the copy was made is never proposed back. Content that is not UTF-8, or a link
    /// the session left, is an error: a proposal carries text files only.
    fn changes(&self) -> Result<Vec<ProposedFile>, String> {
        let mut found = Vec::new();
        walk(&self.dir, &self.dir, &mut found)?;
        let present: std::collections::BTreeSet<String> = found.iter().cloned().collect();
        found.retain(|path| plain_relative(path) && !through_link(&self.workspace, path));
        let mut changes = Vec::new();
        for path in &found {
            let now = std::fs::read(self.dir.join(path))
                .map_err(|error| format!("read {path} in the scratch copy: {error}"))?;
            if self.base.get(path) == Some(&digest(&now)) {
                continue;
            }
            let content = String::from_utf8(now).map_err(|_| {
                format!("{path} is not UTF-8 text; a proposal carries text files only")
            })?;
            changes.push(ProposedFile {
                path: path.clone(),
                content: Some(content),
            });
        }
        for path in self.base.keys() {
            if !present.contains(path) {
                changes.push(ProposedFile {
                    path: path.clone(),
                    content: None,
                });
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }

    /// The paths of `changes` whose file in the workspace is no longer what the copy was made
    /// from: a copied file whose bytes there now differ from the base, or that is gone or no
    /// longer a regular file; a file the session added that now exists there. Proposing such a
    /// change would overwrite a write made in the workspace while the session ran. And, changed
    /// or not, each named path a link of the workspace reaches now and did not when the copy was
    /// made ([`through_link`]): the workspace moved under it, and a proposal naming it, or
    /// leaving it out as gone, would read or write wherever the link points.
    fn stale(&self, changes: &[ProposedFile]) -> Vec<String> {
        let linked = self
            .unlinked
            .iter()
            .filter(|path| through_link(&self.workspace, path))
            .cloned();
        let mut stale: Vec<String> = changes
            .iter()
            .filter(|change| {
                let live = self.workspace.join(&change.path);
                match self.base.get(&change.path) {
                    Some(base) => {
                        let file =
                            std::fs::symlink_metadata(&live).is_ok_and(|meta| meta.is_file());
                        !(file && std::fs::read(&live).is_ok_and(|now| digest(&now) == *base))
                    }
                    None => !matches!(
                        std::fs::symlink_metadata(&live),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound
                    ),
                }
            })
            .map(|change| change.path.clone())
            .chain(linked)
            .collect();
        stale.sort();
        stale.dedup();
        stale
    }
}

impl Drop for ScratchCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Every regular file under `dir`, by its path relative to `root`; a link is an error.
fn walk(root: &Path, dir: &Path, into: &mut Vec<String>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("read {}: {error}", dir.display()))?
            .path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("{} is outside the copy", path.display()))?
            .to_string_lossy()
            .into_owned();
        if meta.file_type().is_symlink() {
            return Err(format!(
                "the session left a link at {relative}; a proposal carries files only"
            ));
        }
        if meta.is_dir() {
            walk(root, &path, into)?;
        } else if meta.is_file() {
            into.push(relative);
        }
    }
    Ok(())
}

/// The git directory of `workspace` and its common directory: its `.git` directory, as both;
/// for a linked work tree, the directory its `.git` file names, and the one that directory's
/// `commondir` names. None when `.git` is neither.
fn git_dirs(workspace: &Path) -> Option<(PathBuf, PathBuf)> {
    let dot_git = workspace.join(".git");
    let meta = std::fs::symlink_metadata(&dot_git).ok()?;
    let own = if meta.is_dir() {
        dot_git
    } else if meta.is_file() {
        let named = std::fs::read_to_string(&dot_git).ok()?;
        workspace.join(named.lines().next()?.strip_prefix("gitdir:")?.trim())
    } else {
        return None;
    };
    let common = match std::fs::read_to_string(own.join("commondir")) {
        Ok(named) => own.join(named.trim_end_matches(['\n', '\r'])),
        Err(_) => own.clone(),
    };
    Some((own, common))
}

/// The formats a repository's `config` declares that decide how its files read, as git spells
/// them: `extensions.objectformat` `sha1` or `sha256`, `extensions.refstorage` `files` or
/// `reftable`, the last of each; any other key or value is left out.
fn formats(config: &str) -> Vec<(&'static str, &'static str)> {
    const KNOWN: [(&str, &str); 4] = [
        ("objectformat", "sha1"),
        ("objectformat", "sha256"),
        ("refstorage", "files"),
        ("refstorage", "reftable"),
    ];
    let mut section = String::new();
    let mut found: Vec<(&'static str, &'static str)> = Vec::new();
    for line in config.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[') {
            section = header
                .split([']', ' ', '\t', '"'])
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.split(['#', ';']).next().unwrap_or_default();
        let value = value.trim().trim_matches('"').to_ascii_lowercase();
        if section == "extensions"
            && let Some(known) = KNOWN.iter().find(|(k, v)| *k == key && *v == value)
        {
            found.retain(|(k, _)| *k != known.0);
            found.push(*known);
        }
    }
    found
}

/// Copies every regular file under `from` to the same path under `to`, making directories as
/// it goes; a link is left out, and a `from` that is not there copies nothing.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(git_state_failed(from, &error)),
    };
    std::fs::create_dir_all(to).map_err(|error| git_state_failed(to, &error))?;
    for entry in entries {
        let path = entry
            .map_err(|error| git_state_failed(from, &error))?
            .path();
        let meta =
            std::fs::symlink_metadata(&path).map_err(|error| git_state_failed(&path, &error))?;
        let Some(name) = path.file_name() else {
            continue;
        };
        if meta.is_dir() {
            copy_tree(&path, &to.join(name))?;
        } else if meta.is_file() {
            std::fs::copy(&path, to.join(name)).map_err(|error| git_state_failed(&path, &error))?;
        }
    }
    Ok(())
}

/// What failed at `path` as the scratch copy was given its git state.
fn git_state_failed(path: &Path, error: &std::io::Error) -> String {
    format!(
        "give the scratch copy its git state, {}: {error}",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_inside_the_copy_only_lexically_under_it() {
        let copy = Path::new("/s/copy");
        for path in ["src/lib.rs", "./a", "a/../b", "/s/copy/x", "/s/copy"] {
            assert!(inside(copy, path), "{path}");
        }
        for path in [
            "../x",
            "/etc/passwd",
            "a/../../b",
            "~/.ssh/id",
            "/s/copy/../x",
        ] {
            assert!(!inside(copy, path), "{path}");
        }
    }

    /// A proposal reads back from its outcome but for its summary, which the arguments do not
    /// carry.
    #[test]
    fn a_proposal_reads_back_from_its_outcome() {
        let mut proposal = Proposal {
            action: "implementation.edit".to_owned(),
            summary: "fixed add".to_owned(),
            files: vec![
                ProposedFile {
                    path: "src/lib.rs".to_owned(),
                    content: Some("x\n".to_owned()),
                },
                ProposedFile {
                    path: "old.rs".to_owned(),
                    content: None,
                },
            ],
        };
        let back = Proposal::from_outcome(&proposal.to_outcome());
        proposal.summary = String::new();
        assert_eq!(back, Some(Ok(proposal)));
    }

    #[test]
    fn only_file_tools_inside_the_copy_are_allowed() {
        let copy = Path::new("/s/copy");
        let input = serde_json::json!({"file_path": "/s/copy/a.rs"});
        assert!(decide("Edit", &input, copy).is_ok());
        assert!(decide("Bash", &serde_json::json!({"command": "ls"}), copy).is_err());
        let outside = serde_json::json!({"file_path": "/var/empty/.bashrc"});
        assert!(decide("Write", &outside, copy).is_err());
    }

    /// A Codex session may run a shell command that names no place outside its copy and patch
    /// files inside it; a command or a patch that names a place outside it, a patch of git's
    /// files, and any other tool are denied.
    #[test]
    fn a_codex_call_is_allowed_only_inside_the_copy() {
        let copy = Path::new("/s/copy");
        let bash = |command: &str| serde_json::json!({ "command": command });
        for command in [
            "sed -n '1,120p' src/lib.rs",
            "rg -n \"fn add\" src",
            "sed -n '/^pub fn/,/^}/p' src/lib.rs",
            "cat /s/copy/src/lib.rs 2>/dev/null",
            "git status --short && git log --oneline -3",
            "git diff HEAD~1..HEAD",
            "/usr/bin/ls -la",
        ] {
            assert!(
                decide_codex("Bash", &bash(command), copy).is_ok(),
                "{command}"
            );
        }
        for command in [
            "cat ~/.ssh/id_ed25519",
            "cat $HOME/.codex/auth.json",
            "cat ${CODEX_HOME}/auth.json",
            "cat /var/empty/.config/secret",
            "ls /etc",
            "cd .. && ls",
            "cat ../other/file",
            "cat /proc/self/environ",
        ] {
            assert!(
                decide_codex("Bash", &bash(command), copy).is_err(),
                "{command}"
            );
        }
        let patch = |path: &str| serde_json::json!({ "command": format!("*** Begin Patch\n*** Update File: {path}\n@@\n-a\n+b\n*** End Patch\n") });
        assert!(decide_codex("apply_patch", &patch("src/lib.rs"), copy).is_ok());
        assert!(decide_codex("apply_patch", &patch("/s/copy/src/lib.rs"), copy).is_ok());
        for path in [
            "../x.rs",
            "/etc/hosts",
            ".git/config",
            "sub/.git/hooks/pre-commit",
        ] {
            assert!(
                decide_codex("apply_patch", &patch(path), copy).is_err(),
                "{path}"
            );
        }
        assert!(decide_codex("WebFetch", &serde_json::json!({}), copy).is_err());
        assert!(decide_codex("mcp__x__y", &serde_json::json!({}), copy).is_err());
    }

    /// A session's Edit or Write never reaches the copy's git state, nor a git directory it
    /// would plant anywhere in the copy; reading the `.git` file, and writing `.gitignore`, is
    /// allowed.
    #[test]
    fn no_file_tool_writes_a_path_under_git() {
        let copy = Path::new("/s/copy");
        for (name, path) in [
            ("Write", "/s/copy/.git"),
            ("Edit", ".git"),
            ("Write", ".git/config"),
            ("Write", "sub/.git/hooks/pre-commit"),
            ("Edit", "./a/../.git/HEAD"),
        ] {
            let input = serde_json::json!({ "file_path": path });
            assert!(decide(name, &input, copy).is_err(), "{name} {path}");
        }
        for (name, path) in [
            ("Read", "/s/copy/.git"),
            ("Write", ".gitignore"),
            ("Edit", "/s/copy/.github/workflows/check.yml"),
        ] {
            let input = serde_json::json!({ "file_path": path });
            assert!(decide(name, &input, copy).is_ok(), "{name} {path}");
        }
    }

    /// A directory of the test's own under the system's temporary directory, with `scratch/`
    /// in it for the copies; removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("session-executor-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A workspace with no git directory gives a copy with none.
    #[test]
    fn a_workspace_without_git_gives_a_copy_without_it() {
        let scratch = Scratch::new("plain");
        let tree = scratch.0.join("tree");
        std::fs::create_dir_all(&tree).expect("the tree");
        std::fs::write(tree.join("a.txt"), "a\n").expect("a file");
        let copy = ScratchCopy::make(&scratch.0.join("scratch"), &tree, &["a.txt".to_owned()])
            .expect("the copy");
        assert!(copy.dir.join("a.txt").is_file());
        assert!(std::fs::symlink_metadata(copy.dir.join(".git")).is_err());
        assert!(std::fs::symlink_metadata(copy.dir.with_file_name("git")).is_err());
    }

    /// A session that never started is charged a known zero; one that started and stated no cost
    /// is charged an unknown cost, never zero; one that stated a cost is charged it.
    #[test]
    fn only_a_session_that_never_started_costs_a_known_zero() {
        let took = Duration::from_millis(7);
        let never = usage_of(&SessionReport::default(), took);
        assert_eq!(
            (
                never.input_tokens,
                never.output_tokens,
                never.cost_micro_usd
            ),
            (0, 0, Some(0))
        );
        assert_eq!(never.duration_ms, 7);
        let silent = SessionReport {
            started: true,
            ..SessionReport::default()
        };
        assert_eq!(usage_of(&silent, took).cost_micro_usd, None);
        let priced = SessionReport {
            started: true,
            cost_usd: Some(0.25),
            input_tokens: Some(3),
            ..SessionReport::default()
        };
        let priced = usage_of(&priced, took);
        assert_eq!(
            (priced.input_tokens, priced.cost_micro_usd),
            (3, Some(250_000))
        );
    }

    /// A session's tokens are counted once: the terminal total when the session states one, even
    /// beside per-call events (metaharness 0.13.0's b10x adapter states no total, so this is
    /// pinned here and not through a scripted session); else the events summed, a figure no event
    /// stated staying absent; else none.
    #[test]
    fn a_terminal_usage_total_is_charged_once_and_events_only_without_one() {
        let usage = |input, output, cached: Option<u64>| Usage {
            input_tokens: Some(input),
            output_tokens: Some(output),
            cache_read_input_tokens: cached,
            ..Usage::default()
        };
        let events = [usage(100, 20, Some(40)), usage(50, 10, None)];
        let total = usage(150, 30, Some(40));
        assert_eq!(charged_usage(Some(&total), &events), Some(total.clone()));
        assert_eq!(charged_usage(Some(&total), &[]), Some(total));
        let summed = charged_usage(None, &events).expect("the events summed");
        assert_eq!(
            (
                summed.input_tokens,
                summed.output_tokens,
                summed.cache_read_input_tokens,
                summed.cache_creation_input_tokens
            ),
            (Some(150), Some(30), Some(40), None)
        );
        assert_eq!(charged_usage(None, &[]), None);
    }

    /// A named path with a linked directory anywhere above it in the workspace is not copied,
    /// and a file the session adds under that directory's name in the copy is not proposed: the
    /// effect would write it through the link.
    #[test]
    fn a_path_through_a_linked_directory_is_neither_copied_nor_proposed() {
        let scratch = Scratch::new("linked");
        let tree = scratch.0.join("tree");
        let outside = scratch.0.join("outside");
        std::fs::create_dir_all(tree.join("src")).expect("the tree");
        std::fs::create_dir_all(outside.join("deep")).expect("a directory outside");
        std::fs::write(tree.join("src/lib.rs"), "a\n").expect("a file");
        std::fs::write(outside.join("deep/secret.txt"), "s\n").expect("a file outside");
        std::os::unix::fs::symlink(&outside, tree.join("src/linked")).expect("a link");
        let files = ["src/lib.rs", "src/linked/deep/secret.txt"].map(str::to_owned);
        assert!(through_link(&tree, "src/linked/deep/secret.txt"));
        assert!(through_link(&tree, "src/linked"));
        assert!(!through_link(&tree, "src/lib.rs"));
        assert!(!through_link(&tree, "src/absent/file.rs"));
        let copy = ScratchCopy::make(&scratch.0.join("scratch"), &tree, &files).expect("the copy");
        assert!(copy.dir.join("src/lib.rs").is_file());
        assert!(!copy.dir.join("src/linked").exists());
        std::fs::create_dir_all(copy.dir.join("src/linked/deep")).expect("a directory in the copy");
        std::fs::write(copy.dir.join("src/linked/deep/new.txt"), "n\n").expect("an added file");
        std::fs::write(copy.dir.join("src/new.rs"), "n\n").expect("an added file");
        let paths: Vec<String> = copy
            .changes()
            .expect("the changes")
            .into_iter()
            .map(|file| file.path)
            .collect();
        assert_eq!(paths, ["src/new.rs"]);
    }

    /// A plain path is read by its own `/`-separated parts: a `.`, `..`, `.git` or empty part,
    /// a root, a NUL or the empty string is not plain.
    #[test]
    fn a_file_has_one_plain_spelling() {
        for plain in [
            "src/lib.rs",
            "README.md",
            ".gitignore",
            "a/.github/b",
            "..a/b.",
        ] {
            assert!(plain_relative(plain), "{plain}");
        }
        for unplain in [
            "",
            "/",
            "/src/lib.rs",
            "src/./lib.rs",
            "./src/lib.rs",
            "src//lib.rs",
            "src/",
            "src/lib.rs/.",
            "src/../lib.rs",
            "a/.git/HEAD",
            ".git",
            "a\0b",
        ] {
            assert!(!plain_relative(unplain), "{unplain:?}");
        }
    }

    /// A named directory turned into a link while the session ran: the copied file under it is
    /// not read as gone, and it and a named path absent at copy time are both stale.
    #[test]
    fn a_named_path_a_link_reaches_since_the_copy_is_stale_not_gone() {
        let scratch = Scratch::new("relinked");
        let tree = scratch.0.join("tree");
        let outside = scratch.0.join("outside");
        std::fs::create_dir_all(tree.join("src")).expect("the tree");
        std::fs::create_dir_all(&outside).expect("a directory outside");
        std::fs::write(tree.join("src/lib.rs"), "a\n").expect("a file");
        std::fs::write(tree.join("README.md"), "r\n").expect("a file");
        std::fs::write(outside.join("lib.rs"), "a\n").expect("the same bytes outside");
        let files = ["src/lib.rs", "src/new.rs", "README.md"].map(str::to_owned);
        let copy = ScratchCopy::make(&scratch.0.join("scratch"), &tree, &files).expect("the copy");
        std::fs::write(copy.dir.join("README.md"), "edited\n").expect("the session's edit");
        std::fs::rename(tree.join("src"), scratch.0.join("moved")).expect("src moved away");
        std::os::unix::fs::symlink(&outside, tree.join("src")).expect("src now a link");
        let changes = copy.changes().expect("the changes");
        let paths: Vec<&str> = changes.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["README.md"]);
        assert_eq!(copy.stale(&changes), ["src/lib.rs", "src/new.rs"]);
    }

    /// The arguments of each plan a [`Recording`] runner was started with.
    type Plans = std::rc::Rc<RefCell<Vec<Vec<String>>>>;

    /// A scripted Claude Code session that keeps the arguments of the plan it is started with,
    /// then replies naming `tests.write`.
    struct Recording(Plans);

    impl ProcessRunner for Recording {
        fn requires_executable(&self) -> bool {
            false
        }

        fn start(
            &mut self,
            plan: &metaharness::LaunchPlanView,
        ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
            self.0.borrow_mut().push(plan.args.to_vec());
            let init = serde_json::json!({
                "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
                "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow, "mcp_servers": [],
                "plugins": [], "output_style": "default", "apiKeySource": "none",
                "model": "scripted", "permissionMode": "default", "session_id": "s-1",
            });
            let reply = serde_json::json!({
                "type": "assistant",
                "message": {"content": [{"type": "text",
                                         "text": "{\"action\": \"tests.write\", \"summary\": \"s\"}"}],
                            "usage": {"input_tokens": 1, "output_tokens": 1}},
            });
            let result = serde_json::json!({
                "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
                "usage": {"input_tokens": 1, "output_tokens": 1},
            });
            metaharness::ScriptedRunner::of_lines(
                [init.to_string(), reply.to_string(), result.to_string()],
                metaharness::ScriptedLog::new(),
            )
            .start(plan)
        }
    }

    /// A Claude Code session's child is started restricted, auto-memory and the cross-session inbox off: `--restricted`
    /// among its arguments beside metaharness's own `--settings`, whatever runner starts it.
    #[test]
    fn a_claude_session_starts_with_auto_memory_off() {
        let scratch = Scratch::new("memory");
        let tree = scratch.0.join("tree");
        std::fs::create_dir_all(&tree).expect("the tree");
        std::fs::write(tree.join("a.txt"), "a\n").expect("a file");
        let plans = Plans::default();
        let mut executor = SessionExecutor::new(&tree, vec!["a.txt".to_owned()], "brief")
            .with_runner(Box::new(Recording(std::rc::Rc::clone(&plans))));
        executor.credentials = CredentialSource::None;
        executor.scratch = scratch.0.join("scratch");
        let mut report = SessionReport::default();
        let proposal = executor.session(&mut report);
        assert_eq!(
            proposal.map(|proposal| proposal.action),
            Ok("tests.write".to_owned()),
            "{:?}",
            report.floor
        );
        let plans = plans.borrow();
        assert_eq!(plans.len(), 1, "one session");
        assert!(
            plans[0].iter().any(|arg| arg == "--restricted"),
            "the child's arguments: {:?}",
            plans[0]
        );
        assert!(
            plans[0].iter().any(|arg| arg == "--settings"),
            "{:?}",
            plans[0]
        );
    }

    /// The tool policy's decision mode is the one metaharness's session request carries, for
    /// each mode, and with no policy given it is the kind's default: `ask` for Claude Code and
    /// Codex.
    #[test]
    fn the_policys_decision_mode_reaches_the_session_request() {
        let copy = Path::new("/s/copy");
        for kind in [Kind::Claude, Kind::Codex] {
            let mut executor =
                SessionExecutor::new("/s/workspace", vec!["a.txt".to_owned()], "brief")
                    .of_kind(kind);
            assert_eq!(
                executor.spec(copy).map(|spec| spec.decisions),
                Ok(DecisionMode::Ask),
                "{kind:?} by default"
            );
            for (mode, expected) in [
                (PolicyDecisions::Frame, DecisionMode::Frame),
                (PolicyDecisions::Ask, DecisionMode::Ask),
                (PolicyDecisions::Observe, DecisionMode::Observe),
            ] {
                let mut policy = executor.policy();
                policy.decisions = mode;
                executor.tools = Some(policy);
                assert_eq!(
                    executor.spec(copy).map(|spec| spec.decisions),
                    Ok(expected),
                    "{kind:?} under {mode:?}"
                );
            }
        }
    }

    /// The offered tools are `allow` less `deny`, in `allow`'s order, each once; an offered tool
    /// this executor cannot check is denied at the seam, as is one not offered.
    #[test]
    fn the_offered_tools_are_allow_less_deny() {
        let mut executor = SessionExecutor::new("/s/workspace", Vec::new(), "brief");
        let tool = |name: &str| name.to_owned();
        executor.tools = Some(ToolPolicy {
            allow: vec![tool("Read"), tool("Bash"), tool("Read"), tool("Edit")],
            deny: vec![tool("Edit"), tool("Grep")],
            egress: Vec::new(),
            decisions: PolicyDecisions::Ask,
        });
        assert_eq!(executor.offered(), [tool("Read"), tool("Bash")]);
        let copy = Path::new("/s/copy");
        let offered = executor.offered();
        let read = serde_json::json!({"file_path": "/s/copy/a.txt"});
        assert!(decide_among(&offered, "Read", &read, copy).is_ok());
        assert!(decide_among(&offered, "Edit", &read, copy).is_err());
        assert!(
            decide_among(
                &offered,
                "Bash",
                &serde_json::json!({"command": "ls"}),
                copy
            )
            .is_err()
        );
        assert!(decide_codex_among(&[tool("Bash")], "apply_patch", &read, copy).is_err());
    }

    /// Set in the child [`a_claude_session_metaharness_spawns_starts_with_auto_memory_off`]
    /// runs: the file the double writes its arguments to.
    const SPAWN_CHILD: &str = "SESSION_EXECUTOR_SPAWN_CHILD";

    /// The same with no runner and no program named: metaharness's own spawn starts the
    /// `claude` on the `PATH` it gives its child, `$HOME/.local/bin` first. Run in a child of
    /// this test binary whose `HOME` holds a double there that writes its arguments out.
    #[test]
    fn a_claude_session_metaharness_spawns_starts_with_auto_memory_off() {
        if let Some(seen) = std::env::var_os(SPAWN_CHILD) {
            let scratch = Scratch::new("memory-spawn");
            let tree = scratch.0.join("tree");
            std::fs::create_dir_all(&tree).expect("the tree");
            std::fs::write(tree.join("a.txt"), "a\n").expect("a file");
            let mut executor = SessionExecutor::new(&tree, vec!["a.txt".to_owned()], "brief");
            executor.credentials = CredentialSource::None;
            executor.scratch = scratch.0.join("scratch");
            let mut report = SessionReport::default();
            let proposal = executor.session(&mut report);
            assert_eq!(
                proposal.map(|proposal| proposal.action),
                Ok("tests.write".to_owned()),
                "{:?} {:?}",
                report.error,
                report.floor
            );
            assert!(Path::new(&seen).is_file(), "the double ran");
            return;
        }
        let scratch = Scratch::new("memory-spawn-home");
        let home = scratch.0.join("home");
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).expect("a program directory");
        let seen = scratch.0.join("args");
        let double = r#"#!/bin/sh
printf '%s\n' "$@" > '@SEEN@'
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"2.1.304\",\"cwd\":\"$PWD\",\"tools\":[\"Read\",\"Edit\",\"Write\",\"Glob\",\"Grep\"],\"mcp_servers\":[],\"plugins\":[],\"output_style\":\"default\",\"apiKeySource\":\"none\",\"model\":\"scripted\",\"permissionMode\":\"default\",\"session_id\":\"s-1\"}"
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"{\"action\": \"tests.write\", \"summary\": \"s\"}","usage":{"input_tokens":1,"output_tokens":1}}'
"#
        .replace("@SEEN@", &seen.display().to_string());
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::write(bin.join("claude"), double).expect("the double");
            std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755))
                .expect("its mode");
        }
        let output = std::process::Command::new(std::env::current_exe().expect("this binary"))
            .args([
                "--exact",
                "tests::a_claude_session_metaharness_spawns_starts_with_auto_memory_off",
                "--nocapture",
            ])
            .env(SPAWN_CHILD, &seen)
            .env("HOME", &home)
            .output()
            .expect("the child runs");
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.status.success() && said.contains("1 passed"),
            "{said}"
        );
        let args = std::fs::read_to_string(&seen).expect("the double's arguments");
        assert!(
            args.lines().any(|line| line == "--restricted"),
            "the child's arguments:\n{args}"
        );
    }
}
