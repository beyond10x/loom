//! The authority provider of a plugin turn.
//!
//! Commission's runtime asks it, at the moment of the call, for a capability the governor's
//! frontier names. It grants the two capabilities of `inbound-answer@1`, `datasource.read` and
//! `reply.propose`, and denies every other: a capability it does not know gets no grant.

use loom_sdk::commission::model::responsibility::{
    AuthorityVerdict, AuthorityVerdictDeny, CommissionData, Unit,
};
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};

/// The capabilities a plugin turn is granted.
pub const GRANTED: [&str; 2] = ["datasource.read", "reply.propose"];

/// Grants [`GRANTED`] and denies any other capability.
#[derive(Debug, Clone, Copy, Default)]
pub struct PluginAuthority;

impl AuthorityProvider for PluginAuthority {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(if GRANTED.contains(&capability) {
            AuthorityVerdict::Allow(Unit(true))
        } else {
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!("a plugin turn is not granted `{capability}`"),
            })
        })
    }
}
