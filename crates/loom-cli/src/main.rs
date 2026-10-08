//! Run a routed intent, initializing resources only for the selected protocol.

use std::io::Write;
use std::path::Path;
use std::process::{Command as Process, ExitCode};
use std::sync::Arc;

use b10x_llm_tool_call::codex_model;
use b10x_loom_cli::evaluate::{decision_json, refusal_json, refused_input, request_from_json};
use b10x_loom_cli::events::{EventStream, FAILED};
use b10x_loom_cli::model_catalog::ModelCatalog;
use b10x_loom_cli::regular_file;
use b10x_loom_cli::{
    Cli, Command, Confinement, EvaluateArgs, Output, ProtocolCommand, RunArgs, exit_status,
};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::clock::HostClock;
use b10x_loom_intake_slice::confinement::{
    ConfinementRefusal, SubstrateRunner, TestRunner, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::context_metrics::ContextMetrics;
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::intent::{
    IntentRequest, Preparation, PreparedIntent, prepare_intent, run_prepared_intent,
};
use b10x_loom_intake_slice::run::{LOCAL_PROTOCOL, RunOptions, SliceRun, StopReason, printable};
use clap::Parser;
use llm_core::Model;
use llm_models::Port;
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::InstallStore;
use serde_json::{Value, json};

const REEXEC_MARKER: &str = "B10X_LOOM_CONFINEMENT_REEXEC";
const REEXEC_READY: &str = "B10X_LOOM_CONFINEMENT_READY";
const REEXEC_HANDOFF: &str = "B10X_LOOM_CONFINEMENT_HANDOFF";

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Run(arguments) => run_command(arguments),
        Command::Protocols(arguments) => match protocols(arguments.command) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => failed(&error),
        },
        Command::Evaluate(arguments) => evaluate_command(&arguments),
    }
}

/// The largest request `evaluate` reads.
const MAX_REQUEST_BYTES: u64 = 16 * 1024 * 1024;

/// Runs `evaluate`: the request from `--input` or standard input, the protocol from this host's
/// catalog, the decision or the refusal on standard output as `EVALUATE_HELP` states.
fn evaluate_command(arguments: &EvaluateArgs) -> ExitCode {
    let text = match read_request(arguments.input.as_deref()) {
        Ok(text) => text,
        Err(error) => return failed(&error),
    };
    let catalog = match InstallStore::user().and_then(|store| store.catalog()) {
        Ok(catalog) => catalog,
        Err(error) => return failed(&error),
    };
    let decided =
        request_from_json(&text).and_then(|request| loom_governor::evaluate(&catalog, &request));
    let (written, status) = match &decided {
        Ok(decision) => (decision_json(decision), ExitCode::SUCCESS),
        Err(refusal) => {
            eprintln!(
                "b10x-loom: evaluation refused: {}: {}",
                refused_input(refusal),
                printable(&refusal.message)
            );
            (refusal_json(refusal), ExitCode::from(3))
        }
    };
    let mut out = std::io::stdout().lock();
    match out.write_all(written.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => status,
        Err(error) => failed(&format!("the decision cannot be written: {error}")),
    }
}

fn read_request(path: Option<&Path>) -> Result<String, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    let read = match path {
        Some(path) => regular_file::open(path)
            .map_err(|reason| format!("the request {} {reason}", path.display()))?
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes),
        None => std::io::stdin()
            .lock()
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes),
    };
    read.map_err(|error| format!("the request cannot be read: {error}"))?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err("the request exceeds 16 MiB".into());
    }
    String::from_utf8(bytes).map_err(|_| "the request is not UTF-8".into())
}

fn failed(error: &str) -> ExitCode {
    eprintln!("b10x-loom: {}", printable(error));
    ExitCode::from(FAILED)
}

/// How `execute` ended: a stop reason, or a delegated child that wrote its own output and whose
/// exit status this process returns.
enum Ending {
    Stopped(SliceRun),
    Delegated(ExitCode),
}

fn protocols(command: ProtocolCommand) -> Result<(), String> {
    let store = InstallStore::user()?;
    match command {
        ProtocolCommand::List => {
            for entry in store.catalog()?.iter() {
                let source = &entry.definition.source;
                let execution = if entry.name() == LOCAL_PROTOCOL {
                    "software change".into()
                } else {
                    entry
                        .clock_compatible()
                        .map(|()| "clock".to_owned())
                        .unwrap_or_else(|error| error)
                };
                println!(
                    "{}  {:?} {} {} {}  sha256:{}  {}",
                    printable(entry.name()),
                    source.kind,
                    printable(&source.location),
                    printable(&source.revision),
                    printable(&source.path),
                    printable(&entry.definition.sha256),
                    printable(&execution)
                );
            }
        }
        ProtocolCommand::Add(arguments) => {
            if let Some(file) = arguments.file {
                store.install_file(&arguments.name, &file, arguments.replace)?;
            } else {
                let source = arguments.source.as_deref().ok_or("--source is required")?;
                let path = arguments
                    .path
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .ok_or("--path must be UTF-8")?;
                store.install_git(&arguments.name, source, path, arguments.replace)?;
            }
            println!("installed {}", printable(&arguments.name));
        }
        ProtocolCommand::Remove { name } => {
            store.remove(&name)?;
            println!("removed {}", printable(&name));
        }
    }
    Ok(())
}

/// Runs `run`; with `--output jsonl` standard output carries the event stream instead of the human
/// lines, ended by the one terminal record whose exit status this returns. A delegated child owns
/// the final measurement output and, with the stream, the terminal record.
fn run_command(arguments: RunArgs) -> ExitCode {
    let options = RunOptions {
        context_policy: arguments.context_policy,
        context_report: arguments.context_report.clone(),
    };
    let stream = (arguments.output == Output::Jsonl).then(|| EventStream::new(std::io::stdout()));
    let mut metrics = ContextMetrics::new(options.context_policy);
    let result = {
        let mut out: Box<dyn Write> = match stream {
            Some(_) => Box::new(std::io::sink()),
            None => Box::new(std::io::stdout().lock()),
        };
        execute(
            &arguments,
            &options,
            &mut metrics,
            &mut *out,
            stream.as_ref(),
        )
    };
    let report = options.context_report.as_ref().map(|path| {
        if matches!(result, Ok(Ending::Delegated(_))) {
            metrics.finish_delegated_report(path)
        } else {
            metrics.write(path)
        }
    });
    let result = match report {
        Some(Err(error)) => {
            let original = result.err().map(|e| format!("{e}; ")).unwrap_or_default();
            Err(format!(
                "{original}context report cannot be written: {error}"
            ))
        }
        _ => result,
    };
    match result {
        Ok(Ending::Delegated(status)) => status,
        Ok(Ending::Stopped(run)) => match &stream {
            Some(stream) => match stream.finish(Ok(&run)) {
                Ok(status) => ExitCode::from(status),
                Err(error) => failed(&format!("the event stream cannot be written: {error}")),
            },
            None => ExitCode::from(exit_status(run.stop_reason)),
        },
        Err(error) => {
            let status = failed(&error);
            if let Some(stream) = &stream
                && let Err(written) = stream.finish(Err(printable(&error).as_str()))
            {
                return failed(&format!("the event stream cannot be written: {written}"));
            }
            status
        }
    }
}

fn execute(
    arguments: &RunArgs,
    options: &RunOptions,
    metrics: &mut ContextMetrics,
    out: &mut dyn Write,
    stream: Option<&EventStream>,
) -> Result<Ending, String> {
    if !(0.0..=1.0).contains(&arguments.threshold) {
        return Err(format!(
            "the threshold {} is not a number from 0 to 1",
            arguments.threshold
        ));
    }
    // Both models are settled here, before any model call: an alias the catalog does not name
    // or a catalog that cannot be read stops the run now.
    let (mut classifier_route, mut agent_route) = match &arguments.catalog {
        Some(path) => {
            let models = ModelCatalog::read(path)?;
            (
                Some(models.model("--classifier-model", &arguments.classifier_model)?),
                Some(models.model("--model", &arguments.model)?),
            )
        }
        None => (None, None),
    };
    let catalog = InstallStore::user()?.catalog()?;
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .map_err(|e| e.to_string())?;
    let mut words = arguments.test_cmd.split_whitespace();
    let program = words.next().unwrap_or("");
    let mut request = IntentRequest {
        intent: arguments.intent.clone(),
        workspace: arguments.workspace.clone(),
        test: TestCommand::new(program, words),
        runner: None,
        max_steps: arguments.max_steps,
        threshold: arguments.threshold,
    };
    let prepared = if std::env::var_os(REEXEC_MARKER).is_some() {
        let handoff = read_handoff()?;
        *metrics = ContextMetrics::restore(&handoff["metrics"], options.context_policy)?;
        PreparedIntent::restore(&handoff["route"], &request, &catalog, options)
            .map_err(|e| e.to_string())?
    } else {
        let classifier = chosen_model(classifier_route.take(), &arguments.classifier_model)?;
        let observed = stream.map(|stream| stream.observe(&*classifier));
        let classifier: &dyn Model = match &observed {
            Some(observed) => observed,
            None => &*classifier,
        };
        let preparation = prepare_intent(&request, &catalog, classifier, out, options, metrics)
            .map_err(|e| e.to_string())?;
        let (protocol, accepted) = match &preparation {
            Preparation::Ready(prepared) => (prepared.protocol(), true),
            Preparation::Stopped(run) => (run.protocol.as_str(), false),
        };
        if let Some(stream) = stream {
            stream
                .route(protocol, accepted)
                .map_err(|e| format!("the event stream cannot be written: {e}"))?;
        }
        match preparation {
            Preparation::Ready(prepared) => prepared,
            Preparation::Stopped(run) => return Ok(Ending::Stopped(run)),
        }
    };
    if prepared.protocol() == LOCAL_PROTOCOL {
        let workspace = request.workspace.as_deref().ok_or(
            "software-change@1 requires --workspace with an existing Git worktree and commit",
        )?;
        case::validate_workspace(workspace).map_err(|e| e.to_string())?;
        if program.is_empty() {
            return Err("--test-cmd names no program".into());
        }
        request.runner = Some(match arguments.confinement {
            Confinement::None => Arc::new(UnconfinedRunner) as Arc<dyn TestRunner>,
            Confinement::Substrate => {
                let root = if std::env::var_os(REEXEC_MARKER).is_some()
                    && arguments.cgroup_root.is_none()
                {
                    match prepare_delegated_cgroup() {
                        Ok(root) => Some(root),
                        Err(error) => {
                            return Ok(confinement_refused(&prepared, &error.to_string()));
                        }
                    }
                } else {
                    arguments.cgroup_root.clone()
                };
                let runner = SubstrateRunner::new(root);
                if let Err(error) = runner.preflight(workspace) {
                    if error.reason == ConfinementRefusal::CgroupUndelegated
                        && std::env::var_os(REEXEC_MARKER).is_none()
                    {
                        out.flush().map_err(|e| e.to_string())?;
                        // Preserve classification even if systemd cannot start the delegated process.
                        if let Some(path) = &options.context_report {
                            metrics.write(path).map_err(|e| e.to_string())?;
                        }
                        return Ok(reexec(&prepared, metrics));
                    }
                    return Ok(confinement_refused(&prepared, &error.to_string()));
                }
                Arc::new(runner)
            }
        });
    }
    if std::env::var_os(REEXEC_MARKER).is_some() {
        let ready = std::env::var_os(REEXEC_READY).ok_or("delegation handshake path is missing")?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(ready)
            .map_err(|e| format!("delegation handshake failed: {e}"))?;
    }
    let agent = chosen_model(agent_route.take(), &arguments.model)?;
    let observed = stream.map(|stream| stream.observe(&*agent));
    let agent: &dyn Model = match &observed {
        Some(observed) => observed,
        None => &*agent,
    };
    let run = run_prepared_intent(
        &request, &prepared, &catalog, &governor, &governor, agent, &HostClock, out, options,
        metrics,
    )
    .map_err(|e| e.to_string())?;
    Ok(Ending::Stopped(run))
}

/// The catalog route's model when `--catalog` named one, otherwise the Codex model `name`.
fn chosen_model(route: Option<Port>, name: &str) -> Result<Box<dyn Model>, String> {
    match route {
        Some(port) => Ok(Box::new(port)),
        None => codex_model(name)
            .map(|model| Box::new(model) as Box<dyn Model>)
            .map_err(|e| e.to_string()),
    }
}

fn confinement_refused(prepared: &PreparedIntent, reason: &str) -> Ending {
    eprintln!("confinement: substrate");
    eprintln!("stopped: ConfinementUnavailable ({})", printable(reason));
    Ending::Stopped(SliceRun {
        protocol: prepared.protocol().to_owned(),
        steps: 0,
        stop_reason: StopReason::ConfinementUnavailable,
        approvals: Vec::new(),
    })
}

fn reexec(prepared: &PreparedIntent, metrics: &ContextMetrics) -> Ending {
    let run = || -> Result<Ending, String> {
        let state = tempfile::Builder::new()
            .prefix("loom-delegation-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(state.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let ready = state.path().join("ready");
        let handoff = state.path().join("handoff.json");
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            file.mode(0o600);
        }
        let file = file.open(&handoff).map_err(|e| e.to_string())?;
        serde_json::to_writer(
            file,
            &json!({"route":prepared.handoff(),"metrics":metrics.snapshot()}),
        )
        .map_err(|e| e.to_string())?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = Process::new("systemd-run")
            .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
            .arg(executable)
            .args(std::env::args_os().skip(1))
            .env(REEXEC_MARKER, "1")
            .env(REEXEC_READY, &ready)
            .env(REEXEC_HANDOFF, &handoff)
            .status()
            .map_err(|e| format!("systemd-run unavailable: {e}"))?;
        // Consumption proves the child received the route, even if its later preflight failed.
        if !handoff.exists() {
            return Ok(Ending::Delegated(ExitCode::from(
                status
                    .code()
                    .and_then(|n| u8::try_from(n).ok())
                    .unwrap_or(1),
            )));
        }
        Err(format!(
            "delegated run exited {status}; configure a working user systemd manager or pass --cgroup-root"
        ))
    };
    run().unwrap_or_else(|error| {
        confinement_refused(prepared, &format!("CgroupUndelegated: {error}"))
    })
}

fn read_handoff() -> Result<Value, String> {
    let path =
        std::env::var_os(REEXEC_HANDOFF).ok_or("delegation requires a private routing handoff")?;
    let ready = std::env::var_os(REEXEC_READY).ok_or("delegation handshake path is missing")?;
    read_private_handoff(Path::new(&path), Path::new(&ready))
}

fn read_private_handoff(path: &Path, ready: &Path) -> Result<Value, String> {
    let parent = path.parent().ok_or("invalid delegation handoff path")?;
    if path.file_name().and_then(|s| s.to_str()) != Some("handoff.json")
        || !parent
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with("loom-delegation-"))
    {
        return Err("invalid delegation handoff path".into());
    }
    if ready != parent.join("ready") {
        return Err("delegation handshake does not match handoff".into());
    }
    let file = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let directory = std::fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
    if !file.is_file() || !directory.is_dir() || file.len() > 1024 * 1024 {
        return Err("invalid delegation handoff file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if file.mode() & 0o777 != 0o600
            || directory.mode() & 0o777 != 0o700
            || file.uid() != directory.uid()
        {
            return Err("delegation handoff is not private".into());
        }
    }
    let value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("invalid delegation handoff: {e}"))?;
    std::fs::remove_file(path).map_err(|e| e.to_string())?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn delegation_reads_only_a_private_regular_file_and_consumes_it() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::Builder::new()
            .prefix("loom-delegation-")
            .tempdir()
            .unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("handoff.json");
        let ready = directory.path().join("ready");
        std::fs::write(&path, br#"{"route":"fixture"}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            read_private_handoff(&path, &ready)
                .unwrap_err()
                .contains("not private")
        );
        assert!(path.exists());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_private_handoff(&path, &directory.path().join("wrong-ready")).is_err());
        assert_eq!(
            read_private_handoff(&path, &ready).unwrap()["route"],
            "fixture"
        );
        assert!(!path.exists());
        let target = directory.path().join("other");
        std::fs::write(&target, "{}").unwrap();
        symlink(target, &path).unwrap();
        assert!(
            read_private_handoff(&path, &ready)
                .unwrap_err()
                .contains("invalid delegation handoff file")
        );
    }
}
