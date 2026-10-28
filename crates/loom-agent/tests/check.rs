//! `b10x_loom_agent::check` says whether a `loom.agent/1` document can be served, before anything
//! runs: the first refusal, in the declaration order of `loom.agent.CheckAgentSpec`'s outcomes,
//! names the member and the detail; a document none of them claims is serveable.
//!
//! The fixtures are the ones the authored ESS scenarios name
//! (`ess/scenarios/agent-check/specs/`), each the operator example with one defect, and the
//! member and detail each test expects are the ones its scenario fixes. The protocols are the
//! plugin catalog's.

use std::path::PathBuf;

use b10x_loom_agent::{Checked, check, check_text, describe};
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::{
    AgentSpec, ChannelSpec, RefusalReason, SourceName, SpecRefused, WriteMode, parse,
};

/// The repository root, read when the test runs.
fn root() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo")).join("../..")
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn operator_text() -> String {
    read("examples/agents/operator.yaml")
}

fn fixture(branch: &str) -> String {
    read(&format!("ess/scenarios/agent-check/specs/{branch}.yaml"))
}

fn operator() -> AgentSpec {
    parse(&operator_text()).expect("the operator example decodes")
}

/// The action ids of a plugin-catalog protocol, by its registration.
fn plugin_actions(name: &str) -> Option<Vec<String>> {
    ProtocolCatalog::plugins()
        .expect("the plugin catalog")
        .get(name)
        .map(|entry| entry.actions())
}

fn refusal(reason: RefusalReason, member: &str, detail: &str) -> SpecRefused {
    SpecRefused {
        reason,
        member: member.to_owned(),
        detail: detail.to_owned(),
    }
}

/// `text` with `from` replaced once by `to`, failing when `from` is not in it, so a fixture that
/// changes under a case cannot turn it into a no-op.
fn with(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the document holds `{from}`");
    text.replacen(from, to, 1)
}

fn refused_text(text: &str) -> SpecRefused {
    check_text(text, &plugin_actions).expect_err("the document is refused")
}

fn refused(spec: &AgentSpec) -> SpecRefused {
    check(spec, &plugin_actions).expect_err("the spec is refused")
}

#[test]
fn a_valid_spec_checks() {
    let checked = check_text(&operator_text(), &plugin_actions).expect("the operator example");
    assert_eq!(
        checked,
        Checked {
            agent: "platform-operator".to_owned(),
            protocol: Some("operator-reply@1".to_owned()),
            states: 3,
            bindings: 3,
        }
    );
}

#[test]
fn a_malformed_document_is_refused() {
    let text = fixture("malformed");
    let decoder = parse(&text)
        .expect_err("the decoder refuses it")
        .to_string();
    assert_eq!(
        refused_text(&text),
        refusal(RefusalReason::Malformed, "document", &decoder)
    );
    assert!(decoder.contains("agent.team"), "{decoder}");
}

#[test]
fn a_secret_literal_is_refused() {
    let refusal_found = refused_text(&fixture("secret-literal"));
    assert_eq!(
        refusal_found,
        refusal(
            RefusalReason::SecretLiteral,
            "bindings[1].base_url",
            "a URL carrying a user name and password"
        )
    );
    for shown in [
        refusal_found.detail.clone(),
        refusal_found.member.clone(),
        describe(&refusal_found),
    ] {
        assert!(
            !shown.contains("hunter2"),
            "the password is repeated: {shown}"
        );
        assert!(
            !shown.contains("reader"),
            "the user name is repeated: {shown}"
        );
    }
}

#[test]
fn an_unknown_protocol_is_refused() {
    assert_eq!(
        refused_text(&fixture("unknown-protocol")),
        refusal(
            RefusalReason::UnknownProtocol,
            "loops.inner.protocol",
            "operator-escalate@1"
        )
    );
}

#[test]
fn a_state_naming_an_unknown_action_is_refused() {
    assert_eq!(
        refused_text(&fixture("unknown-action")),
        refusal(
            RefusalReason::UnknownAction,
            "states[0].actions[2]",
            "reply.escalate"
        )
    );
}

#[test]
fn a_source_without_a_binding_is_refused() {
    assert_eq!(
        refused_text(&fixture("unbound-source")),
        refusal(
            RefusalReason::UnboundSource,
            "states[0].sources[2]",
            "tickets"
        )
    );
}

#[test]
fn an_http_binding_whose_method_is_not_get_is_refused() {
    assert_eq!(
        refused_text(&fixture("http-method")),
        refusal(RefusalReason::HttpMethod, "bindings[2].method", "POST")
    );
}

/// An allowed path no read may use (`ess/domains/agent.yaml` `HttpBinding`) is refused when the
/// document is read, naming the entry, rather than left for `path_allowed` to refuse at every read;
/// it answers `NotAPlainPath` at the entry, and the ESS scenario
/// `an-allow-path-holding-a-dot-dot-segment-is-refused` fixes the first.
#[test]
fn an_allow_path_no_read_may_use_is_refused() {
    let detail = |at: &str| {
        format!(
            "`{at}` is not a plain path, one that starts with `/` and holds no `\\`, no `.` or `..` \
             segment, no encoded separator or dot, no `?` or `#` and no whitespace"
        )
    };
    assert_eq!(
        refused_text(&fixture("not-a-plain-path")),
        refusal(
            RefusalReason::NotAPlainPath,
            "bindings[2].allow_paths[1]",
            &detail("bindings[2].allow_paths[1]")
        )
    );
    for path in [
        "\"/api\\\\..\\\\secret\"",
        "\"/api\\\\v1\"",
        "/api/../secret",
        "/api/./v1",
        "/api/v1/..",
        "/api/%2e%2e/secret",
        "/api/%2Fsecret",
        "\"/api/v1?token=x\"",
        "\"/api/v1#x\"",
        "\"/api/v 1\"",
        "api/v1",
    ] {
        let text = with(
            &operator_text(),
            "allow_paths: [/api/v1/query, /api/v1/alerts]",
            &format!("allow_paths: [/api/v1/query, {path}]"),
        );
        assert_eq!(
            refused_text(&text),
            refusal(
                RefusalReason::NotAPlainPath,
                "bindings[2].allow_paths[1]",
                &detail("bindings[2].allow_paths[1]")
            ),
            "{path}"
        );
    }
    for path in ["/", "/api/v1/", "/api/v1/query_range", "/api/v1.2/a..b"] {
        let text = with(
            &operator_text(),
            "allow_paths: [/api/v1/query, /api/v1/alerts]",
            &format!("allow_paths: [/api/v1/query, {path}]"),
        );
        assert!(check_text(&text, &plugin_actions).is_ok(), "{path}");
    }
}

#[test]
fn an_unknown_format_is_refused() {
    assert_eq!(
        refused_text(&fixture("unknown-format")),
        refusal(
            RefusalReason::UnknownFormat,
            "format",
            "`format` is `loom.agent/2`, not `loom.agent/1`"
        )
    );
}

/// Empty and only-whitespace text are both blank, wherever the text member stands: an identity,
/// an enum spelling, a source name, a binding's kind.
#[test]
fn a_blank_text_member_is_refused() {
    assert_eq!(
        refused_text(&fixture("blank-text")),
        refusal(
            RefusalReason::BlankText,
            "agent.name",
            "`agent.name` is empty"
        )
    );
    for (from, to, member) in [
        ("name: Platform operator", "name: ''", "agent.name"),
        (
            "decisions: ask",
            "decisions: \" \"",
            "session.tools.decisions",
        ),
        (
            "sources: [chat]",
            "sources: [\"\\t\"]",
            "loops.outer.sources[0]",
        ),
        (
            "{name: chat, kind: connectors",
            "{name: chat, kind: ' '",
            "bindings[0].kind",
        ),
    ] {
        let found = refused_text(&with(&operator_text(), from, to));
        assert_eq!(
            found,
            refusal(
                RefusalReason::BlankText,
                member,
                &format!("`{member}` is empty")
            ),
            "{to}"
        );
    }
}

/// The detail names the member and never the text, which may be a secret written in the wrong
/// place.
#[test]
fn a_bearer_env_that_is_not_a_variable_name_is_refused() {
    let found = refused_text(&fixture("not-a-variable-name"));
    assert_eq!(
        found,
        refusal(
            RefusalReason::NotAVariableName,
            "bindings[2].bearer_env",
            "`bindings[2].bearer_env` is not an environment variable name"
        )
    );
    assert!(
        !describe(&found).contains("METRICS-TOKEN"),
        "{}",
        describe(&found)
    );
    for name in ["1TOKEN", "TOKEN VALUE", "tok.en", "\"$TOKEN\""] {
        let text = with(
            &operator_text(),
            "/api/v1/alerts], method: GET,",
            &format!("/api/v1/alerts], method: GET, bearer_env: {name},"),
        );
        let found = refused_text(&text);
        assert_eq!(
            (found.reason, found.member.as_str()),
            (RefusalReason::NotAVariableName, "bindings[2].bearer_env"),
            "{name}"
        );
        assert!(
            !describe(&found).contains(name.trim_matches('"')),
            "{}",
            describe(&found)
        );
    }
    let text = with(
        &operator_text(),
        "/api/v1/alerts], method: GET,",
        "/api/v1/alerts], method: GET, bearer_env: _METRICS_TOKEN2,",
    );
    assert!(check_text(&text, &plugin_actions).is_ok());
}

/// A port is 1 to 65535 wherever it stands: 0 and 65536 are refused, 1 and 65535 check.
#[test]
fn a_port_outside_the_range_is_refused() {
    let detail = |member: &str| format!("`{member}` is not a port from 1 to 65535");
    for branch in ["port-out-of-range", "port-out-of-range-65536"] {
        assert_eq!(
            refused_text(&fixture(branch)),
            refusal(
                RefusalReason::PortOutOfRange,
                "expose.a2a.port",
                &detail("expose.a2a.port")
            ),
            "{branch}"
        );
    }
    for (from, member) in [
        (
            "host: logs.example, port: 3100",
            "session.tools.egress[0].port",
        ),
        (
            "host: chat.example, port: 443",
            "deploy.kubernetes.egress[0].port",
        ),
        (
            "from_namespace: operator-clients, port: 8080",
            "deploy.kubernetes.ingress[0].port",
        ),
    ] {
        let (head, _) = from.rsplit_once(' ').expect("a port");
        for port in ["0", "-1", "65536"] {
            let text = with(&operator_text(), from, &format!("{head} {port}"));
            assert_eq!(
                refused_text(&text),
                refusal(RefusalReason::PortOutOfRange, member, &detail(member)),
                "{member} {port}"
            );
        }
        for port in ["1", "65535"] {
            let text = with(&operator_text(), from, &format!("{head} {port}"));
            assert!(
                check_text(&text, &plugin_actions).is_ok(),
                "{member} {port}"
            );
        }
    }
}

/// Every port member of the operator example: the text that sets it and the member it stands at.
const PORT_MEMBERS: [(&str, &str); 4] = [
    ("a2a: {port: 8080", "expose.a2a.port"),
    (
        "host: logs.example, port: 3100",
        "session.tools.egress[0].port",
    ),
    (
        "host: chat.example, port: 443",
        "deploy.kubernetes.egress[0].port",
    ),
    (
        "from_namespace: operator-clients, port: 8080",
        "deploy.kubernetes.ingress[0].port",
    ),
];

/// An integer the YAML reader holds outside `i64`, up to `u64::MAX`, is a port out of range at
/// every port member, and so is `i64::MIN`. Elsewhere an integer outside `i64` is no integer the
/// domain holds (`Malformed`), and so is one in the models section.
#[test]
fn an_integer_outside_i64_is_a_port_out_of_range() {
    for (from, member) in PORT_MEMBERS {
        let (head, _) = from.rsplit_once(' ').expect("a port");
        for port in [
            "9223372036854775808",
            "18446744073709551615",
            "0xFFFFFFFFFFFFFFFF",
            "-9223372036854775808",
        ] {
            let text = with(&operator_text(), from, &format!("{head} {port}"));
            let found = refused_text(&text);
            assert_eq!(
                (found.reason, found.member.as_str()),
                (RefusalReason::PortOutOfRange, member),
                "{member} {port}"
            );
        }
    }
    for (from, to) in [
        (
            "method: GET, max_bytes: 262144}\n  - {name: metrics",
            "method: GET, max_bytes: 9223372036854775808}\n  - {name: metrics",
        ),
        (
            "method: GET, max_bytes: 262144}\n  - {name: metrics",
            "method: GET, max_bytes: 18446744073709551616}\n  - {name: metrics",
        ),
        ("name: Platform operator", "name: 18446744073709551616"),
        ("routes: routes.yaml", "routes: 18446744073709551616"),
        (
            "{poll_interval_seconds: 60,",
            "{poll_interval_seconds: -9223372036854775809,",
        ),
    ] {
        let found = refused_text(&with(&operator_text(), from, to));
        assert_eq!(
            (found.reason, found.member.as_str()),
            (RefusalReason::Malformed, "document"),
            "{to}"
        );
    }
}

/// An integer past `u64::MAX` or below `i64::MIN` is one the YAML reader does not hold: the
/// document is refused as a whole, `Malformed` at `document`, wherever a port holds it.
#[test]
fn a_port_past_u64_is_malformed() {
    for (from, _) in PORT_MEMBERS {
        let (head, _) = from.rsplit_once(' ').expect("a port");
        for port in [
            "18446744073709551616",
            "0x10000000000000000",
            "-9223372036854775809",
        ] {
            let text = with(&operator_text(), from, &format!("{head} {port}"));
            let found = refused_text(&text);
            assert_eq!(
                (found.reason, found.member.as_str()),
                (RefusalReason::Malformed, "document"),
                "{from} {port}"
            );
        }
    }
}

/// `session.budget.deadline` is a `Timestamp`, an RFC 3339 instant (a date, `T`, a time and an
/// offset), as Commission reads a run's deadline: a date alone, a time without an offset, a day
/// the calendar lacks and a space for the `T` are refused; a full instant checks.
#[test]
fn a_deadline_that_is_not_an_instant_is_refused() {
    assert_eq!(
        refused_text(&fixture("not-an-instant")),
        refusal(
            RefusalReason::NotAnInstant,
            "session.budget.deadline",
            "`session.budget.deadline` is not an RFC 3339 instant"
        )
    );
    assert!(check_text(&fixture("deadline-instant"), &plugin_actions).is_ok());
    let budget = "budget: {max_steps: 20, max_tokens: 400000}";
    for deadline in [
        "2026-12-31",
        "2026-12-31T23:59:59",
        "2026-02-30T00:00:00Z",
        "2026-12-31T24:00:00Z",
        "\"2026-12-31 23:59:59Z\"",
        "26-12-31T23:59:59Z",
        "tomorrow",
    ] {
        let text = with(
            &operator_text(),
            budget,
            &format!("budget: {{max_steps: 20, max_tokens: 400000, deadline: {deadline}}}"),
        );
        let found = refused_text(&text);
        assert_eq!(
            (found.reason, found.member.as_str()),
            (RefusalReason::NotAnInstant, "session.budget.deadline"),
            "{deadline}"
        );
    }
    for deadline in [
        "2026-12-31T23:59:59Z",
        "2026-12-31t23:59:59z",
        "2028-02-29T12:00:00.25+02:00",
        "2026-12-31T23:59:59-05:30",
    ] {
        let text = with(
            &operator_text(),
            budget,
            &format!("budget: {{max_steps: 20, max_tokens: 400000, deadline: {deadline}}}"),
        );
        assert!(check_text(&text, &plugin_actions).is_ok(), "{deadline}");
    }
}

/// The decoder's refusals, `Malformed` and the value refusals, come from one walk: the first
/// member it refuses decides which answers. A YAML tag anywhere is refused before that walk, and
/// a credential before the decoder.
#[test]
fn the_first_member_the_decoder_refuses_decides_its_refusal() {
    let text = fixture("port-out-of-range").replace("name: Platform operator", "name: ' '");
    assert_eq!(refused_text(&text).reason, RefusalReason::BlankText);

    let text = fixture("blank-text").replace("{mode: auto,", "{zzz: 1, mode: auto,");
    assert_eq!(refused_text(&text).reason, RefusalReason::BlankText);

    let text = fixture("malformed").replace("a2a: {port: 8080,", "a2a: {port: 0,");
    assert_eq!(
        (refused_text(&text).reason, refused_text(&text).member),
        (RefusalReason::Malformed, "document".to_owned())
    );

    let text = fixture("unknown-format").replace("name: Platform operator", "name: ' '");
    assert_eq!(refused_text(&text).reason, RefusalReason::UnknownFormat);

    let text = fixture("not-an-instant").replace(
        "method: GET, max_bytes",
        "method: GET, bearer_env: A-B, max_bytes",
    );
    assert_eq!(refused_text(&text).reason, RefusalReason::NotAnInstant);

    let text = fixture("not-a-variable-name").replace("cpu: 250m", "cpu: !Quantity 250m");
    assert_eq!(refused_text(&text).reason, RefusalReason::Malformed);

    let text = fixture("not-a-variable-name").replace(
        "http://logs.monitoring.svc:3100",
        "http://reader:hunter2@logs.monitoring.svc:3100",
    );
    assert_eq!(refused_text(&text).reason, RefusalReason::SecretLiteral);

    let text = fixture("not-a-plain-path").replace("allow: [C0000000001]", "allow: []");
    assert_eq!(refused_text(&text).reason, RefusalReason::NotAPlainPath);
}

/// A binding mapping keeps the walk's order although its `kind` decides which members it
/// declares: a member neither kind declares answers before `kind` is read, then `kind`, then a
/// member only the other kind declares, then the binding's members in the order its kind declares
/// them.
#[test]
fn a_binding_reads_its_undeclared_members_before_its_kind() {
    let chat = "{name: chat, kind: connectors,";
    for (to, expected) in [
        (
            "{name: chat, kind: ' ', zzz: 1,",
            (RefusalReason::Malformed, "document"),
        ),
        (
            "{name: chat, zzz: 1,",
            (RefusalReason::Malformed, "document"),
        ),
        (
            "{name: chat, kind: ftp, zzz: 1,",
            (RefusalReason::Malformed, "document"),
        ),
        (
            "{name: chat, kind: ' ', base_url: ' ',",
            (RefusalReason::BlankText, "bindings[0].kind"),
        ),
        (
            "{name: ' ', kind: connectors, base_url: http://chat.example,",
            (RefusalReason::Malformed, "document"),
        ),
        (
            "{name: ' ', kind: connectors,",
            (RefusalReason::BlankText, "bindings[0].name"),
        ),
    ] {
        let found = refused_text(&with(&operator_text(), chat, to));
        assert_eq!((found.reason, found.member.as_str()), expected, "{to}");
    }
    let found = refused_text(&with(
        &operator_text(),
        "{name: logs, kind: http,",
        "{name: ' ', kind: http, adapter: chat,",
    ));
    assert_eq!(
        (found.reason, found.member.as_str()),
        (RefusalReason::Malformed, "document")
    );
}

#[test]
fn reply_send_without_writes_is_refused() {
    assert_eq!(
        refused_text(&fixture("send-without-writes")),
        refusal(
            RefusalReason::SendWithoutWrites,
            "policy.capabilities[2]",
            "reply.send"
        )
    );
}

#[test]
fn an_empty_channel_allow_is_refused() {
    assert_eq!(
        refused_text(&fixture("empty-channel-allow")),
        refusal(
            RefusalReason::EmptyChannelAllow,
            "channel.allow",
            "no channel id is allowed"
        )
    );
}

#[test]
fn a_spec_with_no_loops_and_no_states_is_serveable() {
    let mut spec = operator();
    spec.loops = None;
    spec.states.clear();
    assert_eq!(
        check(&spec, &plugin_actions),
        Ok(Checked {
            agent: "platform-operator".to_owned(),
            protocol: None,
            states: 0,
            bindings: 3,
        })
    );
}

#[test]
fn a_spec_with_no_channel_is_serveable() {
    let mut spec = operator();
    spec.channel = None;
    assert!(check(&spec, &plugin_actions).is_ok());
}

#[test]
fn states_with_no_inner_loop_are_refused() {
    let mut spec = operator();
    spec.loops = None;
    assert_eq!(
        refused(&spec),
        refusal(
            RefusalReason::UnknownProtocol,
            "states",
            "no inner loop names a protocol to check the states against"
        )
    );
}

#[test]
fn a_source_of_the_outer_loop_or_the_channel_without_a_binding_is_refused() {
    let mut spec = operator();
    spec.loops.as_mut().expect("loops").outer.sources[0] = SourceName("tickets".into());
    assert_eq!(
        refused(&spec),
        refusal(
            RefusalReason::UnboundSource,
            "loops.outer.sources[0]",
            "tickets"
        )
    );
    let mut spec = operator();
    spec.channel = Some(ChannelSpec {
        binding: SourceName("tickets".into()),
        allow: vec!["C0000000001".into()],
        bot_user_id: "U0000000001".into(),
    });
    assert_eq!(
        refused(&spec),
        refusal(RefusalReason::UnboundSource, "channel.binding", "tickets")
    );
}

#[test]
fn reply_send_with_no_writes_declared_is_refused() {
    let mut spec = operator();
    spec.writes = None;
    assert_eq!(
        refused(&spec),
        refusal(
            RefusalReason::SendWithoutWrites,
            "policy.capabilities[2]",
            "reply.send"
        )
    );
    let mut spec = operator();
    spec.writes.as_mut().expect("writes").mode = WriteMode::Approval;
    assert!(check(&spec, &plugin_actions).is_ok());
}

/// A document with two defects answers the one whose outcome `CheckAgentSpec` declares first.
#[test]
fn the_first_refusal_in_declaration_order_is_the_answer() {
    let text = fixture("unknown-action").replace("allow: [C0000000001]", "allow: []");
    assert_eq!(refused_text(&text).reason, RefusalReason::UnknownAction);

    let text = fixture("unknown-protocol").replace(
        "http://logs.monitoring.svc:3100",
        "http://reader:hunter2@logs.monitoring.svc:3100",
    );
    assert_eq!(refused_text(&text).reason, RefusalReason::SecretLiteral);

    let text = fixture("http-method").replace("mode: auto", "mode: none");
    assert_eq!(refused_text(&text).reason, RefusalReason::HttpMethod);

    let text = fixture("malformed").replace("method: GET", "method: POST");
    assert_eq!(refused_text(&text).reason, RefusalReason::Malformed);

    let text = fixture("unbound-source").replace("method: GET", "method: PUT");
    assert_eq!(refused_text(&text).reason, RefusalReason::UnboundSource);
}

/// Every shape a credential takes is refused wherever it stands, naming the member and the shape,
/// never the value.
#[test]
fn every_credential_shape_is_refused_without_repeating_it() {
    let secret = "s3cr3tvalue";
    let prefixes = [
        "xoxb-",
        "xoxp-",
        "xapp-",
        "sk-",
        "sk_live_",
        "sk_test_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "AKIA",
        "Bearer ",
        "-----BEGIN",
    ];
    for prefix in prefixes {
        let text = with(
            &operator_text(),
            "bot_user_id: U0000000001",
            &format!("bot_user_id: \"{prefix}{secret}\""),
        );
        let found = refused_text(&text);
        assert_eq!(
            found,
            refusal(
                RefusalReason::SecretLiteral,
                "channel.bot_user_id",
                &format!("a value with the prefix `{prefix}`")
            )
        );
        assert!(!describe(&found).contains(secret), "{}", describe(&found));
    }

    let text = with(
        &operator_text(),
        "base_url: http://metrics.monitoring.svc:9090",
        &format!("base_url: \"http://metrics.monitoring.svc:9090/?token={secret}\""),
    );
    let found = refused_text(&text);
    assert_eq!(
        found,
        refusal(
            RefusalReason::SecretLiteral,
            "bindings[2].base_url",
            "a URL carrying a query component"
        )
    );
    assert!(!describe(&found).contains(secret));

    let text = with(
        &operator_text(),
        "Never claim a fix you did not observe.",
        &format!("Answer from https://{secret}@logs.example/ and nothing else."),
    );
    let found = refused_text(&text);
    assert_eq!(
        (found.reason, found.member.as_str()),
        (RefusalReason::SecretLiteral, "agent.brief")
    );
    assert!(!describe(&found).contains(secret));

    let text = with(
        &operator_text(),
        "{intent: Find}, actions: [source.read",
        &format!("{{intent: Find}}, actions: [xoxp-{secret}"),
    );
    let found = refused_text(&text);
    assert_eq!(
        (found.reason, found.member.as_str()),
        (RefusalReason::SecretLiteral, "states[1].actions[0]")
    );
    assert!(!describe(&found).contains(secret));
}

/// A URL without user information or a query, and a path with `@` after the host, are no
/// credential.
#[test]
fn a_plain_url_is_no_credential() {
    let text = with(
        &operator_text(),
        "base_url: http://metrics.monitoring.svc:9090",
        "base_url: \"https://metrics.example:9090/team@ops/api\"",
    );
    assert!(check_text(&text, &plugin_actions).is_ok());
}

/// The authority of a URL runs from after `://` to the first `/`: a `#`, a `?` or any other
/// punctuation in a password does not end it, so the `@` after it still marks user information.
#[test]
fn a_password_holding_any_delimiter_but_a_slash_is_refused() {
    for password in [
        "hun#ter2", "hun?ter2", "hun;ter2", "hun!ter2", "hun:ter2", "#?", "x#y?z",
    ] {
        let text = with(
            &operator_text(),
            "base_url: http://logs.monitoring.svc:3100",
            &format!("base_url: \"http://reader:{password}@logs.monitoring.svc:3100/api?x=1#y\""),
        );
        let found = refused_text(&text);
        assert_eq!(
            found,
            refusal(
                RefusalReason::SecretLiteral,
                "bindings[1].base_url",
                "a URL carrying a user name and password"
            ),
            "password {password}"
        );
        assert!(!describe(&found).contains(password), "{}", describe(&found));
    }
}

/// A credential in a member the decoder would refuse (an enum spelling it does not declare) is
/// refused as a credential at that member, before the decoder can describe the value.
#[test]
fn a_credential_in_an_enum_member_is_refused_before_the_decoder() {
    let text = with(&operator_text(), "mode: auto", "mode: xoxb-hunter2");
    let found = refused_text(&text);
    assert_eq!(
        found,
        refusal(
            RefusalReason::SecretLiteral,
            "writes.mode",
            "a value with the prefix `xoxb-`"
        )
    );
    assert!(
        !describe(&found).contains("hunter2"),
        "{}",
        describe(&found)
    );
}

/// A credential spelled as a key the domain does not declare is refused as a credential, naming
/// the mapping that holds it rather than the key, so neither the member nor the detail repeats
/// it.
#[test]
fn a_credential_as_an_unknown_key_is_refused_without_repeating_it() {
    let text = with(
        &operator_text(),
        "writes: {mode: auto,",
        "writes: {xoxb-hunter2: on, mode: auto,",
    );
    let found = refused_text(&text);
    assert_eq!(
        found,
        refusal(
            RefusalReason::SecretLiteral,
            "writes",
            "a value with the prefix `xoxb-`"
        )
    );
    assert!(
        !describe(&found).contains("hunter2"),
        "{}",
        describe(&found)
    );
}

/// Text that is not one YAML document is refused as malformed, with the parser's message, even
/// when it holds a credential: the credential walk reads only a document the parser reads.
#[test]
fn several_documents_are_malformed_before_any_credential() {
    let text = format!(
        "{}---\n{}",
        with(&operator_text(), "mode: auto", "mode: xoxb-hunter2"),
        operator_text()
    );
    let found = refused_text(&text);
    assert_eq!(
        (found.reason, found.member.as_str()),
        (RefusalReason::Malformed, "document")
    );
    assert_eq!(
        found.detail,
        parse(&text).expect_err("two documents").to_string()
    );
}

/// The credential walk reads no more than the decoder does: a document whose aliases expand past
/// the decoder's bound, or that opens more flow collections than it reads, is refused as
/// malformed with the decoder's message, a credential behind it notwithstanding.
#[test]
fn the_credential_walk_keeps_the_decoders_bounds() {
    let bomb = format!(
        "a: &a \"xoxb-hunter2{}\"\nb: [{}]\n",
        "x".repeat(10_000),
        vec!["*a"; 1_000].join(", ")
    );
    let flows = format!(
        "{}x: [{}]\n",
        with(&operator_text(), "mode: auto", "mode: xoxb-hunter2"),
        "[], ".repeat(loom_sdk::agent::MAX_FLOW_OPENERS)
    );
    for text in [bomb, flows] {
        let found = refused_text(&text);
        assert_eq!(
            found,
            refusal(
                RefusalReason::Malformed,
                "document",
                &parse(&text)
                    .expect_err("the decoder refuses it")
                    .to_string()
            )
        );
        assert!(
            !describe(&found).contains("hunter2"),
            "{}",
            describe(&found)
        );
    }
}

/// YAML admits a byte order mark at the start of a stream: the operator example behind one is
/// the operator example.
#[test]
fn the_operator_example_behind_a_byte_order_mark_is_serveable() {
    let text = format!("\u{feff}{}", operator_text());
    assert_eq!(
        check_text(&text, &plugin_actions),
        check_text(&operator_text(), &plugin_actions)
    );
    assert!(check_text(&text, &plugin_actions).is_ok());
}

#[test]
fn a_refusal_reads_reason_member_and_detail() {
    assert_eq!(
        describe(&refused_text(&fixture("unknown-action"))),
        "UnknownAction at states[0].actions[2]: reply.escalate"
    );
}
