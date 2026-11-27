//! Project the server's bound API from ESS, then apply its HTTP-only presentation rules.
use std::{fs, path::Path};

use serde_json::{Value, json};

use super::{Regeneration, Scratch};

const CONTRACT: &str = "website/static/api/openapi/dashboard.json";

pub(super) fn run(root: &Path, ess: &Path, scratch: &Scratch, check: bool) -> Result<(), String> {
    let binding = root.join("ess/dashboard/http.yaml");
    let raw = Regeneration::run(
        root,
        ess,
        scratch,
        "dashboard-api",
        &["generate"],
        "ess/dashboard",
        &[
            "--kind",
            "openapi",
            "--strict",
            "--http",
            binding.to_str().ok_or("non-UTF8 binding path")?,
        ],
    )?;
    let input = fs::read(raw.dir.join("openapi/dashboard.yaml")).map_err(|e| e.to_string())?;
    let mut document: Value = serde_yaml_ng::from_slice(&input).map_err(|e| e.to_string())?;
    serve_presentation(&mut document)?;
    let bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?
    );
    let file = root.join(CONTRACT);
    if check {
        if fs::read(&file).map_err(|e| format!("{CONTRACT}: {e}"))? != bytes.as_bytes() {
            return Err(format!("{CONTRACT} differs; run `task dashboard-api`"));
        }
    } else {
        fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&file, bytes).map_err(|e| e.to_string())?;
    }
    println!("{CONTRACT}: matches the dashboard HTTP projection");
    Ok(())
}

fn bound_get<'a>(doc: &'a mut Value, path: &str) -> Result<&'a mut Value, String> {
    let operation = doc
        .get_mut("paths")
        .and_then(|p| p.get_mut(path))
        .and_then(|endpoint| endpoint.get_mut("get"))
        .ok_or_else(|| format!("the ESS projection does not bind GET {path}"))?;
    let responses = operation["responses"]
        .as_object_mut()
        .ok_or("no bound responses")?;
    // ESS includes a fallback of its synthesized server. Loom handles these reads itself.
    if responses
        .get("501")
        .is_some_and(|r| r["x-ess-outcomes"] == json!([]))
    {
        responses.remove("501");
    }
    Ok(operation)
}

fn serve_presentation(doc: &mut Value) -> Result<(), String> {
    let source = bound_get(doc, "/api/agents/{agent}/spec")?;
    source["summary"] =
        json!("Read the validated source of a registered agent as the server operator");
    source["security"] = json!([{"operatorBearer": []}]);
    let success = &mut source["responses"]["200"];
    let content = success["content"]
        .as_object_mut()
        .ok_or("no agent source body")?;
    // ESS binds this outcome to unwrapped text. Loom serves that text as a YAML document.
    let schema = content
        .remove("text/plain")
        .ok_or("agent source is not bound to text")?;
    content.insert("application/yaml".to_owned(), schema);
    success["description"] = json!(
        "The complete UTF-8 bytes validated on registration, including original formatting and comments. The response also carries charset=utf-8."
    );
    success["headers"]["ETag"] = json!({"description":"Quoted lowercase SHA-256 digest of the exact response bytes.","schema":{"type":"string"}});
    let operation = bound_get(doc, "/api/snapshot")?;
    operation["summary"] = json!("Read the authenticated viewer's server snapshot");
    operation["security"] = json!([{"viewerBearer": []}]);
    operation["parameters"]
        .as_array_mut()
        .ok_or("no snapshot query parameters")?
        .push(json!({
            "in": "header", "name": "If-None-Match", "required": false,
            "description": "The quoted ETag of a previous response for this viewer and filter.",
            "schema": {"type": "string"}
        }));
    let responses = operation["responses"]
        .as_object_mut()
        .ok_or("no snapshot responses")?;
    let tag = json!({"description":"Opaque, double-quoted SHA-256 validator.","schema":{"type":"string"}});
    responses.get_mut("200").ok_or("no snapshot success")?["headers"]["ETag"] = tag;
    responses.insert(
        "304".into(),
        json!({
            "description":"The viewer already has this representation. The response has no body."
        }),
    );
    doc["components"]["securitySchemes"]["viewerBearer"] = json!({"type":"http","scheme":"bearer"});
    doc["components"]["securitySchemes"]["operatorBearer"] = json!({
        "type":"http","scheme":"bearer",
        "description":"The operator credential in the dashboard's private viewer-token file. Scoped viewer tokens cannot read agent source."
    });
    doc["info"]["title"] = json!("Loom server dashboard");
    doc["info"]["description"] = json!(
        "Read-only dashboard HTTP API. Parameters and data types are projected from Loom's ESS model. A2A runs on each individual agent listener."
    );
    doc["paths"]["/api/openapi.json"] = json!({"get":{
        "operationId":"readDashboardContract", "summary":"Read this OpenAPI document",
        "security":[], "responses":{"200":{"description":"The OpenAPI 3.1 contract.","content":{
            "application/json":{"schema":{"type":"object"}}
        }}}
    }});
    // ESS Optional means the field may be absent. The served JSON spells every missing scalar
    // as an explicit null; retain all constraints on the present value.
    let schemas = doc
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
        .ok_or("the ESS projection contains no schemas")?;
    for schema in schemas
        .values_mut()
        .filter(|s| s["x-ess-kind"] == "struct" && s["x-ess-name"] != "loom.dashboard.TurnEvent")
    {
        let required = schema["required"].as_array().cloned().unwrap_or_default();
        let properties = schema["properties"]
            .as_object_mut()
            .ok_or("a struct has no properties")?;
        for (name, field) in properties.iter_mut() {
            if !required.contains(&json!(name)) {
                *field = json!({"anyOf":[field.take(), {"type":"null"}]});
            }
        }
        schema["required"] = json!(properties.keys().collect::<Vec<_>>());
    }
    Ok(())
}
