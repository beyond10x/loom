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
//! | `Claude` | Claude Code | `Strict` | decided here (`Ask`): the file tools [`TOOLS`] on paths inside the copy; restricted, auto-memory and the cross-session inbox off ([`CLAUDE_ARGUMENTS`]) |
//! | `Codex` | Codex (`codex exec`) | `On`, H9 gating | decided here (`Ask`): `Bash` and `apply_patch` ([`CODEX_TOOLS`]) naming no path outside the copy; its own sandbox (`workspace-write` on the copy, no network) confines what a command writes; its home is its own ([`HOME_VARIABLES`]) |
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
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended,
    ExecutorStep, ExecutorUsage, Frontier, ProposedActionArguments, SuspensionReason,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use loom::behaviour::{AgentSessionStorage, Generated};
use loom::obligation::UnmetObligation;
use loom::primitives::Uuid;
use loom::session::obligations::{AgentSessionsQuery as _, ProposeSessionChangeBehavior as _};
use loom::session::{
    AgentSessionSnapshot, AgentSessions, ChangedFile, CommissionId, FrontierId,
    ProposeSessionChange, ProposeSessionChangeOutcome, SessionChangeProposed, SessionId,
    SessionKind,
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
pub const TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Glob", "Grep"];

/// The tools a Codex session may use, as its `PreToolUse` hook names them: the shell, to read
/// the copy, and `apply_patch`, to change it.
pub const CODEX_TOOLS: [&str; 2] = ["Bash", "apply_patch"];

/// The model endpoint a `Kind::B10x` session's loop is pointed at, and the subscription token
/// it reads there: the loop reads the file itself, and metaharness copies nothing.
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
    runner: RefCell<Option<Box<dyn ProcessRunner>>>,
    last: RefCell<Option<SessionReport>>,
    ids: RefCell<Box<dyn FnMut() -> SessionId>>,
    record: RefCell<SessionRecord>,
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
            runner: RefCell::new(None),
            last: RefCell::new(None),
            ids: RefCell::new(Box::new(fresh_session_id)),
            record: RefCell::new(SessionRecord::default()),
        }
    }

    /// The same executor, each call's [`SessionId`] taken from `ids` instead of a fresh one
    /// ([`fresh_session_id`]): for a caller that assigns the ids itself, as a conformance run
    /// assigns the scenario's.
    #[must_use]
    pub fn with_session_ids(self, ids: impl FnMut() -> SessionId + 'static) -> Self {
        *self.ids.borrow_mut() = Box::new(ids);
        self
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
    /// for `commission` over `frontier`, under a new [`SessionId`].
    fn record(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
        report: &SessionReport,
        proposal: &Proposal,
    ) -> Result<SessionChangeProposed, String> {
        let session_id = (self.ids.borrow_mut())();
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
                commission_id: CommissionId(Uuid(commission.data().commission_id.0.0.clone())),
                frontier_id: FrontierId(Uuid(frontier.data().frontier_id.0.0.clone())),
                vendor_session_id: report.vendor_session_id.clone(),
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

    /// The run metaharness is asked for, by kind: Claude Code `Strict` with every call decided
    /// here; Codex `On` with every call decided here (its strict floor fails on four rows its
    /// opening record does not carry, H1a, H1b, H4 and H5, which metaharness cannot decide for
    /// it yet); b10x-harness `On`, observed, pointed at [`Self::endpoint`] with its subscription
    /// token, confined by substrate in the loop's own process so its write tools reach the copy
    /// and nothing else. Each with the copy as its declared scratch working directory.
    fn spec(&self, copy: &Path) -> Result<RunSpec, String> {
        let mut spec = RunSpec::new(self.kind);
        spec.credentials = self.credentials;
        spec.cwd = Some(copy.to_path_buf());
        spec.scratch_cwd = true;
        spec.prompt = Some(self.brief.clone());
        spec.model.clone_from(&self.model);
        match self.kind {
            Kind::Claude => {
                spec.hermetic = HermeticMode::Strict;
                spec.decisions = DecisionMode::Ask;
                spec.max_turns = self.max_turns;
            }
            Kind::Codex => {
                spec.hermetic = HermeticMode::On;
                spec.decisions = DecisionMode::Ask;
            }
            Kind::B10x => {
                let endpoint = self.endpoint.as_ref().ok_or(
                    "a b10x-harness session needs the endpoint its loop is pointed at; none given",
                )?;
                spec.hermetic = HermeticMode::On;
                spec.decisions = DecisionMode::Observe;
                spec.max_turns = self.max_turns;
                spec.credentials = CredentialSource::None;
                spec.model_endpoint = Some(endpoint.base_url.clone());
                spec.model_wire = Some(endpoint.wire.clone());
                spec.model = Some(self.model.clone().unwrap_or(endpoint.model.clone()));
                spec.subscription_token_file = Some(endpoint.token_file.display().to_string());
                spec.subscription_token_pointer
                    .clone_from(&endpoint.token_pointer);
                spec.substrate_embedded = true;
            }
        }
        Ok(spec)
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
                let mut restricted = Restricted { inner };
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
                        Kind::Codex => decide_codex(name, input, &copy.dir),
                        _ => decide(name, input, &copy.dir),
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
        // kept. Under Claude Code's `Strict` a `gap` holds the proposal back, but two facts
        // about the program installed and not about what the session was given: the vendor's
        // version against the adapter's pin (H9), and plugins the session loaded that
        // metaharness did not declare (H1a) when every one of them ships inside the vendor
        // program (`<name>@builtin`, which a version newer than the pin adds). Under Codex's
        // and b10x-harness's `On` any `gap` holds it back, and for Codex an H9 that is not `ok`
        // does too: a codex other than the pinned one proposes nothing. An `unk` there is
        // recorded and holds nothing back: Codex's opening record carries no plugins, output
        // style, credential source or MCP servers (H1a, H1b, H4, H5).
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
                    let installed =
                        row.row == HermeticRow::H9 || (row.row == HermeticRow::H1a && builtin_only);
                    row.verdict == Verdict::Gap && !installed
                }
                Kind::Codex => row.verdict == Verdict::Gap || row.row == HermeticRow::H9,
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
        let proposed = self.session(&mut report).and_then(|proposal| {
            self.record(commission, frontier, &report, &proposal)
                .map(|_| proposal)
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
/// record states and H9 reads. Claude Code offered the file tools [`TOOLS`] and no other; Codex
/// with its sandbox's network stated off and `/tmp` left out of what it may write
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
/// metaharness planned, then [`CLAUDE_ARGUMENTS`].
struct Restricted<'a> {
    inner: &'a mut dyn ProcessRunner,
}

impl ProcessRunner for Restricted<'_> {
    fn requires_executable(&self) -> bool {
        self.inner.requires_executable()
    }

    fn start(
        &mut self,
        plan: &metaharness::LaunchPlanView,
    ) -> std::io::Result<Box<dyn metaharness::HarnessProcess>> {
        let mut args = plan.args.to_vec();
        args.extend(CLAUDE_ARGUMENTS.iter().map(|arg| (*arg).to_owned()));
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
            _ => {
                args.extend(plan.args.iter().cloned());
                // Offered only the file tools, so no other tool costs its schema in every call:
                // each call of one is still decided at the seam ([`decide`]).
                args.extend(["--tools".to_owned(), TOOLS.join(",")]);
            }
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

/// Whether a tool call is allowed: one of [`TOOLS`], every path it names inside `copy`, and an
/// Edit or Write on no path with a part named `.git` (the copy's git state, or a git directory
/// planted in it); else the reason the model is told.
fn decide(name: &str, input: &serde_json::Value, copy: &Path) -> Result<(), String> {
    if !TOOLS.contains(&name) {
        return Err(format!(
            "{name} is not offered here: use only the file tools {} on the files of this directory",
            TOOLS.join(", ")
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

/// Whether a Codex tool call is allowed: one of [`CODEX_TOOLS`]; a `Bash` command naming no
/// place outside `copy` ([`outside_word`]); an `apply_patch` whose every file is a path inside
/// `copy` with no part named `.git`; else the reason the model is told. What a command writes is
/// confined by Codex's own sandbox (`workspace-write` on the copy, no network, [`CODEX_SANDBOX`]);
/// this check keeps a command from reading or naming a place outside the copy, read lexically
/// and so a floor, not a wall: a command that builds a path at run time is not read here.
fn decide_codex(name: &str, input: &serde_json::Value, copy: &Path) -> Result<(), String> {
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
                "cwd": plan.cwd.display().to_string(), "tools": TOOLS, "mcp_servers": [],
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
