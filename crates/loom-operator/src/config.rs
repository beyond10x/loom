//! The controller's configuration, `config.yaml`: the namespace it watches, the agent image and
//! resources a `LoomAgent` gets when it names none, the model route a resource without routes is
//! given, the Secret names and egress hosts a resource may use, and the upstreams a
//! credential-free resource's boundary may relay to.
//!
//! ```yaml
//! namespace: agents
//! defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
//! defaultResources: {cpu: 250m, memory: 256Mi}
//! modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small}
//! allowedSecretNames: [platform-operator-secrets]
//! allowedEgressHosts: [chat.example]
//! allowedClaimNames: [takt-workspace]
//! allowUnconfinedProfiles: false
//! allowedSeccompLocalhostProfiles: [profiles/takt.json]
//! allowedAppArmorLocalhostProfiles: [takt-delivery]
//! allowedBoundaryUpstreams: ['http://llm-gateway.llm.svc:8080/v1']
//! ```
//!
//! `modelRoute` is optional; the lists may be absent, and an empty list admits none;
//! `allowUnconfinedProfiles` defaults to `false`. A member the configuration does not declare is
//! refused, so a misspelt key is never ignored.

use std::fmt;
use std::fs;
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
    /// The resources of a pod whose spec declares no `deploy.kubernetes.resources`.
    pub default_resources: Resources,
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
    /// The upstream base URLs a `spec.boundary` may name, each matched exactly; empty admits
    /// none.
    #[serde(default)]
    pub allowed_boundary_upstreams: Vec<String>,
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
    /// `registry.example/loom-agent` at this crate's version, 250m CPU and 256Mi memory, no model
    /// route, and no Secret name, egress host or boundary upstream admitted.
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
        serde_yaml_ng::from_str(text).map_err(|error| ConfigError(error.to_string()))
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
            "{base}allowedBoundaryUpstreams: ['http://llm-gateway.llm.svc:8080/v1']\n"
        ))
        .expect("a configuration");
        assert_eq!(
            listed.allowed_boundary_upstreams,
            ["http://llm-gateway.llm.svc:8080/v1"]
        );
    }
}
