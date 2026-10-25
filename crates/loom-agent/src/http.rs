//! The read of an `http` binding (`story:agent-http-binding`): one GET of an allow-listed path
//! under the binding's `base_url`, bounded by its `max_bytes`, carrying the bearer the variable
//! `bearer_env` names, with no redirect followed. [`HttpGet`] is the [`HttpReader`] a served agent
//! reads its `http` bindings through; what it returns reaches the model as data through the
//! effect port's transcript, never as instructions.
//!
//! Before any request it decides `loom.agent.AdmitHttpRead` ([`admit_http_read`]) over the
//! binding's method, whether the path is allowed ([`path_allowed`]: exactly one of `allow_paths`,
//! and a `loom_sdk::agent::plain_http_path`, with no `\`, no `.` or `..` segment, no encoded
//! separator or dot, no query or fragment and no whitespace) and whether the bearer variable holds
//! text (unset, empty or blank is missing); the first refusal is the answer and starts no request.
//! The one request is a GET of `base_url` + `path` with the query as parameters; the client
//! follows no redirect, uses no proxy and is bounded by [`HTTP_TIMEOUT`]. It then decides
//! `loom.agent.BoundHttpAnswer` ([`bound_http_answer`]) over the status and the bytes read (at
//! most one past `max_bytes`): a redirect is a refusal naming at most [`LOCATION_BYTES`] of the
//! location, and no more than `max_bytes`, a status outside 2xx a refusal naming it, a body past
//! the bound is cut at it and marked, any other read whole. The text the model reads is never
//! longer than `max_bytes`: the bytes are cut at the bound first, backed off to a character
//! boundary, and a byte that is not UTF-8 is dropped rather than replaced.
//!
//! The crate's HTTP client is reqwest, the one llm's `b10x-llm-http` is built on; that crate
//! exposes no GET at the pinned tag (0.8.1: `post_json`, `post_sse`), so the GET is composed here
//! on the same client, with the same redirect and proxy rules.

use std::ffi::OsStr;
use std::time::Duration;

use b10x_loom_plugin::effects::{HttpAnswer, HttpReadError, HttpReader};
use loom::agent::{
    AdmitHttpRead, AdmitHttpReadOutcome, BoundHttpAnswer, BoundHttpAnswerOutcome, HttpBinding,
    HttpReadRefusalReason, HttpReadRefused,
};
use loom_sdk::agent::plain_http_path;
use reqwest::Url;
use reqwest::header::{AUTHORIZATION, HeaderValue, LOCATION};

/// The bound on one read: connecting, and the whole exchange.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// The most bytes of a redirect's location a refusal carries; the binding's `max_bytes` bounds it
/// further.
pub const LOCATION_BYTES: usize = 256;

/// Decides `loom.agent.AdmitHttpRead` (`ess/domains/agent.yaml`): the first refusal in
/// declaration order, else `admitted`.
pub fn admit_http_read(input: &AdmitHttpRead) -> AdmitHttpReadOutcome {
    let refused = |reason: HttpReadRefusalReason| HttpReadRefused { reason };
    if input.method != "GET" {
        return AdmitHttpReadOutcome::MethodNotGet {
            error: refused(HttpReadRefusalReason::MethodNotGet),
        };
    }
    if !input.path_allowed {
        return AdmitHttpReadOutcome::PathNotAllowed {
            error: refused(HttpReadRefusalReason::PathNotAllowed),
        };
    }
    if input.bearer_missing {
        return AdmitHttpReadOutcome::BearerMissing {
            error: refused(HttpReadRefusalReason::BearerMissing),
        };
    }
    AdmitHttpReadOutcome::Admitted
}

/// Decides `loom.agent.BoundHttpAnswer` (`ess/domains/agent.yaml`): a redirect, a failing status,
/// a body past the bound (`cut`), else `whole`.
pub fn bound_http_answer(input: &BoundHttpAnswer) -> BoundHttpAnswerOutcome {
    let refused = |reason: HttpReadRefusalReason| HttpReadRefused { reason };
    if (300..=399).contains(&input.status) {
        return BoundHttpAnswerOutcome::Redirected {
            error: refused(HttpReadRefusalReason::Redirected),
        };
    }
    if input.status <= 199 || input.status >= 400 {
        return BoundHttpAnswerOutcome::FailedStatus {
            error: refused(HttpReadRefusalReason::FailedStatus),
        };
    }
    if input.bytes > input.max_bytes {
        return BoundHttpAnswerOutcome::Cut;
    }
    BoundHttpAnswerOutcome::Whole
}

/// `reason` as the enum `loom.agent.HttpReadRefusalReason` spells it.
pub fn http_refusal_spelling(reason: HttpReadRefusalReason) -> &'static str {
    match reason {
        HttpReadRefusalReason::MethodNotGet => "MethodNotGet",
        HttpReadRefusalReason::PathNotAllowed => "PathNotAllowed",
        HttpReadRefusalReason::BearerMissing => "BearerMissing",
        HttpReadRefusalReason::Redirected => "Redirected",
        HttpReadRefusalReason::FailedStatus => "FailedStatus",
    }
}

/// Whether `path` is one a binding with `allow_paths` may be read at: a
/// [`plain_http_path`] (it starts with `/` and holds no `\`, no `.` or `..` segment, no
/// percent-encoded separator or dot, no `?` or `#`, the query being given apart, and no whitespace
/// or control character), and exactly one of `allow_paths`. The rule is the one the decoder holds
/// every `allow_paths` entry to, and it holds even for an allow list that names such a path.
pub fn path_allowed(path: &str, allow_paths: &[String]) -> bool {
    plain_http_path(path) && allow_paths.iter().any(|allowed| allowed == path)
}

/// Why `value`, the value of the variable `bearer_env` names, is no bearer: it is unset, or holds
/// empty or blank text. `None` when it holds a token (or bytes that are not text, which
/// [`bearer`] refuses).
fn no_bearer(value: Option<&OsStr>) -> Option<&'static str> {
    match value {
        None => Some("is unset"),
        Some(value) if value.to_str().is_some_and(|text| text.trim().is_empty()) => {
            Some("holds no text")
        }
        Some(_) => None,
    }
}

/// The text of `bytes` cut at `max` bytes: the bytes are cut first, so the text is never longer,
/// and every byte that is not UTF-8, a character the cut split included, is dropped rather than
/// replaced (a replacement character is three bytes).
fn utf8_within(bytes: &[u8], max: usize) -> String {
    bytes[..bytes.len().min(max)]
        .utf8_chunks()
        .map(|chunk| chunk.valid())
        .collect()
}

/// The reader of a served agent's `http` bindings (see the module documentation).
#[derive(Debug, Clone)]
pub struct HttpGet {
    timeout: Duration,
}

impl Default for HttpGet {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpGet {
    /// A reader bounded by [`HTTP_TIMEOUT`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: HTTP_TIMEOUT,
        }
    }

    /// A reader whose every read is bounded by `timeout`: connecting, and the whole exchange.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

/// A refusal naming `reason` in the enum's spelling, then `why`.
fn refused(reason: HttpReadRefusalReason, why: String) -> HttpReadError {
    HttpReadError::Refused(format!("{}: {why}", http_refusal_spelling(reason)))
}

/// The URL of `path` under the binding's `base_url`: an `http` or `https` URL with a host and no
/// user information.
fn url_of(binding: &HttpBinding, path: &str) -> Result<Url, HttpReadError> {
    let base = binding.base_url.trim_end_matches('/');
    let url = Url::parse(&format!("{base}{path}")).map_err(|error| {
        HttpReadError::Refused(format!(
            "the binding's base_url and the path make no URL: {error}"
        ))
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(HttpReadError::Refused(
            "the binding's base_url is not an http or https URL with a host and no user \
             information"
                .to_owned(),
        ));
    }
    Ok(url)
}

/// The `authorization` header of the bearer `variable` holds, marked sensitive; its value is
/// never repeated in a refusal.
fn bearer(variable: &str) -> Result<HeaderValue, HttpReadError> {
    let token = std::env::var(variable).map_err(|_| {
        HttpReadError::Refused(format!(
            "the variable `{variable}` named by bearer_env holds no text"
        ))
    })?;
    let mut value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
        HttpReadError::Refused(format!(
            "the variable `{variable}` named by bearer_env is no header value"
        ))
    })?;
    value.set_sensitive(true);
    Ok(value)
}

impl HttpReader for HttpGet {
    fn read(
        &self,
        binding: &HttpBinding,
        path: &str,
        query: &[(String, String)],
    ) -> Result<HttpAnswer, HttpReadError> {
        let missing = binding
            .bearer_env
            .as_deref()
            .and_then(|variable| no_bearer(std::env::var_os(variable).as_deref()));
        match admit_http_read(&AdmitHttpRead {
            method: binding.method.clone(),
            path_allowed: path_allowed(path, &binding.allow_paths),
            bearer_missing: missing.is_some(),
        }) {
            AdmitHttpReadOutcome::MethodNotGet { error } => {
                return Err(refused(
                    error.reason,
                    format!("the binding's method is `{}`, not GET", binding.method),
                ));
            }
            AdmitHttpReadOutcome::PathNotAllowed { error } => {
                return Err(refused(
                    error.reason,
                    format!(
                        "the path `{path}` is not one of the binding's allow_paths, or holds a \
                         `..` segment, an encoded separator, a query or a fragment"
                    ),
                ));
            }
            AdmitHttpReadOutcome::BearerMissing { error } => {
                return Err(refused(
                    error.reason,
                    format!(
                        "the variable `{}` named by bearer_env {}",
                        binding.bearer_env.as_deref().unwrap_or_default(),
                        missing.unwrap_or("is unset")
                    ),
                ));
            }
            AdmitHttpReadOutcome::Admitted => {}
        }
        let max_bytes = usize::try_from(binding.max_bytes)
            .ok()
            .filter(|bound| *bound > 0)
            .ok_or_else(|| {
                HttpReadError::Refused(format!(
                    "the binding's max_bytes `{}` is no bound",
                    binding.max_bytes
                ))
            })?;
        let url = url_of(binding, path)?;
        let authorization = binding.bearer_env.as_deref().map(bearer).transpose()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                HttpReadError::Unavailable(format!("no runtime for the read: {error}"))
            })?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(self.timeout)
            .timeout(self.timeout)
            .build()
            .map_err(|error| {
                HttpReadError::Unavailable(format!("the HTTP client did not start: {error}"))
            })?;
        runtime.block_on(async move {
            let mut request = client.get(url).query(query);
            if let Some(authorization) = authorization {
                request = request.header(AUTHORIZATION, authorization);
            }
            let mut response = request.send().await.map_err(|error| {
                HttpReadError::Unavailable(format!("the request failed: {}", error.without_url()))
            })?;
            let status = i64::from(response.status().as_u16());
            // The location is the endpoint's text: at most `LOCATION_BYTES` of it, and no more
            // than the binding's bound, reach the refusal.
            let location = response.headers().get(LOCATION).map(|value| {
                let bytes = value.as_bytes();
                let shown = utf8_within(bytes, LOCATION_BYTES.min(max_bytes));
                if shown.len() == bytes.len() {
                    format!("`{shown}`")
                } else {
                    format!(
                        "`{shown}` (cut: {} of {} bytes shown)",
                        shown.len(),
                        bytes.len()
                    )
                }
            });
            // Only a success has its body read, and at most one byte past the bound: the body of a
            // redirect or a failure is never read.
            let mut body: Vec<u8> = Vec::new();
            if response.status().is_success() {
                while body.len() <= max_bytes {
                    match response.chunk().await {
                        Ok(Some(chunk)) => {
                            let room = max_bytes + 1 - body.len();
                            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
                        }
                        Ok(None) => break,
                        Err(error) => {
                            return Err(HttpReadError::Unavailable(format!(
                                "the response body failed: {}",
                                error.without_url()
                            )));
                        }
                    }
                }
            }
            let bytes = i64::try_from(body.len()).unwrap_or(i64::MAX);
            match bound_http_answer(&BoundHttpAnswer {
                status,
                bytes,
                max_bytes: binding.max_bytes,
            }) {
                BoundHttpAnswerOutcome::Redirected { error } => Err(refused(
                    error.reason,
                    format!(
                        "the endpoint answered {status} redirecting to {}, which is not followed",
                        location.as_deref().unwrap_or("no location")
                    ),
                )),
                BoundHttpAnswerOutcome::FailedStatus { error } => Err(refused(
                    error.reason,
                    format!("the endpoint answered {status}"),
                )),
                BoundHttpAnswerOutcome::Cut => Ok(HttpAnswer {
                    status,
                    body: utf8_within(&body, max_bytes),
                    cut: true,
                }),
                BoundHttpAnswerOutcome::Whole => Ok(HttpAnswer {
                    status,
                    body: utf8_within(&body, max_bytes),
                    cut: false,
                }),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `bearer_missing` is true for a variable that is unset or holds empty or blank text, and
    /// false for one holding a token; blank text needs no variable set in the test process.
    #[test]
    fn an_unset_empty_or_blank_variable_is_no_bearer() {
        assert_eq!(no_bearer(None), Some("is unset"));
        for blank in ["", " ", "\t\n ", "\u{3000}"] {
            assert_eq!(
                no_bearer(Some(OsStr::new(blank))),
                Some("holds no text"),
                "{blank:?}"
            );
        }
        assert_eq!(no_bearer(Some(OsStr::new("token"))), None);
        assert_eq!(no_bearer(Some(OsStr::new(" token "))), None);
    }

    /// The text is never past the bound, whatever the bytes.
    #[test]
    fn the_text_of_a_body_is_never_past_the_bound() {
        assert_eq!(utf8_within("\u{e9}\u{e9}".as_bytes(), 3), "\u{e9}");
        assert_eq!(utf8_within(&[0xFF; 9], 9), "");
        assert_eq!(utf8_within(b"a\xffb", 3), "ab");
        assert_eq!(utf8_within(b"abc", 9), "abc");
        for max in 0..12 {
            assert!(utf8_within("a\u{e9}\u{20ac}\u{1f600}".as_bytes(), max).len() <= max);
        }
    }
}
