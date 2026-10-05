//! `b10x-loom`: run an intent through Loom from the command line (story `loom-cli`).
//!
//! `b10x-loom run --workspace <dir> "<intent>"` extracts the intent's references, classifies it,
//! opens the governed case and runs the slice until it stops, printing each step and the stop
//! reason. Everything it does is `b10x_loom_intake_slice::run::run`; this binary parses the
//! arguments (defined in the library, `b10x_loom_cli::Cli`), builds the two models over the
//! operator's Codex login and an in-memory governor, and calls it.

use std::process::{Command as Process, ExitCode};
use std::sync::Arc;

use b10x_llm_tool_call::codex_model;
use b10x_loom_cli::{Cli, Command, Confinement, RunArgs};
use b10x_loom_intake_slice::confinement::ConfinementRefusal;
use b10x_loom_intake_slice::confinement::{SubstrateRunner, TestRunner, prepare_delegated_cgroup};
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::run::{SliceRequest, SliceRun, StopReason, printable, run};
use clap::Parser;
use loom_governor::{CanonGovernor, MemoryCaseStore};

fn main() -> ExitCode {
    let Command::Run(arguments) = Cli::parse().command;
    if arguments.test_cmd.split_whitespace().next().is_none() {
        eprintln!("b10x-loom: --test-cmd names no program");
        return ExitCode::FAILURE;
    }
    if !(0.0..=1.0).contains(&arguments.threshold) {
        eprintln!(
            "b10x-loom: the threshold {} is not a number from 0 to 1",
            arguments.threshold
        );
        return ExitCode::FAILURE;
    }
    let runner: Arc<dyn TestRunner> = match arguments.confinement {
        Confinement::None => Arc::new(UnconfinedRunner),
        Confinement::Substrate => {
            let root =
                if std::env::var_os(REEXEC_MARKER).is_some() && arguments.cgroup_root.is_none() {
                    match prepare_delegated_cgroup() {
                        Ok(root) => Some(root),
                        Err(error) => return confinement_refused(&error.to_string()),
                    }
                } else {
                    arguments.cgroup_root.clone()
                };
            let runner = SubstrateRunner::new(root);
            if let Err(error) = runner.preflight(&arguments.workspace) {
                if error.reason == ConfinementRefusal::CgroupUndelegated
                    && std::env::var_os(REEXEC_MARKER).is_none()
                {
                    return reexec();
                }
                return confinement_refused(&error.to_string());
            }
            Arc::new(runner)
        }
    };
    if let Some(ready) = std::env::var_os(REEXEC_READY)
        && std::env::var_os(REEXEC_MARKER).is_some()
        && let Err(error) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(ready)
    {
        return confinement_refused(&format!(
            "CapabilityUnserved: re-exec handshake failed: {error}"
        ));
    }
    match run_slice(arguments, runner) {
        Ok(slice) => ExitCode::from(exit_status(slice.stop_reason)),
        Err(error) => {
            eprintln!("b10x-loom: {}", printable(&error));
            ExitCode::FAILURE
        }
    }
}

const REEXEC_MARKER: &str = "B10X_LOOM_CONFINEMENT_REEXEC";
const REEXEC_READY: &str = "B10X_LOOM_CONFINEMENT_READY";

fn confinement_refused(reason: &str) -> ExitCode {
    eprintln!("confinement: substrate");
    eprintln!("stopped: ConfinementUnavailable ({})", printable(reason));
    ExitCode::from(3)
}

fn reexec() -> ExitCode {
    let state = match tempfile::Builder::new()
        .prefix("loom-delegation-")
        .tempdir()
    {
        Ok(state) => state,
        Err(error) => return confinement_refused(&format!("CgroupUndelegated: {error}")),
    };
    let ready = state.path().join("ready");
    let executable = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            return confinement_refused(&format!("CgroupUndelegated: cannot re-exec: {error}"));
        }
    };
    let result = Process::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(executable)
        .args(std::env::args_os().skip(1))
        .env(REEXEC_MARKER, "1")
        .env(REEXEC_READY, &ready)
        .status();
    match result {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) if status.code() == Some(3) => ExitCode::from(3),
        Ok(status) if ready.is_file() => ExitCode::from(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        ),
        Ok(status) => confinement_refused(&format!(
            "CgroupUndelegated: delegated run exited {status}; configure a working user systemd manager or pass --cgroup-root"
        )),
        Err(error) => confinement_refused(&format!(
            "CgroupUndelegated: systemd-run unavailable: {error}"
        )),
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
        | StopReason::ConfinementUnavailable
        | StopReason::Refused => 3,
    }
}

fn run_slice(arguments: RunArgs, runner: Arc<dyn TestRunner>) -> Result<SliceRun, String> {
    let mut words = arguments.test_cmd.split_whitespace();
    let program = words
        .next()
        .ok_or_else(|| "--test-cmd names no program".to_owned())?;
    let test = TestCommand::new(program, words);
    let agent = codex_model(&arguments.model).map_err(|error| error.to_string())?;
    let classifier = codex_model(&arguments.classifier_model).map_err(|error| error.to_string())?;
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let request = SliceRequest {
        runner,
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
            StopReason::ConfinementUnavailable,
        ] {
            assert_eq!(exit_status(reason), 3, "{reason:?}");
        }
    }
}
