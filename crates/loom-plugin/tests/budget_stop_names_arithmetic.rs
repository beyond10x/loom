//! Acceptance for `story:turn-budget-stop-names-the-arithmetic`: a turn that ends on its budgets
//! without an answer says which budget ended it and why the two run out together, since each step
//! asks the model twice (a selection and its arguments). Before the story a turn whose eight steps
//! all read was recorded "the turn used its 16 model calls" although its step budget ended it.
//! Since 0.55.2 a turn that has performed a read is offered only the answer on its last step
//! (`story:turn-answers-before-its-budget-ends`), so a budget ends a turn whose reads were all
//! refused, as here.

mod support;

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, TURN_STEP_BUDGET, TurnOptions, turn, turn_with};
use b10x_loom_plugin::{Intent, RecordOutcome};
use serde_json::json;
use support::{Fixture, Scripted, chat, classification, item};

/// The refusal the effect port answers for a read whose `input` is no object.
const REFUSAL: &str = "source.read of `chat` has an `input` that is not an object";

fn read() -> (&'static str, serde_json::Value) {
    (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": "C0FIXTURE1"}),
    )
}

/// Eight refused reads under the default budgets (8 steps, 16 model calls): the step budget ends the
/// turn, and the detail names both budgets and the two calls a step.
#[test]
fn a_turn_ended_by_its_step_budget_names_both_budgets() {
    let fixture = Fixture::new("budget-stop-names-arithmetic", "step-budget");
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

    assert_eq!(seen.lock().unwrap().len(), 2 * TURN_STEP_BUDGET);
    assert_eq!(ended.outcome, RecordOutcome::Stopped);
    assert!(ended.reads.is_empty());
    assert_eq!(
        ended.detail.as_deref(),
        Some(
            format!(
                "the turn used its {TURN_STEP_BUDGET} steps (step_budget) and \
                 {TURN_MODEL_BUDGET} of its {TURN_MODEL_BUDGET} model calls (model_budget) \
                 without an answer, after 0 reads; each step asks the model twice, a \
                 selection and its arguments; the last effect was refused: {REFUSAL}"
            )
            .as_str()
        ),
        "story:turn-budget-stop-names-the-arithmetic"
    );
}

/// A model budget below two calls a step ends the turn before its step budget: the detail says
/// how many steps the model budget allows.
#[test]
fn a_turn_ended_by_its_model_budget_names_the_steps_it_allows() {
    let fixture = Fixture::new("budget-stop-names-arithmetic", "model-budget");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![read(); 4]);
    let options = TurnOptions {
        step_budget: 4,
        model_budget: 4,
        ..TurnOptions::defaults()
    };

    let ended = turn_with(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
        options,
    )
    .expect("the turn runs")
    .result;

    assert_eq!(seen.lock().unwrap().len(), 4);
    assert_eq!(ended.outcome, RecordOutcome::Stopped);
    assert_eq!(
        ended.detail.as_deref(),
        Some(
            "the turn used its 4 model calls (model_budget) after 2 of its 4 steps \
             (step_budget) without an answer; each step asks the model twice, a selection and \
             its arguments, so 4 model calls allow 2 steps; the last effect was refused: \
             source.read of `chat` has an `input` that is not an object"
        ),
        "story:turn-budget-stop-names-the-arithmetic"
    );
}
