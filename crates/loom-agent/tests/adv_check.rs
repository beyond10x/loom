//! Adversary cases for `story:agent-check`: `b10x_loom_agent::check_text` driven from the
//! documents the unit wrote (`ess/domains/agent.yaml`'s `CheckAgentSpec`, the approval record's
//! D4, the fixtures under `ess/scenarios/agent-check/specs/`) on inputs its own tests do not
//! cover: the first binding, a lower-case method, refusal pairs whose order no test fixes, a
//! credential where the decoder answers first, and a password holding a URL delimiter.

use std::path::PathBuf;

use b10x_loom_agent::{check_text, describe};
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::{RefusalReason, SpecRefused};

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn operator() -> String {
    read("examples/agents/operator.yaml")
}

fn fixture(branch: &str) -> String {
    read(&format!("ess/scenarios/agent-check/specs/{branch}.yaml"))
}

fn plugin_actions(name: &str) -> Option<Vec<String>> {
    ProtocolCatalog::plugins()
        .expect("the plugin catalog")
        .get(name)
        .map(|entry| entry.actions())
}

/// `text` with `from` replaced by `to`, failing when `from` is not in it, so a fixture that
/// changes under the case cannot turn it into a no-op.
fn with(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the document holds `{from}`");
    text.replacen(from, to, 1)
}

fn refused(text: &str) -> SpecRefused {
    check_text(text, &plugin_actions).expect_err("the document is refused")
}

fn refusal(reason: RefusalReason, member: &str, detail: &str) -> SpecRefused {
    SpecRefused {
        reason,
        member: member.to_owned(),
        detail: detail.to_owned(),
    }
}

/// D4: "a string of the document" holds no credential, the first binding's included. Kills the
/// mutant that walks `bindings` from index 1 (every existing case puts the secret in binding 1
/// or 2).
#[test]
fn adv_a_connection_string_in_the_first_binding_is_refused() {
    let text = with(
        &operator(),
        "connection: ops",
        "connection: \"postgres://reader:hunter2@db.monitoring.svc/ops\"",
    );
    let found = refused(&text);
    assert_eq!(
        found,
        refusal(
            RefusalReason::SecretLiteral,
            "bindings[0].connection",
            "a URL carrying a user name and password"
        )
    );
    assert!(!describe(&found).contains("hunter2"));
}

/// A query in a nested optional member of a connectors binding.
#[test]
fn adv_a_query_in_a_connectors_operation_is_refused() {
    let text = with(
        &operator(),
        "search: conversations.replies",
        "search: \"https://chat.example/api/conversations.replies?token=hunter2\"",
    );
    let found = refused(&text);
    assert_eq!(
        found,
        refusal(
            RefusalReason::SecretLiteral,
            "bindings[0].search",
            "a URL carrying a query component"
        )
    );
    assert!(!describe(&found).contains("hunter2"));
}

/// `CheckAgentSpec`: "an http binding's method is not GET"; the brief: "not exactly `GET`". Kills
/// the mutant comparing case-insensitively.
#[test]
fn adv_a_lower_case_get_is_refused() {
    let text = with(
        &operator(),
        "allow_paths: [/api/v1/query, /api/v1/alerts], method: GET",
        "allow_paths: [/api/v1/query, /api/v1/alerts], method: get",
    );
    assert_eq!(
        refused(&text),
        refusal(RefusalReason::HttpMethod, "bindings[2].method", "get")
    );
}

/// Declaration order, the pair no existing case fixes: unknown-action before unbound-source.
#[test]
fn adv_an_unknown_action_is_answered_before_an_unbound_source() {
    let text = with(
        &fixture("unknown-action"),
        "sources: [logs, metrics, chat]",
        "sources: [logs, metrics, tickets]",
    );
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::UnknownAction,
            "states[0].actions[2]",
            "reply.escalate"
        )
    );
}

/// Declaration order, the other pair no existing case fixes: send-without-writes before
/// empty-channel-allow.
#[test]
fn adv_send_without_writes_is_answered_before_an_empty_channel_allow() {
    let text = with(
        &fixture("send-without-writes"),
        "allow: [C0000000001]",
        "allow: []",
    );
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::SendWithoutWrites,
            "policy.capabilities[2]",
            "reply.send"
        )
    );
}

/// Within unbound-source: a state, then the outer loop, then the channel.
#[test]
fn adv_a_state_source_is_answered_before_the_outer_loop_and_the_channel() {
    let text = with(
        &fixture("unbound-source"),
        "sources: [chat], min_age_minutes",
        "sources: [tickets], min_age_minutes",
    );
    let text = with(&text, "channel: {binding: chat", "channel: {binding: pager");
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::UnboundSource,
            "states[0].sources[2]",
            "tickets"
        )
    );
    let outer_first = with(
        &operator(),
        "sources: [chat], min_age_minutes",
        "sources: [tickets], min_age_minutes",
    );
    let outer_first = with(
        &outer_first,
        "channel: {binding: chat",
        "channel: {binding: pager",
    );
    assert_eq!(
        refused(&outer_first),
        refusal(
            RefusalReason::UnboundSource,
            "loops.outer.sources[0]",
            "tickets"
        )
    );
}

/// `writes.mode: none` with no `reply.send` capability is serveable: only the capability makes
/// writes necessary.
#[test]
fn adv_writes_none_without_reply_send_is_serveable() {
    let text = with(&operator(), "mode: auto", "mode: none");
    let text = with(
        &text,
        "capabilities: [datasource.read, reply.propose, reply.send]",
        "capabilities: [datasource.read, reply.propose]",
    );
    assert!(check_text(&text, &plugin_actions).is_ok());
}

/// `reply.send` is an action of `operator-reply@1`, not of `inbound-answer@1`: a state naming it
/// under the latter is refused, so the check reads the named protocol's actions only.
#[test]
fn adv_reply_send_under_inbound_answer_is_an_unknown_action() {
    let text = with(
        &operator(),
        "protocol: operator-reply@1",
        "protocol: inbound-answer@1",
    );
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::UnknownAction,
            "states[0].actions[2]",
            "reply.send"
        )
    );
    let text = with(
        &operator(),
        "protocol: operator-reply@1",
        "protocol: operator-reply@2",
    );
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::UnknownProtocol,
            "loops.inner.protocol",
            "operator-reply@2"
        )
    );
}

/// D4: "a URL with a user name and password". A password holding `#` is a password all the same;
/// the check cuts the authority at the first `#` and misses the `@` after it.
#[test]
fn adv_a_password_holding_a_hash_is_refused() {
    let text = with(
        &operator(),
        "base_url: http://logs.monitoring.svc:3100",
        "base_url: \"http://reader:hun#ter2@logs.monitoring.svc:3100\"",
    );
    let found = check_text(&text, &plugin_actions);
    assert!(
        matches!(&found, Err(refused) if refused.reason == RefusalReason::SecretLiteral),
        "a URL carrying a user name and password is served: {found:?}"
    );
}

/// The implementor's decision 5: a URL with a user name and no password is refused with its own
/// detail. No existing case holds it; kills the mutant that always says "and password".
#[test]
fn adv_a_user_name_without_a_password_has_its_own_detail() {
    let text = with(
        &operator(),
        "base_url: http://logs.monitoring.svc:3100",
        "base_url: \"http://reader@logs.monitoring.svc:3100\"",
    );
    assert_eq!(
        refused(&text),
        refusal(
            RefusalReason::SecretLiteral,
            "bindings[1].base_url",
            "a URL carrying a user name"
        )
    );
}

/// D4: "The refusal names the member and never repeats the value." A credential in a member the
/// decoder refuses (an enum) is refused as `Malformed`, and the decoder's message, which is the
/// refusal's detail, repeats it.
#[test]
fn adv_a_credential_in_an_enum_member_is_not_repeated() {
    let text = with(&operator(), "mode: auto", "mode: xoxb-hunter2");
    let found = refused(&text);
    assert!(
        !describe(&found).contains("hunter2"),
        "the refusal repeats the credential: {}",
        describe(&found)
    );
}
