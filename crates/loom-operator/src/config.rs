//! The controller's configuration, `config.yaml`: the namespace it watches, the agent image and
//! resources a `LoomAgent` gets when it names none, the model route a resource without routes is
//! given, the Secret names and egress hosts a resource may use, and the upstreams a
//! credential-free resource's boundary may relay to.
//!
//! ```yaml
//! namespace: agents
//! defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
//! defaultResources: {cpu: 250m, memory: 256Mi}
//! defaultCommandResources: {cpu: '4', memory: 8Gi}
//! modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small}
//! allowedSecretNames: [platform-operator-secrets]
//! allowedEgressHosts: [chat.example]
//! allowedClaimNames: [takt-workspace]
//! allowUnconfinedProfiles: false
//! allowedSeccompLocalhostProfiles: [profiles/takt.json]
//! allowedAppArmorLocalhostProfiles: [takt-delivery]
//! allowedBoundaryUpstreams:
//!   - {kind: model, url: 'http://llm-gateway.llm.svc:8080', audience: llm-gateway}
//!   - {kind: git, url: 'http://connectors.connectors.svc/git', audience: connectors-git, port: 8080}
//! ```
//!
//! `defaultResources` sizes an `AgentServe` pod whose spec declares no
//! `deploy.kubernetes.resources`, since its program is Loom's own. A `Command` pod runs the
//! image's own program, whose need Loom does not know: without declared resources it gets the
//! optional `defaultCommandResources`, and without that the resource is refused
//! `ResourcesRequired`. Each `cpu` and `memory` of the two is a non-negative Kubernetes
//! quantity (`250m`, `4`, `8Gi`, `1e3`); any other text refuses the configuration, naming the
//! member and the value, so neither `run` nor `render --config` starts on it.
//!
//! `modelRoute` is optional; the lists may be absent, and an empty list admits none;
//! `allowUnconfinedProfiles` defaults to `false`. A member the configuration does not declare is
//! refused, so a misspelt key is never ignored.
//!
//! Each `allowedBoundaryUpstreams` entry admits one upstream for one relay: its `kind` (`model`,
//! `connectors` or `git`), its `url` and its token `audience`, which a `spec.boundary.<kind>`
//! must give exactly, and optionally the `port` (1 to 65535) of the upstream's pods that its
//! Service targets, which the pod's NetworkPolicy opens in place of the URL's port, since a
//! NetworkPolicy matches the pod port after the Service has translated it.

use std::fmt;
use std::fs;
use std::num::NonZeroU16;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The controller's configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// The one namespace the controller watches and writes in.
    pub namespace: String,
    /// The agent image a resource without `spec.image` runs.
    pub default_image: Image,
    /// The resources of an `AgentServe` pod whose spec declares no `deploy.kubernetes.resources`.
    pub default_resources: Resources,
    /// The resources of a `Command` pod whose spec declares no `deploy.kubernetes.resources`;
    /// absent, such a resource is refused `ResourcesRequired`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_command_resources: Option<Resources>,
    /// The in-cluster model route a resource with an empty `spec.routes` is given; without one
    /// such a resource is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_route: Option<ModelRoute>,
    /// The Secrets a resource's `spec.secretRef` may name; empty admits none.
    #[serde(default)]
    pub allowed_secret_names: Vec<String>,
    /// The hosts a `deploy.kubernetes.egress` entry may name; empty admits none.
    #[serde(default)]
    pub allowed_egress_hosts: Vec<String>,
    /// The PersistentVolumeClaims a resource's `spec.workspace.claimName` may name; empty admits
    /// none.
    #[serde(default)]
    pub allowed_claim_names: Vec<String>,
    /// Whether a `Command` resource may declare an `Unconfined` seccomp or AppArmor profile;
    /// absent, `false`: the cluster's operator grants it, never the resource alone.
    #[serde(default)]
    pub allow_unconfined_profiles: bool,
    /// The `Localhost` seccomp profiles (paths relative to the kubelet's seccomp directory) a
    /// `spec.seccompProfile` may name; empty admits none.
    #[serde(default)]
    pub allowed_seccomp_localhost_profiles: Vec<String>,
    /// The `Localhost` AppArmor profiles (names the node has loaded) a `spec.appArmorProfile` may
    /// name; empty admits none.
    #[serde(default)]
    pub allowed_app_armor_localhost_profiles: Vec<String>,
    /// The upstreams a `spec.boundary` may name, each for one relay kind, its URL and audience
    /// matched exactly; empty admits none.
    #[serde(default)]
    pub allowed_boundary_upstreams: Vec<AllowedUpstream>,
}

/// The relay a boundary upstream is declared for, as `spec.boundary` names its members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamKind {
    /// `spec.boundary.model`, the model pass-through.
    Model,
    /// `spec.boundary.connectors`, the Connectors wire.
    Connectors,
    /// `spec.boundary.git`, the git smart-HTTP relay.
    Git,
}

impl UpstreamKind {
    /// The member's name under `spec.boundary`: `model`, `connectors` or `git`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Connectors => "connectors",
            Self::Git => "git",
        }
    }
}

/// One `allowedBoundaryUpstreams` entry: a `spec.boundary.<kind>` whose `upstream` is `url` and
/// whose `audience` is `audience`, each exactly, is admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedUpstream {
    /// The relay it may be declared for.
    pub kind: UpstreamKind,
    /// The upstream's base URL.
    pub url: String,
    /// The audience of the projected token the boundary sends it.
    pub audience: String,
    /// The pod port the upstream's Service targets, which the NetworkPolicy opens; absent, the
    /// URL's port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<NonZeroU16>,
}

/// A container image, `repository:tag`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// The repository, registry host included.
    pub repository: String,
    /// The tag.
    pub tag: String,
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.repository, self.tag)
    }
}

/// A pod's CPU and memory, as Kubernetes quantities; requests and limits alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// CPU, such as `250m`.
    pub cpu: String,
    /// Memory, such as `256Mi`.
    pub memory: String,
}

/// An in-cluster model route: an endpoint that needs no credential from the agent (a signing
/// proxy holds it), by URL, `llm.routes/1` protocol and the model id it serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRoute {
    /// The endpoint's base URL, such as `http://model-route.agents.svc:8080/v1`.
    pub url: String,
    /// The endpoint's protocol, as `llm.routes/1` spells it (`messages`, `chat-completions`).
    pub protocol: String,
    /// The model id every role's route names at that endpoint.
    pub model: String,
}

impl ModelRoute {
    /// The port the URL names, or its scheme's default (80, 443 for `https`).
    #[must_use]
    pub fn port(&self) -> u16 {
        let (scheme, rest) = self.url.split_once("://").unwrap_or(("http", &self.url));
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let authority = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        authority
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse().ok())
            .unwrap_or(if scheme.eq_ignore_ascii_case("https") {
                443
            } else {
                80
            })
    }
}

/// Why a configuration was not loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// The configuration `render` uses without `--config`: namespace `default`, the agent image
    /// `registry.example/loom-agent` at this crate's version, 250m CPU and 256Mi memory for an
    /// `AgentServe` resource, no default size for a `Command` resource (one that declares none
    /// is refused `ResourcesRequired`), no model route, and no Secret name, egress host or
    /// boundary upstream admitted.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            namespace: "default".to_owned(),
            default_image: Image {
                repository: "registry.example/loom-agent".to_owned(),
                tag: env!("CARGO_PKG_VERSION").to_owned(),
            },
            default_resources: Resources {
                cpu: "250m".to_owned(),
                memory: "256Mi".to_owned(),
            },
            default_command_resources: None,
            model_route: None,
            allowed_secret_names: Vec::new(),
            allowed_egress_hosts: Vec::new(),
            allowed_claim_names: Vec::new(),
            allow_unconfined_profiles: false,
            allowed_seccomp_localhost_profiles: Vec::new(),
            allowed_app_armor_localhost_profiles: Vec::new(),
            allowed_boundary_upstreams: Vec::new(),
        }
    }

    /// Decodes a configuration from YAML.
    ///
    /// # Errors
    /// The decoder's message, naming the member, when `text` is not a configuration.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: Self =
            serde_yaml_ng::from_str(text).map_err(|error| ConfigError(error.to_string()))?;
        sized("defaultResources", &config.default_resources)?;
        if let Some(size) = &config.default_command_resources {
            sized("defaultCommandResources", size)?;
        }
        Ok(config)
    }

    /// Whether an `allowedBoundaryUpstreams` entry names the relay `kind` (`model`, `connectors`
    /// or `git`) at exactly `url`, whatever its audience.
    #[must_use]
    pub fn allows_upstream_url(&self, kind: &str, url: &str) -> bool {
        self.allowed_boundary_upstreams
            .iter()
            .any(|allowed| allowed.kind.as_str() == kind && allowed.url == url)
    }

    /// The `allowedBoundaryUpstreams` entry admitting the relay `kind` at exactly `url` with
    /// exactly `audience`, if one does.
    #[must_use]
    pub fn allowed_upstream(
        &self,
        kind: &str,
        url: &str,
        audience: &str,
    ) -> Option<&AllowedUpstream> {
        self.allowed_boundary_upstreams.iter().find(|allowed| {
            allowed.kind.as_str() == kind && allowed.url == url && allowed.audience == audience
        })
    }

    /// Reads and decodes the configuration at `path`.
    ///
    /// # Errors
    /// A message naming `path` when it cannot be read or is not a configuration.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path)
            .map_err(|error| ConfigError(format!("--config {}: {error}", path.display())))?;
        Self::parse(&text)
            .map_err(|error| ConfigError(format!("--config {}: {error}", path.display())))
    }
}

/// The suffixes a Kubernetes quantity may end in: none, the decimal SI ones and the binary SI
/// ones.
const QUANTITY_SUFFIXES: [&str; 16] = [
    "", "n", "u", "m", "k", "M", "G", "T", "P", "E", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei",
];

/// Whether `text` is a non-negative Kubernetes quantity as the API server reads one: an
/// optional `+`, digits with an optional decimal part (`2.`, `.5`), then a suffix of
/// [`QUANTITY_SUFFIXES`] or a decimal exponent (`e3`, `E-3`). No blank, space or other unit.
fn quantity(text: &str) -> bool {
    let unsigned = text.strip_prefix('+').unwrap_or(text);
    let end = unsigned
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(unsigned.len());
    let (number, suffix) = unsigned.split_at(end);
    let number = match number.split_once('.') {
        None => !number.is_empty(),
        Some((whole, fraction)) => {
            !fraction.contains('.') && !(whole.is_empty() && fraction.is_empty())
        }
    };
    let exponent = || {
        suffix
            .strip_prefix(['e', 'E'])
            .map(|rest| rest.strip_prefix(['+', '-']).unwrap_or(rest))
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
    };
    number && (QUANTITY_SUFFIXES.contains(&suffix) || exponent())
}

/// Refuses a `cpu` or `memory` of `size`, the configuration's `member`, that is no Kubernetes
/// quantity, naming the member and the value: such a size would be refused at apply, where no
/// refusal names it.
fn sized(member: &str, size: &Resources) -> Result<(), ConfigError> {
    for (field, value) in [("cpu", &size.cpu), ("memory", &size.memory)] {
        if !quantity(value) {
            return Err(ConfigError(format!(
                "{member}.{field}: {value:?} is not a Kubernetes quantity"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(url: &str) -> ModelRoute {
        ModelRoute {
            url: url.to_owned(),
            protocol: "messages".to_owned(),
            model: "m".to_owned(),
        }
    }

    #[test]
    fn the_model_routes_port_is_its_urls_or_the_schemes_default() {
        assert_eq!(route("http://model-route.agents.svc:8080/v1").port(), 8080);
        assert_eq!(route("http://model-route.agents.svc/v1").port(), 80);
        assert_eq!(route("https://model-route.agents.svc").port(), 443);
        assert_eq!(route("http://[fd00::1]:9000/v1").port(), 9000);
        assert_eq!(route("http://[fd00::1]/v1").port(), 80);
    }

    #[test]
    fn an_undeclared_member_is_refused() {
        let error = Config::parse(
            "namespace: a\ndefaultImage: {repository: r, tag: t}\ndefaultResources: {cpu: '1', \
             memory: 1Mi}\nallowedSecretName: [x]\n",
        )
        .expect_err("refused");
        assert!(error.0.contains("allowedSecretName"), "{error}");
    }

    #[test]
    fn the_lists_and_the_model_route_may_be_absent() {
        let config = Config::parse(
            "namespace: a\ndefaultImage: {repository: r, tag: t}\ndefaultResources: {cpu: '1', \
             memory: 1Mi}\n",
        )
        .expect("a configuration");
        assert_eq!(config.model_route, None);
        assert!(config.allowed_secret_names.is_empty());
        assert!(config.allowed_egress_hosts.is_empty());
    }

    #[test]
    fn the_boundary_upstreams_are_listed_and_absent_admit_none() {
        let base = "namespace: a\ndefaultImage: {repository: r, tag: t}\ndefaultResources: {cpu: \
                    '1', memory: 1Mi}\n";
        let absent = Config::parse(base).expect("a configuration");
        assert!(absent.allowed_boundary_upstreams.is_empty());
        assert!(Config::builtin().allowed_boundary_upstreams.is_empty());
        let listed = Config::parse(&format!(
            "{base}allowedBoundaryUpstreams:\n- {{kind: model, url: 'http://llm-gateway.llm.svc:8080', \
             audience: llm-gateway}}\n- {{kind: git, url: 'http://connectors.connectors.svc/git', \
             audience: connectors-git, port: 8080}}\n"
        ))
        .expect("a configuration");
        assert_eq!(
            listed.allowed_boundary_upstreams,
            [
                AllowedUpstream {
                    kind: UpstreamKind::Model,
                    url: "http://llm-gateway.llm.svc:8080".to_owned(),
                    audience: "llm-gateway".to_owned(),
                    port: None,
                },
                AllowedUpstream {
                    kind: UpstreamKind::Git,
                    url: "http://connectors.connectors.svc/git".to_owned(),
                    audience: "connectors-git".to_owned(),
                    port: NonZeroU16::new(8080),
                },
            ]
        );
        assert_eq!(
            listed
                .allowed_upstream(
                    "git",
                    "http://connectors.connectors.svc/git",
                    "connectors-git"
                )
                .and_then(|allowed| allowed.port),
            NonZeroU16::new(8080)
        );
        assert_eq!(
            listed.allowed_upstream(
                "model",
                "http://connectors.connectors.svc/git",
                "connectors-git"
            ),
            None,
            "an entry admits its own kind only"
        );
    }

    /// `story:boundary-container-projected-token`: an `allowedBoundaryUpstreams` entry is a kind,
    /// a URL and an audience, with an optional pod port; a bare URL, an unknown member or kind, a
    /// missing audience and port 0 are refused when the configuration is read.
    #[test]
    fn a_boundary_upstream_entry_out_of_shape_is_refused() {
        let base = "namespace: a\ndefaultImage: {repository: r, tag: t}\ndefaultResources: {cpu: \
                    '1', memory: 1Mi}\n";
        for (entry, named) in [
            (
                "'http://llm-gateway.llm.svc:8080'",
                "struct AllowedUpstream",
            ),
            (
                "{kind: model, url: 'http://gw.llm.svc', audience: gw, host: gw.example}",
                "host",
            ),
            (
                "{kind: tools, url: 'http://gw.llm.svc', audience: gw}",
                "tools",
            ),
            ("{kind: model, url: 'http://gw.llm.svc'}", "audience"),
            (
                "{kind: model, url: 'http://gw.llm.svc', audience: gw, port: 0}",
                "nonzero",
            ),
            (
                "{kind: model, url: 'http://gw.llm.svc', audience: gw, port: 65536}",
                "65536",
            ),
        ] {
            let error = Config::parse(&format!("{base}allowedBoundaryUpstreams: [{entry}]\n"))
                .expect_err(entry);
            assert!(
                error.0.contains("allowedBoundaryUpstreams") && error.0.contains(named),
                "story:boundary-container-projected-token: {entry}: {error}"
            );
        }
    }
}
