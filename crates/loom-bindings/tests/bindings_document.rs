//! `loom.bindings/1`: decoding, the load-time check against a compiled protocol and the page that
//! shows one complete file (`story:bindings-document`).

use b10x_canon::model::{self, Protocol};
use b10x_loom_bindings::{
    DEFAULT_AGENT_MODEL, DEFAULT_MAX_RUNS, FORMAT, Refusal, check, decode, unbound,
};
use loom::bindings::{BindingsRefusalReason as Reason, Executor, ResultSource};
use std::collections::BTreeSet;

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
  code_review: {}
  child_outcome: {}
  signoff: {}
  imported: {}
independence_dimensions:
  different_principal: {}
actions:
  spec.gate:
    may_produce:
      - evidence: ess_gate
  change.implement:
    may_produce:
      - evidence: implementation_change
  change.review:
    may_produce:
      - evidence: code_review
  change.open:
    may_produce:
      - evidence: child_outcome
  change.copy:
    may_produce:
      - evidence: imported
  change.sign:
    may_produce:
      - evidence: signoff
  change.merge: {}
"#;

fn protocol() -> Protocol {
    model::parse(PROTOCOL).expect("the fixture protocol parses")
}

const REGISTRATION: &str = "story@1";

/// A document header every case shares, then the action list item the case brings.
fn document(actions: &str) -> String {
    format!(
        "format: loom.bindings/1\nprotocol: story@1\nartifacts:\n  spec: docs/spec\n  impl: src\n\
         independence:\n  reviewer: [different_principal]\nactions:\n{actions}"
    )
}

const GATE: &str = "  - action: spec.gate\n    executor: {kind: program, argv: [tools/gate.sh, ess]}\n    \
    evidence:\n      - {kind: ess_gate, subject: spec, producer: ess, from: exit, results: {\"0\": passed, \"*\": failed}}\n";

/// Decodes `text` and checks it against the fixture protocol: the first refusal, or none.
fn refused(text: &str) -> Option<Refusal> {
    decode(text)
        .and_then(|doc| check(&doc, REGISTRATION, &protocol()))
        .err()
}

fn assert_refused(text: &str, reason: Reason, action: Option<&str>, member: &str) -> Refusal {
    let refusal = refused(text).unwrap_or_else(|| panic!("not refused:\n{text}"));
    let found = &refusal.0;
    assert_eq!(found.reason, reason, "{refusal}\n{text}");
    assert_eq!(found.action.as_deref(), action, "{refusal}");
    assert_eq!(found.member, member, "{refusal}");
    let shown = refusal.to_string();
    assert!(shown.contains(member), "{shown} names {member}");
    if let Some(action) = action {
        assert!(shown.contains(action), "{shown} names {action}");
    }
    refusal
}

#[test]
fn a_complete_document_decodes_and_checks() {
    let text = document(&format!(
        "{GATE}\
\x20 - action: change.implement\n    executor: {{kind: agent, adapter: claude-code, inputs: [docs/spec], outputs: [src/lib.rs]}}\n    \
    tools: {{allow: [Read, Edit]}}\n    budget: {{max_runs: 3, max_turns: 40, max_tokens: 100000, max_budget_micro_usd: 2000000}}\n    \
    timeout_seconds: 1500\n    retry_when: {{implementation_change: [failed]}}\n    handoff: a person continues\n    \
    arguments: {{domain: bindings}}\n    \
    evidence:\n      - {{kind: implementation_change, subject: impl, producer: agent, from: effect, results: {{performed: opened}}, upstream: [spec]}}\n"
    ));
    let doc = decode(&text).expect("decodes");
    check(&doc, REGISTRATION, &protocol()).expect("checks");
    assert_eq!(doc.format, FORMAT);
    assert_eq!(doc.protocol, "story@1");
    assert_eq!(doc.actions.len(), 2);
    let agent = &doc.actions[1];
    assert_eq!(agent.executor.kind, Executor::Agent);
    assert_eq!(agent.budget.max_runs, 3);
    assert_eq!(agent.budget.max_turns, Some(40));
    assert_eq!(agent.evidence[0].from, ResultSource::Effect);
    assert_eq!(agent.evidence[0].upstream, ["spec"]);
    assert_eq!(agent.evidence[0].results[0].value, "performed");
    assert_eq!(
        agent.evidence[0].results[0].result.as_deref(),
        Some("opened")
    );
    assert_eq!(agent.tools.as_ref().unwrap().allow, ["Read", "Edit"]);
    assert_eq!(agent.handoff.as_deref(), Some("a person continues"));
    assert!(agent.arguments.is_some());
    let gate = &doc.actions[0];
    assert_eq!(gate.evidence[0].from, ResultSource::Exit);
    assert_eq!(gate.evidence[0].results.len(), 2);
    assert_eq!(gate.evidence[0].results[1].value, "*");
}

#[test]
fn every_executor_and_result_source_decodes() {
    let text = document(&format!(
        "{GATE}\
\x20 - action: change.review\n    executor: {{kind: program, argv: [tools/review.sh]}}\n    \
    evidence:\n      - {{kind: code_review, subject: impl, producer: reviewer, from: /verdict/state, results: {{approved: approved, rejected: rejected}}}}\n\
\x20 - action: change.sign\n    executor: {{kind: person, principal: alice}}\n    handoff: sign it\n\
\x20 - action: change.open\n    executor: {{kind: open, each: items, bindings: child.bindings.yaml}}\n    \
    evidence:\n      - {{kind: child_outcome, subject: impl, producer: loom, from: effect, results: {{performed: opened}}}}\n\
\x20 - action: change.copy\n    executor: {{kind: copy, evidence: imported, from: other@1}}\n"
    ));
    let doc = decode(&text).expect("decodes");
    check(&doc, REGISTRATION, &protocol()).expect("checks");
    let kinds: Vec<Executor> = doc.actions.iter().map(|a| a.executor.kind).collect();
    assert_eq!(
        kinds,
        [
            Executor::Program,
            Executor::Program,
            Executor::Person,
            Executor::Open,
            Executor::Copy
        ]
    );
    let review = &doc.actions[1].evidence[0];
    assert_eq!(review.from, ResultSource::Report);
    assert_eq!(review.pointer.as_deref(), Some("/verdict/state"));
    assert_eq!(doc.actions[2].executor.principal.as_deref(), Some("alice"));
    assert_eq!(doc.actions[3].executor.each.as_deref(), Some("items"));
    assert_eq!(doc.actions[4].executor.from.as_deref(), Some("other@1"));
}

#[test]
fn json_is_read_as_yaml() {
    let text = r#"{"format": "loom.bindings/1", "protocol": "story@1",
        "actions": [{"action": "spec.gate", "executor": {"kind": "program", "argv": ["a"]},
        "evidence": [{"kind": "ess_gate", "subject": "spec", "producer": "ess", "from": "exit",
        "results": {"0": "passed", "*": "failed"}}]}]}"#;
    let doc = decode(text).expect("JSON decodes");
    check(&doc, REGISTRATION, &protocol()).expect("checks");
}

#[test]
fn defaults_are_held_by_the_decoded_document() {
    let text = document(
        "  - action: change.implement\n    executor: {kind: agent, adapter: claude-code}\n\
\x20 - action: spec.gate\n    executor: {kind: program, argv: [a]}\n",
    );
    let doc = decode(&text).expect("decodes");
    assert_eq!(DEFAULT_AGENT_MODEL, "claude-haiku-5-5");
    assert_eq!(DEFAULT_MAX_RUNS, 8);
    assert_eq!(
        doc.actions[0].executor.model.as_deref(),
        Some("claude-haiku-5-5")
    );
    assert_eq!(doc.actions[0].budget.max_runs, 8);
    assert_eq!(doc.actions[1].budget.max_runs, 8);
    assert_eq!(doc.actions[1].executor.model, None);
    assert!(doc.actions[1].evidence.is_empty());
    assert!(doc.actions[1].tools.is_none());
}

#[test]
fn an_agent_keeps_the_model_it_names() {
    let text = document(
        "  - action: change.implement\n    executor: {kind: agent, adapter: a, model: m1}\n",
    );
    assert_eq!(
        decode(&text).unwrap().actions[0].executor.model.as_deref(),
        Some("m1")
    );
}

#[test]
fn an_undeclared_member_is_refused_at_every_level() {
    let top = format!("{}extra: 1\n", document(GATE));
    assert_refused(&top, Reason::UndeclaredMember, None, "extra");
    let action = document(&format!("{GATE}    mystery: 1\n"));
    assert_refused(
        &action,
        Reason::UndeclaredMember,
        Some("spec.gate"),
        "mystery",
    );
    let executor =
        document("  - action: spec.gate\n    executor: {kind: program, argv: [a], shell: true}\n");
    assert_refused(
        &executor,
        Reason::UndeclaredMember,
        Some("spec.gate"),
        "executor.shell",
    );
    let evidence = document(
        "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      \
         - {kind: ess_gate, subject: spec, producer: ess, from: exit, results: {\"0\": passed}, extra: 1}\n",
    );
    assert_refused(
        &evidence,
        Reason::UndeclaredMember,
        Some("spec.gate"),
        "evidence[0].extra",
    );
    let budget = document(&format!("{GATE}    budget: {{max_runs: 2, spend: 1}}\n"));
    assert_refused(
        &budget,
        Reason::UndeclaredMember,
        Some("spec.gate"),
        "budget.spend",
    );
    let tools = document(&format!(
        "{GATE}    tools: {{allow: [Read], deny: [Bash]}}\n"
    ));
    assert_refused(
        &tools,
        Reason::UndeclaredMember,
        Some("spec.gate"),
        "tools.deny",
    );
}

#[test]
fn a_missing_or_other_format_is_refused() {
    let missing = "protocol: story@1\nactions: []\n";
    assert_refused(missing, Reason::UnsupportedFormat, None, "format");
    for other in ["loom.bindings/2", "intake.bindings/1", "1", ""] {
        let text = format!("format: \"{other}\"\nprotocol: story@1\nactions: []\n");
        assert_refused(&text, Reason::UnsupportedFormat, None, "format");
    }
}

#[test]
fn an_argv_that_is_empty_or_not_a_list_of_strings_is_refused() {
    for argv in [
        "[]",
        "\"tools/gate.sh ess\"",
        "[a, 2]",
        "[a, null]",
        "[a, [b]]",
        "{a: b}",
        "[\"\", b]",
    ] {
        let text = document(&format!(
            "  - action: spec.gate\n    executor: {{kind: program, argv: {argv}}}\n"
        ));
        assert_refused(
            &text,
            Reason::InvalidArgv,
            Some("spec.gate"),
            "executor.argv",
        );
    }
    let from = document(&format!("{GATE}    arguments_from: []\n"));
    assert_refused(
        &from,
        Reason::InvalidArgv,
        Some("spec.gate"),
        "arguments_from",
    );
    let from = document(&format!("{GATE}    arguments_from: tools/args.sh\n"));
    assert_refused(
        &from,
        Reason::InvalidArgv,
        Some("spec.gate"),
        "arguments_from",
    );
    let missing = document("  - action: spec.gate\n    executor: {kind: program}\n");
    assert_refused(
        &missing,
        Reason::MissingMember,
        Some("spec.gate"),
        "executor.argv",
    );
}

#[test]
fn executor_members_missing_for_their_variant_are_refused() {
    let cases = [
        ("agent", "{kind: agent}", "executor.adapter"),
        ("person", "{kind: person}", "executor.principal"),
        ("open", "{kind: open, each: items}", "executor.bindings"),
        ("open", "{kind: open, bindings: c.yaml}", "executor.name"),
        ("copy", "{kind: copy, evidence: imported}", "executor.from"),
        ("copy", "{kind: copy, from: other@1}", "executor.evidence"),
    ];
    for (_, executor, member) in cases {
        let text = document(&format!(
            "  - action: change.copy\n    executor: {executor}\n"
        ));
        assert_refused(&text, Reason::MissingMember, Some("change.copy"), member);
    }
    let both = document(
        "  - action: change.open\n    executor: {kind: open, name: n, each: e, bindings: c.yaml}\n",
    );
    assert_refused(
        &both,
        Reason::Malformed,
        Some("change.open"),
        "executor.name",
    );
    let unknown = document("  - action: change.open\n    executor: {kind: shell}\n");
    assert_refused(
        &unknown,
        Reason::Malformed,
        Some("change.open"),
        "executor.kind",
    );
}

#[test]
fn a_document_that_is_not_yaml_or_not_an_object_is_refused() {
    for text in [
        "",
        "[1, 2]",
        "just words",
        "a: [",
        "format: loom.bindings/1\nprotocol: story@1\nactions: 4\n",
    ] {
        let refusal = refused(text).unwrap_or_else(|| panic!("not refused: {text}"));
        assert!(
            matches!(
                refusal.0.reason,
                Reason::Malformed | Reason::UnsupportedFormat
            ),
            "{refusal}"
        );
    }
    let duplicate = "format: loom.bindings/1\nformat: loom.bindings/1\nprotocol: story@1\n";
    assert_refused(duplicate, Reason::Malformed, None, "document");
}

#[test]
fn a_protocol_that_is_not_a_registration_is_refused() {
    for protocol in [
        "story",
        "story@",
        "@1",
        "story@one",
        "sto ry@1",
        "story@1.2",
    ] {
        let text = format!("format: loom.bindings/1\nprotocol: \"{protocol}\"\n");
        assert_refused(&text, Reason::InvalidProtocol, None, "protocol");
    }
    let missing = "format: loom.bindings/1\n";
    assert_refused(missing, Reason::MissingMember, None, "protocol");
}

#[test]
fn a_document_for_another_protocol_is_refused_at_load() {
    let text = document(GATE).replace("story@1", "story@2");
    assert_refused(&text, Reason::ProtocolMismatch, None, "protocol");
}

#[test]
fn the_load_check_refuses_an_action_the_protocol_does_not_declare() {
    let text = document("  - action: spec.unknown\n    executor: {kind: program, argv: [a]}\n");
    assert_refused(&text, Reason::UnknownAction, Some("spec.unknown"), "action");
}

#[test]
fn the_load_check_refuses_a_duplicate_action() {
    let text = document(&format!("{GATE}{GATE}"));
    assert_refused(&text, Reason::DuplicateAction, Some("spec.gate"), "action");
}

#[test]
fn the_load_check_refuses_a_kind_outside_may_produce() {
    let text = document(
        "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      \
         - {kind: code_review, subject: spec, producer: ess, from: exit, results: {\"0\": passed}}\n",
    );
    assert_refused(
        &text,
        Reason::KindNotProducible,
        Some("spec.gate"),
        "evidence[0].kind",
    );
    let merge = document(
        "  - action: change.merge\n    executor: {kind: program, argv: [a]}\n    evidence:\n      \
         - {kind: ess_gate, subject: spec, producer: ess, from: exit, results: {\"0\": passed}}\n",
    );
    assert_refused(
        &merge,
        Reason::KindNotProducible,
        Some("change.merge"),
        "evidence[0].kind",
    );
    let copy = document(
        "  - action: change.copy\n    executor: {kind: copy, evidence: ess_gate, from: other@1}\n",
    );
    assert_refused(
        &copy,
        Reason::KindNotProducible,
        Some("change.copy"),
        "executor.evidence",
    );
}

#[test]
fn the_load_check_refuses_an_undeclared_artifact() {
    let subject = document(
        "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      \
         - {kind: ess_gate, subject: nothing, producer: ess, from: exit, results: {\"0\": passed}}\n",
    );
    assert_refused(
        &subject,
        Reason::UndeclaredArtifact,
        Some("spec.gate"),
        "evidence[0].subject",
    );
    let upstream = document(
        "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      \
         - {kind: ess_gate, subject: spec, producer: ess, from: exit, results: {\"0\": passed}, upstream: [spec, nothing]}\n",
    );
    assert_refused(
        &upstream,
        Reason::UndeclaredArtifact,
        Some("spec.gate"),
        "evidence[0].upstream[1]",
    );
    let artifacts = document(GATE).replace("  impl: src\n", "  impl: src\n  ghost: ghost\n");
    assert_refused(
        &artifacts,
        Reason::UndeclaredArtifact,
        None,
        "artifacts.ghost",
    );
}

#[test]
fn the_load_check_refuses_members_that_do_not_fit_the_executor() {
    let tools = document(&format!("{GATE}    tools: {{allow: [Read]}}\n"));
    assert_refused(
        &tools,
        Reason::MemberNotForExecutor,
        Some("spec.gate"),
        "tools",
    );
    for member in ["max_turns", "max_tokens", "max_budget_micro_usd"] {
        let text = document(&format!("{GATE}    budget: {{{member}: 5}}\n"));
        assert_refused(
            &text,
            Reason::MemberNotForExecutor,
            Some("spec.gate"),
            &format!("budget.{member}"),
        );
    }
    for member in ["inputs", "outputs"] {
        let text = document(&format!(
            "  - action: spec.gate\n    executor: {{kind: program, argv: [a], {member}: [x]}}\n"
        ));
        assert_refused(
            &text,
            Reason::MemberNotForExecutor,
            Some("spec.gate"),
            &format!("executor.{member}"),
        );
    }
    let argv_on_agent = document(
        "  - action: change.implement\n    executor: {kind: agent, adapter: a, argv: [x]}\n",
    );
    assert_refused(
        &argv_on_agent,
        Reason::MemberNotForExecutor,
        Some("change.implement"),
        "executor.argv",
    );
    let adapter_on_person = document(
        "  - action: change.sign\n    executor: {kind: person, principal: p, adapter: a}\n",
    );
    assert_refused(
        &adapter_on_person,
        Reason::MemberNotForExecutor,
        Some("change.sign"),
        "executor.adapter",
    );
    let model_on_program =
        document("  - action: spec.gate\n    executor: {kind: program, argv: [a], model: m}\n");
    assert_refused(
        &model_on_program,
        Reason::MemberNotForExecutor,
        Some("spec.gate"),
        "executor.model",
    );
    let from_on_open = document(
        "  - action: change.open\n    executor: {kind: open, name: n, bindings: c.yaml, from: x}\n",
    );
    assert_refused(
        &from_on_open,
        Reason::MemberNotForExecutor,
        Some("change.open"),
        "executor.from",
    );
}

#[test]
fn the_load_check_holds_the_timeout_to_one_to_3600() {
    for bad in ["0", "-5", "3601", "100000"] {
        let text = document(&format!("{GATE}    timeout_seconds: {bad}\n"));
        assert_refused(
            &text,
            Reason::TimeoutOutOfRange,
            Some("spec.gate"),
            "timeout_seconds",
        );
    }
    for good in ["1", "3600"] {
        let text = document(&format!("{GATE}    timeout_seconds: {good}\n"));
        assert!(refused(&text).is_none(), "{good}");
    }
    let text = document(&format!("{GATE}    timeout_seconds: soon\n"));
    assert_refused(
        &text,
        Reason::Malformed,
        Some("spec.gate"),
        "timeout_seconds",
    );
}

#[test]
fn the_load_check_refuses_paths_outside_the_workspace() {
    for bad in [
        "/etc/passwd",
        "../up",
        "a/../../b",
        "",
        "C:/windows",
        "a/../..",
    ] {
        let artifacts = document(GATE).replace("docs/spec", &format!("\"{bad}\""));
        assert_refused(
            &artifacts,
            Reason::PathNotInWorkspace,
            None,
            "artifacts.spec",
        );
        for member in ["inputs", "outputs"] {
            let text = document(&format!(
                "  - action: change.implement\n    executor: {{kind: agent, adapter: a, {member}: [ok/path, \"{bad}\"]}}\n"
            ));
            assert_refused(
                &text,
                Reason::PathNotInWorkspace,
                Some("change.implement"),
                &format!("executor.{member}[1]"),
            );
        }
    }
    for good in ["docs/spec", "a/b.c", "./a", "a/./b", "a..b/c"] {
        let text = document(GATE).replace("docs/spec", good);
        assert!(refused(&text).is_none(), "{good}");
    }
}

#[test]
fn the_load_check_refuses_an_independence_dimension_the_protocol_does_not_declare() {
    let text = document(GATE).replace("[different_principal]", "[different_principal, other]");
    assert_refused(
        &text,
        Reason::UndeclaredDimension,
        None,
        "independence.reviewer[1]",
    );
}

#[test]
fn a_tool_entry_that_is_empty_or_holds_a_comma_or_white_space_is_refused() {
    for bad in [
        "\"\"",
        "\"Read,Edit\"",
        "\"Read Edit\"",
        "\"Read\\tEdit\"",
        "\" \"",
        "\"Read\\n\"",
        "\"Read\\u00a0Edit\"",
    ] {
        let text = document(&format!(
            "  - action: change.implement\n    executor: {{kind: agent, adapter: a}}\n    tools: {{allow: [Read, {bad}]}}\n"
        ));
        let refusal = assert_refused(
            &text,
            Reason::InvalidToolName,
            Some("change.implement"),
            "tools.allow[1]",
        );
        assert!(refusal.0.detail.contains("entry"), "{refusal}");
    }
    let not_text = document(
        "  - action: change.implement\n    executor: {kind: agent, adapter: a}\n    tools: {allow: [Read, 3]}\n",
    );
    assert_refused(
        &not_text,
        Reason::InvalidToolName,
        Some("change.implement"),
        "tools.allow[1]",
    );
}

#[test]
fn an_effect_entry_takes_the_one_value_performed() {
    let entry = |results: &str| {
        document(&format!(
            "  - action: change.open\n    executor: {{kind: open, name: n, bindings: c.yaml}}\n    evidence:\n      \
             - {{kind: child_outcome, subject: impl, producer: loom, from: effect, results: {results}}}\n"
        ))
    };
    let ok = entry("{performed: opened}");
    assert!(refused(&ok).is_none());
    let bad = assert_refused(
        &entry("{performed: opened, \"0\": opened}"),
        Reason::InvalidEffectResult,
        Some("change.open"),
        "evidence[0].results.0",
    );
    assert!(bad.0.detail.contains('0'), "{bad}");
    assert_refused(
        &entry("{\"*\": opened}"),
        Reason::InvalidEffectResult,
        Some("change.open"),
        "evidence[0].results.*",
    );
    assert_refused(
        &entry("{done: opened}"),
        Reason::InvalidEffectResult,
        Some("change.open"),
        "evidence[0].results.done",
    );
}

#[test]
fn an_evidence_entry_needs_a_source_and_a_table() {
    let entry = |body: &str| {
        document(&format!(
            "  - action: spec.gate\n    executor: {{kind: program, argv: [a]}}\n    evidence:\n      - {body}\n"
        ))
    };
    for from in ["sideways", "verdict", "\"\"", "/a~2b", "4"] {
        let text = entry(&format!(
            "{{kind: ess_gate, subject: spec, producer: ess, from: {from:?}, results: {{\"0\": passed}}}}"
        ));
        assert_refused(
            &text,
            Reason::InvalidEvidence,
            Some("spec.gate"),
            "evidence[0].from",
        );
    }
    let empty = entry("{kind: ess_gate, subject: spec, producer: ess, from: exit, results: {}}");
    assert_refused(
        &empty,
        Reason::InvalidEvidence,
        Some("spec.gate"),
        "evidence[0].results",
    );
    let blank =
        entry("{kind: \"\", subject: spec, producer: ess, from: exit, results: {\"0\": passed}}");
    assert_refused(
        &blank,
        Reason::InvalidEvidence,
        Some("spec.gate"),
        "evidence[0].kind",
    );
    let missing = entry("{kind: ess_gate, subject: spec, from: exit, results: {\"0\": passed}}");
    assert_refused(
        &missing,
        Reason::MissingMember,
        Some("spec.gate"),
        "evidence[0].producer",
    );
    let null_result =
        entry("{kind: ess_gate, subject: spec, producer: ess, from: exit, results: {\"0\": null}}");
    let doc = decode(&null_result).expect("a null result decodes");
    assert_eq!(doc.actions[0].evidence[0].results[0].result, None);
}

#[test]
fn a_budget_member_below_one_is_refused() {
    for member in [
        "max_runs",
        "max_turns",
        "max_tokens",
        "max_budget_micro_usd",
    ] {
        for bad in ["0", "-1", "many", "1.5"] {
            let text = document(&format!(
                "  - action: change.implement\n    executor: {{kind: agent, adapter: a}}\n    budget: {{{member}: {bad}}}\n"
            ));
            assert_refused(
                &text,
                Reason::InvalidBudget,
                Some("change.implement"),
                &format!("budget.{member}"),
            );
        }
    }
}

#[test]
fn an_action_with_no_binding_is_not_an_error_and_is_listed() {
    let text = document(GATE);
    let doc = decode(&text).unwrap();
    check(&doc, REGISTRATION, &protocol()).expect("an unbound action is not refused");
    assert_eq!(
        unbound(&doc, &protocol()),
        [
            "change.implement",
            "change.review",
            "change.open",
            "change.copy",
            "change.sign",
            "change.merge"
        ]
    );
    let none = decode("format: loom.bindings/1\nprotocol: story@1\n").unwrap();
    assert!(none.actions.is_empty());
    assert_eq!(unbound(&none, &protocol()).len(), 7);
}

/// Every reason a refusal can carry, so a variant added to the specification fails this file's
/// compilation until a case reaches it.
fn index(reason: Reason) -> usize {
    match reason {
        Reason::Malformed => 0,
        Reason::UndeclaredMember => 1,
        Reason::MissingMember => 2,
        Reason::UnsupportedFormat => 3,
        Reason::InvalidArgv => 4,
        Reason::InvalidProtocol => 5,
        Reason::InvalidEvidence => 6,
        Reason::InvalidBudget => 7,
        Reason::InvalidToolName => 8,
        Reason::InvalidEffectResult => 9,
        Reason::ProtocolMismatch => 10,
        Reason::UnknownAction => 11,
        Reason::DuplicateAction => 12,
        Reason::KindNotProducible => 13,
        Reason::UndeclaredArtifact => 14,
        Reason::MemberNotForExecutor => 15,
        Reason::TimeoutOutOfRange => 16,
        Reason::PathNotInWorkspace => 17,
        Reason::UndeclaredDimension => 18,
        Reason::UndeclaredEvidenceKind => 19,
        Reason::ExitClassConflict => 20,
        Reason::ProgramNotBesideBindings => 21,
    }
}

#[test]
fn every_refusal_reason_is_reachable_from_a_document() {
    let documents = [
        "[1, 2]".to_owned(),
        document(&format!("{GATE}    retry_when: {{nothing: [failed]}}\n")),
        "format: loom.bindings/1\nprotocol: story@1\nextra: 1\n".to_owned(),
        "format: loom.bindings/1\n".to_owned(),
        "protocol: story@1\n".to_owned(),
        document("  - action: spec.gate\n    executor: {kind: program, argv: []}\n"),
        "format: loom.bindings/1\nprotocol: story\n".to_owned(),
        document(
            "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      - {kind: ess_gate, subject: spec, producer: ess, from: exit, results: {}}\n",
        ),
        document(&format!("{GATE}    budget: {{max_runs: 0}}\n")),
        document(
            "  - action: change.implement\n    executor: {kind: agent, adapter: a}\n    tools: {allow: [\"a b\"]}\n",
        ),
        document(
            "  - action: change.open\n    executor: {kind: open, name: n, bindings: c}\n    evidence:\n      - {kind: child_outcome, subject: impl, producer: p, from: effect, results: {no: x}}\n",
        ),
        document(GATE).replace("story@1", "story@2"),
        document("  - action: nope\n    executor: {kind: program, argv: [a]}\n"),
        document(&format!("{GATE}{GATE}")),
        document(
            "  - action: spec.gate\n    executor: {kind: program, argv: [a]}\n    evidence:\n      - {kind: code_review, subject: spec, producer: p, from: exit, results: {\"0\": x}}\n",
        ),
        document(GATE).replace("impl: src", "impl: src\n  ghost: g"),
        document(&format!("{GATE}    tools: {{allow: [Read]}}\n")),
        document(&format!("{GATE}    timeout_seconds: 0\n")),
        document(GATE).replace("docs/spec", "/abs"),
        document(GATE).replace("[different_principal]", "[nothing]"),
        document(&format!(
            "{GATE}    exits: {{error: [1], not_called: [1]}}\n"
        )),
    ];
    let mut reached: BTreeSet<usize> = documents
        .iter()
        .map(|text| index(refused(text).unwrap_or_else(|| panic!("{text}")).0.reason))
        .collect();
    // `resolve_programs` refuses against the directory of the bindings file: `tools/gate.sh` is
    // not beside a file in the build's scratch directory.
    let beside =
        decode(&document(GATE).replace("artifacts:", "program_root: bindings\nartifacts:"))
            .expect("the document decodes");
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(directory).unwrap();
    reached.insert(index(
        b10x_loom_bindings::resolve_programs(&beside, directory)
            .expect_err("the program is not beside the bindings file")
            .0
            .reason,
    ));
    let all: BTreeSet<usize> = (0..22).collect();
    assert_eq!(reached, all);
}

/// The first fenced block of `text` whose info string starts with `yaml` and whose body holds
/// `needle`.
fn block<'a>(text: &'a str, needle: &str) -> &'a str {
    let mut rest = text;
    while let Some(start) = rest.find("```yaml") {
        let after = &rest[start..];
        let body_start = after.find('\n').expect("a fence line ends") + 1;
        let body = &after[body_start..];
        let end = body.find("```").expect("a fence closes");
        if body[..end].contains(needle) {
            return &body[..end];
        }
        rest = &body[end + 3..];
    }
    panic!("no yaml block holding {needle}");
}

#[test]
fn the_guide_page_shows_a_complete_file_that_decodes_and_checks() {
    let page = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../website/docs/guides/bindings.md"
    ))
    .expect("the guide page exists");
    let file = block(&page, "format: loom.bindings/1");
    let protocol = model::parse(block(&page, "format: protocol/1")).expect("the page's protocol");
    let doc = decode(file).expect("the page's file decodes");
    let registration = doc.protocol.clone();
    check(&doc, &registration, &protocol).expect("the page's file checks against its protocol");
    let kinds: BTreeSet<usize> = doc
        .actions
        .iter()
        .map(|action| action.executor.kind as usize)
        .collect();
    assert_eq!(kinds.len(), 5, "the page's file uses all five executors");
    let sources: BTreeSet<usize> = doc
        .actions
        .iter()
        .flat_map(|action| action.evidence.iter().map(|e| e.from as usize))
        .collect();
    assert_eq!(sources.len(), 3, "and all three result sources");
}

#[test]
fn a_retry_kind_the_protocol_declares_is_kept_and_an_undeclared_one_is_refused() {
    let ok = document(&format!("{GATE}    retry_when: {{ess_gate: [failed]}}\n"));
    assert!(refused(&ok).is_none());
    let bad = document(&format!("{GATE}    retry_when: {{ess_gat: [failed]}}\n"));
    assert_refused(
        &bad,
        Reason::UndeclaredEvidenceKind,
        Some("spec.gate"),
        "retry_when.ess_gat",
    );
}

#[test]
fn a_retry_result_that_is_empty_is_refused() {
    let text = document(&format!(
        "{GATE}    retry_when: {{ess_gate: [failed, \"\"]}}\n"
    ));
    assert_refused(
        &text,
        Reason::Malformed,
        Some("spec.gate"),
        "retry_when.ess_gate[1]",
    );
}

#[test]
fn the_child_bindings_path_of_an_open_executor_names_a_file_beside_the_bindings_file() {
    for bad in [
        "/abs/child.yaml",
        "../child.yaml",
        "a/../../c.yaml",
        "C:/c.yaml",
        ".",
        "sub/",
        "sub/.",
    ] {
        let text = document(&format!(
            "  - action: change.open\n    executor: {{kind: open, name: n, bindings: \"{bad}\"}}\n"
        ));
        assert_refused(
            &text,
            Reason::PathNotInWorkspace,
            Some("change.open"),
            "executor.bindings",
        );
    }
    let ok = document(
        "  - action: change.open\n    executor: {kind: open, name: n, bindings: sub/child.yaml}\n",
    );
    assert!(refused(&ok).is_none());
}

#[test]
fn an_evidence_source_must_fit_the_executor_the_story_states() {
    let program = "{kind: program, argv: [a]}";
    let person = "{kind: person, principal: p}";
    let open = "{kind: open, name: n, bindings: c.yaml}";
    let agent = "{kind: agent, adapter: a}";
    let copy = "{kind: copy, evidence: child_outcome, from: other@1}";
    let text = |executor: &str, from: &str| {
        let results = if from == "effect" {
            "{performed: opened}"
        } else {
            "{\"0\": opened}"
        };
        document(&format!(
            "  - action: change.open\n    executor: {executor}\n    evidence:\n      \
             - {{kind: child_outcome, subject: impl, producer: p, from: \"{from}\", results: {results}}}\n"
        ))
    };
    for from in ["exit", "/report/state"] {
        for executor in [person, open] {
            assert_refused(
                &text(executor, from),
                Reason::MemberNotForExecutor,
                Some("change.open"),
                "evidence[0].from",
            );
        }
        assert!(refused(&text(program, from)).is_none(), "program {from}");
    }
    assert_refused(
        &text(program, "effect"),
        Reason::MemberNotForExecutor,
        Some("change.open"),
        "evidence[0].from",
    );
    for executor in [person, open] {
        assert!(refused(&text(executor, "effect")).is_none());
    }
    for executor in [agent, copy] {
        for from in ["exit", "/report/state", "effect"] {
            assert!(
                refused(&text(executor, from)).is_none(),
                "{executor} {from}"
            );
        }
    }
}

#[test]
fn prose_with_yaml_punctuation_is_text_and_ordinary_nesting_decodes() {
    let text = document(&format!(
        "{GATE}    handoff: \"Done! Fish & chips *now*, then [review] {{soon}}\"\n    arguments: {{a: {}1{}}}\n",
        "[".repeat(30),
        "]".repeat(30)
    ));
    let doc = decode(&text).expect("decodes");
    assert_eq!(
        doc.actions[0].handoff.as_deref(),
        Some("Done! Fish & chips *now*, then [review] {soon}")
    );
    let block = document(&format!(
        "{GATE}    handoff: |\n      first line\n      - second & *third*\n"
    ));
    assert!(decode(&block).is_ok());
}

#[test]
fn nesting_at_the_cap_decodes_and_one_past_is_refused() {
    let at = |depth: usize| {
        document(&format!(
            "{GATE}    arguments: {{a: {}1{}}}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        ))
    };
    assert!(decode(&at(40)).is_ok());
    let refusal = decode(&at(b10x_loom_bindings::MAX_NESTING + 1)).expect_err("refused");
    assert_eq!(refusal.0.reason, Reason::Malformed);
    assert_eq!(refusal.0.member, "document");
}

#[test]
fn an_alias_used_within_the_node_bound_decodes() {
    let once = document(&format!(
        "{GATE}    arguments: {{base: &a [1, 2], again: *a}}\n"
    ));
    assert!(decode(&once).is_ok());
}
