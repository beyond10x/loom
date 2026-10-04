#![forbid(unsafe_code)]

//! Test support for Commission: fakes of its ports and conformance kits for adapters.
//! `b10x-commission` does not depend on this crate.

pub mod fake_authority;
pub mod fake_executor;
pub mod fake_governor;
pub mod kits;
