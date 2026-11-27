//! Operator-only metadata projected from the same bindings used to build the model ports.
use llm_core::{Model, Protocol};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    LaunchCatalog,
    SpecRoutes,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Routing {
    pub source: Source,
    pub agent: Role,
    pub classifier: Role,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    pub selection: Option<String>,
    upstream_model: Option<String>,
    endpoint_id: Option<String>,
    base_origin: Option<String>,
    protocol: Protocol,
    provider_id: Option<String>,
    binding_revision: Option<String>,
}

pub(super) fn identifier(value: &str) -> Option<String> {
    let valid = value.len() <= 160
        && value.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !["env:", "file:", "keychain:", "vault:", "secret:"]
                    .iter()
                    .any(|scheme| part.to_ascii_lowercase().starts_with(scheme))
                && !["sk-", "ghp_", "github_pat_", "xoxb-", "AKIA"]
                    .iter()
                    .any(|prefix| part.starts_with(prefix))
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-:".contains(&b))
        });
    valid.then(|| value.to_owned())
}

fn origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") || !url.has_host() {
        return None;
    }
    let mut origin = url.origin().ascii_serialization();
    if matches!(url.path(), "/v1" | "/v1/") {
        origin.push_str("/v1");
    }
    Some(origin)
}

impl Role {
    pub fn selected(selection: &str, model: &dyn Model, upstream: &str, base_url: &str) -> Self {
        let binding = model.provenance();
        let revision = binding.binding_revision.as_str();
        Self {
            selection: identifier(selection),
            upstream_model: identifier(upstream),
            endpoint_id: identifier(binding.endpoint.as_str()),
            base_origin: origin(base_url),
            protocol: binding.protocol,
            provider_id: identifier(binding.provider.as_str()),
            binding_revision: super::store::is_digest(revision).then(|| revision.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_only_the_origin_and_known_api_root() {
        assert_eq!(
            origin("https://user:pw@host.test:444/private/key?q=token#secret").as_deref(),
            Some("https://host.test:444")
        );
        assert_eq!(
            origin("http://[::1]:8001/v1/?q=s").as_deref(),
            Some("http://[::1]:8001/v1")
        );
        assert_eq!(origin("file:///private/key"), None);
        assert_eq!(identifier("lab/Qwen-2.5").as_deref(), Some("lab/Qwen-2.5"));
        for private in [
            "/srv/private/key",
            "file:/secret",
            "env:PRIVATE_VARIABLE",
            "../key",
            "sk-private",
            "https://abc",
        ] {
            assert_eq!(identifier(private), None);
        }
    }
}
