//! A second consumer of `b10x-loom-sdk`, shaped like a product that defines its agents as data.
//!
//! [`agency`] parses a `loom.agent/1` document, keeps the limits it declares per agent, and runs
//! one commission through `loom_sdk::Runtime`. `tests/agent_spec.rs` runs it. Like the binary,
//! it depends on `b10x-loom-sdk` alone.

pub mod agency;
