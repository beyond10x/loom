//! Loom's own model ports that run something.
//!
//! The ported [`crate::harness::wire`] holds neutral values and ports and performs no I/O; a port
//! here implements its [`ModelPort`](crate::harness::wire::ModelPort) over something it runs.
//!
//! [`ClaudeCodePort`] runs on an operator's Claude Code login through the `claude` command, which
//! reads that login itself; Loom code reads no credential for it.

pub mod claude_code;

pub use claude_code::ClaudeCodePort;
