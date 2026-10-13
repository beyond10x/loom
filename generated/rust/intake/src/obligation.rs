// generated from intake v1
// model digest 58fc59c73e160b3816e4c2add829c8c5a6c05ea1c6901838a7d61a911e157818
// contract digest c7bf5a36bfa3d72e856cecfa8dd5cf67906769eb0ed8049fb9d732fc4c997443
// do not edit: regenerate with `ess synthesize --layout crate`

//! The typed refusal of an unmet obligation, and the conversion seams owed between contexts.
//!
//! An obligation is a capability the synthesis plan owes the implementor — the contract is declared,
//! the behaviour is not. Until an implementation satisfies one, its stub returns [`UnmetObligation`]:
//! a value naming the plan entry, never a panic and never a guess, so a workspace built on stubs
//! compiles and reports its own gaps.

/// A capability the synthesis plan owes and nothing has satisfied yet.
///
/// The two fields spell the plan entry: look the pair up in `PLAN.md` for the contract being
/// refused. A satisfying implementation never constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmetObligation {
    /// The capability kind, as the plan spells it.
    pub capability: &'static str,
    /// The construct that requires it, in the specification's own spelling.
    pub source: &'static str,
}

impl core::fmt::Display for UnmetObligation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "unmet obligation: {} `{}` — see PLAN.md", self.capability, self.source)
    }
}
