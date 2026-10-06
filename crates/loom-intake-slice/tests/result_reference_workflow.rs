//! Recorded-model acceptance tests for result references on the real Commission/local-executor
//! path. No model, network, credential, or operator workspace is used. Git identities below
//! belong only to disposable fixture repositories.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, ExecutorOutcomeProposedAction, Frontier,
    GovernorError, ProposedActionArguments, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::{ArgumentContext, ArgumentGenerator};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::executor::{
    InspectedFile, LocalExecutor, Report, TestCommand, UnconfinedRunner,
};
use b10x_loom_intake_slice::run::{SliceRequest, StopReason, run};
use b10x_loom_intake_slice::selector::{Briefing, ModelArguments, TRANSCRIPT_LIMIT};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::{Value, json};

const INTENT: &str = "Change the status to fixed, preserving every other byte, and verify it.";
const OLD_LINE: &str = "status=broken\r\n";
const NEW_LINE: &str = "status=fixed\r\n";
const HIDDEN: &str = "UNSEEN_TAIL_SENTINEL";

#[test]
fn inspected_file_is_composed_and_written_without_model_repeating_the_payload() {
    let payload = Payload::new();
    let workspace = Workspace::new(&payload.original);
    let before = workspace.head();
    let classifier = classifier();
    let agent = Recorded::new(vec![
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
        selection("repository.edit"),
        Reply::Lookup {
            start: payload.start,
            end: payload.end,
        },
        Reply::Compose {
            start: payload.start,
            end: payload.end,
            total: payload.original.len(),
            forge: false,
            require_lookup: true,
            gate: None,
        },
        selection("tests.run"),
        arguments(json!({})),
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut output = Vec::new();
    let result = run(
        &request(&workspace, 3),
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut output,
    )
    .expect("the actual governed software-change path accepts resolved ordinary arguments");

    assert_eq!(result.steps, 3);
    assert_eq!(result.stop_reason, StopReason::StepBudget);
    assert_eq!(workspace.read().as_bytes(), payload.expected.as_bytes());
    assert_ne!(
        workspace.head(),
        before,
        "the existing local executor commits the edit"
    );
    assert_eq!(workspace.git(&["status", "--porcelain"]), "");
    assert!(
        String::from_utf8(output).unwrap().contains("evidence:"),
        "tests.run reaches the verifier"
    );
    agent.assert_consumed();
    let seen = agent.seen.lock().unwrap();
    assert_eq!(
        seen.len(),
        7,
        "one deterministic lookup adds exactly one model turn"
    );
    for (request, _) in seen.iter() {
        let wire = serde_json::to_string(request).unwrap();
        assert!(
            wire.len() < 24 * 1024,
            "bounded descriptor, lookup and edit history: {} bytes",
            wire.len()
        );
        assert!(
            !wire.contains(HIDDEN),
            "unselected tail never enters model context"
        );
        let user = user_text(request);
        assert!(!user.contains(&payload.original));
        assert!(!user.contains(&payload.expected));
        assert!(
            !user.contains(&payload.original[..4096]),
            "neither inspection nor edit history repeats bulk file content"
        );
    }
    let composed = &seen[4].1;
    assert!(
        composed.to_string().len() * 50 < payload.expected.len(),
        "the generated composition is much smaller than its expansion"
    );
    println!(
        "result-reference acceptance: file_bytes={} composition_bytes={} max_request_bytes={} recorded_agent_turns={}",
        payload.original.len(),
        composed.to_string().len(),
        seen.iter()
            .map(|(request, _)| serde_json::to_vec(request).unwrap().len())
            .max()
            .unwrap(),
        seen.len()
    );
    let last_prompt = user_text(&seen[6].0);
    assert!(
        last_prompt.contains("repository.edit"),
        "the edited action remains in history"
    );
    assert!(
        last_prompt.contains("sha256"),
        "history identifies content without repeating it"
    );
}

#[test]
fn forged_digest_is_a_refused_step_the_model_is_told_about() {
    let payload = Payload::new();
    let workspace = Workspace::new(&payload.original);
    let before = workspace.head();
    let classifier = classifier();
    let agent = Recorded::new(vec![
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
        selection("repository.edit"),
        Reply::Compose {
            start: payload.start,
            end: payload.end,
            total: payload.original.len(),
            forge: true,
            require_lookup: false,
            gate: None,
        },
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut output = Vec::new();
    let result = run(
        &request(&workspace, 3),
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut output,
    )
    .expect("a reference that does not resolve refuses the step, and the run goes on");
    assert_eq!(result.stop_reason, StopReason::StepBudget);
    assert_eq!(
        result.steps, 3,
        "the refused step counts against the budget"
    );
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains(
            "step 2: repository.edit (not proposed)\n  effect: refused: the edit contents do not \
             resolve: result SHA-256 does not match retained observation\n  evidence: none\n"
        ),
        "the refused step is printed with its reason:\n{output}"
    );
    assert_eq!(workspace.read(), payload.original);
    assert_eq!(workspace.head(), before);
    assert_eq!(workspace.git(&["status", "--porcelain"]), "");
    agent.assert_consumed();
    let seen = agent.seen.lock().unwrap();
    assert!(
        user_text(&seen[4].0).contains(
            "repository.edit\nrefused: the edit contents do not resolve: result SHA-256 does \
             not match retained observation"
        ),
        "the next selection is told why the edit was not made:\n{}",
        user_text(&seen[4].0)
    );
}

#[test]
fn a_lookup_after_the_eighth_refuses_the_step_and_the_model_is_told() {
    let workspace = Workspace::new("status=broken\n");
    let before = workspace.head();
    let classifier = classifier();
    let mut script = vec![
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
        selection("repository.edit"),
    ];
    script.extend((0..9).map(|_| arguments(json!({"$list_results": 0}))));
    script.extend([
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
    ]);
    let agent = Recorded::new(script);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut output = Vec::new();
    let result = run(
        &request(&workspace, 3),
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut output,
    )
    .expect("a ninth lookup refuses the step, and the run goes on");
    assert_eq!(result.stop_reason, StopReason::StepBudget);
    assert_eq!(result.steps, 3);
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("step 2: repository.edit (not proposed)\n  effect: refused: "),
        "{output}"
    );
    assert_eq!(workspace.read(), "status=broken\n");
    assert_eq!(workspace.head(), before);
    agent.assert_consumed();
    let seen = agent.seen.lock().unwrap();
    let told = user_text(&seen[12].0);
    assert!(
        told.contains("repository.edit\nrefused: ") && told.contains("after the 8 allowed"),
        "the next selection is told why the edit was not made:\n{told}"
    );
}

#[test]
fn a_lookup_mixed_with_arguments_is_answered_as_a_lookup_error() {
    let briefing = inspected_briefing("status=broken\n");
    let model = Recorded::new(vec![
        arguments(json!({"$list_results": 0, "paths": ["check.txt"]})),
        arguments(json!({"paths": ["check.txt"]})),
    ]);
    let generated = generate(&model, &briefing, "repository.inspect");
    assert!(
        generated.is_ok(),
        "a malformed lookup is answered, not an aborted generation: {generated:?}"
    );
    model.assert_consumed();
    let seen = model.seen.lock().unwrap();
    let answered = user_text(&seen[1].0);
    assert!(
        answered.contains("lookup_failed")
            && answered.contains("a result lookup must contain only $read_result or $list_results"),
        "{answered}"
    );
}

#[test]
fn escaped_lookup_data_past_the_total_is_answered_as_a_lookup_error() {
    // A quote escapes to two bytes, so four 8192-byte selections fill the 65536-byte total.
    const LOOKUP: usize = 8192;
    let briefing = inspected_briefing(&"\"".repeat(6 * LOOKUP));
    let mut script: Vec<Reply> = (0..5)
        .map(|i| Reply::Lookup {
            start: i * LOOKUP,
            end: (i + 1) * LOOKUP,
        })
        .collect();
    script.push(arguments(json!({"paths": ["check.txt"]})));
    let model = Recorded::new(script);
    let generated = generate(&model, &briefing, "repository.inspect");
    assert!(
        generated.is_ok(),
        "a lookup past the total is answered, not an aborted generation: {generated:?}"
    );
    model.assert_consumed();
    let seen = model.seen.lock().unwrap();
    assert!(
        seen[0].0.instructions.contains("65536"),
        "the model is told the total: {}",
        seen[0].0.instructions
    );
    let last = user_text(&seen[5].0);
    assert_eq!(last.matches("selected_text").count(), 4, "{last}");
    assert_eq!(last.matches("lookup_failed").count(), 1);
    assert!(last.contains("65536"), "the error names the total");
}

#[test]
fn resolving_a_reference_does_not_bypass_the_current_admission_frontier() {
    let payload = Payload::new();
    let workspace = Workspace::new(&payload.original);
    let before = workspace.head();
    let gate = Arc::new(AtomicBool::new(false));
    let classifier = classifier();
    let agent = Recorded::new(vec![
        selection("repository.inspect"),
        arguments(json!({"paths": ["check.txt"]})),
        selection("repository.edit"),
        Reply::Compose {
            start: payload.start,
            end: payload.end,
            total: payload.original.len(),
            forge: false,
            require_lookup: false,
            gate: Some(gate.clone()),
        },
        // Revalidation refuses the first proposal against the changed frontier. The runtime
        // reloads it; proposing again now reports the explicit operator approval requirement.
        selection("repository.edit"),
        Reply::Compose {
            start: payload.start,
            end: payload.end,
            total: payload.original.len(),
            forge: false,
            require_lookup: false,
            gate: None,
        },
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let frontiers = ApprovalAfterGeneration {
        governor: &governor,
        gate: gate.clone(),
    };
    let result = run(
        &request(&workspace, 3),
        &governor,
        &frontiers,
        &classifier,
        &agent,
        &mut Vec::new(),
    )
    .expect("the runtime reports the new approval requirement");
    assert_eq!(result.stop_reason, StopReason::ApprovalRequired);
    assert!(
        gate.load(Ordering::SeqCst),
        "composition was generated while the action was admissible"
    );
    agent.assert_consumed();
    let seen = agent.seen.lock().unwrap();
    assert!(user_text(&seen[2].0).contains("repository.edit (admissible)"));
    assert!(user_text(&seen[4].0).contains("repository.edit (needs approval)"));
    assert_eq!(
        workspace.read(),
        payload.original,
        "Commission revalidates before LocalExecutor writes"
    );
    assert_eq!(workspace.head(), before);
    assert_eq!(workspace.git(&["status", "--porcelain"]), "");
}

#[test]
fn ordinary_literal_arguments_still_use_the_existing_executor() {
    let workspace = Workspace::new("status=broken\n");
    let classifier = classifier();
    let agent = Recorded::new(vec![
        selection("repository.edit"),
        arguments(
            json!({"files": [{"path": "check.txt", "contents": "status=fixed\n"}], "message": "fix fixture"}),
        ),
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let result = run(
        &request(&workspace, 1),
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(result.stop_reason, StopReason::StepBudget);
    assert_eq!(workspace.read(), "status=fixed\n");
    assert_eq!(workspace.git(&["status", "--porcelain"]), "");
    agent.assert_consumed();
}

#[test]
fn result_access_is_run_local_and_survives_rolling_transcript_history() {
    let payload = Payload::new();
    let briefing = inspected_briefing(&payload.original);
    let descriptor = descriptors(&snapshot(&briefing)).remove(0);
    let reference = byte_reference(&descriptor, payload.start, payload.end);
    assert_eq!(
        descriptor["utf8_bytes"].as_u64(),
        Some(payload.original.len() as u64)
    );
    assert!(descriptor["preview"].as_str().unwrap().len() <= 1024);
    assert_eq!(descriptor["preview_truncated"], true);
    assert!(
        briefing.read_result(&descriptor["reference"]).is_err(),
        "a whole-file lookup cannot exceed the model-facing read budget"
    );
    assert_eq!(briefing.read_result(&reference).unwrap(), OLD_LINE);
    assert_eq!(briefing.clone().read_result(&reference).unwrap(), OLD_LINE);

    // Even an otherwise identical second run must not accidentally resolve a counter-based id.
    let other = inspected_briefing(&payload.original);
    assert!(
        other.read_result(&reference).is_err(),
        "references cannot cross run scope"
    );
    for _ in 0..=TRANSCRIPT_LIMIT {
        briefing.record_refusal("repository.inspect", "fixture refusal");
    }
    assert!(
        descriptors(&snapshot(&briefing)).is_empty(),
        "the capture descriptor aged out of the rolling transcript"
    );
    assert_eq!(
        briefing.read_result(&reference).unwrap(),
        OLD_LINE,
        "artifact lifetime is not transcript lifetime"
    );
}

#[test]
fn command_output_reference_describes_only_the_captured_tail() {
    let source = format!(
        "HEAD_NOT_CAPTURED\n{}TAIL_CAPTURED\n",
        "x".repeat(24 * 1024)
    );
    let workspace = Workspace::new(&source);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, "software-change@1", INTENT, workspace.path()).unwrap();
    let executor = LocalExecutor::new(
        &governor,
        case,
        workspace.path(),
        TestCommand::new("cat", ["check.txt"]),
    )
    .with_runner(Arc::new(UnconfinedRunner));
    let proposed = proposal("tests.run", json!({}));
    let report = executor.execute(&proposed).unwrap();
    let Report::TestsRun(test_run) = &report else {
        panic!("tests.run reports the actual process result");
    };
    let retained = test_run.output_tail();
    assert!(!retained.contains("HEAD_NOT_CAPTURED"));
    assert!(retained.ends_with("TAIL_CAPTURED\n"));
    let briefing = Briefing::new(INTENT, vec![]);
    briefing.record(&proposed, &report);
    let captures = descriptors(&snapshot(&briefing));
    let descriptor = captures
        .iter()
        .find(|descriptor| descriptor["origin"] == "tests.run/output-tail")
        .expect("the test output is a referenceable result");
    // The runner retains 8 KiB per stream and adds stdout/stderr framing. That captured
    // artifact can exceed a single 8 KiB lookup; request only its final 4 KiB here.
    let start = retained.len().saturating_sub(4096);
    let reference = byte_reference(descriptor, start, retained.len());
    let captured = briefing.read_result(&reference).unwrap();
    assert_eq!(captured, retained[start..]);
    assert!(captured.contains("TAIL_CAPTURED"));
    assert!(!captured.contains("HEAD_NOT_CAPTURED"));
    assert!(
        descriptor["capture"]
            .to_string()
            .to_ascii_lowercase()
            .contains("partial"),
        "partial capture is explicit: {descriptor}"
    );
    assert_eq!(
        descriptor["utf8_bytes"].as_u64(),
        Some(retained.len() as u64)
    );
}

struct Payload {
    original: String,
    expected: String,
    start: usize,
    end: usize,
}
impl Payload {
    fn new() -> Self {
        let prefix = "untouched αβγ before\r\n".repeat(3000);
        let suffix = format!(
            "{}\r\n{HIDDEN}\r\n",
            "untouched 日本語 after\r\n".repeat(2200)
        );
        let start = prefix.len();
        let end = start + OLD_LINE.len();
        Self {
            original: format!("{prefix}{OLD_LINE}{suffix}"),
            expected: format!("{prefix}{NEW_LINE}{suffix}"),
            start,
            end,
        }
    }
}

fn inspected_briefing(contents: &str) -> Briefing {
    let briefing = Briefing::new(INTENT, vec![]);
    briefing.record(
        &proposal("repository.inspect", json!({"paths": ["check.txt"]})),
        &Report::Inspected(vec![InspectedFile {
            path: "check.txt".into(),
            contents: contents.into(),
        }]),
    );
    briefing
}

fn generate(model: &Recorded, briefing: &Briefing, action: &str) -> Result<cjson::Value, String> {
    ModelArguments::new(model, briefing.clone()).generate(
        &ArgumentContext {
            prompt: "inspect the fixture".into(),
        },
        &CatalogueEntry {
            action: action.into(),
            status: CatalogueEntryStatus::Admissible,
        },
    )
}

fn snapshot(briefing: &Briefing) -> TurnRequest {
    let model = Recorded::new(vec![arguments(json!({"paths": ["check.txt"]}))]);
    ModelArguments::new(&model, briefing.clone())
        .generate(
            &ArgumentContext {
                prompt: "inspect the fixture".into(),
            },
            &CatalogueEntry {
                action: "repository.inspect".into(),
                status: CatalogueEntryStatus::Admissible,
            },
        )
        .unwrap();
    model.seen.lock().unwrap()[0].0.clone()
}

fn proposal(action: &str, arguments: Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.into(),
        arguments: ProposedActionArguments(cjson::parse(&arguments.to_string()).unwrap()),
    }
}

fn request(workspace: &Workspace, max_steps: usize) -> SliceRequest {
    SliceRequest {
        runner: Arc::new(UnconfinedRunner),
        intent: INTENT.into(),
        workspace: workspace.path().to_path_buf(),
        test: TestCommand::new("grep", ["-q", "status=fixed", "check.txt"]),
        max_steps,
        threshold: 0.5,
    }
}

struct ApprovalAfterGeneration<'g> {
    governor: &'g CanonGovernor<MemoryCaseStore>,
    gate: Arc<AtomicBool>,
}
impl Governor for ApprovalAfterGeneration<'_> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.governor.current_revision(case)
    }
    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.governor.completion(case)
    }
    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let mut data = self.governor.frontier(case)?.into_data();
        if self.gate.load(Ordering::SeqCst) {
            for action in &mut data.actions {
                if action.action == "repository.edit" {
                    action.status = ActionStatus::ApprovalRequired;
                    action.capability = Some("fixture.repository.edit".into());
                    action.reasons = vec!["fixture now requires operator approval".into()];
                }
            }
        }
        Ok(Frontier::new(data))
    }
}

// A reply obtains the opaque id/digest from the actual model-facing descriptor. Byte offsets are
// fixture knowledge; ids are not. This models a small generated program, never a replayed file.
enum Reply {
    Json {
        tool: &'static str,
        value: Value,
    },
    Lookup {
        start: usize,
        end: usize,
    },
    Compose {
        start: usize,
        end: usize,
        total: usize,
        forge: bool,
        require_lookup: bool,
        gate: Option<Arc<AtomicBool>>,
    },
}
fn selection(action: &str) -> Reply {
    Reply::Json {
        tool: "select_action",
        value: json!({"action": action}),
    }
}
fn arguments(value: Value) -> Reply {
    Reply::Json {
        tool: "action_arguments",
        value,
    }
}
fn classifier() -> Recorded {
    Recorded::new(vec![Reply::Json {
        tool: "pick_protocol",
        value: json!({"protocol": "software-change@1", "confidence": 0.99, "reasons": ["recorded software change"]}),
    }])
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
fn descriptors(request: &TurnRequest) -> Vec<Value> {
    user_text(request)
        .lines()
        .filter_map(|line| {
            let start = line.find('{')?;
            let parsed: Value = serde_json::from_str(&line[start..]).ok()?;
            (parsed.get("result").is_some()
                && parsed.get("sha256").is_some()
                && parsed.get("reference").is_some())
            .then_some(parsed)
        })
        .collect()
}
fn byte_reference(descriptor: &Value, start: usize, end: usize) -> Value {
    let mut reference = descriptor["reference"].clone();
    assert!(
        reference.is_object(),
        "a capture advertises a complete reference envelope"
    );
    reference["select"] = json!({"kind": "bytes", "start": start, "end": end});
    reference["rendering"] = json!("text");
    reference
}

struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<(TurnRequest, Value)>>,
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
            "all recorded replies were used"
        );
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
                panic!("expected a forced named tool")
            };
            let reply = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .expect("no unrecorded model turns");
            let (expected_tool, answer) = match reply {
                Reply::Json { tool, value } => (tool, value),
                Reply::Lookup { start, end } => {
                    let descriptor = descriptors(request)
                        .into_iter()
                        .next()
                        .expect("inspection advertises a result descriptor");
                    assert_eq!(descriptor["preview_truncated"], true);
                    assert!(descriptor["preview"].as_str().unwrap().len() <= 1024);
                    (
                        "action_arguments",
                        json!({"$read_result": byte_reference(&descriptor, start, end)}),
                    )
                }
                Reply::Compose {
                    start,
                    end,
                    total,
                    forge,
                    require_lookup,
                    gate,
                } => {
                    if require_lookup {
                        let text = user_text(request);
                        assert!(
                            text.contains(OLD_LINE) || text.contains("status=broken\\r\\n"),
                            "selected bytes reach the next argument-generation turn"
                        );
                    }
                    let descriptor = descriptors(request)
                        .into_iter()
                        .next()
                        .expect("captured file remains available by reference");
                    let mut first = byte_reference(&descriptor, 0, start);
                    if forge {
                        first["sha256"] = json!("0".repeat(64));
                    }
                    let answer = json!({"files": [{"path": "check.txt", "contents": {"segments": [{"ref": first}, {"literal": NEW_LINE}, {"ref": byte_reference(&descriptor, end, total)}]}}], "message": "fix fixture using result slices"});
                    if let Some(gate) = gate {
                        gate.store(true, Ordering::SeqCst);
                    }
                    ("action_arguments", answer)
                }
            };
            assert_eq!(tool.as_str(), expected_tool);
            let mut seen = self.seen.lock().unwrap();
            seen.push((request.clone(), answer.clone()));
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
            .join(format!("result-reference-{}-{nanos}", std::process::id()));
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
    fn read(&self) -> String {
        std::fs::read_to_string(self.root.join("check.txt")).unwrap()
    }
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().into()
    }
    fn git(&self, args: &[&str]) -> String {
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
        String::from_utf8(output.stdout).unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
