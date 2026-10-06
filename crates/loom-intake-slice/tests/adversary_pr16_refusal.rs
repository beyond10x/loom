//! Adversary case for PR 16 (story:result-references): how a run ends when the model's edit
//! composition names a reference that does not resolve.
//!
//! The story's outcome: "Unknown, corrupt, invalid, cross-run and over-budget selections refuse
//! before effects." `run.rs` documents refused steps ("printed as refused, and recorded in the
//! briefing ... so the next selection is told") and reserves the `Suspended` end for "Loom
//! suspended because the agent model could not be reached". The PR's
//! `forged_digest_never_reaches_a_workspace_write` asserts only `result.is_err()`. This case
//! records what that error is. No model, network or credential is used.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::run::{SliceError, SliceRequest, run};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::{Value, json};

const INTENT: &str = "Change the status to fixed and verify it.";

/// A model that is reachable and answers every turn writes an edit composition whose reference
/// names no retained result (the same class as an off-by-one range or a mistyped digest). No write
/// happens, which the PR asserts. The run then ends as an external-availability suspension: the
/// agent model is reported as unavailable, the CLI exits 1 rather than with a stop reason, and the
/// model is never told why its arguments were not used.
#[test]
fn an_unresolvable_reference_is_not_reported_as_an_unreachable_model() {
    let workspace = Workspace::new("status=broken\n");
    let classifier = Scripted::new(vec![(
        "pick_protocol",
        json!({"protocol": "software-change@1", "confidence": 0.99, "reasons": ["fixture"]}),
    )]);
    let reference = json!({"result": "result-not-in-this-run", "sha256": "0".repeat(64),
        "select": {"kind": "whole"}, "rendering": "text"});
    let agent = Scripted::new(vec![
        ("select_action", json!({"action": "repository.edit"})),
        (
            "action_arguments",
            json!({"files": [{"path": "check.txt", "contents": {"segments": [{"ref": reference}]}}],
                "message": "fix fixture"}),
        ),
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut output = Vec::new();
    let result = run(
        &SliceRequest {
            runner: Arc::new(UnconfinedRunner),
            intent: INTENT.into(),
            workspace: workspace.path().to_path_buf(),
            test: TestCommand::new("grep", ["-q", "status=fixed", "check.txt"]),
            max_steps: 2,
            threshold: 0.5,
        },
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut output,
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("check.txt")).unwrap(),
        "status=broken\n",
        "no write (the PR's own claim, which holds)"
    );
    let ended = format!("{result:?}");
    assert!(
        !matches!(&result, Err(SliceError::Suspended(why)) if why.contains("ExternalAvailability")),
        "a reachable model's unresolvable reference ends the run as an unreachable-model outage, \
         not as a refusal the model is told about: {ended}\noutput:\n{}",
        String::from_utf8_lossy(&output)
    );
}

struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(&'static str, Value)>>,
}

impl Scripted {
    fn new(script: Vec<(&'static str, Value)>) -> Self {
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
            let (expected, answer) = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unscripted model turns");
            assert_eq!(tool.as_str(), expected);
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new("call-1").unwrap(),
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
            .join(format!("adversary-pr16-run-{}-{nanos}", std::process::id()));
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
