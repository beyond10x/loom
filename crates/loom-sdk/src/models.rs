//! A host's models, loaded from a configuration file or an `llm.routes/1` document.
//!
//! [`from_config`] reads a [`ModelsConfig`] (YAML: `routes`, the path of a routes document
//! relative to the configuration file's directory, and `roles`, each a [`ModelRole`] and the
//! name of the model that serves it) and returns the model of each declared role as [`Models`].
//! [`from_routes`] returns the model one name of a routes document serves. Both build the model
//! with `llm_models::routes::Routes::port`, a route name or an `<endpoint>/<model-id>` name of a
//! declared endpoint, and resolve credentials from the document's `secret:` references:
//! `env:<VAR>`, and `file:<path>` where `~/` is the `HOME` of the process (a `~/` reference is
//! refused when `HOME` is unset). A `keychain:` reference is refused when the models are built:
//! this build does not enable llm's keychain feature. Loom never reads a secret itself, and
//! nothing is read or sent before the first turn: a variable or file that is missing is refused
//! `unauthorized` at that turn.
//!
//! Every file is read as a regular file only, and every refusal names the file and the role or
//! name it concerns.

use std::fmt;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use llm_core::Model;
use llm_http::{HttpClient, Limits};
use llm_models::routes::{RouteResolvers, Routes, SecretSources};
use serde_yaml_ng::Value;

/// A role Loom asks a model for, generated from `loom.models.ModelRole`.
pub use models_config::models::ModelRole;
/// The configuration of a host's models, generated from `loom.models.ModelsConfig`.
pub use models_config::models::ModelsConfig;
/// The model that serves one role, generated from `loom.models.RoleModel`.
pub use models_config::models::RoleModel;

/// The most bytes of a configuration or routes document read.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Why models could not be loaded; the message names the file and the role or name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsError {
    message: String,
}

impl ModelsError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn in_file(path: &Path, what: impl fmt::Display) -> Self {
        Self::new(format!("{}: {what}", path.display()))
    }
}

impl fmt::Display for ModelsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ModelsError {}

/// The models of a host: one for each role its configuration declares, over the routes document
/// the configuration names, which stays loaded so another name of it can be resolved.
pub struct Models {
    path: PathBuf,
    routes: Routes,
    resolvers: RouteResolvers,
    http: HttpClient,
    roles: Vec<(ModelRole, Arc<dyn Model>)>,
}

impl Models {
    /// The model of `role`, if the configuration declares it.
    #[must_use]
    pub fn get(&self, role: ModelRole) -> Option<Arc<dyn Model>> {
        self.roles
            .iter()
            .find(|(declared, _)| *declared == role)
            .map(|(_, model)| Arc::clone(model))
    }

    /// The model `name` serves in the routes document: a route, else `<endpoint>/<model-id>`.
    ///
    /// # Errors
    /// The name is not one the document serves; the message names the document and the name.
    pub fn model(&self, name: &str) -> Result<Arc<dyn Model>, ModelsError> {
        build(&self.path, &self.routes, &self.resolvers, &self.http, name)
    }
}

impl fmt::Debug for Models {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Models")
            .field("routes", &self.path)
            .field(
                "roles",
                &self.roles.iter().map(|(role, _)| role).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// Parses a models configuration (YAML).
///
/// # Errors
/// The text is not YAML, holds a field other than `routes` and `roles`, lacks either, names a
/// role other than `agent` and `classifier`, or lists a role twice; the message names the field
/// or the role.
pub fn parse_config(text: &str) -> Result<ModelsConfig, ModelsError> {
    if u64::try_from(text.len()).map_or(true, |length| length > MAX_BYTES) {
        return Err(ModelsError::new("the configuration exceeds its byte bound"));
    }
    let value: Value = serde_yaml_ng::from_str(text)
        .map_err(|error| ModelsError::new(format!("invalid YAML: {error}")))?;
    let document = fields(&value, "the configuration", &["routes", "roles"])?;
    let routes = text_field(document.get("routes"), "routes")?;
    let list = document
        .get("roles")
        .ok_or_else(|| ModelsError::new("field `roles` is missing"))?
        .as_sequence()
        .ok_or_else(|| ModelsError::new("field `roles` is not a list"))?;
    let mut roles: Vec<RoleModel> = Vec::new();
    for entry in list {
        let entry = fields(entry, "a role entry", &["role", "model"])?;
        let name = text_field(entry.get("role"), "role")?;
        let role = match name.as_str() {
            "agent" => ModelRole::Agent,
            "classifier" => ModelRole::Classifier,
            _ => {
                return Err(ModelsError::new(format!(
                    "role `{name}` is not declared: the roles are `agent` and `classifier`"
                )));
            }
        };
        if roles.iter().any(|declared| declared.role == role) {
            return Err(ModelsError::new(format!("role `{name}` is listed twice")));
        }
        let model = text_field(entry.get("model"), &format!("model of role `{name}`"))?;
        roles.push(RoleModel { role, model });
    }
    Ok(ModelsConfig { routes, roles })
}

/// The entries of the mapping `value`, refusing any key other than `allowed`.
fn fields<'a>(
    value: &'a Value,
    what: &str,
    allowed: &[&str],
) -> Result<std::collections::BTreeMap<&'a str, &'a Value>, ModelsError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| ModelsError::new(format!("{what} is not a mapping")))?;
    let mut entries = std::collections::BTreeMap::new();
    for (key, value) in mapping {
        let key = key
            .as_str()
            .ok_or_else(|| ModelsError::new(format!("{what} holds a key that is not text")))?;
        if !allowed.contains(&key) {
            return Err(ModelsError::new(format!("unknown field `{key}` in {what}")));
        }
        entries.insert(key, value);
    }
    Ok(entries)
}

/// A non-empty text field.
fn text_field(value: Option<&&Value>, name: &str) -> Result<String, ModelsError> {
    let value = value.ok_or_else(|| ModelsError::new(format!("field `{name}` is missing")))?;
    match value.as_str() {
        Some(text) if !text.trim().is_empty() => Ok(text.to_owned()),
        _ => Err(ModelsError::new(format!(
            "field `{name}` is not a non-empty text"
        ))),
    }
}

/// Loads the models a configuration file declares: the routes document it names (relative to
/// the file's directory) is read and validated, and the model of each declared role is built.
///
/// # Errors
/// The file or the routes document cannot be read as a regular file or is invalid, the
/// configuration is refused by [`parse_config`], or a role's model is a name the routes document
/// does not serve. The message names the file, and the role or name.
pub fn from_config(path: &Path) -> Result<Models, ModelsError> {
    let text = read(path)?;
    let config = parse_config(&text).map_err(|error| ModelsError::in_file(path, error))?;
    let routes_path = path.parent().map_or_else(
        || PathBuf::from(&config.routes),
        |dir| dir.join(&config.routes),
    );
    let routes = load_routes(&routes_path)?;
    let resolvers = resolvers(&routes_path, &routes)?;
    let http = http(&routes_path)?;
    let mut roles = Vec::new();
    for entry in &config.roles {
        let role = match entry.role {
            ModelRole::Agent => "agent",
            ModelRole::Classifier => "classifier",
        };
        let model = build(&routes_path, &routes, &resolvers, &http, &entry.model)
            .map_err(|error| ModelsError::in_file(path, format!("role `{role}`: {error}")))?;
        roles.push((entry.role, model));
    }
    Ok(Models {
        path: routes_path,
        routes,
        resolvers,
        http,
        roles,
    })
}

/// The model `name` serves in the `llm.routes/1` document at `path`: a route, else
/// `<endpoint>/<model-id>` of a declared endpoint.
///
/// # Errors
/// The document cannot be read as a regular file or is invalid, or does not serve the name. The
/// message names the file and the name.
pub fn from_routes(path: &Path, name: &str) -> Result<Arc<dyn Model>, ModelsError> {
    let routes = load_routes(path)?;
    let resolvers = resolvers(path, &routes)?;
    let http = http(path)?;
    build(path, &routes, &resolvers, &http, name)
}

fn load_routes(path: &Path) -> Result<Routes, ModelsError> {
    let text = read(path)?;
    Routes::parse(&text).map_err(|error| ModelsError::in_file(path, error.message))
}

fn resolvers(path: &Path, routes: &Routes) -> Result<RouteResolvers, ModelsError> {
    let mut sources = SecretSources::new();
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        sources = sources.with_home(PathBuf::from(home));
    }
    routes
        .resolvers(&sources)
        .map_err(|error| ModelsError::in_file(path, error.message))
}

fn http(path: &Path) -> Result<HttpClient, ModelsError> {
    HttpClient::new(Limits::default()).map_err(|error| ModelsError::in_file(path, error.message))
}

fn build(
    path: &Path,
    routes: &Routes,
    resolvers: &RouteResolvers,
    http: &HttpClient,
    name: &str,
) -> Result<Arc<dyn Model>, ModelsError> {
    routes
        .port(name, http.clone(), resolvers)
        .map(|port| Arc::new(port) as Arc<dyn Model>)
        .map_err(|error| ModelsError::in_file(path, format!("model `{name}`: {}", error.message)))
}

/// The text of `path`, read as a regular file of at most [`MAX_BYTES`].
///
/// The path is checked before it is opened and the opened handle again, so a named pipe or a
/// device is refused before it is read; on Unix the open does not wait for a writer.
fn read(path: &Path) -> Result<String, ModelsError> {
    let refused = |reason: &str| ModelsError::in_file(path, reason);
    let metadata =
        std::fs::metadata(path).map_err(|error| refused(&format!("cannot be read: {error}")))?;
    if !metadata.is_file() {
        return Err(refused("is not a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| refused(&format!("cannot be read: {error}")))?;
    let opened = file
        .metadata()
        .map_err(|error| refused(&format!("cannot be read: {error}")))?;
    if !opened.is_file() {
        return Err(refused("is not a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| refused(&format!("cannot be read: {error}")))?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > MAX_BYTES) {
        return Err(refused("exceeds its byte bound"));
    }
    String::from_utf8(bytes).map_err(|_| refused("is not UTF-8 text"))
}
