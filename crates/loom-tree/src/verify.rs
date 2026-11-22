//! `tree verify`: every run log of the tree against its chain and the head the tree kept.

use std::path::Path;

use loom::tree::{TreeRefusalReason, TreeVerify};
use loom_governor::{CanonGovernor, FileCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::log::verify_case_log;
use crate::status::members;
use crate::store::{existing_store, read_manifest};
use crate::{Refused, unreadable};

/// Verifies the run log of every case of the tree in `store`, in tree order, and names the first
/// line that fails, counted from 0: a cut, a gap or a broken chain the log's reader refuses, or a
/// last line whose bytes are not the head the tree kept. The store is read, never held.
///
/// # Errors
/// `NotATree` when the store holds no tree; `StoreUnreadable` when a log or a head cannot be read.
pub fn verify_tree(store: &Path, catalog: &ProtocolCatalog) -> Result<TreeVerify, Refused> {
    let store = existing_store(store)?;
    let manifest = read_manifest(&store)?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let members = members(&governor, &manifest.root)?;
    let mut verified = 0i64;
    for member in &members {
        verified += 1;
        if let Some(failure) = verify_case_log(&store, &member.case)? {
            return Ok(TreeVerify {
                cases: verified,
                failure: Some(failure),
            });
        }
    }
    Ok(TreeVerify {
        cases: verified,
        failure: None,
    })
}
