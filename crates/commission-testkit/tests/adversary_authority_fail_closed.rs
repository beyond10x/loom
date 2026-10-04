//! Adversary cases for `story:authority-provider-port`: the paths `authority_port_contract` does not
//! drive. An unknown capability, a capability spelled with case and whitespace, a table entry
//! replaced, a panicking provider and a provider shared across threads must all fail toward less
//! authority, pass the capability exactly, and leave a record of every call.

use b10x_commission::model::json;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::commission_state::Assigned;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, PrincipalId, Unit,
};
use b10x_commission::ports::authority::{
    AuthorityCheck, AuthorityProvider, AuthorityProviderError, check_authority,
};
use b10x_commission_testkit::fake_authority::{AuthorityQuery, StaticAuthorityProvider};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

fn commission() -> Commission<Assigned> {
    let context = json::parse(r#"{"scopes":["ledger"],"limit":"10.50"}"#)
        .unwrap_or_else(|error| panic!("authority context fixture is not JSON: {error:?}"));
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("7c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d".to_owned())),
        case_id: CaseId("case-adversary".to_owned()),
        principal: PrincipalId("principal-b".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

fn query(commission: &Commission<Assigned>, capability: &str) -> AuthorityQuery {
    AuthorityQuery {
        principal: commission.data().principal.clone(),
        authority_context: commission.data().authority_context.clone(),
        capability: capability.to_owned(),
    }
}

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

/// A capability missing from the table is a failure: refused, never allowed, and still recorded.
/// The empty capability and the empty table are the boundaries.
#[test]
fn unknown_capability_is_refused_and_recorded() {
    let commission = commission();
    let fake = StaticAuthorityProvider::new().answer("ledger.read", allow());

    let unknown = check_authority(&fake, &commission, "ledger.delete");
    assert!(
        matches!(unknown, AuthorityCheck::Refused(_)),
        "a capability missing from the table must be refused, got {unknown:?}"
    );
    assert!(!unknown.allows(), "an unknown capability must never allow");

    let empty = check_authority(&fake, &commission, "");
    assert!(
        matches!(empty, AuthorityCheck::Refused(_)),
        "the empty capability is not in the table and must be refused, got {empty:?}"
    );

    let bare = StaticAuthorityProvider::new();
    let nothing = check_authority(&bare, &commission, "ledger.read");
    assert!(
        matches!(nothing, AuthorityCheck::Refused(_)),
        "an empty table must refuse every capability, got {nothing:?}"
    );

    assert_eq!(
        fake.asked(),
        vec![query(&commission, "ledger.delete"), query(&commission, "")],
        "a failed call for an unknown capability must be recorded like any other"
    );
    assert_eq!(bare.asked(), vec![query(&commission, "ledger.read")]);
}

/// The capability reaches the provider exactly as the caller spelled it: no trimming, no case
/// folding, in the check or in the fake's lookup and record.
#[test]
fn capability_is_passed_and_matched_exactly() {
    let commission = commission();
    let spelled = " Payment.Send ";
    let fake = StaticAuthorityProvider::new().answer(spelled, allow());

    let exact = check_authority(&fake, &commission, spelled);
    assert_eq!(exact, AuthorityCheck::Decided(allow()));

    for near in ["Payment.Send", "payment.send", " payment.send "] {
        let other = check_authority(&fake, &commission, near);
        assert!(
            matches!(other, AuthorityCheck::Refused(_)),
            "`{near}` is not the scripted `{spelled}` and must be refused, got {other:?}"
        );
    }

    assert_eq!(
        fake.asked(),
        vec![
            query(&commission, spelled),
            query(&commission, "Payment.Send"),
            query(&commission, "payment.send"),
            query(&commission, " payment.send "),
        ],
        "the fake must record each capability exactly as asked"
    );
}

/// The fake's builder replaces an earlier entry, as its documentation says, in both directions.
#[test]
fn later_table_entry_replaces_earlier_one() {
    let commission = commission();

    let allow_then_fail = StaticAuthorityProvider::new()
        .answer("account.close", allow())
        .fail("account.close", "revoked");
    let failed = check_authority(&allow_then_fail, &commission, "account.close");
    assert_eq!(
        failed,
        AuthorityCheck::Refused(AuthorityProviderError::new("revoked")),
        "a later failure must replace an earlier allow"
    );

    let deny_then_approval = StaticAuthorityProvider::new()
        .answer(
            "account.close",
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: "first".to_owned(),
            }),
        )
        .answer(
            "account.close",
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: "second".to_owned(),
            }),
        );
    assert_eq!(
        check_authority(&deny_then_approval, &commission, "account.close"),
        AuthorityCheck::Decided(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: "second".to_owned(),
            }
        )),
        "a later verdict must replace an earlier one"
    );
}

/// A provider that panics yields no verdict at all: the panic leaves the check, nothing is
/// turned into an allow.
#[test]
fn panicking_provider_yields_no_verdict() {
    struct Panics;
    impl AuthorityProvider for Panics {
        fn decide(
            &self,
            _commission: &CommissionData,
            _capability: &str,
        ) -> Result<AuthorityVerdict, AuthorityProviderError> {
            panic!("provider crashed")
        }
    }

    let commission = commission();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        check_authority(&Panics, &commission, "ledger.read")
    }));
    assert!(
        outcome.is_err(),
        "a panicking provider must not produce a verdict, got {outcome:?}"
    );
}

/// The port is usable as a shared trait object across threads, as a runtime loop will hold it, and
/// the fake records every call made through it.
#[test]
fn shared_provider_records_every_call_across_threads() {
    let commission = Arc::new(commission());
    let provider: Arc<dyn AuthorityProvider + Send + Sync> = Arc::new(
        StaticAuthorityProvider::new()
            .answer("ledger.read", allow())
            .fail("ledger.write", "unreachable"),
    );
    let fake = Arc::new(StaticAuthorityProvider::new().answer("ledger.read", allow()));

    let handles: Vec<_> = (0..8)
        .map(|index| {
            let provider = Arc::clone(&provider);
            let fake = Arc::clone(&fake);
            let commission = Arc::clone(&commission);
            std::thread::spawn(move || {
                let capability = if index % 2 == 0 {
                    "ledger.read"
                } else {
                    "ledger.write"
                };
                let through_port = check_authority(&*provider, &commission, capability);
                let through_fake = check_authority(&*fake, &commission, capability);
                (capability, through_port, through_fake)
            })
        })
        .collect();

    for handle in handles {
        let (capability, through_port, through_fake) = handle
            .join()
            .unwrap_or_else(|_| panic!("authority thread panicked"));
        assert_eq!(
            through_port.allows(),
            capability == "ledger.read",
            "{capability}"
        );
        assert_eq!(
            through_fake.allows(),
            capability == "ledger.read",
            "{capability}"
        );
    }

    let asked = fake.asked();
    assert_eq!(asked.len(), 8, "every call must be recorded, got {asked:?}");
    assert_eq!(
        asked
            .iter()
            .filter(|query| query.capability == "ledger.write")
            .count(),
        4,
        "the four failed calls must be recorded too"
    );
}
