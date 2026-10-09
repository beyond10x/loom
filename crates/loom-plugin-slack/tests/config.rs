//! Acceptance for `story:slack-plugin`, the configuration: a configuration the plugin cannot run
//! on is refused at start, naming what is missing, before any command runs.

use b10x_loom_plugin::PluginError;
use b10x_loom_plugin_slack::{SlackHandler, config};
use serde_json::{Value, json};

/// A whole configuration, every member present.
fn whole() -> Value {
    json!({
        "plugin": {
            "connectors": {"program": "connectors", "state_dir": null, "timeout_seconds": 30},
            "sources": [{"name": "docs", "adapter": "docs", "connection": "docs-fixture",
                         "search": "docs.search"}],
            "objectives": [{"name": "help people with cheap lookups", "weight": 0.9},
                           {"name": "learn about dev stack", "weight": "0.2"}],
            "classify_threshold": 0.5,
            "poll_interval_seconds": 300
        },
        "adapter": "slack",
        "connection": "slack-fixture",
        "list_channels": "conversations.list",
        "history": "conversations.history",
        "replies": "conversations.replies",
        "bot_user_id": "U0BOT",
        "min_age_minutes": 10,
        "seed": 7,
        "channels": [{"channel": "#c0fixture1", "objectives": ["help people with cheap lookups"]}]
    })
}

fn without(member: &str) -> Value {
    let mut config = whole();
    config.as_object_mut().unwrap().remove(member);
    config
}

fn with(member: &str, value: Value) -> Value {
    let mut config = whole();
    config[member] = value;
    config
}

/// The refusal of `config`, which must be a configuration error naming every one of `named`.
fn refused(config: &Value, named: &[&str]) {
    match config::parse(&config.to_string()) {
        Err(PluginError::Config(why)) => {
            for name in named {
                assert!(why.contains(name), "the refusal names `{name}`: {why}");
            }
        }
        other => panic!("{config} is refused, not {other:?}"),
    }
}

#[test]
fn a_whole_configuration_is_read() {
    let read = config::parse(&whole().to_string()).expect("the configuration is read");
    assert_eq!(read.bot_user_id, "U0BOT");
    assert_eq!(read.plugin.sources.len(), 1);
    assert_eq!(read.plugin.objectives[1].weight.0, "0.2");
    assert_eq!(read.lookback_minutes, None);
    assert!(read.plugin.workspace_roots.is_empty());
    SlackHandler::new(read).expect("the plugin starts");
}

#[test]
fn config_without_bot_id_is_refused() {
    refused(&without("bot_user_id"), &["bot_user_id", "missing"]);
    refused(&with("bot_user_id", json!("")), &["bot_user_id", "empty"]);
    let mut read = config::parse(&whole().to_string()).unwrap();
    read.bot_user_id = " ".to_owned();
    match SlackHandler::new(read) {
        Err(PluginError::Config(why)) => assert!(why.contains("bot_user_id"), "{why}"),
        Err(other) => panic!("a configuration error, not {other:?}"),
        Ok(_) => panic!("a plugin without a bot user id does not start"),
    }
}

#[test]
fn config_without_slack_source_is_refused() {
    for member in ["adapter", "connection"] {
        refused(&without(member), &[member, "missing"]);
        refused(&with(member, json!("")), &[member, "empty"]);
        refused(&with(member, json!(null)), &[member, "missing"]);
    }
    for member in ["list_channels", "history", "replies"] {
        refused(&with(member, json!("")), &[member, "empty"]);
    }
}

#[test]
fn a_misspelt_or_unusable_member_is_refused() {
    refused(
        &with("bot_user", json!("U0BOT")),
        &["bot_user", "not a member"],
    );
    refused(&with("min_age_minutes", json!(-1)), &["min_age_minutes"]);
    refused(&with("lookback_minutes", json!(0)), &["lookback_minutes"]);
    refused(&with("seed", json!("seven")), &["seed", "integer"]);
    refused(
        &with(
            "channels",
            json!([{"channel": "C0FIXTURE1", "objectives": ["undeclared"]}]),
        ),
        &["channels[0]", "undeclared"],
    );
    let mut nested = whole();
    nested["plugin"]["connectors"]["programme"] = json!("connectors");
    refused(&nested, &["plugin.connectors.programme"]);
    refused(&json!([]), &["object"]);
    match config::parse("{") {
        Err(PluginError::Config(why)) => assert!(why.contains("JSON"), "{why}"),
        other => panic!("refused, not {other:?}"),
    }
}
