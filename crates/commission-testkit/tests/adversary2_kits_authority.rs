//! Adversary pass 2 on the authority conformance kit (`kits::authority`).
//!
//! Each case runs the kit against a provider over the kit's `Backing` with one caching defect, the
//! kind an adapter adds in front of a slow authority service, and asserts that the kit fails it.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use b10x_commission::model::responsibility::{AuthorityVerdict, CommissionData};
use b10x_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_commission_testkit::kits::authority::{self, AuthorityFixture, Backing};

/// Caches the first verdict it decides and answers every later question with it, whatever
/// capability is asked: the cache key leaves the capability out.
struct CacheKeyedWithoutCapability {
    backing: Backing,
    cached: Mutex<Option<AuthorityVerdict>>,
}

impl AuthorityProvider for CacheKeyedWithoutCapability {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(verdict) = cached.as_ref() {
            return Ok(verdict.clone());
        }
        let verdict = self
            .backing
            .call(capability)
            .map_err(|error| AuthorityProviderError::new(error.message))?;
        *cached = Some(verdict.clone());
        Ok(verdict)
    }
}

struct CacheKeyedWithoutCapabilityFixture;

impl AuthorityFixture for CacheKeyedWithoutCapabilityFixture {
    type Provider = CacheKeyedWithoutCapability;

    fn provider(&self, backing: Backing) -> CacheKeyedWithoutCapability {
        CacheKeyedWithoutCapability {
            backing,
            cached: Mutex::new(None),
        }
    }
}

/// Remembers the last verdict the backing gave per capability and, when the backing call fails,
/// answers with it: a stale allow served while the authority service is down.
struct StaleOnFailure {
    backing: Backing,
    last: Mutex<BTreeMap<String, AuthorityVerdict>>,
}

impl AuthorityProvider for StaleOnFailure {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        match self.backing.call(capability) {
            Ok(verdict) => {
                last.insert(capability.to_owned(), verdict.clone());
                Ok(verdict)
            }
            Err(error) => last
                .get(capability)
                .cloned()
                .ok_or_else(|| AuthorityProviderError::new(error.message)),
        }
    }
}

struct StaleOnFailureFixture;

impl AuthorityFixture for StaleOnFailureFixture {
    type Provider = StaleOnFailure;

    fn provider(&self, backing: Backing) -> StaleOnFailure {
        StaleOnFailure {
            backing,
            last: Mutex::new(BTreeMap::new()),
        }
    }
}

/// The kit's own documentation: "a provider must answer the capability it was asked about". Every
/// check asks one fresh provider about one capability, so a provider that answers every later
/// capability with the first verdict it decided passes; the decoys are held and never asked.
#[test]
fn adversary2_kits_authority_catches_a_cache_that_ignores_the_capability() {
    let report = authority::run(&CacheKeyedWithoutCapabilityFixture);
    assert!(
        !report.failed().is_empty(),
        "a provider answering every capability with the first verdict it decided passed:\n\
         {report}"
    );
}

/// The story: "when that backing call fails the provider must answer deny or an error ... never
/// allow". The backing is fixed when the provider is built, so the kit can never make a call fail
/// after one succeeded, and a provider that serves the last allow while its backing is down passes.
#[test]
fn adversary2_kits_backing_failure_catches_a_stale_allow_served_while_the_backing_is_down() {
    let report = authority::run(&StaleOnFailureFixture);
    assert!(
        report.failed().contains(&authority::Check::BackingFailure),
        "a provider that answers the last allow when its backing call fails passed:\n{report}"
    );
}
