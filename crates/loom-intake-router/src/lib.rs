//! Propose the ELS registry protocol an intent should run under (story router-classifier).
//!
//! [`classify`] offers a model every protocol of `canon_engineering::registry::list()`, described
//! by its Canon description and artifact descriptions, and forces one `pick_protocol` call through
//! `b10x_llm_tool_call::call_tool`. The answer is a [`ProtocolPick`]: a proposal, never authority.
//! A pick outside the registry, or below the caller's confidence threshold, is refused as a
//! [`RouterError`] rather than guessed.

mod classify;

pub use classify::{PICK_TOOL, ProtocolPick, RouterError, classify};
