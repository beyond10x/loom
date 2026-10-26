//! `yaml_stream_credential` walks every document of a YAML stream that is not a `loom.agent/1`
//! document, and answers `TooLarge` rather than "no credential" for a stream it cannot read whole
//! (`story:command-opaque-config-files`). Every token here is a clearly fake one.

use b10x_loom_agent::{StreamWalk, yaml_stream_credential};

fn found(document: usize, member: &str, shape: &str) -> StreamWalk {
    StreamWalk::Found {
        document,
        member: member.to_owned(),
        shape: shape.to_owned(),
    }
}

fn clean(strings: &[&str]) -> StreamWalk {
    StreamWalk::Clean {
        strings: strings.iter().map(|text| (*text).to_owned()).collect(),
    }
}

#[test]
fn a_stream_without_a_credential_is_clean() {
    for text in [
        "",
        "kind: run\n",
        "kind: run\n---\nkind: review\n---\n- a\n- b\n",
        "base_url = \"http://model-route.agents.svc:8080/v1\"\n",
        "[server]\ntoken_env = \"MODEL_TOKEN\"\n",
        "a: [1\n",
        "a: [1\n---\nkind: run\n",
    ] {
        assert!(
            matches!(yaml_stream_credential(text), StreamWalk::Clean { .. }),
            "{text:?}"
        );
    }
}

#[test]
fn a_clean_walk_answers_every_string_it_read() {
    assert_eq!(
        yaml_stream_credential("a: 1\na: 2\ntoken: \"--auth \\x41\"\n---\n- !tag b\n"),
        clean(&["a", "a", "token", "--auth A", "tag", "b"]),
        "a duplicate key does not stop the walk"
    );
}

#[test]
fn a_credential_in_any_document_is_found_decoded() {
    assert_eq!(
        yaml_stream_credential("kind: run\n---\ntoken: \"\\x67hp_000fake\"\n"),
        found(1, "token", "a value with the prefix `ghp_`")
    );
    assert_eq!(
        yaml_stream_credential("\u{feff}--- !tagged\nkey: xoxb-000-fake\n"),
        found(0, "key", "a value with the prefix `xoxb-`")
    );
    assert_eq!(
        yaml_stream_credential("a: 1\na: 2\n---\ntoken: \"\\x67hp_000fake\"\n"),
        found(1, "token", "a value with the prefix `ghp_`"),
        "a document after one with a duplicate key"
    );
}

#[test]
fn after_a_scan_error_the_text_is_walked_split_at_its_document_markers() {
    assert_eq!(
        yaml_stream_credential("token: \"\\x67hp_000fake\"\n---\na: [1\n"),
        found(0, "token", "a value with the prefix `ghp_`"),
        "a reader that reads document by document gets the first one"
    );
    assert_eq!(
        yaml_stream_credential("kind: run\n---\na: [1\n---\ntoken: \"\\x67hp_000fake\"\n"),
        found(2, "token", "a value with the prefix `ghp_`"),
        "a reader that splits the stream at `---` reads the third piece"
    );
    assert_eq!(
        yaml_stream_credential("a: [1\n--- \t# next\r\ntoken: ghp_000fake\n"),
        found(1, "token", "a value with the prefix `ghp_`"),
        "a marker followed by whitespace and a comment"
    );
    assert_eq!(
        yaml_stream_credential("a: [1\n----\n --- \ntoken: ghp_000fake\n"),
        clean(&["a", "1 ---- --- token"]),
        "neither line is a document marker: the piece reads up to its scan error"
    );
}

#[test]
fn a_stream_the_walk_cannot_read_whole_is_too_large() {
    let items = vec!["x"; 100].join(", ");
    let aliases = vec!["*a"; 100].join(", ");
    let laughs = format!("anchor: &a [{items}]\nrepeated: [{aliases}]\n");
    let too_large =
        |text: &str| matches!(yaml_stream_credential(text), StreamWalk::TooLarge { .. });
    assert!(too_large(&laughs));
    assert!(
        too_large(&format!("kind: run\n---\n{laughs}")),
        "the budget is one for the whole stream"
    );
    assert_eq!(
        yaml_stream_credential(&format!("token: \"\\x67hp_000fake\"\n{laughs}")),
        found(0, "token", "a value with the prefix `ghp_`"),
        "a credential found before the walk stopped is the answer"
    );
    let deep = format!("{}x{}", "[".repeat(200), "]".repeat(200));
    assert!(
        too_large(&deep),
        "nesting beyond the parser's recursion limit"
    );
    assert!(
        too_large(&"{".repeat(4097)),
        "beyond the decoder's flow bound"
    );
    assert_eq!(
        yaml_stream_credential(&format!("a: \"--auth \\x41\"\nb: {deep}\n")),
        StreamWalk::TooLarge {
            strings: vec!["a".to_owned(), "--auth A".to_owned(), "b".to_owned()],
        },
        "it answers the strings it read before it stopped"
    );
}
