//! The `LoomAgent` CustomResourceDefinition is projected from the specification, never written by
//! hand (`story:loom-agent-resource-domain`):
//!
//! * the committed `deploy/crd/loomagents.loom.beyond10x.dev.yaml` is a fixed point of
//!   `crd::manifest` over what `ess generate --kind schema` writes for `ess/`;
//! * the projected document holds no key Kubernetes refuses in a structural schema or that only ESS
//!   reads (`$ref`, `$defs`, `$schema`, `$id`, `const`, `title`, `x-ess-*`, and an
//!   `additionalProperties` that is not a schema), by a scan of the whole document written here,
//!   apart from the projection's own check; a copy with one such key put back fails the scan,
//!   naming it, for each key;
//! * every schema node is typed, and nothing inside a junctor sets `type`, `description`,
//!   `default`, `additionalProperties` or `nullable`, or names a member the enclosing schema does
//!   not declare;
//! * every object requires exactly the members the specification declares required, less the lists
//!   the `loom.agent` document declares (its decoder reads an absent one as empty), compared node
//!   by node with the documents `ess` writes; a list another document declares, such as
//!   `model.roles`, stays required;
//! * `spec.agent` is required; `spec.agent.bindings.items` carries `kind` with
//!   `enum: [connectors, http]` and the fields of both variants beside it, and each variant's entry
//!   refuses a member only another variant declares; `spec.routes`, an ESS `Json`, is untyped and
//!   preserves unknown fields;
//!   `status.state` enumerates exactly the lifecycle's states, read from the compiled
//!   specification; a decimal is a number;
//! * a map (`additionalProperties` holding a schema) keeps its value schema, projected;
//! * `examples/agents/operator.yaml`, wrapped as a `LoomAgent`'s `spec.agent`, validates against
//!   the projected `openAPIV3Schema` as JSON Schema (no cluster), and resources the schema must
//!   refuse do not.
//!
//! The specification is read with the `ess` binary on `PATH`, as `task conform` reads it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_resource::crd::{self, CrdError};
use serde_json::{Value, json};

/// The keys no projected document may hold, wherever they stand as a keyword.
const FORBIDDEN: [&str; 6] = ["$ref", "$defs", "$schema", "$id", "const", "title"];

/// What Kubernetes does not allow inside `allOf`, `anyOf`, `oneOf` or `not` of a structural schema.
const NOT_IN_A_JUNCTOR: [&str; 5] = [
    "type",
    "description",
    "default",
    "additionalProperties",
    "nullable",
];

const JUNCTORS: [&str; 4] = ["oneOf", "anyOf", "allOf", "not"];

/// The document whose decoder (`crates/loom-sdk/src/agent.rs`) reads an absent list as empty, so
/// a list it declares required is not required in the CRD.
const ABSENT_LIST_IS_EMPTY: &str = "loom.agent";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("crd-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    dir
}

fn ess(args: &[&str], out: Option<&Path>) -> Vec<u8> {
    let mut command = Command::new("ess");
    command.current_dir(root()).args(args);
    if let Some(out) = out {
        command.arg(out);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "`ess {}` exited {}:\n{}{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// The two documents `ess generate --kind schema` writes for `ess/`: the spec type's and the
/// entity's.
fn schemas() -> &'static (Value, Value) {
    static SCHEMAS: OnceLock<(Value, Value)> = OnceLock::new();
    SCHEMAS.get_or_init(|| {
        let out = scratch("schema");
        ess(
            &["generate", "--path", "ess", "--kind", "schema", "--out"],
            Some(&out),
        );
        let pair = (
            read_json(&out.join(crd::SPEC_SCHEMA)),
            read_json(&out.join(crd::ENTITY_SCHEMA)),
        );
        let _ = fs::remove_dir_all(&out);
        pair
    })
}

/// A fresh projection of `ess/`, as text.
fn projected_text() -> String {
    let (spec, entity) = schemas();
    crd::manifest(spec, entity).unwrap_or_else(|e| panic!("the projection of ess/ fails: {e}"))
}

/// A fresh projection of `ess/`, read back from its YAML.
fn projected() -> Value {
    serde_yaml_ng::from_str(&projected_text()).expect("the projected manifest is YAML")
}

fn open_api(document: &Value) -> &Value {
    &document["spec"]["versions"][0]["schema"]["openAPIV3Schema"]
}

/// Every forbidden key in `value`, by path. A map that is the value of a `properties` keyword
/// holds property names, which are not keywords: a property may be called `title`.
fn forbidden(value: &Value, at: &str, names: bool, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, member) in map {
                let path = format!("{at}/{key}");
                let not_a_schema = key == "additionalProperties" && !member.is_object();
                if !names
                    && (FORBIDDEN.contains(&key.as_str())
                        || key.starts_with("x-ess-")
                        || not_a_schema)
                {
                    found.push(path.clone());
                }
                forbidden(member, &path, !names && key == "properties", found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                forbidden(item, &format!("{at}/{index}"), false, found);
            }
        }
        _ => {}
    }
}

fn forbidden_keys(document: &Value) -> Vec<String> {
    let mut found = Vec::new();
    forbidden(document, "", false, &mut found);
    found
}

/// What Kubernetes refuses in a structural schema, by path: a node without `type` (unless it
/// preserves unknown fields), `properties` beside an `additionalProperties` schema, and inside a
/// junctor ([`JUNCTORS`]), at any depth, a key of [`NOT_IN_A_JUNCTOR`] or a member the schema
/// outside the junctor does not declare.
fn unstructural(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(node) = schema.as_object() else {
        found.push(format!("{at}: not a schema object"));
        return;
    };
    let preserves = node.get("x-kubernetes-preserve-unknown-fields") == Some(&Value::Bool(true));
    if !node.contains_key("type") && !preserves {
        found.push(format!("{at}: no type"));
    }
    if let Some(properties) = node.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            unstructural(property, &format!("{at}/properties/{name}"), found);
        }
    }
    if let Some(values) = node
        .get("additionalProperties")
        .filter(|values| values.is_object())
    {
        if node.contains_key("properties") {
            found.push(format!("{at}: `properties` beside `additionalProperties`"));
        }
        unstructural(values, &format!("{at}/additionalProperties"), found);
    }
    if let Some(items) = node.get("items") {
        unstructural(items, &format!("{at}/items"), found);
    }
    junctors(node, node, at, found);
}

/// The junctors of `node`, checked against `outside`, the schema they stand in.
fn junctors(
    node: &serde_json::Map<String, Value>,
    outside: &serde_json::Map<String, Value>,
    at: &str,
    found: &mut Vec<String>,
) {
    for junctor in JUNCTORS {
        let entries = match node.get(junctor) {
            None => continue,
            Some(Value::Array(entries)) => entries.clone(),
            Some(single) => vec![single.clone()],
        };
        for (index, entry) in entries.iter().enumerate() {
            let here = if junctor == "not" {
                format!("{at}/not")
            } else {
                format!("{at}/{junctor}/{index}")
            };
            inside_junctor(entry, outside, &here, found);
        }
    }
}

/// One schema inside a junctor, `outside` being the schema it constrains.
fn inside_junctor(
    entry: &Value,
    outside: &serde_json::Map<String, Value>,
    here: &str,
    found: &mut Vec<String>,
) {
    let Some(entry) = entry.as_object() else {
        found.push(format!("{here}: not a schema object"));
        return;
    };
    for key in entry.keys() {
        if NOT_IN_A_JUNCTOR.contains(&key.as_str()) || key.starts_with("x-kubernetes-") {
            found.push(format!("{here}: `{key}` inside a junctor"));
        }
    }
    let declared = outside.get("properties").and_then(Value::as_object);
    let required = entry
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    for name in required {
        if !declared.is_some_and(|properties| properties.contains_key(name)) {
            found.push(format!(
                "{here}: `{name}` is not declared outside the junctor"
            ));
        }
    }
    for (name, property) in entry
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        match declared
            .and_then(|properties| properties.get(name))
            .and_then(Value::as_object)
        {
            None => found.push(format!(
                "{here}: `{name}` is not declared outside the junctor"
            )),
            Some(beside) => inside_junctor(
                property,
                beside,
                &format!("{here}/properties/{name}"),
                found,
            ),
        }
    }
    if let Some(items) = entry.get("items") {
        match outside.get("items").and_then(Value::as_object) {
            None => found.push(format!("{here}: `items` not declared outside the junctor")),
            Some(beside) => inside_junctor(items, beside, &format!("{here}/items"), found),
        }
    }
    junctors(entry, outside, here, found);
}

fn unstructural_nodes(document: &Value) -> Vec<String> {
    let mut found = Vec::new();
    unstructural(open_api(document), "openAPIV3Schema", &mut found);
    found
}

fn names(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("not text: {item}"))
                .to_owned()
        })
        .collect()
}

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("not a map: {value}"))
        .keys()
        .cloned()
        .collect()
}

#[test]
fn the_committed_crd_is_a_fixed_point_of_the_projection() {
    let committed = root().join(crd::FILE);
    let text = fs::read_to_string(&committed).unwrap_or_else(|e| {
        panic!(
            "read {}: {e} (write it with `task crd-generate`)",
            committed.display()
        )
    });
    assert!(
        text == projected_text(),
        "{} differs from the projection of ess/: regenerate it with `task crd-generate`",
        crd::FILE
    );
    assert!(
        text.starts_with(crd::HEADER),
        "the file names its generator"
    );
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"));
    assert_eq!(
        projected_text(),
        projected_text(),
        "the projection is stable"
    );
}

#[test]
fn the_crd_names_the_resource_as_the_design_does() {
    let document = projected();
    assert_eq!(document["apiVersion"], "apiextensions.k8s.io/v1");
    assert_eq!(document["kind"], "CustomResourceDefinition");
    assert_eq!(
        document["metadata"]["name"],
        "loomagents.loom.beyond10x.dev"
    );
    assert_eq!(document["spec"]["group"], "loom.beyond10x.dev");
    assert_eq!(document["spec"]["scope"], "Namespaced");
    assert_eq!(
        document["spec"]["names"],
        json!({"kind": "LoomAgent", "listKind": "LoomAgentList", "plural": "loomagents", "singular": "loomagent"})
    );
    let versions = document["spec"]["versions"].as_array().expect("versions");
    assert_eq!(versions.len(), 1);
    let version = &versions[0];
    assert_eq!(version["name"], "v1alpha1");
    assert_eq!(version["served"], true);
    assert_eq!(version["storage"], true);
    assert_eq!(version["subresources"], json!({"status": {}}));
    assert_eq!(
        version["additionalPrinterColumns"],
        json!([
            {"name": "State", "type": "string", "jsonPath": ".status.state"},
            {"name": "Generation", "type": "integer", "jsonPath": ".status.observedGeneration"},
            {"name": "Age", "type": "date", "jsonPath": ".metadata.creationTimestamp"},
        ])
    );
    let schema = open_api(&document);
    assert_eq!(schema["type"], "object");
    assert_eq!(keys(&schema["properties"]), ["spec", "status"]);
    assert_eq!(names(&schema["required"]), ["spec"]);
}

#[test]
fn the_projection_leaves_no_forbidden_key_and_a_copy_with_one_fails_naming_it() {
    let document = projected();
    assert_eq!(forbidden_keys(&document), Vec::<String>::new());
    assert_eq!(unstructural_nodes(&document), Vec::<String>::new());

    // One copy per forbidden key, each put back where a projection could have left it.
    let mut planted: Vec<String> = FORBIDDEN.iter().map(|key| (*key).to_owned()).collect();
    planted.push("x-ess-kind".to_owned());
    planted.push("additionalProperties".to_owned());
    for key in planted {
        let mut copy = document.clone();
        copy["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
            ["agent"]["properties"]["model"][&key] = json!("#/$defs/loom.models.ModelsConfig");
        let found = forbidden_keys(&copy);
        assert_eq!(
            found,
            [format!(
                "/spec/versions/0/schema/openAPIV3Schema/properties/spec/properties/agent/properties/model/{key}"
            )],
            "a copy holding `{key}` must fail the scan naming it, and only it"
        );
    }

    // A property may be called like a keyword: that is a name, not a key the scan refuses.
    let mut named = document.clone();
    named["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["title"] = json!({"type": "string"});
    assert_eq!(forbidden_keys(&named), Vec::<String>::new());

    // A `$ref` left in place of a whole schema is a node without a type as well.
    let mut referenced = document.clone();
    referenced["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["properties"]
        ["agent"] = json!({"$ref": "#/$defs/loom.agent.AgentSpec"});
    assert_eq!(
        unstructural_nodes(&referenced),
        ["openAPIV3Schema/properties/spec/properties/agent: no type"]
    );

    // An `additionalProperties: false` is not a schema; a schema beside `properties` is refused.
    let mut closed = document.clone();
    closed["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["additionalProperties"] =
        json!(false);
    assert_eq!(
        forbidden_keys(&closed),
        ["/spec/versions/0/schema/openAPIV3Schema/properties/spec/additionalProperties"]
    );
    let mut both = document.clone();
    both["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]["additionalProperties"] =
        json!({"type": "string"});
    assert_eq!(forbidden_keys(&both), Vec::<String>::new());
    assert_eq!(
        unstructural_nodes(&both),
        ["openAPIV3Schema/properties/spec: `properties` beside `additionalProperties`"]
    );

    // Inside a junctor, at any depth: no `type` and the like, and no member the schema outside it
    // does not declare.
    let mut junctor = document.clone();
    let items = &mut junctor["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["agent"]["properties"]["bindings"]["items"];
    items["oneOf"][0]["properties"]["kind"]["type"] = json!("string");
    items["oneOf"][1]["not"]["anyOf"][0]["description"] = json!("why");
    items["oneOf"][1]["not"]["anyOf"][0]["required"] = json!(["port"]);
    assert_eq!(
        unstructural_nodes(&junctor),
        [
            "openAPIV3Schema/properties/spec/properties/agent/properties/bindings/items/oneOf/0/properties/kind: `type` inside a junctor",
            "openAPIV3Schema/properties/spec/properties/agent/properties/bindings/items/oneOf/1/not/anyOf/0: `description` inside a junctor",
            "openAPIV3Schema/properties/spec/properties/agent/properties/bindings/items/oneOf/1/not/anyOf/0: `port` is not declared outside the junctor",
        ]
    );
}

/// The ESS schema `node`, its `$ref` followed into `definitions`.
fn resolved<'v>(node: &'v Value, definitions: &'v Value) -> &'v Value {
    match node.get("$ref").and_then(Value::as_str) {
        Some(reference) => resolved(
            &definitions[reference.trim_start_matches("#/$defs/")],
            definitions,
        ),
        None => node,
    }
}

fn sorted(value: Option<&Value>) -> Vec<String> {
    let mut names: Vec<String> = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

/// The definition the ESS schema `node` is part of: the last one its `$ref`s name, else `owner`.
fn definition_of<'v>(node: &'v Value, definitions: &'v Value, owner: &'v str) -> &'v str {
    match node.get("$ref").and_then(Value::as_str) {
        Some(reference) => {
            let name = reference.trim_start_matches("#/$defs/");
            definition_of(&definitions[name], definitions, name)
        }
        None => owner,
    }
}

/// Walks an ESS schema beside the projected node made of it, and records, by path, every object
/// whose projected `required` is not what [`Walk::expected`] says: a struct's own list, and for a
/// union each variant's list (in that variant's `oneOf` entry) and the tag plus what every variant
/// requires (on the flattened node).
struct Walk<'d> {
    /// The `$defs` of the document walked.
    definitions: &'d Value,
    /// Every definition reached.
    reached: BTreeSet<String>,
    /// Every member the specification declares required that the CRD does not require, by path.
    exempt: Vec<String>,
    /// Every mismatch, by path.
    found: Vec<String>,
}

impl<'d> Walk<'d> {
    fn over(definitions: &'d Value) -> Self {
        Self {
            definitions,
            reached: BTreeSet::new(),
            exempt: Vec::new(),
            found: Vec::new(),
        }
    }

    /// What the CRD must require of the ESS object `source`, a part of the definition `owner`:
    /// every member the specification declares required, less each whose schema is a list when
    /// `owner` is a definition of [`ABSENT_LIST_IS_EMPTY`]. A member left out is recorded in
    /// `exempt`, and in `found` unless the projected node `projected` declares it a list too.
    fn expected(
        &mut self,
        source: &Value,
        projected: &Value,
        owner: &str,
        at: &str,
    ) -> Vec<String> {
        let definitions = self.definitions;
        let lenient = owner
            .rsplit_once('.')
            .is_some_and(|(domain, _)| domain == ABSENT_LIST_IS_EMPTY);
        let mut expected = Vec::new();
        for member in sorted(source.get("required")) {
            let schema = resolved(&source["properties"][&member], definitions);
            if !(lenient && schema["type"] == "array") {
                expected.push(member);
                continue;
            }
            if projected["properties"][&member]["type"] != "array" {
                self.found.push(format!(
                    "{at}.{member}: exempt as a list, but projected as {}",
                    projected["properties"][&member]
                ));
            }
            self.exempt.push(format!("{at}.{member}"));
        }
        expected
    }

    /// The ESS schema `source`, a part of the definition `owner`, beside `projected`.
    fn object(&mut self, source: &Value, projected: &Value, owner: &str, at: &str) {
        let definitions = self.definitions;
        let owner = definition_of(source, definitions, owner);
        if source.get("$ref").is_some() {
            self.reached.insert(owner.to_owned());
        }
        let source = resolved(source, definitions);
        if source["x-ess-kind"] == "union" {
            let tag = source["x-ess-union-tag"].as_str().expect("union tag");
            let mut by_all: Option<Vec<String>> = None;
            for variant in source["oneOf"].as_array().expect("variants") {
                let name = variant["properties"][tag]["const"].as_str().expect("tag");
                let reference = &variant["properties"]["value"];
                let variant_owner = definition_of(reference, definitions, owner);
                if reference.get("$ref").is_some() {
                    self.reached.insert(variant_owner.to_owned());
                }
                let value = resolved(reference, definitions);
                let expected =
                    self.expected(value, projected, variant_owner, &format!("{at}<{name}>"));
                let entry = projected["oneOf"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|entry| entry["properties"][tag]["enum"] == json!([name]));
                let entry_required = sorted(entry.and_then(|entry| entry.get("required")));
                if entry.is_none() || entry_required != expected {
                    self.found.push(format!(
                        "{at}<{name}>: requires {entry_required:?}, expected {expected:?}"
                    ));
                }
                by_all = Some(match by_all {
                    None => expected.clone(),
                    Some(all) => all.into_iter().filter(|f| expected.contains(f)).collect(),
                });
                for (field, schema) in value["properties"].as_object().into_iter().flatten() {
                    self.object(
                        schema,
                        &projected["properties"][field],
                        variant_owner,
                        &format!("{at}.{field}"),
                    );
                }
            }
            let mut flattened = by_all.unwrap_or_default();
            flattened.push(tag.to_owned());
            flattened.sort();
            if sorted(projected.get("required")) != flattened {
                self.found.push(format!(
                    "{at}: requires {:?}, the tag and what every variant requires are {flattened:?}",
                    sorted(projected.get("required"))
                ));
            }
            return;
        }
        if let Some(properties) = source.get("properties").and_then(Value::as_object) {
            let expected = self.expected(source, projected, owner, at);
            if sorted(projected.get("required")) != expected {
                self.found.push(format!(
                    "{at}: requires {:?}, expected {expected:?}",
                    sorted(projected.get("required"))
                ));
            }
            for (field, schema) in properties {
                self.object(
                    schema,
                    &projected["properties"][field],
                    owner,
                    &format!("{at}.{field}"),
                );
            }
        }
        if let Some(items) = source.get("items") {
            self.object(items, &projected["items"], owner, &format!("{at}[]"));
        }
        if let Some(values) = source.get("additionalProperties").filter(|v| v.is_object()) {
            self.object(
                values,
                &projected["additionalProperties"],
                owner,
                &format!("{at}.*"),
            );
        }
    }
}

#[test]
fn every_object_requires_exactly_what_the_specification_declares_required() {
    let (spec, entity) = schemas();
    let document = projected();
    let schema = open_api(&document);
    let mut walk = Walk::over(&spec["$defs"]);
    walk.object(spec, &schema["properties"]["spec"], "", "spec");
    let mut status = entity.clone();
    status["properties"]
        .as_object_mut()
        .expect("entity properties")
        .remove(crd::IDENTITY);
    status["required"]
        .as_array_mut()
        .expect("entity required")
        .retain(|name| name != crd::IDENTITY);
    walk.definitions = &entity["$defs"];
    walk.object(&status, &schema["properties"]["status"], "", "status");
    assert_eq!(walk.found, Vec::<String>::new());

    // The exemption is one rule, a list of the `loom.agent` document: it applies (`states`), and a
    // list the models document declares, `model.roles`, stays required.
    assert!(
        walk.exempt.contains(&"spec.agent.states".to_owned()),
        "exempt: {:?}",
        walk.exempt
    );
    assert!(
        walk.exempt
            .iter()
            .all(|path| !path.starts_with("spec.agent.model.")),
        "exempt: {:?}",
        walk.exempt
    );
    let model = &schema["properties"]["spec"]["properties"]["agent"]["properties"]["model"];
    assert!(
        names(&model["required"]).contains(&"roles".to_owned()),
        "`model.roles` stays required: {}",
        model["required"]
    );

    // Every object either document defines is reached, so none goes unchecked.
    let objects: BTreeSet<String> = [spec, entity]
        .into_iter()
        .filter_map(|document| document["$defs"].as_object())
        .flatten()
        .filter(|(_, definition)| {
            definition.get("properties").is_some() || definition["x-ess-kind"] == "union"
        })
        .map(|(name, _)| name.clone())
        .collect();
    assert_eq!(
        objects.difference(&walk.reached).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "definitions the walk did not reach"
    );
    assert!(objects.len() >= 30, "{} object definitions", objects.len());
}

/// A value `schema` admits, the smallest one: an enum's first value, an object with its required
/// members.
fn sample(schema: &Value) -> Value {
    if let Some(first) = schema["enum"].as_array().and_then(|values| values.first()) {
        return first.clone();
    }
    match schema["type"].as_str() {
        Some("string") => json!("x"),
        Some("integer" | "number") => json!(1),
        Some("boolean") => json!(true),
        Some("array") => json!([]),
        _ => {
            let mut object = serde_json::Map::new();
            for name in sorted(schema.get("required")) {
                object.insert(name.clone(), sample(&schema["properties"][&name]));
            }
            Value::Object(object)
        }
    }
}

#[test]
fn each_binding_variant_refuses_a_member_only_another_variant_declares() {
    let document = projected();
    let items =
        open_api(&document)["properties"]["spec"]["properties"]["agent"]["properties"]["bindings"]
            ["items"]
            .clone();
    let validator = jsonschema::validator_for(&items).expect("the bindings item schema compiles");
    let (spec, _) = schemas();
    let definitions = &spec["$defs"];
    let mut own: Vec<(String, Vec<String>)> = Vec::new();
    for variant in definitions["loom.agent.Binding"]["oneOf"]
        .as_array()
        .expect("Binding is a oneOf")
    {
        let tag = variant["properties"]["kind"]["const"]
            .as_str()
            .expect("tag");
        let value = resolved(&variant["properties"]["value"], definitions);
        own.push((tag.to_owned(), keys(&value["properties"])));
    }
    let shared: Vec<String> = own[0]
        .1
        .iter()
        .filter(|field| own.iter().all(|(_, fields)| fields.contains(field)))
        .cloned()
        .collect();
    assert_eq!(shared, ["name"], "the members every variant declares");

    let mut tried = 0;
    for (tag, fields) in &own {
        let entry = items["oneOf"]
            .as_array()
            .expect("oneOf")
            .iter()
            .find(|entry| entry["properties"]["kind"]["enum"] == json!([tag]))
            .unwrap_or_else(|| panic!("no `oneOf` entry for `{tag}`"));
        let mut binding = json!({"kind": tag});
        for field in sorted(entry.get("required")) {
            binding[&field] = sample(&items["properties"][&field]);
        }
        for field in sorted(items.get("required")) {
            if binding.get(&field).is_none() {
                binding[&field] = sample(&items["properties"][&field]);
            }
        }
        assert!(
            validator.is_valid(&binding),
            "the smallest `{tag}` binding is admitted: {binding}"
        );
        for (other, theirs) in &own {
            for field in theirs.iter().filter(|field| !fields.contains(field)) {
                tried += 1;
                let mut crossed = binding.clone();
                crossed[field] = sample(&items["properties"][field]);
                assert!(
                    !validator.is_valid(&crossed),
                    "a `{tag}` binding carrying `{field}`, a member only `{other}` declares, is \
                     admitted: {crossed}"
                );
            }
        }
        for field in &shared {
            let mut kept = binding.clone();
            kept[field] = sample(&items["properties"][field]);
            assert!(
                validator.is_valid(&kept),
                "`{field}`, which every variant declares, is refused in a `{tag}` binding"
            );
        }
    }
    assert!(tried >= 8, "only {tried} cross-variant members tried");
}

#[test]
fn a_map_projects_with_its_value_schema() {
    let spec = definitions(json!({
        "test.A": {
            "type": "object",
            "properties": {
                "labels": {"type": "object", "additionalProperties": {"type": "string"}, "x-ess-map-key": "String"},
                "limits": {"type": "object", "additionalProperties": {"$ref": "#/$defs/test.Limit"}, "x-ess-map-key": "String"},
                "open": {"type": "object", "additionalProperties": true},
            },
            "required": ["labels"],
            "additionalProperties": false,
        },
        "test.Limit": {
            "type": "object",
            "properties": {"max": {"type": "integer"}, "unit": {"type": "string", "const": "s"}},
            "required": ["max"],
            "additionalProperties": false,
        },
    }));
    let document: Value = serde_yaml_ng::from_str(
        &crd::manifest(&spec, &entity()).unwrap_or_else(|e| panic!("a map projects: {e}")),
    )
    .expect("YAML");
    let projected = &open_api(&document)["properties"]["spec"];
    assert_eq!(
        projected["properties"]["labels"],
        json!({"type": "object", "additionalProperties": {"type": "string"}})
    );
    assert_eq!(
        projected["properties"]["limits"],
        json!({"type": "object", "additionalProperties": {
            "type": "object",
            "properties": {"max": {"type": "integer"}, "unit": {"type": "string", "enum": ["s"]}},
            "required": ["max"],
        }})
    );
    assert_eq!(
        projected["properties"]["open"],
        json!({"type": "object", "x-kubernetes-preserve-unknown-fields": true})
    );
    assert!(
        projected.get("additionalProperties").is_none(),
        "{projected}"
    );

    let validator = jsonschema::validator_for(open_api(&document)).expect("compiles");
    let resource = |labels: Value| json!({"spec": {"labels": labels}});
    assert!(validator.is_valid(&resource(json!({"team": "ops", "tier": "1"}))));
    assert!(!validator.is_valid(&resource(json!({"team": 1}))));
}

#[test]
fn a_map_beside_properties_or_keyed_by_other_than_text_is_refused_naming_it() {
    let both = definitions(json!({"test.A": {
        "type": "object",
        "properties": {"a": {"type": "string"}},
        "additionalProperties": {"type": "string"},
    }}));
    let refused = crd::manifest(&both, &entity()).expect_err("properties beside a map");
    assert!(
        refused.to_string().contains("additionalProperties"),
        "{refused}"
    );

    let keyed = definitions(json!({"test.A": {
        "type": "object",
        "properties": {"m": {"type": "object", "additionalProperties": {"type": "string"}, "x-ess-map-key": "Integer"}},
    }}));
    let refused = crd::manifest(&keyed, &entity()).expect_err("a map keyed by integers");
    assert!(
        matches!(&refused, CrdError::Keyword { keyword, .. } if keyword == "x-ess-map-key"),
        "{refused:?}"
    );
}

#[test]
fn spec_agent_is_the_agent_document_and_required() {
    let document = projected();
    let spec = &open_api(&document)["properties"]["spec"];
    assert_eq!(spec["type"], "object");
    assert_eq!(names(&spec["required"]), ["agent", "routes"]);
    assert_eq!(
        keys(&spec["properties"]),
        [
            "agent",
            "command",
            "connectorsConfig",
            "image",
            "routes",
            "secretFiles",
            "secretRef",
            "workspace"
        ]
    );
    let agent = &spec["properties"]["agent"];
    assert_eq!(agent["type"], "object");
    assert!(
        names(&agent["required"]).contains(&"format".to_owned()),
        "the agent document's own required members stay required: {}",
        agent["required"]
    );
}

#[test]
fn bindings_items_carry_the_tag_beside_the_fields_of_both_variants() {
    let document = projected();
    let items = &open_api(&document)["properties"]["spec"]["properties"]["agent"]["properties"]["bindings"]
        ["items"];
    assert_eq!(items["type"], "object");
    assert_eq!(
        items["properties"]["kind"],
        json!({"type": "string", "enum": ["connectors", "http"]})
    );

    // The variants as the specification projects them: `Binding`'s `oneOf`, each a `kind` const
    // beside a `value` naming the variant's struct.
    let (spec, _) = schemas();
    let definitions = &spec["$defs"];
    let union = &definitions["loom.agent.Binding"]["oneOf"];
    let mut required_by_all: Option<Vec<String>> = None;
    let mut variants: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for variant in union.as_array().expect("Binding is a oneOf") {
        let tag = variant["properties"]["kind"]["const"]
            .as_str()
            .expect("tag");
        let reference = variant["properties"]["value"]["$ref"]
            .as_str()
            .expect("value");
        let fields = &definitions[reference.trim_start_matches("#/$defs/")];
        for field in keys(&fields["properties"]) {
            assert!(
                items["properties"].get(&field).is_some(),
                "`{field}` of `{tag}` stands beside the tag: {}",
                items["properties"]
            );
        }
        // Both variants are `loom.agent` definitions, so a list they require is not required.
        let required: Vec<String> = names(&fields["required"])
            .into_iter()
            .filter(|f| resolved(&fields["properties"][f], definitions)["type"] != "array")
            .collect();
        required_by_all = Some(match required_by_all {
            None => required.clone(),
            Some(all) => all.into_iter().filter(|f| required.contains(f)).collect(),
        });
        variants.push((tag.to_owned(), keys(&fields["properties"]), required));
    }
    assert_eq!(variants.len(), 2, "connectors and http");
    let entries: Vec<Value> = variants
        .iter()
        .map(|(tag, own, required)| {
            let mut exclusive: Vec<&String> = variants
                .iter()
                .flat_map(|(_, theirs, _)| theirs)
                .filter(|field| !own.contains(field))
                .collect();
            exclusive.sort();
            exclusive.dedup();
            let refused: Vec<Value> = exclusive
                .iter()
                .map(|field| json!({"required": [field]}))
                .collect();
            json!({
                "properties": {"kind": {"enum": [tag]}},
                "required": required,
                "not": {"anyOf": refused},
            })
        })
        .collect();
    assert_eq!(items["oneOf"], Value::Array(entries));
    let mut required = vec!["kind".to_owned()];
    required.extend(required_by_all.unwrap_or_default());
    assert_eq!(names(&items["required"]), required);
    assert_eq!(names(&items["required"]), ["kind", "name"]);
    assert_eq!(
        items["oneOf"][1],
        json!({
            "properties": {"kind": {"enum": ["http"]}},
            "required": ["name", "base_url", "method", "max_bytes"],
            "not": {"anyOf": [
                {"required": ["adapter"]},
                {"required": ["connection"]},
                {"required": ["get"]},
                {"required": ["list"]},
                {"required": ["search"]},
                {"required": ["write"]},
            ]},
        })
    );
}

#[test]
fn an_agent_list_may_be_absent_a_models_list_may_not_and_a_decimal_is_a_number() {
    let document = projected();
    let agent = &open_api(&document)["properties"]["spec"]["properties"]["agent"];
    assert_eq!(
        names(&agent["required"]),
        ["format", "agent", "model"],
        "`states` and `bindings`, lists of the agent document, may be absent"
    );
    assert_eq!(agent["properties"]["states"]["type"], "array");
    assert_eq!(agent["properties"]["bindings"]["type"], "array");
    assert_eq!(
        names(&agent["properties"]["model"]["required"]),
        ["routes", "roles"],
        "`roles`, a list of the models document, stays required"
    );
    assert_eq!(
        agent["properties"]["loops"]["properties"]["inner"]["properties"]["classify_threshold"],
        json!({"type": "number"})
    );
    assert_eq!(
        agent["properties"]["session"]["properties"]["budget"]["properties"]["deadline"],
        json!({"type": "string", "format": "date-time"})
    );
}

#[test]
fn the_command_variant_members_are_declared_with_what_each_requires() {
    let document = projected();
    let schema = open_api(&document);
    let spec = &schema["properties"]["spec"]["properties"];
    let strings = json!({"type": "array", "items": {"type": "string"}});
    assert_eq!(
        spec["command"],
        json!({"type": "object", "properties": {"args": strings, "entrypoint": strings}}),
        "an absent entrypoint or args is left to the image"
    );
    assert_eq!(
        spec["workspace"],
        json!({
            "type": "object",
            "properties": {"claimName": {"type": "string"}, "mountPath": {"type": "string"}},
            "required": ["mountPath"],
        })
    );
    assert_eq!(
        spec["secretRef"],
        json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"],
        }),
        "the environment Secret is projected one way only"
    );
    assert_eq!(
        spec["secretFiles"],
        json!({
            "type": "object",
            "properties": {"mountPath": {"type": "string"}, "name": {"type": "string"}},
            "required": ["name", "mountPath"],
        })
    );
    let agent = &spec["agent"]["properties"];
    assert!(
        !names(&agent["runtime"]["required"]).contains(&"connectors".to_owned()),
        "runtime.connectors is optional"
    );
    assert!(
        !names(&agent["deploy"]["properties"]["kubernetes"]["required"])
            .contains(&"image_repository".to_owned()),
        "deploy.kubernetes.image_repository is optional"
    );
    assert!(
        names(&agent["runtime"]["required"]).contains(&"state_dir".to_owned()),
        "the rest of runtime stays required"
    );
    assert_eq!(
        schema["properties"]["status"]["properties"]["variant"],
        json!({"type": "string", "enum": ["AgentServe", "Command"]})
    );
}

#[test]
fn spec_routes_is_untyped_and_preserves_unknown_fields() {
    let document = projected();
    assert_eq!(
        open_api(&document)["properties"]["spec"]["properties"]["routes"],
        json!({"x-kubernetes-preserve-unknown-fields": true}),
        "`routes` is an ESS `Json`, which admits any JSON value"
    );
}

#[test]
fn status_is_the_entity_without_its_name_and_its_state_enumerates_the_lifecycle() {
    let compiled: Value = serde_json::from_slice(&ess(
        &["specify", "compile", "--path", "ess", "--format", "json"],
        None,
    ))
    .expect("`ess specify compile --format json` prints JSON");
    let mut lifecycle =
        names(&compiled["entities"]["loom.resource.LoomAgent"]["lifecycle"]["states"]);
    lifecycle.sort();

    let document = projected();
    let status = &open_api(&document)["properties"]["status"];
    assert_eq!(status["type"], "object");
    assert_eq!(
        keys(&status["properties"]),
        [
            "check",
            "conditions",
            "deployment",
            "observedGeneration",
            "run",
            "state",
            "variant"
        ]
    );
    assert_eq!(names(&status["required"]), ["observedGeneration", "state"]);
    assert_eq!(
        status["properties"]["deployment"],
        json!({
            "type": "object",
            "properties": {
                "availability": {"type": "string", "enum": ["Available", "Unavailable"]},
            },
            "required": ["availability"],
        }),
        "what the controller last read of the child Deployment"
    );
    assert_eq!(
        status["properties"]["conditions"],
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "type": {"type": "string", "enum": ["Checked", "Available", "Completed"]},
                    "status": {"type": "string", "enum": ["True", "False", "Unknown"]},
                    "reason": {"type": "string"},
                    "message": {"type": "string"},
                },
                "required": ["type", "status", "reason", "message"],
            },
        }),
        "the Kubernetes conditions, their status text and not a boolean"
    );
    let mut states = names(&status["properties"]["state"]["enum"]);
    states.sort();
    assert_eq!(states, lifecycle);
    assert_eq!(status["properties"]["state"]["type"], "string");
    assert_eq!(
        status["properties"]["check"],
        json!({
            "type": "object",
            "properties": {"exitCode": {"type": "integer"}, "message": {"type": "string"}},
            "required": ["exitCode", "message"],
        })
    );
    assert_eq!(
        status["properties"]["run"],
        json!({
            "type": "object",
            "properties": {
                "exitCode": {"type": "integer"},
                "finishedAt": {"type": "string", "format": "date-time"},
                "reason": {"type": "string"},
            },
        }),
        "how a Command resource's run ended, each member absent when its pod did not say"
    );
}

/// `examples/agents/operator.yaml` as the `spec.agent` of a `LoomAgent`.
fn operator_resource() -> Value {
    let path = root().join("examples/agents/operator.yaml");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let agent: Value = serde_yaml_ng::from_str(&text).expect("the example is YAML");
    json!({
        "apiVersion": "loom.beyond10x.dev/v1alpha1",
        "kind": "LoomAgent",
        "metadata": {"name": "platform-operator"},
        "spec": {"agent": agent, "routes": {}},
    })
}

fn errors(validator: &jsonschema::Validator, resource: &Value) -> Vec<String> {
    validator
        .iter_errors(resource)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

#[test]
fn the_operator_example_validates_as_a_loom_agent_and_refusals_do_not() {
    let document = projected();
    let validator = jsonschema::validator_for(open_api(&document))
        .expect("the projected schema compiles as JSON Schema");
    let resource = operator_resource();
    assert_eq!(errors(&validator, &resource), Vec::<String>::new());

    let mut routed = resource.clone();
    routed["spec"]["routes"] =
        json!({"endpoints": {"local": {"url": "http://llm:8080"}}, "routes": {}});
    routed["spec"]["image"] = json!({"repository": "registry.example/loom-agent", "tag": "0.40.0"});
    routed["spec"]["secretRef"] = json!({"name": "platform-operator-secrets"});
    routed["status"] = json!({"observedGeneration": 2, "state": "Running", "check": {"exitCode": 0, "message": ""}});
    assert_eq!(errors(&validator, &routed), Vec::<String>::new());
    routed["status"]["deployment"] = json!({"availability": "Available"});
    routed["status"]["conditions"] = json!([
        {"type": "Checked", "status": "True", "reason": "SpecChecked", "message": ""},
        {"type": "Available", "status": "True", "reason": "DeploymentAvailable", "message": ""},
    ]);
    assert_eq!(errors(&validator, &routed), Vec::<String>::new());

    // A list of the agent document left out is one its decoder reads as empty.
    let mut listless = resource.clone();
    listless["spec"]["agent"]["states"][2]
        .as_object_mut()
        .expect("state")
        .remove("sources");
    listless["spec"]["agent"]["bindings"][1]
        .as_object_mut()
        .expect("binding")
        .remove("allow_paths");
    assert_eq!(errors(&validator, &listless), Vec::<String>::new());

    // `routes` is an ESS `Json`, so the schema admits any JSON value there; the pod's
    // `b10x-loom agent check` is what reads the routes document.
    let mut named_routes = resource.clone();
    named_routes["spec"]["routes"] = json!("routes.yaml");
    assert_eq!(errors(&validator, &named_routes), Vec::<String>::new());

    let refused = |change: &dyn Fn(&mut Value), why: &str| {
        let mut copy = resource.clone();
        change(&mut copy);
        assert!(
            !errors(&validator, &copy).is_empty(),
            "{why}: the projected schema must refuse it"
        );
    };
    refused(
        &|r| {
            r["spec"].as_object_mut().expect("spec").remove("agent");
        },
        "a resource without spec.agent",
    );
    refused(
        &|r| {
            r.as_object_mut().expect("resource").remove("spec");
        },
        "a resource without spec",
    );
    refused(
        &|r| {
            r["spec"]["agent"]["model"]
                .as_object_mut()
                .expect("model")
                .remove("roles");
        },
        "a models section without `roles`, a list the models document requires",
    );
    refused(
        &|r| r["spec"]["agent"]["bindings"][0]["base_url"] = json!("http://logs:3100"),
        "a connectors binding carrying an http binding's `base_url`",
    );
    refused(
        &|r| r["spec"]["agent"]["bindings"][1]["adapter"] = json!("chat"),
        "an http binding carrying a connectors binding's `adapter`",
    );
    refused(
        &|r| r["spec"]["agent"]["bindings"][1]["kind"] = json!("ftp"),
        "a binding kind the union does not declare",
    );
    refused(
        &|r| {
            r["spec"]["agent"]["bindings"][1]
                .as_object_mut()
                .expect("binding")
                .remove("base_url");
        },
        "an http binding without base_url",
    );
    refused(
        &|r| {
            r["spec"]["agent"]["bindings"][0]
                .as_object_mut()
                .expect("binding")
                .remove("adapter");
        },
        "a connectors binding without adapter",
    );
    refused(
        &|r| r["spec"]["agent"]["session"]["tools"]["decisions"] = json!("maybe"),
        "a decision mode the domain does not declare",
    );
    refused(
        &|r| r["spec"]["agent"]["loops"]["inner"]["step_budget"] = json!("eight"),
        "a budget that is not an integer",
    );
    refused(
        &|r| r["spec"]["agent"]["loops"]["inner"]["classify_threshold"] = json!("half"),
        "a threshold that is not a number",
    );
    refused(
        &|r| r["spec"]["image"] = json!({"repository": "registry.example/loom-agent"}),
        "an image without its tag",
    );
    refused(
        &|r| r["status"] = json!({"observedGeneration": 1, "state": "Crashed"}),
        "a state the lifecycle does not declare",
    );
    refused(
        &|r| {
            r["status"] = json!({"observedGeneration": 1, "state": "Running",
                "conditions": [{"type": "Checked", "status": true, "reason": "SpecChecked", "message": ""}]})
        },
        "a condition status written as a boolean rather than the text `True`",
    );
    refused(
        &|r| {
            r["status"] = json!({"observedGeneration": 1, "state": "Running",
                "deployment": {"availability": "Progressing"}})
        },
        "a Deployment availability the domain does not declare",
    );
}

fn definitions(entries: Value) -> Value {
    json!({"$ref": "#/$defs/test.A", "$defs": entries})
}

fn entity() -> Value {
    json!({
        "type": "object",
        "properties": {"name": {"type": "string"}, "state": {"type": "string", "enum": ["Pending"]}},
        "required": ["name", "state"],
    })
}

#[test]
fn a_cycle_of_references_is_refused_naming_the_type() {
    let spec = definitions(json!({
        "test.A": {"type": "object", "properties": {"b": {"$ref": "#/$defs/test.B"}}},
        "test.B": {"type": "object", "properties": {"a": {"$ref": "#/$defs/test.A"}}},
    }));
    let refused = crd::manifest(&spec, &entity()).expect_err("a cycle cannot be inlined");
    assert!(
        matches!(&refused, CrdError::Cycle { reference } if reference == "test.A"),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("test.A"), "{refused}");
}

#[test]
fn a_reference_to_no_definition_is_refused_naming_it() {
    let spec = definitions(
        json!({"test.A": {"type": "object", "properties": {"b": {"$ref": "#/$defs/test.Missing"}}}}),
    );
    let refused = crd::manifest(&spec, &entity()).expect_err("an unknown reference");
    assert!(refused.to_string().contains("test.Missing"), "{refused}");
}

#[test]
fn two_variants_declaring_one_field_differently_are_refused_naming_it() {
    let spec = definitions(json!({
        "test.A": {
            "x-ess-kind": "union",
            "x-ess-union-tag": "kind",
            "oneOf": [
                {"type": "object", "properties": {"kind": {"type": "string", "const": "x"}, "value": {"$ref": "#/$defs/test.X"}}, "required": ["kind", "value"]},
                {"type": "object", "properties": {"kind": {"type": "string", "const": "y"}, "value": {"$ref": "#/$defs/test.Y"}}, "required": ["kind", "value"]},
            ],
        },
        "test.X": {"type": "object", "properties": {"port": {"type": "integer"}}, "required": ["port"]},
        "test.Y": {"type": "object", "properties": {"port": {"type": "string"}}, "required": ["port"]},
    }));
    let refused = crd::manifest(&spec, &entity()).expect_err("conflicting variants");
    assert!(
        matches!(&refused, CrdError::UnionConflict { property, .. } if property == "port"),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("port"), "{refused}");
}
