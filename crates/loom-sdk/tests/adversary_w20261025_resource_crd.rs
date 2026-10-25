//! Adversary cases for `story:loom-agent-resource-domain` (wave 2026-10-25-resource): the committed
//! `LoomAgent` CRD (`deploy/crd/loomagents.loom.beyond10x.dev.yaml`) driven against the decoder the
//! pod runs, `loom_sdk::agent::parse`, on `examples/agents/operator.yaml` and on one-member changes
//! of it. `crates/loom-resource/src/crd.rs` and AGENTS.md § Resource say the schema follows the
//! decoder: "a list is never required: the decoder reads an absent list as an empty one", and the
//! union is "flattened to the wire form Loom's decoder reads".
//!
//! The schema of `spec.agent` is read as the API server reads it, by a validator written here for
//! the keywords the projection emits: a member an object does not declare is pruned, not refused;
//! a `null` member is pruned; `format` and `pattern` (string grammar) are not modelled; any other
//! keyword stops the test, so the validator never silently ignores what a projection adds. Each
//! case first calibrates it: the decoder and the schema both accept the operator example.

use std::path::PathBuf;

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The schema of `spec.agent`, from the committed CRD.
fn agent_schema() -> Value {
    let crd: Value =
        serde_yaml_ng::from_str(&read("deploy/crd/loomagents.loom.beyond10x.dev.yaml"))
            .expect("the CRD is YAML");
    crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["agent"]
        .clone()
}

/// `examples/agents/operator.yaml`, the document `spec.agent` holds verbatim.
fn operator() -> Value {
    serde_yaml_ng::from_str(&read("examples/agents/operator.yaml")).expect("the example is YAML")
}

/// The decoder's answer: `Ok(())`, or its refusal.
fn decoded(document: &Value) -> Result<(), String> {
    let text = serde_json::to_string(document).expect("JSON");
    loom_sdk::agent::parse(&text)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

const MODELLED: [&str; 16] = [
    "type",
    "enum",
    "properties",
    "required",
    "items",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "x-kubernetes-preserve-unknown-fields",
];

const NOT_MODELLED: [&str; 3] = ["format", "pattern", "description"];

fn join(at: &str, name: &str) -> String {
    format!("{at}.{name}")
}

fn passes(schema: &Value, value: &Value) -> bool {
    let mut found = Vec::new();
    validate(schema, value, "", &mut found);
    found.is_empty()
}

fn bound(node: &Value, key: &str) -> Option<f64> {
    node.get(key).and_then(Value::as_f64)
}

/// What the API server refuses of `value` under `schema`, by path; empty means admitted.
fn validate(schema: &Value, value: &Value, at: &str, found: &mut Vec<String>) {
    let node = schema
        .as_object()
        .unwrap_or_else(|| panic!("{at}: a schema that is not a map"));
    for key in node.keys() {
        assert!(
            MODELLED.contains(&key.as_str()) || NOT_MODELLED.contains(&key.as_str()),
            "{at}: the validator does not model `{key}`"
        );
    }
    if let Some(kind) = node.get("type").and_then(Value::as_str) {
        let typed = match kind {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            other => panic!("{at}: type `{other}`"),
        };
        if !typed {
            found.push(format!("{at}: not {kind}"));
            return;
        }
    }
    if let Some(allowed) = node.get("enum").and_then(Value::as_array)
        && !allowed.contains(value)
    {
        found.push(format!("{at}: {value} is not in the enum"));
    }
    if let Some(number) = value.as_f64() {
        if bound(schema, "minimum").is_some_and(|minimum| number < minimum) {
            found.push(format!("{at}: below minimum"));
        }
        if bound(schema, "maximum").is_some_and(|maximum| number > maximum) {
            found.push(format!("{at}: above maximum"));
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as f64;
        if bound(schema, "minLength").is_some_and(|minimum| length < minimum) {
            found.push(format!("{at}: shorter than minLength"));
        }
        if bound(schema, "maxLength").is_some_and(|maximum| length > maximum) {
            found.push(format!("{at}: longer than maxLength"));
        }
    }
    if let Some(map) = value.as_object() {
        for name in node
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if map.get(name).is_none_or(Value::is_null) {
                found.push(format!("{}: missing", join(at, name)));
            }
        }
        if let Some(properties) = node.get("properties").and_then(Value::as_object) {
            for (name, member) in map {
                // An undeclared member is pruned; a null member of a non-nullable field is pruned.
                if let Some(property) = properties.get(name)
                    && !member.is_null()
                {
                    validate(property, member, &join(at, name), found);
                }
            }
        }
    }
    if let Some(list) = value.as_array() {
        let length = list.len() as f64;
        if bound(schema, "minItems").is_some_and(|minimum| length < minimum) {
            found.push(format!("{at}: fewer than minItems"));
        }
        if bound(schema, "maxItems").is_some_and(|maximum| length > maximum) {
            found.push(format!("{at}: more than maxItems"));
        }
        if let Some(items) = node.get("items") {
            for (index, item) in list.iter().enumerate() {
                validate(items, item, &format!("{at}[{index}]"), found);
            }
        }
    }
    let entries = |junctor: &str| -> Vec<Value> {
        node.get(junctor)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let one_of = entries("oneOf");
    if !one_of.is_empty() {
        let passing = one_of.iter().filter(|entry| passes(entry, value)).count();
        if passing != 1 {
            found.push(format!(
                "{at}: {passing} of {} oneOf entries hold",
                one_of.len()
            ));
        }
    }
    let any_of = entries("anyOf");
    if !any_of.is_empty() && !any_of.iter().any(|entry| passes(entry, value)) {
        found.push(format!("{at}: no anyOf entry holds"));
    }
    for entry in entries("allOf") {
        validate(&entry, value, at, found);
    }
    if let Some(negated) = node.get("not")
        && passes(negated, value)
    {
        found.push(format!("{at}: the `not` schema holds"));
    }
}

fn admitted(document: &Value) -> Result<(), String> {
    let mut found = Vec::new();
    validate(&agent_schema(), document, "", &mut found);
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("; "))
    }
}

fn calibrate(example: &Value) {
    assert_eq!(
        decoded(example),
        Ok(()),
        "the decoder reads the operator example"
    );
    assert_eq!(
        admitted(example),
        Ok(()),
        "the schema admits the operator example"
    );
}

/// Every member of every mapping in `value`, as `(pointer to its mapping, its name)`.
fn members(value: &Value, at: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (name, member) in map {
                out.push((at.to_owned(), name.clone()));
                members(member, &format!("{at}/{name}"), out);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                members(item, &format!("{at}/{index}"), out);
            }
        }
        _ => {}
    }
}

fn verdict(answer: &Result<(), String>) -> String {
    match answer {
        Ok(()) => "admits it".to_owned(),
        Err(why) => format!("refuses it ({why})"),
    }
}

/// Remove one member at a time, anywhere in the operator example: the decoder and the schema must
/// give the same answer. AGENTS.md § Resource: the schema "never requires a list", following
/// `loom_sdk::agent::parse`, "which reads ... an absent list as empty".
#[test]
fn the_crd_and_the_decoder_agree_on_every_member_the_operator_example_can_omit() {
    let example = operator();
    calibrate(&example);
    let mut all = Vec::new();
    members(&example, "", &mut all);
    let mut disagreements = Vec::new();
    for (mapping, name) in &all {
        let mut copy = example.clone();
        copy.pointer_mut(mapping)
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| panic!("{mapping} is a mapping"))
            .remove(name);
        let decoder = decoded(&copy);
        let schema = admitted(&copy);
        if decoder.is_ok() != schema.is_ok() {
            disagreements.push(format!(
                "without {mapping}/{name}: the decoder {}, the CRD schema {}",
                verdict(&decoder),
                verdict(&schema)
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "the CRD schema and the decoder disagree on {} of {} omissions:\n{}",
        disagreements.len(),
        all.len(),
        disagreements.join("\n")
    );
}

/// A binding carrying a member of the other variant: the decoder refuses it as an undeclared
/// member, and the flattened union declares that member beside the tag, so the API server neither
/// prunes nor refuses it.
#[test]
fn the_crd_refuses_a_binding_that_carries_a_member_of_the_other_variant() {
    let example = operator();
    calibrate(&example);
    let bindings = example["bindings"].as_array().expect("bindings");
    let connectors = bindings
        .iter()
        .position(|binding| binding["kind"] == "connectors")
        .expect("a connectors binding");
    let http = bindings
        .iter()
        .position(|binding| binding["kind"] == "http")
        .expect("an http binding");
    let mut disagreements = Vec::new();
    let mut tried = 0;
    for (into, from) in [(connectors, http), (http, connectors)] {
        let own = bindings[into].as_object().expect("binding");
        let other = bindings[from].as_object().expect("binding");
        for (name, value) in other.iter().filter(|(name, _)| !own.contains_key(*name)) {
            tried += 1;
            let mut copy = example.clone();
            copy["bindings"][into][name] = value.clone();
            let decoder = decoded(&copy);
            let schema = admitted(&copy);
            if decoder.is_ok() != schema.is_ok() {
                disagreements.push(format!(
                    "bindings[{into}] (kind {}) with `{name}`: the decoder {}, the CRD schema {}",
                    bindings[into]["kind"],
                    verdict(&decoder),
                    verdict(&schema)
                ));
            }
        }
    }
    assert!(tried > 0, "the example's two bindings differ in members");
    assert!(
        disagreements.is_empty(),
        "the CRD schema admits {} of {tried} cross-variant bindings the decoder refuses:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}
