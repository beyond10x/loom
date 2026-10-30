//! Acceptance for `story:command-confinement-profiles`: a `Command` `LoomAgent` declares the
//! seccomp and AppArmor profiles of its program's container, the controller renders them, and
//! its configuration decides which it admits, so `Unconfined` is the cluster operator's to grant.

use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_yaml_ng::Value;

const BASE: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
defaultCommandResources: {cpu: 100m, memory: 128Mi}
";

const PERMISSIVE: &str = "allowUnconfinedProfiles: true
allowedSeccompLocalhostProfiles: [profiles/takt.json]
allowedAppArmorLocalhostProfiles: [takt-delivery]
";

fn config(extra: &str) -> Config {
    Config::parse(&format!("{BASE}{extra}")).expect("the test configuration")
}

fn command(profiles: &str) -> String {
    format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: takt-run
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000001
  generation: 1
spec:
  routes: {{}}
  command: {{entrypoint: [takt]}}
{profiles}"
    )
}

fn serve(profiles: &str) -> String {
    format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: serve-run
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000002
  generation: 1
spec:
  routes:
    format: llm.routes/1
    endpoints:
      lab: {{base_url: 'http://lab.example/v1', protocol: messages, auth: {{kind: anonymous}}}}
    routes: {{small: lab/small}}
  agent:
    format: loom.agent/1
    agent: {{id: a, name: A, brief: answers}}
    model: {{routes: routes.yaml, roles: [{{role: agent, model: small}}]}}
    runtime:
      state_dir: /var/lib/agent
      connectors: {{program: connectors}}
      confinement: none
{profiles}"
    )
}

fn documents(text: &str) -> Vec<Value> {
    text.split("\n---\n")
        .map(|part| serde_yaml_ng::from_str(part).expect("a rendered document"))
        .collect()
}

fn pod(rendered: &str) -> Value {
    let job = documents(rendered)
        .into_iter()
        .find(|document| document["kind"] == "Job" || document["kind"] == "Deployment")
        .expect("a workload");
    job["spec"]["template"]["spec"].clone()
}

fn refused(result: Result<String, RenderError>) -> (String, String) {
    match result.expect_err("refused") {
        RenderError::Refused(refusal) => (refusal.reason, refusal.member),
        other => panic!("not a refusal: {other}"),
    }
}

#[test]
fn a_declared_profile_is_rendered_on_the_container() {
    let rendered = render_document(
        &command(
            "  seccompProfile: {type: Localhost, localhostProfile: profiles/takt.json}
  appArmorProfile: {type: Localhost, localhostProfile: takt-delivery}
",
        ),
        &config(PERMISSIVE),
    )
    .expect("admitted");
    let pod = pod(&rendered);
    let container = &pod["containers"][0]["securityContext"];
    assert_eq!(container["seccompProfile"]["type"], "Localhost");
    assert_eq!(
        container["seccompProfile"]["localhostProfile"],
        "profiles/takt.json"
    );
    assert_eq!(container["appArmorProfile"]["type"], "Localhost");
    assert_eq!(
        container["appArmorProfile"]["localhostProfile"],
        "takt-delivery"
    );
    assert_eq!(container["runAsNonRoot"], true);
    assert_eq!(container["allowPrivilegeEscalation"], false);
    assert_eq!(
        pod["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    for init in pod["initContainers"]
        .as_sequence()
        .expect("init containers")
    {
        assert!(
            init["securityContext"]["seccompProfile"].is_null()
                && init["securityContext"]["appArmorProfile"].is_null(),
            "an init container keeps the pod's profile"
        );
    }
}

#[test]
fn an_unconfined_profile_the_operator_allows_is_rendered() {
    let rendered = render_document(
        &command("  seccompProfile: {type: Unconfined}\n  appArmorProfile: {type: Unconfined}\n"),
        &config(PERMISSIVE),
    )
    .expect("admitted");
    let container = &pod(&rendered)["containers"][0]["securityContext"];
    assert_eq!(container["seccompProfile"]["type"], "Unconfined");
    assert_eq!(container["appArmorProfile"]["type"], "Unconfined");
}

#[test]
fn an_unconfined_profile_not_allowed_by_the_operator_is_refused() {
    for (profile, member) in [
        ("seccompProfile", "spec.seccompProfile.type"),
        ("appArmorProfile", "spec.appArmorProfile.type"),
    ] {
        let resource = command(&format!("  {profile}: {{type: Unconfined}}\n"));
        assert_eq!(
            refused(render_document(&resource, &config(""))),
            ("ProfileNotAllowed".to_owned(), member.to_owned()),
        );
        let explicit = config("allowUnconfinedProfiles: false\n");
        assert_eq!(
            refused(render_document(&resource, &explicit)),
            ("ProfileNotAllowed".to_owned(), member.to_owned()),
        );
    }
}

#[test]
fn a_localhost_profile_outside_the_operators_list_is_refused() {
    let seccomp =
        command("  seccompProfile: {type: Localhost, localhostProfile: profiles/other.json}\n");
    assert_eq!(
        refused(render_document(&seccomp, &config(PERMISSIVE))),
        (
            "ProfileNotAllowed".to_owned(),
            "spec.seccompProfile.localhostProfile".to_owned()
        ),
    );
    let apparmor =
        command("  appArmorProfile: {type: Localhost, localhostProfile: profiles/takt.json}\n");
    assert_eq!(
        refused(render_document(&apparmor, &config(PERMISSIVE))),
        (
            "ProfileNotAllowed".to_owned(),
            "spec.appArmorProfile.localhostProfile".to_owned()
        ),
        "the AppArmor list is not the seccomp list"
    );
}

#[test]
fn a_profile_names_its_localhost_profile_exactly_when_it_is_localhost() {
    for (declared, member) in [
        ("{type: Localhost}", "localhostProfile"),
        (
            "{type: RuntimeDefault, localhostProfile: profiles/takt.json}",
            "localhostProfile",
        ),
        (
            "{type: Unconfined, localhostProfile: profiles/takt.json}",
            "localhostProfile",
        ),
        (
            "{type: Localhost, localhostProfile: ../escape.json}",
            "localhostProfile",
        ),
        (
            "{type: Localhost, localhostProfile: /etc/shadow}",
            "localhostProfile",
        ),
        (
            "{type: Localhost, localhostProfile: ''}",
            "localhostProfile",
        ),
        ("{type: Bogus}", "type"),
    ] {
        let resource = command(&format!("  seccompProfile: {declared}\n"));
        assert_eq!(
            refused(render_document(&resource, &config(PERMISSIVE))),
            (
                "ProfileInvalid".to_owned(),
                format!("spec.seccompProfile.{member}")
            ),
            "{declared}"
        );
    }
}

#[test]
fn an_agent_serve_resource_keeps_runtime_default() {
    let plain = render_document(&serve(""), &config(PERMISSIVE)).expect("admitted");
    let pod = pod(&plain);
    assert_eq!(
        pod["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    let container = &pod["containers"][0]["securityContext"];
    assert!(container["seccompProfile"].is_null());
    assert!(container["appArmorProfile"].is_null());

    let declared = serve("  seccompProfile: {type: Unconfined}\n");
    assert_eq!(
        refused(render_document(&declared, &config(PERMISSIVE))),
        ("Malformed".to_owned(), "spec.seccompProfile".to_owned()),
    );
}

#[test]
fn a_command_resource_without_a_profile_renders_as_before() {
    let rendered = render_document(&command(""), &config(PERMISSIVE)).expect("admitted");
    let pod = pod(&rendered);
    let container = &pod["containers"][0]["securityContext"];
    assert!(container["seccompProfile"].is_null());
    assert!(container["appArmorProfile"].is_null());
    assert_eq!(
        pod["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
}

#[test]
fn the_configuration_without_the_new_members_denies_what_it_did_not_list() {
    let config = config("");
    assert!(!config.allow_unconfined_profiles);
    assert!(config.allowed_seccomp_localhost_profiles.is_empty());
    assert!(config.allowed_app_armor_localhost_profiles.is_empty());
    assert_eq!(
        config,
        Config::parse(&format!("{BASE}allowUnconfinedProfiles: false\n")).expect("parsed")
    );
}
