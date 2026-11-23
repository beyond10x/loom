//! A suite of scripted tree cycles, `loom.tree-suite/1` (`loom.tree.TreeSuite`,
//! `ess/domains/tree.yaml`), and what `b10x-loom tree test` does with one.
//!
//! - [`decode_suite`] reads the document, applies its defaults to every cycle and refuses, naming
//!   the member, anything the format does not declare: an undeclared member, a step kind, source or
//!   run log event kind it does not know, a path written in the suite that is absolute or whose
//!   `..` leaves its root, a `write` step's `to` with any `.` or `..` component, a regex over 1024 bytes or one the regex syntax refuses, a selection that
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
//!   thread. A cycle that runs past `SuiteSettings::cycle_timeout` has its running program killed
//!   and ends `CycleTimedOut`; [`slowest_line`] names the slowest cycles that ran.
//! - [`list_suite`] gives the line `tree test --list` prints per selected cycle; it reads the
//!   suite and nothing else.
//! - A `write` step runs no tree command: it copies a regular file of the suite's directory, at
//!   most [`MAX_WRITE_BYTES`] and reached through no link, to a path of plain names in the cycle's
//!   workspace (never its store), through a new file renamed over it, so it never follows a link
//!   and never leaves a partial file. It writes nothing to the run log or the store, adds no
//!   minute to `at` and gives no output to `{step: <name>}`.

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
    CYCLE_WALL_SECONDS, MAX_WRITE_BYTES, SLOWEST_CYCLES, SUBSTRATE_WORKERS_UNSERVED,
    SUITE_PRINCIPAL, SUITE_ROOT_CASE, SuiteObserver, SuiteSettings, cycle_line, list_suite,
    not_run_result, report_json, run_suite, slowest_line, summarize, summary_lines, waiting_result,
    when_holds, workers_served,
};
pub use select::{CycleView, MAX_GLOB_PATHS, MAX_SELECTED_FILE_BYTES, Selected, select};
