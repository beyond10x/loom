//! Adversary case for `story:loom-agent-resource-domain`: `crd::manifest` on a construct ESS
//! projects and `crates/loom-resource/src/crd.rs` claims to handle ("Any other keyword is refused
//! as `CrdError::Keyword` rather than carried over unexamined").
//!
//! ESS 0.69.0 writes a `Map<String, V>` field as `{"type": "object", "additionalProperties": V,
//! "x-ess-map-key": "String"}` (measured: `ess generate --kind schema` over a one-struct
//! specification declaring `labels: Map<String, String>`; the document below is its output, minus
//! `x-ess-provenance`). The projection drops every `additionalProperties` as if it were the
//! struct's `false`, so a map projects to `{"type": "object"}`: an object that declares no
//! property and does not preserve unknown fields, whose every entry the API server prunes.

use b10x_loom_resource::crd;
use serde_json::{Value, json};

fn spec(with_map: bool) -> Value {
    let mut properties = json!({"name": {"type": "string"}});
    let mut required = vec!["name"];
    if with_map {
        properties["labels"] = json!({
            "type": "object",
            "additionalProperties": {"type": "string"},
            "x-ess-map-key": "String",
        });
        required.insert(0, "labels");
    }
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/probe.x.Spec",
        "$defs": {
            "probe.x.Spec": {
                "title": "Spec",
                "x-ess-name": "probe.x.Spec",
                "x-ess-kind": "struct",
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            },
        },
    })
}

fn entity() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "state": {"type": "string", "enum": ["Pending"]},
        },
        "required": ["name", "state"],
        "additionalProperties": false,
    })
}

#[test]
fn a_map_field_is_carried_or_refused_never_projected_to_an_object_that_prunes_it() {
    crd::manifest(&spec(false), &entity())
        .unwrap_or_else(|e| panic!("calibration: the spec without the map projects: {e}"));

    match crd::manifest(&spec(true), &entity()) {
        Err(refused) => {
            // Refusing a construct it cannot carry is what crd.rs promises; nothing is lost.
            assert!(!refused.to_string().is_empty());
        }
        Ok(manifest) => {
            let document: Value = serde_yaml_ng::from_str(&manifest).expect("YAML");
            let labels = &document["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
                ["spec"]["properties"]["labels"];
            let keeps_entries = labels.get("additionalProperties")
                == Some(&json!({"type": "string"}))
                || labels.get("x-kubernetes-preserve-unknown-fields") == Some(&json!(true));
            assert!(
                keeps_entries,
                "`labels` (Map<String, String>) projects to {labels}: an object that declares no \
                 property and does not preserve unknown fields, so the API server prunes every \
                 entry, and the projection reported nothing"
            );
        }
    }
}
