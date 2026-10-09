//! The JSON a `LoopStop` is written as, pinned per cause.
//!
//! `LoopStop` is declared in `ess/domains/run.yaml` and generated; the JSON of the three places
//! that carry it (`LoopOutcome.stop`, `LoopEvent::Finished.stop`, `LoopEvent::DelegateFinished.stop`)
//! is a published interface the metaharness reads. Every literal below is what the hand-written
//! serde enum wrote before the model moved to ESS, so each case shows the move changed no byte:
//! the stop is written exactly so, and that text reads back to the same stop.

use b10x_loom_executor::harness::turn_loop::{
    LoopEvent, LoopOutcome, LoopStop, LoopStopAwaitingApproval, LoopStopBudgetUnobservable,
    LoopStopCancelled, LoopStopContextAboveTrigger, LoopStopDeadline, LoopStopMaxCost,
    LoopStopMaxInputTokens, LoopStopMaxOutputTokens, LoopStopMaxTurns, LoopStopProviderIncomplete,
    LoopStopUnstructured,
};
use b10x_loom_executor::harness::wire::CallId;
use serde_json::Value;

/// One stop per cause, beside the exact text it is written as.
fn causes() -> Vec<(LoopStop, &'static str)> {
    vec![
        (LoopStop::Completed, r#"{"kind":"completed"}"#),
        (
            LoopStop::MaxTurns(LoopStopMaxTurns { limit: 20 }),
            r#"{"kind":"max-turns","limit":20}"#,
        ),
        (
            LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
                limit: 1000,
                reported: 1200,
            }),
            r#"{"kind":"max-input-tokens","limit":1000,"reported":1200}"#,
        ),
        (
            LoopStop::MaxOutputTokens(LoopStopMaxOutputTokens {
                limit: 500,
                reported: 512,
            }),
            r#"{"kind":"max-output-tokens","limit":500,"reported":512}"#,
        ),
        (
            LoopStop::MaxCost(LoopStopMaxCost {
                limit_micro_usd: 250_000,
                spent_micro_usd: 250_001,
            }),
            r#"{"kind":"max-cost","limit_micro_usd":250000,"spent_micro_usd":250001}"#,
        ),
        (
            LoopStop::BudgetUnobservable(LoopStopBudgetUnobservable {
                name: "max_input_tokens".to_owned(),
                reason: "the provider reported no usage".to_owned(),
            }),
            r#"{"kind":"budget-unobservable","name":"max_input_tokens","reason":"the provider reported no usage"}"#,
        ),
        (
            LoopStop::Deadline(LoopStopDeadline { limit_ms: 60_000 }),
            r#"{"kind":"deadline","limit_ms":60000}"#,
        ),
        (
            LoopStop::Cancelled(LoopStopCancelled {
                reason: "operator interrupt".to_owned(),
            }),
            r#"{"kind":"cancelled","reason":"operator interrupt"}"#,
        ),
        (
            LoopStop::AwaitingApproval(LoopStopAwaitingApproval {
                checkpoint_id: "chk-7".to_owned(),
            }),
            r#"{"kind":"awaiting-approval","checkpoint_id":"chk-7"}"#,
        ),
        (
            LoopStop::ProviderIncomplete(LoopStopProviderIncomplete {
                reason: "max_output_tokens".to_owned(),
            }),
            r#"{"kind":"provider-incomplete","reason":"max_output_tokens"}"#,
        ),
        (
            LoopStop::Unstructured(LoopStopUnstructured { asked_again: 2 }),
            r#"{"kind":"unstructured","asked_again":2}"#,
        ),
        (
            LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
                window: 200_000,
                target: 100_000,
                occupied: 190_000,
            }),
            r#"{"kind":"context-above-trigger","window":200000,"target":100000,"occupied":190000}"#,
        ),
    ]
}

fn finished(stop: LoopStop) -> LoopEvent {
    LoopEvent::Finished { stop, turns: 3 }
}

fn delegate_finished(stop: LoopStop) -> LoopEvent {
    LoopEvent::DelegateFinished {
        call_id: CallId::new("call-1").expect("valid"),
        stop,
        turns: 4,
    }
}

fn outcome(stop: LoopStop) -> LoopOutcome {
    LoopOutcome {
        stop,
        text: String::new(),
        items: Vec::new(),
        turns: 5,
        usage: Vec::new(),
        cost_micro_usd: None,
        structured: None,
        checkpoint: None,
    }
}

fn finished_text(stop: &str) -> String {
    format!(r#"{{"kind":"finished","stop":{stop},"turns":3}}"#)
}

fn delegate_finished_text(stop: &str) -> String {
    format!(r#"{{"kind":"delegate-finished","call_id":"call-1","stop":{stop},"turns":4}}"#)
}

fn outcome_text(stop: &str) -> String {
    format!(r#"{{"stop":{stop},"text":"","items":[],"turns":5,"usage":[]}}"#)
}

/// Writes `value`, asserts the exact text, and reads that text back to `value`.
fn round_trip<T>(value: &T, expected: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let written = serde_json::to_string(value).expect("serializes");
    assert_eq!(written, expected, "the JSON of {value:?} changed");
    let read: T = serde_json::from_str(expected).expect("deserializes");
    assert_eq!(&read, value, "{expected} reads back to another value");
}

fn assert_case(stop: &str) {
    let (value, text) = causes()
        .into_iter()
        .find(|(_, text)| serde_json::from_str::<Value>(text).expect("json")["kind"] == stop)
        .unwrap_or_else(|| panic!("no case for `{stop}`"));
    round_trip(&finished(value.clone()), &finished_text(text));
    round_trip(
        &delegate_finished(value.clone()),
        &delegate_finished_text(text),
    );
    round_trip(&outcome(value), &outcome_text(text));
}

#[test]
fn completed_round_trips_unchanged() {
    assert_case("completed");
}

#[test]
fn max_turns_round_trips_unchanged() {
    assert_case("max-turns");
}

#[test]
fn max_input_tokens_round_trips_unchanged() {
    assert_case("max-input-tokens");
}

#[test]
fn max_output_tokens_round_trips_unchanged() {
    assert_case("max-output-tokens");
}

#[test]
fn max_cost_round_trips_unchanged() {
    assert_case("max-cost");
}

#[test]
fn budget_unobservable_round_trips_unchanged() {
    assert_case("budget-unobservable");
}

#[test]
fn deadline_round_trips_unchanged() {
    assert_case("deadline");
}

#[test]
fn cancelled_round_trips_unchanged() {
    assert_case("cancelled");
}

#[test]
fn awaiting_approval_round_trips_unchanged() {
    assert_case("awaiting-approval");
}

#[test]
fn provider_incomplete_round_trips_unchanged() {
    assert_case("provider-incomplete");
}

#[test]
fn unstructured_round_trips_unchanged() {
    assert_case("unstructured");
}

#[test]
fn context_above_trigger_round_trips_unchanged() {
    assert_case("context-above-trigger");
}

#[test]
fn every_cause_has_a_pinned_case() {
    // Twelve causes, twelve distinct tags: a cause added without a pinned literal fails here.
    let tags: std::collections::BTreeSet<String> = causes()
        .iter()
        .map(|(_, text)| {
            serde_json::from_str::<Value>(text).expect("json")["kind"]
                .as_str()
                .expect("tag")
                .to_owned()
        })
        .collect();
    assert_eq!(tags.len(), 12, "{tags:?}");
}

/// A stop text no carrier may read: refused inside each of the three.
fn assert_refused(stop: &str, why: &str) {
    for text in [finished_text(stop), delegate_finished_text(stop)] {
        let event = serde_json::from_str::<LoopEvent>(&text);
        assert!(event.is_err(), "{why}: {text} was read: {event:?}");
    }
    let text = outcome_text(stop);
    let outcome = serde_json::from_str::<LoopOutcome>(&text);
    assert!(outcome.is_err(), "{why}: {text} was read: {outcome:?}");
}

#[test]
fn an_unknown_field_is_refused_on_every_cause() {
    for (_, text) in causes() {
        let mut value: Value = serde_json::from_str(text).expect("json");
        value["unexpected"] = Value::from(1);
        assert_refused(&value.to_string(), "unknown field");
    }
}

#[test]
fn an_unknown_or_missing_tag_is_refused() {
    assert_refused(r#"{"kind":"exhausted"}"#, "unknown tag");
    assert_refused(r#"{"kind":"MaxTurns","limit":20}"#, "tag in another case");
    assert_refused(r#"{"limit":20}"#, "no tag");
    assert_refused(r#"{"kind":7}"#, "tag not a string");
    assert_refused(r#""completed""#, "not an object");
}

#[test]
fn a_negative_number_is_refused_in_every_numeric_field() {
    for (_, text) in causes() {
        let value: Value = serde_json::from_str(text).expect("json");
        for (field, member) in value.as_object().expect("object") {
            if member.is_number() {
                let mut negative = value.clone();
                negative[field] = Value::from(-1);
                assert_refused(&negative.to_string(), "negative number");
            }
        }
    }
}

#[test]
fn a_missing_or_mistyped_field_is_refused() {
    assert_refused(r#"{"kind":"max-turns"}"#, "missing field");
    assert_refused(r#"{"kind":"max-turns","limit":"20"}"#, "number as a string");
    assert_refused(r#"{"kind":"max-turns","limit":20.5}"#, "fraction");
    assert_refused(r#"{"kind":"cancelled","reason":7}"#, "string as a number");
    assert_refused(
        r#"{"kind":"context-above-trigger","window":1,"target":1}"#,
        "missing field",
    );
}
