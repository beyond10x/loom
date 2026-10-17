//! Acceptance for `story:sdk-models-from-config`: a host loads its models from a configuration
//! file or a routes document through `loom_sdk::models`. No test reaches the network.

use std::fs;
use std::path::{Path, PathBuf};

use llm_core::{Cancel, ErrorCode, Item, Model, TurnRequest, VecSink};
use loom_sdk::models::{ModelRole, from_config, from_routes, parse_config};

/// A routes document: two endpoints, two routes. Nothing here is ever contacted.
const ROUTES: &str = "format: llm.routes/1
endpoints:
  lab:
    base_url: http://127.0.0.1:9/v1
    protocol: chat-completions
    auth: {kind: anonymous}
  vault:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {kind: bearer, secret: env:LOOM_SDK_MODELS_TEST_NEVER_SET}
routes:
  sonnet: lab/claude-sonnet
  small: lab/small
  sealed: vault/sealed-model
";

/// A directory holding `routes.yaml` and, when given, `models.yaml`.
fn write(config: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    fs::write(dir.path().join("routes.yaml"), ROUTES).expect("the routes are written");
    if let Some(config) = config {
        fs::write(dir.path().join("models.yaml"), config).expect("the config is written");
    }
    dir
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join("models.yaml")
}

fn serving(model: &dyn Model) -> String {
    model.provenance().model.as_str().to_owned()
}

fn refusal(result: Result<impl Sized, loom_sdk::models::ModelsError>) -> String {
    match result {
        Ok(_) => panic!("expected a refusal"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn a_config_names_the_model_of_each_role() {
    let dir = write(Some(
        "routes: routes.yaml
roles:
  - {role: agent, model: sonnet}
  - {role: classifier, model: small}
",
    ));
    let models = from_config(&config_path(dir.path())).expect("the config loads");
    let agent = models.get(ModelRole::Agent).expect("an agent model");
    let classifier = models
        .get(ModelRole::Classifier)
        .expect("a classifier model");
    assert_eq!(serving(agent.as_ref()), "sonnet");
    assert_eq!(serving(classifier.as_ref()), "small");
    // Another name of the same document, for an override.
    let other = models.model("sealed").expect("another route");
    assert_eq!(serving(other.as_ref()), "sealed");

    let parsed = parse_config("routes: r.yaml\nroles:\n  - {role: agent, model: m}\n")
        .expect("the text parses");
    assert_eq!(parsed.routes, "r.yaml");
    assert_eq!(parsed.roles.len(), 1);
    assert_eq!(parsed.roles[0].role, ModelRole::Agent);

    // A config without a role leaves it absent.
    let dir = write(Some(
        "routes: routes.yaml\nroles:\n  - {role: agent, model: sonnet}\n",
    ));
    let models = from_config(&config_path(dir.path())).expect("the config loads");
    assert!(models.get(ModelRole::Classifier).is_none());
}

#[test]
fn a_routes_name_resolves_to_a_model() {
    let dir = write(None);
    let path = dir.path().join("routes.yaml");
    let route = from_routes(&path, "sonnet").expect("a route resolves");
    assert_eq!(serving(route.as_ref()), "sonnet");
    let direct = from_routes(&path, "lab/some-model").expect("a direct name resolves");
    assert_eq!(direct.provenance().endpoint.as_str(), "lab");
    assert!(serving(direct.as_ref()).contains("some-model"));
}

#[test]
fn a_refusal_names_the_file_and_the_name() {
    let dir = write(None);
    let routes = dir.path().join("routes.yaml");
    let config = config_path(dir.path());
    let load = |text: &str| {
        fs::write(&config, text).expect("the config is written");
        refusal(from_config(&config))
    };

    let message = load("routes: routes.yaml\nroles:\n  - {role: planner, model: sonnet}\n");
    assert!(
        message.contains("models.yaml") && message.contains("planner"),
        "{message}"
    );

    let message = load(
        "routes: routes.yaml\nroles:\n  - {role: agent, model: sonnet}\n  - {role: agent, model: small}\n",
    );
    assert!(
        message.contains("models.yaml") && message.contains("agent"),
        "{message}"
    );

    let message = load("routes: routes.yaml\nextra: 1\nroles: []\n");
    assert!(
        message.contains("models.yaml") && message.contains("extra"),
        "{message}"
    );

    let message = load("routes: absent.yaml\nroles:\n  - {role: agent, model: sonnet}\n");
    assert!(message.contains("absent.yaml"), "{message}");

    let message = load("routes: routes.yaml\nroles:\n  - {role: agent, model: nowhere}\n");
    assert!(
        message.contains("models.yaml") && message.contains("agent") && message.contains("nowhere"),
        "{message}"
    );

    // A missing configuration, and a directory in its place.
    let missing = dir.path().join("none.yaml");
    assert!(refusal(from_config(&missing)).contains("none.yaml"));
    assert!(refusal(from_config(dir.path())).contains(&dir.path().display().to_string()));

    // Routes documents: invalid, missing, an undeclared name.
    fs::write(
        dir.path().join("bad.yaml"),
        "format: llm.routes/1\nproviders: {}\n",
    )
    .unwrap();
    assert!(refusal(from_routes(&dir.path().join("bad.yaml"), "x")).contains("bad.yaml"));
    assert!(refusal(from_routes(&dir.path().join("gone.yaml"), "x")).contains("gone.yaml"));
    let message = refusal(from_routes(&routes, "nothing"));
    assert!(
        message.contains("routes.yaml") && message.contains("nothing"),
        "{message}"
    );
}

#[test]
fn a_missing_secret_is_refused_at_the_first_turn_and_never_read_early() {
    assert!(std::env::var_os("LOOM_SDK_MODELS_TEST_NEVER_SET").is_none());
    let dir = write(None);
    // Building succeeds: nothing is read before the first turn.
    let model = from_routes(&dir.path().join("routes.yaml"), "sealed").expect("the route builds");
    let request = TurnRequest::new("sealed", vec![Item::user("hello")]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let mut sink = VecSink::new(16, 1 << 16);
    let error = runtime
        .block_on(model.turn(&request, &mut sink, &Cancel::new()))
        .expect_err("the first turn is refused");
    assert_eq!(error.code, ErrorCode::Unauthorized);
}

#[test]
fn the_routes_path_is_relative_to_the_configuration_directory() {
    let dir = write(None);
    let nested = dir.path().join("host").join("config");
    fs::create_dir_all(&nested).expect("the nested directory is made");
    let config = nested.join("models.yaml");
    fs::write(
        &config,
        "routes: ../../routes.yaml\nroles:\n  - {role: agent, model: sonnet}\n",
    )
    .expect("the config is written");
    // The process's working directory is not the configuration's: the path still resolves.
    assert_ne!(
        std::env::current_dir().ok().as_deref(),
        Some(nested.as_path())
    );
    let models = from_config(&config).expect("the routes resolve against the config's directory");
    let agent = models.get(ModelRole::Agent).expect("an agent model");
    assert_eq!(serving(agent.as_ref()), "sonnet");

    // The same relative name from the working directory is not looked up.
    fs::write(&config, "routes: routes.yaml\nroles: []\n").expect("the config is written");
    let message = refusal(from_config(&config));
    assert!(
        message.contains(&nested.join("routes.yaml").display().to_string()),
        "{message}"
    );
}
