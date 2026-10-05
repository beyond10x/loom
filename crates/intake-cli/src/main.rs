//! `b10x-intake`: run the intake slice from the command line (story `slice-loop-cli`).
//!
//! `b10x-intake run --workspace <dir> "<intent>"` extracts the intent's references, classifies it,
//! opens the governed case and runs the slice until it stops, printing each step and the stop
//! reason. Everything it does is `intake_slice::run::run`; this binary parses the arguments, builds
//! the two models over the operator's Codex login and an in-memory governor, and calls it.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use governor::{CanonGovernor, MemoryCaseStore};
use intake_model::codex_model;
use intake_slice::executor::TestCommand;
use intake_slice::run::{SliceRequest, SliceRun, StopReason, printable, run};

/// The model both the agent and the classifier use unless told otherwise.
const DEFAULT_MODEL: &str = "gpt-5.6-sol";

/// What `run --help` says about the exit status; [`exit_status`] decides it.
const EXIT_STATUS: &str = "Exit status:
  0  the run stopped at its human gate (ApprovalRequired)
  3  the run stopped for another reason (NothingAdmissible, StepBudget, NoLocalExecutor, Refused)
  1  the run failed
  2  the command line is not valid";

#[derive(Debug, Parser)]
#[command(
    name = "b10x-intake",
    version,
    about = "Route an intent and run the intake slice"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the slice on an intent until it is blocked, and say why it stopped.
    #[command(after_help = EXIT_STATUS)]
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct RunArgs {
    /// The root of the git work tree the change is made in.
    #[arg(long, value_name = "DIR")]
    workspace: PathBuf,
    /// The test command, run in the workspace without a shell: a program and its arguments,
    /// split at white space.
    #[arg(long, value_name = "CMD", default_value = "cargo test")]
    test_cmd: String,
    /// The most actions performed before the run stops.
    #[arg(long, value_name = "N", default_value_t = 20)]
    max_steps: usize,
    /// The model that selects actions and writes their arguments.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    model: String,
    /// The model that classifies the intent.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    classifier_model: String,
    /// The confidence, from 0 to 1, below which the router refuses its pick.
    #[arg(long, value_name = "X", default_value_t = 0.5)]
    threshold: f64,
    /// What to do, as given.
    intent: String,
}

fn main() -> ExitCode {
    let Command::Run(arguments) = Cli::parse().command;
    match run_slice(arguments) {
        Ok(slice) => ExitCode::from(exit_status(slice.stop_reason)),
        Err(error) => {
            eprintln!("b10x-intake: {}", printable(&error));
            ExitCode::FAILURE
        }
    }
}

/// The exit status of a run that stopped for `reason`: 0 when it reached its human gate, 3 for
/// every other stop. A failure is 1 and a usage error 2 (clap's).
fn exit_status(reason: StopReason) -> u8 {
    match reason {
        StopReason::ApprovalRequired => 0,
        StopReason::NothingAdmissible
        | StopReason::StepBudget
        | StopReason::NoLocalExecutor
        | StopReason::Refused => 3,
    }
}

fn run_slice(arguments: RunArgs) -> Result<SliceRun, String> {
    let mut words = arguments.test_cmd.split_whitespace();
    let program = words
        .next()
        .ok_or_else(|| "--test-cmd names no program".to_owned())?;
    let test = TestCommand::new(program, words);
    let agent = codex_model(&arguments.model).map_err(|error| error.to_string())?;
    let classifier = codex_model(&arguments.classifier_model).map_err(|error| error.to_string())?;
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let request = SliceRequest {
        intent: arguments.intent,
        workspace: arguments.workspace,
        test,
        max_steps: arguments.max_steps,
        threshold: arguments.threshold,
    };
    let mut out = std::io::stdout().lock();
    run(
        &request,
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut out,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{StopReason, exit_status};

    /// The slice reaching its human gate is success; every other stop is 3, apart from an error
    /// (1) and a usage error (2).
    #[test]
    fn each_stop_reason_has_its_exit_status() {
        assert_eq!(exit_status(StopReason::ApprovalRequired), 0);
        for reason in [
            StopReason::NothingAdmissible,
            StopReason::StepBudget,
            StopReason::NoLocalExecutor,
            StopReason::Refused,
        ] {
            assert_eq!(exit_status(reason), 3, "{reason}");
        }
    }
}
