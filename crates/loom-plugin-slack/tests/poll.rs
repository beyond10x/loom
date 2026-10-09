//! Acceptance for `story:slack-plugin`, the poll: which channels are walked, in what order, from
//! which stored `ts`, and which messages become items. The poll runs on the fake `connectors` of
//! `loom-connectors` answering Slack-shaped JSON, at a fixed clock; no model is asked.

mod support;

use b10x_loom_plugin::{Cursor, Objective, Plugin, PluginState, Poll};
use b10x_loom_plugin_slack::{DEFAULT_LOOKBACK_MINUTES, SlackConfig};
use serde_json::json;
use support::{
    BOT, CHANNEL, Fixture, HISTORY, LIST, NOW_SECONDS, REPLIES, ago, channels, config, fixture,
    handler, history, history_order, invokes, member, message, replies, serves,
};

fn empty() -> PluginState {
    PluginState {
        cursors: Vec::new(),
        handled: Vec::new(),
        failing: Vec::new(),
    }
}

/// One poll of a plugin over `config` on `fixture`, from `state`.
fn poll(fixture: &Fixture, config: SlackConfig, state: &PluginState) -> Poll {
    let weights: Vec<Objective> = config.plugin.objectives.clone();
    let plugin_config = config.plugin.clone();
    let plugin = handler(config);
    plugin
        .poll(&fixture.host(&plugin_config), state, &weights)
        .expect("the poll answers")
}

fn ids(poll: &Poll) -> Vec<String> {
    poll.items.iter().map(|item| item.id.0.clone()).collect()
}

fn item_id(channel: &str, ts: &str) -> String {
    format!("{channel}:{ts}")
}

#[test]
fn poll_lists_member_channels_only() {
    let fixture = fixture("member-channels");
    channels(
        &fixture,
        json!([
            member("C0FIXTURE1", 5),
            {"id": "C0OUTSIDE", "name": "outside", "is_channel": true, "is_member": false,
             "is_archived": false, "num_members": 50},
            {"id": "C0ARCHIVED", "name": "archived", "is_channel": true, "is_member": true,
             "is_archived": true, "num_members": 50},
            member("C0FIXTURE2", 3),
        ]),
    );
    history(
        &fixture,
        vec![message("U0ALICE", &ago(60, 1), "Is staging up?")],
    );

    let polled = poll(&fixture, config(&fixture), &empty());

    let mut read = history_order(&fixture);
    read.sort();
    assert_eq!(read, ["C0FIXTURE1", "C0FIXTURE2"], "only member channels");
    let calls = invokes(&fixture);
    assert_eq!(calls[0].0, LIST, "the channel list is read first");
    assert_eq!(calls[0].1["exclude_archived"], json!(true), "{calls:?}");
    let mut found = ids(&polled);
    found.sort();
    assert_eq!(
        found,
        [
            item_id("C0FIXTURE1", &ago(60, 1)),
            item_id("C0FIXTURE2", &ago(60, 1))
        ]
    );
}

/// Channels the weight, staleness and volume cannot tell apart are ordered by the seed: the same
/// seed twice gives the same order, and some other seed another one.
#[test]
fn walk_is_deterministic_per_seed() {
    let ids = [
        "C0FIXTURE1",
        "C0FIXTURE2",
        "C0FIXTURE3",
        "C0FIXTURE4",
        "C0FIXTURE5",
    ];
    let order = |label: &str, seed: i64| {
        let fixture = fixture(label);
        channels(
            &fixture,
            json!(ids.iter().map(|id| member(id, 8)).collect::<Vec<_>>()),
        );
        history(&fixture, Vec::new());
        let config = SlackConfig {
            seed,
            ..config(&fixture)
        };
        poll(&fixture, config, &empty());
        history_order(&fixture)
    };

    let first = order("seed-first", 7);
    let second = order("seed-second", 7);

    assert_eq!(first.len(), ids.len(), "every member channel is read");
    assert_eq!(first, second, "one seed, one order");
    let other = (8..40)
        .map(|seed| order(&format!("seed-{seed}"), seed))
        .find(|order| *order != first);
    assert!(
        other.is_some(),
        "another seed orders the channels otherwise"
    );
}

#[test]
fn weight_orders_the_walk() {
    for seed in 0..6 {
        let fixture = fixture(&format!("weight-{seed}"));
        channels(
            &fixture,
            json!([
                member("C0FIXTURE1", 8),
                member("C0FIXTURE2", 8),
                member("C0FIXTURE3", 8)
            ]),
        );
        history(&fixture, Vec::new());
        let config = SlackConfig {
            seed,
            channels: vec![
                serves("C0FIXTURE3", &["learn about dev stack"]),
                // By name, with the `#`: the channel list names it `c0fixture2`.
                serves("#c0fixture2", &["help people with cheap lookups"]),
            ],
            ..config(&fixture)
        };

        poll(&fixture, config, &empty());

        assert_eq!(
            history_order(&fixture),
            ["C0FIXTURE2", "C0FIXTURE3", "C0FIXTURE1"],
            "seed {seed}: the heaviest objective first, a channel serving none last"
        );
    }
}

#[test]
fn history_reads_from_the_stored_ts() {
    let fixture = fixture("stored-ts");
    channels(
        &fixture,
        json!([member("C0FIXTURE1", 8), member("C0FIXTURE2", 8)]),
    );
    let stored = ago(240, 7);
    let newer = ago(120, 1);
    history(&fixture, vec![message("U0ALICE", &newer, "Is staging up?")]);
    let state = PluginState {
        cursors: vec![Cursor {
            name: "C0FIXTURE1".to_owned(),
            value: stored.clone(),
        }],
        ..empty()
    };

    let polled = poll(&fixture, config(&fixture), &state);

    let oldest: Vec<(String, String)> = invokes(&fixture)
        .into_iter()
        .filter(|(operation, _)| operation == HISTORY)
        .map(|(_, input)| {
            (
                input["channel"].as_str().unwrap().to_owned(),
                input["oldest"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let lookback = format!("{}.000000", NOW_SECONDS - DEFAULT_LOOKBACK_MINUTES * 60);
    assert_eq!(
        oldest,
        [
            ("C0FIXTURE2".to_owned(), lookback),
            ("C0FIXTURE1".to_owned(), stored)
        ],
        "a channel never read is stalest and read back the lookback; a read one from its ts"
    );
    for channel in ["C0FIXTURE1", "C0FIXTURE2"] {
        assert!(
            polled.cursors.contains(&Cursor {
                name: channel.to_owned(),
                value: newer.clone()
            }),
            "{channel}'s cursor moves to its newest message: {:?}",
            polled.cursors
        );
    }
}

#[test]
fn young_message_is_not_an_item() {
    let fixture = fixture("young");
    channels(&fixture, json!([member(CHANNEL, 8)]));
    let old = ago(11, 1);
    let young = ago(9, 2);
    history(
        &fixture,
        vec![
            message("U0ALICE", &old, "Is staging up?"),
            message("U0CAROL", &young, "Is production up?"),
        ],
    );

    let polled = poll(&fixture, config(&fixture), &empty());

    assert_eq!(ids(&polled), [item_id(CHANNEL, &old)]);
    assert_eq!(
        polled.cursors,
        [Cursor {
            name: CHANNEL.to_owned(),
            value: old
        }],
        "the cursor stops before the young message, which the next poll reads again"
    );
}

#[test]
fn replied_message_is_not_an_item() {
    let asked = ago(60, 1);
    let thread = |label: &str, replier: &str| {
        let fixture = fixture(label);
        channels(&fixture, json!([member(CHANNEL, 8)]));
        let mut parent = message("U0ALICE", &asked, "Who owns the billing service?");
        parent["thread_ts"] = json!(asked);
        parent["reply_count"] = json!(1);
        history(&fixture, vec![parent.clone()]);
        let mut reply = message(replier, &ago(50, 2), "One more detail.");
        reply["thread_ts"] = json!(asked);
        replies(&fixture, vec![parent, reply]);
        let polled = poll(&fixture, config(&fixture), &empty());
        let thread_reads: Vec<_> = invokes(&fixture)
            .into_iter()
            .filter(|(operation, _)| operation == REPLIES)
            .map(|(_, input)| input)
            .collect();
        assert_eq!(thread_reads.len(), 1, "{label}: the thread is read once");
        assert_eq!(thread_reads[0]["channel"], json!(CHANNEL));
        assert_eq!(thread_reads[0]["ts"], json!(asked));
        ids(&polled)
    };

    assert!(
        thread("replied-by-other", "U0ERIN").is_empty(),
        "a reply by somebody else answers it"
    );
    assert_eq!(
        thread("replied-by-poster", "U0ALICE"),
        [item_id(CHANNEL, &asked)],
        "the poster's own reply does not"
    );
}

#[test]
fn bot_reacted_message_is_not_an_item() {
    let fixture = fixture("bot-reacted");
    channels(&fixture, json!([member(CHANNEL, 8)]));
    let mut seen = message("U0ALICE", &ago(60, 1), "Is staging up?");
    seen["reactions"] = json!([{"name": "eyes", "users": ["U0CAROL", BOT], "count": 2}]);
    let mut liked = message("U0ALICE", &ago(59, 2), "Is production up?");
    liked["reactions"] = json!([{"name": "eyes", "users": ["U0CAROL"], "count": 1}]);
    history(&fixture, vec![seen, liked]);

    let polled = poll(&fixture, config(&fixture), &empty());

    assert_eq!(ids(&polled), [item_id(CHANNEL, &ago(59, 2))]);
}

#[test]
fn bot_message_is_not_an_item() {
    let fixture = fixture("bot-message");
    channels(&fixture, json!([member(CHANNEL, 8)]));
    let mut integration = message("U0BUILDS", &ago(60, 1), "Build 42 passed.");
    integration["bot_id"] = json!("B0BUILDS");
    let integration_without_user = json!({"type": "message", "subtype": "bot_message",
        "bot_id": "B0OTHER", "username": "other", "text": "Deploy done?", "ts": ago(59, 2)});
    let own = message(BOT, &ago(58, 3), "I can look that up.");
    let person = message("U0ALICE", &ago(57, 4), "Is staging up?");
    history(
        &fixture,
        vec![integration, integration_without_user, own, person],
    );

    let polled = poll(&fixture, config(&fixture), &empty());

    assert_eq!(ids(&polled), [item_id(CHANNEL, &ago(57, 4))]);
}

#[test]
fn system_message_is_not_an_item() {
    let fixture = fixture("system-message");
    channels(&fixture, json!([member(CHANNEL, 8)]));
    let joined = json!({"type": "message", "subtype": "channel_join", "user": "U0CAROL",
        "text": "<@U0CAROL> has joined the channel", "ts": ago(60, 1)});
    let topic = json!({"type": "message", "subtype": "channel_topic", "user": "U0CAROL",
        "text": "set the channel topic: questions?", "ts": ago(59, 2)});
    let mut shared = message("U0ALICE", &ago(58, 3), "Is this log line an error?");
    shared["subtype"] = json!("file_share");
    history(&fixture, vec![joined, topic, shared]);

    let polled = poll(&fixture, config(&fixture), &empty());

    assert_eq!(
        ids(&polled),
        [item_id(CHANNEL, &ago(58, 3))],
        "a file shared with a question is still a person's message"
    );
}

#[test]
fn mention_comes_first() {
    let fixture = fixture("mention-first");
    channels(
        &fixture,
        json!([member("C0FIXTURE1", 8), member("C0FIXTURE2", 8)]),
    );
    history(
        &fixture,
        vec![
            message("U0ALICE", &ago(60, 1), "Is staging up?"),
            message(
                "U0CAROL",
                &ago(30, 2),
                &format!("<@{BOT}> where is the runbook?"),
            ),
            message(
                "U0DAVE",
                &ago(20, 3),
                &format!("<@{BOT}|bot> who owns billing?"),
            ),
        ],
    );
    let config = SlackConfig {
        channels: vec![serves("C0FIXTURE1", &["help people with cheap lookups"])],
        ..config(&fixture)
    };

    let polled = poll(&fixture, config, &empty());

    assert_eq!(
        ids(&polled),
        [
            item_id("C0FIXTURE1", &ago(30, 2)),
            item_id("C0FIXTURE1", &ago(20, 3)),
            item_id("C0FIXTURE2", &ago(30, 2)),
            item_id("C0FIXTURE2", &ago(20, 3)),
            item_id("C0FIXTURE1", &ago(60, 1)),
            item_id("C0FIXTURE2", &ago(60, 1)),
        ],
        "mentions first, then the rest; each in walk order, oldest first within a channel"
    );
    let mention = &polled.items[0];
    assert_eq!(mention.text, format!("<@{BOT}> where is the runbook?"));
    assert_eq!(mention.revision, ago(30, 2));
    let details = format!("{:?}", mention.details);
    for detail in ["C0FIXTURE1", "U0CAROL", "mention"] {
        assert!(details.contains(detail), "{details}");
    }
}
