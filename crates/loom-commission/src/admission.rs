//! Frontier admission: whether the current frontier admits a proposed action.
//!
//! An executor may not invoke an action absent from the current frontier
//! (`docs/contracts/frontier.md`). The check fails toward less authority: an action the frontier
//! does not list, or lists as `ApprovalRequired` with no capability to ask for, is refused.
//!
//! A frontier may list one action more than once. The check then takes the least-authority entry,
//! and its result does not depend on the order of the entries:
//!
//! 1. any `Blocked` entry: refused, carrying the reasons of every `Blocked` entry;
//! 2. any `ApprovalRequired` entry with no capability, or one that is empty or whitespace only:
//!    refused, carrying the reasons of every such entry;
//! 3. `ApprovalRequired` entries naming different capabilities: refused with one reason naming the
//!    conflict, each capability quoted as a JSON string, in sorted order;
//! 4. any `ApprovalRequired` entry: needs authority, naming its capability;
//! 5. otherwise admissible.
//!
//! Where reasons come from several entries, each entry's reasons keep their own order and the
//! entries are taken in the order of their reason lists, with identical lists taken once.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use crate::model::json::push_text;
use crate::model::responsibility::{
    ActionStatus, Admission, AdmissionNeedsAuthority, AdmissionRefused, Frontier, FrontierAction,
    Unit, frontier_state,
};

/// Sorts the proposed `action` against `frontier`'s actions.
pub fn admit<S: frontier_state::Marker>(frontier: &Frontier<S>, action: &str) -> Admission {
    let listed: Vec<&FrontierAction> = frontier
        .data()
        .actions
        .iter()
        .filter(|candidate| candidate.action == action)
        .collect();
    if listed.is_empty() {
        return refused(action, Vec::new());
    }

    let blocked = || {
        listed
            .iter()
            .filter(|entry| entry.status == ActionStatus::Blocked)
    };
    if blocked().next().is_some() {
        return refused(action, reasons_of(blocked()));
    }

    let approval = || {
        listed
            .iter()
            .filter(|entry| entry.status == ActionStatus::ApprovalRequired)
    };
    let uncapable = || approval().filter(|entry| capability_of(entry).is_none());
    if uncapable().next().is_some() {
        return refused(action, reasons_of(uncapable()));
    }

    let capabilities: BTreeSet<&str> = approval()
        .filter_map(|entry| capability_of(entry))
        .collect();
    match capabilities.len() {
        0 => Admission::Admissible(Unit(true)),
        1 => Admission::NeedsAuthority(AdmissionNeedsAuthority {
            capability: capabilities
                .first()
                .map(|capability| (*capability).to_owned())
                .unwrap_or_default(),
        }),
        _ => refused(
            action,
            vec![format!(
                "conflicting capabilities for an ApprovalRequired action: {}",
                capabilities
                    .into_iter()
                    .map(quoted)
                    .collect::<Vec<_>>()
                    .join(", ")
            )],
        ),
    }
}

/// The capability an entry asks for; an empty or whitespace-only one asks for nothing.
fn capability_of(entry: &FrontierAction) -> Option<&str> {
    entry
        .capability
        .as_deref()
        .filter(|capability| !capability.trim().is_empty())
}

/// `text` as a JSON string, so names that contain the separator stay apart.
fn quoted(text: &str) -> String {
    let mut out = String::new();
    push_text(&mut out, text);
    out
}

/// The reasons of `entries`, independent of the order the frontier lists them in.
fn reasons_of<'a>(entries: impl Iterator<Item = &'a &'a FrontierAction>) -> Vec<String> {
    entries
        .map(|entry| entry.reasons.clone())
        .collect::<BTreeSet<Vec<String>>>()
        .into_iter()
        .flatten()
        .collect()
}

fn refused(action: &str, reasons: Vec<String>) -> Admission {
    Admission::Refused(AdmissionRefused {
        action: action.to_owned(),
        reasons,
    })
}
