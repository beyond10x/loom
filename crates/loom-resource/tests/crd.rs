//! The `LoomAgent` CustomResourceDefinition is projected from the specification, never written by
//! hand (`story:loom-agent-resource-domain`):
//!
//! * the committed `deploy/crd/loomagents.loom.beyond10x.dev.yaml` is a fixed point of
//!   `crd::manifest` over what `ess generate --kind schema` writes for `ess/`;
//! * the projected document holds no key Kubernetes refuses in a structural schema or that only ESS
//!   reads (`$ref`, `$defs`, `$schema`, `$id`, `const`, `additionalProperties`, `title`, `x-ess-*`),
//!   by a scan of the whole document written here, apart from the projection's own check; a copy
//!   with one such key put back fails the scan, naming it, for each key;
//! * every schema node is typed, and every `oneOf` entry holds only the tag's `enum` and `required`
//!   members the enclosing schema declares;
//! * `spec.agent` is required; `spec.agent.bindings.items` carries `kind` with
//!   `enum: [connectors, http]` and the fields of both variants beside it; `spec.routes` preserves
//!   unknown fields; `status.state` enumerates exactly the lifecycle's states, read from the
//!   compiled specification;
//! * the schema accepts what the agent document's decoder reads: a list is never required (the
//!   decoder reads an absent one as empty) and a decimal is a number;
//! * `examples/agents/operator.yaml`, wrapped as a `LoomAgent`'s `spec.agent`, validates against
//!   the projected `openAPIV3Schema` as JSON Schema (no cluster), and resources the decoder would
//!   refuse do not.
//!
//! The specification is read with the `ess` binary on `PATH`, as `task conform` reads it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_resource::crd::{self, CrdError};
use serde_json::{Value, json};

/// The keys no projected document may hold, wherever they stand as a keyword.
const FORBIDDEN: [&str; 7] = [
    "$ref",
    "$defs",
    "$schema",
    "$id",
    "const",
    "additionalProperties",
    "title",
];

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
                if !names && (FORBIDDEN.contains(&key.as_str()) || key.starts_with("x-ess-")) {
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
/// preserves unknown fields), a `oneOf` entry holding anything but `properties` and `required`,
/// a property inside it with anything but `enum`, and a name inside it the enclosing schema does
/// not declare.
fn unstructural(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(node) = schema.as_object() else {
        found.push(format!("{at}: not a schema object"));
        return;
    };
    let preserves = node.get("x-kubernetes-preserve-unknown-fields") == Some(&Value::Bool(true));
    if !node.contains_key("type") && !preserves {
        found.push(format!("{at}: no type"));
    }
    let declared = node.get("properties").and_then(Value::as_object);
    if let Some(properties) = declared {
        for (name, property) in properties {
            unstructural(property, &format!("{at}/properties/{name}"), found);
        }
    }
    if let Some(items) = node.get("items") {
        unstructural(items, &format!("{at}/items"), found);
    }
    for junctor in ["oneOf", "anyOf", "allOf"] {
        let Some(entries) = node.get(junctor).and_then(Value::as_array) else {
            continue;
        };
        for (index, entry) in entries.iter().enumerate() {
            let here = format!("{at}/{junctor}/{index}");
            let entry = entry.as_object().cloned().unwrap_or_default();
            for key in entry.keys() {
                if key != "properties" && key != "required" {
                    found.push(format!("{here}: `{key}` inside a junctor"));
                }
            }
            let mut named: Vec<String> = Vec::new();
            if let Some(properties) = entry.get("properties").and_then(Value::as_object) {
                for (name, property) in properties {
                    named.push(name.clone());
                    let keys: Vec<&String> = property
                        .as_object()
                        .map(|p| p.keys().collect())
                        .unwrap_or_default();
                    if keys != ["enum"] {
                        found.push(format!("{here}/properties/{name}: holds {keys:?}"));
                    }
                }
            }
            if let Some(required) = entry.get("required").and_then(Value::as_array) {
                named.extend(required.iter().filter_map(Value::as_str).map(str::to_owned));
            }
            for name in named {
                if !declared.is_some_and(|properties| properties.contains_key(&name)) {
                    found.push(format!(
                        "{here}: `{name}` is not declared outside the junctor"
                    ));
                }
            }
        }
    }
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
}

#[test]
fn spec_agent_is_the_agent_document_and_required() {
    let document = projected();
    let spec = &open_api(&document)["properties"]["spec"];
    assert_eq!(spec["type"], "object");
    assert_eq!(names(&spec["required"]), ["agent", "routes"]);
    assert_eq!(
        keys(&spec["properties"]),
        ["agent", "connectorsConfig", "image", "routes", "secretRef"]
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
    let mut entries = Vec::new();
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
        // A list is not required: the decoder reads an absent one as empty.
        let required: Vec<String> = names(&fields["required"])
            .into_iter()
            .filter(|field| fields["properties"][field]["type"] != "array")
            .collect();
        required_by_all = Some(match required_by_all {
            None => required.clone(),
            Some(all) => all.into_iter().filter(|f| required.contains(f)).collect(),
        });
        entries.push(json!({"properties": {"kind": {"enum": [tag]}}, "required": required}));
    }
    assert_eq!(entries.len(), 2, "connectors and http");
    assert_eq!(items["oneOf"], Value::Array(entries));
    let mut required = vec!["kind".to_owned()];
    required.extend(required_by_all.unwrap_or_default());
    assert_eq!(names(&items["required"]), required);
    assert_eq!(names(&items["required"]), ["kind", "name"]);
    assert_eq!(
        items["oneOf"][1],
        json!({"properties": {"kind": {"enum": ["http"]}}, "required": ["name", "base_url", "method", "max_bytes"]})
    );
}

#[test]
fn a_list_is_never_required_and_a_decimal_is_a_number() {
    let document = projected();
    let agent = &open_api(&document)["properties"]["spec"]["properties"]["agent"];
    assert_eq!(
        names(&agent["required"]),
        ["format", "agent", "model"],
        "`states` and `bindings` are lists, which the decoder reads as empty when absent"
    );
    assert_eq!(agent["properties"]["states"]["type"], "array");
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
fn spec_routes_preserve_unknown_fields() {
    let document = projected();
    assert_eq!(
        open_api(&document)["properties"]["spec"]["properties"]["routes"],
        json!({"type": "object", "x-kubernetes-preserve-unknown-fields": true})
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
        ["check", "observedGeneration", "state"]
    );
    assert_eq!(names(&status["required"]), ["observedGeneration", "state"]);
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

    let mut stateless = resource.clone();
    stateless["spec"]["agent"]
        .as_object_mut()
        .expect("agent")
        .remove("states");
    assert_eq!(
        errors(&validator, &stateless),
        Vec::<String>::new(),
        "an absent list is an empty one, as the decoder reads it"
    );

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
        &|r| r["spec"]["routes"] = json!("routes.yaml"),
        "routes that are not a document",
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
