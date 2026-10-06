//! Adversary cases for PR 16 (story:result-references): the 4 KiB encoded-reference cap on
//! `Briefing::read_result` ("Encoded read references are capped at 4 KiB").
//!
//! The PR's `selector::tests::tiny_selection_cannot_amplify_an_oversized_reference` uses a
//! 5001-byte pointer and asserts the error contains "4096". `ResultStore::select` (the call
//! `read_result` makes after its cap) already refuses that pointer with "pointer exceeds 4096
//! bytes", so the test passes with the cap deleted. The cases below express that mutant inside a
//! test (the store's `select` is `read_result` without the cap) and give the cap an input only it
//! refuses.

use b10x_loom_intake_slice::results::ResultStore;
use b10x_loom_intake_slice::selector::Briefing;
use intake_model::results::Capture;
use serde_json::{Value, json};

/// A reference that `ResultStore::select` accepts field by field: a 2100-byte pointer of quotes
/// is under the 4096-byte pointer limit, and JSON-encodes to more than 4096 bytes.
fn escaped_reference(store: &mut ResultStore) -> Value {
    let stored = store
        .insert("json", r#"{"a":"x"}"#.to_owned(), Capture::Complete)
        .unwrap();
    json!({"result": stored.result_id, "sha256": stored.sha256,
        "select": {"kind": "json_pointer", "pointer": format!("/{}", "\"".repeat(2100))},
        "rendering": "text"})
}

/// Mutant evidence: the input of the PR's test is refused with a "4096" message by the uncapped
/// path alone, so that test's `contains("4096")` holds whether or not the cap exists.
#[test]
fn the_prs_amplification_input_is_refused_with_4096_even_without_the_encoded_cap() {
    let mut store = ResultStore::new();
    let key = "a".repeat(5000);
    let stored = store
        .insert(
            "json",
            json!({key.clone(): "x"}).to_string(),
            Capture::Complete,
        )
        .unwrap();
    let reference = json!({"result": stored.result_id, "sha256": stored.sha256,
        "select": {"kind": "json_pointer", "pointer": format!("/{key}")}, "rendering": "text"});
    let uncapped = store.select(&reference, 8192).unwrap_err();
    assert!(
        uncapped.contains("4096"),
        "the field limit alone satisfies the PR test's assertion: {uncapped}"
    );
}

/// The rewritten case: only the encoded cap refuses this reference. Against the mutant (the
/// store's `select`, no cap) the refusal is a missing pointer member, not the cap.
#[test]
fn only_the_encoded_cap_refuses_a_reference_within_every_field_limit() {
    let mut store = ResultStore::new();
    let reference = escaped_reference(&mut store);
    assert!(reference["select"]["pointer"].as_str().unwrap().len() <= 4096);
    assert!(reference.to_string().len() > 4096);
    let uncapped = store.select(&reference, 8192).unwrap_err();
    assert!(
        !uncapped.contains("encoded"),
        "without the cap the reference parses and fails on its pointer: {uncapped}"
    );

    let briefing = Briefing::new("intent", vec![]);
    let refused = briefing.read_result(&reference).unwrap_err();
    assert_eq!(refused, "encoded result reference exceeds 4096 bytes");
}
