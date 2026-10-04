//! Adversary cases for the authority conformance kit (`kits::authority`).
//!
//! Each case runs the kit against a provider over the kit's `Backing` with exactly one defect. The
//! proof cases show a check catches a plain mutant; the hole cases are defects the kit's own
//! documentation says it catches, and does not.

use std::panic::{AssertUnwindSafe, catch_unwind};

use b10x_commission::model::responsibility::{
    AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, CommissionData,
};
use b10x_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_commission_testkit::kits::authority::{self, AuthorityFixture, Backing, Check};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Defect {
    /// An allow comes back as approval-required.
    AllowBecomesApproval,
    /// A deny comes back with its reason rewritten.
    DenyReasonRewritten,
    /// An approval-required comes back as a deny.
    ApprovalBecomesDeny,
    /// The capability asked about is ignored: the first entry the backing holds answers it.
    IgnoresCapability,
    /// A failing backing call panics, as `.expect()` adapter code does.
    PanicsOnBackingFailure,
}

struct Mutant {
    backing: Backing,
    defect: Defect,
}

impl AuthorityProvider for Mutant {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let answer = match self.defect {
            Defect::IgnoresCapability => self
                .backing
                .entries()
                .into_iter()
                .next()
                .map(|(_, answer)| answer)
                .unwrap_or_else(|| self.backing.call(capability)),
            Defect::PanicsOnBackingFailure => Ok(self
                .backing
                .call(capability)
                .unwrap_or_else(|error| panic!("backing call failed: {error}"))),
            _ => self.backing.call(capability),
        };
        let verdict = answer.map_err(|error| AuthorityProviderError::new(error.message))?;
        Ok(match (self.defect, verdict) {
            (Defect::AllowBecomesApproval, AuthorityVerdict::Allow(_)) => {
                AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                    request: "approve".to_owned(),
                })
            }
            (Defect::DenyReasonRewritten, AuthorityVerdict::Deny(_)) => {
                AuthorityVerdict::Deny(AuthorityVerdictDeny {
                    reason: "rewritten".to_owned(),
                })
            }
            (Defect::ApprovalBecomesDeny, AuthorityVerdict::ApprovalRequired(asked)) => {
                AuthorityVerdict::Deny(AuthorityVerdictDeny {
                    reason: asked.request,
                })
            }
            (_, verdict) => verdict,
        })
    }
}

struct MutantFixture(Defect);

impl AuthorityFixture for MutantFixture {
    type Provider = Mutant;

    fn provider(&self, backing: Backing) -> Mutant {
        Mutant {
            backing,
            defect: self.0,
        }
    }
}

// Proof cases: each as-given check catches a plain mutant, and only that check fails.

#[test]
fn adversary_kits_allow_as_given_catches_allow_turned_approval() {
    let report = authority::run(&MutantFixture(Defect::AllowBecomesApproval));
    assert_eq!(report.failed(), vec![Check::AllowAsGiven], "{report}");
}

#[test]
fn adversary_kits_deny_as_given_catches_a_rewritten_reason() {
    let report = authority::run(&MutantFixture(Defect::DenyReasonRewritten));
    assert_eq!(report.failed(), vec![Check::DenyAsGiven], "{report}");
}

#[test]
fn adversary_kits_approval_required_as_given_catches_approval_turned_deny() {
    let report = authority::run(&MutantFixture(Defect::ApprovalBecomesDeny));
    assert_eq!(
        report.failed(),
        vec![Check::ApprovalRequiredAsGiven],
        "{report}"
    );
}

// Hole cases.

/// Every check hands the provider a backing with one entry, for the one capability it then asks
/// about. A provider that answers every capability with whatever verdict it holds first, the
/// capability ignored, passes: no check asks for a capability the backing answers differently.
#[test]
fn adversary_kits_authority_catches_a_provider_that_ignores_the_capability() {
    let report = authority::run(&MutantFixture(Defect::IgnoresCapability));
    assert!(
        !report.failed().is_empty(),
        "a provider that answers every capability with the first verdict it holds passed: \
         {report}"
    );
}

/// The kit documents that it reports each check it ran, naming each one that failed. A provider
/// that panics when its backing call fails makes `run` panic, so no report names backing-failure.
#[test]
fn adversary_kits_authority_reports_a_panicking_check_as_failed() {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        authority::run(&MutantFixture(Defect::PanicsOnBackingFailure))
    }));
    match outcome {
        Ok(report) => assert_eq!(report.failed(), vec![Check::BackingFailure], "{report}"),
        Err(_) => panic!(
            "authority::run panicked on a provider that panics when its backing call fails; \
             no report named backing-failure"
        ),
    }
}
