//! Acceptance for `story:refused-effect-goes-back-to-the-model`, the plugin turn's half: a read
//! the effect port refuses for its arguments is an observation the model gets on its next request,
//! and the turn goes on, so the model can correct the read within the turn's budgets. Before the
//! story the Run ended `AwaitingApproval` on the next iteration and the turn was recorded `stopped`
//! naming approval, never the refusal.

mod support;

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::turn::{TurnOptions, turn, turn_with};
use b10x_loom_plugin::{Intent, RecordOutcome};
use serde_json::json;
use support::{Fixture, Scripted, chat, classification, item, request_text};

/// The refusal the effect port answers for a read whose `input` is no object.
const REFUSAL: &str = "source.read of `chat` has an `input` that is not an object";

/// A refused read, then the corrected read, then a proposal: the turn proposes, its one read is the
/// corrected one, and the request after the refusal carries it.
#[test]
fn a_refused_read_goes_back_to_the_model_and_the_turn_goes_on() {
    let fixture = Fixture::new("refused-read-goes-back", "corrected");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let (model, seen) = Scripted::new(vec![
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

    assert_eq!(
        (ended.outcome, ended.proposal.as_deref()),
        (RecordOutcome::Proposed, Some("Yes: the deploy finished.")),
        "story:refused-effect-goes-back-to-the-model: a refused read does not end the turn: {:?}",
        ended.detail
    );
    assert_eq!(
        ended.reads.len(),
        1,
        "only the corrected read was performed"
    );
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 6, "three steps, two requests each");
    assert!(
        !request_text(&requests[1]).contains(REFUSAL),
        "the refusal is not known before the read"
    );
    assert!(
        request_text(&requests[2]).contains(&format!("refused: {REFUSAL}")),
        "the selection after the refused read tells the model why"
    );
}

/// A turn whose model repeats the refused read until its step budget stops it is recorded
/// `stopped` naming the refusal, never approval.
#[test]
fn a_turn_that_stops_after_a_refused_read_names_the_refusal() {
    let fixture = Fixture::new("refused-read-goes-back", "stops");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let refused = (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": "C0FIXTURE1"}),
    );
    let (model, _seen) = Scripted::new(vec![refused.clone(), refused.clone(), refused]);
    let options = TurnOptions {
        step_budget: 3,
        model_budget: 6,
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

    assert_eq!(ended.outcome, RecordOutcome::Stopped);
    let detail = ended.detail.unwrap_or_default();
    assert!(
        detail.contains(REFUSAL),
        "story:refused-effect-goes-back-to-the-model: the stop names the refusal: {detail}"
    );
    assert!(
        !detail.contains("AwaitingApproval"),
        "the stop is no approval: {detail}"
    );
}
