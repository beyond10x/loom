//! Real governed query workflow with recorded models and an injected clock; no network.
use b10x_loom_intake_slice::{
    clock::{Clock, READ_TIME},
    context::ContextPolicy,
    executor::TestCommand,
    intent::{IntentRequest, run_intent_with_options},
    run::{RunOptions, StopReason},
};
use chrono::{DateTime, FixedOffset};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
const INTENT: &str = "need to know the current time";
const YAML: &str = include_str!("../../../protocols/system-query/1.yaml");
struct FixedClock {
    reads: Cell<usize>,
    fail: bool,
    instant: &'static str,
}
impl Clock for FixedClock {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        self.reads.set(self.reads.get() + 1);
        if self.fail {
            Err("fixture clock unavailable".into())
        } else {
            DateTime::parse_from_rfc3339(self.instant).map_err(|e| e.to_string())
        }
    }
}
fn request(workspace: Option<PathBuf>) -> IntentRequest {
    IntentRequest {
        intent: INTENT.into(),
        workspace,
        runner: None,
        test: TestCommand::new("must-never-run", std::iter::empty::<&str>()),
        max_steps: 2,
        threshold: 0.5,
    }
}
fn classifier(protocol: &str) -> Recorded {
    Recorded::new(vec![args_for(
        "pick_protocol",
        json!({"protocol":protocol,"confidence":0.99,"reasons":["read clock"]}),
    )])
}
fn run_case(
    catalog: &ProtocolCatalog,
    name: &str,
    policy: ContextPolicy,
    workspace: Option<PathBuf>,
) {
    let gov = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(catalog)
        .unwrap();
    let router = classifier(name);
    let agent = Recorded::new(vec![select(READ_TIME), args(json!({}))]);
    let clock = FixedClock {
        reads: Cell::new(0),
        fail: false,
        instant: "2026-10-07T00:30:00+02:00",
    };
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("metrics.json");
    let mut output = Vec::new();
    let result = run_intent_with_options(
        &request(workspace),
        catalog,
        &gov,
        &gov,
        &router,
        &agent,
        &clock,
        &mut output,
        &RunOptions {
            context_policy: policy,
            context_report: Some(report.clone()),
        },
    )
    .unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    assert_eq!(result.steps, 1);
    assert_eq!(clock.reads.get(), 1);
    router.assert_consumed();
    agent.assert_consumed();
    let out = String::from_utf8(output).unwrap();
    assert!(
        out.contains("Local time: 2026-10-07T00:30:00+02:00"),
        "{out}"
    );
    assert!(out.contains("UTC: 2026-10-06T22:30:00Z"), "{out}");
    assert!(out.contains("stopped: Completed (answered)"), "{out}");
    assert!(!out.contains("confinement:"), "{out}");
    let metrics: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(metrics["model_calls"], 3);
    assert!(!metrics.to_string().contains(INTENT));
    assert_eq!(metrics["requests"][0]["input_tokens"], Value::Null);
    for r in router.requests().iter().chain(agent.requests().iter()) {
        assert!(serde_json::to_vec(r).unwrap().len() <= 65536);
    }
}
#[test]
fn clock_query_runs_in_both_policies_without_workspace_resources() {
    let catalog = ProtocolCatalog::bundled().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let missing = scratch.path().join("never-created");
    for policy in [ContextPolicy::Legacy, ContextPolicy::Bounded] {
        for path in [None, Some(PathBuf::from("/tmp")), Some(missing.clone())] {
            run_case(&catalog, "system-query@1", policy, path);
        }
    }
    assert!(!missing.exists());
}
#[test]
fn local_snapshot_with_custom_identity_uses_the_same_clock_binding() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("clock.yaml");
    std::fs::write(&source, YAML.replace("id: system.query", "id: team.clock")).unwrap();
    let store = InstallStore::new(dir.path().join("installed"));
    store.install_file("team-clock@1", &source, false).unwrap();
    std::fs::remove_file(&source).unwrap();
    run_case(
        &store.catalog().unwrap(),
        "team-clock@1",
        ContextPolicy::Bounded,
        None,
    );
}
#[test]
fn pinned_git_snapshot_executes_offline_after_its_source_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(&source)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init", "--quiet"]);
    std::fs::write(
        source.join("clock.yaml"),
        YAML.replace("id: system.query", "id: package.clock"),
    )
    .unwrap();
    git(&["add", "clock.yaml"]);
    git(&["commit", "--quiet", "-m", "clock fixture"]);
    let sha = git(&["rev-parse", "HEAD"]);
    let store = InstallStore::new(dir.path().join("installed"));
    store
        .install_git(
            "package-clock@1",
            &format!("git+file://{}#{sha}", source.display()),
            "clock.yaml",
            false,
        )
        .unwrap();
    std::fs::remove_dir_all(source).unwrap();
    run_case(
        &store.catalog().unwrap(),
        "package-clock@1",
        ContextPolicy::Bounded,
        None,
    );
}
#[test]
fn fabricated_clock_arguments_and_write_selection_never_read_the_clock() {
    for replies in [
        vec![
            select(READ_TIME),
            args(json!({"utc":"forged","evidence":"observed"})),
        ],
        vec![select("repository.edit")],
    ] {
        let catalog = ProtocolCatalog::bundled().unwrap();
        let gov = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .unwrap();
        let clock = FixedClock {
            reads: Cell::new(0),
            fail: false,
            instant: "2026-10-07T00:30:00+02:00",
        };
        let mut req = request(None);
        req.max_steps = 1;
        let mut out = Vec::new();
        let result = run_intent_with_options(
            &req,
            &catalog,
            &gov,
            &gov,
            &classifier("system-query@1"),
            &Recorded::new(replies),
            &clock,
            &mut out,
            &RunOptions::default(),
        )
        .unwrap();
        assert_ne!(result.stop_reason, StopReason::Completed);
        assert_eq!(clock.reads.get(), 0);
        assert!(
            !String::from_utf8(out)
                .unwrap()
                .contains("system_time observed")
        );
    }
}
#[test]
fn clock_failure_has_no_success_evidence_and_retains_metrics() {
    let catalog = ProtocolCatalog::bundled().unwrap();
    let gov = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let clock = FixedClock {
        reads: Cell::new(0),
        fail: true,
        instant: "unused",
    };
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.json");
    let mut out = Vec::new();
    let error = run_intent_with_options(
        &request(None),
        &catalog,
        &gov,
        &gov,
        &classifier("system-query@1"),
        &Recorded::new(vec![select(READ_TIME), args(json!({}))]),
        &clock,
        &mut out,
        &RunOptions {
            context_policy: ContextPolicy::Bounded,
            context_report: Some(report.clone()),
        },
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("fixture clock unavailable"),
        "{error}"
    );
    assert!(
        !String::from_utf8(out)
            .unwrap()
            .contains("system_time observed")
    );
    let metrics: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(metrics["model_calls"], 3);
}
type Answer = Box<dyn FnOnce(&TurnRequest) -> Value + Send>;
struct Reply {
    tool: &'static str,
    answer: Answer,
}
fn reply(tool: &'static str, answer: impl FnOnce(&TurnRequest) -> Value + Send + 'static) -> Reply {
    Reply {
        tool,
        answer: Box::new(answer),
    }
}
fn args_for(tool: &'static str, value: Value) -> Reply {
    reply(tool, move |_| value)
}
fn select(action: &str) -> Reply {
    args_for("select_action", json!({"action": action}))
}
fn args(value: Value) -> Reply {
    args_for("action_arguments", value)
}
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<TurnRequest>>,
}
impl Recorded {
    fn new(script: Vec<Reply>) -> Self {
        let id = |text: &str| Id::new(text).unwrap();
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("recorded-model"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8192)
            },
            script: Mutex::new(script.into()),
            seen: Mutex::default(),
        }
    }
    fn assert_consumed(&self) {
        assert!(
            self.script.lock().unwrap().is_empty(),
            "all recorded replies are used"
        );
    }
    fn requests(&self) -> Vec<TurnRequest> {
        self.seen.lock().unwrap().clone()
    }
}
impl Model for Recorded {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            request
                .validate_for(&self.provenance, &self.capabilities)
                .unwrap();
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("forced tool expected")
            };
            let reply = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unrecorded model requests");
            assert_eq!(tool.as_str(), reply.tool);
            let answer = (reply.answer)(request);
            let mut seen = self.seen.lock().unwrap();
            seen.push(request.clone());
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(format!("call-{}", seen.len())).unwrap(),
                    name: tool.clone(),
                    arguments: answer,
                })],
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}
