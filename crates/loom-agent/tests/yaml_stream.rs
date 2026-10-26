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

#[test]
fn a_stream_without_a_credential_is_clean() {
    for text in [
        "",
        "kind: run\n",
        "kind: run\n---\nkind: review\n---\n- a\n- b\n",
        "base_url = \"http://model-route.agents.svc:8080/v1\"\n",
        "a: [1\n",
    ] {
        assert_eq!(yaml_stream_credential(text), StreamWalk::Clean, "{text:?}");
    }
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
}

#[test]
fn a_scan_error_ends_the_stream_after_the_documents_before_it() {
    assert_eq!(
        yaml_stream_credential("token: \"\\x67hp_000fake\"\n---\na: [1\n"),
        found(0, "token", "a value with the prefix `ghp_`"),
        "a reader that reads document by document gets the first one"
    );
    assert_eq!(
        yaml_stream_credential("kind: run\n---\na: [1\n---\ntoken: ghp_000fake\n"),
        StreamWalk::Clean,
        "no YAML reader reads past the scan error"
    );
}

#[test]
fn a_stream_the_walk_cannot_read_whole_is_too_large() {
    let items = vec!["x"; 100].join(", ");
    let aliases = vec!["*a"; 100].join(", ");
    let laughs = format!("anchor: &a [{items}]\nrepeated: [{aliases}]\n");
    assert_eq!(yaml_stream_credential(&laughs), StreamWalk::TooLarge);
    assert_eq!(
        yaml_stream_credential(&format!("kind: run\n---\n{laughs}")),
        StreamWalk::TooLarge,
        "the budget is one for the whole stream"
    );
    assert_eq!(
        yaml_stream_credential(&format!("token: \"\\x67hp_000fake\"\n{laughs}")),
        found(0, "token", "a value with the prefix `ghp_`"),
        "a credential found before the walk stopped is the answer"
    );
    let deep = format!("{}x{}", "[".repeat(200), "]".repeat(200));
    assert_eq!(
        yaml_stream_credential(&deep),
        StreamWalk::TooLarge,
        "nesting beyond the parser's recursion limit"
    );
    assert_eq!(
        yaml_stream_credential(&"{".repeat(4097)),
        StreamWalk::TooLarge,
        "beyond the decoder's flow bound"
    );
}
