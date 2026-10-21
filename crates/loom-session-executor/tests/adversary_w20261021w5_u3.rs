//! Adversary pass 1 on `story:metaharness-session-executor` (wave 2026-10-21-w5, U3).
//!
//! Each case states what the story, the session domain or the executor's own documentation
//! promises, and drives the executor over a scripted session (metaharness's `ScriptedRunner`): no
//! harness program runs, no model is called and nothing reaches the network.

use std::cell::RefCell;
use std::iter::repeat_n;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, ObservationId, PrincipalId, RunBudget, RunId, RunOutcome, RunOutcomeSuspended,
    SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, Proposal, ProposedFile, SessionExecutor, TOOLS,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the case's own under the system's temporary directory (metaharness refuses a
/// Claude session whose copy has memory files above it, so not under the repository), holding
/// `workspace/` with `src/lib.rs` and `README.md`, and `scratch/` for the copies; removed when
/// dropped.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "session-executor-adversary-u3-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(workspace.join("src")).expect("the workspace");
        std::fs::write(workspace.join("src/lib.rs"), "pub fn add() {}\n").expect("a file");
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

/// What a case does in the copy while the session runs, given the copy's directory.
type Look = Box<dyn FnMut(&Path)>;

/// A scripted Claude Code session: it runs its [`Look`] in the copy it is started in, then
/// replies naming the edit; it reports tokens and a duration and no cost.
struct Probe(Look);

impl ProcessRunner for Probe {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        (self.0)(plan.cwd);
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": TOOLS, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "vendor-adv",
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

/// A Claude Code program that cannot be started: what the executor meets when `claude` is not
/// installed or not runnable.
struct Absent;

impl ProcessRunner for Absent {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, _plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "claude is not installed",
        ))
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adv".to_owned()),
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

/// The files the step's proposed action carries; none for a step that proposes nothing.
fn proposed(step: &ExecutorStep) -> Option<Vec<ProposedFile>> {
    Proposal::from_outcome(&step.outcome).map(|read| read.expect("a proposal's arguments").files)
}

/// An effect that writes each admitted file into `workspace`, removing one whose content is
/// null, and counts its invocations.
struct Applying {
    workspace: PathBuf,
    invoked: RefCell<usize>,
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
        *self.invoked.borrow_mut() += 1;
        let outcome = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: request.data().action.clone(),
            arguments: request.data().arguments.clone(),
        });
        let files = Proposal::from_outcome(&outcome)
            .expect("a proposed action")
            .expect("a proposal's arguments")
            .files;
        for file in files {
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
    budget: Option<RunBudget>,
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
        self.budget.clone()
    }
}

fn run(
    governor: &FakeGovernor,
    case: &CaseId,
    executor: &SessionExecutor,
    effects: &dyn EffectPort,
    budget: Option<RunBudget>,
) -> LoopEnd {
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x101))));
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
            budget,
        },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// The session domain and the module documentation: a session that fails proposes nothing and
/// the call returns `Suspended` with `ExternalAvailability`. ExecutorUsage: "A cost the executor
/// could not price is absent, never zero"; a session that never started spent nothing, a known
/// cost. Under a run budget with a cost limit, a session whose program cannot start must suspend
/// the Run for `ExternalAvailability`, not for `RunBudget` with an unknown charge: the executor's
/// usage of a session that never ran must not make the run's cost unknown.
#[test]
fn a_session_that_never_started_suspends_for_availability_under_a_cost_limit() {
    let fixture = Fixture::new("absent");
    let executor = fixture.executor(&["src/lib.rs"], Box::new(Absent));
    let case = CaseId("case-absent".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(1)]);
    let budget = RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: Some(1_000_000),
        per_action: Vec::new(),
    };
    let effects = Applying {
        workspace: fixture.workspace.clone(),
        invoked: RefCell::new(0),
    };

    let end = run(&governor, &case, &executor, &effects, Some(budget));

    let reported = executor.last_report().and_then(|report| report.error);
    assert!(
        matches!(
            end.outcome,
            RunOutcome::Suspended(RunOutcomeSuspended {
                reason: SuspensionReason::ExternalAvailability(_)
            })
        ),
        "a session whose program could not start ({reported:?}) suspends the run for \
         ExternalAvailability; the run ended {:?} with charges {:?}",
        end.outcome,
        end.charges.run
    );
}

/// The executor's own rule (`plain_relative`, `decide`, `decide_codex` and `changes`): no
/// proposal carries a path with a part named `.git`. A session that plants a git directory below
/// the copy's top (a Codex `Bash` command or a b10x-harness loop can; Claude Code's Edit and
/// Write are refused there) must not have it proposed for the effect to write into the workspace.
#[test]
fn a_git_directory_planted_below_the_top_is_never_proposed() {
    let fixture = Fixture::new("nested-git");
    let look: Look = Box::new(|copy: &Path| {
        std::fs::write(copy.join("src/lib.rs"), "pub fn add() { /* fixed */ }\n").expect("edit");
        std::fs::create_dir_all(copy.join("vendor/dep/.git/hooks")).expect("a directory");
        std::fs::write(copy.join("vendor/dep/.git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
        std::fs::write(
            copy.join("vendor/dep/.git/config"),
            "[core]\n\tfsmonitor = sh -c 'touch /var/empty/owned'\n",
        )
        .expect("a config");
        std::fs::write(
            copy.join("vendor/dep/.git/hooks/post-checkout"),
            "#!/bin/sh\n",
        )
        .expect("a hook");
    });
    let executor = fixture.executor(&["src/lib.rs", "README.md"], Box::new(Probe(look)));
    let case = CaseId("case-nested-git".to_owned());

    let step = executor.run_with_usage(&commission(&case), &frontier(&case));

    let files = proposed(&step).unwrap_or_default();
    let into_git: Vec<&str> = files
        .iter()
        .map(|file| file.path.as_str())
        .filter(|path| {
            Path::new(path)
                .components()
                .any(|part| part == Component::Normal(".git".as_ref()))
        })
        .collect();
    assert!(
        into_git.is_empty(),
        "the proposal carries paths into a git directory: {into_git:?}"
    );
}

/// The scratch copy's own rule (`ScratchCopy::make`): "a link, a directory or a path that is not
/// plain is left out", and the story: the session works in a copy of the workspace. A named file
/// reached through a linked directory of the workspace lies outside it: its bytes must not enter
/// the copy the session reads, and no proposal may name it, since an effect writing that path
/// writes through the link, outside the workspace.
#[test]
fn a_file_reached_through_a_linked_directory_is_neither_copied_nor_proposed() {
    let fixture = Fixture::new("linked-dir");
    let outside = fixture.root.join("outside");
    std::fs::create_dir_all(&outside).expect("a directory outside the workspace");
    std::fs::write(outside.join("secret.txt"), "outside the workspace\n").expect("a file");
    std::os::unix::fs::symlink(&outside, fixture.workspace.join("linked")).expect("a link");
    let seen: Rc<RefCell<Option<String>>> = Rc::default();
    let held = Rc::clone(&seen);
    let look: Look = Box::new(move |copy: &Path| {
        let at = copy.join("linked/secret.txt");
        *held.borrow_mut() = std::fs::read_to_string(&at).ok();
        if at.is_file() {
            std::fs::write(&at, "rewritten by the session\n").expect("an edit");
        }
    });
    let executor = fixture.executor(&["src/lib.rs", "linked/secret.txt"], Box::new(Probe(look)));
    let case = CaseId("case-linked".to_owned());

    let step = executor.run_with_usage(&commission(&case), &frontier(&case));

    let read_in = seen.borrow().clone();
    let named: Vec<String> = proposed(&step)
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.path)
        .filter(|path| path.starts_with("linked"))
        .collect();
    assert_eq!(
        (read_in, named),
        (None, Vec::new()),
        "the copy held a file from outside the workspace, and the proposal names it"
    );
}

/// Staleness (Decisions, "Stale diff"; Acceptance, the concurrent write): a stale change must not
/// reach the effect. The case revision does not move on a workspace write (the session tests
/// say so), so here a file the session changed is written in the workspace while the session
/// runs: the admitted proposal carries the whole file as the session's copy had it, and applying
/// it erases the concurrent write. Either the executor refuses the stale change or the
/// concurrent write survives the run.
#[test]
fn a_concurrent_write_to_a_file_the_session_changed_survives_the_run() {
    let fixture = Fixture::new("lost-update");
    let concurrent = "a library, edited by somebody else\n";
    let readme = fixture.workspace.join("README.md");
    let look: Look = Box::new(move |copy: &Path| {
        std::fs::write(copy.join("README.md"), "a library, edited by the session\n")
            .expect("the session's edit");
        std::fs::write(&readme, concurrent).expect("the concurrent write");
    });
    let executor = fixture.executor(&["src/lib.rs", "README.md"], Box::new(Probe(look)));
    let effects = Applying {
        workspace: fixture.workspace.clone(),
        invoked: RefCell::new(0),
    };
    let case = CaseId("case-lost-update".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(open(1), 5).chain([open(1).complete("D")]),
    );

    let end = run(&governor, &case, &executor, &effects, None);

    assert_eq!(
        std::fs::read_to_string(fixture.workspace.join("README.md")).expect("the README"),
        concurrent,
        "the concurrent write was overwritten by the stale proposal (effect invoked {} times, \
         run ended {:?})",
        effects.invoked.borrow(),
        end.outcome
    );
}
