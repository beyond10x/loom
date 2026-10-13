//! [`FileSuspensionStore`]: Commission's suspension port in a directory, re-exported from
//! `loom_governor`, where the production store lives beside `FileCaseStore` and shares its lock
//! and atomic-rename code. Kept here so code that named the testkit's store keeps compiling.

pub use loom_governor::{FileSuspensionStore, LONGEST_COMMISSION_ID as LONGEST_ID};
