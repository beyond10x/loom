//! Propose the host-catalog protocol an intent should run under.
//!
//! [`classify`] retains the engineering bundle; [`classify_with_catalog`] accepts the host catalog, described
//! by its Canon description and artifact descriptions, and forces one `pick_protocol` call through
//! `b10x_llm_tool_call::call_tool`. The answer is a [`ProtocolPick`]: a proposal, never authority.
//! A pick outside the registry, or below the caller's confidence threshold, is refused as a
//! [`RouterError`] rather than guessed.

mod classify;

pub use classify::{PICK_TOOL, ProtocolPick, RouterError, classify, classify_with_catalog};
