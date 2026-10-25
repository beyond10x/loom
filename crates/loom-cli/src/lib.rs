//! The b10x-loom command line: its clap definition.
//!
//! The command is the `b10x-loom` binary (`src/main.rs`). This library holds only the definition
//! of its command line, so `loom-docs` can generate the CLI reference from it, the exit status a
//! stop reason maps to, the run event stream `run --output jsonl` writes ([`events`]), the
//! JSON `evaluate` reads and writes ([`evaluate`]), the llm catalog `run --catalog` reads
//! ([`model_catalog`]), the lines `plugin run --once` and `plugin report` print ([`plugin`]),
//! and what `agent check` reads and prints ([`agent`]).

use std::path::PathBuf;

use b10x_loom_commission::runtime::{ResumeError, ResumeRefusal};
pub use b10x_loom_intake_slice::case::{CaseInputs, ChangeKind, Risk};
pub use b10x_loom_intake_slice::context_metrics::ContextPolicy;
pub use b10x_loom_intake_slice::executor::{RegressionTestCommand, RegressionTestCommandError};
use b10x_loom_intake_slice::grant::ApproveRefusal;
use b10x_loom_intake_slice::run::{ApprovalGate, SliceRun, StopReason};
pub use b10x_loom_intake_slice::test_paths::{TestPaths, TestPathsError};
use clap::builder::{ArgPredicate, TypedValueParser};
use clap::{Args, Parser, Subcommand, ValueEnum};

/// The Codex model both the agent and the classifier use unless told otherwise.
pub const DEFAULT_MODEL: &str = "gpt-5.6-sol";

/// What `run --help` says about the exit status; the binary's `exit_status` decides it.
pub const EXIT_STATUS: &str = "With --state, only a run whose last line is `stopped: ApprovalRequired (<action>)` holds a
Suspension `approve` can approve.

Exit status:
  0  the query completed (Completed) or the run reached its human gate (ApprovalRequired)
  4  with --state, the run reached its human gate holding no Suspension:
     `stopped: ApprovalRequired (<action>), nothing suspended`; there is nothing to approve
  3  the run stopped for another reason (NothingAdmissible, StepBudget, NoLocalExecutor, Refused, ConfinementUnavailable)
  1  the run failed
  2  the command line is not valid";

/// The exit status of a run or a resume with a state directory that stopped `ApprovalRequired`
/// holding no Suspension ([`ApprovalGate::NothingSuspended`]).
pub const NOTHING_SUSPENDED_AT_GATE: u8 = 4;

/// What `evaluate --help` says about its input, its output and its exit status.
pub const EVALUATE_HELP: &str = "The request is one JSON object (loom.evaluation.EvaluationRequest):
  {\"protocol\": \"<name>@<major>\", \"snapshot\": {canon-case/1}, \"evidence\": [{canon-evidence/1}, ...], \"at\": \"<RFC 3339>\",
   \"decisions\": [{\"decision\": \"<name>\", \"outcome\": \"<outcome>\", \"principal\": \"<who>\", \"case_revision\": \"<revision>\"}, ...]}
`at` and `decisions` are optional. The protocol comes from this host's catalog (`protocols list`), never from the request.
`decisions` are the canon-decisions/1 explicit decisions taken for the case; one applies only at the snapshot's revision.
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

/// What `approve --help` says about the grant and the exit status; [`approve_exit_status`]
/// decides the status of a refusal.
pub const APPROVE_HELP: &str =
    "Approval performs nothing. It reads the commission's Suspension in the state directory and
writes a grant (intake.routing.Grant) for the one request it holds, which `resume` reads; a later
approval of the commission's next Suspension replaces it. The capability must be the one the
suspended request needs, as `run` or `resume` named it when it stopped.

Only a stop line `stopped: ApprovalRequired (<action>)` of `run --state` or `resume` leaves a
Suspension to approve. `stopped: ApprovalRequired (<action>), nothing suspended` leaves none, and
`stopped: MergeHandedOff (<request>, HEAD <sha>)` says that HEAD was already handed off: a merge the
agent proposed again at it holds no Suspension, so approving it is refused NothingSuspended, and only
new work, a new HEAD, is approved again.

With --mandate FILE (loom.authority.MandateHost, a host-trusted JSON file), approval also records a
Mandate approval of the request in the state directory's approval log (mandate-approvals.jsonl): the
file's approver approves the capability on the case for the file's verified subject, expiring
approval_seconds later; the approver is neither the verified subject nor its actor. The grant names
it (mandate_approval), and only `resume --mandate` clears it: `resume` without --mandate refuses it.

Exit status:
  0   the grant is written
  10  NothingSuspended: the commission holds no Suspension
  11  InDoubt: the commission's Suspension is being resumed, so its request may have been performed
  19  OtherCapability: the suspended request needs another capability than the one given
  23  AlreadyHandedOff: the suspended repository.merge would hand off a HEAD the commission already handed off
  1   the state directory or the Mandate file cannot be used, or Mandate recorded no approval
  2   the command line is not valid";

/// What `resume --help` says about the resume and the exit status; [`resume_exit_status`]
/// decides the status of a refusal or a failure of Commission's resume.
pub const RESUME_HELP: &str = "The run is rebuilt from the commission's resume context: the same intent, route, workspace (validated
again), test command, step budget, confinement and agent model; the route is used only when the intent,
the protocol catalog and the context policy are still the ones it was accepted for. The workspace's HEAD
is reported to the case first, so a commit made since the Suspension is refused CaseMoved. The grant
`approve` wrote allows only the request the commission's Suspension holds, and only while it is held. A
granted repository.merge is performed as a merge hand-off: a record of the commission, the request, the
workspace and its HEAD (intake.routing.MergeHandoff, merge-<hex of the request id>.json in the state
directory), only at the HEAD the case holds and once per HEAD of a commission. No Git ref moves; merging
happens outside Loom. The request is performed once, the Suspension is released, and the run goes on as
`run` does. The model does not see the steps before the Suspension.

With --mandate FILE (loom.authority.MandateHost), the request is performed only when Mandate's
check_with_approval clears the approval the grant names (`approve --mandate`), for the file's verified
subject, the capability and the case; its consumption is appended to the state directory's approval
log before anything is performed. A grant naming no approval, an expired, consumed or foreign
approval, or any other Mandate refusal is NotGranted, and nothing is performed. Without --mandate, a
grant naming a Mandate approval (`approve --mandate`) is NotGranted and nothing is performed: only
Mandate clears it.

When the resumed run reaches its human gate again, its last line says what is left to approve:
  stopped: ApprovalRequired (<action>)                    a Suspension `approve` can approve
  stopped: ApprovalRequired (<action>), nothing suspended  nothing to approve
  stopped: MergeHandedOff (<request>, HEAD <sha>)          the case is at a HEAD this commission
                                                           already handed off; nothing to approve

Exit status:
  0   the resumed run reached its human gate again (ApprovalRequired, or MergeHandedOff)
  4   the resumed run reached its human gate holding no Suspension (ApprovalRequired, nothing suspended)
  3   the resumed run stopped for another reason (NothingAdmissible, StepBudget, NoLocalExecutor, ConfinementUnavailable)
  10  NothingSuspended: the commission holds no Suspension, or another resume took and released it first
  11  InDoubt: a resume took the Suspension and did not release it; its request is never performed again
  12  OtherRequest: the Suspension is now held for another request than the one revalidated
  13  OtherCommission: the Suspension or its run names another commission, case or run revision
  14  CaseMoved: the case is no longer at the revision the request was suspended at
  15  NotAdmitted: the current frontier refuses the request's action
  16  NotGranted: no grant allows the request, the grant names a Mandate approval and --mandate is
      not given, or with --mandate Mandate did not clear its approval
  17  NotPerformed: nothing performs the request's action
  18  RunNotSuspended: the run store does not hold the run suspended
  20  the suspension store could not be read; nothing was resumed
  21  the governor could not answer before the run resumed; nothing was resumed
  22  the run could not be resumed, the Suspension could not be taken or released, or the resumed
      loop stopped without a run outcome
  1   anything else failed: the state directory, the resume context, the workspace, the models
  2   the command line is not valid
Nothing is performed on any status from 10 to 21.";

/// The exit status of a resume Commission refused, as [`RESUME_HELP`] states it: one per
/// refusal.
pub fn resume_refusal_status(refusal: &ResumeRefusal) -> u8 {
    match refusal {
        ResumeRefusal::NothingSuspended => 10,
        ResumeRefusal::InDoubt { .. } => 11,
        ResumeRefusal::OtherRequest { .. } => 12,
        ResumeRefusal::OtherCommission => 13,
        ResumeRefusal::CaseMoved { .. } => 14,
        ResumeRefusal::NotAdmitted(_) => 15,
        ResumeRefusal::NotGranted(_) => 16,
        ResumeRefusal::NotPerformed => 17,
        ResumeRefusal::RunNotSuspended(_) => 18,
    }
}

/// The exit status of a resume that returned no run outcome, as [`RESUME_HELP`] states it.
pub fn resume_exit_status(error: &ResumeError) -> u8 {
    match error {
        ResumeError::Refused(refusal) => resume_refusal_status(refusal),
        ResumeError::Store(_) => 20,
        ResumeError::Governor(_) => 21,
        ResumeError::Loop(_) => 22,
    }
}

/// The exit status of a refused approval, as [`APPROVE_HELP`] states it.
pub fn approve_exit_status(refusal: &ApproveRefusal) -> u8 {
    match refusal {
        ApproveRefusal::NothingSuspended => 10,
        ApproveRefusal::InDoubt { .. } => 11,
        ApproveRefusal::OtherCapability { .. } => 19,
        ApproveRefusal::AlreadyHandedOff(_) => 23,
        ApproveRefusal::Store(_) | ApproveRefusal::Mandate(_) => 1,
    }
}

/// The exit status `run` and `resume` return for `run`: [`exit_status`] of its stop reason, but
/// [`NOTHING_SUSPENDED_AT_GATE`] for a run with a state directory that stopped at its human gate
/// holding no Suspension, as [`EXIT_STATUS`] and [`RESUME_HELP`] state it.
pub fn run_exit_status(run: &SliceRun) -> u8 {
    match run.gate {
        ApprovalGate::NothingSuspended => NOTHING_SUSPENDED_AT_GATE,
        ApprovalGate::Approvable | ApprovalGate::HandedOff { .. } => exit_status(run.stop_reason),
    }
}

pub mod a2a;
pub mod agent;
pub mod evaluate;
pub mod events;
pub mod mandate;
pub mod model_catalog;
pub mod plugin;
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
    Run(Box<RunArgs>),
    /// Install, inspect, or remove protocol definitions available to this host.
    Protocols(ProtocolsArgs),
    /// Decide a case snapshot and its evidence under a catalog protocol, and write the decision as
    /// JSON. It decides and never acts.
    #[command(after_help = EVALUATE_HELP)]
    Evaluate(EvaluateArgs),
    /// Host a plugin, or print what it proposed. A plugin reads and proposes; it never sends.
    Plugin(PluginArgs),
    /// Check an agent defined as data, a loom.agent/1 spec, before anything runs.
    Agent(AgentArgs),
    /// Approve the request a commission's Suspension holds: write a grant into the state
    /// directory. Approval performs nothing; `resume` does.
    #[command(after_help = APPROVE_HELP)]
    Approve(ApproveArgs),
    /// Continue a commission's suspended run after its approval: perform the suspended request
    /// once, release the Suspension, and run on until the run stops.
    #[command(after_help = RESUME_HELP)]
    Resume(ResumeArgs),
}

/// The arguments of `b10x-loom approve`.
#[derive(Debug, Args)]
pub struct ApproveArgs {
    /// The commission whose Suspension is approved: the id its resume context and Suspension carry.
    #[arg(value_name = "COMMISSION")]
    pub commission: String,
    /// The capability approved; it must be the one the suspended request needs.
    #[arg(value_name = "CAPABILITY")]
    pub capability: String,
    /// The state directory `run --state` kept the commission in. It must exist.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
    /// The host-trusted Mandate file (loom.authority.MandateHost): also record a Mandate approval of
    /// the request, by the file's approver, in the state directory's approval log; the grant names
    /// it, and `resume --mandate` clears it through Mandate.
    #[arg(long, value_name = "FILE")]
    pub mandate: Option<PathBuf>,
}

/// The arguments of `b10x-loom resume`.
#[derive(Debug, Args)]
pub struct ResumeArgs {
    /// The commission whose suspended run is continued.
    #[arg(value_name = "COMMISSION")]
    pub commission: String,
    /// The state directory `run --state` kept the commission in, and `approve` wrote its grant
    /// into. It must exist; the merge hand-off is recorded there.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
    /// The host-trusted Mandate file (loom.authority.MandateHost): perform the request only when
    /// Mandate's check_with_approval clears and consumes the approval the grant names, keeping the
    /// consumption in the state directory's approval log first.
    #[arg(long, value_name = "FILE")]
    pub mandate: Option<PathBuf>,
}

/// What `plugin run --help` says about the configuration, the models, the output and the exit
/// status.
pub const PLUGIN_RUN_HELP: &str = "The configuration is the plugin's JSON configuration file. For slack-handler it is
loom.slack.SlackConfig: the host's plugin configuration under \"plugin\" (the connectors command line,
the data sources a turn may read, the objectives, the classification threshold, the poll interval),
the Slack adapter, connection and read operations, the bot's user id, min_age_minutes, the seed,
lookback_minutes and the objectives each channel serves. It is checked before anything runs.

Each cycle polls, classifies every new item, answers a question in a governed read-only turn over the
data sources and records the proposed reply, records a task's proposed case, and appends one line per
handled item to record.jsonl in the state directory. Nothing is sent. With --once it runs one cycle
and prints one line per proposal it recorded. Without it, it polls every poll_interval_seconds until
SIGTERM or SIGINT, which end it after the item being handled (a second one ends it at once, exit
status 130), and then prints one line per proposal it recorded.

Exit status:
  0  the cycles ran
  1  the configuration, the models or the state directory cannot be used, or the last cycle's poll
     failed (a failed poll is otherwise retried after poll_interval_seconds)
  2  the command line is not valid (an unknown plugin name among them)
  130  a second SIGTERM or SIGINT ended it at once";

/// The plugins `plugin` hosts, by the name the command line takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PluginName {
    /// Walks the Slack channels the bot is a member of and proposes answers to unanswered
    /// messages, mentions first.
    #[value(name = "slack-handler")]
    SlackHandler,
}

/// The arguments of `b10x-loom plugin`.
#[derive(Debug, Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

/// The commands of `b10x-loom plugin`.
#[derive(Debug, Subcommand)]
pub enum PluginCommand {
    /// Host a plugin: poll, classify and answer each new item in a governed read-only turn, and
    /// record one line per handled item. Nothing is sent.
    #[command(after_help = PLUGIN_RUN_HELP)]
    Run(PluginRunArgs),
    /// Print one line per proposal a plugin recorded in its state directory: a proposed reply or
    /// a proposed case.
    Report(PluginReportArgs),
}

/// The arguments of `b10x-loom plugin run`.
#[derive(Debug, Args)]
pub struct PluginRunArgs {
    /// The plugin to host.
    #[arg(value_enum, value_name = "PLUGIN")]
    pub plugin: PluginName,
    /// The plugin's JSON configuration file.
    #[arg(long, value_name = "PATH")]
    pub config: PathBuf,
    /// The plugin's state directory: its cursors and record. It must lie outside every
    /// workspace root and checkout the configuration names; one host holds it at a time.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
    /// Run one cycle and stop.
    #[arg(long)]
    pub once: bool,
    /// An llm catalog (`llm.catalog/1` TOML) in a regular file. With it, `--model` and
    /// `--classifier-model` each name a route alias of this catalog instead of a Codex model; an
    /// alias it does not declare, or a catalog that cannot be read, stops before any model call.
    #[arg(long, value_name = "PATH")]
    pub catalog: Option<PathBuf>,
    /// The model each turn runs on: a Codex model name, or with `--catalog` a route alias.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub model: String,
    /// The model that classifies each item and picks a task's protocol: a Codex model name, or
    /// with `--catalog` a route alias.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL)]
    pub classifier_model: String,
}

/// The arguments of `b10x-loom plugin report`.
#[derive(Debug, Args)]
pub struct PluginReportArgs {
    /// The plugin whose record is read.
    #[arg(value_enum, value_name = "PLUGIN")]
    pub plugin: PluginName,
    /// The plugin's state directory.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
}

/// What `agent check --help` says about the check and its exit status.
pub const AGENT_CHECK_HELP: &str = "The spec is checked against the plugin catalog, which holds inbound-answer@1 and operator-reply@1.
The first refusal is the answer, in this order: the document does not decode (Malformed); a string of
it holds a credential, a URL with a user name or a query, or a token prefix such as xoxb- or sk-
(SecretLiteral; the value is never repeated); the inner loop's protocol is not in the catalog, or
states are declared with no inner loop (UnknownProtocol); a state names an action its protocol lacks
(UnknownAction); a state, the outer loop or the channel names a source no binding declares
(UnboundSource); an http binding's method is not GET (HttpMethod); reply.send is a capability while
writes are none or undeclared (SendWithoutWrites); the channel allows no channel id
(EmptyChannelAllow). Nothing runs and nothing but the spec is read.

Exit status:
  0  serveable: standard output carries \"checked <agent>: <n> states, <m> bindings, protocol <protocol>\"
     (protocol none without loops)
  3  refused: standard error carries \"refused: <reason> at <member>: <detail>\"
  1  the file cannot be read
  2  the command line is not valid";

/// The arguments of `b10x-loom agent`.
#[derive(Debug, Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

/// What `agent serve --help` says about the host and its exit status.
pub const AGENT_SERVE_HELP: &str = "The spec is checked as `agent check` does before anything runs. The host is the plugin host over
the spec: it polls the channel binding's channels that channel.allow lists (a message anywhere else is
no item), classifies each new item, runs a turn on the inner loop's protocol under the state whose
`when` names the item's intent, with the spec's capabilities as its authority, and records one line
per handled item in record.jsonl in the state directory. reply.send is a governed write of the recorded
proposal to the item's channel, at most writes.max_per_hour an hour (sends.jsonl). In writes.mode
approval a send is held: the turn ends proposed naming its commission, the case and the Suspension
stay in the state directory, and `b10x-loom approve <COMMISSION> reply.send --state <DIR>` grants it;
the host performs it at its next poll, or `agent resume` performs it now. writes.mode auto is not
served yet; none sends nothing.

The models are the spec's `model` section (its routes document, relative to the spec, and the
`agent` and `classifier` roles), or with --catalog the route aliases --model and --classifier-model
name. With --once it runs one cycle and prints one line per proposal it recorded; without it, it
polls every poll_interval_seconds until SIGTERM or SIGINT (a second one exits 130 at once).

With expose.a2a the agent also answers A2A on --a2a-address and expose.a2a.port, beside the poll:
the agent card at /.well-known/agent-card.json, and at expose.a2a.path JSON-RPC SendMessage, one
turn on inbound-answer@1 answered with its record (nothing is sent through A2A), and GetTask. Every
request but GET /ready and GET /live needs `Authorization: Bearer` with the token LOOM_A2A_BEARER
holds; the host refuses to start without it. The listener stops when the host does.

Exit status:
  0  the cycles ran
  1  the spec, the models or the state directory cannot be used, or the last cycle's poll failed
  2  the command line is not valid
  3  the spec cannot be served
  130  a second SIGTERM or SIGINT ended it at once";

/// What `agent resume --help` says about the send and its exit status.
pub const AGENT_RESUME_HELP: &str = "Performs the send the commission's Suspension holds, once, after `b10x-loom approve <COMMISSION>
reply.send --state <DIR>` wrote its grant: the write goes through the connectors command line with the
idempotency key derived from the admitted request and the grant file as its proof, the Suspension is
released, and the send is counted. A serving host performs granted sends itself at its next poll.

Exit status:
  0  the send was performed; standard output says what Connectors answered
  1  the spec or a store cannot be used, or the resume failed
  2  the command line is not valid
  3  no send of the commission is held in the state directory
  4  the send is not granted yet; nothing was performed
  5  the resume was refused (in doubt, the case moved), or the send was refused; nothing was sent";

/// The commands of `b10x-loom agent`.
#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Say whether a loom.agent/1 spec can be served: refuse, naming the member and the reason, a
    /// spec that cannot. It runs nothing.
    #[command(after_help = AGENT_CHECK_HELP)]
    Check(AgentCheckArgs),
    /// Host the agent a loom.agent/1 spec declares: poll its channel, answer each item in a
    /// governed turn, and send a reply only as the spec's writes admit.
    #[command(after_help = AGENT_SERVE_HELP)]
    Serve(AgentServeArgs),
    /// Perform a held send after its approval, once.
    #[command(after_help = AGENT_RESUME_HELP)]
    Resume(AgentResumeArgs),
}

/// The arguments of `b10x-loom agent serve`.
#[derive(Debug, Args)]
pub struct AgentServeArgs {
    /// The loom.agent/1 document, YAML or JSON, in a regular file.
    #[arg(long, value_name = "FILE")]
    pub spec: PathBuf,
    /// The host's state directory: its cursors, record, cases, Suspensions, grants and send
    /// counter. One host holds it at a time.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
    /// Run one cycle and stop.
    #[arg(long)]
    pub once: bool,
    /// An llm catalog (`llm.catalog/1` TOML) in a regular file, instead of the spec's models:
    /// `--model` and `--classifier-model` then each name a route alias of it.
    #[arg(long, value_name = "PATH", requires_all = ["model", "classifier_model"])]
    pub catalog: Option<PathBuf>,
    /// The model each turn runs on: a name the spec's routes document serves, or with
    /// `--catalog` a route alias. By default the spec's `agent` role.
    #[arg(long, value_name = "ID")]
    pub model: Option<String>,
    /// The model that classifies each item: a name the spec's routes document serves, or with
    /// `--catalog` a route alias. By default the spec's `classifier` role.
    #[arg(long, value_name = "ID")]
    pub classifier_model: Option<String>,
    /// The address the A2A listener binds when the spec declares `expose.a2a`, on its port.
    #[arg(long, value_name = "IP", default_value = a2a::DEFAULT_ADDRESS)]
    pub a2a_address: std::net::IpAddr,
}

/// The arguments of `b10x-loom agent resume`.
#[derive(Debug, Args)]
pub struct AgentResumeArgs {
    /// The commission whose held send was approved, as the record line names it.
    #[arg(value_name = "COMMISSION")]
    pub commission: String,
    /// The loom.agent/1 document the host served.
    #[arg(long, value_name = "FILE")]
    pub spec: PathBuf,
    /// The host's state directory.
    #[arg(long, value_name = "DIR")]
    pub state: PathBuf,
}

/// The arguments of `b10x-loom agent check`.
#[derive(Debug, Args)]
pub struct AgentCheckArgs {
    /// The loom.agent/1 document, YAML or JSON, in a regular file; a named pipe, a device or a
    /// directory is refused.
    #[arg(long, value_name = "FILE")]
    pub spec: PathBuf,
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
    /// A models configuration (YAML, a regular file): `routes`, the path of an `llm.routes/1`
    /// document relative to the configuration's directory, and `roles`, a list of `{role, model}`
    /// giving the roles `agent` and `classifier` each a route or `<endpoint>/<model-id>` name of
    /// that document, whose credentials are its `secret:` references. With it, `--model` and
    /// `--classifier-model` have no default: given, each overrides its role with a name of the
    /// same document. A configuration that cannot be loaded, a role neither configured nor given,
    /// or a model that cannot take a forced tool call stops the run before any model call. Not
    /// with `--catalog`. With `--state`, the resume context keeps the configuration's absolute
    /// path, and `resume` continues on the model it serves the role `agent`.
    #[arg(long, value_name = "PATH", conflicts_with = "catalog")]
    pub config: Option<PathBuf>,
    /// The model that selects actions and writes their arguments: a Codex model name, with
    /// `--catalog` a route alias of that catalog, or with `--config` a name of its routes
    /// document, overriding the role `agent`.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL, default_value_if("config", ArgPredicate::IsPresent, None::<&str>))]
    pub model: Option<String>,
    /// The model that classifies the intent: a Codex model name, with `--catalog` a route alias
    /// of that catalog, or with `--config` a name of its routes document, overriding the role
    /// `classifier`.
    #[arg(long, value_name = "ID", default_value = DEFAULT_MODEL, default_value_if("config", ArgPredicate::IsPresent, None::<&str>))]
    pub classifier_model: Option<String>,
    /// The confidence, from 0 to 1, below which the router refuses its pick.
    #[arg(long, value_name = "X", default_value_t = 0.5)]
    pub threshold: f64,
    /// How much proof the change must carry, the software-change@1 case input `risk`: each level
    /// owes every obligation of the level below it and one more (tests, review, provenance,
    /// health). A protocol that declares no case input ignores it.
    #[arg(long, value_name = "RISK", default_value = "standard", value_parser = clap::builder::PossibleValuesParser::new(["trivial", "standard", "elevated", "critical"]).map(|value| match value.as_str() {
        "trivial" => Risk::Trivial,
        "elevated" => Risk::Elevated,
        "critical" => Risk::Critical,
        _ => Risk::Standard,
    }))]
    pub risk: Risk,
    /// What the change is, the software-change@1 case input `change_kind`. A bugfix edits the
    /// implementation (implementation.edit) only once a test failed on the current revision.
    #[arg(long, value_name = "KIND", default_value = "feature", value_parser = clap::builder::PossibleValuesParser::new(["feature", "bugfix", "refactor"]).map(|value| match value.as_str() {
        "bugfix" => ChangeKind::Bugfix,
        "refactor" => ChangeKind::Refactor,
        _ => ChangeKind::Feature,
    }))]
    pub change_kind: ChangeKind,
    /// Whether the change affects a security boundary, the software-change@1 case input
    /// `affects_security_boundary`. One that does is verified only once another principal
    /// approved its security review, which the slice never submits.
    #[arg(long, value_name = "BOOL", default_value = "false", action = clap::ArgAction::Set, value_parser = clap::builder::PossibleValuesParser::new(["true", "false"]).map(|value| value == "true"))]
    pub affects_security_boundary: bool,
    /// A test-path pattern, repeatable: tests.write changes only paths one of them matches, for
    /// every change kind; a path outside them refuses the action before anything is written.
    /// Relative to the workspace root; `*` within one path segment, `**` across any number.
    /// Without one: tests/**, **/tests/**, test/**, **/test/**, **/*_test.*, **/test_*.*,
    /// **/*.test.* and **/*.spec.*.
    #[arg(long = "test-path", value_name = "GLOB", value_parser = test_path_pattern)]
    pub test_path: Vec<String>,
    /// The regression-test command, run in the workspace without a shell and split at white
    /// space, with {path} in at least one word. On a bugfix, after each tests.run that follows a
    /// tests.write that committed, it runs once per test path those writes committed, {path}
    /// filled, and only its result is a regression_test_result. Without it no run is one, so a
    /// bugfix's red_first.required stays open.
    #[arg(long, value_name = "CMD", value_parser = regression_test_command)]
    pub regression_test_command: Option<RegressionTestCommand>,
    /// What standard output carries: lines for a person, or one JSON record per line, the last
    /// one the terminal record with the stop reason and the exit status.
    #[arg(long, value_enum, value_name = "FORMAT", default_value_t = Output::Human)]
    pub output: Output,
    /// Keep the run's case, its commission's Suspension and the commission's resume context in
    /// this directory, created when missing, so a run stopped for an approval leaves what a later
    /// resume continues from. Without it nothing outlives the run.
    #[arg(long, value_name = "DIR")]
    pub state: Option<PathBuf>,
    /// A run configuration file (JSON, a regular file) binding an installed protocol's actions to
    /// commands: {"protocol": "<name>@<major>", "timeout_seconds": <n>, "unconfined_bindings":
    /// <true|false>, "bindings": [{"action": "<action>", "argv": ["<program>", "<argument>",
    /// ...]}]}. A run of that protocol starts an
    /// admitted action's program without a shell, with the action's arguments as JSON on standard
    /// input, a cleared environment but PATH, and a timeout (60 s when absent); exit 0 is
    /// performed, with standard output, one JSON object, as the report; any other end is refused.
    /// A binding confers no authority. A file that names an action the protocol does not declare
    /// is refused before any model call; an action it binds no command to stops the run
    /// NoLocalExecutor, naming it. Substrate does not confine these commands, so the run is
    /// refused ConfinementUnavailable (CommandUnconfined), exit 3, before any model call unless
    /// --unconfined-bindings is given or the file sets "unconfined_bindings": true.
    #[arg(long, value_name = "FILE")]
    pub action_bindings: Option<PathBuf>,
    /// Run the commands --action-bindings binds with the operator's rights, outside Substrate.
    /// Without it, and without "unconfined_bindings": true in that file, a run given
    /// --action-bindings is refused ConfinementUnavailable (CommandUnconfined). The flag wins over
    /// the file; standard error reports which one opted in.
    #[arg(long)]
    pub unconfined_bindings: bool,
    /// What to do, as given.
    pub intent: String,
}

impl RunArgs {
    /// The case inputs the flags give, the host's trusted context for the case `run` opens.
    pub fn inputs(&self) -> CaseInputs {
        CaseInputs {
            risk: self.risk,
            change_kind: self.change_kind,
            affects_security_boundary: self.affects_security_boundary,
        }
    }

    /// The test-path patterns the flags give, the host's trusted context `tests.write` is held
    /// to: every `--test-path`, or the defaults without one.
    ///
    /// # Errors
    /// [`TestPathsError`] naming a pattern that is not one.
    pub fn test_paths(&self) -> Result<TestPaths, TestPathsError> {
        if self.test_path.is_empty() {
            Ok(TestPaths::default())
        } else {
            TestPaths::new(self.test_path.iter().cloned())
        }
    }
}

/// `--test-path`'s value, refused unless it is a test-path pattern.
fn test_path_pattern(value: &str) -> Result<String, TestPathsError> {
    TestPaths::new([value]).map(|_| value.to_owned())
}

/// `--regression-test-command`'s value, refused unless a word names `{path}`.
fn regression_test_command(
    value: &str,
) -> Result<RegressionTestCommand, RegressionTestCommandError> {
    RegressionTestCommand::new(value)
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

    /// Every refusal and failure of a resume, and every refusal of an approval that is not a
    /// store failure, has a status of its own, none of them one `run`, clap or a signal takes
    /// (0, 1, 2, 3, 4, 130), and the help names each one by its status. An approval refusal shares a
    /// status only with the resume refusal of the same name.
    #[test]
    fn each_resume_refusal_has_its_own_exit_status() {
        use b10x_loom_commission::model::primitives::Uuid;
        use b10x_loom_commission::model::responsibility::{
            ActionRequestId, GovernorError, ResumeRunOutcome, RevalidateActionRequestOutcome,
        };
        use b10x_loom_commission::ports::authority::{AuthorityCheck, AuthorityProviderError};
        use b10x_loom_commission::ports::suspension::SuspensionStoreError;
        use b10x_loom_commission::runtime::{LoopError, LoopFailure};

        let request = || ActionRequestId(Uuid("request".to_owned()));
        let refusals = [
            ("NothingSuspended", ResumeRefusal::NothingSuspended),
            (
                "InDoubt",
                ResumeRefusal::InDoubt {
                    action_request_id: request(),
                },
            ),
            (
                "OtherRequest",
                ResumeRefusal::OtherRequest {
                    action_request_id: request(),
                    held_action_request_id: request(),
                },
            ),
            ("OtherCommission", ResumeRefusal::OtherCommission),
            (
                "CaseMoved",
                ResumeRefusal::CaseMoved {
                    expected_case_revision: 1,
                    current_case_revision: 2,
                },
            ),
            (
                "NotAdmitted",
                ResumeRefusal::NotAdmitted(RevalidateActionRequestOutcome::Admitted),
            ),
            (
                "NotGranted",
                ResumeRefusal::NotGranted(AuthorityCheck::Refused(AuthorityProviderError::new(
                    "x",
                ))),
            ),
            ("NotPerformed", ResumeRefusal::NotPerformed),
            (
                "RunNotSuspended",
                ResumeRefusal::RunNotSuspended(ResumeRunOutcome::WrongStateUnknownInstance),
            ),
        ];
        let mut errors: Vec<(String, ResumeError)> = refusals
            .into_iter()
            .map(|(name, refusal)| (name.to_owned(), ResumeError::Refused(refusal)))
            .collect();
        errors.extend([
            (
                "the suspension store".to_owned(),
                ResumeError::Store(SuspensionStoreError::new("x")),
            ),
            (
                "the governor".to_owned(),
                ResumeError::Governor(GovernorError::UnknownCase),
            ),
            (
                "the run could not be resumed".to_owned(),
                ResumeError::Loop(LoopError {
                    run_id: None,
                    failure: LoopFailure::Governor(GovernorError::GovernorUnavailable),
                    suspension: None,
                    suspension_reason: None,
                }),
            ),
        ]);
        let mut seen = std::collections::BTreeSet::new();
        for (name, error) in &errors {
            let status = resume_exit_status(error);
            assert!(seen.insert(status), "{name}: {status} is taken twice");
            assert!(
                ![0, 1, 2, 3, 4, 130].contains(&status),
                "{name}: {status} is taken"
            );
            let line = format!("  {status:<3} {name}");
            assert!(
                RESUME_HELP.contains(&line),
                "the help names {name} as {status}: {line:?}"
            );
        }

        for (name, refusal, resumed) in [
            (
                "NothingSuspended",
                ApproveRefusal::NothingSuspended,
                Some(ResumeRefusal::NothingSuspended),
            ),
            (
                "InDoubt",
                ApproveRefusal::InDoubt {
                    action_request_id: "request".to_owned(),
                },
                Some(ResumeRefusal::InDoubt {
                    action_request_id: request(),
                }),
            ),
            (
                "OtherCapability",
                ApproveRefusal::OtherCapability { held: None },
                None,
            ),
            (
                "AlreadyHandedOff",
                ApproveRefusal::AlreadyHandedOff(
                    b10x_loom_intake_slice::handoff::HeadAlreadyHandedOff {
                        commission_id: b10x_loom_intake_slice::handoff::CommissionId(
                            intake_model::primitives::Uuid("commission".to_owned()),
                        ),
                        head: "0123abcd".to_owned(),
                    },
                ),
                None,
            ),
        ] {
            let status = approve_exit_status(&refusal);
            let line = format!("  {status:<3} {name}");
            assert!(APPROVE_HELP.contains(&line), "{name}: {line:?}");
            match resumed {
                Some(resumed) => assert_eq!(status, resume_refusal_status(&resumed), "{name}"),
                None => assert!(
                    seen.insert(status) && ![0, 1, 2, 3, 4, 130].contains(&status),
                    "{name}: {status} is taken"
                ),
            }
        }
    }

    /// A run or a resume with a state directory that stopped at its human gate holding no
    /// Suspension has a status of its own, which both helps name; every other run's status is its
    /// stop reason's, a hand-off's gate included.
    #[test]
    fn a_gate_with_nothing_suspended_has_its_own_exit_status() {
        let run = |gate: ApprovalGate| SliceRun {
            protocol: "software-change@1".to_owned(),
            steps: 3,
            stop_reason: StopReason::ApprovalRequired,
            approvals: vec!["repository.merge".to_owned()],
            unperformed: Vec::new(),
            gate,
        };
        assert_eq!(run_exit_status(&run(ApprovalGate::Approvable)), 0);
        let handed_off = ApprovalGate::HandedOff {
            action_request_id: "request".to_owned(),
            head: "0123abcd".to_owned(),
        };
        assert_eq!(run_exit_status(&run(handed_off)), 0);
        let status = run_exit_status(&run(ApprovalGate::NothingSuspended));
        assert_eq!(status, NOTHING_SUSPENDED_AT_GATE);
        assert!(![0, 1, 2, 3, 130].contains(&status));
        assert!(EXIT_STATUS.contains(&format!("  {status}  with --state")));
        assert!(RESUME_HELP.contains(&format!(
            "  {status:<3} the resumed run reached its human gate holding no Suspension"
        )));
        for help in [EXIT_STATUS, APPROVE_HELP, RESUME_HELP] {
            assert!(
                help.contains("`stopped: ApprovalRequired (<action>)`")
                    || help.contains("stopped: ApprovalRequired (<action>)  "),
                "{help}"
            );
            assert!(help.contains("nothing suspended"), "{help}");
        }
    }

    #[test]
    fn each_plugin_name_is_the_name_its_crate_registers() {
        let names: Vec<String> = PluginName::value_variants()
            .iter()
            .filter_map(|name| name.to_possible_value())
            .map(|value| value.get_name().to_owned())
            .collect();
        assert_eq!(names, [b10x_loom_plugin_slack::NAME]);
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
    /// No input flag gives the slice's run defaults; each flag takes exactly the values
    /// `software.change/1` declares, and nothing else.
    #[test]
    fn the_case_input_flags_default_to_the_run_defaults_and_take_the_declared_values() {
        let run = |extra: &[&str]| {
            let mut argv = vec!["b10x-loom", "run", "--workspace", "."];
            argv.extend_from_slice(extra);
            argv.push("fix tests");
            Cli::try_parse_from(argv).map(|cli| match cli.command {
                Command::Run(run) => run.inputs(),
                _ => panic!("run command"),
            })
        };
        assert_eq!(
            run(&[]).unwrap(),
            b10x_loom_intake_slice::case::RUN_DEFAULTS
        );
        assert_eq!(
            run(&[
                "--risk",
                "critical",
                "--change-kind",
                "bugfix",
                "--affects-security-boundary",
                "true",
            ])
            .unwrap(),
            CaseInputs {
                risk: Risk::Critical,
                change_kind: ChangeKind::Bugfix,
                affects_security_boundary: true,
            }
        );
        for (risk, expected) in [
            ("trivial", Risk::Trivial),
            ("standard", Risk::Standard),
            ("elevated", Risk::Elevated),
        ] {
            assert_eq!(run(&["--risk", risk]).unwrap().risk, expected);
        }
        assert_eq!(
            run(&["--change-kind", "refactor"]).unwrap().change_kind,
            ChangeKind::Refactor
        );
        for refused in [
            &["--risk", "extreme"][..],
            &["--change-kind", "chore"],
            &["--affects-security-boundary", "yes"],
            &["--affects-security-boundary"],
        ] {
            assert!(run(refused).is_err(), "{refused:?} is refused");
        }
    }

    /// Without `--test-path` the default patterns apply; each `--test-path` adds one, in the
    /// order given, replacing the defaults; a value that is not a pattern is refused.
    #[test]
    fn the_test_path_flag_repeats_and_replaces_the_defaults() {
        let run = |extra: &[&str]| {
            let mut argv = vec!["b10x-loom", "run", "--workspace", "."];
            argv.extend_from_slice(extra);
            argv.push("fix tests");
            Cli::try_parse_from(argv).map(|cli| match cli.command {
                Command::Run(run) => run.test_paths().expect("checked patterns"),
                _ => panic!("run command"),
            })
        };
        assert_eq!(run(&[]).unwrap(), TestPaths::default());
        assert_eq!(
            run(&["--test-path", "spec/**", "--test-path", "**/*_spec.rb"])
                .unwrap()
                .patterns(),
            ["spec/**", "**/*_spec.rb"]
        );
        for refused in ["", "/abs/**", "../x", "a**"] {
            assert!(
                run(&["--test-path", refused]).is_err(),
                "`{refused}` is refused"
            );
        }
    }

    /// Without `--regression-test-command` a run has none; with one it keeps the text as given;
    /// a command naming no `{path}` word is refused.
    #[test]
    fn the_regression_test_command_flag_needs_the_path_placeholder() {
        let run = |extra: &[&str]| {
            let mut argv = vec!["b10x-loom", "run", "--workspace", "."];
            argv.extend_from_slice(extra);
            argv.push("fix tests");
            Cli::try_parse_from(argv).map(|cli| match cli.command {
                Command::Run(run) => run.regression_test_command,
                _ => panic!("run command"),
            })
        };
        assert_eq!(run(&[]).unwrap(), None);
        assert_eq!(
            run(&["--regression-test-command", "cargo test --test {path}"])
                .unwrap()
                .as_ref()
                .map(RegressionTestCommand::text),
            Some("cargo test --test {path}")
        );
        for refused in ["", "cargo test", "  "] {
            assert!(
                run(&["--regression-test-command", refused]).is_err(),
                "`{refused}` is refused"
            );
        }
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
