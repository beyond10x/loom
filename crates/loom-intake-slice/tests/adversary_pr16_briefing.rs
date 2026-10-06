//! Adversary cases for PR 16 (story:result-references): what the briefing tells the model about a
//! performed `tests.run` once its output became a stored result. Before the PR the transcript
//! entry quoted the executor's report (`Report`'s `Display`: "the test command exited with <n> on a
//! work tree with uncommitted changes", "confinement: <backend>", the output); `selector.rs`'s
//! module doc still says "An entry quotes what the executor reported", and the PR's AGENTS.md
//! section says "Keep original statuses ... explicit". No model, network or credential is used.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    ExecutorOutcomeProposedAction, ProposedActionArguments,
};
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::{ArgumentContext, ArgumentGenerator};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::executor::{LocalExecutor, Report, TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::selector::{Briefing, ModelArguments};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::{Value, json};

const INTENT: &str = "Change the status to fixed and verify it.";

/// The executor reported which confinement the test command ran under; the model is no longer
/// told it.
#[test]
fn a_tests_run_entry_still_tells_the_model_the_confinement_the_executor_reported() {
    let (report, prompt) = briefed_test_run(false);
    assert!(
        report.to_string().contains("confinement: none"),
        "the executor's own report names the applied confinement: {report}"
    );
    assert!(
        prompt.contains("confinement"),
        "the briefing entry for tests.run drops the applied confinement the report carried:\n{prompt}"
    );
}

/// On a dirty work tree the executor's report says so in words, which is what tells a model why
/// no evidence came of the run (the 2026-10-04 live run lost every tests.run to this). The new
/// entry renders only `implementation=None`.
#[test]
fn a_tests_run_on_a_dirty_tree_still_says_the_tree_had_uncommitted_changes() {
    let (report, prompt) = briefed_test_run(true);
    assert!(
        report
            .to_string()
            .contains("on a work tree with uncommitted changes"),
        "the executor's own report states the dirty tree: {report}"
    );
    assert!(
        prompt.contains("uncommitted"),
        "the briefing entry for tests.run no longer states that the tree was dirty:\n{prompt}"
    );
}

/// Runs `tests.run` through the real executor (unconfined, as fixtures opt in), records it in a
/// briefing, and returns the report and the user text the next model turn is shown.
fn briefed_test_run(dirty: bool) -> (Report, String) {
    let workspace = Workspace::new("status=broken\n");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, "software-change@1", INTENT, workspace.path()).unwrap();
    if dirty {
        std::fs::write(workspace.path().join("check.txt"), "status=fixed\n").unwrap();
    }
    let executor = LocalExecutor::new(
        &governor,
        case,
        workspace.path(),
        TestCommand::new("grep", ["-q", "status=fixed", "check.txt"]),
    )
    .with_runner(Arc::new(UnconfinedRunner));
    let proposed = proposal("tests.run", json!({}));
    let report = executor.execute(&proposed).expect("tests.run is performed");
    let briefing = Briefing::new(INTENT, vec![]);
    briefing.record(&proposed, &report);

    let model = Scripted::new(vec![json!({"paths": ["check.txt"]})]);
    ModelArguments::new(&model, briefing)
        .generate(
            &ArgumentContext {
                prompt: INTENT.into(),
            },
            &CatalogueEntry {
                action: "repository.inspect".into(),
                status: CatalogueEntryStatus::Admissible,
            },
        )
        .unwrap();
    let prompt = user_text(&model.seen.lock().unwrap()[0]);
    (report, prompt)
}

fn proposal(action: &str, arguments: Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.into(),
        arguments: ProposedActionArguments(cjson::parse(&arguments.to_string()).unwrap()),
    }
}

fn user_text(request: &TurnRequest) -> String {
    request
        .items
        .iter()
        .filter_map(|item| match item {
            Item::UserText { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Value>>,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Scripted {
    fn new(script: Vec<Value>) -> Self {
        let id = |text: &str| Id::new(text).unwrap();
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("scripted"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("scripted-model"),
                binding_revision: id("scripted-1"),
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
}

impl Model for Scripted {
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
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("expected a forced named tool")
            };
            let answer = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unscripted model turns");
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

struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(contents: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("adversary-pr16-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let fixture = Self { root };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.root.join("check.txt"), contents).unwrap();
        fixture.git(&["add", "--", "check.txt"]);
        fixture.git(&["commit", "--quiet", "--message", "fixture"]);
        fixture
    }
    fn path(&self) -> &Path {
        &self.root
    }
    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
