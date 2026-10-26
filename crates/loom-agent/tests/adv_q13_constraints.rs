//! Adversary pass against `story:agent-spec-constraints-in-ess` (wave 2026-10-26-q13): the six
//! value refusals of `loom.agent.CheckAgentSpec` driven at their edges, against the unit's own
//! claims (`ess/domains/agent.yaml` `RefusalReason` and `CheckAgentSpec`, AGENTS.md § Agent spec,
//! the `loom_sdk::agent` module documentation) and against the reader of a run's deadline,
//! `crates/loom-commission/src/rfc3339.rs`, which the decoder says it agrees with.

use std::path::PathBuf;

use b10x_loom_agent::{check_text, describe};
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::{RefusalReason, SpecRefused};

fn root() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo")).join("../..")
}

fn operator_text() -> String {
    let path = root().join("examples/agents/operator.yaml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn plugin_actions(name: &str) -> Option<Vec<String>> {
    ProtocolCatalog::plugins()
        .expect("the plugin catalog")
        .get(name)
        .map(|entry| entry.actions())
}

fn with(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the document holds `{from}`");
    text.replacen(from, to, 1)
}

fn outcome(text: &str) -> Result<(), (RefusalReason, String)> {
    check_text(text, &plugin_actions)
        .map(|_| ())
        .map_err(|SpecRefused { reason, member, .. }| (reason, member))
}

const BUDGET: &str = "budget: {max_steps: 20, max_tokens: 400000}";

/// The operator example with `session.budget.deadline` set to `yaml`, a YAML scalar as written.
fn with_deadline(yaml: &str) -> String {
    with(
        &operator_text(),
        BUDGET,
        &format!("budget: {{max_steps: 20, max_tokens: 400000, deadline: {yaml}}}"),
    )
}

/// `ess/domains/agent.yaml` `RefusalReason`: "a `Timestamp` member is not an RFC 3339 instant
/// (`NotAnInstant`)"; `loom_sdk::agent::timestamp`: "a full time ... and `Z` (or `z`) or a numeric
/// offset, the instant Commission reads a run's deadline as". RFC 3339's `time-numoffset` is
/// `("+" / "-") time-hour ":" time-minute`, the ASCII hyphen-minus; Commission's reader
/// (`rfc3339::offset`) takes `+`, `-`, `Z` or `z` and nothing else. chrono's
/// `parse_from_rfc3339` also takes U+2212 MINUS SIGN there, so the decoder admits a deadline
/// Commission answers `InvalidDeadline` for.
#[test]
fn adv_a_deadline_whose_offset_sign_is_a_unicode_minus_is_not_an_instant() {
    let text = with_deadline("\"2026-12-31T23:59:59\u{2212}05:00\"");
    assert_eq!(
        outcome(&text),
        Err((
            RefusalReason::NotAnInstant,
            "session.budget.deadline".to_owned()
        )),
        "an offset signed with U+2212 is no RFC 3339 instant"
    );
}

/// The deadlines Commission's reader (`crates/loom-commission/src/rfc3339.rs`) takes are the ones
/// the decoder takes: a leap second at any minute, year 0000, a fraction longer than nine digits,
/// `-00:00` and `+23:59`; and the ones it refuses are refused, as `NotAnInstant` at the member.
#[test]
fn adv_the_deadline_edges_are_the_ones_commission_reads() {
    for accepted in [
        "\"2026-12-31T23:59:60Z\"",
        "\"2026-06-30T12:30:60+02:00\"",
        "\"0000-01-01T00:00:00Z\"",
        "\"9999-12-31T23:59:59.123456789012Z\"",
        "\"2026-12-31T23:59:59-00:00\"",
        "\"2026-12-31T23:59:59+23:59\"",
        "\"2024-02-29t00:00:00z\"",
    ] {
        assert_eq!(outcome(&with_deadline(accepted)), Ok(()), "{accepted}");
    }
    for refused in [
        "\"2026-12-31T23:59:59.Z\"",
        "\"2026-12-31T23:59:59+24:00\"",
        "\"2026-12-31T23:59:59+0000\"",
        "\"2026-12-31T23:59:59+00\"",
        "\"2026-12-31T23:59:59+00:60\"",
        "\"2026-12-31T23:59:59Z \"",
        "\" 2026-12-31T23:59:59Z\"",
        "\"2026-12-31T23:59:59Z\\n\"",
        "\"2026-12-31T23:59:61Z\"",
        "\"2026-12-31T23:60:00Z\"",
        "\"2026-13-01T00:00:00Z\"",
        "\"2025-02-29T00:00:00Z\"",
        "\"2026-12-31T23:59:59,5Z\"",
        "\"2026-12-31T23:59:59ZZ\"",
        "\"+2026-12-31T23:59:59Z\"",
        "\"12026-12-31T23:59:59Z\"",
        "\"2026-12-31_23:59:59Z\"",
        "\"\u{ff12}026-12-31T23:59:59Z\"",
    ] {
        assert_eq!(
            outcome(&with_deadline(refused)),
            Err((
                RefusalReason::NotAnInstant,
                "session.budget.deadline".to_owned()
            )),
            "{refused}"
        );
    }
    assert_eq!(
        outcome(&with_deadline("\"   \"")),
        Err((
            RefusalReason::BlankText,
            "session.budget.deadline".to_owned()
        ))
    );
}

/// AGENTS.md § Agent spec, the `loom_sdk::agent` and `b10x_loom_agent` module documentation and
/// the `CheckAgentSpec` comment in `ess/domains/agent.yaml`: the walk reads "in each mapping its
/// undeclared members, then its members in the order the domain declares them". A binding mapping
/// with an undeclared member answers that member (`Malformed` at `document`) before any value of
/// its own; the decoder reads the binding's `kind` before it looks for undeclared members.
#[test]
fn adv_a_binding_answers_its_undeclared_member_before_a_blank_kind() {
    let text = with(
        &operator_text(),
        "{name: chat, kind: connectors,",
        "{name: chat, kind: ' ', zzz: 1,",
    );
    assert_eq!(
        outcome(&text),
        Err((RefusalReason::Malformed, "document".to_owned())),
        "the undeclared `bindings[0].zzz` comes first in its mapping"
    );
}

/// The story's table: `PortOutOfRange` is "a port outside 1 to 65535". An integer beyond `i64`
/// is outside that range and is an integer, not a value of the wrong type.
#[test]
fn adv_a_port_beyond_i64_is_out_of_range() {
    for port in ["9223372036854775808", "18446744073709551615"] {
        let text = with(
            &operator_text(),
            "a2a: {port: 8080,",
            &format!("a2a: {{port: {port},"),
        );
        assert_eq!(
            outcome(&text),
            Err((RefusalReason::PortOutOfRange, "expose.a2a.port".to_owned())),
            "{port}"
        );
    }
}

/// `bearer_env` edges: `=`, a non-ASCII letter, a NUL, a leading hyphen and a bare digit are no
/// variable name, and the detail never repeats the text; an ideographic space alone is blank.
#[test]
fn adv_bearer_env_edges() {
    let place = |name: &str| {
        with(
            &operator_text(),
            "/api/v1/alerts], method: GET,",
            &format!("/api/v1/alerts], method: GET, bearer_env: {name},"),
        )
    };
    for (name, needle) in [
        ("\"A=B\"", "A=B"),
        ("\"\u{e9}T\"", "\u{e9}T"),
        ("\"TOK\\0EN\"", "TOK"),
        ("\"-TOKEN\"", "-TOKEN"),
        ("\"9\"", "9"),
        ("\"TOKEN \"", "TOKEN "),
    ] {
        let found = check_text(&place(name), &plugin_actions).expect_err(name);
        assert_eq!(
            (found.reason, found.member.as_str()),
            (RefusalReason::NotAVariableName, "bindings[2].bearer_env"),
            "{name}"
        );
        let shown = describe(&found);
        let shown = shown.replace("bindings[2]", "");
        assert!(!shown.contains(needle), "{name}: {shown}");
    }
    assert_eq!(
        outcome(&place("\"\u{3000}\"")),
        Err((
            RefusalReason::BlankText,
            "bindings[2].bearer_env".to_owned()
        ))
    );
    for name in ["_", "a", "lower_case_1"] {
        assert_eq!(outcome(&place(name)), Ok(()), "{name}");
    }
}

/// Port edges the suite does not name: a hexadecimal 65536 is out of range, a port that is a
/// string or a float is a value of the wrong type (`Malformed`).
#[test]
fn adv_port_edges() {
    let place = |port: &str| {
        with(
            &operator_text(),
            "a2a: {port: 8080,",
            &format!("a2a: {{port: {port},"),
        )
    };
    assert_eq!(
        outcome(&place("0x10000")),
        Err((RefusalReason::PortOutOfRange, "expose.a2a.port".to_owned()))
    );
    for wrong in ["'8080'", "8080.0", "1e3"] {
        assert_eq!(
            outcome(&place(wrong)),
            Err((RefusalReason::Malformed, "document".to_owned())),
            "{wrong}"
        );
    }
}

/// Allow-path edges the suite does not name: a bare `.` or `..`, a trailing `/.`, an upper-case
/// encoded dot; an empty segment is a plain path.
#[test]
fn adv_allow_path_edges() {
    let place = |path: &str| {
        with(
            &operator_text(),
            "allow_paths: [/api/v1/query, /api/v1/alerts]",
            &format!("allow_paths: [/api/v1/query, {path}]"),
        )
    };
    for path in ["'.'", "'..'", "/api/.", "/%2E%2E/x", "\"/a\\u0000b\""] {
        assert_eq!(
            outcome(&place(path)),
            Err((
                RefusalReason::NotAPlainPath,
                "bindings[2].allow_paths[1]".to_owned()
            )),
            "{path}"
        );
    }
    assert_eq!(
        outcome(&place("''")),
        Err((
            RefusalReason::BlankText,
            "bindings[2].allow_paths[1]".to_owned()
        ))
    );
}
