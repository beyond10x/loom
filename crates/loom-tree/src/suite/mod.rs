//! A suite of scripted tree cycles, `loom.tree-suite/1` (`loom.tree.TreeSuite`,
//! `ess/domains/tree.yaml`), and what `b10x-loom tree test` does with one.
//!
//! - [`decode_suite`] reads the document, applies its defaults to every cycle and refuses, naming
//!   the member, anything the format does not declare: an undeclared member, a step kind, source or
//!   run log event kind it does not know, a path written in the suite that is absolute or whose
//!   `..` leaves its root, a regex over 1024 bytes or one the regex syntax refuses, a selection that
//!   does not start with exactly one source, an `after` or `step` naming no step before it.
//! - [`select`] reads what a cycle left (`<scratch>/<n>/store` and `work`), the steps' JSON output
//!   and the suite's own files, and nothing else; [`evaluate_checks`] tests what a selection gives
//!   against the thirteen check kinds and names the first difference. A check is data: no shell, no
//!   query string, no expression language, no network.
//! - [`SUITE_AVAILABILITY`] marks each step kind, source, event kind and member as available or as
//!   waiting for a named story; a cycle that uses one that waits is pending and never run.
//! - [`run_suite`] runs the selected cycles (`SuiteSettings::only`, every cycle by default), up to
//!   `SuiteSettings::workers` at once (one by default), each step through the function of its
//!   `tree` command in process, and answers a [`SuiteReport`](loom::tree::SuiteReport) in suite
//!   order; a [`SuiteObserver`] is given each cycle's result in suite order, on the caller's
//!   thread.

mod availability;
mod check;
mod decode;
mod path;
mod run;
mod select;
mod value;

pub use availability::{
    Availability, AvailabilityEntry, KindUse, SUITE_AVAILABILITY, first_waiting,
};
pub use check::{Captured, capture_after, evaluate_checks, printable_found};
pub use decode::{
    DEFAULT_MAX_ROUNDS, MAX_REGEX_BYTES, MAX_ROUNDS, MAX_SUITE_BYTES, SUITE_FORMAT, SuiteRefused,
    decode_suite, read_suite,
};
pub use run::{
    CYCLE_WALL_SECONDS, SUBSTRATE_WORKERS_UNSERVED, SUITE_PRINCIPAL, SUITE_ROOT_CASE,
    SuiteObserver, SuiteSettings, cycle_line, not_run_result, report_json, run_suite, summarize,
    summary_lines, waiting_result, when_holds, workers_served,
};
pub use select::{CycleView, MAX_GLOB_PATHS, MAX_SELECTED_FILE_BYTES, Selected, select};
