//! Adversary pass 2 for `story:authority-provider-port`: what Commission's refusal carries when a
//! provider fails. `check_authority` hands the provider's error text through as
//! `AuthorityCheck::Refused`, unchanged, and `AuthorityProviderError`'s `Display` renders it
//! verbatim. These cases drive that pass-through against:
//!
//! * the specification's promise that the authority context is "opaque to Commission: passed to the
//!   AuthorityProvider unchanged and read by nobody else" (`ess/domains/responsibility.yaml`,
//!   the `AuthorityContext` comment) — a provider that echoes its request into its error puts the
//!   context into Commission's refusal, where every reader of the refusal reads it;
//! * a log line: the `Display` of a refusal is one line, so a provider cannot forge a second one;
//! * a bound: the `Display` of a refusal does not grow with whatever the provider returned.
//!
//! The 4096-byte bound is this file's choice, not a documented one.

use b10x_commission::model::json;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::commission_state::Assigned;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId, Commission, CommissionData,
    CommissionId, PrincipalId,
};
use b10x_commission::ports::authority::{
    AuthorityCheck, AuthorityProvider, AuthorityProviderError, check_authority,
};
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;

/// A value only the authority context holds.
const DELEGATED_SECRET: &str = "delegation-token-9f41c2";

fn commission() -> Commission<Assigned> {
    let context = json::parse(&format!(
        r#"{{"delegation":{{"token":"{DELEGATED_SECRET}","scopes":["ledger"]}}}}"#
    ))
    .unwrap_or_else(|error| panic!("authority context fixture is not JSON: {error:?}"));
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("3e4f5a6b-7c8d-4e9f-8a0b-1c2d3e4f5a6b".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("9a8b7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d".to_owned())),
        case_id: CaseId("case-adversary-2".to_owned()),
        principal: PrincipalId("principal-c".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

/// A provider whose backend rejects the request and whose error quotes the request it sent, as an
/// HTTP-backed adapter that wraps a 4xx body commonly does.
struct EchoesItsRequest;

impl AuthorityProvider for EchoesItsRequest {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let mut request = String::new();
        json::push_value(&mut request, &commission.authority_context.0);
        Err(AuthorityProviderError::new(format!(
            "backend rejected {capability} for request {request}"
        )))
    }
}

/// The refusal Commission returns must not carry the authority context, which the specification
/// says nobody but the provider reads.
#[test]
fn refusal_does_not_carry_the_authority_context() {
    let commission = commission();
    let refusal = check_authority(&EchoesItsRequest, &commission, "ledger.write");
    assert!(!refusal.allows(), "a provider failure must never allow");

    let shown = match &refusal {
        AuthorityCheck::Refused(error) => error.to_string(),
        other => panic!("a provider failure must be refused, got {other:?}"),
    };
    let logged = format!("{refusal:?}");
    assert!(
        !shown.contains(DELEGATED_SECRET) && !logged.contains(DELEGATED_SECRET),
        "Commission's refusal carries the opaque authority context to every reader of it: \
         Display `{shown}`, Debug `{logged}`"
    );
}

/// A refusal renders as one line: a provider's text cannot start a second, forged log line.
#[test]
fn refusal_display_is_one_line() {
    let commission = commission();
    let fake = StaticAuthorityProvider::new().fail(
        "payment.send",
        "timeout\nauthority allowed payment.send for principal-c",
    );
    let refusal = check_authority(&fake, &commission, "payment.send");
    let AuthorityCheck::Refused(error) = &refusal else {
        panic!("a provider failure must be refused, got {refusal:?}");
    };
    let shown = error.to_string();
    assert_eq!(
        shown.lines().count(),
        1,
        "a refusal's Display must be one line, got {shown:?}"
    );
}

/// A refusal's rendering is bounded whatever the provider returned.
#[test]
fn refusal_display_is_bounded() {
    let commission = commission();
    let huge = "x".repeat(1 << 20);
    let fake = StaticAuthorityProvider::new().fail("payment.send", huge);
    let refusal = check_authority(&fake, &commission, "payment.send");
    let AuthorityCheck::Refused(error) = &refusal else {
        panic!("a provider failure must be refused, got a decision");
    };
    let shown = error.to_string();
    assert!(
        shown.len() <= 4096,
        "a refusal's Display must be bounded, got {} bytes from a {}-byte provider message",
        shown.len(),
        1 << 20
    );
}
