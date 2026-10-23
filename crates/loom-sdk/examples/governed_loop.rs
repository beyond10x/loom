//! A looping agent under governance, embedded with `b10x-loom-sdk` alone.
//!
//! The agent has to make `fizzbuzz.sh` print FizzBuzz from 1 to 15. It loops: write an attempt, run
//! the check, read the next frontier, try again. It holds three attempts; the first two are wrong
//! (plain numbers, then `Fizz` on 15 because it tests 3 before 15) and the third is right. After the
//! second failure it gets impatient and selects `repository.merge` anyway.
//!
//! What governs it, each printed as it happens:
//!
//! - **The frontier.** The agent sees only the catalogue the governor projects from the case. It
//!   never learns that a check passed from its own belief: `repository.merge` enters the
//!   catalogue only once the verifier has submitted a passing `test_result`, and that is the
//!   signal the agent loops on.
//! - **Refusal.** The impatient merge is outside the catalogue, so Loom refuses the selection and
//!   nothing is proposed or performed; the loop goes on with the next step.
//! - **Evidence.** The check's result comes from the slice's test-result verifier, which ran the
//!   command, never from what the agent says happened.
//! - **Authority.** Once the merge is admissible, the operator's [`OperatorPolicy`] answers that a
//!   merge needs a human, so the run stops at `ApprovalRequired (repository.merge)` and nothing is
//!   merged.
//! - **Budget.** The same agent run again with a step budget of 4 is suspended for its budget
//!   before it can fix anything.
//!
//! The agent is scripted: no model, no network. Every step is still a structured proposal that
//! Commission revalidates against the case revision, the frontier and the authority provider.
//!
//! Run it with `cargo run -p b10x-loom-sdk --example governed_loop`. The scratch repositories are
//! `target/loom-sdk-governed-loop/<scenario>` under the current directory, recreated on every run.
//!
//! [`run`] is the whole embedding; `crates/loom-sdk/tests/governed_loop_runs.rs` runs it too.

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
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, CaseId, Commission, CommissionData,
    CommissionId, ObservationId, PrincipalId, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RuntimeEvent, SuspensionReason, commission_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::intake::slice::case;
use loom_sdk::intake::slice::effect::{Console, LocalEffects};
use loom_sdk::intake::slice::executor::{LocalExecutor, TestCommand};
use loom_sdk::intake::slice::run::SliceReads;
use loom_sdk::intake::slice::selector::Briefing;
use loom_sdk::intake::slice::verifier::TestResultVerifier;
use loom_sdk::loom::arguments::ArgumentContext;
use loom_sdk::loom::model::run::{CatalogueEntry, CatalogueEntryStatus, SelectionStrategy};
use loom_sdk::loom::selection::{Choice, SelectionContext, SelectorError};
use loom_sdk::{
    ActionSelector, ArgumentGenerator, CanonGovernor, EventSink, Loom, LoopContext, LoopEnd,
    MemoryCaseStore, run_until_blocked,
};

/// The protocol each case is opened on.
pub const PROTOCOL: &str = "software-change@1";

/// The intent each case starts from, and the prompt Loom works on.
pub const INTENT: &str = "make fizzbuzz.sh pass check.sh";

/// The producer the verifier attributes its evidence to.
pub const PRODUCER: &str = "loom-sdk-governed-loop/verifier";

/// The action the agent edits with, the one it checks with, and the one it wants to end on.
const EDIT: &str = "implementation.edit";
const CHECK: &str = "tests.run";
const MERGE: &str = "repository.merge";

/// The agent's attempts at `fizzbuzz.sh`, in order. Only the last one passes `check.sh`.
pub const ATTEMPTS: [(&str, &str); 3] = [
    (
        "print the numbers",
        "i=1\nwhile [ $i -le 15 ]; do\n  echo $i\n  i=$((i + 1))\ndone\n",
    ),
    (
        "add Fizz, Buzz and FizzBuzz",
        "i=1\nwhile [ $i -le 15 ]; do\n  if [ $((i % 3)) -eq 0 ]; then echo Fizz\n  elif [ $((i % 5)) -eq 0 ]; then echo Buzz\n  elif [ $((i % 15)) -eq 0 ]; then echo FizzBuzz\n  else echo $i\n  fi\n  i=$((i + 1))\ndone\n",
    ),
    (
        "test 15 before 3 and 5",
        "i=1\nwhile [ $i -le 15 ]; do\n  if [ $((i % 15)) -eq 0 ]; then echo FizzBuzz\n  elif [ $((i % 3)) -eq 0 ]; then echo Fizz\n  elif [ $((i % 5)) -eq 0 ]; then echo Buzz\n  else echo $i\n  fi\n  i=$((i + 1))\ndone\n",
    ),
];

/// The check the verifier runs: the output of `fizzbuzz.sh` against `expected.txt`.
const CHECK_SCRIPT: &str = "test \"$(sh fizzbuzz.sh)\" = \"$(cat expected.txt)\"\n";

/// What `fizzbuzz.sh` has to print.
const EXPECTED: &str = "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n";

/// The step budget of the first scenario, enough for three attempts and the merge, and of the
/// second, which runs out after two attempts.
pub const ROOMY: usize = 12;
pub const TIGHT: usize = 4;

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new("target").join("loom-sdk-governed-loop");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    for (scenario, budget) in [("roomy", ROOMY), ("tight", TIGHT)] {
        println!("=== scenario {scenario}: step budget {budget}");
        let end = run(&root.join(scenario), budget)?;
        println!("stopped: {}\n", stop_reason(&end));
    }
    Ok(())
}

/// Creates the scratch repository at `workspace`, opens a `software-change@1` case on it and runs
/// the looping agent under Commission's runtime with at most `budget` steps, returning how the
/// loop ended. Each step is printed to standard output.
///
/// # Errors
/// When the repository cannot be made, the case cannot be opened, an effect fails, or the runtime
/// fails without a run outcome.
pub fn run(workspace: &Path, budget: usize) -> Result<LoopEnd, Box<dyn Error>> {
    let workspace = scratch_repository(workspace)?;

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PROTOCOL, INTENT, &workspace, &case::RUN_DEFAULTS)?;

    // Loom proposes what the agent selects; the runtime revalidates each proposal against the
    // frontier and the operator's policy, and hands what it admits to the slice's local effect
    // adapter, which performs it in the workspace and prints it on the console.
    let loom = Loom::new(LoopingAgent::default(), Attempts::default(), INTENT);
    let mut stdout = io::stdout();
    let console = Console::new(&mut stdout);
    let effects = LocalEffects::new(
        LocalExecutor::new(
            &governor,
            case.clone(),
            workspace.clone(),
            TestCommand::new("sh", ["check.sh"]),
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
        &SliceReads(&governor),
        &loom,
        &OperatorPolicy,
        &effects,
        &commission(&case),
        &mut runs,
        &mut Host {
            budget,
            events: Narrator::default(),
        },
    );
    drop(effects);
    if let Some(failure) = console.take_failure() {
        return Err(failure.into());
    }
    Ok(end?)
}

/// Why the run stopped: `ApprovalRequired (<action>)` when it waits for approval of an action,
/// `Budget` when it ran out of steps, else the runtime's outcome.
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
        RunOutcome::Suspended(suspended)
            if matches!(
                suspended.reason,
                SuspensionReason::Budget(_) | SuspensionReason::LoopBudget(_)
            ) =>
        {
            "Budget".to_owned()
        }
        other => format!("{other:?}"),
    }
}

/// A git repository at `workspace` with one commit: `fizzbuzz.sh` that prints nothing,
/// `expected.txt` and `check.sh`. Its own git calls read no system or global configuration; the
/// repository carries a local identity, which the executor's commits use. Returns the
/// repository's absolute path.
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
    fs::write(workspace.join("fizzbuzz.sh"), "")?;
    fs::write(workspace.join("expected.txt"), EXPECTED)?;
    fs::write(workspace.join("check.sh"), CHECK_SCRIPT)?;
    git(&["add", "--all"])?;
    git(&["commit", "--quiet", "--message", "an empty fizzbuzz"])?;
    Ok(workspace)
}

/// The agent's selector. It reads the catalogue it is offered each step: when the merge is in it,
/// the check passed and it selects the merge; after an edit it runs the check; otherwise it edits
/// again. After its second failed check it selects the merge without waiting for the catalogue.
#[derive(Default)]
struct LoopingAgent {
    memory: Mutex<Memory>,
}

/// What the agent remembers between steps: its last selection and how many checks failed.
#[derive(Default)]
struct Memory {
    last: Option<&'static str>,
    failed_checks: usize,
    impatient: bool,
}

impl ActionSelector for LoopingAgent {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let mut memory = self.memory.lock().unwrap_or_else(PoisonError::into_inner);
        println!("  catalogue: {}", catalogue(candidates));
        let offered = |action: &str| candidates.iter().any(|entry| entry.action == action);

        if memory.last == Some(CHECK) && !offered(MERGE) {
            memory.failed_checks += 1;
        }
        let action = if offered(MERGE) {
            MERGE
        } else if memory.failed_checks == 2 && !memory.impatient {
            memory.impatient = true;
            println!("  agent: two failures, I'll merge it anyway");
            MERGE
        } else if memory.last == Some(EDIT) {
            CHECK
        } else {
            EDIT
        };
        println!("  agent selects {action}");
        memory.last = Some(action);
        Ok(Choice {
            action: action.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// The catalogue as one line: each action, with its status when it is not plainly admissible.
fn catalogue(candidates: &[CatalogueEntry]) -> String {
    candidates
        .iter()
        .map(|entry| match entry.status {
            CatalogueEntryStatus::Admissible => entry.action.clone(),
            CatalogueEntryStatus::ApprovalRequired => format!("{} (approval)", entry.action),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The agent's argument generator: the next of [`ATTEMPTS`] for an edit, none for the rest. With
/// no attempt left the generator fails, which the runtime takes as an outage.
#[derive(Default)]
struct Attempts {
    made: Mutex<usize>,
}

impl ArgumentGenerator for Attempts {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        if entry.action != EDIT {
            return Ok(Value::Object(Vec::new()));
        }
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        let (message, contents) = ATTEMPTS
            .get(*made)
            .ok_or_else(|| "the agent has no attempt left".to_owned())?;
        *made += 1;
        json::parse(&format!(
            r#"{{"files":[{{"path":"fizzbuzz.sh","contents":{}}}],"message":{}}}"#,
            json_text(contents),
            json_text(message),
        ))
        .map_err(|error| format!("{error:?}"))
    }
}

/// `text` as a JSON string literal.
fn json_text(text: &str) -> String {
    let mut quoted = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// The operator's policy: a merge needs a human, and nothing else is delegated. In
/// `software-change@1` only the merge asks for a capability.
struct OperatorPolicy;

impl AuthorityProvider for OperatorPolicy {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(if capability == MERGE {
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: format!("a human approves `{capability}`"),
            })
        } else {
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!("`{capability}` is not delegated to this agent"),
            })
        })
    }
}

/// Prints the runtime's events that the console does not: each step's start, the runtime's
/// answer to each proposal, each authority decision, and a step that ended with no proposal. An
/// event is never evidence.
#[derive(Default)]
struct Narrator {
    /// Whether a step is open and no request was proposed in it yet.
    unproposed: bool,
}

impl Narrator {
    /// Closes the open step, saying so when Loom proposed nothing in it.
    fn close_step(&mut self) {
        if std::mem::take(&mut self.unproposed) {
            println!("  runtime: refused the selection; nothing proposed, nothing performed");
        }
    }
}

impl EventSink for Narrator {
    fn emit(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::StepStarted(started) => {
                self.close_step();
                self.unproposed = true;
                println!(
                    "[runtime] step {} at case revision {}",
                    started.step, started.case_revision
                );
            }
            RuntimeEvent::Proposed(proposed) => {
                self.unproposed = false;
                println!("  runtime: {} is {:?}", proposed.action, proposed.answer)
            }
            RuntimeEvent::Stopped(_) => self.close_step(),
            RuntimeEvent::AuthorityDecided(decided) => match &decided.verdict {
                Some(AuthorityVerdict::Allow(_)) => {
                    println!("  authority: {} allowed", decided.capability)
                }
                Some(AuthorityVerdict::ApprovalRequired(approval)) => {
                    println!("  authority: {}", approval.request)
                }
                Some(AuthorityVerdict::Deny(deny)) => {
                    println!("  authority: denied, {}", deny.reason)
                }
                None => println!("  authority: no answer for {}", decided.capability),
            },
            _ => {}
        }
    }
}

/// What the loop needs that no model may supply: new ids, the time, the step budget and where its
/// events go. The clock is fixed, so runs differ only in their ids and commit ids.
struct Host {
    budget: usize,
    events: Narrator,
}

impl LoopContext for Host {
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
        Some(self.budget)
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.events
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
