//! Acceptance for `story:metaharness-session-executor`: the session executor returns a session's
//! change as a `ProposedAction`, which Commission revalidates and an effect applies, and the
//! workspace is untouched until the effect applies it.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the records a Claude Code
//! session prints): no harness program runs, no model is called and nothing reaches the network.
//! A scripted session makes its change in the scratch copy it is started in, as a real one does
//! through its file tools.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    BudgetLimit, CaseId, ChargedUsage, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep,
    ExecutorUsage, Frontier, FrontierAction, FrontierData, FrontierId, ObservationId, PrincipalId,
    RunBudget, RunBudgetBreach, RunId, RunOutcome, RunOutcomeCaseMovedOn, RunOutcomeIdleBound,
    RunOutcomeSuspended, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{EXECUTOR_SOURCE, LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_session_executor::metaharness::protocol::{
    EVENT_FORMAT, parse_event_line, project_document, summarize,
};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner, VendorSource,
    project_vendor_record,
};
use b10x_loom_session_executor::model::agent::ToolPolicy;
use b10x_loom_session_executor::model::session::{
    AgentSessionState, ChangedFile, SessionChangeProposed, SessionKind,
};
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, Proposal, ProposedFile, SessionExecutor,
    ToolPolicyDefaults as _,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A workspace of three files, and a scratch directory for the copies, in a directory of the
/// test's own under the system's temporary directory; removed when dropped. Not under the
/// repository: metaharness refuses a Claude session whose copy has memory files above it.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-proposes-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        for (path, text) in [
            (
                "src/lib.rs",
                "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
            ),
            ("old.rs", "// left over\n"),
            ("README.md", "a library\n"),
        ] {
            let at = workspace.join(path);
            std::fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            std::fs::write(at, text).expect("a fixture file");
        }
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        Self { root, workspace }
    }

    fn files() -> Vec<String> {
        ["src/lib.rs", "old.rs", "README.md"]
            .map(str::to_owned)
            .to_vec()
    }

    /// An executor over the workspace's three files, its sessions the ones `script` plays.
    fn executor(&self, script: &Script) -> SessionExecutor {
        let mut executor = SessionExecutor::new(&self.workspace, Self::files(), "fix add")
            .with_runner(Box::new(script.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.root.join("scratch");
        executor
    }

    /// Every file of the workspace, by its path relative to it, with its bytes.
    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).expect("the directory reads") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    walk(root, &path, into);
                } else {
                    let relative = path.strip_prefix(root).expect("under the root");
                    into.insert(
                        relative.display().to_string(),
                        std::fs::read(&path).expect("the file"),
                    );
                }
            }
        }
        let mut found = BTreeMap::new();
        walk(&self.workspace, &self.workspace, &mut found);
        found
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A scripted Claude Code session: it makes `edits` in the copy it is started in (a `None`
/// content removes the file), writes `meanwhile` into the live workspace when there is one, as
/// somebody else would while the session runs, and replies naming `action` with the next of
/// `summaries`. Its records state `vendor` as the session's id, and what it spent: 1000 input
/// and 50 output tokens, 2000 ms and USD 0.0003. It leaves the vendor record `kept` says.
#[derive(Clone)]
struct Script {
    edits: Vec<(String, Option<String>)>,
    meanwhile: Option<(PathBuf, String)>,
    action: String,
    summaries: Vec<String>,
    vendor: Option<String>,
    kept: Kept,
    calls: Rc<Cell<usize>>,
}

/// The vendor record a scripted session leaves where metaharness keeps one, as a program writes
/// what it prints there.
#[derive(Clone)]
enum Kept {
    /// None: a `ScriptedRunner` writes no record, so the executor has only the events.
    Nothing,
    /// The records it prints, one line each.
    Printed,
    /// These bytes, whatever it prints.
    Bytes(Vec<u8>),
}

const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

impl Script {
    /// Fixes `add` in `src/lib.rs`, removes `old.rs` and adds `src/more.rs`.
    fn fixing() -> Self {
        Self {
            edits: vec![
                ("src/lib.rs".to_owned(), Some(FIXED.to_owned())),
                ("old.rs".to_owned(), None),
                (
                    "src/more.rs".to_owned(),
                    Some("pub fn more() {}\n".to_owned()),
                ),
            ],
            meanwhile: None,
            action: EDIT.to_owned(),
            summaries: vec!["fixed add".to_owned()],
            vendor: Some("vendor-session-1".to_owned()),
            kept: Kept::Nothing,
            calls: Rc::new(Cell::new(0)),
        }
    }

    /// Fixes `add` in `src/lib.rs` and nothing else.
    fn fixing_only_add() -> Self {
        Self {
            edits: vec![("src/lib.rs".to_owned(), Some(FIXED.to_owned()))],
            ..Self::fixing()
        }
    }

    fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl ProcessRunner for Script {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        for (path, content) in &self.edits {
            let at = plan.cwd.join(path);
            match content {
                Some(text) => {
                    std::fs::create_dir_all(at.parent().expect("a parent"))?;
                    std::fs::write(at, text)?;
                }
                None => std::fs::remove_file(at)?,
            }
        }
        if let Some((path, text)) = &self.meanwhile {
            std::fs::write(path, text)?;
        }
        let mut init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default",
        });
        if let Some(vendor) = &self.vendor {
            init["session_id"] = serde_json::Value::String(vendor.clone());
        }
        let summary = &self.summaries[call % self.summaries.len()];
        let said = if self.action.is_empty() {
            summary.clone()
        } else {
            serde_json::json!({"action": self.action, "summary": summary}).to_string()
        };
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 1000, "output_tokens": 50}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 2000, "total_cost_usd": 0.0003,
            "usage": {"input_tokens": 1000, "output_tokens": 50},
        });
        let printed = [init.to_string(), reply.to_string(), result.to_string()];
        match &self.kept {
            Kept::Nothing => {}
            Kept::Printed => std::fs::write(plan.transcript, printed.join("\n") + "\n")?,
            Kept::Bytes(bytes) => std::fs::write(plan.transcript, bytes)?,
        }
        ScriptedRunner::of_lines(printed, ScriptedLog::new()).start(plan)
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId, id: u64) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(id)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![admissible()],
    })
}

fn admissible() -> FrontierAction {
    FrontierAction {
        action: EDIT.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

/// The case open at `revision`, its frontier listing the edit admissible.
fn open(revision: i64) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), vec![admissible()])
}

/// The files a proposed action's arguments carry, as the effect reads them.
fn proposed_files(request: &ActionRequestData) -> Vec<ProposedFile> {
    let outcome = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: request.action.clone(),
        arguments: request.arguments.clone(),
    });
    Proposal::from_outcome(&outcome)
        .expect("a proposed action")
        .expect("a proposal's arguments")
        .files
}

/// An effect that performs the edit. Applying, it writes each admitted file into `workspace`
/// (removing one whose content is null), after keeping the workspace's bytes as it found them.
/// Not applying, it only counts.
struct Effects {
    workspace: Option<PathBuf>,
    invoked: RefCell<Vec<ActionRequestData>>,
    found: RefCell<Vec<BTreeMap<String, Vec<u8>>>>,
}

impl Effects {
    fn counting() -> Self {
        Self {
            workspace: None,
            invoked: RefCell::new(Vec::new()),
            found: RefCell::new(Vec::new()),
        }
    }

    fn applying(fixture: &Fixture) -> Self {
        Self {
            workspace: Some(fixture.workspace.clone()),
            ..Self::counting()
        }
    }

    fn invoked(&self) -> usize {
        self.invoked.borrow().len()
    }
}

impl EffectPort for Effects {
    fn performs(&self, action: &str) -> bool {
        action == EDIT
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.borrow_mut().push(request.data().clone());
        if let Some(workspace) = &self.workspace {
            let mut found = BTreeMap::new();
            for path in ["src/lib.rs", "old.rs", "README.md", "src/more.rs"] {
                if let Ok(bytes) = std::fs::read(workspace.join(path)) {
                    found.insert(path.to_owned(), bytes);
                }
            }
            self.found.borrow_mut().push(found);
            for file in proposed_files(request.data()) {
                let at = workspace.join(&file.path);
                let written = match file.content {
                    Some(text) => std::fs::write(&at, text),
                    None => std::fs::remove_file(&at),
                };
                written.map_err(|error| EffectError {
                    message: format!("{}: {error}", file.path),
                })?;
            }
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("edited".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// New ids from counters, one trusted time, the step budget and the run budget given.
struct Context {
    requests: u64,
    observations: u64,
    steps: Option<usize>,
    budget: Option<RunBudget>,
}

impl Context {
    fn new(steps: Option<usize>, budget: Option<RunBudget>) -> Self {
        Self {
            requests: 0,
            observations: 0,
            steps,
            budget,
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-21T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.steps
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.budget.clone()
    }
}

/// `run_until_blocked` of `executor` on `governor`, which holds `case`, with `effects`.
fn run(
    governor: &FakeGovernor,
    case: &CaseId,
    executor: &SessionExecutor,
    effects: &Effects,
    mut context: Context,
) -> LoopEnd {
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// The JSON a proposed file is carried as.
fn carried(path: &str, content: Option<&str>) -> Value {
    Value::Object(vec![
        ("path".to_owned(), Value::Text(path.to_owned())),
        (
            "content".to_owned(),
            content.map_or(Value::Null, |text| Value::Text(text.to_owned())),
        ),
    ])
}

/// Whether `id` is a UUID as Loom writes one: five groups of lowercase hex digits.
fn well_formed(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    groups.iter().map(|group| group.len()).eq([8, 4, 4, 4, 12])
        && groups.iter().all(|group| {
            group
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
}

/// Over a scripted session on the fixture workspace, the executor returns a `ProposedAction`
/// whose arguments are `{files}` only, a removed file's content null; it mints a new session id
/// for each call and records each session under the real commission and frontier ids; and the
/// workspace's bytes are as they were.
#[test]
fn a_session_change_returns_as_a_proposed_action() {
    let fixture = Fixture::new("proposed");
    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let before = fixture.bytes();
    let case = CaseId("case-proposed".to_owned());

    let first = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));
    let second = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf2));

    let expected = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: EDIT.to_owned(),
        arguments: b10x_loom_commission::model::responsibility::ProposedActionArguments(
            Value::Object(vec![(
                "files".to_owned(),
                Value::Array(vec![
                    carried("old.rs", None),
                    carried("src/lib.rs", Some(FIXED)),
                    carried("src/more.rs", Some("pub fn more() {}\n")),
                ]),
            )]),
        ),
    });
    assert_eq!(
        first.outcome,
        expected,
        "{:?}",
        executor.last_report().and_then(|report| report.error)
    );
    assert_eq!(second.outcome, expected);
    assert_eq!(script.calls(), 2);
    assert_eq!(
        fixture.bytes(),
        before,
        "the workspace is untouched: the session worked in its copy"
    );

    let sessions = executor.sessions();
    assert_eq!(sessions.len(), 2, "one session per call");
    let ids: Vec<&str> = sessions
        .iter()
        .map(|row| row.session_id.0.0.as_str())
        .collect();
    assert_ne!(ids[0], ids[1], "each call mints a new session id");
    assert!(ids.iter().all(|id| well_formed(id)), "{ids:?}");
    let files = vec![
        ChangedFile {
            path: "old.rs".to_owned(),
            content: None,
        },
        ChangedFile {
            path: "src/lib.rs".to_owned(),
            content: Some(FIXED.to_owned()),
        },
        ChangedFile {
            path: "src/more.rs".to_owned(),
            content: Some("pub fn more() {}\n".to_owned()),
        },
    ];
    for (row, frontier_id) in sessions.iter().zip([0xf1, 0xf2]) {
        assert_eq!(row.kind, SessionKind::ClaudeCode);
        assert_eq!(
            row.commission_id.0.0,
            uuid(0xc1).0,
            "the real commission id"
        );
        assert_eq!(
            row.frontier_id.0.0,
            uuid(frontier_id).0,
            "the real frontier id"
        );
        assert_eq!(row.vendor_session_id.as_deref(), Some("vendor-session-1"));
        assert_eq!(row.action, EDIT);
        assert_eq!(row.files, files);
        assert_eq!(row.state, AgentSessionState::Ended);
    }
    let proposed: Vec<SessionChangeProposed> = executor.proposed();
    assert_eq!(proposed.len(), 2);
    assert_eq!(proposed[0].session_id, sessions[0].session_id);
    assert_eq!(proposed[1].files, files);
}

/// A session that changed nothing proposes an empty list of files, and a session that fails
/// proposes nothing, suspends for `ExternalAvailability` and is recorded as failed: no action, no
/// files, a `failure`.
#[test]
fn an_empty_session_proposes_no_files_and_a_failed_one_suspends() {
    let fixture = Fixture::new("empty");
    let case = CaseId("case-empty".to_owned());
    let empty = Script {
        edits: Vec::new(),
        ..Script::fixing()
    };
    let executor = fixture.executor(&empty);
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));
    let proposal = Proposal::from_outcome(&step.outcome)
        .expect("a proposed action")
        .expect("a proposal");
    assert_eq!(proposal.files, Vec::new());
    assert_eq!(executor.sessions().len(), 1);

    // A session whose reply names no action has failed.
    let failing = Script {
        action: String::new(),
        summaries: vec!["I could not decide".to_owned()],
        ..Script::fixing()
    };
    let executor = fixture.executor(&failing);
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf2));
    assert!(
        matches!(
            step.outcome,
            ExecutorOutcome::Suspended(ref suspended)
                if matches!(suspended.reason, SuspensionReason::ExternalAvailability(_))
        ),
        "{:?}",
        step.outcome
    );
    let rows = executor.sessions();
    assert_eq!(rows.len(), 1, "a failed session is recorded: {rows:?}");
    assert!(rows[0].failure.is_some(), "with its failure: {rows:?}");
    assert_eq!(rows[0].action, "", "naming no action");
    assert!(rows[0].files.is_empty(), "and no file");
    assert!(executor.proposed().is_empty());
}

/// Run through `run_until_blocked`: the case moves from revision 1 to 2 while the session runs,
/// so revalidation finds the proposal bound to a revision the case is no longer at; it is
/// refused and the effect is never invoked.
#[test]
fn a_proposal_on_a_moved_case_is_refused_at_revalidation() {
    let fixture = Fixture::new("moved");
    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let effects = Effects::applying(&fixture);
    let case = CaseId("case-moved".to_owned());
    let governor = FakeGovernor::new();
    // The iteration reads the revision, the completion and the frontier at 1; the session runs;
    // revalidation reads the revision at 2.
    governor.script(case.clone(), repeat_n(open(1), 3).chain([open(2)]));
    let before = fixture.bytes();

    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(None, None),
    );

    assert_eq!(script.calls(), 1);
    assert_eq!(effects.invoked(), 0, "a stale proposal is never invoked");
    assert!(end.admitted.is_empty());
    assert_eq!(end.requests.len(), 1);
    assert_eq!(end.requests[0].request.expected_case_revision, 1);
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 1,
            current_case_revision: 2,
        })
    );
    assert_eq!(fixture.bytes(), before, "nothing reached the workspace");
}

/// A workspace file written while the session runs is never proposed back. Loom's governor does
/// not derive the case revision from the workspace's bytes, so the write cannot be shown to move
/// it: the executor reads the change against the bytes its copy was made from, the workspace as
/// the frontier was handed, and the concurrent write stays the workspace's.
#[test]
fn a_concurrent_workspace_write_refuses_the_stale_proposal() {
    let fixture = Fixture::new("concurrent");
    let case = CaseId("case-concurrent".to_owned());
    let written = "a library, edited by somebody else\n";
    let script = Script {
        meanwhile: Some((fixture.workspace.join("README.md"), written.to_owned())),
        ..Script::fixing_only_add()
    };
    let executor = fixture.executor(&script);

    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));

    let proposal = Proposal::from_outcome(&step.outcome)
        .expect("a proposed action")
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        proposal.files,
        vec![ProposedFile {
            path: "src/lib.rs".to_owned(),
            content: Some(FIXED.to_owned()),
        }],
        "only the session's change: the README written meanwhile is not proposed back"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.workspace.join("README.md")).expect("the README"),
        written,
        "the concurrent write stands"
    );
}

/// A file the session changed that was also written in the workspace while it ran, or a file it
/// added that now exists there, makes the call propose nothing: it suspends for
/// `ExternalAvailability` naming the path, is recorded as failed with no files, and the
/// concurrent write stands.
#[test]
fn a_concurrent_write_to_a_file_the_session_changed_suspends_the_step() {
    let fixture = Fixture::new("stale");
    let case = CaseId("case-stale".to_owned());
    let written = "pub fn add(a: i32, b: i32) -> i32 {\n    b - a\n}\n";
    for (path, frontier_id) in [("src/lib.rs", 0xf1), ("src/more.rs", 0xf2)] {
        let script = Script {
            meanwhile: Some((fixture.workspace.join(path), written.to_owned())),
            ..Script::fixing()
        };
        let executor = fixture.executor(&script);

        let step = executor.run_with_usage(&commission(&case), &frontier(&case, frontier_id));

        let why = match &step.outcome {
            ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
                SuspensionReason::ExternalAvailability(Value::Text(why)) => why.clone(),
                other => panic!("{path}: suspended for {other:?}"),
            },
            other => panic!("{path}: a stale change was not refused: {other:?}"),
        };
        assert!(why.contains(path), "{path}: {why}");
        let rows = executor.sessions();
        assert_eq!(
            rows.len(),
            1,
            "{path}: a stale change is recorded: {rows:?}"
        );
        assert!(rows[0].failure.is_some(), "{path}: as failed: {rows:?}");
        assert!(rows[0].files.is_empty(), "{path}: proposing no file");
        assert!(executor.proposed().is_empty());
        assert_eq!(
            std::fs::read_to_string(fixture.workspace.join(path)).expect("the file"),
            written,
            "{path}: the concurrent write stands"
        );
    }
}

/// A named file spelled other than as one plain relative path (a `.` part, an empty part, a
/// trailing `/`, a root) fails the step before any session starts: it suspends with
/// `ExternalAvailability` naming the spelling, records nothing and leaves the workspace as it was.
#[test]
fn a_named_file_not_spelled_plainly_suspends_before_any_session() {
    let fixture = Fixture::new("unplain");
    let case = CaseId("case-unplain".to_owned());
    let before = fixture.bytes();
    for (spelling, frontier_id) in [
        ("src/./lib.rs", 0xf1),
        ("src//lib.rs", 0xf2),
        ("src/", 0xf3),
        ("/src/lib.rs", 0xf4),
    ] {
        let script = Script::fixing();
        let mut executor = fixture.executor(&script);
        executor.files.push(spelling.to_owned());

        let step = executor.run_with_usage(&commission(&case), &frontier(&case, frontier_id));

        let why = match &step.outcome {
            ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
                SuspensionReason::ExternalAvailability(Value::Text(why)) => why.clone(),
                other => panic!("{spelling}: suspended for {other:?}"),
            },
            other => panic!("{spelling}: the step was not refused: {other:?}"),
        };
        assert!(why.contains(spelling), "{spelling}: {why}");
        assert_eq!(script.calls(), 0, "{spelling}: a session started");
        assert!(executor.sessions().is_empty(), "{spelling}: recorded");
        assert_eq!(fixture.bytes(), before, "{spelling}: the workspace changed");
    }
}

/// With a test effect, the admitted proposal's files reach the workspace only through the
/// effect: the effect finds the workspace as it was, and leaves it changed.
#[test]
fn the_effect_applies_the_admitted_change() {
    let fixture = Fixture::new("applied");
    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let effects = Effects::applying(&fixture);
    let case = CaseId("case-applied".to_owned());
    let governor = FakeGovernor::new();
    // The iteration (3 reads) and revalidation (2) at 1, then the case complete.
    governor.script(
        case.clone(),
        repeat_n(open(1), 5).chain([open(1).complete("D")]),
    );
    let before = fixture.bytes();

    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(None, None),
    );

    assert_eq!(effects.invoked(), 1, "{:?}", end.outcome);
    assert!(
        matches!(end.outcome, RunOutcome::Completed(_)),
        "{:?}",
        end.outcome
    );
    let found = effects.found.borrow();
    assert_eq!(
        found[0],
        before
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect::<BTreeMap<_, _>>(),
        "the effect found the workspace untouched by the session"
    );
    let after = fixture.bytes();
    assert_eq!(
        after.get("src/lib.rs").map(Vec::as_slice),
        Some(FIXED.as_bytes())
    );
    assert_eq!(
        after.get("src/more.rs").map(Vec::as_slice),
        Some("pub fn more() {}\n".as_bytes())
    );
    assert!(!after.contains_key("old.rs"), "the removal applied");
    assert_eq!(after.get("README.md"), before.get("README.md"));
}

/// The digest of a transcript, lowercase hex, computed here apart from the executor.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The step's `ExecutorStep` carries the transcript's digest and the session's summary; the
/// runtime delivers both in the step's observation, and no evidence is submitted for them.
#[test]
fn the_session_trace_is_never_evidence() {
    let fixture = Fixture::new("trace");
    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let case = CaseId("case-trace".to_owned());

    let step: ExecutorStep = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));
    let transcript = executor.last_report().expect("a report").transcript;
    assert!(!transcript.is_empty(), "the session left a transcript");
    assert_eq!(step.transcript_digest, Some(sha256_hex(&transcript)));
    assert_eq!(step.summary.as_deref(), Some("fixed add"));

    let effects = Effects::counting();
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(open(1), 5).chain([open(1).complete("D")]),
    );
    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(None, None),
    );
    assert_eq!(effects.invoked(), 1, "{:?}", end.outcome);

    assert!(governor.evidence().is_empty(), "no evidence is submitted");
    let observed: Vec<Vec<(String, Value)>> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == EXECUTOR_SOURCE)
        .filter_map(|observation| match observation.payload {
            Value::Object(fields) => Some(fields),
            _ => None,
        })
        .collect();
    assert_eq!(observed.len(), 1);
    let field = |name: &str| {
        observed[0]
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
    };
    let transcript = executor.last_report().expect("a report").transcript;
    assert_eq!(
        field("transcript_digest"),
        Some(Value::Text(sha256_hex(&transcript)))
    );
    assert_eq!(field("summary"), Some(Value::Text("fixed add".to_owned())));
    let Some(Value::Object(arguments)) = field("arguments") else {
        panic!("the observation carries the proposal's arguments: {observed:?}");
    };
    let names: Vec<&str> = arguments.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["files"], "the arguments are the files only");
}

/// `story:run-record-names-agent-sessions`: the report's trace is metaharness's `trace-ir/1`
/// projection of the session's record, summarized by `summarize`, under the step's digest: the
/// events the executor received when the program left no vendor record, the vendor record through
/// its kind's reader when it left one. A record metaharness does not project keeps its digest and
/// says why, and the call proposes as before.
#[test]
fn the_session_record_is_summarized_by_metaharness() {
    let fixture = Fixture::new("summarized");
    let case = CaseId("case-summarized".to_owned());

    let executor = fixture.executor(&Script::fixing());
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));
    let report = executor.last_report().expect("a report");
    let trace = report.trace.expect("a session that started has a trace");
    assert_eq!(trace.source.as_deref(), Some(EVENT_FORMAT));
    assert_eq!(trace.transcript_digest, step.transcript_digest);
    let lines: Vec<_> = std::str::from_utf8(&report.transcript)
        .expect("the events are UTF-8")
        .lines()
        .map(|line| parse_event_line(line).expect("an event line"))
        .collect();
    let summary = trace.summary.expect("the events project");
    assert_eq!(
        summary,
        summarize(&project_document(&lines, &report.transcript))
    );
    assert_eq!(summary.turns, Some(1));
    assert_eq!(summary.cost_usd, Some(0.0003));

    let printed = Script {
        kept: Kept::Printed,
        ..Script::fixing()
    };
    let executor = fixture.executor(&printed);
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf2));
    let report = executor.last_report().expect("a report");
    let trace = report.trace.expect("a trace");
    assert_eq!(trace.source.as_deref(), Some("claude-code-stream-json"));
    assert_eq!(trace.transcript_digest, step.transcript_digest);
    let record = fixture.root.join("transcript.jsonl");
    std::fs::write(&record, &report.transcript).expect("the record, again");
    let document =
        project_vendor_record(&record, VendorSource::ClaudeCodeStreamJson).expect("it projects");
    assert_eq!(trace.summary, Ok(summarize(&document)));
    assert_eq!(
        trace.transcript_digest.as_deref(),
        Some(document.transcript_digest.as_str())
    );

    let unreadable = Script {
        kept: Kept::Bytes(b"not a session record\n".to_vec()),
        ..Script::fixing()
    };
    let executor = fixture.executor(&unreadable);
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf3));
    let report = executor.last_report().expect("a report");
    assert_eq!(report.transcript, b"not a session record\n");
    let trace = report.trace.expect("a trace");
    assert_eq!(trace.source.as_deref(), Some("claude-code-stream-json"));
    assert_eq!(trace.transcript_digest, step.transcript_digest);
    let why = trace.summary.expect_err("the record does not project");
    assert!(
        why.contains("did not project the claude-code-stream-json record"),
        "{why}"
    );
    Proposal::from_outcome(&step.outcome)
        .expect("the call still proposes")
        .unwrap_or_else(|why| panic!("{why}"));
}

/// The executor implements `run_with_usage`: each session's tokens and cost reach the Run's
/// charges, and a session that takes the run past its cost limit suspends it with `RunBudget`.
#[test]
fn a_session_usage_is_charged_to_the_run() {
    let fixture = Fixture::new("usage");
    let case = CaseId("case-usage".to_owned());
    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let step = executor.run_with_usage(&commission(&case), &frontier(&case, 0xf1));
    assert_eq!(
        step.usage,
        Some(ExecutorUsage {
            input_tokens: 1000,
            output_tokens: 50,
            duration_ms: 2000,
            cost_micro_usd: Some(300),
        })
    );

    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let effects = Effects::counting();
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(1)]);
    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(Some(3), None),
    );
    assert_eq!(script.calls(), 3, "{:?}", end.outcome);
    assert_eq!(
        end.charges.run,
        ChargedUsage {
            trials: 3,
            input_tokens: 3000,
            output_tokens: 150,
            duration_ms: 6000,
            cost_micro_usd: Some(900),
        },
        "each session's usage is charged to the run"
    );

    let script = Script::fixing();
    let executor = fixture.executor(&script);
    let effects = Effects::counting();
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(1)]);
    let budget = RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: Some(500),
        max_tokens: None,
        deadline: None,
        per_action: Vec::new(),
    };
    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(Some(3), Some(budget)),
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxCostMicroUsd,
                max: 500,
                charged: Some(600),
            }),
        }),
        "the second session's cost takes the run past its limit"
    );
    assert_eq!(script.calls(), 2);
    assert_eq!(
        effects.invoked(),
        1,
        "the breaching session's proposal is not acted on"
    );
}

/// Two sessions that propose the same files with different summaries are the same request for
/// the idle check: with no step budget, the run ends at the idle bound.
#[test]
fn a_repeated_proposal_keeps_its_identity() {
    let fixture = Fixture::new("repeated");
    let script = Script {
        summaries: ["fixed add", "fixed add again", "add is fixed"]
            .map(str::to_owned)
            .to_vec(),
        ..Script::fixing()
    };
    let executor = fixture.executor(&script);
    let effects = Effects::counting();
    let case = CaseId("case-repeated".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(1)]);

    let end = run(
        &governor,
        &case,
        &executor,
        &effects,
        Context::new(None, None),
    );

    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
    assert_eq!(script.calls(), 3);
    let arguments: Vec<&Value> = end
        .requests
        .iter()
        .map(|made| &made.request.arguments.0)
        .collect();
    assert_eq!(arguments.len(), 3);
    assert!(arguments.windows(2).all(|pair| pair[0] == pair[1]));
    let summaries: Vec<String> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == EXECUTOR_SOURCE)
        .filter_map(|observation| match observation.payload {
            Value::Object(fields) => fields
                .into_iter()
                .find(|(name, _)| name == "summary")
                .and_then(|(_, value)| match value {
                    Value::Text(text) => Some(text),
                    _ => None,
                }),
            _ => None,
        })
        .collect();
    assert_eq!(
        summaries,
        vec!["fixed add", "fixed add again", "add is fixed"],
        "each session's summary is observed, outside the request"
    );
}
