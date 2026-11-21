#![forbid(unsafe_code)]

//! Decision requests, as Loom's specification declares them: the `loom.decision` domain
//! (`ess/domains/decision.yaml`), a copy of the SDC's contract.
//!
//! * The eight commands ([`request`], [`recommend`], [`decide`], [`take_recommendation`],
//!   [`record_produced`], [`escalate`], [`supersede`], [`expire`]) each read the addressed request
//!   from a [`DecisionStore`], answer the outcome the domain declares for it in ESS's precedence
//!   (the request alone, then whether the request exists, then what it holds, then acceptance),
//!   and write the request back only on an outcome that moves or creates it, from the state they
//!   read. A refusal writes nothing. The holder and the eligible principals are read from the
//!   stored request; `principal` and `principal_kind` are the command's. `principal` is held
//!   against the stored `eligible` list, and `principal_kind` must be the kind of the
//!   principal's name (`decider:<kind>`, `rule:<name>`, `silence`, any other name a person;
//!   `kind-not-principal` otherwise) before it decides which kind guards apply. No guard reads
//!   `addressed_to`.
//! * [`DecisionStore`] is the port: read, write from a read state ([`StoreConflict`] otherwise),
//!   list, find the Open or Escalated request for a record key, and the
//!   [`EscalationSuppressedRecord`](model::EscalationSuppressedRecord)s kept beside a request.
//!   [`InMemoryDecisionStore`] implements it; a store that cannot read or write answers
//!   [`StoreFailure`]. Neither is an ESS error: no command of the domain answers them. Every
//!   store refuses, in the step that writes, a write that would leave two Open or Escalated
//!   requests with one record key ([`StoreConflict::record_key_held_by`]).
//! * [`open_decisions_for_principal`], [`artifact_decision_history`] and [`decision_requests`]
//!   are the domain's three views over a store, by their generated queries.
//! * [`model`] is the generated `loom.decision` module, so a caller names one path for the
//!   types; [`Timestamp`], the generated primitive the instants are, is re-exported beside it.
//!
//! The specification keeps the eight behaviours obligations of this crate (their errors' fields
//! have no declared source), so they are written here against the generated types. `Request`
//! refuses `RecordKeyOpen` for the request the store finds open for its record key, or the one
//! the command names when the store finds none, and again when the store refuses its write for
//! a request another writer opened after that check: one Open or Escalated request per record key
//! holds whether or not the caller looked. An instant compared by a guard (`expires_at` against a tick)
//! is read as RFC 3339; one that does not read decides no outcome ([`DecisionError::Undecided`]).

mod apply;
mod instant;
mod store;
mod views;

pub use apply::{
    DecisionError, Undecided, decide, escalate, expire, recommend, record_produced, request,
    supersede, take_recommendation,
};
pub use loom::decision as model;
pub use loom::primitives::Timestamp;
pub use store::{
    AppendError, DecisionStore, InMemoryDecisionStore, StoreConflict, StoreFailure, WriteError,
};
pub use views::{artifact_decision_history, decision_requests, open_decisions_for_principal};
