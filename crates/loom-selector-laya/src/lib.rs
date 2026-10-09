#![forbid(unsafe_code)]

//! A `FastTyped` [`ActionSelector`] over a Laya endpoint: an experiment, never Loom's default.
//!
//! [`LayaSelector`] asks a Laya server to choose one action of the candidate set it was handed, and
//! reports the probability Laya gives that choice as the selection's confidence. The confidence
//! grants nothing (Atlas ADR 0073): Loom still refuses an action its catalogue does not list,
//! Commission still rechecks every proposal, and thresholding is not here.
//!
//! # The wire
//!
//! The request and answer follow the Laya repository README at commit `1adc59f`,
//! <https://github.com/NandhaKishorM/laya/blob/1adc59f7e371deb601fcfa18a14e25db238addcc/README.md>,
//! § "Self-Hosting: HTTP Server (Jev-compatible)" and its description of an answer. Most Laya
//! servers speak the same `POST /v1/systemone` wire (<https://laya.tools/laya-api-servers>).
//!
//! The selector sends one `choice` question, [`QUESTION`], whose criteria are the candidate action
//! ids:
//!
//! ```json
//! {"state": {"goal": "<the prompt>"},
//!  "questions": {"action": {"type": "choice", "instructions": "<instructions>",
//!                           "criteria": {"<action id>": "<action id>"}}}}
//! ```
//!
//! and reads `answers.action.choice` as the action and `answers.action.answer_confidence`, "the
//! probability of the reported answer", as the confidence. It does not read `confidence`, which on
//! Laya is 1 minus the normalised entropy of the distribution, not a probability. Every other field
//! of the answer is ignored.
//!
//! # Failures
//!
//! A failure is a selection error, so the caller can fall back to a stronger selector
//! (`docs/integrations/laya-fast-selection.md`). Each of these is [`SelectorError::Unavailable`]
//! naming the reason, and none is retried: a transport failure, a timeout, a non-2xx status (the
//! server answers 422 for a malformed request and 413 for over [`MAX_CANDIDATES`] options), an
//! answer that is not JSON or nests past serde_json's 128 levels, an answer with no `choice` or no
//! `answer_confidence`, a probability outside [0, 1], and a `choice` that names no candidate.
//! Refusing an out-of-set choice here, rather than passing it on for Loom's catalogue check, keeps
//! the rule in the adapter that reads the answer; Loom's check still stands behind it. More than
//! [`MAX_CANDIDATES`] distinct candidates are refused before anything is sent, and no candidates
//! at all is [`SelectorError::NothingAdmissible`].
//!
//! # Locality and boundary
//!
//! The endpoint is configuration, so one adapter serves a local `laya-serve` and a hosted Laya
//! alike. This story sends no credential. The crate depends on the executor; no product crate
//! (`b10x-loom-cli`, `b10x-loom-sdk`) depends on it, so Loom builds and runs with no Laya code.
//! The transport is llm's `b10x-llm-http` (`HttpClient::post_json`), run on a Tokio runtime the
//! selector owns, so the synchronous [`ActionSelector`] port can be called from any thread.

use std::collections::BTreeSet;
use std::sync::mpsc;
use std::time::Duration;

use b10x_loom_executor::model::primitives::Decimal;
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, SelectorError};
use llm_core::Cancel;
use llm_http::{HeaderMap, HttpClient, Limits};
use serde_json::{Map, Value, json};

/// The path of a Laya server's selection endpoint, below the configured base URL.
pub const PATH: &str = "/v1/systemone";

/// The id of the one question the selector asks, and so the key of its answer.
pub const QUESTION: &str = "action";

/// The most choice options a Laya server answers in one question (`MAX_CHOICE_OPTIONS`; more is a
/// 413 before inference).
pub const MAX_CANDIDATES: usize = 100;

/// The instructions the question carries unless [`LayaSelector::with_instructions`] replaces them.
pub const DEFAULT_INSTRUCTIONS: &str =
    "Which one of these actions should be taken next to make progress on the goal?";

/// How long one selection may take, connecting included, unless
/// [`LayaSelector::with_timeout`] says otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest power of ten a rendered probability may carry; anything past it is not an answer.
const MAX_EXPONENT: i64 = 400;

/// Why a [`LayaSelector`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupError(String);

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SetupError {}

/// A `FastTyped` selector that asks a Laya endpoint to choose.
pub struct LayaSelector {
    url: String,
    instructions: String,
    client: HttpClient,
    runtime: Option<tokio::runtime::Runtime>,
}

impl std::fmt::Debug for LayaSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayaSelector")
            .field("url", &self.url)
            .field("instructions", &self.instructions)
            .finish_non_exhaustive()
    }
}

impl LayaSelector {
    /// A selector for the Laya server at `endpoint`, its base URL (`http://127.0.0.1:8000`, or
    /// with a reverse-proxy prefix such as `https://host/laya`), bounded by [`DEFAULT_TIMEOUT`].
    ///
    /// # Errors
    /// Refuses an endpoint that is not an `http` or `https` URL, and a runtime or HTTP client that
    /// does not start.
    pub fn new(endpoint: &str) -> Result<Self, SetupError> {
        Self::with_timeout(endpoint, DEFAULT_TIMEOUT)
    }

    /// As [`LayaSelector::new`], with every selection bounded by `timeout`: connecting, the
    /// response headers, each read and the whole exchange.
    ///
    /// # Errors
    /// As [`LayaSelector::new`], and a zero timeout or one over a day.
    pub fn with_timeout(endpoint: &str, timeout: Duration) -> Result<Self, SetupError> {
        let endpoint = endpoint.trim_end_matches('/');
        if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
            return Err(SetupError(format!(
                "the Laya endpoint `{endpoint}` is not an http or https URL"
            )));
        }
        let limits = Limits {
            response_headers: timeout,
            idle: timeout,
            total: timeout,
        };
        let client = HttpClient::with_connect_timeout(limits, timeout)
            .map_err(|error| SetupError(format!("the HTTP client did not start: {error}")))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("loom-selector-laya")
            .enable_all()
            .build()
            .map_err(|error| {
                SetupError(format!("the selector's runtime did not start: {error}"))
            })?;
        Ok(Self {
            url: format!("{endpoint}{PATH}"),
            instructions: DEFAULT_INSTRUCTIONS.to_owned(),
            client,
            runtime: Some(runtime),
        })
    }

    /// The same selector, asking its question with `instructions`.
    #[must_use]
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    /// The URL every selection is posted to.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The request body asking Laya to choose one of `candidates` for `goal`.
    fn request(&self, goal: &str, candidates: &BTreeSet<&str>) -> Value {
        let criteria: Map<String, Value> = candidates
            .iter()
            .map(|action| ((*action).to_owned(), Value::String((*action).to_owned())))
            .collect();
        json!({
            "state": {"goal": goal},
            "questions": {
                (QUESTION): {
                    "type": "choice",
                    "instructions": self.instructions,
                    "criteria": criteria,
                }
            }
        })
    }

    /// Posts `body` once on the selector's runtime and waits for the answer on this thread.
    fn post(&self, body: Vec<u8>) -> Result<Value, String> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| "the selector's runtime is shut down".to_owned())?;
        let client = self.client.clone();
        let url = self.url.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        runtime.spawn(async move {
            let answer = client
                .post_json(&url, HeaderMap::new(), body, &Cancel::new())
                .await;
            let _ = sender.send(answer);
        });
        receiver
            .recv()
            .map_err(|_| "the request stopped before it answered".to_owned())?
            .map_err(|error| error.to_string())
    }
}

impl Drop for LayaSelector {
    fn drop(&mut self) {
        // Dropping a runtime blocks, which panics inside another runtime; shutting it down in the
        // background does not.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl ActionSelector for LayaSelector {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let actions: BTreeSet<&str> = candidates
            .iter()
            .map(|entry| entry.action.as_str())
            .collect();
        if actions.is_empty() {
            return Err(SelectorError::NothingAdmissible);
        }
        if actions.len() > MAX_CANDIDATES {
            return Err(SelectorError::Unavailable(format!(
                "{} candidates exceed the {MAX_CANDIDATES} options a Laya server answers in one \
                 question; nothing was sent",
                actions.len()
            )));
        }
        let body = self.request(&context.prompt, &actions).to_string();
        let answer = self.post(body.into_bytes()).map_err(|reason| {
            SelectorError::Unavailable(format!("the Laya endpoint {} failed: {reason}", self.url))
        })?;
        read_answer(&answer, &actions).map_err(|reason| {
            SelectorError::Unavailable(format!(
                "the Laya endpoint {} answered unusably: {reason}",
                self.url
            ))
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::FastTyped
    }
}

/// The choice `answer` makes among `candidates`, with its probability as confidence.
fn read_answer(answer: &Value, candidates: &BTreeSet<&str>) -> Result<Choice, String> {
    let question = answer
        .get("answers")
        .and_then(|answers| answers.get(QUESTION))
        .ok_or_else(|| format!("the answer has no `answers.{QUESTION}`"))?;
    let action = question
        .get("choice")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`answers.{QUESTION}.choice` is missing or not a string"))?;
    if !candidates.contains(action) {
        return Err(format!(
            "it chose `{action}`, which is not one of the {} candidates",
            candidates.len()
        ));
    }
    let probability = question
        .get("answer_confidence")
        .and_then(Value::as_number)
        .ok_or_else(|| {
            format!("`answers.{QUESTION}.answer_confidence` is missing or not a number")
        })?;
    let in_range = probability
        .as_f64()
        .is_some_and(|value| (0.0..=1.0).contains(&value));
    let rendered = probability.to_string();
    let confidence = if in_range {
        plain_decimal(&rendered)
    } else {
        None
    }
    .ok_or_else(|| format!("the probability {rendered} is not within [0, 1]"))?;
    Ok(Choice {
        action: action.to_owned(),
        confidence: Some(Decimal(confidence)),
    })
}

/// `number`, a JSON number's rendering, as a plain decimal string (`1.5e-5` is `0.000015`), or
/// `None` when it is negative and not zero or its exponent is past [`MAX_EXPONENT`].
fn plain_decimal(number: &str) -> Option<String> {
    let (negative, unsigned) = match number.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, number),
    };
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (unsigned, 0),
    };
    if exponent.abs() > MAX_EXPONENT {
        return None;
    }
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if negative {
        // Only a negative zero is within [0, 1]; it is zero.
        return whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b == b'0')
            .then(|| "0".to_owned());
    }
    let digits = format!("{whole}{fraction}");
    let point = i64::try_from(whole.len()).ok()? + exponent;
    let length = i64::try_from(digits.len()).ok()?;
    Some(if point <= 0 {
        format!("0.{}{digits}", "0".repeat(usize::try_from(-point).ok()?))
    } else if point >= length {
        format!(
            "{digits}{}",
            "0".repeat(usize::try_from(point - length).ok()?)
        )
    } else {
        let point = usize::try_from(point).ok()?;
        format!("{}.{}", &digits[..point], &digits[point..])
    })
}

#[cfg(test)]
mod tests {
    use super::plain_decimal;

    #[test]
    fn a_probability_renders_as_a_plain_decimal() {
        for (number, plain) in [
            ("0.97", Some("0.97")),
            ("1", Some("1")),
            ("0", Some("0")),
            ("1.0", Some("1.0")),
            ("1.5e-5", Some("0.000015")),
            ("9.7E-1", Some("0.97")),
            ("5e-1", Some("0.5")),
            ("-0.0", Some("0")),
            ("-0.5", None),
            ("1e-401", None),
            ("abc", None),
            (".5", None),
        ] {
            assert_eq!(plain_decimal(number).as_deref(), plain, "{number}");
        }
    }
}
