//! Acceptance for `story:turn-answers-before-its-budget-ends`: a turn whose remaining steps or
//! model calls only cover one more step offers that step only the answer (the frontier's actions
//! other than `source.read`) once it has performed a read, so a turn whose later reads were
//! refused, or that read until its last step, still gets the step it needs to answer. Before the
//! story the last step was offered every read, and a model that read on it ended the turn without
//! an answer. A turn that has read nothing keeps its reads to the end, since a proposal completes
//! the case only from a source read (`refused_read_goes_back.rs`).

mod support;

use b10x_loom_plugin::project::{READ, project};
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, TURN_STEP_BUDGET, TurnOptions, turn, turn_with};
use b10x_loom_plugin::{Intent, RecordOutcome};
use llm_core::TurnRequest;
use serde_json::json;
use support::{Fixture, Scripted, chat, classification, item};

/// The actions a selection request offers: the `enum` of its forced tool's schema.
fn offered(request: &TurnRequest) -> Vec<String> {
    fn find(value: &serde_json::Value) -> Option<Vec<String>> {
        match value {
            serde_json::Value::Object(fields) => fields
                .get("enum")
                .and_then(serde_json::Value::as_array)
                .map(|names| {
                    names
                        .iter()
                        .filter_map(|name| name.as_str().map(str::to_owned))
                        .collect()
                })
                .or_else(|| fields.values().find_map(find)),
            serde_json::Value::Array(values) => values.iter().find_map(find),
            _ => None,
        }
    }
    let tool = serde_json::to_value(&request.tools[0]).expect("a tool encodes");
    find(&tool).expect("a selection request lists its candidates")
}

fn refused_read() -> (&'static str, serde_json::Value) {
    (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": "C0FIXTURE1"}),
    )
}

fn read() -> (&'static str, serde_json::Value) {
    (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
    )
}

fn propose() -> (&'static str, serde_json::Value) {
    (
        "reply_propose",
        json!({"text": "Yes: the deploy finished."}),
    )
}

/// Three steps, a read and then a refused read: the third selection offers no read, and the
/// model's answer ends the turn with a proposal.
#[test]
fn the_last_step_after_a_refused_read_is_offered_only_the_answer() {
    let fixture = Fixture::new("answer-before-budget-ends", "last-step");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![read(), refused_read(), propose()]);

    let ended = turn_with(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
        TurnOptions {
            step_budget: 3,
            model_budget: 6,
            ..TurnOptions::defaults()
        },
    )
    .expect("the turn runs")
    .result;

    let requests = seen.lock().unwrap().clone();
    assert!(
        offered(&requests[0]).contains(&READ.to_owned()),
        "a step with more left is offered the reads: {:?}",
        offered(&requests[0])
    );
    assert!(
        offered(&requests[2]).contains(&READ.to_owned()),
        "two steps left still cover a read: {:?}",
        offered(&requests[2])
    );
    assert!(
        !offered(&requests[4]).contains(&READ.to_owned()),
        "story:turn-answers-before-its-budget-ends: the last step is offered only the answer: {:?}",
        offered(&requests[4])
    );
    assert_eq!(
        ended.outcome,
        RecordOutcome::Proposed,
        "{:?} {:?}",
        ended.detail,
        ended.steps
    );
}

/// Model calls for one more step only (2 of 6 left after two reads), steps to spare: that step is
/// offered only the answer.
#[test]
fn the_step_the_model_calls_still_cover_is_offered_only_the_answer() {
    let fixture = Fixture::new("answer-before-budget-ends", "model-calls");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![read(), read(), propose()]);

    let ended = turn_with(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
        TurnOptions {
            step_budget: 8,
            model_budget: 6,
            ..TurnOptions::defaults()
        },
    )
    .expect("the turn runs")
    .result;

    let requests = seen.lock().unwrap().clone();
    assert!(
        offered(&requests[2]).contains(&READ.to_owned()),
        "with four calls left a read is offered: {:?}",
        offered(&requests[2])
    );
    assert!(
        !offered(&requests[4]).contains(&READ.to_owned()),
        "story:turn-answers-before-its-budget-ends: two calls left cover only the answer: {:?}",
        offered(&requests[4])
    );
    assert_eq!(
        ended.outcome,
        RecordOutcome::Proposed,
        "{:?} {:?}",
        ended.detail,
        ended.steps
    );
    assert_eq!(ended.reads.len(), 2);
}

/// A model that selects a read on the step offered only the answer ends the turn naming that it
/// ran out before answering, the answers it was offered and what it used.
#[test]
fn a_turn_whose_model_answers_none_on_its_last_step_names_that_it_ran_out() {
    let fixture = Fixture::new("answer-before-budget-ends", "ran-out");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![read(); TURN_STEP_BUDGET]);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(ended.outcome, RecordOutcome::Stopped, "{:?}", ended.detail);
    assert_eq!(seen.lock().unwrap().len(), 2 * TURN_STEP_BUDGET - 1);
    assert_eq!(ended.reads.len(), TURN_STEP_BUDGET - 1);
    assert_eq!(
        ended.detail.as_deref(),
        Some(
            format!(
                "the turn ran out before answering: its last step was offered only the answer \
                 (reply.decline, reply.propose) and the model selected none, after \
                 {TURN_STEP_BUDGET} of its {TURN_STEP_BUDGET} steps (step_budget), {} of its \
                 {TURN_MODEL_BUDGET} model calls (model_budget) and {} reads",
                2 * TURN_STEP_BUDGET - 1,
                TURN_STEP_BUDGET - 1
            )
            .as_str()
        ),
        "story:turn-answers-before-its-budget-ends"
    );
}
