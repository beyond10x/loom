//! Acceptance for `story:operator-constraints-in-ess`: each authored fixture under
//! `ess/scenarios/resource-admission/` answers its own reason of `loom.resource.AdmitLoomAgent` at
//! its own member. The conformance suite runs the same fixtures through the same function and
//! requires one passing scenario per declared reason; these tests name the member each reason is
//! answered at and keep the carried adversary case of wave 2026-10-26-q13: a `Command` resource
//! answers the decoder's own reason, not `Malformed`.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::admission::{Refusal, admit, schedule_name};
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::parse_resource;
use loom_protocols::ProtocolCatalog;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    root().join("ess/scenarios/resource-admission")
}

/// The answer `admit` gives the fixture `name` under the scenarios' configuration.
fn answer(name: &str) -> Result<(), Refusal> {
    let config = Config::parse(
        &fs::read_to_string(fixtures().join("config.yaml")).expect("the scenarios' configuration"),
    )
    .expect("the scenarios' configuration parses");
    let text = fs::read_to_string(fixtures().join(format!("resources/{name}.yaml")))
        .unwrap_or_else(|error| panic!("the fixture {name}: {error}"));
    let agent = parse_resource(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
    let namespace = agent
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| config.namespace.clone());
    let catalog = ProtocolCatalog::plugins().expect("the plugin catalog");
    let actions = |protocol: &str| catalog.get(protocol).map(|entry| entry.actions());
    let resource = agent.metadata.name.clone().unwrap_or_default();
    admit(&agent.spec, &namespace, &config, &actions)
        .and_then(|_| schedule_name(&resource, &agent.spec))
}

fn refusal(name: &str) -> Refusal {
    match answer(name) {
        Ok(()) => panic!("{name} was admitted"),
        Err(refusal) => refusal,
    }
}

/// Each fixture, the reason it is refused with and the member it is refused at.
const REFUSED: [(&str, &str, &str); 35] = [
    ("malformed", "Malformed", "spec.agent"),
    (
        "secret-literal",
        "SecretLiteral",
        "spec.agent.bindings[1].base_url",
    ),
    ("unknown-format", "UnknownFormat", "spec.agent.format"),
    ("blank-text", "BlankText", "spec.agent.agent.name"),
    (
        "not-a-variable-name",
        "NotAVariableName",
        "spec.agent.bindings[2].bearer_env",
    ),
    (
        "port-out-of-range",
        "PortOutOfRange",
        "spec.agent.expose.a2a.port",
    ),
    (
        "not-a-plain-path",
        "NotAPlainPath",
        "spec.agent.bindings[2].allow_paths[1]",
    ),
    (
        "not-an-instant",
        "NotAnInstant",
        "spec.agent.session.budget.deadline",
    ),
    (
        "unknown-protocol",
        "UnknownProtocol",
        "spec.agent.loops.inner.protocol",
    ),
    (
        "unknown-action",
        "UnknownAction",
        "spec.agent.states[0].actions[2]",
    ),
    (
        "unbound-source",
        "UnboundSource",
        "spec.agent.states[0].sources[2]",
    ),
    ("http-method", "HttpMethod", "spec.agent.bindings[2].method"),
    (
        "send-without-writes",
        "SendWithoutWrites",
        "spec.agent.policy.capabilities[2]",
    ),
    (
        "empty-channel-allow",
        "EmptyChannelAllow",
        "spec.agent.channel.allow",
    ),
    (
        "command-not-a-variable-name",
        "NotAVariableName",
        "spec.agent.bindings[2].bearer_env",
    ),
    (
        "command-port-out-of-range",
        "PortOutOfRange",
        "spec.agent.expose.a2a.port",
    ),
    (
        "command-not-an-instant",
        "NotAnInstant",
        "spec.agent.session.budget.deadline",
    ),
    ("no-model-route", "NoModelRoute", "spec.routes"),
    ("too-large", "TooLarge", "spec.command"),
    (
        "boundary-with-secret",
        "BoundaryWithSecret",
        "spec.secretRef",
    ),
    (
        "boundary-upstream-invalid",
        "BoundaryUpstreamInvalid",
        "spec.boundary.model.upstream",
    ),
    (
        "boundary-upstream-not-allowed",
        "BoundaryUpstreamNotAllowed",
        "spec.boundary.model.upstream",
    ),
    (
        "boundary-egress-host",
        "BoundaryEgressHost",
        "spec.agent.deploy.kubernetes.egress[0].host",
    ),
    (
        "boundary-egress-namespace",
        "BoundaryEgressNamespace",
        "spec.agent.deploy.kubernetes.egress[0].namespace",
    ),
    (
        "secret-not-allowed",
        "SecretNotAllowed",
        "spec.secretRef.name",
    ),
    (
        "namespace-mismatch",
        "NamespaceMismatch",
        "spec.agent.deploy.kubernetes.namespace",
    ),
    (
        "egress-host-not-allowed",
        "EgressHostNotAllowed",
        "spec.agent.deploy.kubernetes.egress[0].host",
    ),
    ("env-name-conflict", "EnvNameConflict", "spec.env[0].name"),
    (
        "config-file-invalid",
        "ConfigFileInvalid",
        "spec.configFiles[0].name",
    ),
    (
        "config-file-conflict",
        "ConfigFileConflict",
        "spec.configFiles[0].name",
    ),
    (
        "mount-path-invalid",
        "MountPathInvalid",
        "spec.workspace.mountPath",
    ),
    (
        "claim-not-allowed",
        "ClaimNotAllowed",
        "spec.workspace.claimName",
    ),
    (
        "profile-invalid",
        "ProfileInvalid",
        "spec.seccompProfile.localhostProfile",
    ),
    (
        "profile-not-allowed",
        "ProfileNotAllowed",
        "spec.seccompProfile.type",
    ),
    (
        "resources-required",
        "ResourcesRequired",
        "spec.agent.deploy.kubernetes.resources",
    ),
];

#[test]
fn each_fixture_is_refused_with_its_reason_at_its_member() {
    let mut wrong = Vec::new();
    for (name, reason, member) in REFUSED {
        let refusal = refusal(name);
        if refusal.reason != reason || refusal.member != member {
            wrong.push(format!("{name}: {}", refusal.message()));
        }
    }
    assert!(
        wrong.is_empty(),
        "answered otherwise:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_admitted_fixtures_are_admitted() {
    for name in ["admitted", "admitted-command"] {
        if let Err(refusal) = answer(name) {
            panic!("{name}: {}", refusal.message());
        }
    }
}

/// Carried from the adversary pass of wave 2026-10-26-q13: the `AgentServe` variant passes the
/// value reasons of `loom.agent.CheckAgentSpec` through at the member, under `spec.agent`.
#[test]
fn adv_q13_the_serve_variant_answers_the_new_reason_at_the_member() {
    for (name, reason, member) in [
        (
            "port-out-of-range",
            "PortOutOfRange",
            "spec.agent.expose.a2a.port",
        ),
        (
            "not-an-instant",
            "NotAnInstant",
            "spec.agent.session.budget.deadline",
        ),
        (
            "not-a-variable-name",
            "NotAVariableName",
            "spec.agent.bindings[2].bearer_env",
        ),
    ] {
        let refusal = refusal(name);
        assert_eq!(
            (refusal.reason.as_str(), refusal.member.as_str()),
            (reason, member),
            "{name}: {}",
            refusal.message()
        );
    }
}

/// The carried case's other half: a `Command` resource's `spec.agent` goes through the decoder
/// only, and the decoder's own reason and member are its answer, where it was `Malformed` at
/// `spec.agent` before.
#[test]
fn a_command_resource_answers_the_decoders_reason_at_the_member() {
    for (name, reason, member) in [
        (
            "command-port-out-of-range",
            "PortOutOfRange",
            "spec.agent.expose.a2a.port",
        ),
        (
            "command-not-an-instant",
            "NotAnInstant",
            "spec.agent.session.budget.deadline",
        ),
        (
            "command-not-a-variable-name",
            "NotAVariableName",
            "spec.agent.bindings[2].bearer_env",
        ),
    ] {
        let refusal = refusal(name);
        assert_eq!(
            (refusal.reason.as_str(), refusal.member.as_str()),
            (reason, member),
            "{name}: {}",
            refusal.message()
        );
    }
}
