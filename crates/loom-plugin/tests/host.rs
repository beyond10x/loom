//! Acceptance for `story:plugin-host`, the host loop: `run_plugin` over a fake plugin whose poll
//! answers fixed items, whose classifier is recorded and whose turns run on scripted model ports
//! against the fake `connectors` of `loom-connectors`. The state directory is the fixture's own.

mod support;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_plugin::datasource::{ReadKind, SourceName};
use b10x_loom_plugin::state::{RECORD_FILE, STATE_FILE, StateDir};
use b10x_loom_plugin::{
    Cursor, Decimal, Intent, ItemId, PluginError, PluginState, RecordLine, RecordOutcome,
    SourceRead, decode_record_line, encode_record_line, run_plugin, run_plugin_on,
};
use serde_json::json;
use support::{FakePlugin, Fixture, Recorded, chat, item};

/// A turn that reads chat's list, then proposes `text`.
fn read_then_propose(text: &str) -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "source_read",
            json!({"source": "chat", "kind": "list", "input": {"channel": "C0FIXTURE1"}}),
        ),
        ("reply_propose", json!({"text": text})),
    ]
}

#[test]
fn low_confidence_runs_no_turn() {
    let fixture = Fixture::new("host", "low_confidence");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.2),
    )
    .with_turn(read_then_propose("unused"));

    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("one cycle");

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].outcome, RecordOutcome::Unclassified);
    assert_eq!(lines[0].intent, Some(Intent::Ask));
    assert_eq!(lines[0].confidence, Some(Decimal("0.2".to_owned())));
    assert!(lines[0].reads.is_empty() && lines[0].proposal.is_none());
    assert_eq!(plugin.turn_models.load(Ordering::SeqCst), 0, "no turn ran");
    assert!(fixture.argv().is_empty(), "no source was described or read");

    // At the threshold an item is classified: 0.5 against the default 0.5 runs the turn.
    let fixture = Fixture::new("host", "at_threshold");
    let mut config = fixture.config(vec![chat()]);
    config.classify_threshold = None;
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.5),
    )
    .with_turn(read_then_propose("Yes."));
    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("one cycle");
    assert_eq!(lines[0].outcome, RecordOutcome::Proposed, "{:?}", lines[0]);
    assert_eq!(plugin.turn_models.load(Ordering::SeqCst), 1);
}

#[test]
fn record_line_carries_reads() {
    let fixture = Fixture::new("host", "record_line");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.9),
    )
    .with_turn(read_then_propose("Yes: the deploy finished."));

    run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("one cycle");

    let text = fs::read_to_string(fixture.plugin_state().join(RECORD_FILE)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1, "{text}");
    let line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(
        line,
        json!({
            "item": "item-1",
            "intent": "ask",
            "confidence": 0.9,
            "outcome": "proposed",
            "reads": [{"source": "chat", "kind": "list"}],
            "proposal": "Yes: the deploy finished."
        })
    );
    let read = decode_record_line(lines[0]).expect("a record line");
    assert_eq!(
        read,
        RecordLine {
            item: ItemId("item-1".to_owned()),
            intent: Some(Intent::Ask),
            confidence: Some(Decimal("0.9".to_owned())),
            outcome: RecordOutcome::Proposed,
            reads: vec![SourceRead {
                source: SourceName("chat".to_owned()),
                kind: ReadKind::List
            }],
            proposal: Some("Yes: the deploy finished.".to_owned()),
            proposed_case: None,
            detail: None,
        }
    );
    assert_eq!(encode_record_line(&read), lines[0], "the codec round-trips");
}

#[test]
fn state_written_atomically() {
    let fixture = Fixture::new("host", "atomic");
    let config = fixture.config(vec![chat()]);
    let state = fixture.plugin_state();
    fs::create_dir_all(&state).unwrap();
    let before = "{\"cursors\":[],\"handled\":[\"item-0\"],\"failing\":[]}\n";
    fs::write(state.join(STATE_FILE), before).unwrap();
    // A second name for the state file as it was: an in-place write would change it too.
    fs::hard_link(state.join(STATE_FILE), state.join("held.json")).unwrap();
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.1),
    );

    run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    )
    .expect("one cycle");

    assert_eq!(
        fs::read_to_string(state.join("held.json")).unwrap(),
        before,
        "the old state file was replaced by a rename, never written in place"
    );
    assert_eq!(
        StateDir::open(&state, &config).unwrap().load().unwrap(),
        PluginState {
            cursors: vec![Cursor {
                name: "C0FIXTURE1".to_owned(),
                value: "poll-1".to_owned()
            }],
            handled: vec![ItemId("item-0".to_owned()), ItemId("item-1".to_owned())],
            failing: Vec::new(),
        }
    );
    let mut names: Vec<String> = fs::read_dir(&state)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["held.json", RECORD_FILE, STATE_FILE],
        "no temporary file is left behind"
    );
}

#[test]
fn state_inside_a_checkout_is_refused() {
    let fixture = Fixture::new("host", "inside_checkout");
    let checkout = fixture.root.join("checkout");
    let roots = fixture.root.join("workspaces");
    fs::create_dir_all(checkout.join("src")).unwrap();
    fs::create_dir_all(&roots).unwrap();
    std::os::unix::fs::symlink(&checkout, fixture.root.join("link")).unwrap();
    let mut config = fixture.config(vec![chat()]);
    config.checkouts = vec![checkout.display().to_string()];
    config.workspace_roots = vec![roots.display().to_string()];
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.1),
    );

    for state in [
        checkout.clone(),
        checkout.join("plugin-state"),
        checkout.join("src/deeper/plugin-state"),
        fixture.root.join("elsewhere/../checkout/plugin-state"),
        fixture.root.join("link/plugin-state"),
        roots.join("one/plugin-state"),
    ] {
        let refused = run_plugin(&plugin, &config, &state, &AtomicBool::new(true));
        assert!(
            matches!(refused, Err(PluginError::StateInsideCheckout { .. })),
            "{}: {refused:?}",
            state.display()
        );
    }
    assert_eq!(plugin.polls.load(Ordering::SeqCst), 0, "nothing was polled");
    assert!(
        !checkout.join("plugin-state").exists(),
        "nothing was created"
    );
    assert!(!roots.join("one").exists());

    // A sibling whose name only starts with the checkout's is outside it.
    let sibling = fixture.root.join("checkout-state");
    run_plugin(&plugin, &config, &sibling, &AtomicBool::new(true)).expect("outside the checkout");
    assert!(sibling.join(STATE_FILE).exists());
}

#[test]
fn host_polls_until_stopped() {
    let fixture = Fixture::new("host", "until_stopped");
    let config = fixture.config(vec![chat()]);
    assert_eq!(config.poll_interval_seconds, 0);
    let stop = Arc::new(AtomicBool::new(false));
    let plugin = FakePlugin {
        stop_at: Some((2, Arc::clone(&stop))),
        ..FakePlugin::new(Vec::new(), Recorded::classifying("ask", &[], 0.9))
    };

    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &stop,
    )
    .expect("polls until stopped");

    assert_eq!(plugin.polls.load(Ordering::SeqCst), 2, "two polls recorded");
    assert!(lines.is_empty());
    let state = StateDir::open(&fixture.plugin_state(), &config)
        .unwrap()
        .load()
        .unwrap();
    assert_eq!(
        state.cursors,
        vec![Cursor {
            name: "C0FIXTURE1".to_owned(),
            value: "poll-2".to_owned()
        }],
        "the cursor of the last poll is kept"
    );
}

#[test]
fn once_twice_handles_an_item_once() {
    let fixture = Fixture::new("host", "once_twice");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(
        vec![item("item-1"), item("item-2")],
        Recorded::classifying("ask", &["chat"], 0.9),
    )
    .with_turn(read_then_propose("first"))
    .with_turn(read_then_propose("second"));
    let host = fixture.host(&config);

    let first = run_plugin_on(
        &plugin,
        &host,
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("the first run");
    let second = run_plugin_on(
        &plugin,
        &host,
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("the second run");

    assert_eq!(first.len(), 2);
    assert!(
        first
            .iter()
            .all(|line| line.outcome == RecordOutcome::Proposed),
        "{first:?}"
    );
    assert!(second.is_empty(), "the second run adds no line: {second:?}");
    assert_eq!(
        plugin.polls.load(Ordering::SeqCst),
        2,
        "each run polled once"
    );
    assert_eq!(
        plugin.turn_models.load(Ordering::SeqCst),
        2,
        "one turn per item"
    );
    let record = StateDir::open(&fixture.plugin_state(), &config)
        .unwrap()
        .record()
        .unwrap();
    assert_eq!(record, first, "the record holds each item once");
    assert_eq!(fixture.runs("invoke").len(), 2, "each item read once");
}

/// A classify or turn hook that fails leaves the item unhandled, records no line and counts the
/// failure; the item is tried on each run, and its third failure records it `stopped` and handled.
#[test]
fn a_failing_item_is_retried_three_times_then_stopped() {
    // The classifier answers no classification: classify fails.
    let fixture = Fixture::new("host", "retried_classify");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::answering(vec![(
            "classify_item",
            json!({"intent": "chat", "hints": [], "confidence": 0.9}),
        )]),
    );
    let host = fixture.host(&config);
    let state = fixture.plugin_state();
    for attempt in 1..=2 {
        let lines = run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true))
            .expect("a failing item is no host error");
        assert!(
            lines.is_empty(),
            "attempt {attempt} records no line: {lines:?}"
        );
        let held = StateDir::open(&state, &config).unwrap();
        assert!(held.record().unwrap().is_empty(), "attempt {attempt}");
        let current = held.load().unwrap();
        assert!(current.handled.is_empty(), "attempt {attempt}: not handled");
        assert_eq!(current.failing.len(), 1, "attempt {attempt}");
        assert_eq!(current.failing[0].item.id, ItemId("item-1".to_owned()));
        assert_eq!(current.failing[0].failures, attempt);
        assert!(
            current.failing[0].last_failure.contains("intent `chat`"),
            "{:?}",
            current.failing[0]
        );
    }
    let third = run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true)).expect("the third");
    assert_eq!(third.len(), 1, "{third:?}");
    assert_eq!(third[0].outcome, RecordOutcome::Stopped);
    assert_eq!(third[0].intent, None, "classification never succeeded");
    let detail = third[0].detail.as_deref().unwrap_or_default();
    assert!(
        detail.contains("3 attempts failed") && detail.contains("intent `chat`"),
        "{detail}"
    );
    let fourth = run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true)).expect("the fourth");
    assert!(fourth.is_empty(), "a stopped item is handled: {fourth:?}");
    assert_eq!(
        plugin.classifier.requests().len(),
        3,
        "classified three times"
    );
    let held = StateDir::open(&state, &config).unwrap();
    let current = held.load().unwrap();
    assert_eq!(current.handled, vec![ItemId("item-1".to_owned())]);
    assert!(
        current.failing.is_empty(),
        "a handled item has no failures left"
    );
    assert_eq!(held.record().unwrap(), third);
    drop(held);

    // The turn fails: the fake has no turn model to give.
    let fixture = Fixture::new("host", "retried_turn");
    let config = fixture.config(vec![chat()]);
    let plugin = FakePlugin::new(
        vec![item("item-2")],
        Recorded::classifying("ask", &["chat"], 0.9),
    );
    let host = fixture.host(&config);
    let state = fixture.plugin_state();
    for _ in 1..=2 {
        let lines = run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true)).unwrap();
        assert!(lines.is_empty(), "{lines:?}");
    }
    let third = run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true)).unwrap();
    assert_eq!(third.len(), 1);
    assert_eq!(third[0].outcome, RecordOutcome::Stopped);
    assert_eq!(
        third[0].intent,
        Some(Intent::Ask),
        "the classification is kept"
    );
    assert!(
        third[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("no further turn")),
        "{:?}",
        third[0]
    );
    assert_eq!(plugin.turn_models.load(Ordering::SeqCst), 3);
    assert!(
        run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true))
            .unwrap()
            .is_empty()
    );
}

/// A second host on a held state directory is refused, naming it, and writes nothing.
#[test]
fn a_held_state_directory_refuses_a_second_host() {
    let fixture = Fixture::new("host", "held");
    let config = fixture.config(vec![chat()]);
    let state = fixture.plugin_state();
    let held = StateDir::open(&state, &config).expect("the first host");
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.1),
    );

    let refused = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    );

    assert_eq!(
        refused,
        Err(PluginError::StateInUse {
            state: state.display().to_string()
        })
    );
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains(&state.display().to_string())
    );
    assert!(held.record().unwrap().is_empty(), "nothing was written");
    assert!(StateDir::open(&state, &config).is_err(), "still held");
    drop(held);
    StateDir::open(&state, &config).expect("released once the first host is gone");
}

/// A plugin whose classify panics, as a host killed mid-attempt does, while `crash` is set.
struct Crashing {
    items: Vec<b10x_loom_plugin::InboundItem>,
    classifier: Recorded,
    crash: AtomicBool,
}

impl b10x_loom_plugin::Plugin for Crashing {
    fn name(&self) -> &str {
        "crashing"
    }

    fn poll(
        &self,
        _host: &b10x_loom_plugin::Host<'_>,
        _state: &PluginState,
        _objectives: &[b10x_loom_plugin::Objective],
    ) -> Result<b10x_loom_plugin::Poll, PluginError> {
        Ok(b10x_loom_plugin::Poll {
            items: self.items.clone(),
            cursors: Vec::new(),
        })
    }

    fn classify(
        &self,
        host: &b10x_loom_plugin::Host<'_>,
        item: &b10x_loom_plugin::InboundItem,
    ) -> Result<b10x_loom_plugin::Classification, PluginError> {
        assert!(
            !self.crash.load(Ordering::SeqCst),
            "the host dies in the middle of an attempt"
        );
        b10x_loom_plugin::classify::classify(&self.classifier, host.config(), item)
    }
}

/// An attempt is counted before it starts: one that never returns counts, an attempt that
/// succeeds clears the count, and after three that never returned the item is recorded `stopped`
/// without a fourth.
#[test]
fn a_crashing_attempt_is_counted() {
    let crash = |plugin: &Crashing, fixture: &Fixture, config: &b10x_loom_plugin::PluginConfig| {
        let host = fixture.host(config);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_plugin_on(
                plugin,
                &host,
                &fixture.plugin_state(),
                &AtomicBool::new(true),
            )
        }));
        assert!(crashed.is_err(), "the attempt crashed");
    };

    // One crash, then an attempt that succeeds.
    let fixture = Fixture::new("host", "crash_once");
    let config = fixture.config(vec![chat()]);
    let plugin = Crashing {
        items: vec![item("item-1")],
        classifier: Recorded::classifying("ask", &["chat"], 0.1),
        crash: AtomicBool::new(true),
    };
    crash(&plugin, &fixture, &config);
    let held = StateDir::open(&fixture.plugin_state(), &config).expect("released by the crash");
    let current = held.load().unwrap();
    assert!(held.record().unwrap().is_empty());
    assert_eq!(current.failing.len(), 1, "{current:?}");
    assert_eq!(
        current.failing[0].item,
        item("item-1"),
        "the item itself is kept"
    );
    assert_eq!(current.failing[0].failures, 1, "the crashed attempt counts");
    assert_eq!(current.failing[0].last_failure, "attempt 1 did not finish");
    drop(held);
    plugin.crash.store(false, Ordering::SeqCst);
    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("the second attempt");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].outcome, RecordOutcome::Unclassified);
    let current = StateDir::open(&fixture.plugin_state(), &config)
        .unwrap()
        .load()
        .unwrap();
    assert!(current.failing.is_empty(), "a success clears the count");

    // Three crashes: the item is stopped without a fourth attempt.
    let fixture = Fixture::new("host", "crash_thrice");
    let config = fixture.config(vec![chat()]);
    let plugin = Crashing {
        items: vec![item("item-1")],
        classifier: Recorded::classifying("ask", &["chat"], 0.1),
        crash: AtomicBool::new(true),
    };
    for attempt in 1..=3 {
        crash(&plugin, &fixture, &config);
        let current = StateDir::open(&fixture.plugin_state(), &config)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(current.failing[0].failures, attempt);
    }
    plugin.crash.store(false, Ordering::SeqCst);
    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &fixture.plugin_state(),
        &AtomicBool::new(true),
    )
    .expect("the fourth run");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].outcome, RecordOutcome::Stopped);
    assert_eq!(
        lines[0].detail.as_deref(),
        Some("3 attempts did not succeed; the last: attempt 3 did not finish")
    );
    assert!(
        plugin.classifier.requests().is_empty(),
        "no fourth attempt was made"
    );
}

/// A failure that is not about the item, here the `connectors` program missing so no source can
/// be described, counts against no item: each cycle ends there, saves no cursor, and the item is
/// handled once the outage is over.
#[test]
fn a_host_wide_outage_stops_no_item() {
    let fixture = Fixture::new("host", "outage");
    let config = fixture.config(vec![chat()]);
    let mut broken = config.clone();
    broken.connectors.program = fixture.root.join("missing").display().to_string();
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.9),
    );
    for _ in 0..5 {
        plugin
            .turns
            .lock()
            .unwrap()
            .push_back(read_then_propose("Yes."));
    }
    let state = fixture.plugin_state();

    for cycle in 1..=4 {
        let lines = run_plugin_on(
            &plugin,
            &fixture.host(&broken),
            &state,
            &AtomicBool::new(true),
        )
        .expect("an outage is no host error");
        assert!(lines.is_empty(), "cycle {cycle}: {lines:?}");
        let held = StateDir::open(&state, &config).unwrap();
        let current = held.load().unwrap();
        assert!(held.record().unwrap().is_empty(), "cycle {cycle}: no line");
        assert!(current.handled.is_empty(), "cycle {cycle}: not handled");
        assert!(
            current.failing.is_empty(),
            "cycle {cycle}: the outage counts against no item: {current:?}"
        );
        assert!(
            current.cursors.is_empty(),
            "cycle {cycle}: no cursor is saved past an item not handled"
        );
    }

    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    )
    .expect("the outage is over");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].outcome, RecordOutcome::Proposed, "{:?}", lines[0]);
    assert_eq!(plugin.classifier.requests().len(), 5);
}

/// A read that finds Connectors or the connection down, the connectors program timing out or a
/// `not_granted` refusal, counts against no item: the cycle ends there, saves no cursor, and the
/// item is handled once Connectors answers again.
#[test]
fn a_connectors_timeout_stops_no_item() {
    let fixture = Fixture::new("host", "connectors_timeout");
    let config = fixture.config(vec![chat()]);
    // Describes as the fake does; an invoke outlives the one-second bound and is stopped.
    let slow = fixture.root.join("slow-connectors");
    fs::write(
        &slow,
        "#!/bin/sh\ncase \" $* \" in *\" invoke \"*) exec sleep 5 ;; esac\nexec \"$(dirname \"$0\")/connectors\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(
        &slow,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();
    let mut timing_out = config.clone();
    timing_out.connectors.program = slow.display().to_string();
    timing_out.connectors.timeout_seconds = Some(1);
    let plugin = FakePlugin::new(
        vec![item("item-1")],
        Recorded::classifying("ask", &["chat"], 0.9),
    );
    for _ in 0..5 {
        plugin
            .turns
            .lock()
            .unwrap()
            .push_back(read_then_propose("Yes."));
    }
    let state = fixture.plugin_state();
    let unhandled = |label: &str| {
        let held = StateDir::open(&state, &config).unwrap();
        let current = held.load().unwrap();
        assert!(held.record().unwrap().is_empty(), "{label}: no line");
        assert!(current.handled.is_empty(), "{label}: not handled");
        assert!(
            current.failing.is_empty(),
            "{label}: counted against no item: {current:?}"
        );
        assert!(current.cursors.is_empty(), "{label}: no cursor saved");
    };

    for cycle in 1..=2 {
        let lines = run_plugin_on(
            &plugin,
            &fixture.host(&timing_out),
            &state,
            &AtomicBool::new(true),
        )
        .expect("a Connectors outage is no host error");
        assert!(lines.is_empty(), "timeout {cycle}: {lines:?}");
        unhandled(&format!("timeout {cycle}"));
    }
    assert_eq!(
        fixture.runs("invoke").len(),
        0,
        "the slow invokes never reached the fake"
    );

    fixture.answer("invoke", support::LIST, "invoke-not-granted.json");
    fixture.exit("invoke", 1);
    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    )
    .expect("a connection without its grant is no host error");
    assert!(lines.is_empty(), "not_granted: {lines:?}");
    unhandled("not_granted");
    assert_eq!(
        fixture.runs("invoke").len(),
        1,
        "the refused read was invoked once"
    );

    fs::remove_file(fixture.connectors_state().join("invoke.exit")).unwrap();
    fixture.answer("invoke", support::LIST, "invoke-read.json");
    let lines = run_plugin_on(
        &plugin,
        &fixture.host(&config),
        &state,
        &AtomicBool::new(true),
    )
    .expect("Connectors answers again");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].outcome, RecordOutcome::Proposed, "{:?}", lines[0]);
    assert_eq!(
        plugin.turn_models.load(Ordering::SeqCst),
        4,
        "one turn per cycle"
    );
}
