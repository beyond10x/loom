//! Adversary pass 2 on `story:metaharness-session-executor` (wave 2026-10-21-w5, U3): the fixes
//! of pass 1 (`through_link`, `ScratchCopy::stale`) and the b10x-harness usage sum.
//!
//! Each case states what the executor's own documentation, `AGENTS.md` or the harness program it
//! drives promises, and drives the executor over a scripted session (metaharness's
//! `ScriptedRunner`): no harness program runs, no model is called and nothing reaches the network.

use std::cell::RefCell;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, ObservationId, PrincipalId, RunBudget, RunId, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, ProposedFile, SessionExecutor,
    TOOLS, plain_relative,
};

const EDIT: &str = "implementation.edit";
const ORIGINAL: &str = "pub fn add() {}\n";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the case's own under the system's temporary directory, holding `workspace/`
/// with `src/lib.rs` and `README.md`, and `scratch/` for the copies; removed when dropped.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-adversary2-u3-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("src")).expect("the workspace");
        std::fs::write(workspace.join("src/lib.rs"), ORIGINAL).expect("a file");
        std::fs::write(workspace.join("README.md"), "a library\n").expect("a file");
        std::fs::create_dir_all(root.join("scratch")).expect("the scratch directory");
        Self { root, workspace }
    }

    fn executor(&self, files: &[&str], runner: Box<dyn ProcessRunner>) -> SessionExecutor {
        let files = files.iter().map(|file| (*file).to_owned()).collect();
        let mut executor =
            SessionExecutor::new(&self.workspace, files, "fix add").with_runner(runner);
        executor.credentials = CredentialSource::None;
        executor.scratch = self.root.join("scratch");
        executor
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// What a case does while the session runs, given the copy's directory; run once, on the first
/// session only.
type Look = Box<dyn FnMut(&Path)>;

/// A scripted Claude Code session: on its first start it runs its [`Look`], then replies naming
/// the edit.
struct Probe(Option<Look>);

impl ProcessRunner for Probe {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        if let Some(mut look) = self.0.take() {
            look(plan.cwd);
        }
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": TOOLS, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "vendor-adv2",
        });
        let said = serde_json::json!({"action": EDIT, "summary": "edited"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 1, "output_tokens": 1}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "usage": {"input_tokens": 1, "output_tokens": 1},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adv2".to_owned()),
        authority_context: AuthorityContext(Value::Null),
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

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![admissible()],
    })
}

fn open(revision: i64) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), vec![admissible()])
}

/// An effect that writes each admitted file into `workspace`, removing one whose content is
/// null, and keeps every file it was handed.
struct Applying {
    workspace: PathBuf,
    handed: RefCell<Vec<ProposedFile>>,
}

impl Applying {
    fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            handed: RefCell::new(Vec::new()),
        }
    }
}

impl EffectPort for Applying {
    fn performs(&self, action: &str) -> bool {
        action == EDIT
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let outcome = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: request.data().action.clone(),
            arguments: request.data().arguments.clone(),
        });
        let files = Proposal::from_outcome(&outcome)
            .expect("a proposed action")
            .expect("a proposal's arguments")
            .files;
        for file in files {
            self.handed.borrow_mut().push(file.clone());
            let at = self.workspace.join(&file.path);
            let written = match file.content {
                Some(text) => std::fs::write(&at, text),
                None => std::fs::remove_file(&at),
            };
            written.map_err(|error| EffectError {
                message: format!("{}: {error}", file.path),
            })?;
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("edited".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

struct Context {
    requests: u64,
    observations: u64,
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
        Some(1)
    }

    fn run_budget(&self) -> Option<RunBudget> {
        None
    }
}

/// One run of `executor` over `case` to where it blocks, the governor open at revision 1 and
/// completing once the effect was applied.
fn run(case: &CaseId, executor: &SessionExecutor, effects: &dyn EffectPort) -> LoopEnd {
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(open(1), 5).chain([open(1).complete("D")]),
    );
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        &governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
        },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// `ScratchCopy::changes` ("never a path that reaches through a link in the workspace") and
/// `AGENTS.md` § Session executor ("one reached through a link in the workspace, is neither
/// copied nor proposed"). Here `src/` of the workspace is replaced by a link to a directory
/// outside it while the session runs, the concurrent workspace change `ScratchCopy::stale`
/// exists for; the session never touches `src/lib.rs`. `changes` drops the copy's `src/lib.rs`
/// as reached through a link, then reads the copied file as gone and proposes its deletion under
/// that same path; `stale` follows the link, finds the same bytes and lets it through. The
/// effect then deletes the file outside the workspace.
#[test]
fn a_directory_turned_into_a_link_mid_session_is_not_deleted_through_it() {
    let fixture = Fixture::new("link-mid-session");
    let outside = fixture.root.join("outside");
    std::fs::create_dir_all(&outside).expect("a directory outside the workspace");
    std::fs::write(outside.join("lib.rs"), ORIGINAL).expect("a file outside the workspace");
    let workspace = fixture.workspace.clone();
    let moved = fixture.root.join("moved-src");
    let link_to = outside.clone();
    let look: Look = Box::new(move |copy: &Path| {
        std::fs::write(copy.join("README.md"), "a library, edited by the session\n")
            .expect("the session's edit");
        std::fs::rename(workspace.join("src"), &moved).expect("src moved away");
        std::os::unix::fs::symlink(&link_to, workspace.join("src")).expect("src now a link");
    });
    let executor = fixture.executor(&["src/lib.rs", "README.md"], Box::new(Probe(Some(look))));
    let effects = Applying::new(&fixture.workspace);
    let case = CaseId("case-link-mid-session".to_owned());

    let end = run(&case, &executor, &effects);

    let through: Vec<ProposedFile> = effects
        .handed
        .borrow()
        .iter()
        .filter(|file| file.path.starts_with("src"))
        .cloned()
        .collect();
    assert!(
        outside.join("lib.rs").is_file() && through.is_empty(),
        "the effect was handed {through:?} through the link at src/ and the file outside the \
         workspace is {} (run ended {:?})",
        if outside.join("lib.rs").is_file() {
            "still there"
        } else {
            "deleted"
        },
        end.outcome
    );
}

/// `plain_relative`'s own documentation: "one plain path relative to a root: no root, no `.` or
/// `..` part". `src/./lib.rs` has a `.` part.
#[test]
fn a_path_with_a_dot_part_is_not_plain() {
    assert!(
        !plain_relative("src/./lib.rs"),
        "plain_relative accepts src/./lib.rs, a path with a `.` part"
    );
}

/// The copy's base is keyed by the spelling the embedder named, the changes by the spelling the
/// walk of the copy finds. A file named both as `src/lib.rs` and as `src/./lib.rs` (both accepted
/// by `plain_relative`) is read back as `src/./lib.rs` gone, so a session that changed only the
/// README proposes deleting `src/./lib.rs`, and the effect deletes `src/lib.rs`. The story: the
/// proposal is what the session changed in its copy.
#[test]
fn a_file_named_under_two_spellings_is_not_deleted_by_a_session_that_left_it_alone() {
    let fixture = Fixture::new("two-spellings");
    let look: Look = Box::new(|copy: &Path| {
        std::fs::write(copy.join("README.md"), "a library, edited by the session\n")
            .expect("the session's edit");
    });
    let executor = fixture.executor(
        &["src/lib.rs", "src/./lib.rs", "README.md"],
        Box::new(Probe(Some(look))),
    );
    let effects = Applying::new(&fixture.workspace);
    let case = CaseId("case-two-spellings".to_owned());

    let end = run(&case, &executor, &effects);

    let handed: Vec<String> = effects
        .handed
        .borrow()
        .iter()
        .map(|file| {
            format!(
                "{} {}",
                file.path,
                file.content.as_deref().map_or("removed", |_| "written")
            )
        })
        .collect();
    assert_eq!(
        std::fs::read_to_string(fixture.workspace.join("src/lib.rs")).ok(),
        Some(ORIGINAL.to_owned()),
        "the effect was handed {handed:?} and src/lib.rs, which the session never touched, is \
         gone (run ended {:?})",
        end.outcome
    );
}

/// A scripted b10x-harness session: it edits `src/lib.rs` in its copy and prints b10x-harness's
/// own lines, one `usage` line of 100 input tokens of which 40 cached, 20 output.
struct B10xSession;

impl ProcessRunner for B10xSession {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        std::fs::write(plan.cwd.join("src/lib.rs"), "pub fn add() -> i32 { 1 }\n")?;
        let said = serde_json::json!({"action": EDIT, "summary": "fixed add"}).to_string();
        let lines = [
            serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                               "credential_source": "none"}),
            serde_json::json!({"kind": "text-delta", "text": said}),
            serde_json::json!({"kind": "usage", "model": "synthetic", "input_tokens": 100,
                               "output_tokens": 20, "cached_input_tokens": 40}),
            serde_json::json!({"kind": "cost", "micro_usd": 1234}),
            serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}),
        ];
        ScriptedRunner::new(
            lines
                .into_iter()
                .map(|line| ScriptStep::line(line.to_string()))
                .collect(),
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

/// The executor's rule: an input figure that "holds the cached tokens it also states apart" is
/// charged less them, "so no token is counted twice". b10x-harness 0.15.1, the version
/// metaharness 0.13.0 pins, states `input_tokens` as the whole on every wire: `harness_wire::Usage`
/// ("`input_tokens` is the whole, and the cache figures are parts of it"; the wire that reports
/// its three input figures disjointly has "its projection sums them" before writing it). So a
/// b10x-harness session on `anthropic-messages` reporting 100
/// input tokens with 40 cached is charged 60, as on `openai-responses`.
#[test]
fn a_b10x_session_on_the_anthropic_wire_is_charged_its_input_less_the_cached_tokens() {
    let fixture = Fixture::new("b10x-anthropic");
    let token = fixture.root.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let mut executor = fixture
        .executor(&["src/lib.rs"], Box::new(B10xSession))
        .of_kind(Kind::B10x);
    executor.endpoint = Some(Endpoint {
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        wire: "anthropic-messages".to_owned(),
        model: "synthetic".to_owned(),
        token_file: token,
        token_pointer: Some("/token".to_owned()),
    });
    let case = CaseId("case-b10x-anthropic".to_owned());

    let step: ExecutorStep = executor.run_with_usage(&commission(&case), &frontier(&case));

    let report = executor.last_report().expect("a report");
    assert!(
        Proposal::from_outcome(&step.outcome).is_some(),
        "the session proposed nothing: {:?}",
        report.error
    );
    let usage = step.usage.expect("the step's usage");
    assert_eq!(
        (usage.input_tokens, usage.output_tokens),
        (60, 20),
        "a b10x-harness session on anthropic-messages is charged its whole input, cached tokens \
         included (cache_read {:?})",
        report.cache_read
    );
}
