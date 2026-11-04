//! Whether the listener answers a request, and how: `loom.agent.AdmitA2aRequest`
//! (`ess/domains/agent.yaml`), and the [`Bearer`] it compares against. No refusal and no probe
//! depends on whether the request asks for a stream, so the listener decides them on the request
//! line and its headers, before the body is read ([`admit`]); once an admitted request's body
//! names its method, [`Presented::decide`] answers `streamed` or `admitted`.

use std::fmt;

pub use loom::agent::{A2aRefusalReason, A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome};

/// Decides `loom.agent.AdmitA2aRequest`: the first refusal in declaration order (a request that is
/// no probe and carries no bearer, then one whose bearer is not the agent's), then `probe`, then
/// `streamed` for `SendStreamingMessage`, else `admitted`.
pub fn admit_a2a_request(input: &AdmitA2aRequest) -> AdmitA2aRequestOutcome {
    let refused = |reason| A2aRefused { reason };
    if !input.probe && !input.bearer_presented {
        return AdmitA2aRequestOutcome::MissingBearer {
            error: refused(A2aRefusalReason::MissingBearer),
        };
    }
    if !input.probe && !input.bearer_matches {
        return AdmitA2aRequestOutcome::WrongBearer {
            error: refused(A2aRefusalReason::WrongBearer),
        };
    }
    if input.probe {
        return AdmitA2aRequestOutcome::Probe;
    }
    if input.stream {
        return AdmitA2aRequestOutcome::Streamed;
    }
    AdmitA2aRequestOutcome::Admitted
}

/// `reason` as the enum `loom.agent.A2aRefusalReason` spells it.
pub fn a2a_refusal_spelling(reason: A2aRefusalReason) -> &'static str {
    match reason {
        A2aRefusalReason::MissingBearer => "MissingBearer",
        A2aRefusalReason::WrongBearer => "WrongBearer",
    }
}

/// The token every request but a probe must carry as `Authorization: Bearer <token>`: the one the
/// agent's Secret gives it. It is never printed: `Debug` names no byte of it.
#[derive(Clone)]
pub struct Bearer(Vec<u8>);

impl Bearer {
    /// The bearer `token`, without the whitespace around it: a Secret written from a file ends
    /// in a newline, and a header value is read trimmed, so no request could present it.
    ///
    /// # Errors
    /// `token` is empty or blank, so any request presenting an empty bearer would match it; or
    /// it holds a control character inside, which no header can carry, so no request could
    /// present it either. The message names no byte of it.
    pub fn new(token: &str) -> Result<Self, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("the A2A bearer is empty".to_owned());
        }
        if token.chars().any(char::is_control) {
            return Err("the A2A bearer holds a control character".to_owned());
        }
        Ok(Self(token.as_bytes().to_vec()))
    }

    /// Whether `presented` is this token, compared in time independent of where they differ.
    pub fn matches(&self, presented: &[u8]) -> bool {
        let mut difference = self.0.len() ^ presented.len();
        for (index, byte) in self.0.iter().enumerate() {
            let other = presented.get(index).copied().unwrap_or(0);
            difference |= usize::from(byte ^ other);
        }
        difference == 0
    }
}

impl fmt::Debug for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Bearer(..)")
    }
}

/// What the request line and headers of one request present to [`admit_a2a_request`]: whether it
/// is a probe and what bearer it carries. The method its body names is decided later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented {
    probe: bool,
    bearer_presented: bool,
    bearer_matches: bool,
}

impl Presented {
    /// `probe` whether the request is `GET /ready` or `GET /live`, `authorization` the value of
    /// its `Authorization` header, when it carries one. A header that is not `Bearer <token>`
    /// (another scheme, or no token) presents no bearer.
    pub fn new(bearer: &Bearer, probe: bool, authorization: Option<&str>) -> Self {
        let presented = authorization.and_then(|value| {
            let (scheme, token) = value.trim().split_once(' ')?;
            scheme
                .eq_ignore_ascii_case("bearer")
                .then(|| token.trim())
                .filter(|token| !token.is_empty())
        });
        Self {
            probe,
            bearer_presented: presented.is_some(),
            bearer_matches: presented.is_some_and(|token| bearer.matches(token.as_bytes())),
        }
    }

    /// The decision for the request, `stream` whether its method is `SendStreamingMessage`.
    pub fn decide(self, stream: bool) -> AdmitA2aRequestOutcome {
        admit_a2a_request(&AdmitA2aRequest {
            probe: self.probe,
            bearer_presented: self.bearer_presented,
            bearer_matches: self.bearer_matches,
            stream,
        })
    }
}

/// The decision for one request on its request line and headers, before its body is read (see
/// [`Presented::new`]): a refusal, `probe`, or `admitted` for a request whose method is not known
/// yet.
pub fn admit(bearer: &Bearer, probe: bool, authorization: Option<&str>) -> AdmitA2aRequestOutcome {
    Presented::new(bearer, probe, authorization).decide(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bearer() -> Bearer {
        Bearer::new("token-1").expect("a bearer")
    }

    #[test]
    fn only_the_exact_token_matches() {
        let bearer = bearer();
        assert!(bearer.matches(b"token-1"));
        for other in [&b"token-2"[..], b"token-", b"token-11", b"", b"TOKEN-1"] {
            assert!(!bearer.matches(other), "{other:?}");
        }
    }

    #[test]
    fn the_whitespace_around_a_bearer_is_dropped_and_a_control_character_inside_refused() {
        for stored in ["token-1\n", "token-1\r\n", " token-1\t"] {
            let bearer = Bearer::new(stored).expect("a bearer");
            assert!(bearer.matches(b"token-1"), "{stored:?}");
            assert_eq!(
                admit(&bearer, false, Some("Bearer token-1")),
                AdmitA2aRequestOutcome::Admitted,
                "{stored:?}"
            );
        }
        for refused in ["\n", " \r\n ", "tok\nen-1", "token\u{7}-1"] {
            let why = Bearer::new(refused).expect_err("refused");
            assert!(!why.contains("token") && !why.contains("tok"), "{why}");
        }
    }

    #[test]
    fn a_header_of_another_scheme_presents_no_bearer() {
        let bearer = bearer();
        for header in [
            "Basic token-1",
            "token-1",
            "Bearer",
            "Bearer  ",
            "Bearertoken-1",
        ] {
            assert_eq!(
                admit(&bearer, false, Some(header)),
                AdmitA2aRequestOutcome::MissingBearer {
                    error: A2aRefused {
                        reason: A2aRefusalReason::MissingBearer
                    }
                },
                "{header:?}"
            );
        }
        assert_eq!(
            admit(&bearer, false, Some("bearer token-1")),
            AdmitA2aRequestOutcome::Admitted
        );
        assert_eq!(
            admit(&bearer, false, Some("Bearer token-2")),
            AdmitA2aRequestOutcome::WrongBearer {
                error: A2aRefused {
                    reason: A2aRefusalReason::WrongBearer
                }
            }
        );
    }

    /// The listener refuses on the head, before it knows the method; that is sound only while no
    /// refusal and no probe depends on `stream`, for every input.
    #[test]
    fn no_refusal_and_no_probe_depends_on_the_stream() {
        for probe in [false, true] {
            for bearer_presented in [false, true] {
                for bearer_matches in [false, true] {
                    let decide = |stream| {
                        admit_a2a_request(&AdmitA2aRequest {
                            probe,
                            bearer_presented,
                            bearer_matches,
                            stream,
                        })
                    };
                    let (head, streamed) = (decide(false), decide(true));
                    match head {
                        AdmitA2aRequestOutcome::Admitted => {
                            assert_eq!(streamed, AdmitA2aRequestOutcome::Streamed);
                        }
                        other => assert_eq!(streamed, other),
                    }
                }
            }
        }
    }

    #[test]
    fn a_probe_is_answered_whatever_it_carries() {
        let bearer = bearer();
        for header in [None, Some("Bearer wrong"), Some("Bearer token-1")] {
            assert_eq!(
                admit(&bearer, true, header),
                AdmitA2aRequestOutcome::Probe,
                "{header:?}"
            );
        }
    }
}
