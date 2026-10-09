//! Acceptance for `story:slack-plugin`, the fixture run: one channel with five messages (a mention
//! asking a question, a question, an answered thread, a bot message and a task request) and a
//! `docs` source with a `search` operation, hosted for one cycle by the plugin host. The
//! classifier answers by the item's text, each turn is a scripted model port, and the fake
//! `connectors` answers from `tests/fixtures/`.

mod support;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use b10x_loom_plugin::datasource::{ReadKind, SourceName};
use b10x_loom_plugin::state::StateDir;
use b10x_loom_plugin::{Intent, RecordLine, RecordOutcome, SourceRead};
use b10x_loom_plugin_slack::SlackHandler;
use support::{
    CHANNEL, Fixture, HISTORY, Keyed, LIST, PROTOCOL, REPLIES, SEARCH, Shared, classifier, config,
    fixture, handler, search_then_propose, turns,
};

const MENTION: &str = "C0FIXTURE1:1700000100.000100";
const QUESTION: &str = "C0FIXTURE1:1700000200.000200";
const TASK: &str = "C0FIXTURE1:1700000500.000500";

/// The fixture run's plugin: its classifier, and one scripted turn for each of the two answers.
fn plugin(fixture: &Fixture) -> (SlackHandler, Arc<Keyed>, Arc<Mutex<usize>>) {
    let keyed = Arc::new(classifier());
    let (models, made) = turns(vec![
        search_then_propose(
            "rotate deploy key",
            "Rotate it with the deploy-key runbook (runbooks/deploy-key.md).",
        ),
        search_then_propose(
            "staging database runbook",
            "The staging database runbook is runbooks/staging-database.md.",
        ),
    ]);
    let plugin = handler(config(fixture))
        .with_classifier(Box::new(Shared(Arc::clone(&keyed))))
        .with_turn_models(models);
    (plugin, keyed, made)
}

/// One cycle of the fixture run on `fixture`'s state directory.
fn run_once(fixture: &Fixture, plugin: &SlackHandler) -> Vec<RecordLine> {
    plugin
        .run_on(
            &fixture.host(&plugin.config().plugin),
            &fixture.plugin_state(),
            &AtomicBool::new(true),
        )
        .expect("one cycle runs")
}

fn line<'l>(lines: &'l [RecordLine], item: &str) -> &'l RecordLine {
    lines
        .iter()
        .find(|line| line.item.0 == item)
        .unwrap_or_else(|| panic!("a line for {item}: {lines:?}"))
}

#[test]
fn fixture_run_records_three_proposals() {
    let fixture = fixture("run-three");
    let (plugin, _, _) = plugin(&fixture);

    let lines = run_once(&fixture, &plugin);

    let items: Vec<&str> = lines.iter().map(|line| line.item.0.as_str()).collect();
    assert_eq!(
        items,
        [MENTION, QUESTION, TASK],
        "the mention first: {lines:?}"
    );
    let recorded = StateDir::open(&fixture.plugin_state(), &plugin.config().plugin)
        .expect("the state directory opens")
        .record()
        .expect("the record reads");
    assert_eq!(recorded, lines, "exactly these three lines are recorded");
}

#[test]
fn fixture_run_intents() {
    let fixture = fixture("run-intents");
    let (plugin, keyed, _) = plugin(&fixture);

    let lines = run_once(&fixture, &plugin);

    assert_eq!(line(&lines, MENTION).intent, Some(Intent::Ask));
    assert_eq!(line(&lines, QUESTION).intent, Some(Intent::Find));
    assert_eq!(line(&lines, TASK).intent, Some(Intent::Task));
    assert_eq!(
        keyed.requests(),
        4,
        "one classification per item, and the task's protocol pick"
    );
}

#[test]
fn fixture_answers_cite_their_reads() {
    let fixture = fixture("run-reads");
    let (plugin, _, made) = plugin(&fixture);

    let lines = run_once(&fixture, &plugin);

    let docs = SourceRead {
        source: SourceName("docs".to_owned()),
        kind: ReadKind::Search,
    };
    for answer in [MENTION, QUESTION] {
        let line = line(&lines, answer);
        assert_eq!(line.outcome, RecordOutcome::Proposed, "{line:?}");
        assert!(
            line.reads.contains(&docs),
            "{answer} cites its read: {line:?}"
        );
        assert!(
            line.proposal
                .as_deref()
                .is_some_and(|text| text.contains("runbooks/"))
        );
    }
    let task = line(&lines, TASK);
    assert_eq!(task.outcome, RecordOutcome::ProposedCase, "{task:?}");
    assert_eq!(
        task.proposed_case
            .as_ref()
            .map(|case| case.protocol.as_str()),
        Some(PROTOCOL)
    );
    assert!(task.reads.is_empty() && task.proposal.is_none());
    assert_eq!(
        *made.lock().unwrap(),
        2,
        "a turn for each answer, none for the task"
    );
}

#[test]
fn fixture_run_invokes_no_write() {
    let fixture = fixture("run-no-write");
    let (plugin, _, _) = plugin(&fixture);

    run_once(&fixture, &plugin);

    let argv = fixture.argv();
    assert!(!argv.is_empty(), "the fake was run");
    let mut invoked = Vec::new();
    for run in &argv {
        let verb = run
            .windows(2)
            .find(|w| w[0] == "operations")
            .map(|w| w[1].as_str());
        let operation = run
            .windows(2)
            .find(|w| w[0] == "--operation")
            .map(|w| w[1].as_str())
            .unwrap_or_default();
        assert!(
            matches!(verb, Some("describe" | "invoke")),
            "only describe and invoke run: {run:?}"
        );
        assert!(
            [LIST, HISTORY, REPLIES, SEARCH].contains(&operation),
            "only the three Slack reads and the docs search: {run:?}"
        );
        assert!(
            !run.iter().any(|argument| argument == "--approval-file"),
            "no run carries an approval: {run:?}"
        );
        if verb == Some("invoke") {
            invoked.push(operation.to_owned());
        }
    }
    assert_eq!(
        invoked,
        [LIST, HISTORY, REPLIES, SEARCH, SEARCH],
        "the list, {CHANNEL}'s history, the answered thread, and one search per answer"
    );
}

#[test]
fn second_run_records_nothing_new() {
    let fixture = fixture("run-twice");
    let (plugin, keyed, made) = plugin(&fixture);
    let first = run_once(&fixture, &plugin);
    let asked = keyed.requests();

    let second = run_once(&fixture, &plugin);

    assert_eq!(first.len(), 3);
    assert!(second.is_empty(), "nothing new is recorded: {second:?}");
    let recorded = StateDir::open(&fixture.plugin_state(), &plugin.config().plugin)
        .unwrap()
        .record()
        .unwrap();
    assert_eq!(recorded.len(), 3, "the record still holds three lines");
    assert_eq!(keyed.requests(), asked, "no item was classified again");
    assert_eq!(*made.lock().unwrap(), 2, "no further turn ran");
}
