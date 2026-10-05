//! Embeds a governed agent with `b10x-loom-sdk` alone.
//!
//! The example creates a scratch git repository with one failing check (`check.txt` holds
//! `broken`; the test command is `grep -qx fixed check.txt`), opens a case on `software-change@1`
//! through the governor, and runs Commission's runtime over it with Loom as the executor.
//!
//! Loom's selector and argument generator are scripted fakes: no model, no network. The script
//! edits the check, runs the test and then selects the merge. The slice's local effect adapter
//! performs the edit and the test run in the scratch repository, prints each step to standard
//! output, and lets the verifier submit the passing test result as evidence, which leaves the merge
//! needing approval. The authority provider never grants a
//! capability, so the run stops at `ApprovalRequired (repository.merge)`: nothing is merged.
//!
//! Run it with `cargo run -p b10x-loom-sdk --example software_change`. The scratch repository is
//! `target/loom-sdk-example` under the current directory, recreated on every run.
//!
//! [`run`] is the whole embedding; `crates/loom-sdk/tests/example_runs.rs` runs it too.

use std::collections::VecDeque;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use loom_sdk::commission::model::behaviour::Generated;
use loom_sdk::commission::model::json::{self, Value};
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    ObservationId, PrincipalId, RevalidateActionRequestOutcome, RunId, RunOutcome,
    commission_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::intake::slice::case;
use loom_sdk::intake::slice::effect::{Console, LocalEffects};
use loom_sdk::intake::slice::executor::{LocalExecutor, TestCommand};
use loom_sdk::intake::slice::selector::Briefing;
use loom_sdk::intake::slice::verifier::TestResultVerifier;
use loom_sdk::loom::arguments::ArgumentContext;
use loom_sdk::loom::model::run::{CatalogueEntry, SelectionStrategy};
use loom_sdk::loom::selection::{Choice, SelectionContext, SelectorError};
use loom_sdk::{
    ActionSelector, ArgumentGenerator, CanonGovernor, Loom, LoopContext, LoopEnd, MemoryCaseStore,
    run_until_blocked,
};

/// The protocol the case is opened on.
pub const PROTOCOL: &str = "software-change@1";

/// The intent the case starts from, and the prompt Loom works on.
pub const INTENT: &str = "make the failing check pass";

/// The producer the verifier attributes its evidence to.
pub const PRODUCER: &str = "loom-sdk-example/verifier";

/// The scripted agent: the action it selects on each frontier, in order, with its arguments as
/// JSON text.
pub const SCRIPT: [(&str, &str); 3] = [
    (
        "repository.edit",
        r#"{"files":[{"path":"check.txt","contents":"fixed\n"}],"message":"fix the check"}"#,
    ),
    ("tests.run", "{}"),
    ("repository.merge", "{}"),
];

/// The most steps the run takes before the runtime suspends it.
const STEP_BUDGET: usize = 10;

fn main() -> Result<(), Box<dyn Error>> {
    let workspace = Path::new("target").join("loom-sdk-example");
    if workspace.exists() {
        fs::remove_dir_all(&workspace)?;
    }
    println!("workspace: {}", workspace.display());
    let end = run(&workspace)?;
    println!("stopped: {}", stop_reason(&end));
    Ok(())
}

/// Creates the scratch repository at `workspace`, opens a `software-change@1` case on it and runs
/// Commission's runtime over the case until it stops, returning how the loop ended. Each step is
/// printed to standard output.
///
/// # Errors
/// When the repository cannot be made, the case cannot be opened, an effect fails, or the runtime
/// fails without a run outcome.
pub fn run(workspace: &Path) -> Result<LoopEnd, Box<dyn Error>> {
    let workspace = scratch_repository(workspace)?;

    // The governor holds the case and evaluates its protocol; the case starts at the repository's
    // HEAD.
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PROTOCOL, INTENT, &workspace)?;

    // Loom proposes; the runtime revalidates each proposal against the frontier and the authority
    // provider, and hands what it admits to the slice's local effect adapter, which performs it in
    // the workspace and prints it on the console.
    let loom = Loom::new(ScriptedSelector::new(), ScriptedArguments, INTENT);
    let mut stdout = io::stdout();
    let console = Console::new(&mut stdout);
    let effects = LocalEffects::new(
        LocalExecutor::new(
            &governor,
            case.clone(),
            workspace.clone(),
            TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        )
        .with_runner(std::sync::Arc::new(loom_sdk::UnconfinedRunner)),
        TestResultVerifier::new(&governor, case.clone(), PRODUCER),
        &governor,
        case.clone(),
        PROTOCOL,
        Briefing::new(INTENT, Vec::new()),
        &console,
    );
    let mut runs = Generated::new(RunStore::new(|| RunId(fresh(Kind::Run))));
    let end = run_until_blocked(
        &governor,
        &loom,
        &NoGrant,
        &effects,
        &commission(&case),
        &mut runs,
        &mut Clock,
    );
    drop(effects);
    // An effect that failed is reported through the port as an error and kept, typed, on the
    // console.
    if let Some(failure) = console.take_failure() {
        return Err(failure.into());
    }
    Ok(end?)
}

/// Why the run stopped, as `ApprovalRequired (<action>)` when it waits for approval of an action,
/// else the runtime's outcome.
pub fn stop_reason(end: &LoopEnd) -> String {
    match &end.outcome {
        RunOutcome::NeedsAuthority(_) => {
            let action = end
                .requests
                .iter()
                .rev()
                .find_map(|made| match &made.outcome {
                    RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                        Some(error.action.as_str())
                    }
                    _ => None,
                });
            match action {
                Some(action) => format!("ApprovalRequired ({action})"),
                None => "ApprovalRequired".to_owned(),
            }
        }
        RunOutcome::AwaitingApproval(awaiting) => {
            format!("ApprovalRequired ({})", awaiting.actions.join(", "))
        }
        other => format!("{other:?}"),
    }
}

/// A git repository at `workspace` with one commit: `check.txt` holding `broken`. Its own git
/// calls read no system or global configuration; the repository carries a local identity, which
/// the executor's commits use. Returns the repository's absolute path.
fn scratch_repository(workspace: &Path) -> Result<PathBuf, Box<dyn Error>> {
    fs::create_dir_all(workspace)?;
    let workspace = fs::canonicalize(workspace)?;
    let git = |args: &[&str]| -> Result<(), Box<dyn Error>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into())
        }
    };
    git(&["init", "--quiet", "--initial-branch=main"])?;
    git(&["config", "user.name", "Example Author"])?;
    git(&["config", "user.email", "example@example.invalid"])?;
    git(&["config", "commit.gpgsign", "false"])?;
    fs::write(workspace.join("check.txt"), "broken\n")?;
    git(&["add", "--all"])?;
    git(&["commit", "--quiet", "--message", "a failing check"])?;
    Ok(workspace)
}

/// A fake selector: the next action of [`SCRIPT`], whatever the frontier offers. Loom refuses an
/// action the catalogue does not list.
struct ScriptedSelector {
    next: Mutex<VecDeque<&'static str>>,
}

impl ScriptedSelector {
    fn new() -> Self {
        Self {
            next: Mutex::new(SCRIPT.iter().map(|(action, _)| *action).collect()),
        }
    }
}

impl ActionSelector for ScriptedSelector {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let next = self
            .next
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        match next {
            Some(action) => Ok(Choice {
                action: action.to_owned(),
                confidence: None,
            }),
            None => Err(SelectorError::Unavailable(
                "the script has no further action".to_owned(),
            )),
        }
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// A fake argument generator: the arguments [`SCRIPT`] gives the selected action.
struct ScriptedArguments;

impl ArgumentGenerator for ScriptedArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        let (_, arguments) = SCRIPT
            .iter()
            .find(|(action, _)| *action == entry.action)
            .ok_or_else(|| format!("the script has no arguments for `{}`", entry.action))?;
        json::parse(arguments).map_err(|error| format!("{error:?}"))
    }
}

/// The authority provider: no capability is delegated, so every one needs approval.
struct NoGrant;

impl AuthorityProvider for NoGrant {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: format!("approval for `{capability}`"),
            },
        ))
    }
}

/// What the loop needs that no model may supply: new ids, the time and the step budget. The
/// example's clock is fixed, so its runs differ only in their ids.
struct Clock;

impl LoopContext for Clock {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(fresh(Kind::ActionRequest))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(fresh(Kind::Observation))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-01-01T00:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(STEP_BUDGET)
    }
}

/// The commission the run works under: for an operator, with no authority context.
fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(fresh(Kind::Commission)),
        agent_revision_id: AgentRevisionId(fresh(Kind::AgentRevision)),
        case_id: case.clone(),
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// What a new id is for: its first group, so ids of two kinds never collide.
#[derive(Clone, Copy)]
enum Kind {
    Run = 1,
    ActionRequest = 2,
    Observation = 3,
    Commission = 4,
    AgentRevision = 5,
}

/// A new version-8 UUID: the kind and a process-wide counter.
fn fresh(kind: Kind) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(1);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    Uuid(format!("{:08x}-0000-8000-8000-{issued:012x}", kind as u8))
}
