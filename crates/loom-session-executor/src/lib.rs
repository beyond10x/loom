//! An [`AgentExecutor`] of loom-sdk over a metaharness session: the session works in a scratch
//! copy of a workspace, never in the workspace itself, and the executor returns what it changed
//! there as a proposal ([`Proposal`]); it performs nothing. The embedder's edit effect applies
//! a proposal, or refuses it, under its own guard.
//!
//! One executor, the harness its [`Kind`]: the same brief, the same scratch copy, the same
//! proposal for each. The copy is the session's working directory, declared as the embedder's
//! scratch copy (`with_cwd` and `with_scratch_cwd`); the child's environment is metaharness's,
//! constructed from an allowlist and never inherited, so no variable of the embedder's reaches
//! it but those metaharness names.
//!
//! | kind | harness | hermetic | calls |
//! |---|---|---|---|
//! | `Claude` | Claude Code | `Strict` | decided here (`Ask`): the file tools [`TOOLS`] on paths inside the copy |
//! | `Codex` | Codex (`codex exec`) | `On`, H9 gating | decided here (`Ask`): `Bash` and `apply_patch` ([`CODEX_TOOLS`]) naming no path outside the copy; its own sandbox (`workspace-write` on the copy, no network) confines what a command writes; its home is its own ([`HOME_VARIABLES`]) |
//! | `B10x` | b10x-harness | `On`, a gap holds the proposal | observed, never decided: metaharness's b10x adapter decides no call; the loop's toolset is what substrate confines to the copy |
//!
//! This crate depends on no crate of the application that embeds it, so it can move into Loom
//! unchanged.

use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, Frontier,
    ProposedActionArguments, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use metaharness::protocol::{
    Command, Decision, DecisionMode, Event, HermeticMode, HermeticRow, RunSpec, SeamFactory,
    Severity, Verdict,
};
use metaharness::{ClaudeSeams, CodexSeams, EventPoll, Input, Metaharness};

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
    pub summary: String,
    pub files: Vec<ProposedFile>,
}

impl Proposal {
    /// The proposal as the executor outcome that carries it: `ProposedAction` with the action
    /// and, as its arguments, `{"summary": …, "files": [{"path": …, "content": …}]}`, a removed
    /// file's `content` null.
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
            arguments: ProposedActionArguments(Json::Object(vec![
                ("summary".to_owned(), Json::Text(self.summary.clone())),
                ("files".to_owned(), Json::Array(files)),
            ])),
        })
    }

    /// The proposal an outcome carries: none for an outcome that proposes no action; an error
    /// naming what does not read for a `ProposedAction` whose arguments are not a proposal's.
    pub fn from_outcome(outcome: &ExecutorOutcome) -> Option<Result<Self, String>> {
        let ExecutorOutcome::ProposedAction(proposed) = outcome else {
            return None;
        };
        let arguments = &proposed.arguments.0;
        let read = || -> Result<Self, String> {
            let summary = match arguments.member("summary") {
                Some(Json::Text(text)) => text.clone(),
                None | Some(Json::Null) => String::new(),
                Some(other) => return Err(format!("summary is {}", other.describes())),
            };
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
                summary,
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
}

/// The executor: one call runs one session over a scratch copy of `workspace` holding the
/// files `files` names, with `brief` as its prompt, and proposes what it changed there.
pub struct SessionExecutor {
    /// The harness the session runs: `Kind::Claude` by default.
    pub kind: Kind,
    /// The workspace the copy is made from; never written.
    pub workspace: PathBuf,
    /// The files copied, by their paths relative to the workspace.
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
        }
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
        let copy = ScratchCopy::make(&self.scratch, &self.workspace, &self.files)?;
        report.cwd = Some(copy.dir.clone());
        let builder = Metaharness::from_spec(self.spec(&copy.dir)?);
        let mut seams: Box<dyn SeamFactory> = match self.kind {
            Kind::Claude => Box::new(ClaudeSeams),
            Kind::Codex => Box::new(CodexSeams),
            Kind::B10x => Box::new(metaharness_b10x::B10xSeams::new(None, None, None)),
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
        let runner: Option<&mut dyn ProcessRunner> = match (supplied.as_mut(), named.as_mut()) {
            (Some(runner), _) => Some(runner.as_mut()),
            (None, Some(named)) => Some(named),
            (None, None) => None,
        };
        let started = match (runner, &home) {
            (Some(inner), Some(home)) => {
                let mut homed = Homed { home, inner };
                builder.start_with(Input::FromSpec, &mut homed, &mut *seams)
            }
            (Some(runner), None) => builder.start_with(Input::FromSpec, runner, &mut *seams),
            (None, _) => builder.start(Input::FromSpec),
        };
        drop(named);
        drop(supplied);
        let mut run =
            started.map_err(|refusal| format!("metaharness refused the session: {refusal}"))?;
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
                Err(error) => return Err(format!("the session's stream: {error}")),
            };
            if let Ok(text) = serde_json::to_string(&*line) {
                report.transcript.extend_from_slice(text.as_bytes());
                report.transcript.push(b'\n');
            }
            match &line.event {
                Event::SessionStarted {
                    harness_version,
                    model,
                    plugins,
                    ..
                } => {
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
                    if let Some(usage) = usage {
                        report.input_tokens = usage.input_tokens;
                        report.output_tokens = usage.output_tokens;
                        report.cache_read = usage.cache_read_input_tokens;
                        report.cache_creation = usage.cache_creation_input_tokens;
                        // Codex's input figure, and the OpenAI Responses wire's, holds the cached
                        // tokens it also states apart: the input charged is the rest, so no
                        // token is counted twice.
                        let responses = self.kind == Kind::Codex
                            || self.endpoint.as_ref().is_some_and(|endpoint| {
                                self.kind == Kind::B10x && endpoint.wire == "openai-responses"
                            });
                        if responses
                            && let (Some(input), Some(cached)) =
                                (usage.input_tokens, usage.cache_read_input_tokens)
                        {
                            report.input_tokens = Some(input.saturating_sub(cached));
                        }
                    }
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
        let raw = run
            .transcript()
            .path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .filter(|raw| !raw.is_empty());
        let mut replied = None;
        if let Some(raw) = raw {
            replied = result_of(&String::from_utf8_lossy(&raw));
            report.transcript = raw;
        }
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
        let files = copy.changes(&self.workspace, &self.files)?;
        Ok(Proposal {
            action,
            summary,
            files,
        })
    }
}

impl AgentExecutor for SessionExecutor {
    /// One session over the frontier the brief describes; the proposal as `ProposedAction`, or
    /// `Suspended` with `ExternalAvailability` naming why there is none.
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut report = SessionReport::default();
        let outcome = match self.session(&mut report) {
            Ok(proposal) => proposal.to_outcome(),
            Err(why) => {
                report.error = Some(why.clone());
                ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                    reason: SuspensionReason::ExternalAvailability(Json::Text(why)),
                })
            }
        };
        *self.last.borrow_mut() = Some(report);
        outcome
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
/// named `.git`, not empty.
#[must_use]
pub fn plain_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(name) if name != ".git"))
}

/// The scratch copy a session works in: `<scratch>/session-<pid>-<n>/copy`, and beside it
/// `git`, the git directory the copy's `.git` names when the workspace has one; both removed
/// when dropped.
struct ScratchCopy {
    root: PathBuf,
    dir: PathBuf,
}

impl ScratchCopy {
    /// Makes the copy of `files` of `workspace` under `scratch`: each regular file with its
    /// bytes and its executable bit; a link, a directory or a path that is not plain is left out;
    /// then the workspace's git state ([`Self::carry_git`]).
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
        };
        std::fs::create_dir(&copy.dir)
            .map_err(|error| format!("make {}: {error}", copy.dir.display()))?;
        // Resolved, as metaharness resolves the working directory it is given, so a path the
        // session names under it reads as inside it.
        copy.dir = std::fs::canonicalize(&copy.dir)
            .map_err(|error| format!("resolve {}: {error}", copy.dir.display()))?;
        for file in files.iter().filter(|file| plain_relative(file)) {
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

    /// Every file of the copy that differs from `workspace`: a file `files` named whose bytes
    /// differ or that is gone, and any file the session added; never `.git` or a path under it,
    /// the copy's git state, which no proposal carries. Content that is not UTF-8, or a link the
    /// session left, is an error: a proposal carries text files only.
    fn changes(&self, workspace: &Path, files: &[String]) -> Result<Vec<ProposedFile>, String> {
        let mut found = Vec::new();
        walk(&self.dir, &self.dir, &mut found)?;
        found.retain(|path| !Path::new(path).starts_with(".git"));
        let listed: std::collections::BTreeSet<&str> = files
            .iter()
            .filter(|file| plain_relative(file))
            .map(String::as_str)
            .collect();
        let mut changes = Vec::new();
        for path in &found {
            let now = std::fs::read(self.dir.join(path))
                .map_err(|error| format!("read {path} in the scratch copy: {error}"))?;
            let was = if listed.contains(path.as_str()) {
                std::fs::read(workspace.join(path)).ok()
            } else {
                None
            };
            if was.as_deref() == Some(now.as_slice()) {
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
        let present: std::collections::BTreeSet<&str> = found.iter().map(String::as_str).collect();
        for path in &listed {
            let copied =
                std::fs::symlink_metadata(workspace.join(path)).is_ok_and(|meta| meta.is_file());
            if copied && !present.contains(path) {
                changes.push(ProposedFile {
                    path: (*path).to_owned(),
                    content: None,
                });
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
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

    #[test]
    fn a_proposal_reads_back_from_its_outcome() {
        let proposal = Proposal {
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

    /// `git` with `args` in `dir`, as a fixed identity and never signing; its standard output.
    fn git(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf-8")
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

        /// A copy of `tree` made as a session's is, of the files git lists there.
        fn copy(&self, tree: &Path) -> (ScratchCopy, Vec<String>) {
            let files = listed(tree);
            let copy = ScratchCopy::make(&self.0.join("scratch"), tree, &files).expect("the copy");
            (copy, files)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The files of `tree` a session's copy holds: tracked, and untracked but not ignored.
    fn listed(tree: &Path) -> Vec<String> {
        git(
            tree,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
        )
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
    }

    /// A repository at `dir` in the object format `objects` and the ref format `refs`: two
    /// commits on `work`, a tag, refs packed between them; a config naming a remote, a hook path
    /// and an include, and a hook; a change staged, one not, a tracked file removed, a file
    /// untracked and one ignored.
    fn repository(dir: &Path, objects: &str, refs: &str) {
        let write = |path: &str, text: &str| {
            let at = dir.join(path);
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            std::fs::write(at, text).expect("a file");
        };
        std::fs::create_dir_all(dir).expect("the tree");
        git(
            dir,
            &[
                "init",
                "-q",
                "-b",
                "work",
                &format!("--object-format={objects}"),
                &format!("--ref-format={refs}"),
            ],
        );
        write(".gitignore", "target/\n");
        write("README.md", "a story\n");
        write("src/lib.rs", "pub fn a() {}\n");
        write("tests/t.rs", "#[test]\nfn t() {}\n");
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", "start"]);
        git(dir, &["tag", "v1"]);
        git(dir, &["pack-refs", "--all"]);
        write("src/lib.rs", "pub fn a() -> i32 {\n    1\n}\n");
        git(dir, &["commit", "-q", "-am", "second"]);
        for (key, value) in [
            ("remote.origin.url", "https://example.invalid/r.git"),
            ("core.hooksPath", "hooks-elsewhere"),
            ("include.path", "nowhere.inc"),
        ] {
            git(dir, &["config", key, value]);
        }
        write(".git/hooks/pre-commit", "#!/bin/sh\nexit 1\n");
        write("README.md", "a story, changed\n");
        write("staged.txt", "staged\n");
        git(dir, &["add", "staged.txt"]);
        std::fs::remove_file(dir.join("tests/t.rs")).expect("removed");
        write("notes.txt", "untracked\n");
        write("target/out", "ignored\n");
    }

    /// Every file under `dir` by its path relative to it, with its bytes and the time it was
    /// last written.
    fn stamped(dir: &Path) -> std::collections::BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
        let mut found = Vec::new();
        walk(dir, dir, &mut found).expect("the directory reads");
        found
            .into_iter()
            .map(|path| {
                let at = dir.join(&path);
                let written = std::fs::metadata(&at)
                    .and_then(|meta| meta.modified())
                    .expect("a time");
                (path, (std::fs::read(&at).expect("the file"), written))
            })
            .collect()
    }

    /// Acceptance 2: the copy is a git work tree, the copy its top, at the workspace's `HEAD`,
    /// on its branch, with its status, staged and unstaged diff and log, in either object format
    /// and either ref format.
    #[test]
    fn the_copy_is_a_work_tree_at_the_workspaces_head() {
        for (objects, refs) in [("sha1", "files"), ("sha256", "files"), ("sha1", "reftable")] {
            let scratch = Scratch::new(&format!("head-{objects}-{refs}"));
            let tree = scratch.0.join("tree");
            repository(&tree, objects, refs);
            let (copy, _) = scratch.copy(&tree);
            let case = format!("{objects} {refs}");
            assert_eq!(
                git(&copy.dir, &["rev-parse", "--is-inside-work-tree"]),
                "true\n",
                "{case}"
            );
            assert_eq!(
                git(&copy.dir, &["rev-parse", "--show-toplevel"]).trim_end(),
                copy.dir.to_str().expect("utf-8"),
                "{case}"
            );
            assert_eq!(
                git(&copy.dir, &["log", "-1", "--format=%H"]).trim_end(),
                git(&tree, &["rev-parse", "HEAD"]).trim_end(),
                "{case}"
            );
            let status = git(&tree, &["status", "--porcelain"]);
            for line in [" M README.md", "A  staged.txt", " D tests/t.rs"] {
                assert!(status.contains(line), "{case}: the tree's status: {status}");
            }
            for args in [
                &["status", "--porcelain"][..],
                &["branch", "--show-current"],
                &["log", "--oneline", "-n", "5"],
                &["tag", "--list"],
                &["diff"],
                &["diff", "--cached"],
            ] {
                assert_eq!(
                    git(&copy.dir, args),
                    git(&tree, args),
                    "{case}: git {args:?} in the copy and in the tree"
                );
            }
        }
    }

    /// A linked work tree's copy is at that work tree's own `HEAD` and status, its refs and
    /// objects read from the repository it belongs to.
    #[test]
    fn a_linked_work_trees_copy_is_at_its_own_head() {
        let scratch = Scratch::new("linked");
        let main = scratch.0.join("tree");
        repository(&main, "sha1", "files");
        let linked = scratch.0.join("linked");
        let at = linked.to_str().expect("utf-8");
        git(&main, &["worktree", "add", "-q", "-b", "side", at]);
        std::fs::write(linked.join("side.txt"), "side\n").expect("a file");
        git(&linked, &["add", "side.txt"]);
        git(&linked, &["commit", "-q", "-m", "side"]);
        std::fs::write(linked.join("README.md"), "changed on the side\n").expect("a file");
        let (copy, _) = scratch.copy(&linked);
        for args in [
            &["status", "--porcelain"][..],
            &["branch", "--show-current"],
            &["log", "-1", "--format=%H"],
        ] {
            assert_eq!(git(&copy.dir, args), git(&linked, args), "git {args:?}");
        }
        assert_ne!(
            git(&copy.dir, &["log", "-1", "--format=%H"]),
            git(&main, &["rev-parse", "HEAD"])
        );
    }

    /// The copy's `.git` is one file naming a git directory beside the copy, outside it, so a
    /// session's Glob or Grep there lists that file and never git's own files; the directory has
    /// an empty `hooks/`, the workspace's `info/exclude`, its objects as an alternate, and a
    /// config naming no remote, include, hook path or monitor; it goes with the copy.
    #[test]
    fn the_copys_git_state_is_one_file_naming_a_directory_outside_it() {
        let scratch = Scratch::new("shape");
        let tree = scratch.0.join("tree");
        repository(&tree, "sha1", "files");
        let (copy, _) = scratch.copy(&tree);
        let mut found = Vec::new();
        walk(&copy.dir, &copy.dir, &mut found).expect("the copy reads");
        let under: Vec<&str> = found
            .iter()
            .map(String::as_str)
            .filter(|path| Path::new(path).starts_with(".git"))
            .collect();
        assert_eq!(under, [".git"]);
        let link = std::fs::read_to_string(copy.dir.join(".git")).expect("a file");
        let named = PathBuf::from(
            link.strip_prefix("gitdir: ")
                .and_then(|rest| rest.strip_suffix('\n'))
                .expect("a gitlink"),
        );
        assert_eq!(named, copy.dir.with_file_name("git"));
        assert!(!named.starts_with(&copy.dir));
        assert_eq!(
            std::fs::read_dir(named.join("hooks"))
                .expect("hooks")
                .count(),
            0
        );
        assert!(git(&tree, &["config", "--list", "--local"]).contains("remote.origin.url="));
        assert_eq!(
            std::fs::read_to_string(named.join("config")).expect("a config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n"
        );
        let objects = std::fs::canonicalize(tree.join(".git/objects")).expect("the objects");
        assert_eq!(
            std::fs::read_to_string(named.join("objects/info/alternates")).expect("alternates"),
            format!("{}\n", objects.display())
        );
        assert_eq!(
            std::fs::read(named.join("info/exclude")).expect("an exclude"),
            std::fs::read(tree.join(".git/info/exclude")).expect("the tree's")
        );
        drop(copy);
        assert!(!named.exists(), "removed with the copy");
    }

    /// git run in the copy (a status that may refresh the index, a log, diffs, the branch)
    /// writes nothing of the workspace's git directory: every file of it, the index, refs and
    /// `HEAD` among them, keeps its bytes and its time of last write.
    #[test]
    fn git_in_the_copy_never_writes_the_workspaces_git_directory() {
        for refs in ["files", "reftable"] {
            let scratch = Scratch::new(&format!("unwritten-{refs}"));
            let tree = scratch.0.join("tree");
            repository(&tree, "sha1", refs);
            let before = stamped(&tree.join(".git"));
            for path in ["HEAD", "index", "config"] {
                assert!(before.contains_key(path), "{refs}: {path}");
            }
            let (copy, _) = scratch.copy(&tree);
            std::fs::write(copy.dir.join("src/lib.rs"), "changed in the copy\n").expect("a file");
            for args in [
                &["status"][..],
                &["status", "--porcelain"],
                &["--no-optional-locks", "status", "--short"],
                &["log", "--oneline", "-n", "5"],
                &["diff"],
                &["diff", "--cached"],
                &["branch", "--show-current"],
                &["config", "user.name"],
            ] {
                let _ = std::process::Command::new("git")
                    .args(args)
                    .current_dir(&copy.dir)
                    .output()
                    .expect("git runs");
            }
            drop(copy);
            assert_eq!(stamped(&tree.join(".git")), before, "{refs}");
        }
    }

    /// What a session writes into the copy's git state is no proposal's: a `.git` file
    /// rewritten, or replaced by a directory with a hook and a config, proposes nothing under
    /// `.git`, beside the file the session did change.
    #[test]
    fn a_change_to_the_copys_git_state_is_left_out_of_the_proposal() {
        let scratch = Scratch::new("changes");
        let tree = scratch.0.join("tree");
        repository(&tree, "sha1", "files");
        let (copy, files) = scratch.copy(&tree);
        std::fs::write(copy.dir.join("src/lib.rs"), "changed\n").expect("a file");
        let expected = vec![ProposedFile {
            path: "src/lib.rs".to_owned(),
            content: Some("changed\n".to_owned()),
        }];
        let planted = format!("gitdir: {}\n", tree.join(".git").display());
        std::fs::write(copy.dir.join(".git"), planted).expect("the gitlink rewritten");
        assert_eq!(copy.changes(&tree, &files), Ok(expected.clone()));
        std::fs::remove_file(copy.dir.join(".git")).expect("the gitlink removed");
        std::fs::create_dir_all(copy.dir.join(".git/hooks")).expect("a directory");
        std::fs::write(copy.dir.join(".git/hooks/pre-commit"), "#!/bin/sh\n").expect("a hook");
        std::fs::write(copy.dir.join(".git/config"), "[core]\n\tfsmonitor = x\n").expect("a file");
        assert_eq!(copy.changes(&tree, &files), Ok(expected));
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
}
