//! The b10x-loom command line: its clap definition.
//!
//! The command is the `b10x-loom` binary (`src/main.rs`). This library holds only the definition
//! of its command line, so `loom-docs` can generate the CLI reference from it.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// The model both the agent and the classifier use unless told otherwise.
pub const DEFAULT_MODEL: &str = "gpt-5.6-sol";

/// What `run --help` says about the exit status; the binary's `exit_status` decides it.
pub const EXIT_STATUS: &str = "Exit status:
  0  the run stopped at its human gate (ApprovalRequired)
  3  the run stopped for another reason (NothingAdmissible, StepBudget, NoLocalExecutor, Refused, ConfinementUnavailable)
  1  the run failed
  2  the command line is not valid";

/// The `b10x-loom` command line.
#[derive(Debug, Parser)]
#[command(
    name = "b10x-loom",
    version,
    about = "Loom: route an intent and run it until it is blocked"
)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands of `b10x-loom`.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the slice on an intent until it is blocked, and say why it stopped.
    #[command(after_help = EXIT_STATUS)]
    Run(RunArgs),
}

/// The arguments of `b10x-loom run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Confine tests with Substrate, or explicitly run with the operator's rights.
    #[arg(long, value_enum, default_value_t = Confinement::Substrate)]
    pub confinement: Confinement,
    /// An explicitly delegated cgroup v2 root for confined tests.
    #[arg(long, value_name = "DIR")]
    pub cgroup_root: Option<PathBuf>,
    /// The root of the git work tree the change is made in.
    #[arg(long, value_name = "DIR")]
    pub workspace: PathBuf,
    /// The test command, run in the workspace without a shell: a program and its arguments,
    /// split at white space.
    #[arg(long, value_name = "CMD", default_value = "cargo test")]
    pub test_cmd: String,
    /// The most actions performed before the run stops.
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub max_steps: usize,
    /// The model that selects actions and writes their arguments.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub model: String,
    /// The model that classifies the intent.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub classifier_model: String,
    /// The confidence, from 0 to 1, below which the router refuses its pick.
    #[arg(long, value_name = "X", default_value_t = 0.5)]
    pub threshold: f64,
    /// What to do, as given.
    pub intent: String,
}

/// The operator's requested test execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Confinement {
    /// Require Substrate and refuse when unavailable.
    Substrate,
    /// Run unconfined. This opt-out is reported in every test observation.
    None,
}
