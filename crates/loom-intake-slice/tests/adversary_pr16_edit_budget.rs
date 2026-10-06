//! Adversary case for PR 16 (story:result-references): the design's "at most 16 MiB of expanded
//! content across an edit". `Briefing::resolve_arguments` carries that budget from file to file
//! (`remaining`); no test in the package calls it with more than one file, so a mutant that never
//! decrements `remaining` (16 MiB per file, unbounded per edit) passes the suite. This case fails
//! on that mutant. No model, network or credential is used.

use std::collections::VecDeque;
use std::sync::Mutex;

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    ExecutorOutcomeProposedAction, ProposedActionArguments,
};
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::{ArgumentContext, ArgumentGenerator};
use b10x_loom_intake_slice::executor::{InspectedFile, Report};
use b10x_loom_intake_slice::selector::{Briefing, ModelArguments};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::{Value, json};

const MIB16: usize = 16 * 1024 * 1024;

#[test]
fn two_files_composed_from_one_full_size_capture_exceed_the_per_edit_budget() {
    let briefing = Briefing::new("copy the file", vec![]);
    briefing.record(
        &ExecutorOutcomeProposedAction {
            action: "repository.inspect".into(),
            arguments: ProposedActionArguments(
                cjson::parse(&json!({"paths": ["big.txt"]}).to_string()).unwrap(),
            ),
        },
        &Report::Inspected(vec![InspectedFile {
            path: "big.txt".into(),
            contents: "x".repeat(MIB16),
        }]),
    );
    let whole = whole_reference(&briefing);
    let file = |path: &str| json!({"path": path, "contents": {"segments": [{"ref": whole}]}});

    let one = briefing
        .resolve_arguments(
            "repository.edit",
            json!({"files": [file("a.txt")], "message": "copy"}),
        )
        .expect("one 16 MiB expansion is within the per-edit budget");
    assert_eq!(one["files"][0]["contents"].as_str().unwrap().len(), MIB16);

    let two = briefing.resolve_arguments(
        "repository.edit",
        json!({"files": [file("a.txt"), file("b.txt")], "message": "copy"}),
    );
    assert!(
        two.is_err(),
        "32 MiB of expanded content across one edit exceeds its 16 MiB budget"
    );
}

/// The whole-result reference the briefing advertises to the model for the first capture.
fn whole_reference(briefing: &Briefing) -> Value {
    let model = Scripted::new(json!({"paths": ["big.txt"]}));
    ModelArguments::new(&model, briefing.clone())
        .generate(
            &ArgumentContext {
                prompt: "copy the file".into(),
            },
            &CatalogueEntry {
                action: "repository.inspect".into(),
                status: CatalogueEntryStatus::Admissible,
            },
        )
        .unwrap();
    let seen = model.seen.lock().unwrap();
    let text: String = seen[0]
        .items
        .iter()
        .filter_map(|item| match item {
            Item::UserText { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(&line[line.find('{')?..]).ok())
        .find(|parsed| parsed.get("reference").is_some())
        .expect("the capture is advertised")["reference"]
        .clone()
}

struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Value>>,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Scripted {
    fn new(answer: Value) -> Self {
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
            script: Mutex::new(VecDeque::from([answer])),
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
            let answer = self.script.lock().unwrap().pop_front().expect("one turn");
            self.seen.lock().unwrap().push(request.clone());
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
