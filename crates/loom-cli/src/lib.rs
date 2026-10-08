//! The b10x-loom command line: its clap definition.
//!
//! The command is the `b10x-loom` binary (`src/main.rs`). This library holds only the definition
//! of its command line, so `loom-docs` can generate the CLI reference from it, the exit status a
//! stop reason maps to, the run event stream `run --output jsonl` writes ([`events`]), the
//! JSON `evaluate` reads and writes ([`evaluate`]), and the llm catalog `run --catalog` reads
//! ([`model_catalog`]).

use std::path::PathBuf;

pub use b10x_loom_intake_slice::context_metrics::ContextPolicy;
use b10x_loom_intake_slice::run::StopReason;
use clap::builder::TypedValueParser;
use clap::{Args, Parser, Subcommand, ValueEnum};

/// The Codex model both the agent and the classifier use unless told otherwise.
pub const DEFAULT_MODEL: &str = "gpt-5.6-sol";

/// What `run --help` says about the exit status; the binary's `exit_status` decides it.
pub const EXIT_STATUS: &str = "Exit status:
  0  the query completed (Completed) or the run reached its human gate (ApprovalRequired)
  3  the run stopped for another reason (NothingAdmissible, StepBudget, NoLocalExecutor, Refused, ConfinementUnavailable)
  1  the run failed
  2  the command line is not valid";

/// What `evaluate --help` says about its input, its output and its exit status.
pub const EVALUATE_HELP: &str = "The request is one JSON object (loom.evaluation.EvaluationRequest):
  {\"protocol\": \"<name>@<major>\", \"snapshot\": {canon-case/1}, \"evidence\": [{canon-evidence/1}, ...], \"at\": \"<RFC 3339>\"}
`at` is optional. The protocol comes from this host's catalog (`protocols list`), never from the request.
A record that is not a readable canon-evidence/1 record, or repeats an earlier record's id, is refused.
A readable record that does not apply to the case (an undeclared kind or subject, for instance) is set
aside, as the governor sets it aside, and the decision is made from the rest without listing it.
Unlike the governor, which sets an unreadable record and a repeated id aside (keeping the first) and
still decides, evaluate refuses both (a repeated id as duplicate-identifier, naming the later record).

Exit status:
  0  decided: standard output carries the decision (loom.evaluation.EvaluationDecision)
  3  refused: standard output carries the refusal (loom.evaluation.EvaluationRefusal), and standard error names the input
  1  the request or the protocol catalog cannot be read
  2  the command line is not valid";

pub mod evaluate;
pub mod events;
pub mod model_catalog;
pub mod regular_file;

/// The exit status a run that stopped for `reason` returns, as [`EXIT_STATUS`] states it.
pub fn exit_status(reason: StopReason) -> u8 {
    match reason {
        StopReason::Completed | StopReason::ApprovalRequired => 0,
        StopReason::NothingAdmissible
        | StopReason::StepBudget
        | StopReason::NoLocalExecutor
        | StopReason::ConfinementUnavailable
        | StopReason::Refused => 3,
    }
}

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
    /// Install, inspect, or remove protocol definitions available to this host.
    Protocols(ProtocolsArgs),
    /// Decide a case snapshot and its evidence under a catalog protocol, and write the decision as
    /// JSON. It decides and never acts.
    #[command(after_help = EVALUATE_HELP)]
    Evaluate(EvaluateArgs),
}

/// The arguments of `b10x-loom evaluate`.
#[derive(Debug, Args)]
pub struct EvaluateArgs {
    /// Read the request from this regular file instead of standard input; a named pipe, a device
    /// or a directory is refused (pipe a request to standard input instead).
    #[arg(long, value_name = "PATH")]
    pub input: Option<PathBuf>,
}

/// The arguments of `b10x-loom run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Keep the legacy rolling transcript, or opt into bounded working context and history lookup.
    #[arg(long, default_value = "legacy", value_parser = clap::builder::PossibleValuesParser::new(["legacy", "bounded"]).map(|value| match value.as_str() {
        "bounded" => ContextPolicy::Bounded,
        _ => ContextPolicy::Legacy,
    }))]
    pub context_policy: ContextPolicy,
    /// Write payload-free measurements, including routing and startup failures.
    #[arg(long, value_name = "PATH")]
    pub context_report: Option<PathBuf>,
    /// Confine tests with Substrate, or explicitly run with the operator's rights.
    #[arg(long, value_enum, default_value_t = Confinement::Substrate)]
    pub confinement: Confinement,
    /// An explicitly delegated cgroup v2 root for confined tests.
    #[arg(long, value_name = "DIR")]
    pub cgroup_root: Option<PathBuf>,
    /// Existing Git worktree for software changes; unnecessary for system queries.
    #[arg(long, value_name = "DIR")]
    pub workspace: Option<PathBuf>,
    /// The test command, run in the workspace without a shell: a program and its arguments,
    /// split at white space.
    #[arg(long, value_name = "CMD", default_value = "cargo test")]
    pub test_cmd: String,
    /// The most actions performed before the run stops.
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub max_steps: usize,
    /// An llm catalog (`llm.catalog/1` TOML) in a regular file; a named pipe such as `<(cmd)`, a
    /// device or a directory is refused. With it, `--model` and `--classifier-model` each name a
    /// route alias of this catalog instead of a Codex model, both of them; an alias it does not
    /// declare, or a catalog that cannot be read, stops the run before any model call.
    #[arg(long, value_name = "PATH")]
    pub catalog: Option<PathBuf>,
    /// The model that selects actions and writes their arguments: a Codex model name, or with
    /// `--catalog` a route alias of that catalog.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub model: String,
    /// The model that classifies the intent: a Codex model name, or with `--catalog` a route
    /// alias of that catalog.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub classifier_model: String,
    /// The confidence, from 0 to 1, below which the router refuses its pick.
    #[arg(long, value_name = "X", default_value_t = 0.5)]
    pub threshold: f64,
    /// What standard output carries: lines for a person, or one JSON record per line, the last
    /// one the terminal record with the stop reason and the exit status.
    #[arg(long, value_enum, value_name = "FORMAT", default_value_t = Output::Human)]
    pub output: Output,
    /// What to do, as given.
    pub intent: String,
}

/// Protocol catalog management.
#[derive(Debug, Args)]
pub struct ProtocolsArgs {
    #[command(subcommand)]
    pub command: ProtocolCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProtocolCommand {
    /// Validate and snapshot one protocol from a file or a pinned Git repository.
    Add(AddProtocolArgs),
    /// List bundled and installed definitions, provenance, and available execution bindings.
    List,
    /// Remove an installed definition; bundled definitions cannot be removed.
    Remove { name: String },
}

#[derive(Debug, Args)]
pub struct AddProtocolArgs {
    /// Registration identity, such as clock-check@1.
    pub name: String,
    /// Local YAML to validate and snapshot.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with = "source",
        required_unless_present = "source"
    )]
    pub file: Option<PathBuf>,
    /// Git locator with a full commit pin: git+https://host/repo.git#<commit>.
    #[arg(
        long,
        value_name = "LOCATOR",
        conflicts_with = "file",
        requires = "path",
        required_unless_present = "file"
    )]
    pub source: Option<String>,
    /// Regular-file path inside the pinned Git commit.
    #[arg(long, value_name = "RELATIVE_PATH", requires = "source")]
    pub path: Option<PathBuf>,
    /// Explicitly replace an existing installed definition.
    #[arg(long)]
    pub replace: bool,
}

/// What `run` writes on standard output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Output {
    /// The human lines: references, the pick, each step, and the stop reason last.
    Human,
    /// The run event stream, one JSON object per line (`intake.events`).
    Jsonl,
}

/// The operator's requested test execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Confinement {
    /// Require Substrate and refuse when unavailable.
    Substrate,
    /// Run unconfined. This opt-out is reported in every test observation.
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stop_reason_has_its_exit_status() {
        assert_eq!(exit_status(StopReason::Completed), 0);
        assert_eq!(exit_status(StopReason::ApprovalRequired), 0);
        for reason in [
            StopReason::NothingAdmissible,
            StopReason::StepBudget,
            StopReason::NoLocalExecutor,
            StopReason::Refused,
            StopReason::ConfinementUnavailable,
        ] {
            assert_eq!(exit_status(reason), 3);
        }
    }

    #[test]
    fn context_policy_is_opt_in_and_report_destination_is_optional() {
        let Command::Run(defaults) =
            Cli::try_parse_from(["b10x-loom", "run", "--workspace", ".", "fix tests"])
                .unwrap()
                .command
        else {
            panic!("run command")
        };
        assert_eq!(defaults.context_policy, ContextPolicy::Legacy);
        assert!(defaults.context_report.is_none());
        let Command::Run(bounded) = Cli::try_parse_from([
            "b10x-loom",
            "run",
            "--workspace",
            ".",
            "--context-policy",
            "bounded",
            "--context-report",
            "metrics.json",
            "fix tests",
        ])
        .unwrap()
        .command
        else {
            panic!("run command")
        };
        assert_eq!(bounded.context_policy, ContextPolicy::Bounded);
        assert_eq!(bounded.context_report, Some(PathBuf::from("metrics.json")));
        assert!(
            Cli::try_parse_from([
                "b10x-loom",
                "run",
                "--workspace",
                ".",
                "--context-policy",
                "automatic",
                "fix tests",
            ])
            .is_err()
        );
    }
    #[test]
    fn query_requires_no_workspace_and_protocol_install_sources_are_exclusive() {
        let Command::Run(query) = Cli::try_parse_from(["b10x-loom", "run", "current time"])
            .unwrap()
            .command
        else {
            panic!("run");
        };
        assert!(query.workspace.is_none());
        assert!(
            Cli::try_parse_from([
                "b10x-loom",
                "protocols",
                "add",
                "clock-check@1",
                "--file",
                "clock.yaml"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "b10x-loom",
                "protocols",
                "add",
                "clock-check@1",
                "--source",
                "git+https://example.invalid/r#abc",
                "--path",
                "clock.yaml"
            ])
            .is_ok()
        );
        for args in [
            vec!["b10x-loom", "protocols", "add", "clock-check@1"],
            vec![
                "b10x-loom",
                "protocols",
                "add",
                "clock-check@1",
                "--source",
                "repo",
            ],
            vec![
                "b10x-loom",
                "protocols",
                "add",
                "clock-check@1",
                "--file",
                "x",
                "--source",
                "repo",
                "--path",
                "x",
            ],
            vec!["b10x-loom", "run", "--protocol", "system-query@1", "time"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
}
