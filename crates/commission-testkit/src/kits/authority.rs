//! The conformance kit for an `AuthorityProvider` adapter.
//!
//! An adapter crate implements [`AuthorityFixture`] and calls [`run`], usually as
//! `run(&MyFixture).assert_passed()` in one of its tests. For each check the kit hands the fixture a
//! [`Backing`]: a stand-in for the service the adapter's provider asks, scripted per capability by
//! the kit, which it can make fail. The adapter builds its provider over it. The kit runs every
//! [`Check`] and reports each check it ran, naming each one that failed. A check during which the
//! provider or the fixture panics fails with the panic's message, and the other checks still run.
//!
//! Each backing holds, beside the capability a check asks about, two other capabilities that
//! answer differently, one sorting before it and one after; a provider must answer the capability
//! it was asked about.
//!
//! The kit keeps its own handle on each backing and may change what it answers after the provider
//! is built, so a provider must ask its backing at each decision rather than answer from what the
//! backing said earlier.
//!
//! - [`Check::AllowAsGiven`], [`Check::DenyAsGiven`], [`Check::ApprovalRequiredAsGiven`]: the
//!   verdict the backing gives comes back from the provider unchanged, and the same provider then
//!   answers a second capability with that capability's own answer;
//! - [`Check::BackingFailure`]: once the backing call fails, the provider answers deny or an
//!   error, which Commission reads as a refusal; never allow, never approval-required, and not the
//!   allow the backing gave before it failed.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, PrincipalId, Unit,
    commission_state,
};
use b10x_commission::ports::authority::{AuthorityProvider, check_authority};

use super::governor::unwound;

/// Why a backing call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackingError {
    /// What the backing service reported.
    pub message: String,
}

impl fmt::Display for BackingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BackingError {}

/// What a backing holds for one capability: a verdict, or a failure.
type Answer = Result<AuthorityVerdict, BackingError>;

/// The backing service the kit hands an adapter's provider: per capability, a verdict or a
/// failure. A capability it holds no entry for fails.
///
/// Clones share one table: the kit keeps a clone and may change an answer after the provider is
/// built, and the provider's next [`call`](Backing::call) sees the change.
#[derive(Debug, Clone, Default)]
pub struct Backing {
    table: Arc<Mutex<BTreeMap<String, Answer>>>,
}

impl Backing {
    /// What the backing service answers for `capability` now.
    ///
    /// # Errors
    ///
    /// When the kit made the call for `capability` fail, or holds no entry for it.
    pub fn call(&self, capability: &str) -> Result<AuthorityVerdict, BackingError> {
        self.table().get(capability).cloned().unwrap_or_else(|| {
            Err(BackingError {
                message: format!("no entry for capability `{capability}`"),
            })
        })
    }

    /// Every entry as it stands now, by capability. A provider that loads these up front and does
    /// not ask again misses an answer the kit changes later.
    pub fn entries(&self) -> Vec<(String, Result<AuthorityVerdict, BackingError>)> {
        self.table()
            .iter()
            .map(|(capability, answer)| (capability.clone(), answer.clone()))
            .collect()
    }

    /// A backing answering [`CAPABILITY`] with `answer`, and each of [`DECOYS`] with `decoy`.
    fn with(answer: Answer, decoy: Answer) -> Self {
        let mut table = BTreeMap::from([(CAPABILITY.to_owned(), answer)]);
        for capability in DECOYS {
            table.insert(capability.to_owned(), decoy.clone());
        }
        Self {
            table: Arc::new(Mutex::new(table)),
        }
    }

    /// From now on, `capability` answers `answer`.
    fn set(&self, capability: &str, answer: Answer) {
        self.table().insert(capability.to_owned(), answer);
    }

    fn table(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Answer>> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What an adapter supplies to the authority kit.
pub trait AuthorityFixture {
    /// The adapter's provider.
    type Provider: AuthorityProvider;

    /// The adapter's provider, asking `backing` where it would ask its real authority source.
    fn provider(&self, backing: Backing) -> Self::Provider;
}

/// One check of the authority kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// An allow from the backing comes back as allow.
    AllowAsGiven,
    /// A deny from the backing comes back as that deny.
    DenyAsGiven,
    /// An approval-required from the backing comes back as that approval-required.
    ApprovalRequiredAsGiven,
    /// A failing backing call never comes back as allow or approval-required.
    BackingFailure,
}

impl Check {
    /// Every check, in the order [`run`] runs them.
    pub const ALL: [Check; 4] = [
        Check::AllowAsGiven,
        Check::DenyAsGiven,
        Check::ApprovalRequiredAsGiven,
        Check::BackingFailure,
    ];

    /// The check's name, as a report shows it.
    pub fn name(&self) -> &'static str {
        match self {
            Check::AllowAsGiven => "allow-as-given",
            Check::DenyAsGiven => "deny-as-given",
            Check::ApprovalRequiredAsGiven => "approval-required-as-given",
            Check::BackingFailure => "backing-failure",
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What one run of the kit found: every check it ran, each with its failure if it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    outcomes: Vec<(Check, Result<(), String>)>,
}

impl Report {
    /// Every check run, in the order run.
    pub fn ran(&self) -> Vec<Check> {
        self.outcomes.iter().map(|(check, _)| *check).collect()
    }

    /// Every check that failed, in the order run.
    pub fn failed(&self) -> Vec<Check> {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| outcome.is_err())
            .map(|(check, _)| *check)
            .collect()
    }

    /// Panics, with the report, unless every check passed.
    ///
    /// # Panics
    ///
    /// When any check failed.
    pub fn assert_passed(&self) {
        assert!(
            self.failed().is_empty(),
            "authority conformance failed:\n{self}"
        );
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (check, outcome) in &self.outcomes {
            match outcome {
                Ok(()) => writeln!(f, "pass {check}")?,
                Err(failure) => writeln!(f, "FAIL {check}: {failure}")?,
            }
        }
        Ok(())
    }
}

/// Runs every check against providers from `fixture`, each over its own backing.
pub fn run<F: AuthorityFixture>(fixture: &F) -> Report {
    Report {
        outcomes: Check::ALL
            .iter()
            .map(|check| (*check, unwound(|| run_check(fixture, *check))))
            .collect(),
    }
}

/// The capability every check asks about.
const CAPABILITY: &str = "kit.capability";

/// Capabilities every backing also holds, answering differently from [`CAPABILITY`]: one sorts
/// before it and one after, so a provider that answers from the first or the last entry it holds
/// answers the wrong one.
const DECOYS: [&str; 2] = ["kit.0-decoy", "kit.z-decoy"];

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

fn deny_because(reason: &str) -> AuthorityVerdict {
    AuthorityVerdict::Deny(AuthorityVerdictDeny {
        reason: reason.to_owned(),
    })
}

fn approval_for(request: &str) -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: request.to_owned(),
    })
}

fn unavailable() -> BackingError {
    BackingError {
        message: "backing service unavailable".to_owned(),
    }
}

/// Each as-given check's decoys answer differently from its verdict. A deny or approval-required
/// check's decoys are the same kind with other text; an allow has no other text, so an allow
/// check's decoys deny.
fn run_check<F: AuthorityFixture>(fixture: &F, check: Check) -> Result<(), String> {
    match check {
        Check::AllowAsGiven => as_given(
            fixture,
            allow(),
            deny_because("a decoy denied by the conformance kit"),
        ),
        Check::DenyAsGiven => as_given(
            fixture,
            deny_because("denied by the conformance kit"),
            deny_because("a decoy denied by the conformance kit"),
        ),
        Check::ApprovalRequiredAsGiven => as_given(
            fixture,
            approval_for("approval asked by the conformance kit"),
            approval_for("a decoy approval asked by the conformance kit"),
        ),
        Check::BackingFailure => backing_failure(fixture),
    }
}

/// The commission every check asks for.
fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId("kit-case-authority".to_owned()),
        principal: PrincipalId("kit-principal".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// Asks `provider` for `capability` and holds the answer to `backed`, the backing's verdict for
/// it: exactly, or, with `exact` false, by kind of verdict only.
fn answers_as_backed<P: AuthorityProvider>(
    provider: &P,
    capability: &str,
    backed: &AuthorityVerdict,
    exact: bool,
) -> Result<(), String> {
    let decided = provider.decide(commission().data(), capability);
    let matches = match &decided {
        Ok(answer) if exact => answer == backed,
        Ok(answer) => std::mem::discriminant(answer) == std::mem::discriminant(backed),
        Err(_) => false,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "for `{capability}` the backing answered {backed:?}; the provider answered \
             {decided:?}"
        ))
    }
}

/// The provider answers [`CAPABILITY`] with `verdict`, exactly, and then the first decoy with
/// `decoy`. The decoy is held to its kind of verdict only when that kind differs from `verdict`'s,
/// so a provider that alters, say, a deny's reason is named by deny-as-given alone.
fn as_given<F: AuthorityFixture>(
    fixture: &F,
    verdict: AuthorityVerdict,
    decoy: AuthorityVerdict,
) -> Result<(), String> {
    let provider = fixture.provider(Backing::with(Ok(verdict.clone()), Ok(decoy.clone())));
    answers_as_backed(&provider, CAPABILITY, &verdict, true)?;
    let same_kind = std::mem::discriminant(&verdict) == std::mem::discriminant(&decoy);
    answers_as_backed(&provider, DECOYS[0], &decoy, same_kind)
}

fn backing_failure<F: AuthorityFixture>(fixture: &F) -> Result<(), String> {
    let backing = Backing::with(Ok(allow()), Ok(allow()));
    let provider = fixture.provider(backing.clone());
    let commission = commission();
    // The provider first decides while the backing allows. What it answers is the as-given
    // checks' business, not this one's; the call is there so a provider that remembers it has
    // something to remember.
    let _ = provider.decide(commission.data(), CAPABILITY);

    backing.set(CAPABILITY, Err(unavailable()));

    match provider.decide(commission.data(), CAPABILITY) {
        Ok(AuthorityVerdict::Deny(_)) | Err(_) => {}
        Ok(decided) => {
            return Err(format!(
                "the backing call failed after it had allowed; the provider answered \
                 {decided:?}, not deny or an error"
            ));
        }
    }
    let check = check_authority(&provider, &commission, CAPABILITY);
    if check.allows() {
        return Err(format!(
            "the backing call failed after it had allowed; Commission's authority check \
             allowed: {check:?}"
        ));
    }
    Ok(())
}
