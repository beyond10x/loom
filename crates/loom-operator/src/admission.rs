//! The checks a `LoomAgent` passes before any child is rendered, in this order; the first refusal
//! is the answer:
//!
//! 1. `spec.agent`, as YAML, through `b10x_loom_agent::check_text` against the plugin catalog:
//!    the check `b10x-loom agent check` runs, in-process (`Malformed`, `SecretLiteral`,
//!    `UnknownProtocol`, `UnknownAction`, `UnboundSource`, `HttpMethod`, `SendWithoutWrites`,
//!    `EmptyChannelAllow`), then a `runtime` without `connectors`, which `agent serve` needs and
//!    the check does not read (`Malformed`), as the decoder refused it before 0.42.0; an
//!    `AgentServe` resource without `spec.agent` is `Malformed` at `spec.agent`. For the
//!    `Command` variant (`spec.command` present), whose
//!    document the image's program reads and `agent serve` does not, only the credential walk
//!    of that library (`SecretLiteral`) and the `loom.agent/1` decoder (`Malformed`), and only
//!    when the resource has one: a `Command` resource may leave `spec.agent` out.
//! 2. `spec.routes` walked for a credential by the same library (`SecretLiteral`); a routes
//!    document that is not a mapping is `Malformed`, and so is one llm's strict `llm.routes/1`
//!    parser (`llm_models::routes::Routes::parse`, the reader of the pod's `routes.yaml`)
//!    refuses: an unknown field, an `auth.secret` that is a value and not an `env:`, `file:` or
//!    `keychain:` reference, a route to an undeclared endpoint and the like.
//! 3. `spec.connectorsConfig`, which the ConfigMap receives verbatim, walked for a credential
//!    (`SecretLiteral`) three ways, so neither an escape, a comment nor a leading word hides one:
//!    decoded as TOML (`Malformed` when it is not TOML) with every key and string of the decoded
//!    table walked whole; every word of each decoded key and string; and every word of the raw
//!    text, comments included, named by its line. A word is what whitespace and `=`, `"`, `'`,
//!    `,`, `[`, `]`, `{`, `}` and the shell's `<`, `>`, `|`, `&`, `(`, `)`, `$` and backtick
//!    separate, its leading `#` dropped; a word holding `:` or `;` is followed by each piece
//!    between them (`user:ghp_…`, `x-api-key:sk-…`); a word or piece that is a token prefix
//!    ending in a space (`Bearer`, also in `Authorization:Bearer`) is walked with the one after.
//! 4. An empty `spec.routes` while the controller configures no model route (`NoModelRoute`);
//!    a `Command` resource then gets an empty `routes.yaml`. With a model route, the default
//!    routes document routes each model a role of `spec.agent` names; a `Command` resource
//!    without `spec.agent` names none, so its document declares the endpoint and no route. With
//!    `spec.boundary.model` the endpoint is the boundary's model relay, `http://127.0.0.1:8401/v1`,
//!    in place of the model route's URL; its protocol and model stay the model route's. An empty
//!    `spec.routes` beside a `spec.boundary` that declares an upstream but no `model`, whose pod's
//!    NetworkPolicy opens no model route, is `NoModelRoute` at `spec.boundary.model` for an
//!    `AgentServe` resource, whose agent would reach no model; a `Command` resource gets an empty
//!    `routes.yaml`, with or without a model route.
//! 5. A present `spec.boundary` (story boundary-container-projected-token, design D243): one
//!    declaring none of `model`, `connectors` and `git` is `Malformed`, and a `spec.secretRef`
//!    or `spec.secretFiles` beside it is `BoundaryWithSecret` at that member. Then each declared
//!    upstream in that order: its `upstream` is a plain `http://` URL of a cluster service
//!    ([`cluster_service`]: no user information, query or fragment; the host
//!    `<service>.<namespace>.svc` or `<service>.<namespace>.svc.cluster.local` of DNS-1123
//!    labels; an optional port and path of plain segments) that `b10x_loom_boundary::Upstream`
//!    parses, else `BoundaryUpstreamInvalid`, an `https` upstream included; an
//!    `allowedBoundaryUpstreams` entry of the configuration has its kind (`model`, `connectors`,
//!    `git`) and exactly its URL, else `BoundaryUpstreamNotAllowed` at `upstream`; its `audience`
//!    is one word, neither empty nor holding whitespace or a control character (`Malformed`);
//!    `connectors.operations` and `git.repositories` are not empty and give each entry once and
//!    in form for the operator and for the relay (an operation id 1 to 256 bytes of
//!    `[A-Za-z0-9._-]`, a repository `<owner>/<repo>` of such parts, neither `.` nor `..`, the
//!    repository not ending in `.git`; neither starting with `-`, which would read as a flag),
//!    else `Malformed` at the member; and the relay `b10x-loom-boundary` builds from those
//!    values, as `b10x-loom agent boundary` builds it from the rendered flags, is accepted, else
//!    `Malformed` at the list. Then the member is walked for a credential, whole and each string
//!    word by word as `spec.command` is (`SecretLiteral`), and each declared `audience` is
//!    exactly that of an entry of its kind and URL, else `BoundaryUpstreamNotAllowed` at
//!    `audience` (an audience the API server accepts would make the token a Kubernetes API
//!    credential). A credential-free pod's NetworkPolicy
//!    opens DNS and the upstreams only (story credential-free-network-policy), so a
//!    `deploy.kubernetes.egress` entry of `spec.agent`, in order, naming a host is
//!    `BoundaryEgressHost` at its `host`, and one naming a namespace `BoundaryEgressNamespace` at
//!    its `namespace`.
//! 6. A `spec.secretRef` or `spec.secretFiles` outside the configuration's `allowedSecretNames`
//!    (`SecretNotAllowed`).
//! 7. A `deploy.kubernetes.namespace` other than the resource's (`NamespaceMismatch`).
//! 8. A `deploy.kubernetes.egress` entry naming neither a host nor a namespace, whose rule would
//!    admit every destination on its port, or naming a host outside `allowedEgressHosts`
//!    (`EgressHostNotAllowed`).
//! 9. `spec.command`, each entrypoint and argument walked whole and word by word as the
//!    connectors text is (`SecretLiteral`); one beyond the walk's bound is `TooLarge`.
//! 10. `spec.env`, on either variant: the list walked whole, names and values, then each value
//!     word by word as `spec.command` is (`SecretLiteral`; `TooLarge` beyond the walk's bound);
//!     then, entry by entry, a name that is not an environment variable name
//!     `[A-Za-z_][A-Za-z0-9_]*` (`NotAVariableName`, `loom.agent`'s reason for the same rule), a
//!     name the controller sets on a container itself ([`CONTROLLER_ENV`]) or one an earlier
//!     entry gives (`EnvNameConflict`). Then each `spec.env` value and each `spec.command`
//!     entrypoint and argument again, as the program may read it after the kubelet's `$(VAR)`
//!     expansion (`$$` is `$`). The walk knows only the controller's own environment (with
//!     `spec.boundary`, the URL of each relay it declares) and the `spec.env` entries: a value
//!     expands against those before it, a command against all of them. A name it does not know
//!     (a key of the `spec.secretRef` Secret, the service environment such as `KUBERNETES_PORT`)
//!     is read twice: left as written, and as [`UNKNOWN_REFERENCE`], a scheme, since such a
//!     reference may supply one. Each reading is walked whole and word by word (`SecretLiteral`,
//!     its detail saying it was found after expansion; `TooLarge` when the references add more
//!     than [`MAX_EXPANDED_BYTES`]).
//! 11. A non-empty `spec.configFiles`, only for `Command`: on an `AgentServe` resource, whose
//!     program reads none, it is `Malformed`. Then entry by entry, a name that is not a plain
//!     file name (`[A-Za-z0-9._-]+`, not `.`, not starting with `..`, at most [`MAX_CONFIG_FILE_NAME`] bytes:
//!     `ConfigFileInvalid`), one an earlier entry gives or one the ConfigMap already holds
//!     (`routes.yaml`, `connectors.toml`, and `agent.yaml` when `spec.agent` is present:
//!     `ConfigFileConflict`).
//! 12. The ConfigMap, its keys and values together, over Kubernetes's limit of
//!     [`MAX_CONFIG_MAP_BYTES`] (`TooLarge`), refused rather than failing at apply.
//! 13. Each `spec.configFiles` content, which the controller decodes nothing of, walked for a
//!     credential (`SecretLiteral`) by every walk step 3 makes and more: the list whole, names
//!     and contents; each content as a YAML stream, every document in turn and every string the
//!     walk reads in it (key, value or tag, aliases replayed, a duplicate key included) whole
//!     and word by word; after a scan error, the content split at its lines that are exactly
//!     `---` (optionally followed by whitespace or a comment), each piece walked as a stream
//!     of its own up to its own first scan error, under the same budget; then as a TOML table
//!     when it reads as one, whole and word by word, so a YAML or TOML escape (`\x67`,
//!     `\u0067`) does not hide one from a program that reads the file as either; and every word
//!     of the raw text, named by its line. An encoding neither reader undoes (base64, say) can
//!     still hide one, as can a YAML escape after a scan error and before the next `---` line:
//!     the walk knows credential shapes, not every secret. A content beyond the walk's bound,
//!     or a YAML stream it cannot read whole (its aliases or nesting cost more than the agent
//!     decoder's bound allows, or the parser's recursion or repetition limit stops it), is
//!     `TooLarge` once the strings read before it stopped hold no credential.
//! 14. A `spec.workspace.mountPath` or `spec.secretFiles.mountPath` that is not absolute and
//!     plain, is, holds or lies beneath a directory the controller mounts ([`RESERVED_MOUNTS`])
//!     or the other one (`MountPathInvalid`).
//! 15. A `spec.workspace.claimName` outside the configuration's `allowedClaimNames`
//!     (`ClaimNotAllowed`).
//! 16. `spec.seccompProfile` and `spec.appArmorProfile`: on an `AgentServe` resource
//!     `Malformed`; a type other than `RuntimeDefault`, `Localhost` and `Unconfined`, a
//!     `localhostProfile` beside `RuntimeDefault` or `Unconfined`, a `Localhost` profile without
//!     one, or one that is not a plain name or path (`ProfileInvalid`); an `Unconfined` profile
//!     the configuration's `allowUnconfinedProfiles` does not allow, or a `Localhost` name
//!     outside `allowedSeccompLocalhostProfiles` or `allowedAppArmorLocalhostProfiles`
//!     (`ProfileNotAllowed`).
//! 17. The size of the pod's containers but the boundary sidecar, requests and limits alike:
//!     `spec.agent.deploy.kubernetes.resources` when the resource declares it, else for an
//!     `AgentServe` resource the configuration's `defaultResources` and for a `Command` resource
//!     its `defaultCommandResources`. A `Command` resource that declares none under a
//!     configuration without `defaultCommandResources` is `ResourcesRequired` at
//!     `spec.agent.deploy.kubernetes.resources`: Loom cannot size a program it does not know.
//!
//! The walk of steps 2, 3, 5 and 13 reads a text that is not a `loom.agent/1` document only
//! within the agent decoder's bounds, [`MAX_WALKED_OPENERS`] `[` or `{` bytes and
//! [`MAX_WALKED_BYTES`]; a document beyond either is refused `TooLarge` rather than admitted
//! unread. Words are walked in sequences that each stay within both bounds.
//!
//! A refusal reads `refused: <reason> at <member>: <detail>`, as `b10x-loom agent check` prints
//! it, the member spelt from the resource (`spec.agent.bindings[1].base_url`); its detail never
//! repeats a value of the resource. An admitted spec reads as the check's line, `checked <agent>:
//! <n> states, <m> bindings, protocol <protocol>`; an admitted `Command` spec as `admitted <agent>
//! for its own command: …`, or `admitted for its own command: …` without `spec.agent`.

use std::collections::{BTreeMap, HashMap};

use b10x_loom_agent::{
    Checked, RefusalReason, StreamWalk, TOKEN_PREFIXES, check_text, spelling,
    yaml_stream_credential,
};
use b10x_loom_boundary::{Relay as BoundaryRelay, Upstream as BoundaryUrl};
use llm_models::routes::Routes;
use loom_sdk::agent::{AgentSpec, parse};
use serde_yaml_ng::{Mapping, Value};

use b10x_loom_resource::model::AgentVariant;

use crate::config::{Config, ModelRoute, Resources};
use crate::render::{
    Admits, BOUNDARY_TOKENS_DIR, CONFIG_DIR, CONTROLLER_ENV, RELAYS, Upstream, controller_env,
    upstreams,
};
use crate::resource::{Boundary, CommandSpec, ConfigFile, EnvVar, LoomAgentSpec};

/// The exit code a refusal is recorded with, as `b10x-loom agent check` exits on one.
pub const REFUSED: i64 = 3;

/// The most `[` and `{` bytes a walked routes or connectors text may hold: the agent decoder's
/// bound, beyond which `check_text` does not walk a text that is not a `loom.agent/1` document.
pub const MAX_WALKED_OPENERS: usize = loom_sdk::agent::MAX_FLOW_OPENERS;

/// The longest walked routes or connectors text, in bytes: the agent decoder's byte bound, beyond
/// which `check_text` does not walk a text that is not a `loom.agent/1` document.
pub const MAX_WALKED_BYTES: usize = 2 * 1024 * 1024;

/// The name of the endpoint the default routes document declares.
pub const MODEL_ROUTE_ENDPOINT: &str = "model-route";

/// Why a resource is refused: the reason, the member of the resource and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The reason, in CamelCase: a `loom.agent.RefusalReason` or one of the controller's.
    pub reason: String,
    /// The member, spelt from the resource: `spec.agent.deploy.kubernetes.namespace`.
    pub member: String,
    /// What is wrong, never repeating a value.
    pub detail: String,
}

impl Refusal {
    fn new(reason: &str, member: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            reason: reason.to_owned(),
            member: member.into(),
            detail: detail.into(),
        }
    }

    /// The line the refusal is recorded as in `status.check.message`.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "refused: {} at {}: {}",
            self.reason, self.member, self.detail
        )
    }
}

/// An admitted resource: its decoded spec, the files of its ConfigMap and the check's answer.
#[derive(Debug, Clone)]
pub struct Admitted {
    /// The decoded `loom.agent/1` document; absent for a `Command` resource without one.
    pub spec: Option<AgentSpec>,
    /// `agent.yaml`: `spec.agent` as YAML; absent, and no such key, without `spec.agent`.
    pub agent: Option<String>,
    /// `routes.yaml`: `spec.routes` as YAML, or the default routes document.
    pub routes: String,
    /// `connectors.toml`: `spec.connectorsConfig`, empty when absent.
    pub connectors: String,
    /// Whether `routes.yaml` is the default routes document, which opens the model route.
    pub default_routes: bool,
    /// What the pod runs.
    pub variant: AgentVariant,
    /// What the serve check found; absent for the `Command` variant, whose spec is decoded and
    /// walked for a credential but not checked against the plugin catalog.
    pub checked: Option<Checked>,
    /// `spec.configFiles`, each a key of the ConfigMap by its name, its content verbatim.
    pub config_files: Vec<ConfigFile>,
    /// The CPU and memory every container of the pod but the boundary sidecar requests and is
    /// limited to (step 17).
    pub resources: Resources,
}

/// The data of a `<name>-spec` ConfigMap holding `connectors`, `routes`, `agent` when present
/// and each of `files`.
fn config_map_data(
    connectors: &str,
    routes: &str,
    agent: Option<&str>,
    files: &[ConfigFile],
) -> BTreeMap<String, String> {
    let mut data = BTreeMap::from([
        ("connectors.toml".to_owned(), connectors.to_owned()),
        ("routes.yaml".to_owned(), routes.to_owned()),
    ]);
    if let Some(agent) = agent {
        data.insert("agent.yaml".to_owned(), agent.to_owned());
    }
    for file in files {
        data.insert(file.name.clone(), file.content.clone());
    }
    data
}

impl Admitted {
    /// The data of the `<name>-spec` ConfigMap: `routes.yaml`, `connectors.toml`, `agent.yaml`
    /// when the resource has `spec.agent`, and each of `spec.configFiles`.
    #[must_use]
    pub fn config_map_data(&self) -> BTreeMap<String, String> {
        config_map_data(
            &self.connectors,
            &self.routes,
            self.agent.as_deref(),
            &self.config_files,
        )
    }

    /// The line the check is recorded as in `status.check.message`.
    #[must_use]
    pub fn message(&self) -> String {
        match &self.checked {
            Some(checked) => format!(
                "checked {}: {} states, {} bindings, protocol {}",
                checked.agent,
                checked.states,
                checked.bindings,
                checked.protocol.as_deref().unwrap_or("none")
            ),
            None => {
                let files = match self.config_files.len() {
                    0 => String::new(),
                    1 => format!("1 configuration file with no credential, at {CONFIG_DIR}"),
                    n => format!("{n} configuration files with no credential, at {CONFIG_DIR}"),
                };
                match (&self.spec, files.is_empty()) {
                    (Some(spec), true) => format!(
                        "admitted {} for its own command: a loom.agent/1 document with no \
                         credential, at {COMMAND_SPEC_FILE}",
                        spec.agent.id.0
                    ),
                    (Some(spec), false) => format!(
                        "admitted {} for its own command: a loom.agent/1 document with no \
                         credential, at {COMMAND_SPEC_FILE}; {files}",
                        spec.agent.id.0
                    ),
                    (None, true) => {
                        "admitted for its own command: no loom.agent/1 document and no \
                         configuration file"
                            .to_owned()
                    }
                    (None, false) => format!("admitted for its own command: {files}"),
                }
            }
        }
    }
}

/// Where the `Command` variant's container reads `spec.agent`.
pub const COMMAND_SPEC_FILE: &str = "/config/agent.yaml";

/// The paths the controller mounts itself: the serve variant's spec directory, the `Command`
/// variant's `/config`, the state directory and `/tmp`. A `spec.workspace` or
/// `spec.secretFiles.mountPath` may be none of them, nor beneath or above one.
pub const RESERVED_MOUNTS: [&str; 4] = ["/etc/agent", "/config", "/var/lib/agent", "/tmp"];

/// The longest mount path the controller accepts, in bytes.
pub const MAX_MOUNT_PATH: usize = 1024;

/// Whether one of the two absolute plain paths is the other or beneath it.
fn overlaps(a: &str, b: &str) -> bool {
    let beneath = |inner: &str, outer: &str| {
        inner == outer
            || inner
                .strip_prefix(outer)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    beneath(a, b) || beneath(b, a)
}

/// Refuses `path`, the member `at`, unless it is absolute and plain: no control character, no
/// empty, `.` or `..` component, not `/`, no trailing `/`, at most [`MAX_MOUNT_PATH`] bytes.
fn plain_mount_path(path: &str, at: &str) -> Result<(), Refusal> {
    let refused = |why: &str| Err(Refusal::new("MountPathInvalid", at, why));
    if path.len() > MAX_MOUNT_PATH {
        return refused("it is longer than the controller accepts");
    }
    if path.chars().any(char::is_control) {
        return refused("it holds a control character");
    }
    let Some(rest) = path.strip_prefix('/') else {
        return refused("it is not an absolute path");
    };
    if rest.is_empty() {
        return refused("it is the root directory");
    }
    if rest
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return refused("it has an empty, `.` or `..` component");
    }
    Ok(())
}

/// Refuses a workspace or Secret mount path that is not plain, overlaps a mount the controller
/// makes, or overlaps the other.
fn mount_paths(spec: &LoomAgentSpec) -> Result<(), Refusal> {
    let mut paths = Vec::new();
    if let Some(workspace) = &spec.workspace {
        paths.push((workspace.mount_path.as_str(), "spec.workspace.mountPath"));
    }
    if let Some(files) = &spec.secret_files {
        paths.push((files.mount_path.as_str(), "spec.secretFiles.mountPath"));
    }
    for (path, at) in &paths {
        plain_mount_path(path, at)?;
        if RESERVED_MOUNTS
            .iter()
            .any(|reserved| overlaps(path, reserved))
        {
            return Err(Refusal::new(
                "MountPathInvalid",
                *at,
                "it is or overlaps a directory the controller mounts",
            ));
        }
    }
    if let [(first, _), (second, at)] = paths.as_slice()
        && overlaps(first, second)
    {
        return Err(Refusal::new(
            "MountPathInvalid",
            *at,
            "it is or overlaps spec.workspace.mountPath",
        ));
    }
    Ok(())
}

/// Walks `spec.command`, each entry whole and word by word, for a credential.
fn command_credential(command: &CommandSpec) -> Result<(), Refusal> {
    credential(&yaml(command, "spec.command")?, "spec.command")?;
    let mut found = Vec::new();
    for (list, items) in [("entrypoint", &command.entrypoint), ("args", &command.args)] {
        for (index, item) in items.iter().flatten().enumerate() {
            found.extend(words(item).into_iter().map(|text| Word {
                text,
                member: format!("spec.command.{list}[{index}]"),
                line: None,
            }));
        }
    }
    word_credential(&found, "spec.command")
}

/// Walks `spec.env` for a credential, the list whole (names and values) and then each value word
/// by word as `spec.command` is; one beyond the walk's bound is `TooLarge`.
fn env_credential(env: &[EnvVar]) -> Result<(), Refusal> {
    credential(&yaml(&env, "spec.env")?, "spec.env")?;
    let found: Vec<Word> = env
        .iter()
        .enumerate()
        .flat_map(|(index, entry)| {
            words(&entry.value).into_iter().map(move |text| Word {
                text,
                member: format!("spec.env[{index}].value"),
                line: None,
            })
        })
        .collect();
    word_credential(&found, "spec.env")
}

/// Whether `name` is an environment variable name, `[A-Za-z_][A-Za-z0-9_]*`: the rule
/// `loom.agent` holds a `bearer_env` to.
fn variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Refuses a `spec.env` name that is not a variable name (`NotAVariableName`), one the
/// controller sets itself ([`CONTROLLER_ENV`]) or one an earlier entry gives (`EnvNameConflict`),
/// never repeating the name.
fn env_names(env: &[EnvVar]) -> Result<(), Refusal> {
    let mut seen: HashMap<&str, usize> = HashMap::with_capacity(env.len());
    for (index, entry) in env.iter().enumerate() {
        let at = format!("spec.env[{index}].name");
        if !variable_name(&entry.name) {
            return Err(Refusal::new(
                spelling(RefusalReason::NotAVariableName),
                at,
                "it is not an environment variable name, [A-Za-z_][A-Za-z0-9_]*",
            ));
        }
        if CONTROLLER_ENV.contains(&entry.name.as_str()) {
            return Err(Refusal::new(
                "EnvNameConflict",
                at,
                "the controller sets it on the container",
            ));
        }
        if let Some(first) = seen.insert(entry.name.as_str(), index) {
            return Err(Refusal::new(
                "EnvNameConflict",
                at,
                format!("it repeats spec.env[{first}].name"),
            ));
        }
    }
    Ok(())
}

/// The most bytes `$(VAR)` references may add to `spec.env` and `spec.command` together when
/// expanded, in each of the two readings the walk takes; beyond it the expanded text is not
/// walked and the resource is `TooLarge`.
pub const MAX_EXPANDED_BYTES: usize = MAX_WALKED_BYTES;

/// What the walk puts in place of a reference to a name it does not know, in its second
/// reading: a scheme. The kubelet also expands from the `spec.secretRef` Secret's keys and the
/// service environment, which every pod has (`KUBERNETES_PORT` is `tcp://<address>:443`), so
/// such a reference may supply the `scheme://` that makes the text after it a URL.
pub const UNKNOWN_REFERENCE: &str = "a://";

/// `text` as the kubelet expands it against `known`: `$$` is `$`, `$(NAME)` is the value of
/// `NAME` when `known` holds it, any other `$` is itself. A reference to a name `known` does not
/// hold stays as written, or becomes `unknown` when one is given. Each substituted value is
/// charged to `budget`; `None` when it runs out.
fn expand(
    text: &str,
    known: &HashMap<&str, String>,
    unknown: Option<&str>,
    budget: &mut usize,
) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if let Some(tail) = after.strip_prefix('$') {
            out.push('$');
            rest = tail;
        } else if let Some(inner) = after.strip_prefix('(')
            && let Some(close) = inner.find(')')
        {
            let name = &inner[..close];
            match (known.get(name), unknown) {
                (Some(value), _) => {
                    *budget = budget.checked_sub(value.len())?;
                    out.push_str(value);
                }
                (None, Some(placeholder)) => {
                    *budget = budget.checked_sub(placeholder.len())?;
                    out.push_str(placeholder);
                }
                (None, None) => {
                    out.push_str("$(");
                    out.push_str(name);
                    out.push(')');
                }
            }
            rest = &inner[close + 1..];
        } else {
            out.push('$');
            rest = after;
        }
    }
    out.push_str(rest);
    Some(out)
}

/// One way of reading the environment the kubelet expands from: what the walk knows of each
/// name, what a reference to any other becomes, and the bytes its references may still add.
struct Reading<'s> {
    known: HashMap<&'s str, String>,
    unknown: Option<&'static str>,
    budget: usize,
}

/// The words the text `written` at `member` is walked as after expansion: each distinct
/// expanded reading whole, then each word of it; none for a reading that changed nothing,
/// since the text as written was walked already.
fn expanded_words(written: &str, readings: [String; 2], member: &str, out: &mut Vec<Word>) {
    let [first, second] = readings;
    let distinct = if second == first {
        vec![first]
    } else {
        vec![first, second]
    };
    for expanded in distinct {
        if expanded == written {
            continue;
        }
        let pieces = words(&expanded);
        out.push(Word {
            text: expanded,
            member: member.to_owned(),
            line: None,
        });
        out.extend(pieces.into_iter().map(|text| Word {
            text,
            member: member.to_owned(),
            line: None,
        }));
    }
}

/// Walks `spec.env` and `spec.command` as the program may read them after the kubelet's
/// `$(VAR)` expansion. The walk knows the controller's own environment of `variant` and each
/// `spec.env` entry once declared: a value expands against those before it, an entrypoint or
/// argument against all of them. It does not know the `spec.secretRef` Secret's keys nor the
/// service environment, which the kubelet also expands from, so it reads each text twice: with
/// a reference to a name it does not know left as written, as the kubelet leaves one it does
/// not know either, and with that reference as [`UNKNOWN_REFERENCE`], a scheme, so a URL whose
/// scheme such a reference supplies shows its user information. A credential found in either
/// reading is `SecretLiteral` at the member, never repeating it; references adding more than
/// [`MAX_EXPANDED_BYTES`] to a reading are `TooLarge`.
fn expanded_credential(spec: &LoomAgentSpec, variant: AgentVariant) -> Result<(), Refusal> {
    let own: HashMap<&str, String> = controller_env(variant, spec.boundary.as_ref())
        .into_iter()
        .map(|(name, value)| (name, value.to_owned()))
        .collect();
    let mut readings = [None, Some(UNKNOWN_REFERENCE)].map(|unknown| Reading {
        known: own.clone(),
        unknown,
        budget: MAX_EXPANDED_BYTES,
    });
    let too_large = |member: &str| {
        Refusal::new(
            "TooLarge",
            member,
            format!(
                "its `$(VAR)` references, expanded, add more than {MAX_EXPANDED_BYTES} bytes, \
                 more than the credential walk reads"
            ),
        )
    };
    let read = |readings: &mut [Reading<'_>; 2], text: &str, member: &str| {
        let mut out = [String::new(), String::new()];
        for (slot, reading) in out.iter_mut().zip(readings.iter_mut()) {
            *slot = expand(text, &reading.known, reading.unknown, &mut reading.budget)
                .ok_or_else(|| too_large(member))?;
        }
        Ok::<_, Refusal>(out)
    };
    let mut found = Vec::new();
    for (index, entry) in spec.env.iter().flatten().enumerate() {
        let member = format!("spec.env[{index}].value");
        let expanded = read(&mut readings, &entry.value, &member)?;
        for (reading, value) in readings.iter_mut().zip(expanded.iter()) {
            reading.known.insert(entry.name.as_str(), value.clone());
        }
        expanded_words(&entry.value, expanded, &member, &mut found);
    }
    if let Some(command) = &spec.command {
        for (list, items) in [("entrypoint", &command.entrypoint), ("args", &command.args)] {
            for (index, item) in items.iter().flatten().enumerate() {
                let member = format!("spec.command.{list}[{index}]");
                let expanded = read(&mut readings, item, &member)?;
                expanded_words(item, expanded, &member, &mut found);
            }
        }
    }
    word_credential(&found, "spec.env").map_err(|refusal| {
        if refusal.reason == spelling(RefusalReason::SecretLiteral) {
            Refusal {
                detail: format!("{}, after `$(VAR)` expansion", refusal.detail),
                ..refusal
            }
        } else {
            refusal
        }
    })
}

/// `member` of a document at `at` in the resource.
fn within(at: &str, member: &str) -> String {
    if member == "document" {
        at.to_owned()
    } else if member.starts_with('[') {
        format!("{at}{member}")
    } else {
        format!("{at}.{member}")
    }
}

/// How many `[` and `{` bytes `text` holds.
fn openers(text: &str) -> usize {
    text.bytes()
        .filter(|byte| matches!(byte, b'[' | b'{'))
        .count()
}

/// The member, within `text`, and the shape of the first credential `text`, a YAML document
/// standing at `at` in the resource, holds; `TooLarge` when the text is beyond what the walk
/// reads.
fn first_credential(text: &str, at: &str) -> Result<Option<(String, String)>, Refusal> {
    if openers(text) > MAX_WALKED_OPENERS {
        return Err(Refusal::new(
            "TooLarge",
            at,
            format!(
                "it holds more than {MAX_WALKED_OPENERS} `[` or `{{` bytes, more than the \
                 credential walk reads"
            ),
        ));
    }
    if text.len() > MAX_WALKED_BYTES {
        return Err(Refusal::new(
            "TooLarge",
            at,
            format!(
                "it is longer than {MAX_WALKED_BYTES} bytes as YAML, more than the credential \
                 walk reads"
            ),
        ));
    }
    Ok(match check_text(text, &|_| None) {
        Err(refused) if refused.reason == RefusalReason::SecretLiteral => {
            Some((refused.member, refused.detail))
        }
        _ => None,
    })
}

/// The first credential `text`, a YAML document standing at `at` in the resource, holds, as a
/// refusal; `TooLarge` when the text is beyond what the walk reads.
fn credential(text: &str, at: &str) -> Result<(), Refusal> {
    match first_credential(text, at)? {
        Some((member, detail)) => Err(Refusal::new(
            spelling(RefusalReason::SecretLiteral),
            within(at, &member),
            detail,
        )),
        None => Ok(()),
    }
}

/// One word walked for a credential: its text, the member it stands in and, for a word of the
/// raw text, its line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Word {
    text: String,
    member: String,
    line: Option<usize>,
}

/// What a word may cost in a walked sequence, at most: YAML writes a byte as up to four, and an
/// item adds its `- `, its quotes and its newline.
fn cost(word: &str) -> usize {
    word.len().saturating_mul(4).saturating_add(8)
}

/// The words of `text`, as the module documentation separates them; a token prefix ending in a
/// space (`Bearer `) is joined with the word or piece after it.
fn words(text: &str) -> Vec<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in text
        .split(|c: char| c.is_whitespace() || "=\"',[]{}<>|&()$`".contains(c))
        .map(|part| part.trim_start_matches('#'))
        .filter(|part| !part.is_empty())
    {
        parts.push(part);
        if part.contains([':', ';']) {
            parts.extend(part.split([':', ';']).filter(|piece| !piece.is_empty()));
        }
    }
    let spaced = |part: &str| {
        TOKEN_PREFIXES
            .iter()
            .any(|prefix| prefix.strip_suffix(' ') == Some(part))
    };
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| match parts.get(index + 1) {
            Some(next) if spaced(part) => format!("{part} {next}"),
            _ => (*part).to_owned(),
        })
        .collect()
}

/// The words of each key and string of `table`, the table at `member`, in document order: a
/// key's words before what it holds, standing in the table that holds the key, so a member
/// never spells a key found to be a credential.
fn decoded_words(table: &toml::Table, member: &str, out: &mut Vec<Word>) {
    for (key, value) in table {
        out.extend(words(key).into_iter().map(|text| Word {
            text,
            member: member.to_owned(),
            line: None,
        }));
        value_words(value, &format!("{member}.{key}"), out);
    }
}

/// The words of each key and string of `value`, the value at `member`.
fn value_words(value: &toml::Value, member: &str, out: &mut Vec<Word>) {
    match value {
        toml::Value::String(text) => out.extend(words(text).into_iter().map(|text| Word {
            text,
            member: member.to_owned(),
            line: None,
        })),
        toml::Value::Table(table) => decoded_words(table, member, out),
        toml::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                value_words(item, &format!("{member}[{index}]"), out);
            }
        }
        _ => {}
    }
}

/// The words of `text`, a raw text at `member`, each with its line.
fn raw_words(text: &str, member: &str) -> Vec<Word> {
    text.lines()
        .enumerate()
        .flat_map(|(index, line)| {
            words(line).into_iter().map(move |text| Word {
                text,
                member: member.to_owned(),
                line: Some(index + 1),
            })
        })
        .collect()
}

/// The first credential among `words` as a refusal at the word's member, its line added to the
/// detail. The words are walked as YAML sequences, each within the walk's bounds.
fn word_credential(words: &[Word], at: &str) -> Result<(), Refusal> {
    let mut start = 0;
    while start < words.len() {
        let (mut end, mut bytes, mut opened) = (start, 0_usize, 0_usize);
        while let Some(word) = words.get(end) {
            let (more_bytes, more_opened) = (cost(&word.text), openers(&word.text));
            if end > start
                && (bytes + more_bytes > MAX_WALKED_BYTES
                    || opened + more_opened > MAX_WALKED_OPENERS)
            {
                break;
            }
            bytes += more_bytes;
            opened += more_opened;
            end += 1;
        }
        let chunk = &words[start..end];
        let texts: Vec<&str> = chunk.iter().map(|word| word.text.as_str()).collect();
        let text = serde_yaml_ng::to_string(&texts).map_err(|_| {
            Refusal::new(
                spelling(RefusalReason::Malformed),
                at,
                "cannot be written as YAML",
            )
        })?;
        if let Some((member, detail)) = first_credential(&text, &chunk[0].member)? {
            let found = member
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| chunk.get(index));
            return Err(match found {
                Some(word) => Refusal::new(
                    spelling(RefusalReason::SecretLiteral),
                    word.member.clone(),
                    match word.line {
                        Some(line) => format!("{detail} (line {line})"),
                        None => detail,
                    },
                ),
                None => Refusal::new(spelling(RefusalReason::SecretLiteral), at, detail),
            });
        }
        start = end;
    }
    Ok(())
}

/// Walks `text`, the `spec.connectorsConfig` TOML, for a credential: the decoded table whole,
/// the words of each decoded key and string, then the words of the raw text.
fn connectors_credential(text: &str) -> Result<(), Refusal> {
    let table = connectors_table(text)?;
    credential(
        &yaml(&table, "spec.connectorsConfig")?,
        "spec.connectorsConfig",
    )?;
    let mut decoded = Vec::new();
    decoded_words(&table, "spec.connectorsConfig", &mut decoded);
    word_credential(&decoded, "spec.connectorsConfig")?;
    word_credential(
        &raw_words(text, "spec.connectorsConfig"),
        "spec.connectorsConfig",
    )
}

/// The longest `spec.configFiles` name, in bytes: Kubernetes's bound on a ConfigMap key.
pub const MAX_CONFIG_FILE_NAME: usize = 253;

/// The most bytes a ConfigMap's keys and values may hold together: Kubernetes's limit, beyond
/// which the API server refuses the ConfigMap at apply.
pub const MAX_CONFIG_MAP_BYTES: usize = 1024 * 1024;

/// Whether `name` is a plain file name: `[A-Za-z0-9._-]+`, not `.`, not starting with `..`, at most
/// [`MAX_CONFIG_FILE_NAME`] bytes, so it is a ConfigMap key and a file directly under `/config`.
fn plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_CONFIG_FILE_NAME
        && name != "."
        && !name.starts_with("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Refuses `spec.configFiles` on an `AgentServe` resource (`Malformed`), a name that is not a
/// plain file name (`ConfigFileInvalid`), one an earlier entry gives or one the ConfigMap holds
/// already (`ConfigFileConflict`), never repeating the name.
fn config_file_names(
    files: &[ConfigFile],
    variant: AgentVariant,
    has_agent: bool,
) -> Result<(), Refusal> {
    if variant == AgentVariant::AgentServe {
        return Err(Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.configFiles",
            "`b10x-loom agent serve` reads no configuration file; only a Command resource may \
             give them",
        ));
    }
    let mut seen: HashMap<&str, usize> = HashMap::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let at = format!("spec.configFiles[{index}].name");
        if !plain_file_name(&file.name) {
            return Err(Refusal::new(
                "ConfigFileInvalid",
                at,
                format!(
                    "it is not a plain file name, [A-Za-z0-9._-]+ other than `.` and not starting with `..`, of at \
                     most {MAX_CONFIG_FILE_NAME} bytes"
                ),
            ));
        }
        let held = match file.name.as_str() {
            "routes.yaml" | "connectors.toml" => true,
            "agent.yaml" => has_agent,
            _ => false,
        };
        if held {
            return Err(Refusal::new(
                "ConfigFileConflict",
                at,
                "the controller writes a file of that name under /config itself",
            ));
        }
        if let Some(first) = seen.insert(file.name.as_str(), index) {
            return Err(Refusal::new(
                "ConfigFileConflict",
                at,
                format!("it repeats spec.configFiles[{first}].name"),
            ));
        }
    }
    Ok(())
}

/// The words of `strings`, the strings the stream walk read (each key, value and tag of every
/// document it walked, aliases replayed), each at `at`.
fn stream_words(strings: &[String], at: &str) -> Vec<Word> {
    strings
        .iter()
        .flat_map(|text| words(text))
        .map(|text| Word {
            text,
            member: at.to_owned(),
            line: None,
        })
        .collect()
}

/// Walks `spec.configFiles` for a credential: the list whole, names and contents; then each
/// content as a YAML stream (every document, and after a scan error each piece between `---`
/// lines; each string it reads whole and word by word; `TooLarge` when the walk cannot read it
/// whole and what it read holds none) and as a TOML table when it reads as one, whole and word by
/// word; then every word of each raw content, named by its line. Nothing decoded is kept: the
/// walk only reads what the program may read. A refusal stands at the content, never at a member
/// inside it, so it spells no key of the file; beyond the walk's bound it is `TooLarge`.
fn config_files_credential(files: &[ConfigFile]) -> Result<(), Refusal> {
    credential(&yaml(&files, "spec.configFiles")?, "spec.configFiles")?;
    for (index, file) in files.iter().enumerate() {
        let at = format!("spec.configFiles[{index}].content");
        let inside = |refusal: Refusal| {
            if refusal.reason == spelling(RefusalReason::SecretLiteral) {
                Refusal {
                    member: at.clone(),
                    ..refusal
                }
            } else {
                refusal
            }
        };
        let (strings, whole) = match yaml_stream_credential(&file.content) {
            StreamWalk::Found { shape, .. } => {
                return Err(Refusal::new(
                    spelling(RefusalReason::SecretLiteral),
                    at,
                    format!("{shape}, read as YAML"),
                ));
            }
            StreamWalk::TooLarge { strings } => (strings, false),
            StreamWalk::Clean { strings } => (strings, true),
        };
        word_credential(&stream_words(&strings, &at), &at).map_err(|refusal| {
            if refusal.reason == spelling(RefusalReason::SecretLiteral) {
                Refusal {
                    detail: format!("{}, read as YAML", refusal.detail),
                    ..refusal
                }
            } else {
                refusal
            }
        })?;
        if !whole {
            return Err(Refusal::new(
                "TooLarge",
                at,
                "read as YAML, it costs more than the credential walk reads (its size, `[` or \
                 `{` bytes, aliases or nesting)",
            ));
        }
        if let Ok(table) = toml::from_str::<toml::Table>(&file.content) {
            credential(&yaml(&table, &at)?, &at).map_err(inside)?;
            let mut decoded = Vec::new();
            decoded_words(&table, &at, &mut decoded);
            for word in &mut decoded {
                word.member.clone_from(&at);
            }
            word_credential(&decoded, &at).map_err(inside)?;
        }
        word_credential(&raw_words(&file.content, &at), &at)?;
    }
    Ok(())
}

/// `text`, the `spec.connectorsConfig` TOML, decoded.
fn connectors_table(text: &str) -> Result<toml::Table, Refusal> {
    toml::from_str(text).map_err(|error| {
        let detail = error.span().map_or_else(
            || "it is not a TOML document".to_owned(),
            |span| {
                let line = text
                    .as_bytes()
                    .iter()
                    .take(span.start)
                    .filter(|byte| **byte == b'\n')
                    .count()
                    + 1;
                format!("it is not a TOML document (line {line})")
            },
        );
        Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.connectorsConfig",
            detail,
        )
    })
}

/// `value`, a member of the resource at `at`, as YAML.
///
/// The value goes through its JSON text: a `serde_json::Value` number does not serialize as a
/// number to another format when `serde_json`'s `arbitrary_precision` is on, as it is in this
/// workspace, and JSON text reads as YAML.
pub fn yaml(value: &impl serde::Serialize, at: &str) -> Result<String, Refusal> {
    let refused = || {
        Refusal::new(
            spelling(RefusalReason::Malformed),
            at,
            "cannot be written as YAML",
        )
    };
    let json = serde_json::to_string(value).map_err(|_| refused())?;
    let value: Value = serde_yaml_ng::from_str(&json).map_err(|_| refused())?;
    serde_yaml_ng::to_string(&value).map_err(|_| refused())
}

/// Whether `routes` asks for the controller's model route: an empty object, or nothing.
fn empty(routes: &serde_json::Value) -> bool {
    match routes {
        serde_json::Value::Null => true,
        serde_json::Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// The routes document a resource with an empty `spec.routes` is given: `llm.routes/1`, one
/// anonymous endpoint at the model route, and one route per model a role of `spec` names, to the
/// model route's model; without a spec, no route.
#[must_use]
pub fn default_routes(route: &ModelRoute, spec: Option<&AgentSpec>) -> String {
    let mut auth = Mapping::new();
    auth.insert("kind".into(), "anonymous".into());
    let mut endpoint = Mapping::new();
    endpoint.insert("base_url".into(), route.url.clone().into());
    endpoint.insert("protocol".into(), route.protocol.clone().into());
    endpoint.insert("auth".into(), Value::Mapping(auth));
    let mut endpoints = Mapping::new();
    endpoints.insert(MODEL_ROUTE_ENDPOINT.into(), Value::Mapping(endpoint));
    let mut routes = Mapping::new();
    for role in spec.iter().flat_map(|spec| &spec.model.roles) {
        routes.insert(
            role.model.clone().into(),
            format!("{MODEL_ROUTE_ENDPOINT}/{}", route.model).into(),
        );
    }
    let mut document = Mapping::new();
    document.insert("format".into(), "llm.routes/1".into());
    document.insert("endpoints".into(), Value::Mapping(endpoints));
    document.insert("routes".into(), Value::Mapping(routes));
    serde_yaml_ng::to_string(&Value::Mapping(document)).unwrap_or_default()
}

/// The cluster service a boundary upstream names: the namespace its NetworkPolicy rule selects
/// and the TCP port it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterService<'a> {
    /// The namespace, the host's second label.
    pub namespace: &'a str,
    /// The port the URL names, 80 when it names none.
    pub port: u16,
}

/// Whether `label` is a DNS-1123 label: 1 to 63 bytes of `[a-z0-9-]`, starting and ending with
/// a letter or digit.
fn dns_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    (1..=63).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
}

/// Whether `segment` is a plain path segment: `[A-Za-z0-9._~-]`, not empty, `.` or `..`.
fn plain_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'-'))
}

/// The cluster service `url`, a boundary upstream, names. It is a plain `http://` URL without
/// user information, query or fragment; its host is `<service>.<namespace>.svc` or
/// `<service>.<namespace>.svc.cluster.local`, each a DNS-1123 label; its port, when it names
/// one, is 1 to 65535; its path, when it has one, is plain segments.
///
/// # Errors
/// What is wrong with `url`, never repeating it.
pub fn cluster_service(url: &str) -> Result<ClusterService<'_>, &'static str> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(if url.starts_with("https://") {
            "an https upstream is not admitted; the boundary relays plain http to a cluster service"
        } else {
            "it is not a plain http:// URL"
        });
    };
    if rest.contains(['?', '#']) {
        return Err("it holds a query or a fragment");
    }
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    if authority.contains('@') {
        return Err("it holds user information");
    }
    let (host, port) = match authority.rsplit_once(':') {
        None => (authority, 80),
        Some((host, port)) => {
            if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("its port is not a number");
            }
            match port.parse::<u16>() {
                Ok(port) if port > 0 => (host, port),
                _ => return Err("its port is not between 1 and 65535"),
            }
        }
    };
    let labels: Vec<&str> = host.split('.').collect();
    let namespace = match labels.as_slice() {
        [service, namespace, "svc"] | [service, namespace, "svc", "cluster", "local"]
            if dns_label(service) && dns_label(namespace) =>
        {
            *namespace
        }
        _ => {
            return Err(
                "its host is not a cluster service, <service>.<namespace>.svc with an optional \
                 .cluster.local, each a DNS-1123 label",
            );
        }
    };
    if path.is_some_and(|path| !path.split('/').all(plain_segment)) {
        return Err("its path is not plain segments of [A-Za-z0-9._~-], none empty, `.` or `..`");
    }
    Ok(ClusterService { namespace, port })
}

/// Whether `id` is a Connectors operation id the boundary may pass as `--operation`:
/// `[A-Za-z0-9._-]+`, not starting with `-`, which would read as a flag. Admission also takes
/// `b10x_loom_boundary::operation_id`, which bounds it to 256 bytes.
fn operation_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Whether `entry` is a repository the boundary may pass as `--repository`: `<owner>/<repo>`,
/// each part `[A-Za-z0-9._-]+` and not `.` or `..`, not starting with `-`, the repository not
/// ending in `.git`.
fn repository(entry: &str) -> bool {
    let part = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    };
    !entry.starts_with('-')
        && entry.split_once('/').is_some_and(|(owner, repo)| {
            part(owner) && part(repo) && !repo.to_ascii_lowercase().ends_with(".git")
        })
}

/// Refuses `items`, the list at `at`, when it is empty or an entry is not `plain` (`Malformed`,
/// naming `form`) or repeats an earlier one (`Malformed`), never repeating an entry.
fn boundary_list(
    items: &[String],
    at: &str,
    plain: fn(&str) -> bool,
    form: &str,
) -> Result<(), Refusal> {
    if items.is_empty() {
        return Err(Refusal::new(
            spelling(RefusalReason::Malformed),
            at,
            "it is empty, so the boundary would admit nothing on this upstream",
        ));
    }
    let mut seen: HashMap<&str, usize> = HashMap::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let member = format!("{at}[{index}]");
        if !plain(item) {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                member,
                format!("it is not {form}"),
            ));
        }
        if let Some(first) = seen.insert(item.as_str(), index) {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                member,
                format!("it repeats {at}[{first}]"),
            ));
        }
    }
    Ok(())
}

/// Walks `boundary`, whose upstreams are `declared`, for a credential as `spec.command` is: the
/// member whole, then each URL, audience, operation id and repository word by word.
fn boundary_credential(boundary: &Boundary, declared: &[Upstream<'_>]) -> Result<(), Refusal> {
    credential(&yaml(boundary, "spec.boundary")?, "spec.boundary")?;
    let mut found = Vec::new();
    for upstream in declared {
        let at = format!("spec.boundary.{}", upstream.relay.kind);
        let mut strings = vec![
            (upstream.url, format!("{at}.upstream")),
            (upstream.audience, format!("{at}.audience")),
        ];
        let (list, items) = match upstream.admits {
            Admits::Everything => ("", &[][..]),
            Admits::Operations(items) => ("operations", items),
            Admits::Repositories(items) => ("repositories", items),
        };
        strings.extend(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| (item.as_str(), format!("{at}.{list}[{index}]"))),
        );
        for (text, member) in strings {
            found.extend(words(text).into_iter().map(|text| Word {
                text,
                member: member.clone(),
                line: None,
            }));
        }
    }
    word_credential(&found, "spec.boundary")
}

/// Step 5: a present `spec.boundary`. It declares an upstream (`Malformed`); no `spec.secretRef`
/// or `spec.secretFiles` stands beside it (`BoundaryWithSecret`); each upstream is a cluster
/// service ([`cluster_service`]) the relay parses (`BoundaryUpstreamInvalid`), which an
/// `allowedBoundaryUpstreams` entry of its kind lists exactly (`BoundaryUpstreamNotAllowed` at
/// `upstream`), its audience one word (`Malformed`), its operations or repositories in form for
/// the operator and the relay, each once (`Malformed`), and the relay `b10x-loom-boundary`
/// builds from it accepted ([`relay`]); its strings hold no credential (`SecretLiteral`); each
/// audience is exactly that of an entry of its kind and URL (`BoundaryUpstreamNotAllowed` at
/// `audience`); and no `deploy.kubernetes.egress` entry of `spec.agent` names a host
/// (`BoundaryEgressHost`) or a namespace (`BoundaryEgressNamespace`).
fn boundary(
    spec: &LoomAgentSpec,
    decoded: Option<&AgentSpec>,
    config: &Config,
) -> Result<(), Refusal> {
    let Some(boundary) = &spec.boundary else {
        return Ok(());
    };
    let declared = upstreams(boundary);
    if declared.is_empty() {
        return Err(Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.boundary",
            "it declares none of model, connectors and git, so it relays nothing",
        ));
    }
    for (present, member) in [
        (spec.secret_ref.is_some(), "spec.secretRef"),
        (spec.secret_files.is_some(), "spec.secretFiles"),
    ] {
        if present {
            return Err(Refusal::new(
                "BoundaryWithSecret",
                member,
                "a resource with spec.boundary holds no credential in its pod; its boundary's \
                 projected tokens stand in for a Secret",
            ));
        }
    }
    for upstream in &declared {
        let kind = upstream.relay.kind;
        let at = format!("spec.boundary.{kind}");
        let invalid = |detail: String| {
            Refusal::new("BoundaryUpstreamInvalid", format!("{at}.upstream"), detail)
        };
        cluster_service(upstream.url).map_err(|detail| invalid(detail.to_owned()))?;
        let url = BoundaryUrl::parse(upstream.url).map_err(|refused| {
            invalid(format!(
                "`b10x-loom agent boundary` refuses it: it {refused}"
            ))
        })?;
        if !config.allows_upstream_url(kind, upstream.url) {
            return Err(Refusal::new(
                "BoundaryUpstreamNotAllowed",
                format!("{at}.upstream"),
                "the controller's allowedBoundaryUpstreams does not hold it for this relay",
            ));
        }
        if upstream.audience.is_empty()
            || upstream
                .audience
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                format!("{at}.audience"),
                "a token audience is one word: not empty, without whitespace or a control \
                 character",
            ));
        }
        match upstream.admits {
            Admits::Everything => {}
            Admits::Operations(items) => boundary_list(
                items,
                &format!("{at}.operations"),
                |id| operation_id(id) && b10x_loom_boundary::operation_id(id),
                "a Connectors operation id, 1 to 256 bytes of [A-Za-z0-9._-] not starting with -",
            )?,
            Admits::Repositories(items) => boundary_list(
                items,
                &format!("{at}.repositories"),
                |entry| repository(entry) && b10x_loom_boundary::repository_name(entry),
                "<owner>/<repo>, each part [A-Za-z0-9._-]+ and not . or .., not starting with -, \
                 the repository not ending in .git",
            )?,
        }
        relay(upstream, url, &at)?;
    }
    boundary_credential(boundary, &declared)?;
    for upstream in &declared {
        if config
            .allowed_upstream(upstream.relay.kind, upstream.url, upstream.audience)
            .is_none()
        {
            return Err(Refusal::new(
                "BoundaryUpstreamNotAllowed",
                format!("spec.boundary.{}.audience", upstream.relay.kind),
                "the controller's allowedBoundaryUpstreams does not hold this upstream with this \
                 audience",
            ));
        }
    }
    let egress = decoded
        .and_then(|decoded| decoded.deploy.as_ref())
        .and_then(|deploy| deploy.kubernetes.as_ref())
        .map(|kubernetes| kubernetes.egress.as_slice())
        .unwrap_or_default();
    for (index, entry) in egress.iter().enumerate() {
        let at = format!("spec.agent.deploy.kubernetes.egress[{index}]");
        if entry.host.is_some() {
            return Err(Refusal::new(
                "BoundaryEgressHost",
                format!("{at}.host"),
                "a resource with spec.boundary reaches the outside only through its boundary's \
                 upstreams, so its NetworkPolicy opens no host",
            ));
        }
        if entry.namespace.is_some() {
            return Err(Refusal::new(
                "BoundaryEgressNamespace",
                format!("{at}.namespace"),
                "a resource with spec.boundary reaches only DNS and its boundary's upstreams, so \
                 its NetworkPolicy opens no namespace",
            ));
        }
    }
    Ok(())
}

/// Builds the relay `upstream` declares (at `at`, its URL already parsed as `url`) with
/// `b10x-loom-boundary`, from the values the rendered `--<kind>-upstream`, `--operation` and
/// `--repository` flags carry, as `b10x-loom agent boundary` builds it from them, so a boundary
/// admission admits is one the relay accepts. The per-entry checks before it name the entry;
/// this refuses whatever else the relay refuses (`Malformed` at the list).
fn relay(upstream: &Upstream<'_>, url: BoundaryUrl, at: &str) -> Result<(), Refusal> {
    let token = format!("{BOUNDARY_TOKENS_DIR}/{}", upstream.relay.kind);
    let built = match upstream.admits {
        Admits::Everything => Ok(BoundaryRelay::model(url, token)),
        Admits::Operations(items) => BoundaryRelay::connectors(url, token, items.iter().cloned())
            .map_err(|refused| ("operations", refused)),
        Admits::Repositories(items) => BoundaryRelay::git(url, token, items.iter().cloned())
            .map_err(|refused| ("repositories", refused)),
    };
    built.map(|_| ()).map_err(|(list, refused)| {
        Refusal::new(
            spelling(RefusalReason::Malformed),
            format!("{at}.{list}"),
            format!("`b10x-loom agent boundary` refuses it: {refused}"),
        )
    })
}

/// Admits `spec`, a `LoomAgent` in `namespace`, under `config`, checking `spec.agent` against
/// `actions` (the action ids of a catalog protocol by its `name@major`).
///
/// # Errors
/// The first refusal, in the order the module documentation gives.
pub fn admit(
    spec: &LoomAgentSpec,
    namespace: &str,
    config: &Config,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<Admitted, Refusal> {
    let variant = spec.variant();
    let agent = match (&spec.agent, variant) {
        (Some(agent), _) => Some(yaml(agent, "spec.agent")?),
        (None, AgentVariant::Command) => None,
        (None, AgentVariant::AgentServe) => {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.agent",
                "`b10x-loom agent serve` needs a loom.agent/1 document; only a Command resource \
                 may leave it out",
            ));
        }
    };
    let checked = match (variant, &agent) {
        (AgentVariant::AgentServe, Some(agent)) => {
            Some(check_text(agent, actions).map_err(|refused| {
                Refusal::new(
                    spelling(refused.reason),
                    within("spec.agent", &refused.member),
                    refused.detail,
                )
            })?)
        }
        (_, Some(agent)) => {
            credential(agent, "spec.agent").map_err(|refusal| {
                if refusal.reason == "TooLarge" {
                    Refusal::new(
                        spelling(RefusalReason::Malformed),
                        "spec.agent",
                        "it is beyond the loom.agent/1 decoder's bound",
                    )
                } else {
                    refusal
                }
            })?;
            None
        }
        (_, None) => None,
    };
    let decoded = agent
        .as_deref()
        .map(|agent| {
            parse(agent).map_err(|error| {
                Refusal::new(
                    spelling(RefusalReason::Malformed),
                    "spec.agent",
                    error.to_string(),
                )
            })
        })
        .transpose()?;
    if variant == AgentVariant::AgentServe
        && decoded
            .as_ref()
            .and_then(|decoded| decoded.runtime.as_ref())
            .is_some_and(|runtime| runtime.connectors.is_none())
    {
        return Err(Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.agent.runtime.connectors",
            "`b10x-loom agent serve` needs a connectors command line; only a Command resource \
             may leave it out",
        ));
    }

    let given = (!empty(&spec.routes))
        .then(|| yaml(&spec.routes, "spec.routes"))
        .transpose()?;
    if let Some(text) = &given {
        credential(text, "spec.routes")?;
        if !spec.routes.is_object() {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.routes",
                "an llm.routes/1 document is a mapping",
            ));
        }
        Routes::parse(text).map_err(|error| {
            Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.routes",
                error.message,
            )
        })?;
    }

    let connectors = spec.connectors_config.clone().unwrap_or_default();
    connectors_credential(&connectors)?;

    let boundary_without_model = spec
        .boundary
        .as_ref()
        .is_some_and(|boundary| boundary.model.is_none() && !upstreams(boundary).is_empty());
    let (routes, default_routes) = match (given, &config.model_route) {
        (Some(text), _) => (text, false),
        (None, _) if boundary_without_model => {
            if variant == AgentVariant::AgentServe {
                return Err(Refusal::new(
                    "NoModelRoute",
                    "spec.boundary.model",
                    "spec.routes is empty and the boundary declares no model relay, the only \
                     model a credential-free pod reaches",
                ));
            }
            (String::new(), false)
        }
        (None, Some(route)) => {
            let route = match spec
                .boundary
                .as_ref()
                .and_then(|boundary| boundary.model.as_ref())
            {
                Some(_) => ModelRoute {
                    url: RELAYS[0].url.to_owned(),
                    ..route.clone()
                },
                None => route.clone(),
            };
            (default_routes(&route, decoded.as_ref()), true)
        }
        (None, None) if variant == AgentVariant::Command => (String::new(), false),
        (None, None) => {
            return Err(Refusal::new(
                "NoModelRoute",
                "spec.routes",
                "it is empty and the controller configures no model route",
            ));
        }
    };

    boundary(spec, decoded.as_ref(), config)?;

    if let Some(secret) = &spec.secret_ref
        && !config.allowed_secret_names.contains(&secret.name)
    {
        return Err(Refusal::new(
            "SecretNotAllowed",
            "spec.secretRef.name",
            "the controller's allowedSecretNames does not hold it",
        ));
    }
    if let Some(files) = &spec.secret_files
        && !config.allowed_secret_names.contains(&files.name)
    {
        return Err(Refusal::new(
            "SecretNotAllowed",
            "spec.secretFiles.name",
            "the controller's allowedSecretNames does not hold it",
        ));
    }

    if let Some(kubernetes) = decoded
        .as_ref()
        .and_then(|decoded| decoded.deploy.as_ref())
        .and_then(|deploy| deploy.kubernetes.as_ref())
    {
        if kubernetes.namespace != namespace {
            return Err(Refusal::new(
                "NamespaceMismatch",
                "spec.agent.deploy.kubernetes.namespace",
                "it names another namespace than the resource's",
            ));
        }
        for (index, egress) in kubernetes.egress.iter().enumerate() {
            if egress.host.is_none() && egress.namespace.is_none() {
                return Err(Refusal::new(
                    "EgressHostNotAllowed",
                    format!("spec.agent.deploy.kubernetes.egress[{index}]"),
                    "it names neither a host nor a namespace, so its rule would admit every \
                     destination on its port",
                ));
            }
            if let Some(host) = &egress.host
                && !config.allowed_egress_hosts.contains(host)
            {
                return Err(Refusal::new(
                    "EgressHostNotAllowed",
                    format!("spec.agent.deploy.kubernetes.egress[{index}].host"),
                    "the controller's allowedEgressHosts does not hold it",
                ));
            }
        }
    }

    if let Some(command) = &spec.command {
        command_credential(command)?;
    }

    if let Some(env) = &spec.env {
        env_credential(env)?;
        env_names(env)?;
    }
    expanded_credential(spec, variant)?;

    let config_files = spec.config_files.clone().unwrap_or_default();
    if !config_files.is_empty() {
        config_file_names(&config_files, variant, agent.is_some())?;
    }
    let bytes: usize = config_map_data(&connectors, &routes, agent.as_deref(), &config_files)
        .iter()
        .map(|(key, value)| key.len().saturating_add(value.len()))
        .fold(0, usize::saturating_add);
    if bytes > MAX_CONFIG_MAP_BYTES {
        return Err(Refusal::new(
            "TooLarge",
            if config_files.is_empty() {
                "spec"
            } else {
                "spec.configFiles"
            },
            format!(
                "the ConfigMap it makes holds more than {MAX_CONFIG_MAP_BYTES} bytes, more than \
                 Kubernetes accepts"
            ),
        ));
    }
    config_files_credential(&config_files)?;

    mount_paths(spec)?;

    if let Some(claim) = spec
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.claim_name.as_ref())
        && !config.allowed_claim_names.contains(claim)
    {
        return Err(Refusal::new(
            "ClaimNotAllowed",
            "spec.workspace.claimName",
            "the controller's allowedClaimNames does not hold it",
        ));
    }

    confinement_profiles(spec, variant, config)?;

    let resources = pod_resources(decoded.as_ref(), variant, config)?;

    Ok(Admitted {
        spec: decoded,
        agent,
        routes,
        connectors,
        default_routes,
        variant,
        checked,
        config_files,
        resources,
    })
}

/// Step 17: the resources the spec declares, else the configuration's default for `variant`;
/// `ResourcesRequired` for a `Command` resource when neither gives one.
fn pod_resources(
    spec: Option<&AgentSpec>,
    variant: AgentVariant,
    config: &Config,
) -> Result<Resources, Refusal> {
    if let Some(declared) = spec
        .and_then(|spec| spec.deploy.as_ref())
        .and_then(|deploy| deploy.kubernetes.as_ref())
        .map(|kubernetes| &kubernetes.resources)
    {
        return Ok(Resources {
            cpu: declared.cpu.clone(),
            memory: declared.memory.clone(),
        });
    }
    match variant {
        AgentVariant::AgentServe => Ok(config.default_resources.clone()),
        AgentVariant::Command => config.default_command_resources.clone().ok_or_else(|| {
            Refusal::new(
                "ResourcesRequired",
                "spec.agent.deploy.kubernetes.resources",
                "Loom cannot size a program it does not know: declare its cpu and memory in \
                 spec.agent.deploy.kubernetes.resources, or set defaultCommandResources in the \
                 controller's configuration",
            )
        }),
    }
}

/// The longest `localhostProfile` the controller admits, in bytes.
pub const MAX_PROFILE_NAME: usize = 253;

/// Whether `name` is a plain profile name or path: `[A-Za-z0-9._/-]`, no leading `/`, no empty
/// or `..` segment, at most [`MAX_PROFILE_NAME`] bytes.
fn plain_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_PROFILE_NAME
        && !name.starts_with('/')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'))
        && name
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "..")
}

/// Step 16: the profiles of `spec.seccompProfile` and `spec.appArmorProfile`.
fn confinement_profiles(
    spec: &LoomAgentSpec,
    variant: AgentVariant,
    config: &Config,
) -> Result<(), Refusal> {
    let declared = [
        (
            "spec.seccompProfile",
            &spec.seccomp_profile,
            &config.allowed_seccomp_localhost_profiles,
            "allowedSeccompLocalhostProfiles",
        ),
        (
            "spec.appArmorProfile",
            &spec.app_armor_profile,
            &config.allowed_app_armor_localhost_profiles,
            "allowedAppArmorLocalhostProfiles",
        ),
    ];
    for (member, profile, allowed, list) in declared {
        let Some(profile) = profile else { continue };
        if variant != AgentVariant::Command {
            return Err(Refusal::new(
                "Malformed",
                member,
                "only a Command resource declares a profile; an AgentServe pod keeps \
                 RuntimeDefault",
            ));
        }
        match (profile.kind.as_str(), &profile.localhost_profile) {
            ("RuntimeDefault", None) => {}
            ("Unconfined", None) => {
                if !config.allow_unconfined_profiles {
                    return Err(Refusal::new(
                        "ProfileNotAllowed",
                        format!("{member}.type"),
                        "the controller's allowUnconfinedProfiles does not allow Unconfined",
                    ));
                }
            }
            ("Localhost", Some(name)) => {
                if !plain_profile_name(name) {
                    return Err(Refusal::new(
                        "ProfileInvalid",
                        format!("{member}.localhostProfile"),
                        format!(
                            "a profile name is [A-Za-z0-9._/-], not absolute, with no empty or \
                             `..` segment, at most {MAX_PROFILE_NAME} bytes"
                        ),
                    ));
                }
                if !allowed.contains(name) {
                    return Err(Refusal::new(
                        "ProfileNotAllowed",
                        format!("{member}.localhostProfile"),
                        format!("the controller's {list} does not hold it"),
                    ));
                }
            }
            ("RuntimeDefault" | "Unconfined", Some(_)) => {
                return Err(Refusal::new(
                    "ProfileInvalid",
                    format!("{member}.localhostProfile"),
                    "only a Localhost profile names one",
                ));
            }
            ("Localhost", None) => {
                return Err(Refusal::new(
                    "ProfileInvalid",
                    format!("{member}.localhostProfile"),
                    "a Localhost profile needs the name of the profile",
                ));
            }
            _ => {
                return Err(Refusal::new(
                    "ProfileInvalid",
                    format!("{member}.type"),
                    "a profile is RuntimeDefault, Localhost or Unconfined",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        let mut config = Config::builtin();
        config.namespace = "agents".to_owned();
        config.default_command_resources = Some(config.default_resources.clone());
        config
    }

    fn spec(connectors: Option<&str>) -> LoomAgentSpec {
        LoomAgentSpec {
            agent: Some(serde_json::json!({
                "format": "loom.agent/1",
                "agent": {"id": "a", "name": "A", "brief": "answers"},
                "model": {"routes": "routes.yaml", "roles": [{"role": "agent", "model": "small"}]},
            })),
            routes: serde_json::json!({
                "format": "llm.routes/1",
                "endpoints": {"lab": {"base_url": "http://lab.example/v1", "protocol": "messages",
                                      "auth": {"kind": "anonymous"}}},
                "routes": {"small": "lab/small"},
            }),
            connectors_config: connectors.map(str::to_owned),
            secret_ref: None,
            secret_files: None,
            env: None,
            image: None,
            command: None,
            workspace: None,
            config_files: None,
            seccomp_profile: None,
            app_armor_profile: None,
            boundary: None,
        }
    }

    fn admitted(spec: &LoomAgentSpec) -> Result<Admitted, Refusal> {
        admit(spec, "agents", &config(), &|_| None)
    }

    #[test]
    fn a_spec_without_loops_is_admitted_with_the_checks_line() {
        let admitted = admitted(&spec(None)).expect("admitted");
        assert_eq!(
            admitted.message(),
            "checked a: 0 states, 0 bindings, protocol none"
        );
        assert!(!admitted.default_routes);
        assert_eq!(admitted.connectors, "");
    }

    #[test]
    fn a_runtime_without_connectors_is_refused_for_serve_and_admitted_for_a_command() {
        let mut serve = spec(None);
        serve.agent.as_mut().expect("an agent")["runtime"] =
            serde_json::json!({"state_dir": "/var/lib/agent", "confinement": "none"});
        let refusal = admitted(&serve).expect_err("agent serve needs connectors");
        assert_eq!(refusal.reason, "Malformed");
        assert_eq!(refusal.member, "spec.agent.runtime.connectors");

        let command = LoomAgentSpec {
            command: Some(CommandSpec {
                entrypoint: Some(vec!["takt".to_owned()]),
                args: None,
            }),
            ..serve
        };
        admitted(&command).expect("a Command resource need not name connectors");
    }

    #[test]
    fn a_token_in_the_connectors_toml_is_refused_without_repeating_it() {
        let refusal = admitted(&spec(Some(
            "[connections.ops]\nadapter = \"chat\"\ntoken = \"xoxb-000-fake\"\n",
        )))
        .expect_err("refused");
        assert_eq!(refusal.reason, "SecretLiteral");
        assert_eq!(
            refusal.member,
            "spec.connectorsConfig.connections.ops.token"
        );
        assert!(!refusal.message().contains("xoxb-000-fake"));
        admitted(&spec(Some(
            "[connections.ops]\nadapter = \"chat\"\ntoken = \"env:CHAT_TOKEN\"\n",
        )))
        .expect("a reference is admitted");
    }

    #[test]
    fn a_token_spelt_with_toml_escapes_or_as_a_key_is_refused_as_decoded() {
        for (toml, member) in [
            (
                "[connections.ops]\ntoken = \"\\u0078oxb-000-fake\"\n",
                "spec.connectorsConfig.connections.ops.token",
            ),
            (
                "[connections.ops]\ntoken = \"\"\"\n\\U00000078oxb-000-fake\"\"\"\n",
                "spec.connectorsConfig.connections.ops.token",
            ),
            (
                "[connections]\n\"\\u0078oxb-000-fake\" = 1\n",
                "spec.connectorsConfig.connections",
            ),
        ] {
            let refusal = admitted(&spec(Some(toml))).expect_err(toml);
            assert_eq!(refusal.reason, "SecretLiteral", "{toml}");
            assert_eq!(refusal.member, member, "{toml}");
            assert!(!refusal.message().contains("oxb-000-fake"), "{toml}");
        }
    }

    #[test]
    fn a_connectors_config_that_is_not_toml_is_malformed_without_repeating_it() {
        let refusal = admitted(&spec(Some("[connections.ops]\ntoken = xoxb-000-fake\n")))
            .expect_err("refused");
        assert_eq!(
            refusal.message(),
            "refused: Malformed at spec.connectorsConfig: it is not a TOML document (line 2)"
        );
    }

    /// The ConfigMap receives the raw text: a token in a comment, after another word of a
    /// string, after `Bearer `, or spelt with an escape after another word, is found all the
    /// same, named by its member or its line.
    #[test]
    fn a_token_in_a_comment_or_after_a_word_of_the_connectors_toml_is_refused_without_repeating_it()
    {
        let ops = "[connections.ops]\nadapter = \"chat\"\n";
        for (toml, message) in [
            (
                format!("{ops}# token = \"xoxb-000-fake\"\ntoken = \"env:CHAT_TOKEN\"\n"),
                "refused: SecretLiteral at spec.connectorsConfig: a value with the prefix \
                 `xoxb-` (line 3)",
            ),
            (
                format!("{ops}token = \"env:CHAT_TOKEN\" #xoxb-000-fake\n"),
                "refused: SecretLiteral at spec.connectorsConfig: a value with the prefix \
                 `xoxb-` (line 3)",
            ),
            (
                format!("{ops}# authorization = \"Bearer 000fake\"\n"),
                "refused: SecretLiteral at spec.connectorsConfig: a value with the prefix \
                 `Bearer ` (line 3)",
            ),
            (
                format!("{ops}headers = {{ authorization = \"token ghp_000fake\" }}\n"),
                "refused: SecretLiteral at spec.connectorsConfig.connections.ops.headers.\
                 authorization: a value with the prefix `ghp_`",
            ),
            (
                format!("{ops}headers = {{ authorization = \"token \\u0067hp_000fake\" }}\n"),
                "refused: SecretLiteral at spec.connectorsConfig.connections.ops.headers.\
                 authorization: a value with the prefix `ghp_`",
            ),
            (
                format!("{ops}notes = [\"first\", \"then\\tBearer 000fake\"]\n"),
                "refused: SecretLiteral at spec.connectorsConfig.connections.ops.notes[1]: a \
                 value with the prefix `Bearer `",
            ),
            (
                "[connections]\n\"token ghp_000fake\" = 1\n".to_owned(),
                "refused: SecretLiteral at spec.connectorsConfig.connections: a value with the \
                 prefix `ghp_`",
            ),
        ] {
            let refusal = admitted(&spec(Some(&toml))).expect_err(&toml);
            assert_eq!(refusal.message(), message, "{toml}");
            assert!(!refusal.message().contains("000fake"), "{toml}");
        }
        let annotated = format!(
            "# the chat connection, its token from the environment\n{ops}token = \
             \"env:CHAT_TOKEN\" # rotated monthly\nheaders = {{ accept = \"application/json\" }}\n"
        );
        admitted(&spec(Some(&annotated))).expect("comments and words without a token");
    }

    /// The words are walked in sequences within the walk's bounds, so a long text of short
    /// words is walked to its end and not refused `TooLarge`.
    #[test]
    fn the_words_of_a_long_connectors_toml_are_walked_to_the_last() {
        let lines = MAX_WALKED_BYTES / 4 / 12;
        let mut toml = String::from("[connections.ops]\n");
        for _ in 0..lines {
            toml.push_str("# a b c d e\n");
        }
        assert!(toml.len() < MAX_WALKED_BYTES);
        assert!(lines * 5 * cost("a") > MAX_WALKED_BYTES);
        admitted(&spec(Some(&toml))).expect("no token");
        toml.push_str("# xoxb-000-fake\n");
        let refusal = admitted(&spec(Some(&toml))).expect_err("refused");
        assert_eq!(
            refusal.message(),
            format!(
                "refused: SecretLiteral at spec.connectorsConfig: a value with the prefix \
                 `xoxb-` (line {})",
                lines + 2
            )
        );
    }

    #[test]
    fn an_egress_entry_naming_neither_host_nor_namespace_is_refused_naming_it() {
        let deployed = |egress: serde_json::Value| {
            let mut deployed = spec(None);
            deployed.agent.as_mut().expect("an agent")["deploy"] = serde_json::json!({"kubernetes": {
                "namespace": "agents", "service_account": "a",
                "image_repository": "registry.example/a",
                "resources": {"cpu": "100m", "memory": "128Mi"},
                "egress": egress,
            }});
            admitted(&deployed)
        };
        let logs = serde_json::json!({"name": "logs", "namespace": "monitoring", "port": 3100});
        deployed(serde_json::json!([logs])).expect("a namespace entry is admitted");
        for anywhere in [
            serde_json::json!({"name": "anywhere", "port": 6443}),
            serde_json::json!({"name": "anywhere", "selector": [{"name": "app", "value": "x"}],
                               "port": 6443}),
        ] {
            let refusal = deployed(serde_json::json!([logs, anywhere])).expect_err("refused");
            assert_eq!(
                refusal.message(),
                "refused: EgressHostNotAllowed at spec.agent.deploy.kubernetes.egress[1]: it \
                 names neither a host nor a namespace, so its rule would admit every \
                 destination on its port"
            );
        }
    }

    #[test]
    fn a_routes_secret_that_is_a_value_is_refused_by_llms_parser_without_repeating_it() {
        let mut literal = spec(None);
        literal.routes["endpoints"]["lab"]["auth"] =
            serde_json::json!({"kind": "bearer", "secret": "not-a-reference-000-fake"});
        let refusal = admitted(&literal).expect_err("refused");
        assert_eq!(refusal.reason, "Malformed");
        assert_eq!(refusal.member, "spec.routes");
        assert!(
            refusal.detail.contains("must be a reference"),
            "{refusal:?}"
        );
        assert!(!refusal.message().contains("not-a-reference-000-fake"));

        let mut reference = spec(None);
        reference.routes["endpoints"]["lab"]["auth"] =
            serde_json::json!({"kind": "bearer", "secret": "env:LAB_KEY"});
        admitted(&reference).expect("a reference is admitted");

        let mut unknown = spec(None);
        unknown.routes["note"] = serde_json::json!("anything");
        assert_eq!(admitted(&unknown).expect_err("refused").reason, "Malformed");
    }

    /// The walk's bound, held at its edge: a token beside exactly [`MAX_WALKED_OPENERS`] braces
    /// is still found, and one more brace refuses the text unwalked.
    #[test]
    fn a_routes_text_beyond_the_walks_bound_is_too_large_and_one_within_it_is_walked() {
        let at = |braces: usize| {
            let mut routes = spec(None);
            routes.routes["note"] = serde_json::json!("{".repeat(braces));
            routes.routes["endpoints"]["lab"]["auth"] =
                serde_json::json!({"kind": "bearer", "secret": "xoxb-000-fake"});
            admitted(&routes).expect_err("refused")
        };
        let within = at(MAX_WALKED_OPENERS);
        assert_eq!(within.reason, "SecretLiteral");
        assert_eq!(within.member, "spec.routes.endpoints.lab.auth.secret");
        let beyond = at(MAX_WALKED_OPENERS + 1);
        assert_eq!(
            beyond.message(),
            "refused: TooLarge at spec.routes: it holds more than 4096 `[` or `{` bytes, more \
             than the credential walk reads"
        );

        let long = |bytes: usize| {
            let mut connectors = String::from("token = \"xoxb-000-fake\"\npad = \"");
            let pad = bytes.saturating_sub(connectors.len() + 2);
            connectors.push_str(&"a".repeat(pad));
            connectors.push_str("\"\n");
            admitted(&spec(Some(&connectors))).expect_err("refused")
        };
        assert_eq!(long(MAX_WALKED_BYTES - 64).reason, "SecretLiteral");
        let beyond = long(MAX_WALKED_BYTES + 64);
        assert_eq!(beyond.reason, "TooLarge");
        assert_eq!(beyond.member, "spec.connectorsConfig");
        assert!(!beyond.message().contains("xoxb-000-fake"));
    }

    #[test]
    fn the_default_routes_read_as_llm_routes() {
        let route = ModelRoute {
            url: "http://model-route.agents.svc:8080/v1".to_owned(),
            protocol: "messages".to_owned(),
            model: "small-model".to_owned(),
        };
        let decoded = parse(
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, \
             roles: [{role: agent, model: small}]}\n",
        )
        .expect("a spec");
        Routes::parse(&default_routes(&route, Some(&decoded)))
            .expect("llm reads the default routes");
    }

    #[test]
    fn routes_that_are_not_a_mapping_are_malformed() {
        let mut list = spec(None);
        list.routes = serde_json::json!(["lab/small"]);
        let refusal = admitted(&list).expect_err("refused");
        assert_eq!(
            refusal.message(),
            "refused: Malformed at spec.routes: an llm.routes/1 document is a mapping"
        );
    }

    #[test]
    fn the_default_routes_name_each_roles_model_once() {
        let route = ModelRoute {
            url: "http://model-route.agents.svc:8080/v1".to_owned(),
            protocol: "messages".to_owned(),
            model: "small-model".to_owned(),
        };
        let decoded = parse(
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, \
             roles: [{role: agent, model: small}, {role: classifier, model: small}]}\n",
        )
        .expect("a spec");
        assert_eq!(
            default_routes(&route, Some(&decoded)),
            "format: llm.routes/1\nendpoints:\n  model-route:\n    base_url: \
             http://model-route.agents.svc:8080/v1\n    protocol: messages\n    auth:\n      \
             kind: anonymous\nroutes:\n  small: model-route/small-model\n"
        );
    }

    fn command(agent: bool, files: &[(&str, &str)]) -> LoomAgentSpec {
        let base = spec(None);
        LoomAgentSpec {
            agent: if agent { base.agent.clone() } else { None },
            routes: serde_json::json!({}),
            command: Some(CommandSpec {
                entrypoint: Some(vec!["takt".to_owned()]),
                args: None,
            }),
            config_files: Some(
                files
                    .iter()
                    .map(|(name, content)| ConfigFile {
                        name: (*name).to_owned(),
                        content: (*content).to_owned(),
                    })
                    .collect(),
            ),
            ..base
        }
    }

    #[test]
    fn a_command_resource_without_spec_agent_says_what_it_admitted() {
        let files = [("endpoint.toml", "model = \"small\"\n"), ("run.toml", "")];
        let both = admitted(&command(true, &files)).expect("admitted");
        assert_eq!(
            both.message(),
            "admitted a for its own command: a loom.agent/1 document with no credential, at \
             /config/agent.yaml; 2 configuration files with no credential, at /config"
        );
        let only = admitted(&command(false, &files[..1])).expect("admitted");
        assert_eq!(
            only.message(),
            "admitted for its own command: 1 configuration file with no credential, at /config"
        );
        assert!(only.spec.is_none() && only.agent.is_none());
        assert_eq!(
            only.config_map_data().keys().collect::<Vec<_>>(),
            ["connectors.toml", "endpoint.toml", "routes.yaml"]
        );
        let neither = admitted(&command(false, &[])).expect("admitted");
        assert_eq!(
            neither.message(),
            "admitted for its own command: no loom.agent/1 document and no configuration file"
        );
    }

    #[test]
    fn the_default_routes_of_a_command_resource_without_spec_agent_route_nothing() {
        let route = ModelRoute {
            url: "http://model-route.agents.svc:8080/v1".to_owned(),
            protocol: "messages".to_owned(),
            model: "small-model".to_owned(),
        };
        let mut with_route = config();
        with_route.model_route = Some(route.clone());
        let admitted =
            admit(&command(false, &[]), "agents", &with_route, &|_| None).expect("admitted");
        assert!(admitted.default_routes, "the model route stays open");
        assert_eq!(admitted.routes, default_routes(&route, None));
        assert!(
            admitted.routes.ends_with("routes: {}\n"),
            "{}",
            admitted.routes
        );
        Routes::parse(&admitted.routes).expect("llm reads it");
    }
}
