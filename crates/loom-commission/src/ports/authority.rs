//! The `AuthorityProvider` port: the authority verdict at the call.
//!
//! Authority stays outside the model. Commission asks the provider at the moment of the call,
//! passing the commission and the capability, and the provider answers with the generated
//! [`AuthorityVerdict`]. The commission's [`AuthorityContext`](crate::model::responsibility::AuthorityContext)
//! is opaque to Commission: it reaches the provider unchanged and the provider is its only reader.
//!
//! A provider that fails does not decide. [`check_authority`] turns the failure into
//! [`AuthorityCheck::Refused`], which never allows: authority fails toward less authority.
//!
//! The verdict is returned at the call and not stored; a stored `AuthorityDecision` belongs to one
//! action request at one case revision and is written elsewhere.

use crate::model::json;
use crate::model::responsibility::{
    AuthorityContext, AuthorityVerdict, Commission, CommissionData, Unit, commission_state,
};
use std::fmt;

/// Decides whether a commission may use a capability, at the moment of the call.
pub trait AuthorityProvider {
    /// The verdict for `capability` under the commission's principal and authority context.
    ///
    /// An `Err` is a failure to decide, never a decision; Commission refuses on it.
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError>;
}

/// Why a provider could not decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityProviderError {
    /// What the provider reported.
    pub message: String,
}

impl AuthorityProviderError {
    /// A failure carrying the provider's own message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// What the `Display` of an [`AuthorityProviderError`] puts before the provider's message.
pub const REFUSAL_PREFIX: &str = "authority provider failed: ";

/// The most bytes the `Display` of a refusal from [`check_authority`] renders.
pub const REFUSAL_MAX_BYTES: usize = 4096;

/// What ends a provider message [`check_authority`] cut to fit [`REFUSAL_MAX_BYTES`].
pub const TRUNCATED: &str = "…";

/// What replaces the commission's authority context in a refusal.
pub const AUTHORITY_CONTEXT_REDACTED: &str = "<authority context>";

impl fmt::Display for AuthorityProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{REFUSAL_PREFIX}{}", self.message)
    }
}

impl std::error::Error for AuthorityProviderError {}

/// The answer to Commission's authority check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityCheck {
    /// The provider decided; its verdict, unchanged.
    Decided(AuthorityVerdict),
    /// The provider failed to decide, so Commission refuses. Never an allow.
    Refused(AuthorityProviderError),
}

impl AuthorityCheck {
    /// Whether the capability may be used now: only a provider's explicit allow.
    ///
    /// `Unit` is declared always true, so `Allow(Unit(false))` cannot occur; if it does, it does not
    /// allow.
    pub fn allows(&self) -> bool {
        matches!(self, Self::Decided(AuthorityVerdict::Allow(Unit(true))))
    }
}

/// Asks `provider` whether `commission` may use `capability`, at the call.
///
/// The commission is passed as it holds itself; Commission does not read its authority context.
///
/// A provider failure becomes [`AuthorityCheck::Refused`], carrying the provider's message after
/// three rules, applied in this order:
///
/// 1. every occurrence of the commission's authority context, serialized as compact JSON
///    ([`json::push_value`]), is replaced with [`AUTHORITY_CONTEXT_REDACTED`], so a provider that
///    echoes its request does not hand the opaque context to every reader of the refusal;
/// 2. control characters (newlines included) and the Unicode line and paragraph separators are
///    escaped as `\n`, `\r`, `\t` or `\u{…}`, so the refusal renders as one line;
/// 3. the message is cut on a char boundary and ended with [`TRUNCATED`] so that the refusal's
///    `Display`, [`REFUSAL_PREFIX`] included, is at most [`REFUSAL_MAX_BYTES`].
///
/// Rule 1 removes the context only in that exact form. A provider that quotes part of the context,
/// or renders it another way, is not caught by it.
pub fn check_authority<P, S>(
    provider: &P,
    commission: &Commission<S>,
    capability: &str,
) -> AuthorityCheck
where
    P: AuthorityProvider + ?Sized,
    S: commission_state::Marker,
{
    match provider.decide(commission.data(), capability) {
        Ok(verdict) => AuthorityCheck::Decided(verdict),
        Err(error) => AuthorityCheck::Refused(AuthorityProviderError {
            message: sanitize(&error.message, &commission.data().authority_context),
        }),
    }
}

/// The provider's failure text as a refusal may carry it: see [`check_authority`].
fn sanitize(message: &str, context: &AuthorityContext) -> String {
    let mut serialized = String::new();
    json::push_value(&mut serialized, &context.0);
    let redacted = message.replace(&serialized, AUTHORITY_CONTEXT_REDACTED);

    let mut line = String::with_capacity(redacted.len());
    for c in redacted.chars() {
        if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
            line.extend(c.escape_default());
        } else {
            line.push(c);
        }
    }

    let budget = REFUSAL_MAX_BYTES - REFUSAL_PREFIX.len();
    if line.len() > budget {
        let cut = line.floor_char_boundary(budget - TRUNCATED.len());
        line.truncate(cut);
        line.push_str(TRUNCATED);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorityCheck, AuthorityProvider, AuthorityProviderError, REFUSAL_MAX_BYTES,
        REFUSAL_PREFIX, TRUNCATED, check_authority,
    };
    use crate::model::json;
    use crate::model::primitives::Uuid;
    use crate::model::responsibility::{
        AgentRevisionId, AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired,
        AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, PrincipalId, Unit,
        commission_state,
    };

    /// `allows()` is true for exactly one value: `Decided(Allow(Unit(true)))`. `Unit` is declared
    /// always true, so `Allow(Unit(false))` cannot occur; if it does, it fails closed.
    #[test]
    fn allows_only_an_explicit_true_allow() {
        assert!(AuthorityCheck::Decided(AuthorityVerdict::Allow(Unit(true))).allows());

        let not_allowing = [
            AuthorityCheck::Decided(AuthorityVerdict::Allow(Unit(false))),
            AuthorityCheck::Decided(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: "r".to_owned(),
            })),
            AuthorityCheck::Decided(AuthorityVerdict::ApprovalRequired(
                AuthorityVerdictApprovalRequired {
                    request: "q".to_owned(),
                },
            )),
            AuthorityCheck::Refused(AuthorityProviderError::new("down")),
        ];
        for check in not_allowing {
            assert!(!check.allows(), "{check:?} must not allow");
        }
    }

    /// A provider that fails with a scripted message.
    struct Fails(String);

    impl AuthorityProvider for Fails {
        fn decide(
            &self,
            _commission: &CommissionData,
            _capability: &str,
        ) -> Result<AuthorityVerdict, AuthorityProviderError> {
            Err(AuthorityProviderError::new(self.0.clone()))
        }
    }

    fn commission() -> Commission<commission_state::Assigned> {
        let context = json::parse(r#"{"delegation":{"token":"tok-7d1e","scopes":["ledger"]}}"#)
            .unwrap_or_else(|error| panic!("authority context fixture is not JSON: {error:?}"));
        Commission::new(CommissionData {
            commission_id: CommissionId(Uuid("6f5e4d3c-2b1a-4f9e-8d7c-6b5a4f3e2d1c".to_owned())),
            agent_revision_id: AgentRevisionId(Uuid(
                "2c3d4e5f-6a7b-4c8d-9e0f-1a2b3c4d5e6f".to_owned(),
            )),
            case_id: CaseId("case-unit".to_owned()),
            principal: PrincipalId("principal-u".to_owned()),
            authority_context: AuthorityContext(context),
        })
    }

    fn refused_message(provider_message: String) -> String {
        match check_authority(&Fails(provider_message), &commission(), "ledger.write") {
            AuthorityCheck::Refused(error) => error.message,
            other => panic!("a provider failure must be refused, got {other:?}"),
        }
    }

    /// Rule (a): every occurrence of the serialized authority context becomes
    /// `<authority context>`; the rest of the provider's text stays.
    #[test]
    fn refusal_redacts_every_serialized_authority_context() {
        let mut context = String::new();
        json::push_value(&mut context, &commission().data().authority_context.0);
        let message = refused_message(format!("rejected {context}; retried {context} again"));
        assert_eq!(
            message,
            "rejected <authority context>; retried <authority context> again"
        );
        assert!(!message.contains("tok-7d1e"), "context leaked: {message}");
    }

    /// Rule (b): control characters, newlines included, are escaped, so the refusal is one line.
    /// Other text, non-ASCII included, is kept as it was.
    #[test]
    fn refusal_escapes_control_characters_to_one_line() {
        let message = refused_message("a\nb\r\nc\td\u{7}e\u{85}f\u{2028}g\u{2029}h é".to_owned());
        assert_eq!(message, r"a\nb\r\nc\td\u{7}e\u{85}f\u{2028}g\u{2029}h é");
        let shown = AuthorityProviderError::new(message).to_string();
        assert_eq!(shown.lines().count(), 1, "not one line: {shown:?}");
    }

    /// Rule (c): the rendered refusal is at most `REFUSAL_MAX_BYTES`, cut on a char boundary and
    /// marked with `…`; a message that fits is not cut.
    #[test]
    fn refusal_is_cut_to_the_bound_on_a_char_boundary() {
        // Two-byte characters, so a cut at an odd byte would split one.
        let long = "é".repeat(REFUSAL_MAX_BYTES);
        let message = refused_message(long.clone());
        let shown = AuthorityProviderError::new(message.clone()).to_string();
        assert!(
            shown.len() <= REFUSAL_MAX_BYTES,
            "refusal is {} bytes, bound {REFUSAL_MAX_BYTES}",
            shown.len()
        );
        let kept = message
            .strip_suffix(TRUNCATED)
            .unwrap_or_else(|| panic!("a cut message must end with `{TRUNCATED}`"));
        assert!(long.starts_with(kept), "the cut must keep a prefix");
        assert!(
            shown.len() + "é".len() > REFUSAL_MAX_BYTES,
            "the cut must keep as much as fits, got {} bytes",
            shown.len()
        );

        let fits = "x".repeat(REFUSAL_MAX_BYTES - REFUSAL_PREFIX.len());
        assert_eq!(
            refused_message(fits.clone()),
            fits,
            "a message that fits is kept whole"
        );
    }
}
