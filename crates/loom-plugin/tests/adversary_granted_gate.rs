//! Adversary cases for `story:granted-gate-ends-run` (wave 2026-10-12-w1, unit U2), the plugin
//! turn after the re-run rule was removed. Each case asserts what the crate's documentation says
//! and is red against the tree it was written for.

mod support;

use b10x_loom_plugin::project::project;
use b10x_loom_plugin::turn::{TURN_STEP_BUDGET, turn};
use b10x_loom_plugin::{Intent, RecordOutcome};
use serde_json::json;
use support::{Fixture, Scripted, chat, classification, item};

/// A turn whose model reads on every step but the last and proposes on the last one, within
/// `TURN_STEP_BUDGET` steps, reaches `proposed`: "the case's outcome is the turn's: `proposed` with
/// the proposal's text" (`turn.rs` module doc). The proposal is performed on the turn's last
/// budgeted step and completes the case. A granted step leaves no gate pending, so the Run must
/// load the case before it is suspended for its budget; had it not, the turn would report
/// `stopped` with no proposal although the reply was proposed and recorded.
#[test]
fn a_proposal_on_the_last_budgeted_step_ends_the_turn_proposed() {
    let fixture = Fixture::new("adversary-granted-gate", "last-step-proposal");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["chat"], "0.9");
    let projection = project(&config, &classified);
    let read = (
        "source_read",
        json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
    );
    let mut script = vec![read; TURN_STEP_BUDGET - 1];
    script.push((
        "reply_propose",
        json!({"text": "Yes: the deploy finished."}),
    ));
    let (model, seen) = Scripted::new(script);

    let ended = turn(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &projection,
    )
    .expect("the turn runs");

    assert_eq!(
        seen.lock().unwrap().len(),
        TURN_STEP_BUDGET,
        "precondition: one model turn per step, the proposal on the last"
    );
    assert_eq!(ended.reads.len(), TURN_STEP_BUDGET - 1, "{ended:?}");
    assert_eq!(
        (ended.outcome, ended.proposal.as_deref()),
        (RecordOutcome::Proposed, Some("Yes: the deploy finished.")),
        "the proposal performed on the last budgeted step is the turn's outcome: {:?}",
        ended.detail
    );
}
