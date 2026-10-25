//! Adversary cases for `story:loom-agent-resource-domain`, pass 2: the keyword table of
//! `crates/loom-resource/src/crd.rs` after the correction ("Every keyword of a schema node is
//! carried, rewritten to an equivalent, dropped as an annotation or refused by name; none is passed
//! over unexamined"; "a map keeps its entries").
//!
//! The input is what ESS 0.69.0 writes (`ess generate --kind schema`, minus `x-ess-provenance`) for
//! a one-domain specification declaring `Labels` (a newtype of `Map<String, String>`), `Raw` (a
//! newtype of `Json`), a struct `S`, a union `U` (tag `kind`, variants `labels: Labels`,
//! `raw: Raw`, `s: S`) and a struct `Spec` with `u: U` and `values: Map<String, Json>`.
//!
//! What the API server keeps of an object it validates: a member the node declares in `properties`,
//! or any member when the node preserves unknown fields or carries an `additionalProperties`
//! schema; every other member is pruned.

use b10x_loom_resource::crd;
use serde_json::{Value, json};

fn definitions() -> Value {
    json!({
        "probe.x.Labels": {"title": "Labels", "x-ess-name": "probe.x.Labels", "x-ess-kind": "newtype", "type": "object", "additionalProperties": {"type": "string"}, "x-ess-map-key": "String"},
        "probe.x.Raw": {"title": "Raw", "x-ess-name": "probe.x.Raw", "x-ess-kind": "newtype"},
        "probe.x.S": {"title": "S", "x-ess-name": "probe.x.S", "x-ess-kind": "struct", "type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"], "additionalProperties": false},
        "probe.x.Spec": {"title": "Spec", "x-ess-name": "probe.x.Spec", "x-ess-kind": "struct", "type": "object", "properties": {"u": {"$ref": "#/$defs/probe.x.U"}, "values": {"type": "object", "additionalProperties": {}, "x-ess-map-key": "String"}}, "required": ["u", "values"], "additionalProperties": false},
        "probe.x.U": {"title": "U", "x-ess-name": "probe.x.U", "x-ess-kind": "union", "oneOf": [
            {"title": "labels", "type": "object", "properties": {"kind": {"type": "string", "const": "labels"}, "value": {"$ref": "#/$defs/probe.x.Labels"}}, "required": ["kind", "value"], "additionalProperties": false},
            {"title": "raw", "type": "object", "properties": {"kind": {"type": "string", "const": "raw"}, "value": {"$ref": "#/$defs/probe.x.Raw"}}, "required": ["kind", "value"], "additionalProperties": false},
            {"title": "s", "type": "object", "properties": {"kind": {"type": "string", "const": "s"}, "value": {"$ref": "#/$defs/probe.x.S"}}, "required": ["kind", "value"], "additionalProperties": false},
        ], "x-ess-union-tag": "kind"},
    })
}

/// The `Spec` document with only the members of `Spec` named in `keep`, and `U` with only the
/// variants named in `variants`.
fn spec(keep: &[&str], variants: &[&str]) -> Value {
    let mut definitions = definitions();
    let spec = &mut definitions["probe.x.Spec"];
    let properties = spec["properties"].as_object_mut().expect("properties");
    properties.retain(|name, _| keep.contains(&name.as_str()));
    spec["required"] = json!(keep);
    definitions["probe.x.U"]["oneOf"]
        .as_array_mut()
        .expect("oneOf")
        .retain(|variant| {
            variant["properties"]["kind"]["const"]
                .as_str()
                .is_some_and(|tag| variants.contains(&tag))
        });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/probe.x.Spec",
        "$defs": definitions,
    })
}

fn entity() -> Value {
    json!({
        "type": "object",
        "properties": {"name": {"type": "string"}, "state": {"type": "string", "enum": ["Pending"]}},
        "required": ["name", "state"],
        "additionalProperties": false,
    })
}

/// The projected `spec` schema of a manifest that projected.
fn spec_schema(manifest: &str) -> Value {
    let document: Value = serde_yaml_ng::from_str(manifest).expect("YAML");
    document["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"].clone()
}

/// Whether the API server keeps `member` of an object validated against `node`.
fn kept(node: &Value, member: &str) -> bool {
    node["properties"].get(member).is_some()
        || node.get("x-kubernetes-preserve-unknown-fields") == Some(&json!(true))
        || node
            .get("additionalProperties")
            .is_some_and(Value::is_object)
}

/// A union variant whose value is a map, or any JSON value, has members the flattened union does
/// not declare: the projection has to keep them or refuse the variant, never let the API server
/// prune them while it reports nothing.
#[test]
fn a_union_variant_holding_a_map_or_json_is_kept_or_refused_never_pruned() {
    let calibration = crd::manifest(&spec(&["u"], &["s"]), &entity())
        .unwrap_or_else(|e| panic!("calibration: a union of one struct variant projects: {e}"));
    assert!(
        !kept(&spec_schema(&calibration)["properties"]["u"], "team"),
        "calibration: an undeclared member of a struct variant is pruned"
    );

    let mut pruned = Vec::new();
    for (variant, member) in [("labels", "team"), ("raw", "anything")] {
        match crd::manifest(&spec(&["u"], &[variant, "s"]), &entity()) {
            Err(refused) => assert!(!refused.to_string().is_empty()),
            Ok(manifest) => {
                let union = &spec_schema(&manifest)["properties"]["u"];
                if !kept(union, member) {
                    pruned.push(format!(
                        "`u` with kind `{variant}` carries `{member}`, a member of the variant's \
                         value; the flattened union {union} declares no such member and neither \
                         preserves unknown fields nor carries the value's schema"
                    ));
                }
            }
        }
    }
    assert!(
        pruned.is_empty(),
        "the API server prunes these and the projection reported nothing:\n{}",
        pruned.join("\n")
    );
}

/// `Map<String, Json>`: a map's value is any JSON value, as the specification's `Json` is.
#[test]
fn a_map_of_json_admits_every_json_value() {
    let manifest = crd::manifest(&spec(&["values"], &["s"]), &entity())
        .unwrap_or_else(|e| panic!("a map of Json projects: {e}"));
    let schema = spec_schema(&manifest);
    let validator = jsonschema::validator_for(&schema).expect("the projected spec is JSON Schema");
    let document = json!({"values": {"object": {"a": 1}, "number": 1, "text": "x", "list": [1], "flag": true}});
    let refused: Vec<String> = validator
        .iter_errors(&document)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert_eq!(
        refused,
        Vec::<String>::new(),
        "the projected `values` {} refuses JSON values the specification's `Json` admits",
        schema["properties"]["values"]
    );
}
