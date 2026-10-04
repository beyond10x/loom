//! A static fake `AuthorityProvider`.
//!
//! Its table maps each capability to a verdict or to a failure. A capability missing from the
//! table is a failure too, so the fake never grants what it was not told to. Every call is
//! recorded with the principal and authority context it was asked about.

use b10x_commission::model::responsibility::{
    AuthorityContext, AuthorityVerdict, CommissionData, PrincipalId,
};
use b10x_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// One question the fake was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityQuery {
    /// The commission's principal, as passed.
    pub principal: PrincipalId,
    /// The commission's authority context, as passed.
    pub authority_context: AuthorityContext,
    /// The capability asked about.
    pub capability: String,
}

/// What the table holds for one capability.
#[derive(Debug, Clone)]
enum Scripted {
    Verdict(AuthorityVerdict),
    Fail(String),
}

/// An `AuthorityProvider` answering from a fixed table and recording what it is asked.
#[derive(Debug, Default)]
pub struct StaticAuthorityProvider {
    table: BTreeMap<String, Scripted>,
    asked: Mutex<Vec<AuthorityQuery>>,
}

impl StaticAuthorityProvider {
    /// A provider with an empty table: every capability fails.
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `capability` with `verdict`, replacing any earlier entry for it.
    pub fn answer(mut self, capability: impl Into<String>, verdict: AuthorityVerdict) -> Self {
        self.table
            .insert(capability.into(), Scripted::Verdict(verdict));
        self
    }

    /// Fails on `capability` with `message`, replacing any earlier entry for it.
    pub fn fail(mut self, capability: impl Into<String>, message: impl Into<String>) -> Self {
        self.table
            .insert(capability.into(), Scripted::Fail(message.into()));
        self
    }

    /// Every question asked so far, in call order.
    pub fn asked(&self) -> Vec<AuthorityQuery> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AuthorityProvider for StaticAuthorityProvider {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(AuthorityQuery {
                principal: commission.principal.clone(),
                authority_context: commission.authority_context.clone(),
                capability: capability.to_owned(),
            });
        match self.table.get(capability) {
            Some(Scripted::Verdict(verdict)) => Ok(verdict.clone()),
            Some(Scripted::Fail(message)) => Err(AuthorityProviderError::new(message.clone())),
            None => Err(AuthorityProviderError::new(format!(
                "no entry for capability `{capability}`"
            ))),
        }
    }
}
