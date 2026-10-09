// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1 on `story:loop-stop-model`: the `LoopStop` wire codec
//! (`crates/loom-executor/src/harness/turn_loop/stop_codec.rs`) driven against the declaration it
//! claims to spell (`loom.run.LoopStop` in `ess/domains/run.yaml`), against the hand-written serde
//! enum it replaced (`turn_loop/mod.rs` at 6a4b165, copied below as `Before`, for comparison only),
//! and against the loop's `u64` counts that now reach the stop through `figure`.
//!
//! Every model here is a scripted [`ModelPort`] in this process: no socket.

use std::path::PathBuf;

use b10x_loom_executor::harness::turn_loop::{
    AgentLoop, ApproveAll, Budget, LoopConfig, LoopEvent, LoopOutcome, LoopSink, LoopStop,
    LoopStopAwaitingApproval, LoopStopBudgetUnobservable, LoopStopCancelled,
    LoopStopContextAboveTrigger, LoopStopDeadline, LoopStopMaxCost, LoopStopMaxInputTokens,
    LoopStopMaxOutputTokens, LoopStopMaxTurns, LoopStopProviderIncomplete, LoopStopUnstructured,
};
use b10x_loom_executor::harness::wire::{
    Approval, CallId, Envelope, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName,
    ToolOutcome, ToolPort, ToolSpec, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// --- the declaration, read from `ess/domains/run.yaml` ------------------------------------------

fn run_yaml() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ess/domains/run.yaml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The `(tag, payload type)` of every variant `loom.run.LoopStop` declares, in declaration order.
fn declared_variants(yaml: &str) -> Vec<(String, Option<String>)> {
    let lines: Vec<&str> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_end() == "  - name: loom.run.LoopStop")
        .expect("run.yaml declares loom.run.LoopStop");
    let variants = (start..lines.len())
        .find(|&i| lines[i].trim() == "variants:")
        .expect("the union lists its variants");
    let mut out = Vec::new();
    for line in &lines[variants + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent < 6 {
            break;
        }
        let (tag, payload) = trimmed.split_once(':').expect("`tag: Type` or `tag:`");
        let payload = payload.trim();
        out.push((
            tag.trim().to_owned(),
            (!payload.is_empty()).then(|| payload.to_owned()),
        ));
    }
    out
}

/// The `(field, type)` of the struct `name`, in declaration order.
fn declared_fields(yaml: &str, name: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = yaml.lines().collect();
    let header = format!("  - name: {name}");
    let start = lines
        .iter()
        .position(|l| l.trim_end() == header)
        .unwrap_or_else(|| panic!("run.yaml declares {name}"));
    let mut out = Vec::new();
    let mut field: Option<String> = None;
    for line in &lines[start + 1..] {
        if line.starts_with("  - name:") {
            break;
        }
        let trimmed = line.trim();
        if let Some(f) = trimmed.strip_prefix("- name:") {
            field = Some(f.trim().to_owned());
        } else if let Some(t) = trimmed.strip_prefix("type:")
            && let Some(f) = field.take()
        {
            out.push((f, t.trim().to_owned()));
        }
    }
    out
}

fn outcome_text(stop: &str) -> String {
    format!(r#"{{"stop":{stop},"text":"","items":[],"turns":5,"usage":[]}}"#)
}

fn finished_text(stop: &str) -> String {
    format!(r#"{{"kind":"finished","stop":{stop},"turns":3}}"#)
}

fn nested_delegate_text(stop: &str) -> String {
    format!(
        r#"{{"kind":"delegated","call_id":"call-1","event":{{"kind":"delegate-finished","call_id":"call-1","stop":{stop},"turns":4}}}}"#
    )
}

/// Every cause `ess/domains/run.yaml` declares is read and written back by the codec under the
/// tag and the field names the declaration gives, in declaration order.
///
/// The codec keeps its own string table of tags (`TAGS`, the arms of `read`, `tag`) and of field
/// names (the literals passed to `write_figure` / `read_figure`). The compiler forces `serialize`
/// and `tag` to cover a variant ESS adds, but nothing forces `read` or the field-name literals to
/// follow the declaration: `loop_stop_wire.rs` pins twelve hand-written literals and
/// `every_listed_tag_is_one_the_reader_knows` iterates the codec's own `TAGS`. This case reads
/// the declaration itself, so a declared cause the reader does not know, or a field spelled
/// otherwise on the wire than in ESS, fails here.
#[test]
fn every_cause_the_ess_union_declares_reads_and_writes_back_under_its_declared_names() {
    let failures = off_the_declaration(&run_yaml());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The check above fails on a declaration the codec does not follow: run on a planted copy of
/// `run.yaml` that declares one more cause, and one that renames a field, so it is shown it can
/// fail.
#[test]
fn the_declaration_check_fails_on_a_cause_or_a_field_the_codec_does_not_know() {
    let yaml = run_yaml();
    let added = yaml.replacen(
        "      max-turns: loom.run.LoopStopMaxTurns\n",
        "      max-turns: loom.run.LoopStopMaxTurns\n      exhausted: loom.run.LoopStopMaxTurns\n",
        1,
    );
    assert_ne!(added, yaml, "the plant took");
    let failures = off_the_declaration(&added);
    assert!(
        failures.iter().any(|f| f.starts_with("`exhausted`")),
        "{failures:?}"
    );
    let renamed = yaml.replacen("      - name: limit_ms\n", "      - name: ceiling_ms\n", 1);
    assert_ne!(renamed, yaml, "the plant took");
    let failures = off_the_declaration(&renamed);
    assert!(
        failures.iter().any(|f| f.starts_with("`deadline`")),
        "{failures:?}"
    );
}

/// Every declared cause the codec does not read, or writes back other than declared.
fn off_the_declaration(yaml: &str) -> Vec<String> {
    let variants = declared_variants(yaml);
    assert!(
        variants.len() >= 12,
        "the parse found the union's variants: {variants:?}"
    );
    let mut failures = Vec::new();
    for (tag, payload) in &variants {
        // Built by hand, so the members are in the declaration's order.
        let mut text = format!(r#"{{"kind":{}"#, Value::from(tag.as_str()));
        if let Some(payload) = payload {
            let fields = declared_fields(yaml, payload);
            assert!(!fields.is_empty(), "{payload} declares its fields");
            for (field, ty) in fields {
                let value = match ty.as_str() {
                    "Integer" => Value::from(7),
                    "String" => Value::from("s"),
                    other => {
                        panic!("{payload}.{field} has a type this case does not fill: {other}")
                    }
                };
                text.push_str(&format!(",{}:{value}", Value::from(field.as_str())));
            }
        }
        text.push('}');
        let expected = outcome_text(&text);
        match serde_json::from_str::<LoopOutcome>(&expected) {
            Err(error) => failures.push(format!("`{tag}`: {text} is not read: {error}")),
            Ok(read) => {
                let written = serde_json::to_string(&read).expect("serializes");
                if written != expected {
                    failures.push(format!("`{tag}`: wrote {written}, declared {expected}"));
                }
            }
        }
    }
    failures
}

// --- the hand-written enum at 6a4b165, for comparison only --------------------------------------

/// `LoopStop` as `crates/loom-executor/src/harness/turn_loop/mod.rs` declared it at 6a4b165,
/// copied verbatim but for the name, so the codec can be compared with what it replaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Before {
    Completed,
    MaxTurns {
        limit: u64,
    },
    MaxInputTokens {
        limit: u64,
        reported: u64,
    },
    MaxOutputTokens {
        limit: u64,
        reported: u64,
    },
    MaxCost {
        limit_micro_usd: u64,
        spent_micro_usd: u64,
    },
    BudgetUnobservable {
        name: String,
        reason: String,
    },
    Deadline {
        limit_ms: u64,
    },
    Cancelled {
        reason: String,
    },
    AwaitingApproval {
        checkpoint_id: String,
    },
    ProviderIncomplete {
        reason: String,
    },
    Unstructured {
        asked_again: u32,
    },
    ContextAboveTrigger {
        window: u64,
        target: u64,
        occupied: u64,
    },
}

/// The same stop in both models, every figure `n` (`asked_again` capped to `u32`).
fn both(n: i64) -> Vec<(LoopStop, Before)> {
    let u = n as u64;
    let small = u32::try_from(n).unwrap_or(u32::MAX);
    let text = || "a \"quoted\" reason \u{1F600}".to_owned();
    vec![
        (LoopStop::Completed, Before::Completed),
        (
            LoopStop::MaxTurns(LoopStopMaxTurns { limit: n }),
            Before::MaxTurns { limit: u },
        ),
        (
            LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
                limit: n,
                reported: n,
            }),
            Before::MaxInputTokens {
                limit: u,
                reported: u,
            },
        ),
        (
            LoopStop::MaxOutputTokens(LoopStopMaxOutputTokens {
                limit: n,
                reported: n,
            }),
            Before::MaxOutputTokens {
                limit: u,
                reported: u,
            },
        ),
        (
            LoopStop::MaxCost(LoopStopMaxCost {
                limit_micro_usd: n,
                spent_micro_usd: n,
            }),
            Before::MaxCost {
                limit_micro_usd: u,
                spent_micro_usd: u,
            },
        ),
        (
            LoopStop::BudgetUnobservable(LoopStopBudgetUnobservable {
                name: text(),
                reason: text(),
            }),
            Before::BudgetUnobservable {
                name: text(),
                reason: text(),
            },
        ),
        (
            LoopStop::Deadline(LoopStopDeadline { limit_ms: n }),
            Before::Deadline { limit_ms: u },
        ),
        (
            LoopStop::Cancelled(LoopStopCancelled { reason: text() }),
            Before::Cancelled { reason: text() },
        ),
        (
            LoopStop::AwaitingApproval(LoopStopAwaitingApproval {
                checkpoint_id: text(),
            }),
            Before::AwaitingApproval {
                checkpoint_id: text(),
            },
        ),
        (
            LoopStop::ProviderIncomplete(LoopStopProviderIncomplete { reason: text() }),
            Before::ProviderIncomplete { reason: text() },
        ),
        (
            LoopStop::Unstructured(LoopStopUnstructured {
                asked_again: i64::from(small),
            }),
            Before::Unstructured { asked_again: small },
        ),
        (
            LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
                window: n,
                target: n,
                occupied: n,
            }),
            Before::ContextAboveTrigger {
                window: u,
                target: u,
                occupied: u,
            },
        ),
    ]
}

/// At 0, 1 and `i64::MAX` the codec writes, inside a `finished` event and inside a nested
/// `delegated` / `delegate-finished` event, exactly the text the enum at 6a4b165 wrote, and that
/// text reads back to the same stop through both.
#[test]
fn the_codec_writes_what_the_enum_at_the_base_wrote_at_the_i64_boundaries() {
    for n in [0, 1, i64::MAX] {
        for (stop, before) in both(n) {
            let old = serde_json::to_string(&before).expect("the old enum writes");
            let event = LoopEvent::Finished {
                stop: stop.clone(),
                turns: 3,
            };
            assert_eq!(
                serde_json::to_string(&event).expect("serializes"),
                finished_text(&old),
                "{stop:?}"
            );
            let nested = LoopEvent::Delegated {
                call_id: CallId::new("call-1").expect("valid"),
                event: Box::new(LoopEvent::DelegateFinished {
                    call_id: CallId::new("call-1").expect("valid"),
                    stop: stop.clone(),
                    turns: 4,
                }),
            };
            let nested_text = nested_delegate_text(&old);
            assert_eq!(
                serde_json::to_string(&nested).expect("serializes"),
                nested_text,
                "{stop:?}"
            );
            assert_eq!(
                serde_json::from_str::<LoopEvent>(&nested_text).expect("reads"),
                nested,
                "{nested_text}"
            );
            assert_eq!(
                serde_json::from_value::<LoopEvent>(
                    serde_json::from_str::<Value>(&nested_text).expect("json")
                )
                .expect("reads from a Value"),
                nested,
                "{nested_text}"
            );
        }
    }
}

/// Texts the enum at 6a4b165 read, whose figures are in 0..=i64::MAX and which carry no member
/// beyond the cause's own: the codec reads every one to the same stop. Members in another order,
/// an escaped tag, whitespace.
#[test]
fn every_text_the_base_enum_read_in_range_the_codec_reads_to_the_same_stop() {
    let texts = [
        r#"{"limit":20,"kind":"max-turns"}"#,
        r#"{"occupied":3,"target":2,"kind":"context-above-trigger","window":1}"#,
        r#"{"kind":"max-turns","limit":20}"#,
        r#" { "kind" : "deadline" , "limit_ms" : 9223372036854775807 } "#,
        r#"{"reason":"r","kind":"cancelled"}"#,
        r#"{"kind":"unstructured","asked_again":4294967295}"#,
    ];
    for text in texts {
        let before: Before = serde_json::from_str(text).expect("the base enum read it");
        let read = serde_json::from_str::<LoopEvent>(&finished_text(text))
            .unwrap_or_else(|e| panic!("{text}: {e}"));
        let LoopEvent::Finished { stop, .. } = read else {
            panic!("{text}: not a finished event");
        };
        assert_eq!(
            serde_json::to_string(&LoopEvent::Finished { stop, turns: 3 }).expect("serializes"),
            finished_text(&serde_json::to_string(&before).expect("writes")),
            "{text}"
        );
    }
}

// --- the loop's counts, through `figure` --------------------------------------------------------

struct Reporting {
    wire: WireId,
    turns: usize,
    input_tokens: u64,
}

impl ModelPort for Reporting {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.turns += 1;
        Ok(TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![Item::ToolCall(ToolCall {
                call_id: CallId::new(format!("call-{}", self.turns)).expect("valid"),
                name: ToolName::new("a").expect("valid"),
                arguments: json!({}),
            })],
            usage: Some(Usage {
                model: "scripted-model".to_owned(),
                input_tokens: self.input_tokens,
                output_tokens: 5,
                cached_input_tokens: 0,
                cache_creation_input_tokens: None,
            }),
        })
    }
}

struct OneTool {
    specs: Vec<ToolSpec>,
}

impl ToolPort for OneTool {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
        ToolOutcome::ok(json!({}))
    }
}

#[derive(Default)]
struct Events(Vec<LoopEvent>);

impl LoopSink for Events {
    fn emit(&mut self, event: LoopEvent) {
        self.0.push(event);
    }
}

/// A provider that reports more input tokens than `i64::MAX` stops a run held to an input
/// ceiling, and the stop says the ceiling bound: `reported` is the saturated `i64::MAX`, never a
/// figure below the limit, never zero and never negative, and the `finished` event that carries it
/// is written.
///
/// `figure` (`turn_loop/mod.rs`) documents that it saturates so that "a figure that did would
/// still read as past any declared limit"; no case in the suite drives a count above `i64::MAX`
/// into a stop, so replacing `figure` by `as i64` (a negative, which the codec then refuses to
/// write) or by `unwrap_or(0)` (a stop reporting 0 tokens against a limit of 1000) stays green.
#[test]
fn a_reported_count_above_i64_max_saturates_in_the_stop_and_still_reads_past_the_limit() {
    let above = i64::MAX as u64 + 1;
    let mut model = Reporting {
        wire: WireId::new("scripted").expect("valid"),
        turns: 0,
        input_tokens: above,
    };
    let mut tools = OneTool {
        specs: vec![ToolSpec {
            name: ToolName::new("a").expect("valid"),
            description: "the a tool".to_owned(),
            envelope: Envelope::default(),
            input_schema: json!({"type": "object"}),
            approval: Approval::NotRequired,
        }],
    };
    let mut approvals = ApproveAll;
    let mut config = LoopConfig::new("scripted-model", "be useful");
    config.budget = Budget {
        max_input_tokens: Some(1000),
        ..Budget::default()
    };
    let mut sink = Events::default();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config)
        .run("do the thing", &mut sink)
        .expect("a bound that binds is an outcome");
    assert_eq!(
        outcome.stop,
        LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
            limit: 1000,
            reported: i64::MAX,
        })
    );
    let finished = sink
        .0
        .iter()
        .find(|event| matches!(event, LoopEvent::Finished { .. }))
        .expect("a finished event");
    assert_eq!(
        serde_json::to_value(finished).expect("the finished event is written")["stop"],
        json!({"kind": "max-input-tokens", "limit": 1000, "reported": i64::MAX})
    );
    assert_eq!(
        serde_json::to_value(&outcome).expect("the outcome is written")["usage"][0]["input_tokens"],
        json!(above),
        "the outcome's usage keeps the figure the provider reported"
    );
}
