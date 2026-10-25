//! Whether a `loom.agent/1` document can be served, answered before anything runs:
//! `loom.agent.CheckAgentSpec` (`ess/domains/agent.yaml`), which `b10x-loom agent check --spec`
//! runs.
//!
//! [`check_text`] reads the document (one leading byte order mark dropped), walks its YAML for
//! credentials, decodes it with `loom_sdk::agent::parse` and [`check`]s it against the protocols
//! a caller's catalog holds, given as a closure from a protocol's `name@major` to its action ids,
//! as `b10x_loom_executor::workflow::load` takes them; the command line and the conformance target
//! pass `ProtocolCatalog::plugins()`. The first refusal found is the answer, in this order:
//!
//! 1. [`RefusalReason::Malformed`]: the text is not one YAML document the decoder reads (a scan
//!    error, an empty document, several documents, past the decoder's byte, flow or alias bound):
//!    `document`, the decoder's message;
//! 2. [`RefusalReason::SecretLiteral`]: a scalar, key or tag anywhere in the document holds a
//!    credential: a URL with user information or a query component, or a value starting with a
//!    known token prefix ([`TOKEN_PREFIXES`]). The detail names the shape and never repeats the
//!    value. It is found by walking the YAML before decoding, so a member the decoder would
//!    refuse, whose message would repeat the value, is answered here first;
//! 3. [`RefusalReason::Malformed`]: the decoder refuses the document (`document`, its message);
//! 4. [`RefusalReason::UnknownProtocol`]: the catalog does not hold the inner loop's protocol, or
//!    states are declared with no inner loop;
//! 5. [`RefusalReason::UnknownAction`]: a state names an action its protocol does not declare;
//! 6. [`RefusalReason::UnboundSource`]: a state, the outer loop or the channel names a source no
//!    binding declares, in that order;
//! 7. [`RefusalReason::HttpMethod`]: an `http` binding's method is not exactly `GET`;
//! 8. [`RefusalReason::SendWithoutWrites`]: `reply.send` is a capability while writes are `none`
//!    or undeclared;
//! 9. [`RefusalReason::EmptyChannelAllow`]: the channel allows no channel id.
//!
//! A member is named as the document spells it: `bindings[1].base_url`, `states[0].actions[2]`,
//! `policy.capabilities[2]`. Nothing runs and nothing but the given text is read.
//!
//! Serving a checked spec is [`serve`]: [`SpecPlugin`], the plugin of the host built from it,
//! which `b10x-loom agent serve --spec --state` runs; its sends decide `loom.agent.AdmitSend`
//! ([`admit_send`]), and its `http` bindings are read by [`http`]: [`HttpGet`], deciding
//! `loom.agent.AdmitHttpRead` ([`admit_http_read`]) before any request and
//! `loom.agent.BoundHttpAnswer` ([`bound_http_answer`]) over the answer.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use loom_sdk::agent::{AgentSpec, Binding};

pub use http::{
    HTTP_TIMEOUT, HttpGet, admit_http_read, bound_http_answer, http_refusal_spelling, path_allowed,
};
pub use loom::agent::{
    AdmitHttpRead, AdmitHttpReadOutcome, BoundHttpAnswer, BoundHttpAnswerOutcome,
    HttpReadRefusalReason, HttpReadRefused,
};
pub use loom_sdk::agent::{
    AdmitSend, AdmitSendOutcome, RefusalReason, SendRefusalReason, SendRefused, SpecRefused,
    WriteMode,
};
pub use serve::{
    ResumeFailure, Resumed, SpecAuthority, SpecPlugin, admit_send, commission_id,
    send_refusal_spelling,
};

mod credentials;
pub mod http;
pub mod serve;

/// What a serveable spec was checked as: its agent id, the inner loop's protocol (none without
/// loops), and how many states and bindings it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The agent's id.
    pub agent: String,
    /// The protocol the inner loop runs on, as `name@major`; `None` when the spec has no loops.
    pub protocol: Option<String>,
    /// How many states the spec declares.
    pub states: usize,
    /// How many bindings the spec declares.
    pub bindings: usize,
}

/// The prefixes of the credentials a value may not start with: chat bot, user and app tokens,
/// model-provider and payment keys, source-host tokens, a cloud access key id, an authorization
/// header and a PEM block.
pub const TOKEN_PREFIXES: [&str; 11] = [
    "xoxb-",
    "xoxp-",
    "xapp-",
    "sk-",
    "sk_live_",
    "sk_test_",
    "ghp_",
    "github_pat_",
    "AKIA",
    "Bearer ",
    "-----BEGIN",
];

/// The capability that sends a reply, which needs writes.
const SEND: &str = "reply.send";

/// Checks `text`, one `loom.agent/1` document, against `actions`, which answers the action ids of
/// a plugin-catalog protocol by its `name@major` or `None` when the catalog does not hold it.
///
/// One leading U+FEFF is dropped first: YAML admits a byte order mark at the start of a stream.
///
/// # Errors
/// The first refusal, as the crate documentation orders them; [`RefusalReason::Malformed`] when
/// the document does not decode, naming `document` and carrying the decoder's message, unless
/// the document holds a credential the decoder's message could repeat.
pub fn check_text(
    text: &str,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<Checked, SpecRefused> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let decoded = loom_sdk::agent::parse(text);
    let budget = match &decoded {
        Ok(_) => Some(usize::MAX),
        Err(_) => credentials::bound(text),
    };
    if let Some((member, shape)) = budget.and_then(|budget| credentials::first(text, budget)) {
        return Err(refused(RefusalReason::SecretLiteral, member, shape));
    }
    let spec = decoded
        .map_err(|error| refused(RefusalReason::Malformed, "document", error.to_string()))?;
    check(&spec, actions)
}

/// Checks `spec`, a decoded `loom.agent/1` document, against `actions` as [`check_text`] does,
/// from [`RefusalReason::UnknownProtocol`] on. A credential is the text's to refuse: it is found
/// by walking the YAML before decoding, which a spec decoded or built in code no longer has.
///
/// # Errors
/// The first refusal, as the crate documentation orders them.
pub fn check(
    spec: &AgentSpec,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<Checked, SpecRefused> {
    let declared = protocol(spec, actions)?;
    state_actions(spec, &declared)?;
    sources(spec)?;
    http_methods(spec)?;
    send_without_writes(spec)?;
    channel_allow(spec)?;
    Ok(Checked {
        agent: spec.agent.id.0.clone(),
        protocol: spec
            .loops
            .as_ref()
            .map(|loops| loops.inner.protocol.clone()),
        states: spec.states.len(),
        bindings: spec.bindings.len(),
    })
}

/// `reason` as the enum `loom.agent.RefusalReason` spells it.
pub fn spelling(reason: RefusalReason) -> &'static str {
    match reason {
        RefusalReason::Malformed => "Malformed",
        RefusalReason::SecretLiteral => "SecretLiteral",
        RefusalReason::UnknownProtocol => "UnknownProtocol",
        RefusalReason::UnknownAction => "UnknownAction",
        RefusalReason::UnboundSource => "UnboundSource",
        RefusalReason::HttpMethod => "HttpMethod",
        RefusalReason::SendWithoutWrites => "SendWithoutWrites",
        RefusalReason::EmptyChannelAllow => "EmptyChannelAllow",
    }
}

/// `refusal` as a person reads it: `<reason> at <member>: <detail>`, the reason in the enum's
/// spelling. `SpecRefused` is a generated type, so this is a function rather than its `Display`.
pub fn describe(refusal: &SpecRefused) -> String {
    format!(
        "{} at {}: {}",
        spelling(refusal.reason),
        refusal.member,
        refusal.detail
    )
}

fn refused(
    reason: RefusalReason,
    member: impl Into<String>,
    detail: impl Into<String>,
) -> SpecRefused {
    SpecRefused {
        reason,
        member: member.into(),
        detail: detail.into(),
    }
}

/// The shape of the credential `value` holds, never the value: a URL anywhere in it carrying user
/// information or a query component, or a known token prefix it starts with.
///
/// A URL's authority runs from after `://` to the first `/`, or to its end: a `#` or a `?` does
/// not end it, so a password holding either is user information all the same.
fn credential(value: &str) -> Option<String> {
    for url in urls(value) {
        let rest = &url[url.find("://").map_or(0, |at| at + 3)..];
        let authority = &rest[..rest.find('/').unwrap_or(rest.len())];
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            return Some(if userinfo.contains(':') {
                "a URL carrying a user name and password".to_owned()
            } else {
                "a URL carrying a user name".to_owned()
            });
        }
        if rest.contains('?') {
            return Some("a URL carrying a query component".to_owned());
        }
    }
    let start = value.trim_start();
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| start.starts_with(*prefix))
        .map(|prefix| format!("a value with the prefix `{prefix}`"))
}

/// Every URL `value` holds: each `://` with the scheme before it (letters, digits, `+`, `-`, `.`,
/// at least one) and everything after it up to whitespace.
fn urls(value: &str) -> Vec<&str> {
    let is_scheme = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.');
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = value[from..].find("://").map(|at| from + at) {
        let start = value[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_scheme(*c))
            .last()
            .map_or(at, |(index, _)| index);
        let end = value[at..]
            .find(char::is_whitespace)
            .map_or(value.len(), |end| at + end);
        if start < at {
            found.push(&value[start..end]);
        }
        from = at + 3;
    }
    found
}

/// The actions the inner loop's protocol declares, `None` when the spec has neither loops nor
/// states.
fn protocol(
    spec: &AgentSpec,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<Option<Vec<String>>, SpecRefused> {
    match &spec.loops {
        Some(loops) => actions(&loops.inner.protocol).map(Some).ok_or_else(|| {
            refused(
                RefusalReason::UnknownProtocol,
                "loops.inner.protocol",
                loops.inner.protocol.clone(),
            )
        }),
        None if spec.states.is_empty() => Ok(None),
        None => Err(refused(
            RefusalReason::UnknownProtocol,
            "states",
            "no inner loop names a protocol to check the states against",
        )),
    }
}

fn state_actions(spec: &AgentSpec, declared: &Option<Vec<String>>) -> Result<(), SpecRefused> {
    let Some(declared) = declared else {
        return Ok(());
    };
    for (i, state) in spec.states.iter().enumerate() {
        for (j, action) in state.actions.iter().enumerate() {
            if !declared.contains(action) {
                return Err(refused(
                    RefusalReason::UnknownAction,
                    format!("states[{i}].actions[{j}]"),
                    action.clone(),
                ));
            }
        }
    }
    Ok(())
}

fn binding_name(binding: &Binding) -> &str {
    match binding {
        Binding::Connectors(binding) => &binding.name.0,
        Binding::Http(binding) => &binding.name.0,
    }
}

fn sources(spec: &AgentSpec) -> Result<(), SpecRefused> {
    let bound = |source: &str| spec.bindings.iter().any(|b| binding_name(b) == source);
    let mut named: Vec<(String, &str)> = Vec::new();
    for (i, state) in spec.states.iter().enumerate() {
        for (j, source) in state.sources.iter().enumerate() {
            named.push((format!("states[{i}].sources[{j}]"), &source.0));
        }
    }
    if let Some(loops) = &spec.loops {
        for (j, source) in loops.outer.sources.iter().enumerate() {
            named.push((format!("loops.outer.sources[{j}]"), &source.0));
        }
    }
    if let Some(channel) = &spec.channel {
        named.push(("channel.binding".to_owned(), &channel.binding.0));
    }
    match named.into_iter().find(|(_, source)| !bound(source)) {
        Some((member, source)) => Err(refused(RefusalReason::UnboundSource, member, source)),
        None => Ok(()),
    }
}

fn http_methods(spec: &AgentSpec) -> Result<(), SpecRefused> {
    for (i, binding) in spec.bindings.iter().enumerate() {
        if let Binding::Http(binding) = binding
            && binding.method != "GET"
        {
            return Err(refused(
                RefusalReason::HttpMethod,
                format!("bindings[{i}].method"),
                binding.method.clone(),
            ));
        }
    }
    Ok(())
}

fn send_without_writes(spec: &AgentSpec) -> Result<(), SpecRefused> {
    let writes = spec
        .writes
        .as_ref()
        .is_some_and(|writes| writes.mode != WriteMode::None);
    if writes {
        return Ok(());
    }
    let capabilities = spec.policy.iter().flat_map(|policy| &policy.capabilities);
    match capabilities
        .enumerate()
        .find(|(_, capability)| *capability == SEND)
    {
        Some((i, _)) => Err(refused(
            RefusalReason::SendWithoutWrites,
            format!("policy.capabilities[{i}]"),
            SEND,
        )),
        None => Ok(()),
    }
}

fn channel_allow(spec: &AgentSpec) -> Result<(), SpecRefused> {
    match &spec.channel {
        Some(channel) if channel.allow.is_empty() => Err(refused(
            RefusalReason::EmptyChannelAllow,
            "channel.allow",
            "no channel id is allowed",
        )),
        _ => Ok(()),
    }
}
