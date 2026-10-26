//! The checks a `LoomAgent` passes before any child is rendered, in this order; the first refusal
//! is the answer:
//!
//! 1. `spec.agent`, as YAML, through `b10x_loom_agent::check_text` against the plugin catalog:
//!    the check `b10x-loom agent check` runs, in-process (`Malformed`, `SecretLiteral`,
//!    `UnknownProtocol`, `UnknownAction`, `UnboundSource`, `HttpMethod`, `SendWithoutWrites`,
//!    `EmptyChannelAllow`), then a `runtime` without `connectors`, which `agent serve` needs and
//!    the check does not read (`Malformed`), as the decoder refused it before 0.42.0. For the
//!    `Command` variant (`spec.command` present), whose
//!    document the image's program reads and `agent serve` does not, only the credential walk
//!    of that library (`SecretLiteral`) and the `loom.agent/1` decoder (`Malformed`).
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
//!    a `Command` resource then gets an empty `routes.yaml`.
//! 5. A `spec.secretRef` or `spec.secretFiles` outside the configuration's `allowedSecretNames`
//!    (`SecretNotAllowed`).
//! 6. A `deploy.kubernetes.namespace` other than the resource's (`NamespaceMismatch`).
//! 7. A `deploy.kubernetes.egress` entry naming neither a host nor a namespace, whose rule would
//!    admit every destination on its port, or naming a host outside `allowedEgressHosts`
//!    (`EgressHostNotAllowed`).
//! 8. `spec.command`, each entrypoint and argument walked whole and word by word as the
//!    connectors text is (`SecretLiteral`); one beyond the walk's bound is `TooLarge`.
//! 9. `spec.env`, on either variant: the list walked whole, names and values, then each value
//!    word by word as `spec.command` is (`SecretLiteral`; `TooLarge` beyond the walk's bound);
//!    then, entry by entry, a name that is not an environment variable name
//!    `[A-Za-z_][A-Za-z0-9_]*` (`NotAVariableName`, `loom.agent`'s reason for the same rule), a
//!    name the controller sets on a container itself ([`CONTROLLER_ENV`]) or one an earlier
//!    entry gives (`EnvNameConflict`). Then each `spec.env` value and each `spec.command`
//!    entrypoint and argument again, as the program reads it after the kubelet's `$(VAR)`
//!    expansion (`$$` is `$`; a value against the controller's own environment and the entries
//!    before it, a command against the whole environment; an unknown name, such as a key of
//!    the `spec.secretRef` Secret, stays as written), whole and word by word (`SecretLiteral`,
//!    its detail saying it was found after expansion; `TooLarge` when the references add more
//!    than [`MAX_EXPANDED_BYTES`]).
//! 10. A `spec.workspace.mountPath` or `spec.secretFiles.mountPath` that is not absolute and
//!     plain, is, holds or lies beneath a directory the controller mounts ([`RESERVED_MOUNTS`])
//!     or the other one (`MountPathInvalid`).
//! 11. A `spec.workspace.claimName` outside the configuration's `allowedClaimNames`
//!     (`ClaimNotAllowed`).
//!
//! The walk of steps 2 and 3 reads a text that is not a `loom.agent/1` document only within the
//! agent decoder's bounds, [`MAX_WALKED_OPENERS`] `[` or `{` bytes and [`MAX_WALKED_BYTES`]; a
//! document beyond either is refused `TooLarge` rather than admitted unread. Words are walked in
//! sequences that each stay within both bounds.
//!
//! A refusal reads `refused: <reason> at <member>: <detail>`, as `b10x-loom agent check` prints
//! it, the member spelt from the resource (`spec.agent.bindings[1].base_url`); its detail never
//! repeats a value of the resource. An admitted spec reads as the check's line, `checked <agent>:
//! <n> states, <m> bindings, protocol <protocol>`; an admitted `Command` spec as `admitted <agent>
//! for its own command: …`.

use std::collections::HashMap;

use b10x_loom_agent::{Checked, RefusalReason, TOKEN_PREFIXES, check_text, spelling};
use llm_models::routes::Routes;
use loom_sdk::agent::{AgentSpec, parse};
use serde_yaml_ng::{Mapping, Value};

use b10x_loom_resource::model::AgentVariant;

use crate::config::{Config, ModelRoute};
use crate::render::{CONTROLLER_ENV, controller_env};
use crate::resource::{CommandSpec, EnvVar, LoomAgentSpec};

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

/// An admitted resource: its decoded spec, the three files of its ConfigMap and the check's
/// answer.
#[derive(Debug, Clone)]
pub struct Admitted {
    /// The decoded `loom.agent/1` document.
    pub spec: AgentSpec,
    /// `agent.yaml`: `spec.agent` as YAML.
    pub agent: String,
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
}

impl Admitted {
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
            None => format!(
                "admitted {} for its own command: a loom.agent/1 document with no credential, \
                 at {COMMAND_SPEC_FILE}",
                self.spec.agent.id.0
            ),
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
/// expanded; beyond it the expanded text is not walked and the resource is `TooLarge`.
pub const MAX_EXPANDED_BYTES: usize = MAX_WALKED_BYTES;

/// `text` as the kubelet expands it against `known`: `$$` is `$`, `$(NAME)` is the value of
/// `NAME` when `known` holds it and stays as written otherwise, any other `$` is itself. Each
/// substituted value is charged to `budget`; `None` when it runs out.
fn expand(text: &str, known: &HashMap<&str, String>, budget: &mut usize) -> Option<String> {
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
            match known.get(name) {
                Some(value) => {
                    *budget = budget.checked_sub(value.len())?;
                    out.push_str(value);
                }
                None => {
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

/// The words a text expanded to `expanded`, standing at `member`, is walked as: the text whole,
/// then each word of it; none when the expansion changed nothing, since the text as written
/// was walked already.
fn expanded_words(written: &str, expanded: String, member: String, out: &mut Vec<Word>) {
    if expanded == written {
        return;
    }
    let pieces = words(&expanded);
    out.push(Word {
        text: expanded,
        member: member.clone(),
        line: None,
    });
    out.extend(pieces.into_iter().map(|text| Word {
        text,
        member: member.clone(),
        line: None,
    }));
}

/// Walks `spec.env` and `spec.command` as the program reads them, after the kubelet's `$(VAR)`
/// expansion: each `spec.env` value against the controller's own environment of `variant` and
/// the `spec.env` entries before it, then each entrypoint and argument against the whole
/// environment. A key of the `spec.secretRef` Secret is not known here, so a reference to one
/// stays as written, as the kubelet leaves an unknown one. A credential found is `SecretLiteral`
/// at the member, never repeating it; references adding more than [`MAX_EXPANDED_BYTES`] are
/// `TooLarge`.
fn expanded_credential(spec: &LoomAgentSpec, variant: AgentVariant) -> Result<(), Refusal> {
    let mut known: HashMap<&str, String> = controller_env(variant)
        .into_iter()
        .map(|(name, value)| (name, value.to_owned()))
        .collect();
    let mut budget = MAX_EXPANDED_BYTES;
    let too_large = |member: String| {
        Refusal::new(
            "TooLarge",
            member,
            format!(
                "its `$(VAR)` references, expanded, add more than {MAX_EXPANDED_BYTES} bytes, \
                 more than the credential walk reads"
            ),
        )
    };
    let mut found = Vec::new();
    for (index, entry) in spec.env.iter().flatten().enumerate() {
        let member = format!("spec.env[{index}].value");
        let expanded =
            expand(&entry.value, &known, &mut budget).ok_or_else(|| too_large(member.clone()))?;
        known.insert(entry.name.as_str(), expanded.clone());
        expanded_words(&entry.value, expanded, member, &mut found);
    }
    if let Some(command) = &spec.command {
        for (list, items) in [("entrypoint", &command.entrypoint), ("args", &command.args)] {
            for (index, item) in items.iter().flatten().enumerate() {
                let member = format!("spec.command.{list}[{index}]");
                let expanded =
                    expand(item, &known, &mut budget).ok_or_else(|| too_large(member.clone()))?;
                expanded_words(item, expanded, member, &mut found);
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

/// The words of `text`, the raw `spec.connectorsConfig`, each with its line.
fn raw_words(text: &str) -> Vec<Word> {
    text.lines()
        .enumerate()
        .flat_map(|(index, line)| {
            words(line).into_iter().map(move |text| Word {
                text,
                member: "spec.connectorsConfig".to_owned(),
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
    word_credential(&raw_words(text), "spec.connectorsConfig")
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
/// anonymous endpoint at the model route, and one route per model a role names, to the model
/// route's model.
#[must_use]
pub fn default_routes(route: &ModelRoute, spec: &AgentSpec) -> String {
    let mut auth = Mapping::new();
    auth.insert("kind".into(), "anonymous".into());
    let mut endpoint = Mapping::new();
    endpoint.insert("base_url".into(), route.url.clone().into());
    endpoint.insert("protocol".into(), route.protocol.clone().into());
    endpoint.insert("auth".into(), Value::Mapping(auth));
    let mut endpoints = Mapping::new();
    endpoints.insert(MODEL_ROUTE_ENDPOINT.into(), Value::Mapping(endpoint));
    let mut routes = Mapping::new();
    for role in &spec.model.roles {
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
    let agent = yaml(&spec.agent, "spec.agent")?;
    let checked = match variant {
        AgentVariant::AgentServe => Some(check_text(&agent, actions).map_err(|refused| {
            Refusal::new(
                spelling(refused.reason),
                within("spec.agent", &refused.member),
                refused.detail,
            )
        })?),
        AgentVariant::Command => {
            credential(&agent, "spec.agent").map_err(|refusal| {
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
    };
    let decoded = parse(&agent).map_err(|error| {
        Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.agent",
            error.to_string(),
        )
    })?;
    if variant == AgentVariant::AgentServe
        && decoded
            .runtime
            .as_ref()
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

    let (routes, default_routes) = match (given, &config.model_route) {
        (Some(text), _) => (text, false),
        (None, Some(route)) => (default_routes(route, &decoded), true),
        (None, None) if variant == AgentVariant::Command => (String::new(), false),
        (None, None) => {
            return Err(Refusal::new(
                "NoModelRoute",
                "spec.routes",
                "it is empty and the controller configures no model route",
            ));
        }
    };

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
        .deploy
        .as_ref()
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

    Ok(Admitted {
        spec: decoded,
        agent,
        routes,
        connectors,
        default_routes,
        variant,
        checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        let mut config = Config::builtin();
        config.namespace = "agents".to_owned();
        config
    }

    fn spec(connectors: Option<&str>) -> LoomAgentSpec {
        LoomAgentSpec {
            agent: serde_json::json!({
                "format": "loom.agent/1",
                "agent": {"id": "a", "name": "A", "brief": "answers"},
                "model": {"routes": "routes.yaml", "roles": [{"role": "agent", "model": "small"}]},
            }),
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
        serve.agent["runtime"] =
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
            deployed.agent["deploy"] = serde_json::json!({"kubernetes": {
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
        Routes::parse(&default_routes(&route, &decoded)).expect("llm reads the default routes");
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
            default_routes(&route, &decoded),
            "format: llm.routes/1\nendpoints:\n  model-route:\n    base_url: \
             http://model-route.agents.svc:8080/v1\n    protocol: messages\n    auth:\n      \
             kind: anonymous\nroutes:\n  small: model-route/small-model\n"
        );
    }
}
