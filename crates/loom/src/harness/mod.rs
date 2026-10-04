// SPDX-License-Identifier: Apache-2.0

//! The model/tool loop ported from `beyond10x/harness` (`docs/design/harness-map.md`, `port` rows).
//!
//! Each submodule is one Harness crate at `798325f03cf5a18df8fadb346d31b314826136ec` (release
//! 0.13.3), carried with its source unchanged except for these:
//!
//! - paths: a reference to the crate itself or to a sibling crate is now a path below this module;
//! - SPDX headers: each file states its licence on its first line;
//! - one hostname: an example gateway host in `responses` is neutral;
//! - citation comments: a comment citing a Harness document (its `AGENTS.md` invariants, its
//!   design records, its roadmap) names Harness and the revision;
//! - one provenance paragraph at the end of each module's own documentation;
//! - 11 test assertions in `turn_loop/tests.rs` and `responses/mod.rs`, rewritten from
//!   `assert!(x.is_empty())` to `assert_eq!(x, [] as [T; 0])` because `clippy::pedantic` denies
//!   `clippy::assert_is_empty` under the toolchain Loom builds with;
//! - reflowed comment blocks: paragraphs the longer paths and citations pushed past 100 columns
//!   are rewrapped, with no other word changed.
//!
//! The ported source is Apache-2.0 (`AGENTS.md` § Boundary); Harness keeps its own licence.
//!
//! The provider adapters are held to the provider-wire contracts copied into
//! `crates/loom/tests/fixtures/provider-wires/` by `crates/loom/tests/harness_port.rs` and by
//! Harness's own contract, summary-request and transport suites, carried into
//! `crates/loom/tests/harness_port_contract.rs`. Harness's `tests/provider_emulated.rs` (39 cases
//! across the two wires, driving Python fake endpoints over a socket) is not carried.
//!
//! The lints are Harness's own: its workspace denied `clippy::all` and `clippy::pedantic` and
//! allowed the two below.
#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    reason = "allowed by Harness's workspace lints at 798325f0"
)]
#![allow(
    clippy::must_use_candidate,
    reason = "allowed by Harness's workspace lints at 798325f0"
)]

pub mod http;
pub mod messages;
pub mod responses;
pub mod turn_loop;
pub mod wire;
