#![forbid(unsafe_code)]

//! The Loom SDK: one crate an application depends on to embed a governed agent.
//!
//! It re-exports the crates an embedder needs, under stable module names. Its one module of its
//! own is [`llm`], the model port over an llm `Model`, kept here because the executor does not
//! depend on llm's model crates.
//!
//! | Module | Crate | What it is for |
//! | --- | --- | --- |
//! | [`commission`] | `b10x-loom-commission` | The contracts and the runtime: the generated responsibility model ([`commission::model`]), the ports an embedder implements or calls ([`commission::ports`]), and the loop that drives a commission until it is blocked ([`commission::runtime`]). |
//! | [`connectors`] | `b10x-loom-connectors` | Commission's `ConnectorInvoker` over a Connectors service: [`connectors::ConnectorsInvoker`] invokes the operation an action is bound to once, on the host's endpoint for its instance, and reports the attempt Connectors recorded. |
//! | [`loom`] | `b10x-loom-executor` | The executor: [`Loom`] proposes one action per frontier through an [`ActionSelector`] and an [`ArgumentGenerator`] the embedder supplies. |
//! | [`governor`] | `b10x-loom-governor` | The governor: [`CanonGovernor`] evaluates a case's protocol with Canon over a [`CaseStore`] and issues its frontier. |
//! | [`llm`] | `b10x-llm-core` | [`llm::LlmModelPort`], a harness `ModelPort` over an llm `Model`: one `Model::turn` per turn, the port `b10x-loom` runs a plugin turn on. |
//! | [`intake`] | `b10x-loom-intake-router`, `b10x-loom-intake-slice` | Intake: the router that proposes a protocol for an intent ([`intake::router`]), and the slice's case opening, local executor, test-result verifier and local effect adapter ([`intake::slice`]). |
//!
//! The items an embedding always touches are also re-exported at the top: [`run_until_blocked`],
//! [`LoopContext`], [`LoopEnd`], [`EffectPort`], [`Loom`], [`ActionSelector`],
//! [`ArgumentGenerator`], [`CanonGovernor`], [`CaseStore`] and [`MemoryCaseStore`].
//! [`RunMeter`], [`MeteredModelPort`] and [`MeteredExecutor`] report a run's model calls, tokens
//! and cost per executor call (`loom::metering`).
//!
//! `examples/software_change.rs` embeds the runtime end to end: it opens a case on
//! `software-change@1`, runs the loop over scripted fake models and stops at
//! `ApprovalRequired (repository.merge)`.
//!
//! # Stability
//!
//! The stable surface of this crate is named on one page, `website/docs/reference/sdk-stability.md`
//! (<https://beyond10x.github.io/loom/docs/reference/sdk-stability>): the ports a host implements,
//! the types and functions an embedding calls, and every `loom_sdk` path control-plane imports. It
//! stays stable within a release line, and a release that breaks any of it names each break in its
//! `CHANGELOG.md` entry under `Breaking:`. A consumer pins a bare-version tag
//! (`tag = "<version>"`) and moves by changing it. Items not on the page, `#[doc(hidden)]` items and
//! generated model types are outside that promise; a generated type changes with its ESS
//! declaration, and a release names such a change as a break. `tests/host_ports.rs` implements
//! every listed port through `loom_sdk` paths alone and fails when the page and this crate disagree.

/// Commission's contracts and runtime (`b10x-loom-commission`).
pub use b10x_loom_commission as commission;

/// Commission's `ConnectorInvoker` over a Connectors service (`b10x-loom-connectors`).
pub use b10x_loom_connectors as connectors;

/// The Loom executor (`b10x-loom-executor`).
pub use b10x_loom_executor as loom;

pub mod llm;

pub mod models;

/// The governor (`b10x-loom-governor`).
pub use loom_governor as governor;

/// Protocol definitions and the immutable host catalog (`b10x-loom-protocols`).
pub use loom_protocols as protocols;

/// Intake: routing an intent to a protocol, and the local slice that performs `software-change@1`.
pub mod intake {
    /// The router (`b10x-loom-intake-router`).
    pub use b10x_loom_intake_router as router;

    /// The slice (`b10x-loom-intake-slice`): case opening, the local executor, the test-result
    /// verifier and the local effect adapter.
    pub use b10x_loom_intake_slice as slice;
}

pub use b10x_loom_commission::ports::effect::EffectPort;
pub use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
/// A Loom workflow definition run as Commission's executor, and the effect port wrapper it reads
/// (`loom_sdk::loom::workflow`); a host checks a definition with `workflow::load` against
/// `ProtocolEntry::actions` of the protocol it names.
pub use b10x_loom_executor::workflow::{RecordingEffects, WorkflowExecutor};
pub use b10x_loom_executor::{ActionSelector, ArgumentGenerator, Loom};
pub use b10x_loom_executor::{MeteredExecutor, MeteredModelPort, RunMeter};
pub use b10x_loom_intake_slice::confinement::{
    ConfinementError, SubstrateRunner, TestExecution, TestRunner,
};
pub use b10x_loom_intake_slice::executor::UnconfinedRunner;
pub use loom_governor::{CanonGovernor, CaseIndependence, CaseInputs, CaseStore, MemoryCaseStore};

/// The protocol catalog shared by routing, case initialization and governor admission.
pub use loom_protocols::ProtocolCatalog;
