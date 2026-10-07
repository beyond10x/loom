//! Recorded software workflows through the general catalog-aware entrypoint used by the CLI.
//! Tests execute a real local check; no model, credential, network, or confinement dependency.
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use b10x_loom_commission::model::{
    json as cjson,
    responsibility::{ActionStatus, CaseId},
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_slice::{
    clock::Clock,
    confinement::{Backend, ConfinementError, TestExecution, TestRunner},
    context::ContextPolicy,
    executor::{TestCommand, UnconfinedRunner},
    intent::{IntentRequest, run_intent_with_options},
    run::{RunOptions, SliceError, StopReason},
};
use chrono::{DateTime, FixedOffset};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

const INTENT: &str = "Fix the check in check.txt and verify the change.";
struct NoClock;
impl Clock for NoClock {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        panic!("software changes must not read the query clock")
    }
}
#[derive(Default)]
struct Runner {
    runs: AtomicUsize,
    inspected: AtomicUsize,
}
impl TestRunner for Runner {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        UnconfinedRunner.run(command, root)
    }
    fn backend(&self) -> Backend {
        self.inspected.fetch_add(1, Ordering::SeqCst);
        UnconfinedRunner.backend()
    }
}
fn request(workspace: Option<PathBuf>, runner: Arc<Runner>) -> IntentRequest {
    IntentRequest {
        intent: INTENT.into(),
        workspace,
        runner: Some(runner),
        test: TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        max_steps: 8,
        threshold: 0.5,
    }
}
fn classifier() -> Recorded {
    Recorded::new(vec![(
        "pick_protocol",
        json!({
            "protocol":"software-change@1", "confidence":0.99, "reasons":["change code"]
        }),
    )])
}
fn action(name: &str, arguments: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("select_action", json!({"action":name})),
        ("action_arguments", arguments),
    ]
}
fn edit(contents: &str) -> Value {
    json!({"files":[{"path":"check.txt","contents":contents}],"message":"update the check"})
}

#[test]
fn software_intent_edits_tests_current_revision_and_stops_for_merge_approval_in_both_policies() {
    for policy in [ContextPolicy::Legacy, ContextPolicy::Bounded] {
        let fixture = Fixture::new("broken\n");
        let before = fixture.head();
        let catalog = ProtocolCatalog::bundled().unwrap();
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .unwrap();
        let router = classifier();
        let agent = Recorded::new(
            [
                action("tests.run", json!({})),
                action("repository.edit", edit("fixed\n")),
                action("tests.run", json!({})),
                action("repository.merge", json!({})),
            ]
            .concat(),
        );
        let runner = Arc::new(Runner::default());
        let report = fixture.directory.path().join("metrics.json");
        let mut output = Vec::new();
        let run = run_intent_with_options(
            &request(Some(fixture.workspace.clone()), Arc::clone(&runner)),
            &catalog,
            &governor,
            &governor,
            &router,
            &agent,
            &NoClock,
            &mut output,
            &RunOptions {
                context_policy: policy,
                context_report: Some(report.clone()),
            },
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(run.stop_reason, StopReason::ApprovalRequired, "{output}");
        assert_eq!(run.steps, 3);
        assert_eq!(run.protocol, "software-change@1");
        assert!(
            output.ends_with("stopped: ApprovalRequired (repository.merge)\n"),
            "{output}"
        );
        assert_eq!(output.matches("evidence: test_result fail").count(), 1);
        assert_eq!(output.matches("evidence: test_result pass").count(), 1);
        assert_eq!(runner.runs.load(Ordering::SeqCst), 2);
        assert_eq!(router.calls(), 1);
        assert_eq!(agent.calls(), 8);
        router.assert_consumed();
        agent.assert_consumed();

        let after = fixture.head();
        assert_ne!(before, after);
        assert_eq!(fixture.git(&["rev-parse", "HEAD^"]).trim(), before);
        assert_eq!(fixture.git(&["show", "HEAD:check.txt"]), "fixed\n");
        assert_eq!(fixture.git(&["status", "--porcelain"]), "");
        assert_eq!(fixture.git(&["branch", "--list"]).trim(), "* main");
        let case = CaseId("case-1".into());
        assert_eq!(governor.revisions(&case).unwrap()["implementation"], after);
        let evidence = governor.evidence(&case).unwrap();
        assert_eq!(evidence.len(), 2);
        for (record, revision, result) in [
            (&evidence[0], &before, "fail"),
            (&evidence[1], &after, "pass"),
        ] {
            assert_eq!(record.kind, "test_result");
            assert_eq!(
                record.facts.member("subject_revision"),
                Some(&cjson::Value::Text(revision.clone()))
            );
            assert_eq!(
                record.facts.member("result"),
                Some(&cjson::Value::Text(result.into()))
            );
            assert_eq!(record.observation_ids.len(), 1);
        }
        let metrics: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
        assert_eq!(metrics["model_calls"], 9);
        assert_eq!(metrics["requests"][0]["phase"], "classification");
        assert!(!metrics.to_string().contains(INTENT));
        if policy == ContextPolicy::Bounded {
            for metric in metrics["requests"].as_array().unwrap() {
                assert!(metric["request_bytes"].as_u64().unwrap() <= 65536);
            }
        }
    }
}

#[test]
fn software_intent_refuses_missing_workspaces_before_touching_runner_or_agent() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    for policy in [ContextPolicy::Legacy, ContextPolicy::Bounded] {
        for workspace in [None, Some(missing.clone())] {
            let catalog = ProtocolCatalog::bundled().unwrap();
            let governor = CanonGovernor::new(MemoryCaseStore::default())
                .with_catalog(&catalog)
                .unwrap();
            let router = classifier();
            let agent = Recorded::new(vec![]);
            let runner = Arc::new(Runner::default());
            let mut out = Vec::new();
            let error = run_intent_with_options(
                &request(workspace, Arc::clone(&runner)),
                &catalog,
                &governor,
                &governor,
                &router,
                &agent,
                &NoClock,
                &mut out,
                &RunOptions {
                    context_policy: policy,
                    context_report: None,
                },
            )
            .unwrap_err();
            assert!(matches!(error, SliceError::Case(_)), "{error}");
            assert_eq!(router.calls(), 1);
            assert_eq!(agent.calls(), 0);
            assert_eq!(runner.runs.load(Ordering::SeqCst), 0);
            assert_eq!(runner.inspected.load(Ordering::SeqCst), 0);
            assert!(!String::from_utf8(out).unwrap().contains("confinement:"));
            assert!(governor.observations().is_empty());
        }
    }
    assert!(!missing.exists());
}

#[test]
fn passing_software_test_becomes_stale_after_an_edit_through_the_general_entrypoint() {
    let fixture = Fixture::new("fixed\n");
    let before = fixture.head();
    let catalog = ProtocolCatalog::bundled().unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let runner = Arc::new(Runner::default());
    let mut request = request(Some(fixture.workspace.clone()), runner);
    request.max_steps = 2;
    let agent = Recorded::new(
        [
            action("tests.run", json!({})),
            action("repository.edit", edit("broken\n")),
        ]
        .concat(),
    );
    let run = run_intent_with_options(
        &request,
        &catalog,
        &governor,
        &governor,
        &classifier(),
        &agent,
        &NoClock,
        &mut Vec::new(),
        &RunOptions {
            context_policy: ContextPolicy::Bounded,
            context_report: None,
        },
    )
    .unwrap();
    assert_eq!(run.stop_reason, StopReason::StepBudget);
    assert_ne!(fixture.head(), before);
    let case = CaseId("case-1".into());
    let frontier = governor.frontier(&case).unwrap().into_data();
    assert_eq!(
        frontier
            .actions
            .iter()
            .find(|a| a.action == "repository.merge")
            .unwrap()
            .status,
        ActionStatus::Blocked
    );
    let evidence = governor.evidence(&case).unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(
        evidence[0].facts.member("subject_revision"),
        Some(&cjson::Value::Text(before))
    );
    agent.assert_consumed();
}

struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(&'static str, Value)>>,
    calls: AtomicUsize,
}
impl Recorded {
    fn new(script: Vec<(&'static str, Value)>) -> Self {
        let id = |text: &str| Id::new(text).unwrap();
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("recorded"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8192)
            },
            script: Mutex::new(script.into()),
            calls: AtomicUsize::new(0),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn assert_consumed(&self) {
        assert!(self.script.lock().unwrap().is_empty());
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
        _: &'a mut dyn StreamSink,
        _: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            request
                .validate_for(&self.provenance, &self.capabilities)
                .unwrap();
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("expected forced tool")
            };
            let (expected, arguments) = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected model request");
            assert_eq!(tool.as_str(), expected);
            let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new(format!("call-{count}")).unwrap(),
                    name: tool.clone(),
                    arguments,
                })],
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}
struct Fixture {
    directory: tempfile::TempDir,
    workspace: PathBuf,
}
impl Fixture {
    fn new(contents: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let workspace = directory.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let fixture = Self {
            directory,
            workspace,
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), contents).unwrap();
        fixture.git(&["add", "check.txt"]);
        fixture.git(&["commit", "--quiet", "-m", "initial check"]);
        fixture
    }
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }
    fn git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        for variable in b10x_loom_intake_slice::case::REDIRECTING_GIT_VARIABLES {
            command.env_remove(variable);
        }
        let output = command
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
