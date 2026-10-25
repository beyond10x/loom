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
/// the ESS scenario `an-allow-path-holding-a-dot-dot-segment-is-refused` fixes the first.
#[test]
fn an_allow_path_no_read_may_use_is_refused() {
    let detail = |at: &str| {
        format!(
            "`{at}` is not a plain path, one that starts with `/` and holds no `\\`, no `.` or `..` \
             segment, no encoded separator or dot, no `?` or `#` and no whitespace"
        )
    };
    assert_eq!(
        refused_text(&fixture("allow-path-dot-dot")),
        refusal(
            RefusalReason::Malformed,
            "document",
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
                RefusalReason::Malformed,
                "document",
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
