//! Adversary cases for PR 16 (story:result-references): stored-result lookups during argument
//! generation, driven against what `ARGUMENTS_INSTRUCTIONS` tells the model and what
//! `docs/design/result-references.md` promises. No model, network or credential is used.

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

/// The lookup size the model is told it may request ("each at most 8192 UTF-8 bytes").
const LOOKUP: usize = 8192;
/// The lookup count the model is told it may make ("At most eight lookups").
const LOOKUPS: usize = 8;

/// The model does exactly what its instructions allow: it reads an inspected ASCII file that
/// needs no JSON escaping in eight consecutive 8192-byte ranges, then answers with the action.
/// The design says "At most eight lookups are allowed before a final action answer" and the
/// instructions say "At most eight lookups, each at most 8192 UTF-8 bytes, are allowed". The
/// 64 KiB aggregate response cap (never mentioned to the model) is reached by the eighth, and
/// that aborts argument generation instead of answering it.
#[test]
fn eight_lookups_of_the_size_the_model_is_told_leave_room_for_the_action() {
    let contents = "abcdefghijklmnop".repeat((LOOKUPS + 1) * LOOKUP / 16);
    let briefing = inspected("big.txt", &contents);
    let mut replies: Vec<Reply> = (0..LOOKUPS)
        .map(|i| -> Reply {
            Box::new(move |request: &TurnRequest| {
                let descriptor = descriptor(request);
                json!({"$read_result": byte_reference(&descriptor, i * LOOKUP, (i + 1) * LOOKUP)})
            })
        })
        .collect();
    replies.push(Box::new(|_: &TurnRequest| json!({"paths": ["big.txt"]})));
    let model = Scripted::new(replies);

    let generated = generate(&model, &briefing, "repository.inspect");

    assert!(
        generated.is_ok(),
        "eight lookups of 8192 bytes are what the model was told it may make, so the action \
         answer after them must be accepted: {generated:?}"
    );
    let seen = model.seen.lock().unwrap();
    assert_eq!(seen.len(), LOOKUPS + 1, "eight lookups and one answer");
    assert_eq!(
        user_text(&seen[LOOKUPS]).matches("selected_text").count(),
        LOOKUPS,
        "every lookup's data reached the final turn"
    );
}

/// The design: "At most eight lookups are allowed before a final action answer; errors are
/// explicit and consume the same budget. ... Catalogue pages share this lookup budget." A failed
/// `$read_result` is answered as `lookup_failed` and generation continues. A `$list_results`
/// whose offset is not a nonnegative integer instead aborts argument generation, so one malformed
/// catalogue request (the Responses projection is `strict:false`, so the schema's `minimum: 0` is
/// not enforced by the provider) ends the run.
#[test]
fn a_malformed_catalogue_offset_is_a_lookup_error_not_an_aborted_generation() {
    let briefing = inspected("small.txt", "small file\n");
    let model = Scripted::new(vec![
        Box::new(|_: &TurnRequest| json!({"$list_results": -1})),
        Box::new(|_: &TurnRequest| json!({"paths": ["small.txt"]})),
    ]);

    let generated = generate(&model, &briefing, "repository.inspect");

    assert!(
        generated.is_ok(),
        "a catalogue lookup error consumes lookup budget like a read error does: {generated:?}"
    );
    assert_eq!(model.seen.lock().unwrap().len(), 2);
}

fn inspected(path: &str, contents: &str) -> Briefing {
    let briefing = Briefing::new("read the file", vec![]);
    briefing.record(
        &ExecutorOutcomeProposedAction {
            action: "repository.inspect".into(),
            arguments: ProposedActionArguments(
                cjson::parse(&json!({"paths": [path]}).to_string()).unwrap(),
            ),
        },
        &Report::Inspected(vec![InspectedFile {
            path: path.into(),
            contents: contents.into(),
        }]),
    );
    briefing
}

fn generate(model: &Scripted, briefing: &Briefing, action: &str) -> Result<cjson::Value, String> {
    ModelArguments::new(model, briefing.clone()).generate(
        &ArgumentContext {
            prompt: "read the file".into(),
        },
        &CatalogueEntry {
            action: action.into(),
            status: CatalogueEntryStatus::Admissible,
        },
    )
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

/// The first capture descriptor the briefing shows the model.
fn descriptor(request: &TurnRequest) -> Value {
    user_text(request)
        .lines()
        .filter_map(|line| {
            let parsed: Value = serde_json::from_str(&line[line.find('{')?..]).ok()?;
            (parsed.get("result").is_some() && parsed.get("reference").is_some()).then_some(parsed)
        })
        .next()
        .expect("the inspection advertises a capture descriptor")
}

fn byte_reference(descriptor: &Value, start: usize, end: usize) -> Value {
    let mut reference = descriptor["reference"].clone();
    reference["select"] = json!({"kind": "bytes", "start": start, "end": end});
    reference
}

type Reply = Box<dyn Fn(&TurnRequest) -> Value + Send>;

struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Scripted {
    fn new(script: Vec<Reply>) -> Self {
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
                .expect("no unscripted model turns");
            let answer = reply(request);
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
