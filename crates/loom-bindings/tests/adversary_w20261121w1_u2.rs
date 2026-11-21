//! Adversary pass 1 on `story:bindings-document` (wave 2026-11-21-w1, unit u2). Each case states the
//! behaviour it asserts; the ones marked HOLD pass today and exist to hold a check no other case
//! holds.

use b10x_canon::model::{self, Protocol};
use b10x_loom_bindings::{MAX_DOCUMENT_BYTES, Refusal, check, decode};
use loom::bindings::{
    ActionBudget, BindingsDocument, BindingsRefusalReason as Reason, BoundAction, Executor,
    ExecutorSpec,
};
use loom::json::Value as Json;
use std::sync::mpsc;
use std::time::Duration;

const PROTOCOL: &str = r#"
format: protocol/1
protocol:
  id: story
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  ess_gate: {}
  implementation_change: {}
independence_dimensions:
  different_principal: {}
actions:
  spec.gate:
    may_produce:
      - evidence: ess_gate
  change.implement:
    may_produce:
      - evidence: implementation_change
"#;

fn protocol() -> Protocol {
    model::parse(PROTOCOL).expect("the fixture protocol parses")
}

const HEAD: &str = "format: loom.bindings/1\nprotocol: story@1\n";

fn gate(tail: &str) -> String {
    format!(
        "{HEAD}actions:\n  - action: spec.gate\n    executor: {{kind: program, argv: [a]}}\n{tail}"
    )
}

fn refused(text: &str) -> Option<Refusal> {
    decode(text)
        .and_then(|doc| check(&doc, "story@1", &protocol()))
        .err()
}

fn table(results: &str) -> String {
    gate(&format!(
        "    evidence:\n      - {{kind: ess_gate, subject: spec, producer: p, from: exit, results: {results}}}\n"
    ))
}

/// A result table key is the value the program reported: `1.50`, `007` and `True` written as keys
/// are read back as the text the operator wrote (the decoder's own comment: "spelled as written").
#[test]
fn result_keys_are_kept_as_the_operator_wrote_them() {
    let doc = decode(&table("{1.50: a, 007: b, True: c}")).expect("decodes");
    let values: Vec<&str> = doc.actions[0].evidence[0]
        .results
        .iter()
        .map(|rule| rule.value.as_str())
        .collect();
    assert_eq!(values, ["1.50", "007", "True"]);
}

/// `0` and `"0"` are two YAML keys and one result value: the table must not hold the value twice.
#[test]
fn a_result_value_named_twice_through_two_spellings_is_refused() {
    let text = table("{0: passed, \"0\": failed}");
    assert!(refused(&text).is_some(), "loaded:\n{text}");
}

/// The same for `retry_when`: `1` and `"1"` are one evidence kind.
#[test]
fn a_retry_kind_named_twice_through_two_spellings_is_refused() {
    let text = gate("    retry_when: {1: [failed], \"1\": [passed]}\n");
    assert!(refused(&text).is_some(), "loaded:\n{text}");
}

/// The same for `independence`: one producer, two lists of dimensions.
#[test]
fn a_producer_named_twice_through_two_spellings_is_refused() {
    let text =
        format!("{HEAD}independence:\n  1: [different_principal]\n  \"1\": []\nactions: []\n");
    assert!(refused(&text).is_some(), "loaded:\n{text}");
}

/// `protocol` is "a registration, `name@major`" (story Outcome). The registry's rule
/// (`crates/loom-protocols/src/lib.rs` `parse_registration`): lowercase letters, digits and
/// hyphens, first a lowercase letter, at most 100 bytes, then a positive canonical major.
#[test]
fn a_protocol_the_registry_would_not_register_is_refused_at_decode() {
    let long = format!("{}@1", "a".repeat(101));
    let cases = [
        "Story@1",
        "story@0",
        "story@01",
        "1story@1",
        "story_x@1",
        "story.x@1",
        "story@18446744073709551616",
        long.as_str(),
    ];
    let loaded: Vec<&str> = cases
        .into_iter()
        .filter(|protocol| {
            decode(&format!(
                "format: loom.bindings/1\nprotocol: \"{protocol}\"\n"
            ))
            .is_ok()
        })
        .collect();
    assert!(
        loaded.is_empty(),
        "decode accepted protocols no registry holds: {loaded:?}"
    );
}

/// YAML merge keys are not supported; a `<<` member of the literal `arguments` must not become a
/// member named `<<` that the consumer then passes on as data.
#[test]
fn a_merge_key_in_the_arguments_is_not_kept_as_a_member() {
    let text = gate("    arguments: {<<: {a: 1}, b: 2}\n");
    let kept = decode(&text)
        .ok()
        .and_then(|doc| doc.actions[0].arguments.clone());
    if let Some(Json::Object(members)) = kept {
        let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            !names.contains(&"<<"),
            "arguments kept a member named <<: {names:?}"
        );
    }
}

/// `retry_when` names an evidence kind. A kind the protocol does not declare never matches a record;
/// it is a typo the operator should hear about at load, as for `kind`, `subject` and `upstream`.
#[test]
fn a_retry_kind_the_protocol_does_not_declare_is_refused() {
    let text = gate("    retry_when: {ess_gat: [failed]}\n");
    assert!(refused(&text).is_some(), "loaded:\n{text}");
}

/// A document of exactly `MAX_DOCUMENT_BYTES` is read; one byte more is refused.
#[test]
fn the_document_size_boundary_is_one_mebibyte_exactly() {
    let head = gate("    handoff: ");
    let pad = MAX_DOCUMENT_BYTES - head.len();
    let at = format!("{head}{}", "x".repeat(pad));
    assert_eq!(at.len(), MAX_DOCUMENT_BYTES);
    let doc = decode(&at).expect("a document of exactly the limit decodes");
    assert_eq!(doc.actions[0].handoff.as_ref().map(String::len), Some(pad));
    let over = format!("{at}x");
    let refusal = decode(&over).expect_err("one byte over is refused");
    assert_eq!(refusal.0.reason, Reason::Malformed);
    assert_eq!(refusal.0.member, "document");
}

/// More than one YAML document in the text is refused.
#[test]
fn a_second_yaml_document_is_refused() {
    let text = format!("{}---\nformat: other\n", gate(""));
    assert!(decode(&text).is_err());
    assert!(decode(&format!("---\n{}\n---\n{}", gate(""), gate(""))).is_err());
}

/// A tag is refused wherever it stands.
#[test]
fn tags_are_refused_in_every_position() {
    for text in [
        "format: !x loom.bindings/1\nprotocol: story@1\n".to_owned(),
        "format: loom.bindings/1\nprotocol: !x story@1\n".to_owned(),
        format!(
            "{HEAD}actions:\n  - action: !x spec.gate\n    executor: {{kind: program, argv: [a]}}\n"
        ),
        format!(
            "{HEAD}actions:\n  - action: spec.gate\n    executor: {{kind: !x program, argv: [a]}}\n"
        ),
        format!(
            "{HEAD}actions:\n  - action: spec.gate\n    executor: {{kind: program, argv: [!x a]}}\n"
        ),
        format!(
            "{HEAD}actions:\n  - action: spec.gate\n    executor: {{kind: program, argv: [a]}}\n    arguments: {{a: !x b}}\n"
        ),
        format!(
            "{HEAD}actions:\n  - action: spec.gate\n    executor: {{kind: program, argv: [a]}}\n    arguments: {{!x a: b}}\n"
        ),
        format!(
            "{HEAD}actions:\n  - action: spec.gate\n    executor: !x {{kind: program, argv: [a]}}\n"
        ),
    ] {
        assert!(decode(&text).is_err(), "decoded:\n{text}");
    }
}

/// Decodes `text` on its own thread and reports whether it was refused, or `None` when `decode` did
/// not return within `seconds`.
fn refused_within(text: String, seconds: u64) -> Option<bool> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(decode(&text).is_err());
    });
    receiver.recv_timeout(Duration::from_secs(seconds)).ok()
}

fn deep(depth: usize) -> String {
    format!("{}1{}", "[".repeat(depth), "]".repeat(depth))
}

/// Nesting past the parser's depth limit is refused, quickly and without a panic.
#[test]
fn nesting_past_the_limit_is_refused_without_a_panic() {
    for depth in [130, 1000] {
        let text = gate(&format!("    arguments: {{a: {}}}\n", deep(depth)));
        assert_eq!(refused_within(text, 20), Some(true), "depth {depth}");
    }
}

/// A document the size limit admits, made of nothing but opening brackets, is refused at once.
/// (A 1 MiB document is the largest `decode` reads.)
#[test]
fn a_mebibyte_of_open_brackets_is_refused_in_seconds() {
    let text = "[".repeat(MAX_DOCUMENT_BYTES);
    assert_eq!(refused_within(text, 20), Some(true));
}

/// The same for a 100 000-deep flow sequence inside `arguments`, a 200 KB document.
#[test]
fn a_hundred_thousand_deep_flow_sequence_is_refused_in_seconds() {
    let text = gate(&format!("    arguments: {{a: {}}}\n", deep(100_000)));
    assert!(text.len() < 250_000);
    assert_eq!(refused_within(text, 20), Some(true));
}

/// The same for nested block sequences.
#[test]
fn a_half_mebibyte_of_nested_block_dashes_is_refused_in_seconds() {
    let text = "- ".repeat(MAX_DOCUMENT_BYTES / 2);
    assert_eq!(refused_within(text, 20), Some(true));
}

/// The same for nested flow mappings.
#[test]
fn a_quarter_mebibyte_of_open_braces_is_refused_in_seconds() {
    let text = "a: {".repeat(MAX_DOCUMENT_BYTES / 4);
    assert_eq!(refused_within(text, 20), Some(true));
}

/// Aliases are YAML the decoder reads. The 1 MiB limit is the bound on what is read, so the text an
/// alias expands to must stay bounded by it: here 8 KB of text expand to two million nodes (a
/// 250-fold expansion; 350 MB resident and 3 s in a debug build), and a document of the limit's size
/// to hundreds of millions.
#[test]
fn alias_expansion_is_bounded_by_the_document_limit() {
    let anchor = format!("[{}]", "x,".repeat(1000));
    let uses = "*a,".repeat(2000);
    let text = gate(&format!(
        "    arguments: {{base: &a {anchor}, all: [{uses}]}}\n"
    ));
    assert!(text.len() < 10 * 1024, "{}", text.len());
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let outcome = decode(&text).map(|doc| doc.actions.len());
        let _ = sender.send(outcome.is_ok());
    });
    match receiver.recv_timeout(Duration::from_secs(60)) {
        Ok(decoded) => assert!(
            !decoded,
            "decoded an 8 KB document that expands to two million nodes"
        ),
        Err(_) => panic!("decode did not return within 60 s"),
    }
}

/// HOLD `decode.rs:226`: `arguments` and `arguments_from` are alternatives.
#[test]
fn arguments_and_arguments_from_together_are_refused() {
    let text = gate("    arguments: {a: 1}\n    arguments_from: [tools/args.sh]\n");
    let refusal = refused(&text).expect("refused");
    assert_eq!(refusal.0.reason, Reason::Malformed);
    assert_eq!(refusal.0.action.as_deref(), Some("spec.gate"));
    assert_eq!(refusal.0.member, "arguments_from");
}

/// HOLD `load.rs:29`: `check` holds a document it did not decode to the format too.
#[test]
fn check_refuses_a_hand_built_document_of_another_format() {
    let document = BindingsDocument {
        format: "loom.bindings/2".to_owned(),
        protocol: "story@1".to_owned(),
        artifacts: Vec::new(),
        independence: Vec::new(),
        actions: Vec::new(),
    };
    let refusal = check(&document, "story@1", &protocol()).expect_err("refused");
    assert_eq!(refusal.0.reason, Reason::UnsupportedFormat);
}

/// HOLD `load.rs:75-88`: a NUL byte and a backslash path leave or break the workspace.
#[test]
fn workspace_paths_with_a_nul_or_a_backslash_escape_are_refused() {
    for bad in [r#""a\u0000b""#, r"'\\server\share'", r"'\abs'", r"'a\..\b'"] {
        let text = gate("").replace("actions:", &format!("artifacts:\n  spec: {bad}\nactions:"));
        let refusal = refused(&text).unwrap_or_else(|| panic!("loaded {bad}"));
        assert_eq!(refusal.0.reason, Reason::PathNotInWorkspace, "{bad}");
    }
}

/// HOLD `decode.rs:715`: a number JSON cannot write is refused in the literal arguments.
#[test]
fn a_number_json_cannot_write_is_refused_in_the_arguments() {
    for bad in [".inf", "-.inf", ".nan"] {
        let text = gate(&format!("    arguments: {{a: {bad}}}\n"));
        assert!(decode(&text).is_err(), "decoded {bad}");
    }
}

/// HOLD: an argv never becomes a string and a string never stands for a list, anywhere.
#[test]
fn a_string_never_stands_for_a_list() {
    let shape = |executor: &str, tail: &str| {
        format!("{HEAD}actions:\n  - action: spec.gate\n    executor: {executor}\n{tail}")
    };
    for text in [
        shape("{kind: program, argv: \"a b\"}", ""),
        shape("{kind: agent, adapter: a, inputs: src}", ""),
        shape("{kind: agent, adapter: a, outputs: src}", ""),
        shape("{kind: agent, adapter: a}", "    tools: {allow: Read}\n"),
        shape(
            "{kind: program, argv: [a]}",
            "    retry_when: {ess_gate: failed}\n",
        ),
        shape(
            "{kind: program, argv: [a]}",
            "    evidence:\n      - {kind: ess_gate, subject: spec, producer: p, from: exit, results: {\"0\": x}, upstream: spec}\n",
        ),
        format!("{HEAD}independence: {{p: different_principal}}\n"),
    ] {
        assert!(decode(&text).is_err(), "decoded:\n{text}");
    }
}

/// HOLD: nothing in a decoded document joins an argv into one string.
#[test]
fn an_argv_decodes_to_the_list_it_was_written_as() {
    let doc = decode(&gate("    arguments_from: [sh, \"-c\", \"echo a b\"]\n")).expect("decodes");
    let action: &BoundAction = &doc.actions[0];
    let spec: &ExecutorSpec = &action.executor;
    assert_eq!(spec.kind, Executor::Program);
    assert_eq!(spec.argv.as_deref(), Some(["a".to_owned()].as_slice()));
    assert_eq!(
        action.arguments_from.as_deref(),
        Some(["sh".to_owned(), "-c".to_owned(), "echo a b".to_owned()].as_slice())
    );
}

/// HOLD: the defaults are held where the story says and nowhere else.
#[test]
fn defaults_apply_to_the_model_and_max_runs_only() {
    let doc = decode(&gate("")).expect("decodes");
    let budget: &ActionBudget = &doc.actions[0].budget;
    assert_eq!(budget.max_runs, 8);
    assert_eq!(doc.actions[0].timeout_seconds, None);
    assert_eq!(doc.actions[0].executor.model, None);
    let agent = format!(
        "{HEAD}actions:\n  - action: change.implement\n    executor: {{kind: agent, adapter: a}}\n"
    );
    let doc = decode(&agent).expect("decodes");
    assert_eq!(
        doc.actions[0].executor.model.as_deref(),
        Some("claude-haiku-5-5")
    );
    let blank = agent.replace("adapter: a", "adapter: a, model: \"\"");
    assert!(decode(&blank).is_err());
}
