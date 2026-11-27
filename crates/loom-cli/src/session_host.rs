//! The A2A host for an explicitly requested Claude Code or Codex session.
use b10x_loom_a2a::Answerer;
use b10x_loom_agent::serve::a2a::{RecordedAnswers, session_admission};
use b10x_loom_agent::serve::{SpecAuthority, commission_id};
use b10x_loom_cli::dashboard::{A2aIngress, Dashboard, Harness};
use b10x_loom_cli::{AgentServeArgs, a2a};
use b10x_loom_plugin::observe::TurnObserver;
use b10x_loom_plugin::state::{HeldRecord, StateDir};
use b10x_loom_plugin::turn::session::SessionProtocol;
use b10x_loom_plugin::{InboundItem, RecordLine, record_line, turn::stopped};
use loom_sdk::EventSink;
use loom_sdk::agent::{AgentSpec, SessionKind};
use loom_sdk::session::SessionExecutor;
use std::path::{Component, Path};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

fn valid_inputs(workspace: &Path, files: &[String]) -> bool {
    workspace.is_dir()
        && !files.is_empty()
        && files.iter().all(|file| {
            let path = Path::new(file);
            !file.is_empty()
                && path
                    .components()
                    .all(|p| matches!(p, Component::Normal(part) if part != ".git"))
                && file
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        })
}

pub(super) fn serve(
    args: &AgentServeArgs,
    spec: AgentSpec,
    source: &str,
    state: &Path,
    workspace: &Path,
) -> Result<(), String> {
    let session = spec
        .session
        .as_ref()
        .ok_or("a session host requires session.kind")?;
    if !session.tools.egress.is_empty() {
        return Err("session hosting cannot enforce custom egress apertures".into());
    }
    let validated = SessionExecutor::from_spec(&spec, workspace, args.session_file.clone());
    let admitted = session_admission(
        &spec,
        valid_inputs(workspace, &args.session_file),
        validated.is_ok(),
    );
    validated.map_err(|e| e.to_string())?;
    let kind = admitted?;
    let harness = match kind {
        SessionKind::ClaudeCode => Harness::Claude,
        SessionKind::Codex => Harness::Codex,
        SessionKind::Harness => return Err("unsupported external session harness".into()),
    };
    let bearer = b10x_loom_cli::dashboard::agent_bearer()?.ok_or("the A2A bearer is required")?;
    let (server, socket) =
        a2a::prepare(&spec, args.a2a_address, Some(&bearer))?.ok_or("no A2A listener")?;
    let path = &spec
        .expose
        .as_ref()
        .and_then(|e| e.a2a.as_ref())
        .ok_or("no A2A exposure")?
        .path;
    let ingress = A2aIngress::new(&socket, path, &bearer)?;
    let state_dir = StateDir::for_workspace(state, workspace).map_err(|e| e.to_string())?;
    let protocol = SessionProtocol::open(state)?;
    let stop = Arc::new(AtomicBool::new(false));
    super::stop_on_signals(stop.clone())?;
    let board = Dashboard::start_session(
        state,
        &spec,
        source,
        harness,
        ingress,
        workspace,
        &args.session_file,
    )?;
    let authority = SpecAuthority::new(
        spec.policy
            .as_ref()
            .map(|p| p.capabilities.clone())
            .unwrap_or_default(),
        spec.writes
            .as_ref()
            .map(|w| w.mode)
            .unwrap_or(loom_sdk::agent::WriteMode::None),
    );
    let answering = Sessions {
        spec: &spec,
        observer: board.observer(),
        authority,
        record: state_dir.held(),
        replay: RecordedAnswers::default(),
        protocol,
        workspace,
        files: &args.session_file,
    };
    eprintln!(
        "b10x-loom: A2A listening on {}",
        socket.local_addr().map_err(|e| e.to_string())?
    );
    server
        .serve(socket, &answering, &stop)
        .map_err(|e| e.to_string())
}

struct Sessions<'a> {
    spec: &'a AgentSpec,
    observer: Arc<dyn TurnObserver>,
    authority: SpecAuthority,
    record: HeldRecord,
    replay: RecordedAnswers,
    protocol: SessionProtocol,
    workspace: &'a Path,
    files: &'a [String],
}

impl Answerer for Sessions<'_> {
    fn answer(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        self.replay.answer(
            &self.record,
            item,
            events,
            Some(self.observer.as_ref()),
            |sink| self.turn(item, sink),
        )
    }
}

impl Sessions<'_> {
    fn turn(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        // The protocol offers only this capability. A denied turn can finish before a vendor starts.
        if !self.authority.holds("session.propose") {
            return Ok(record_line(
                item,
                None,
                stopped(Vec::new(), "Session lacks authority to propose"),
            ));
        }
        let spec = self.spec;
        let session = spec
            .session
            .as_ref()
            .ok_or("session configuration unavailable")?;
        // A new executor confines the runtime's session record and private transcript to this item.
        let mut executor = SessionExecutor::from_spec(spec, self.workspace, self.files.to_vec())
            .map_err(|_| "session configuration unavailable")?;
        let work = serde_json::to_string(&item.text).map_err(|_| "invalid work request")?;
        executor.brief = format!(
            "{}\n\nWork request (JSON string): {work}\n\
            Return a final JSON line with action \"session.propose\" and a summary. \
            Changes in the scratch copy are proposals.",
            spec.agent.brief
        );
        executor.timeout = Some(Duration::from_millis(
            session.budget.max_duration_ms.unwrap_or(120_000).max(1) as u64,
        ));
        let (executor, mut observed) =
            b10x_loom_cli::metering::observe_session(executor, &item.id.0, events);
        let result = self.protocol.run(
            &executor,
            item,
            commission_id(&spec.agent.id.0, &item.id.0, &item.revision),
            session,
            &self.authority,
            &mut observed,
        );
        match result {
            Ok(line) => Ok(line),
            Err(error) => {
                let reason = "The governed session could not finish";
                eprintln!(
                    "b10x-loom: governed session failed: {}",
                    b10x_loom_intake_slice::run::printable(&error)
                );
                // An infrastructure failure after a vendor started must also acquire a durable reply.
                if executor.last_report().is_some_and(|report| report.started) {
                    Ok(record_line(item, None, stopped(Vec::new(), reason)))
                } else {
                    Err(reason.into())
                }
            }
        }
    }
}
