//! The model-routes guide shows files Loom accepts: its routes document and its `loom.yaml` are
//! read by the parsers Loom itself uses.

use std::path::Path;

use llm_models::routes::Routes;
use loom_sdk::models::{ModelRole, parse_config};

const PAGE: &str = "website/docs/guides/model-routes.md";

fn page() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(PAGE)).expect("the model-routes guide exists")
}

/// The fenced `yaml` blocks of the page, in order.
fn yaml_blocks(page: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut open: Option<String> = None;
    for line in page.lines() {
        match (&mut open, line.trim_end()) {
            (None, "```yaml") => open = Some(String::new()),
            (Some(block), "```") => {
                blocks.push(std::mem::take(block));
                open = None;
            }
            (Some(block), text) => {
                block.push_str(text);
                block.push('\n');
            }
            (None, _) => {}
        }
    }
    blocks
}

#[test]
fn the_documented_model_routes_parse() {
    let page = page();
    let blocks = yaml_blocks(&page);
    let routes_text = blocks
        .iter()
        .find(|block| block.starts_with("format: llm.routes/1"))
        .expect("the page shows a routes document");
    let config_text = blocks
        .iter()
        .find(|block| block.starts_with("routes:"))
        .expect("the page shows a loom.yaml");

    let routes = Routes::parse(routes_text).expect("the documented routes document parses");
    let endpoints: Vec<_> = routes.endpoints().collect();
    let bearer = endpoints
        .iter()
        .any(|endpoint| format!("{:?}", endpoint.auth_kind()) == "Bearer");
    let anonymous = endpoints
        .iter()
        .any(|endpoint| format!("{:?}", endpoint.auth_kind()) == "Anonymous");
    assert!(bearer, "an endpoint with auth.kind bearer");
    assert!(anonymous, "an endpoint with auth.kind anonymous");
    for host in ["https://api.anthropic.com", "https://openrouter.ai"] {
        assert!(
            endpoints
                .iter()
                .any(|endpoint| endpoint.base_url().as_str().starts_with(host)),
            "an endpoint at {host}"
        );
    }

    let config = parse_config(config_text).expect("the documented loom.yaml parses");
    let roles: Vec<ModelRole> = config.roles.iter().map(|entry| entry.role).collect();
    assert_eq!(roles, [ModelRole::Agent, ModelRole::Classifier]);
    for entry in &config.roles {
        let route = routes
            .routes()
            .any(|declared| declared.name().to_string() == entry.model);
        assert!(route, "role {:?} names a route of the document", entry.role);
    }

    for phrase in [
        "b10x-loom run --config loom.yaml",
        "--model",
        "--classifier-model",
        "loom_sdk::models::from_config",
        "loom_sdk::models::from_routes",
    ] {
        assert!(page.contains(phrase), "the page contains `{phrase}`");
    }
}
