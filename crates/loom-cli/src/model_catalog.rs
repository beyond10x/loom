//! The llm catalog `run --catalog` reads, and the models its route aliases name.
//!
//! With `--catalog <PATH>`, `--model` and `--classifier-model` each name a route alias of that
//! catalog, and each model is the port llm builds for the route's first target
//! ([`llm_models::port`]). Everything here happens before any model call, sends nothing and
//! resolves no secret; a refusal names the file, or the alias and the flag that gave it.
//!
//! The command line supplies no credential resolver, so an alias whose account needs a
//! credential is refused. A route that permits fallback to a second target is refused too: each
//! of Loom's model ports is single-attempt and Loom owns its retries (`model_retry`), so no
//! request is moved to another target behind its back.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use llm_core::{ErrorCode, Id, Model};
use llm_credentials::SecretResolver;
use llm_http::{HttpClient, Limits};
use llm_models::Port;
use llm_routing::{Catalog, CatalogDocument, MAX_CONFIG_BYTES};

/// One route as the catalog declares it: its first target's serving model, its target count and
/// whether it permits fallback.
struct Route {
    serving_model: Id,
    targets: usize,
    fallback_enabled: bool,
}

/// A validated llm catalog read from one file, with its routes by alias.
pub struct ModelCatalog {
    path: PathBuf,
    catalog: Catalog,
    routes: BTreeMap<String, Route>,
}

impl ModelCatalog {
    /// Reads and validates the catalog at `path`.
    ///
    /// # Errors
    /// A file that cannot be read, exceeds llm's catalog bound, or is not a valid
    /// `llm.catalog/1` document, naming the file.
    pub fn read(path: &Path) -> Result<Self, String> {
        let refused = |reason: String| format!("the model catalog {} {reason}", path.display());
        let mut bytes = Vec::new();
        crate::regular_file::open(path)
            .map_err(refused)?
            .take(MAX_CONFIG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| refused(format!("cannot be read: {error}")))?;
        let text = String::from_utf8(bytes).map_err(|_| refused("is not UTF-8".into()))?;
        let invalid = |error: llm_core::Error| {
            refused(format!(
                "is not a valid llm.catalog/1 document: {}",
                error.message
            ))
        };
        let document = CatalogDocument::parse(&text).map_err(invalid)?;
        let mut routes = BTreeMap::new();
        for route in &document.routes {
            let mut targets: Vec<_> = document
                .targets
                .iter()
                .filter(|target| target.route_id == route.id)
                .collect();
            targets.sort_by_key(|target| target.position);
            if let Some(first) = targets.first() {
                routes.insert(
                    route.alias.as_str().to_owned(),
                    Route {
                        serving_model: first.serving_model_id.clone(),
                        targets: targets.len(),
                        fallback_enabled: route.fallback_enabled,
                    },
                );
            }
        }
        let catalog = document.validate().map_err(invalid)?;
        Ok(Self {
            path: path.to_owned(),
            catalog,
            routes,
        })
    }

    /// The model the route `alias` names, for the command-line `flag` that gave it.
    ///
    /// # Errors
    /// An alias the catalog does not declare, a route that permits fallback to a second target,
    /// an account that needs a credential, or a serving model that cannot take a forced tool
    /// call, naming the alias and the flag.
    pub fn model(&self, flag: &str, alias: &str) -> Result<Port, String> {
        let refused = |reason: &str| {
            format!(
                "{flag} {alias}: the model catalog {} {reason}",
                self.path.display()
            )
        };
        let route = self
            .routes
            .get(alias)
            .ok_or_else(|| refused("declares no route alias by that name"))?;
        if route.fallback_enabled && route.targets > 1 {
            return Err(refused(
                "lets that route fall back to another target; b10x-loom serves one target per model and never falls back",
            ));
        }
        let http = HttpClient::new(Limits::default()).map_err(|error| refused(&error.message))?;
        let no_resolver = |_: &Id| None::<Arc<dyn SecretResolver>>;
        let port = llm_models::port(&self.catalog, &route.serving_model, http, &no_resolver)
            .map_err(|error| match error.code {
                ErrorCode::Unauthorized => refused(&format!(
                    "serves that route under an account that needs a credential, which the command line cannot supply ({})",
                    error.message
                )),
                _ => refused(&format!("cannot build that route's model: {}", error.message)),
            })?;
        let capabilities = port.capabilities();
        if !capabilities.tools || !capabilities.tool_choice {
            return Err(refused(
                "serves that route with a model that cannot take a forced tool call",
            ));
        }
        Ok(port)
    }
}
