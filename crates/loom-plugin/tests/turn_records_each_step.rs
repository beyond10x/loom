//! Acceptance for `story:turn-records-each-step`: a turn's result, and the record line made of
//! it, carry one entry per step in order: the action selected, how the effect port answered it,
//! the refusal reason, and the steps and model calls left after it. A record line written before
//! the member existed is read back with no steps. Before the story a stopped turn kept only its
//! reads and one detail, so the step where it went wrong could not be found from the record.

mod support;

use b10x_loom_plugin::project::{PROPOSE, READ, project};
use b10x_loom_plugin::turn::turn;
use b10x_loom_plugin::{
    Intent, ItemId, RecordLine, RecordOutcome, StepOutcome, TurnStep, decode_record_line,
    encode_record_line,
};
use serde_json::json;
use support::{Fixture, Scripted, chat, classification, item};

/// The refusal the effect port answers for a read whose `input` is no object.
const REFUSAL: &str = "source.read of `chat` has an `input` that is not an object";

fn step(
    action: &str,
    outcome: StepOutcome,
    reason: Option<&str>,
    steps_left: i64,
    model_calls_left: i64,
) -> TurnStep {
    TurnStep {
        action: Some(action.to_owned()),
        outcome,
        reason: reason.map(str::to_owned),
        steps_left,
        model_calls_left,
    }
}

/// A refused read, a read and a proposal under the default budgets (8 steps, 16 model calls): three
/// entries, each with what the turn had left after it.
#[test]
fn a_turn_records_each_step_with_its_outcome_and_the_budget_left() {
    let fixture = Fixture::new("turn-records-each-step", "three-steps");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, _seen) = Scripted::new(vec![
        (
            "source_read",
            json!({"source": "chat", "kind": "list", "input": "C0FIXTURE1"}),
        ),
        (
            "source_read",
            json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
        ),
        (
            "reply_propose",
            json!({"text": "Yes: the deploy finished."}),
        ),
    ]);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(ended.outcome, RecordOutcome::Proposed, "{:?}", ended.detail);
    assert_eq!(
        ended.steps,
        vec![
            step(READ, StepOutcome::Refused, Some(REFUSAL), 7, 14),
            step(READ, StepOutcome::Performed, None, 6, 12),
            step(PROPOSE, StepOutcome::Performed, None, 5, 10),
        ],
        "story:turn-records-each-step: one entry per step, in order"
    );
}

/// The steps reach the record line and are read back from it; a line without them (written
/// before 0.55.2) is read back with none.
#[test]
fn a_record_line_keeps_the_steps_and_an_older_line_reads_with_none() {
    let line = RecordLine {
        item: ItemId("item-1".to_owned()),
        intent: Some(Intent::Ask),
        confidence: None,
        outcome: RecordOutcome::Stopped,
        reads: Vec::new(),
        proposal: None,
        proposed_case: None,
        detail: Some("the turn used its 1 step (step_budget)".to_owned()),
        steps: vec![
            step(READ, StepOutcome::Refused, Some(REFUSAL), 0, 14),
            TurnStep {
                action: None,
                outcome: StepOutcome::NotInvoked,
                reason: None,
                steps_left: 0,
                model_calls_left: 14,
            },
        ],
    };

    let text = encode_record_line(&line);
    assert_eq!(
        decode_record_line(&text).as_ref(),
        Ok(&line),
        "story:turn-records-each-step: the steps survive the record: {text}"
    );

    let older = text
        .split_once(",\"steps\":")
        .map(|(head, _)| format!("{head}}}"))
        .expect("the line names its steps last");
    let read_back = decode_record_line(&older).expect("an older line still reads");
    assert!(read_back.steps.is_empty(), "{read_back:?}");
}
