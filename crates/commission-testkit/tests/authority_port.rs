//! Acceptance for `story:authority-provider-port`: Commission's authority check asks the
//! `AuthorityProvider` at the call, returns the provider's verdict unchanged, and turns a provider
//! failure into a refusal, never an allow. The static fake records what it was asked.

use b10x_commission::model::json;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::commission_state::Assigned;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, PrincipalId, Unit,
};
use b10x_commission::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use b10x_commission_testkit::fake_authority::{AuthorityQuery, StaticAuthorityProvider};

const ALLOWED: &str = "ledger.read";
const DENIED: &str = "ledger.write";
const NEEDS_APPROVAL: &str = "payment.send";
const FAILING: &str = "account.close";

const REASON: &str = "outside the delegated budget";
const REQUEST: &str = "approve a payment of 120.00 to supplier 7";
const FAILURE: &str = "authority backend unreachable";

/// A commission whose principal and authority context the provider must see exactly as held.
///
/// The context is a nested document with an array and a decimal kept in its arrival spelling, so a
/// provider handed a re-rendered or partially read copy would not compare equal.
fn commission() -> Commission<Assigned> {
    let context = json::parse(
        r#"{"delegation":{"budget":"500.00","scopes":["ledger","payment"]},"tenant":"tenant-a","limits":[1,2.50,null,true]}"#,
    )
    .unwrap_or_else(|error| panic!("authority context fixture is not JSON: {error:?}"));
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("0b6f3c1e-2a4d-4e8f-9a1b-3c5d7e9f1a2b".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("5d2e8f4a-7b1c-4d3e-8f6a-9b0c1d2e3f4a".to_owned())),
        case_id: CaseId("case-17".to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

fn provider() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new()
        .answer(ALLOWED, AuthorityVerdict::Allow(Unit(true)))
        .answer(
            DENIED,
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: REASON.to_owned(),
            }),
        )
        .answer(
            NEEDS_APPROVAL,
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: REQUEST.to_owned(),
            }),
        )
        .fail(FAILING, FAILURE)
}

#[test]
fn authority_port_contract() {
    let commission = commission();
    let fake = provider();
    // The fake is used through the port, as an adapter would be.
    let provider: &dyn AuthorityProvider = &fake;

    // Expectation 1: a capability the fake allows returns allow.
    let allowed = check_authority(provider, &commission, ALLOWED);
    assert_eq!(
        allowed,
        AuthorityCheck::Decided(AuthorityVerdict::Allow(Unit(true))),
        "an allowed capability must return allow"
    );
    assert!(allowed.allows(), "allow must read as allowing");

    // Expectation 2: a capability the fake denies with reason R returns deny carrying R.
    let denied = check_authority(provider, &commission, DENIED);
    assert_eq!(
        denied,
        AuthorityCheck::Decided(AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: REASON.to_owned(),
        })),
        "a denied capability must return deny carrying the provider's reason"
    );
    assert!(!denied.allows(), "deny must not read as allowing");

    // Expectation 3: approval required with request Q returns approval required carrying Q.
    let approval = check_authority(provider, &commission, NEEDS_APPROVAL);
    assert_eq!(
        approval,
        AuthorityCheck::Decided(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: REQUEST.to_owned(),
            }
        )),
        "approval required must carry the provider's request"
    );
    assert!(
        !approval.allows(),
        "approval required must not read as allowing"
    );

    // Expectation 4: a provider error returns a refusal, not an allow.
    let failed = check_authority(provider, &commission, FAILING);
    match &failed {
        AuthorityCheck::Refused(error) => assert!(
            error.to_string().contains(FAILURE),
            "the refusal must carry the provider's failure, got `{error}`"
        ),
        other => panic!("a provider failure must be refused, got {other:?}"),
    }
    assert!(
        !failed.allows(),
        "a provider failure must never read as allowing"
    );

    // Expectation 5: every call recorded the commission's principal and authority context exactly
    // as the commission holds them, with the capability asked about, in call order.
    let held = commission.data();
    let expected: Vec<AuthorityQuery> = [ALLOWED, DENIED, NEEDS_APPROVAL, FAILING]
        .into_iter()
        .map(|capability| AuthorityQuery {
            principal: held.principal.clone(),
            authority_context: held.authority_context.clone(),
            capability: capability.to_owned(),
        })
        .collect();
    assert_eq!(
        fake.asked(),
        expected,
        "the fake must record one query per call, carrying the commission's principal and \
         authority context unchanged"
    );
}
