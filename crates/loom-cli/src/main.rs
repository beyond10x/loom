//! `b10x-loom`: run an intent through Loom from the command line (story `loom-cli`).
//!
//! `b10x-loom run --workspace <dir> "<intent>"` extracts the intent's references, classifies it,
//! opens the governed case and runs the slice until it stops, printing each step and the stop
//! reason. Everything it does is `b10x_loom_intake_slice::run::run`; this binary parses the
//! arguments (defined in the library, `b10x_loom_cli::Cli`), builds the two models over the
//! operator's Codex login and an in-memory governor, and calls it.

use std::process::ExitCode;

use b10x_llm_tool_call::codex_model;
use b10x_loom_cli::{Cli, Command, RunArgs};
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_intake_slice::run::{SliceRequest, SliceRun, StopReason, printable, run};
use clap::Parser;
use loom_governor::{CanonGovernor, MemoryCaseStore};

fn main() -> ExitCode {
    let Command::Run(arguments) = Cli::parse().command;
    match run_slice(arguments) {
        Ok(slice) => ExitCode::from(exit_status(slice.stop_reason)),
        Err(error) => {
            eprintln!("b10x-loom: {}", printable(&error));
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
