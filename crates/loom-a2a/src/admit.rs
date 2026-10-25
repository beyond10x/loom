//! Whether the listener answers a request: `loom.agent.AdmitA2aRequest`
//! (`ess/domains/agent.yaml`), decided on the request line and its headers before the body is
//! read, and the [`Bearer`] it compares against.

use std::fmt;

pub use loom::agent::{A2aRefusalReason, A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome};

/// Decides `loom.agent.AdmitA2aRequest`: the first refusal in declaration order (a request that is
/// no probe and carries no bearer, then one whose bearer is not the agent's), then `probe`, else
/// `admitted`.
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
    /// The bearer `token`.
    ///
    /// # Errors
    /// `token` is empty or blank, so any request presenting an empty bearer would match it.
    pub fn new(token: &str) -> Result<Self, String> {
        if token.trim().is_empty() {
            return Err("the A2A bearer is empty".to_owned());
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

/// The decision for one request: `probe` whether it is `GET /ready` or `GET /live`,
/// `authorization` the value of its `Authorization` header, when it carries one. A header that is
/// not `Bearer <token>` (another scheme, or no token) presents no bearer.
pub fn admit(bearer: &Bearer, probe: bool, authorization: Option<&str>) -> AdmitA2aRequestOutcome {
    let presented = authorization.and_then(|value| {
        let (scheme, token) = value.trim().split_once(' ')?;
        scheme
            .eq_ignore_ascii_case("bearer")
            .then(|| token.trim())
            .filter(|token| !token.is_empty())
    });
    admit_a2a_request(&AdmitA2aRequest {
        probe,
        bearer_presented: presented.is_some(),
        bearer_matches: presented.is_some_and(|token| bearer.matches(token.as_bytes())),
    })
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
