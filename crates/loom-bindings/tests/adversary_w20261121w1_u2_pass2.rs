//! Adversary pass 2 on `story:bindings-document` (wave 2026-11-21-w1, unit u2), head e991251b: the
//! bounded reader (`tree.rs`), the new load checks and the guide's statements. A case that is red
//! says so in its comment; the others hold a behaviour no other case holds.

use b10x_canon::model::{self, Protocol};
use b10x_loom_bindings::{MAX_NESTING, MAX_NODES, Refusal, check, decode};
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
  change.open:
    may_produce:
      - evidence: ess_gate
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

/// Whether `decode` refused `text`, or `None` when it did not return within `seconds`.
fn refused_within(text: String, seconds: u64) -> Option<bool> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(decode(&text).is_err());
    });
    receiver.recv_timeout(Duration::from_secs(seconds)).ok()
}

fn results_of(table: &str) -> String {
    gate(&format!(
        "    evidence:\n      - {{kind: ess_gate, subject: spec, producer: p, from: exit, results: {table}}}\n"
    ))
}

/// RED. A literal `1.0` in the arguments is the JSON number `1.0` at the base (`serde_yaml_ng`
/// writes floats with a fraction); the reader's `f64` display writes `1`, so a float the operator
/// wrote becomes an integer for the program that reads the arguments.
#[test]
fn a_float_written_with_a_fraction_stays_a_float_in_the_arguments() {
    let doc = decode(&gate("    arguments: {a: 1.0, b: 2.50}\n")).expect("decodes");
    let Some(Json::Object(members)) = &doc.actions[0].arguments else {
        panic!("arguments are an object");
    };
    let a = members.iter().find(|(name, _)| name == "a").expect("a");
    let Json::Number(text) = &a.1 else {
        panic!("a is a number");
    };
    assert!(
        text.contains('.') || text.contains('e'),
        "1.0 became {text}"
    );
}

/// RED. A file saved with a byte order mark (an editor on Windows does) is valid YAML text and is
/// refused, with a message about "more than one document".
#[test]
fn a_leading_byte_order_mark_does_not_refuse_the_file() {
    let text = format!("\u{feff}{}", gate(""));
    let result = decode(&text);
    assert!(result.is_ok(), "{}", result.err().unwrap());
}

/// The bound is "more than 100 000 values": a document of exactly that many decodes, one more is
/// refused. Twelve values precede the list in `gate` (the document, `format`, `protocol`, `actions`,
/// the action, its `action`, `executor`, `kind`, `argv`, `argv[0]`, `arguments` and `a`).
#[test]
fn the_value_bound_is_exact() {
    let list = |n: usize| gate(&format!("    arguments: {{a: [{}]}}\n", "1,".repeat(n)));
    let before = 12;
    assert!(decode(&list(MAX_NODES - before)).is_ok());
    assert!(decode(&list(MAX_NODES - before + 1)).is_err());
}

/// RED. The node count bounds values, not bytes: one 100 KB scalar named by an anchor and used 800
/// times is 80 MB of strings from an 103 KB document, and a 1 MiB document reaches 50 GB (100 000
/// uses of a 500 KB scalar).
#[test]
fn an_alias_to_a_large_scalar_is_bounded_in_bytes() {
    let big = "x".repeat(100 * 1024);
    let uses = "*a,".repeat(800);
    let text = gate(&format!(
        "    arguments: {{base: &a \"{big}\", all: [{uses}]}}\n"
    ));
    assert!(text.len() < 110 * 1024);
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        fn bytes(value: &Json) -> usize {
            match value {
                Json::Text(text) => text.len(),
                Json::Array(items) => items.iter().map(bytes).sum(),
                Json::Object(members) => members.iter().map(|(_, v)| bytes(v)).sum(),
                _ => 0,
            }
        }
        let total = match decode(&text) {
            Ok(doc) => doc.actions[0].arguments.as_ref().map_or(0, bytes),
            Err(_) => 0,
        };
        let _ = sender.send(total);
    });
    let total = receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("decode returns");
    assert!(
        total <= 64 * 1024 * 1024,
        "decoded {total} bytes of strings from a 103 KB document"
    );
}

/// The classic exponential alias document is refused, and quickly.
#[test]
fn an_exponential_alias_document_is_refused_in_time() {
    let mut text = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
    for level in 1..9 {
        let previous = level - 1;
        let uses = vec![format!("*a{previous}"); 10].join(", ");
        text.push_str(&format!("a{level}: &a{level} [{uses}]\n"));
    }
    let text = gate(&format!(
        "    arguments:\n{}",
        text.lines()
            .map(|line| format!("      {line}\n"))
            .collect::<String>()
    ));
    assert_eq!(refused_within(text, 30), Some(true));
}

/// Realistic prose in an agent's text: code, JSON, a regular expression, markdown, comments, a
/// quoted `&name` and `*name`, a `#` inside a string. All balanced, all valid YAML.
#[test]
fn realistic_prose_with_brackets_and_yaml_punctuation_loads() {
    let json_skeleton = r#"{
  "findings": [
    {"file": "src/lib.rs", "line": 12, "tags": ["a", "b"]},
    {"file": "src/main.rs", "line": 3, "tags": []}
  ],
  "summary": {"state": "passed", "counts": {"error": 0, "warning": [1, 2]}}
}"#;
    let mut prose = String::new();
    for _ in 0..8 {
        prose.push_str("Return JSON shaped like this:\n");
        prose.push_str(json_skeleton);
        prose.push('\n');
        prose.push_str("Match lines with ^\\[(\\d+)\\] (.*)$ and see [the guide](https://example.test/a_(b)).\n");
        prose.push_str("fn parse(c: char) -> bool { matches!(c, '[' | ']' | '{' | '}') }\n");
        prose.push_str("- [ ] a checklist item with a # hash and an & and a *star*\n");
        prose.push_str("| col | [x] | {y} |\n");
    }
    assert!(prose.len() > 3000, "{}", prose.len());
    let body: String = prose
        .lines()
        .map(|line| format!("      {line}\n"))
        .collect();
    let text = gate(&format!(
        "    # a comment with [brackets] and {{braces}}\n    handoff: |\n{body}    arguments: {{note: \"x &name *name # not a comment [\", other: 'it''s {{'}}\n"
    ));
    let doc = decode(&text).expect("decodes");
    assert_eq!(
        doc.actions[0].handoff.as_deref().map(str::trim_end),
        Some(prose.trim_end())
    );
}

/// RED. The smallest realistic text refused wrongly: a listing of 62 lines of Go or Rust function
/// headers whose bodies are left out, in a block scalar. Valid YAML; the prescan nets 62 unclosed
/// braces across the lines and counts the block scalar's text as nesting.
#[test]
fn a_listing_of_headers_with_unclosed_braces_in_a_block_scalar_loads() {
    let body: String = (0..62)
        .map(|i| format!("      func handler{i}(w http.ResponseWriter) {{\n"))
        .collect();
    let text = gate(&format!("    handoff: |\n{body}"));
    let result = decode(&text);
    assert!(result.is_ok(), "{}", result.err().unwrap());
}

/// RED. Comment lines are not data: 70 comments that open a bracket are valid YAML.
#[test]
fn comment_lines_with_an_open_bracket_do_not_count_as_nesting() {
    let comments: String = (0..70).map(|i| format!("    # note {i} [see\n")).collect();
    let text = gate(&format!("{comments}    handoff: done\n"));
    let result = decode(&text);
    assert!(result.is_ok(), "{}", result.err().unwrap());
}

/// RED. Indentation inside a block scalar is text: a line of 140 leading spaces (an aligned table
/// or a diagram) is content and not a level of nesting.
#[test]
fn indentation_inside_a_block_scalar_is_text() {
    let text = gate(&format!(
        "    handoff: |\n      first\n      {}wide\n",
        " ".repeat(140)
    ));
    let result = decode(&text);
    assert!(result.is_ok(), "{}", result.err().unwrap());
}

/// Nesting by one column of indentation a level (the prescan counts half a level for it) is still
/// refused past the cap by the depth check on the values, and quickly.
#[test]
fn nesting_by_single_spaces_is_refused_past_the_cap() {
    let mut lines = String::from("    arguments:\n");
    for level in 0..100 {
        lines.push_str(&format!("{}k{level}:\n", " ".repeat(5 + level)));
    }
    lines.push_str(&format!("{}leaf: 1\n", " ".repeat(105)));
    assert_eq!(refused_within(gate(&lines), 20), Some(true));
}

/// Block entries separated by a tab (`-<TAB>-<TAB>`) are nesting the prescan does not count as
/// dashes; the document is refused quickly all the same.
#[test]
fn tab_separated_block_entries_are_refused_in_time() {
    let text = format!("{HEAD}actions:\n{}", "-\t".repeat(500_000));
    assert_eq!(refused_within(text, 20), Some(true));
}

/// Complex-key nesting (`? ? ? ...`) carries no bracket and no dash.
#[test]
fn nested_complex_keys_are_refused_in_time() {
    let text = format!("{HEAD}actions:\n{}", "? ".repeat(500_000));
    assert_eq!(refused_within(text, 20), Some(true));
}

/// A deep flow collection on a document whose lines end with a bare carriage return (no `\n`).
#[test]
fn carriage_return_line_ends_do_not_hide_deep_nesting() {
    let text = gate(&format!("    arguments: {}\r", "[".repeat(100_000))).replace('\n', "\r");
    assert_eq!(refused_within(text, 20), Some(true));
}

/// Brackets in a quoted key are counted too (a count that errs toward refusing), quickly.
#[test]
fn a_quoted_key_full_of_brackets_is_refused_in_time() {
    let text = gate(&format!(
        "    arguments: {{\"{}\": 1}}\n",
        "[".repeat(100_000)
    ));
    assert_eq!(refused_within(text, 20), Some(true));
}

/// The value depth bound is exact: sixty values deep in `arguments` is 64 levels, one more is 65.
#[test]
fn the_nesting_bound_is_exact() {
    let at = |n: usize| {
        gate(&format!(
            "    arguments: {{a: {}1{}}}\n",
            "[".repeat(n),
            "]".repeat(n)
        ))
    };
    assert!(decode(&at(MAX_NESTING - 4)).is_ok());
    assert!(decode(&at(MAX_NESTING - 3)).is_err());
}

/// A mapping key is the text written, for every spelling, with no panic: the table keeps `0.0`,
/// `0x0`, `00`, `+0`, `~`, `null`, `yes`, `no`, `on` and `true` as distinct values.
#[test]
fn result_keys_are_the_text_written_for_every_spelling() {
    let keys = [
        "0.0", "0x0", "00", "+0", "~", "null", "yes", "no", "on", "true",
    ];
    let table = format!(
        "{{{}}}",
        keys.iter()
            .map(|k| format!("{k}: x"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let doc = decode(&results_of(&table)).expect("decodes");
    let values: Vec<&str> = doc.actions[0].evidence[0]
        .results
        .iter()
        .map(|rule| rule.value.as_str())
        .collect();
    assert_eq!(values, keys);
}

/// The rule of `parse_registration` in `crates/loom-protocols/src/lib.rs:243-256`, copied (the
/// function is private).
fn registry_accepts(name: &str) -> bool {
    let Some((stem, version)) = name.split_once('@') else {
        return false;
    };
    if stem.is_empty()
        || stem.len() > 100
        || !stem.as_bytes()[0].is_ascii_lowercase()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return false;
    }
    let Ok(major) = version.parse::<u64>() else {
        return false;
    };
    major != 0 && major.to_string() == version
}

/// `decode` answers as the registry's rule does on 66 000 generated protocol names and a list of
/// edge cases.
#[test]
fn registration_agrees_with_the_registry_rule() {
    let alphabet = ["a", "Z", "0", "1", "-", "_", ".", "@", "+"];
    let mut names: Vec<String> = vec![String::new()];
    let mut layer: Vec<String> = vec![String::new()];
    for _ in 0..5 {
        layer = layer
            .iter()
            .flat_map(|prefix| alphabet.iter().map(move |c| format!("{prefix}{c}")))
            .collect();
        names.extend(layer.iter().cloned());
    }
    for edge in [
        format!("{}@1", "a".repeat(100)),
        format!("{}@1", "a".repeat(101)),
        "a@18446744073709551615".to_owned(),
        "a@18446744073709551616".to_owned(),
        "a@+1".to_owned(),
        "a@1 ".to_owned(),
        "a@\u{661}".to_owned(),
        "\u{e9}@1".to_owned(),
        "a@-1".to_owned(),
        "a@1@2".to_owned(),
        "a-@1".to_owned(),
        "a--b9@10".to_owned(),
    ] {
        names.push(edge);
    }
    let mut disagree = Vec::new();
    for name in &names {
        let text = format!("format: loom.bindings/1\nprotocol: \"{name}\"\n");
        if decode(&text).is_ok() != registry_accepts(name) {
            disagree.push(name.clone());
        }
    }
    assert!(
        disagree.is_empty(),
        "{} disagree: {:?}",
        disagree.len(),
        &disagree[..disagree.len().min(10)]
    );
}

/// `retry_when` names an evidence kind of the protocol, one the action may not itself produce.
#[test]
fn a_retry_kind_declared_for_another_action_loads() {
    let text = gate("    retry_when: {implementation_change: [failed]}\n");
    assert!(refused(&text).is_none());
}

/// An agent's and a copy's entries may read `effect`; a program's `exit` and a pointer load.
#[test]
fn the_effect_source_loads_on_an_agent_and_a_copy_and_not_on_a_program() {
    let entry = |executor: &str, from: &str| {
        let results = if from == "effect" {
            "{performed: opened}"
        } else {
            "{\"0\": opened}"
        };
        format!(
            "{HEAD}actions:\n  - action: change.open\n    executor: {executor}\n    evidence:\n      - {{kind: ess_gate, subject: impl, producer: p, from: \"{from}\", results: {results}}}\n"
        )
    };
    assert!(refused(&entry("{kind: agent, adapter: a}", "effect")).is_none());
    assert!(
        refused(&entry(
            "{kind: copy, evidence: ess_gate, from: other@1}",
            "effect"
        ))
        .is_none()
    );
    assert!(refused(&entry("{kind: program, argv: [a]}", "effect")).is_some());
    assert!(refused(&entry("{kind: program, argv: [a]}", "/r/s")).is_none());
}

/// A file with Windows line ends decodes to the same document as one with Unix line ends.
#[test]
fn windows_line_ends_decode_like_unix_ones() {
    let unix = gate("    handoff: |\n      one\n      two\n    timeout_seconds: 60\n");
    let windows = unix.replace('\n', "\r\n");
    let a = decode(&unix).expect("unix");
    let b = decode(&windows).expect("windows");
    assert_eq!(a.actions[0].timeout_seconds, b.actions[0].timeout_seconds);
    assert_eq!(a.actions[0].handoff.as_deref(), Some("one\ntwo\n"));
    assert_eq!(b.actions[0].handoff.as_deref(), Some("one\ntwo\n"));
}
