#![forbid(unsafe_code)]

//! The Loom SDK: one crate an application depends on to embed a governed agent.
//!
//! It re-exports the crates an embedder needs, under stable module names, and adds no behaviour of
//! its own.
//!
//! | Module | Crate | What it is for |
//! | --- | --- | --- |
//! | [`commission`] | `b10x-commission` | The contracts and the runtime: the generated responsibility model ([`commission::model`]), the ports an embedder implements or calls ([`commission::ports`]), and the loop that drives a commission until it is blocked ([`commission::runtime`]). |
//! | [`loom`] | `b10x-loom` | The executor: [`Loom`] proposes one action per frontier through an [`ActionSelector`] and an [`ArgumentGenerator`] the embedder supplies. |
//! | [`governor`] | `b10x-governor` | The governor: [`CanonGovernor`] evaluates a case's protocol with Canon over a [`CaseStore`] and issues its frontier. |
//! | [`intake`] | `b10x-intake-router`, `b10x-intake-slice` | Intake: the router that proposes a protocol for an intent ([`intake::router`]), and the slice's case opening, local executor, test-result verifier and local effect adapter ([`intake::slice`]). |
//!
//! The items an embedding always touches are also re-exported at the top: [`run_until_blocked`],
//! [`LoopContext`], [`LoopEnd`], [`EffectPort`], [`Loom`], [`ActionSelector`],
//! [`ArgumentGenerator`], [`CanonGovernor`], [`CaseStore`] and [`MemoryCaseStore`].
//!
//! `examples/software_change.rs` embeds the runtime end to end: it opens a case on
//! `software-change@1`, runs the loop over scripted fake models and stops at
//! `ApprovalRequired (repository.merge)`.

/// Commission's contracts and runtime (`b10x-commission`).
pub use b10x_commission as commission;

/// The Loom executor (`b10x-loom`).
pub use b10x_loom as loom;

/// The governor (`b10x-governor`).
pub use ::governor;

/// Intake: routing an intent to a protocol, and the local slice that performs `software-change@1`.
pub mod intake {
    /// The router (`b10x-intake-router`).
    pub use intake_router as router;

    /// The slice (`b10x-intake-slice`): case opening, the local executor, the test-result
    /// verifier and the local effect adapter.
    pub use intake_slice as slice;
}

pub use b10x_commission::ports::effect::EffectPort;
pub use b10x_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
pub use b10x_loom::{ActionSelector, ArgumentGenerator, Loom};
pub use governor::{CanonGovernor, CaseStore, MemoryCaseStore};
