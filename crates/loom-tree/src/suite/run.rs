//! The results of a suite's cycles and what is printed of them: one line per cycle, the two
//! summary lines of `~/beyond10x/sdc/scripts/run-finding-cycles.sh` with the waiting clause added,
//! and the `loom.tree.SuiteReport` document `--report` writes. A pending cycle is never counted as
//! expected, failed or skipped, and a cycle waiting for a story is never run.
//!
//! [`run_suite`] runs the cycles a caller selects, up to `workers` at once, each thread taking the
//! next cycle in suite order; each cycle works in `<scratch>/<n>/` (`store/`, `work/`), `n` its
//! position in the suite, and runs each step through the function of its `tree` command in
//! process. The results, and the observer's calls, are in suite order whatever order the cycles
//! end in. It starts no `b10x-loom` process and no program of its own, prints nothing, reads no
//! environment variable and no current directory: every path it is given is absolute.
//!
//! Each running cycle has a flag of its own, which its worker hands `run_tree` and reads before
//! each step. One watcher thread per suite run holds each running cycle's deadline (its start plus
//! `SuiteSettings::cycle_timeout`) and, every [`CANCEL_WATCH`], sets the flag of each cycle past
//! its deadline, and of every running cycle once the suite's cancel flag is set; it ends when the
//! last worker has ended. A flag set while a step runs ends `run_tree` `Cancelled`, its program
//! killed; the cycle then ends `CycleTimedOut` naming that step, unless the suite was cancelled.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::TestRunner;
use loom::tree::{
    ApprovalDecision, CheckDifference, CycleCondition, CycleResult, CycleStep, DecisionKind,
    DifferenceReason, ExitExpectation, NextDecision, ScriptedDecision, StepKind, SuiteCycle,
    SuiteRefusalReason, SuiteReport, TreeRun, TreeStop,
};
use loom_protocols::ProtocolCatalog;
use serde_json::{Map, Value, json};

use super::availability::{AvailabilityEntry, SUITE_AVAILABILITY, first_waiting, step_name};
use super::check::{Captured, capture_after, evaluate_checks, holds};
use super::decode::{SuiteRefused, read_suite};
use super::select::{CycleView, select_values};
use crate::run::CANCEL_WATCH;
use crate::{
    ApproveRequest, CloseRequest, HoldRequest, KeptProtocol, OpenRequest, Reasons, Refused,
    ReopenRequest, RunSettings, TreeTime, open_tree, read_manifest, read_protocols_directory,
    replay_difference, replay_json, replay_tree, run_tree, status_json, tree_status, verify_json,
    verify_tree,
};
use loom::primitives::Decimal;
use loom::tree::{SelectItem, WaitingCycle};

/// The result of a cycle that waits for a story: pending, not run.
pub fn waiting_result(cycle: &SuiteCycle, waiting: WaitingCycle) -> CycleResult {
    CycleResult {
        name: cycle.name.clone(),
        pending: true,
        waits_for: Some(waiting),
        run: false,
        not_run_when: None,
        as_expected: false,
        difference: None,
        exit: None,
        seconds: Decimal("0.0".into()),
    }
}

/// The result of a cycle whose `when[index]` does not hold: not run, counted in neither
/// `expected` nor `pending`.
pub fn not_run_result(cycle: &SuiteCycle, index: usize) -> CycleResult {
    CycleResult {
        name: cycle.name.clone(),
        pending: cycle.pending,
        waits_for: None,
        run: false,
        not_run_when: i64::try_from(index).ok(),
        as_expected: false,
        difference: None,
        exit: None,
        seconds: Decimal("0.0".into()),
    }
}

/// Whether `condition` holds: a protocol of `catalog` (by registration or protocol id) declares
/// the action or outcome it names, or the file of `suite` it names holds its test. A protocol the
/// catalog does not hold declares nothing.
pub fn when_holds(condition: &CycleCondition, catalog: &ProtocolCatalog, suite: &Path) -> bool {
    if let (Some(protocol), Some(declares)) = (&condition.protocol, &condition.declares) {
        let Some(entry) = catalog.iter().find(|entry| {
            entry.name() == protocol.as_str()
                || entry.model.protocol.id.as_str() == protocol.as_str()
        }) else {
            return false;
        };
        let Ok(document) = serde_yaml_ng::from_str::<Value>(&entry.definition.yaml) else {
            return false;
        };
        return match (&declares.action, &declares.outcome) {
            (Some(action), _) => document["actions"].get(action.as_str()).is_some(),
            (None, Some(outcome)) => document["outcomes"].get(outcome.as_str()).is_some(),
            (None, None) => false,
        };
    }
    let (Some(file), Some(test)) = (&condition.suite, &condition.test) else {
        return false;
    };
    let item = SelectItem {
        log: None,
        records: None,
        runs: None,
        cases: None,
        doc: None,
        files: None,
        step: None,
        suite: Some(file.clone()),
        requests: None,
        calls: None,
        r#where: None,
        pick: None,
        path: None,
        follow: None,
        r#as: None,
    };
    let steps = std::collections::BTreeMap::new();
    let view = CycleView {
        work: suite,
        store: suite,
        suite,
        steps: &steps,
        catalog,
    };
    let Ok(values) = select_values(&[item], &view) else {
        return false;
    };
    let conditions = loom::tree::Conditions {
        all: vec![test.clone()],
        any_of: Vec::new(),
    };
    values.len() == 1 && holds(&values[0].value, &conditions).unwrap_or(false)
}

/// The counts of `cycles` in suite order: a full run (`only` absent) of a suite of those cycles
/// (`suite_cycles` their number).
pub fn summarize(cycles: Vec<CycleResult>) -> SuiteReport {
    let ran = |result: &&CycleResult| result.run;
    let expected: Vec<&CycleResult> = cycles.iter().filter(ran).filter(|r| !r.pending).collect();
    let pending = cycles
        .iter()
        .filter(|r| r.pending && (r.run || r.waits_for.is_some()))
        .count();
    SuiteReport {
        expected: i64::try_from(expected.len()).unwrap_or(i64::MAX),
        as_expected: i64::try_from(expected.iter().filter(|r| r.as_expected).count())
            .unwrap_or(i64::MAX),
        failed: expected
            .iter()
            .filter(|r| !r.as_expected)
            .map(|r| r.name.clone())
            .collect(),
        pending: i64::try_from(pending).unwrap_or(i64::MAX),
        pending_passing: cycles
            .iter()
            .filter(|r| r.pending && r.run && r.as_expected)
            .map(|r| r.name.clone())
            .collect(),
        waiting: cycles.iter().filter_map(|r| r.waits_for.clone()).collect(),
        not_run: cycles
            .iter()
            .filter(|r| r.not_run_when.is_some())
            .map(|r| r.name.clone())
            .collect(),
        only: None,
        suite_cycles: i64::try_from(cycles.len()).unwrap_or(i64::MAX),
        cycles,
    }
}

fn names(list: &[String]) -> String {
    if list.is_empty() {
        "none".into()
    } else {
        list.join(", ")
    }
}

fn difference_text(difference: &CheckDifference) -> String {
    let mut text = format!(
        "{} ({}, {:?}): expected {}",
        difference.check, difference.kind, difference.reason, difference.expected
    );
    if let Some(found) = &difference.found {
        text.push_str(&format!(", found {found}"));
    }
    text
}

/// The line one cycle's result prints.
pub fn cycle_line(result: &CycleResult) -> String {
    let name = &result.name;
    let seconds = &result.seconds.0;
    if let Some(waiting) = &result.waits_for {
        return format!(
            "{name}: pending, waits for {} ({})",
            waiting.story, waiting.kind
        );
    }
    if let Some(index) = result.not_run_when {
        return format!("{name}: not run (when[{index}])");
    }
    match (result.pending, result.as_expected, &result.difference) {
        (false, true, _) => format!("{name}: as expected ({seconds} s)"),
        (false, false, Some(difference)) => {
            format!(
                "{name}: FAILED: {} ({seconds} s)",
                difference_text(difference)
            )
        }
        (false, false, None) => format!("{name}: FAILED ({seconds} s)"),
        (true, true, _) => format!("{name}: pending, passing ({seconds} s)"),
        (true, false, Some(difference)) => format!(
            "{name}: pending, still failing: {} ({seconds} s)",
            difference_text(difference)
        ),
        (true, false, None) => format!("{name}: pending, still failing ({seconds} s)"),
    }
}

/// The two summary lines of `report`. With `only` set the first says how many cycles were
/// selected of how many, so that it is never taken for a full run.
pub fn summary_lines(report: &SuiteReport) -> [String; 2] {
    let selected = match &report.only {
        Some(only) => format!("selected {} of {}, ", only.len(), report.suite_cycles),
        None => String::new(),
    };
    let still_failing: Vec<String> = report
        .cycles
        .iter()
        .filter(|r| r.pending && r.run && !r.as_expected)
        .map(|r| r.name.clone())
        .collect();
    let waiting = if report.waiting.is_empty() {
        "none".to_owned()
    } else {
        report
            .waiting
            .iter()
            .map(|w| format!("{} on {}", w.name, w.story))
            .collect::<Vec<_>>()
            .join(", ")
    };
    [
        format!(
            "cycles: {selected}{} expected, {} as expected, {} failed: {}",
            report.expected,
            report.as_expected,
            report.failed.len(),
            names(&report.failed)
        ),
        format!(
            "pending: {} ({} still failing: {}; passing, move to expected: {}; {} waiting: {})",
            report.pending,
            still_failing.len(),
            names(&still_failing),
            names(&report.pending_passing),
            report.waiting.len(),
            waiting
        ),
    ]
}

/// How many cycles the `slowest:` line names at most.
pub const SLOWEST_CYCLES: usize = 10;

/// The line printed after the two summary lines: `slowest: <name> (<s> s), …`, the cycles of
/// `report` that ran, at most [`SLOWEST_CYCLES`], slowest first and ties in suite order, read from
/// their `seconds`; `slowest: none` when no cycle ran.
pub fn slowest_line(report: &SuiteReport) -> String {
    let mut ran: Vec<(&CycleResult, f64)> = report
        .cycles
        .iter()
        .filter(|result| result.run)
        .map(|result| (result, result.seconds.0.parse::<f64>().unwrap_or(0.0)))
        .collect();
    // A stable sort: cycles of equal seconds stay in suite order.
    ran.sort_by(|left, right| right.1.total_cmp(&left.1));
    let named: Vec<String> = ran
        .iter()
        .take(SLOWEST_CYCLES)
        .map(|(result, _)| format!("{} ({} s)", result.name, result.seconds.0))
        .collect();
    format!("slowest: {}", names(&named))
}

/// The line `tree test --list` prints for `cycle`: `<name>`, `<name>: pending`, or `<name>:
/// pending, waits for story:<id> (<kind>)` when it uses a part `availability` marks waiting.
fn list_line(cycle: &SuiteCycle, availability: &[AvailabilityEntry]) -> String {
    match first_waiting(cycle, availability) {
        Some(waiting) => format!(
            "{}: pending, waits for {} ({})",
            cycle.name, waiting.story, waiting.kind
        ),
        None if cycle.pending => format!("{}: pending", cycle.name),
        None => cycle.name.clone(),
    }
}

/// The lines `tree test --list` prints for the suite file `suite`: one per cycle `only` selects
/// (every cycle when it is empty), in suite order. It reads the suite and nothing else: no `when`
/// is evaluated, no protocol read and nothing written.
///
/// # Errors
/// [`SuiteRefused`]: what [`read_suite`] refuses, `CycleUnknown` for an `only` name that is no
/// cycle of the suite, and `SuiteUnreadable` for a relative `suite`.
pub fn list_suite(
    suite: &Path,
    only: &[String],
    availability: &[AvailabilityEntry],
) -> Result<Vec<String>, SuiteRefused> {
    let suite = absolute_path(suite, "the suite", SuiteRefusalReason::SuiteUnreadable)?;
    let decoded = read_suite(suite)?;
    let chosen = selected(&decoded.cycles, only)?;
    Ok(chosen
        .iter()
        .map(|&index| list_line(&decoded.cycles[index], availability))
        .collect())
}

fn put(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value);
    }
}

fn waiting_json(waiting: &WaitingCycle) -> Value {
    json!({"name": waiting.name, "story": waiting.story, "kind": waiting.kind})
}

fn difference_json(difference: &CheckDifference) -> Value {
    let mut object = Map::new();
    object.insert("check".into(), json!(difference.check));
    object.insert("kind".into(), json!(difference.kind));
    object.insert("reason".into(), json!(format!("{:?}", difference.reason)));
    object.insert("expected".into(), json!(difference.expected));
    put(
        &mut object,
        "found",
        difference.found.clone().map(Value::from),
    );
    Value::Object(object)
}

fn result_json(result: &CycleResult) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), json!(result.name));
    object.insert("pending".into(), json!(result.pending));
    put(
        &mut object,
        "waits_for",
        result.waits_for.as_ref().map(waiting_json),
    );
    object.insert("run".into(), json!(result.run));
    put(
        &mut object,
        "not_run_when",
        result.not_run_when.map(Value::from),
    );
    object.insert("as_expected".into(), json!(result.as_expected));
    put(
        &mut object,
        "difference",
        result.difference.as_ref().map(difference_json),
    );
    put(&mut object, "exit", result.exit.map(Value::from));
    object.insert(
        "seconds".into(),
        serde_json::from_str::<Value>(&result.seconds.0).unwrap_or(json!(result.seconds.0)),
    );
    Value::Object(object)
}

/// `report` as the `loom.tree.SuiteReport` document `--report` writes, member for member; `only`
/// is absent in a full run.
pub fn report_json(report: &SuiteReport) -> Value {
    let mut document = json!({
        "cycles": report.cycles.iter().map(result_json).collect::<Vec<_>>(),
        "expected": report.expected,
        "as_expected": report.as_expected,
        "failed": report.failed,
        "pending": report.pending,
        "pending_passing": report.pending_passing,
        "waiting": report.waiting.iter().map(waiting_json).collect::<Vec<_>>(),
        "not_run": report.not_run,
        "suite_cycles": report.suite_cycles,
    });
    if let (Some(object), Some(only)) = (document.as_object_mut(), &report.only) {
        object.insert("only".into(), json!(only));
    }
    document
}

/// The seconds of wall time one cycle's `tree run` rounds share.
pub const CYCLE_WALL_SECONDS: u64 = 60;

/// The principal a step or a scripted entry acts as when neither it, its cycle nor the printed
/// command names one.
pub const SUITE_PRINCIPAL: &str = "operator";

/// The root case id every cycle opens, as `tree open` without `--case`.
pub const SUITE_ROOT_CASE: &str = "root";

/// The detail of the refusal `SubstrateWorkersUnserved`.
pub const SUBSTRATE_WORKERS_UNSERVED: &str = "--workers above 1 under --confinement substrate \
waits for story:suite-runs-cycles-under-one-cgroup-root";

/// Called with each cycle's result in suite order, on the thread that called [`run_suite`]:
/// result `n` as soon as results `1..=n` have all ended, so a later result that ended first waits.
pub trait SuiteObserver {
    /// One cycle's result.
    fn cycle(&mut self, result: &CycleResult);
}

impl<F: FnMut(&CycleResult)> SuiteObserver for F {
    fn cycle(&mut self, result: &CycleResult) {
        self(result);
    }
}

/// How a suite runs: plain data, every path absolute.
#[derive(Clone)]
pub struct SuiteSettings {
    /// The directory whose `.yaml` files are the protocols every cycle's tree keeps, read once as
    /// `tree open --protocols` reads them.
    pub protocols: PathBuf,
    /// The directory the cycles work in: absent or empty.
    pub scratch: PathBuf,
    /// Where the steps read their instants: with [`TreeTime::At`], a step's `at` is minutes after
    /// it; with [`TreeTime::Host`], the host clock.
    pub time: TreeTime,
    /// The runner every bound program of every cycle starts through.
    pub runner: Arc<dyn TestRunner>,
    /// The `--confinement` value of the runner, which a printed handoff command carries.
    pub confinement: Option<String>,
    /// Which step kinds, sources, event kinds and members run; [`SUITE_AVAILABILITY`] by default.
    pub availability: Vec<AvailabilityEntry>,
    /// How many cycles run at once; 1 by default, which runs them one after another. More than
    /// one is refused `SubstrateWorkersUnserved` when `confinement` is `substrate`.
    pub workers: NonZeroUsize,
    /// The names of the cycles to run; empty by default, which runs every cycle. A name that is no
    /// cycle of the suite is refused `CycleUnknown`.
    pub only: Vec<String>,
    /// How long a cycle may run, from its start: past it, its running program is killed and it
    /// ends `CycleTimedOut`. [`CYCLE_WALL_SECONDS`] by default; `tree test --cycle-timeout` takes
    /// 1 to [`CYCLE_WALL_SECONDS`] seconds and refuses any other value.
    pub cycle_timeout: Duration,
}

impl SuiteSettings {
    /// Settings with [`SUITE_AVAILABILITY`], one worker, every cycle and a cycle timeout of
    /// [`CYCLE_WALL_SECONDS`].
    pub fn new(
        protocols: PathBuf,
        scratch: PathBuf,
        time: TreeTime,
        runner: Arc<dyn TestRunner>,
        confinement: Option<String>,
    ) -> Self {
        Self {
            protocols,
            scratch,
            time,
            runner,
            confinement,
            availability: SUITE_AVAILABILITY.to_vec(),
            workers: NonZeroUsize::MIN,
            only: Vec::new(),
            cycle_timeout: Duration::from_secs(CYCLE_WALL_SECONDS),
        }
    }
}

impl std::fmt::Debug for SuiteSettings {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("SuiteSettings")
            .field("protocols", &self.protocols)
            .field("scratch", &self.scratch)
            .field("time", &self.time)
            .field("runner", &self.runner.label())
            .field("confinement", &self.confinement)
            .field("availability", &self.availability.len())
            .field("workers", &self.workers)
            .field("only", &self.only)
            .field("cycle_timeout", &self.cycle_timeout)
            .finish()
    }
}

fn refused(reason: SuiteRefusalReason, member: Option<String>, detail: String) -> SuiteRefused {
    SuiteRefused::new(reason, member, detail)
}

/// `path` when it is absolute; else the refusal of `reason` naming `what`.
fn absolute_path<'a>(
    path: &'a Path,
    what: &str,
    reason: SuiteRefusalReason,
) -> Result<&'a Path, SuiteRefused> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(refused(
            reason,
            None,
            format!("{what} {} is not an absolute path", path.display()),
        ))
    }
}

/// `scratch` absent, or an empty directory that is no link.
fn vacant_scratch(scratch: &Path) -> Result<(), SuiteRefused> {
    let not_empty = |detail: String| refused(SuiteRefusalReason::ScratchNotEmpty, None, detail);
    match fs::symlink_metadata(scratch) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(not_empty(format!("{}: {error}", scratch.display()))),
        Ok(entry) if !entry.file_type().is_dir() => Err(not_empty(format!(
            "{} is not a directory",
            scratch.display()
        ))),
        Ok(_) => {
            let mut entries = fs::read_dir(scratch)
                .map_err(|error| not_empty(format!("{}: {error}", scratch.display())))?;
            match entries.next() {
                None => Ok(()),
                Some(_) => Err(not_empty(format!("{} is not empty", scratch.display()))),
            }
        }
    }
}

/// Every file below `path` (`path` itself when it is a file), or the first link or other entry
/// that is neither a regular file nor a directory.
fn seed_files(path: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    let entry = fs::symlink_metadata(path).map_err(|error| format!("{error}"))?;
    let kind = entry.file_type();
    if kind.is_symlink() {
        return Err("is a symbolic link".to_owned());
    }
    if kind.is_file() {
        found.push(path.to_owned());
        return Ok(());
    }
    if !kind.is_dir() {
        return Err("is neither a regular file nor a directory".to_owned());
    }
    let mut entries = fs::read_dir(path)
        .map_err(|error| format!("{error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{error}"))?;
    entries.sort();
    for entry in entries {
        seed_files(&entry, found).map_err(|error| {
            format!(
                "{} {error}",
                entry.strip_prefix(path).unwrap_or(&entry).display()
            )
        })?;
    }
    Ok(())
}

/// Every seed of every cycle names a file or a directory of the suite that holds no link.
fn usable_seeds(cycles: &[SuiteCycle], suite_dir: &Path) -> Result<(), SuiteRefused> {
    for (index, cycle) in cycles.iter().enumerate() {
        for (at, seed) in cycle.seed.iter().enumerate() {
            seed_files(&suite_dir.join(&seed.from), &mut Vec::new()).map_err(|error| {
                refused(
                    SuiteRefusalReason::SeedUnusable,
                    Some(format!("cycles[{index}].seed[{at}].from")),
                    format!("{}: {error}", seed.from),
                )
            })?;
        }
    }
    Ok(())
}

/// The most bytes the file a `write` step copies (its `from`) may hold.
pub const MAX_WRITE_BYTES: u64 = 1024 * 1024;

/// What a directory entry of `kind` is found as.
fn entry_kind(kind: fs::FileType) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt as _;
        if kind.is_fifo() {
            return "fifo";
        }
        if kind.is_socket() {
            return "socket";
        }
        if kind.is_block_device() || kind.is_char_device() {
            return "device";
        }
    }
    if kind.is_symlink() {
        "symbolic link"
    } else if kind.is_dir() {
        "directory"
    } else if kind.is_file() {
        "regular file"
    } else {
        "entry of another kind"
    }
}

/// The bytes of a `write` step's `from`, a path of `suite_dir` that stays in it: a regular file of
/// at most [`MAX_WRITE_BYTES`], with no link at it or at any directory between `suite_dir` and
/// it, opened without following a link. The error says what it is instead, never what it holds.
fn write_source(suite_dir: &Path, from: &str) -> Result<Vec<u8>, String> {
    let mut parts = Vec::new();
    for component in Path::new(from).components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    let Some((last, parents)) = parts.split_last() else {
        return Err("is the suite's directory".to_owned());
    };
    let mut path = suite_dir.to_path_buf();
    for part in parents {
        path.push(part);
        let entry = fs::symlink_metadata(&path).map_err(|error| error.kind().to_string())?;
        if !entry.file_type().is_dir() {
            return Err(format!("is behind a {}", entry_kind(entry.file_type())));
        }
    }
    path.push(last);
    let entry = fs::symlink_metadata(&path).map_err(|error| error.kind().to_string())?;
    if !entry.file_type().is_file() {
        return Err(format!(
            "is a {}, not a regular file",
            entry_kind(entry.file_type())
        ));
    }
    let over = || "is over 1 MiB".to_owned();
    if entry.len() > MAX_WRITE_BYTES {
        return Err(over());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let unreadable = |error: io::Error| error.kind().to_string();
    let file = options.open(&path).map_err(unreadable)?;
    if !file.metadata().map_err(unreadable)?.is_file() {
        return Err("is not a regular file".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(MAX_WRITE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_WRITE_BYTES {
        return Err(over());
    }
    Ok(bytes)
}

/// Every `write` step of every cycle copies a usable source ([`write_source`]).
fn usable_writes(cycles: &[SuiteCycle], suite_dir: &Path) -> Result<(), SuiteRefused> {
    for (index, cycle) in cycles.iter().enumerate() {
        for (at, step) in cycle.steps.iter().enumerate() {
            let (StepKind::Write, Some(from)) = (step.r#do, &step.from) else {
                continue;
            };
            write_source(suite_dir, from).map_err(|error| {
                refused(
                    SuiteRefusalReason::SeedUnusable,
                    Some(format!("cycles[{index}].steps[{at}].from")),
                    format!("{from}: {error}"),
                )
            })?;
        }
    }
    Ok(())
}

/// How many temporary files `write` steps of this process have named.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a new file `temporary` of mode 0644, never through a link, and renames it
/// over `target`, removing it when anything after its creation fails.
fn replace(temporary: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o644).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(temporary)?;
    let written = (|| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            file.set_permissions(fs::Permissions::from_mode(0o644))?;
        }
        file.write_all(bytes)?;
        drop(file);
        fs::rename(temporary, target)
    })();
    if written.is_err() {
        let _ = fs::remove_file(temporary);
    }
    written
}

/// `SubstrateWorkersUnserved` when `workers` is more than one and `confinement` is `substrate`:
/// Substrate keys its driver by a workspace's parent directory, and each cycle's workspace has a
/// parent of its own, so a second cycle would open a second driver under the root while the first
/// one's program runs.
///
/// # Errors
/// [`SuiteRefused`] `SubstrateWorkersUnserved`, saying [`SUBSTRATE_WORKERS_UNSERVED`].
pub fn workers_served(
    confinement: Option<&str>,
    workers: NonZeroUsize,
) -> Result<(), SuiteRefused> {
    if workers.get() > 1 && confinement == Some("substrate") {
        Err(refused(
            SuiteRefusalReason::SubstrateWorkersUnserved,
            None,
            SUBSTRATE_WORKERS_UNSERVED.to_owned(),
        ))
    } else {
        Ok(())
    }
}

/// The positions of the cycles `only` names, in suite order; every position when it is empty.
fn selected(cycles: &[SuiteCycle], only: &[String]) -> Result<Vec<usize>, SuiteRefused> {
    if let Some(unknown) = only
        .iter()
        .find(|name| !cycles.iter().any(|cycle| &cycle.name == *name))
    {
        return Err(refused(
            SuiteRefusalReason::CycleUnknown,
            None,
            format!("--only names {unknown}, which is no cycle of the suite"),
        ));
    }
    Ok(cycles
        .iter()
        .enumerate()
        .filter(|(_, cycle)| only.is_empty() || only.contains(&cycle.name))
        .map(|(index, _)| index)
        .collect())
}

/// Runs the suite file `suite` (see the module documentation): refuses more than one worker under
/// `substrate` before it reads anything, reads and decodes the suite, refuses an `only` name that
/// is no cycle of it, refuses a `scratch` that is not absent or empty before the protocols are
/// read, reads the protocols once, checks every seed and every `write` step's source, then runs
/// the selected cycles on up to `settings.workers` threads. A cycle that uses a part
/// `settings.availability` marks waiting is pending and not run; one whose `when` fails is not
/// run. `observer` is given each result in
/// suite order (see [`SuiteObserver`]); `cancel`, when set, starts no further cycle and ends each
/// running one, its running program killed or before its next step, its difference saying so. A
/// cycle that runs past `settings.cycle_timeout` ends the same way with `CycleTimedOut`.
///
/// # Errors
/// [`SuiteRefused`], before any cycle: `SubstrateWorkersUnserved`, what [`read_suite`] refuses,
/// `CycleUnknown`, `ScratchNotEmpty`, `ProtocolsUnreadable` and `SeedUnusable` (a seed, or a
/// `write` step's `from`, naming `cycles[<i>].steps[<j>].from`); a relative
/// `suite`, `protocols` or `scratch` is `SuiteUnreadable`, `ProtocolsUnreadable` or
/// `ScratchNotEmpty`.
pub fn run_suite(
    suite: &Path,
    settings: &SuiteSettings,
    observer: Option<&mut dyn SuiteObserver>,
    cancel: Option<&AtomicBool>,
) -> Result<SuiteReport, SuiteRefused> {
    workers_served(settings.confinement.as_deref(), settings.workers)?;
    let suite = absolute_path(suite, "the suite", SuiteRefusalReason::SuiteUnreadable)?;
    let decoded = read_suite(suite)?;
    let chosen = selected(&decoded.cycles, &settings.only)?;
    let suite_dir = suite.parent().unwrap_or(suite);
    let scratch = absolute_path(
        &settings.scratch,
        "--scratch",
        SuiteRefusalReason::ScratchNotEmpty,
    )?;
    vacant_scratch(scratch)?;
    let protocols = absolute_path(
        &settings.protocols,
        "--protocols",
        SuiteRefusalReason::ProtocolsUnreadable,
    )?;
    let directory = read_protocols_directory(protocols).map_err(|refusal| {
        refused(
            SuiteRefusalReason::ProtocolsUnreadable,
            None,
            refusal.to_string(),
        )
    })?;
    usable_seeds(&decoded.cycles, suite_dir)?;
    usable_writes(&decoded.cycles, suite_dir)?;
    fs::create_dir_all(scratch).map_err(|error| {
        refused(
            SuiteRefusalReason::ScratchNotEmpty,
            None,
            format!("{}: {error}", scratch.display()),
        )
    })?;
    let suite_run = SuiteRun {
        suite_dir,
        scratch,
        catalog: &directory.catalog,
        kept: &directory.kept,
        settings,
        cancel,
    };
    let results = suite_run.cycles(&decoded.cycles, &chosen, observer);
    let mut report = summarize(results);
    report.suite_cycles = i64::try_from(decoded.cycles.len()).unwrap_or(i64::MAX);
    if !settings.only.is_empty() {
        report.only = Some(
            chosen
                .iter()
                .map(|&index| decoded.cycles[index].name.clone())
                .collect(),
        );
    }
    Ok(report)
}

/// What every cycle of one suite run shares.
struct SuiteRun<'a> {
    suite_dir: &'a Path,
    scratch: &'a Path,
    catalog: &'a ProtocolCatalog,
    kept: &'a [KeptProtocol],
    settings: &'a SuiteSettings,
    cancel: Option<&'a AtomicBool>,
}

/// The one watcher of a suite run: the flag and deadline of each running cycle, and how many
/// workers are still running.
struct Watcher {
    watched: Mutex<Watched>,
    changed: Condvar,
}

struct Watched {
    /// Each running cycle's position among the chosen, its flag and its deadline.
    cycles: Vec<(usize, Arc<AtomicBool>, Instant)>,
    workers: usize,
}

impl Watcher {
    fn new(workers: usize) -> Self {
        Self {
            watched: Mutex::new(Watched {
                cycles: Vec::new(),
                workers,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Watched> {
        self.watched.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds the cycle at position `at` with its `flag`, which is set at `deadline`.
    fn hold(&self, at: usize, flag: Arc<AtomicBool>, deadline: Instant) {
        self.lock().cycles.push((at, flag, deadline));
    }

    /// Takes the cycle at position `at` back.
    fn release(&self, at: usize) {
        self.lock().cycles.retain(|(held, _, _)| *held != at);
    }

    /// One worker ended; the last one ends the watch.
    fn worker_ended(&self) {
        let mut watched = self.lock();
        watched.workers = watched.workers.saturating_sub(1);
        drop(watched);
        self.changed.notify_all();
    }

    /// Every [`CANCEL_WATCH`] until the last worker ended: sets the flag of each held cycle past
    /// its deadline, and of every held cycle once `cancelled` answers true.
    fn watch(&self, cancelled: impl Fn() -> bool) {
        let mut watched = self.lock();
        while watched.workers > 0 {
            let suite = cancelled();
            let now = Instant::now();
            for (_, flag, deadline) in &watched.cycles {
                if suite || now >= *deadline {
                    flag.store(true, Ordering::SeqCst);
                }
            }
            watched = self
                .changed
                .wait_timeout(watched, CANCEL_WATCH)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// Tells the watcher its worker ended, however the worker ends, a panic included.
struct WorkerEnd<'a>(&'a Watcher);

impl Drop for WorkerEnd<'_> {
    fn drop(&mut self) {
        self.0.worker_ended();
    }
}

/// What one step ended with.
struct StepEnd {
    exit: i64,
    refusal: Option<String>,
    output: Option<Value>,
}

impl StepEnd {
    fn done(output: Option<Value>) -> Self {
        Self {
            exit: 0,
            refusal: None,
            output,
        }
    }

    fn refused(refusal: &Refused) -> Self {
        Self {
            exit: 1,
            refusal: Some(format!("{:?}", refusal.reason())),
            output: None,
        }
    }
}

/// The decision an `approve` step or entry names: `Deny` for `deny: true`, else `Allow`.
fn decision_of(deny: Option<bool>) -> ApprovalDecision {
    if deny == Some(true) {
        ApprovalDecision::Deny
    } else {
        ApprovalDecision::Allow
    }
}

/// The `reasons` of an `approve` step or entry as the one text `tree approve --reasons` takes:
/// the entries joined by line feeds, so the 60-line bound counts the entries and the lines inside
/// them. `tree approve` bounds and refuses it as it stands.
fn joined_reasons(reasons: Option<&[String]>) -> Option<String> {
    reasons.map(|reasons| reasons.join("\n"))
}

/// One cycle as it runs.
struct Cycle<'a> {
    cycle: &'a SuiteCycle,
    work: PathBuf,
    store: PathBuf,
    started: Instant,
    /// The cycle's own flag: set by the watcher at its deadline or when the suite is cancelled;
    /// handed to `run_tree` and read before each step.
    flag: Arc<AtomicBool>,
    outputs: BTreeMap<String, Value>,
    captured: Captured,
    uses: Vec<i64>,
    last_run: Option<TreeRun>,
    run_exit: Option<i64>,
}

fn difference(
    check: impl Into<String>,
    kind: &str,
    reason: DifferenceReason,
    expected: impl Into<String>,
    found: Option<String>,
) -> CheckDifference {
    CheckDifference {
        check: check.into(),
        kind: kind.to_owned(),
        reason,
        expected: expected.into(),
        found,
    }
}

/// The exit status `tree run` ends with for `stop`.
fn stop_exit(stop: TreeStop) -> i64 {
    match stop {
        TreeStop::Terminal => 0,
        TreeStop::AwaitingPerson | TreeStop::AwaitingAuthority => 3,
        TreeStop::Stuck => 4,
        TreeStop::BudgetSpent | TreeStop::Cancelled => 5,
    }
}

fn exit_text(expected: &ExitExpectation) -> String {
    match (expected.status, expected.not) {
        (Some(status), _) => status.to_string(),
        (None, Some(not)) => format!("not {not}"),
        (None, None) => "0".to_owned(),
    }
}

fn exit_holds(expected: &ExitExpectation, exit: i64) -> bool {
    match (expected.status, expected.not) {
        (Some(status), _) => exit == status,
        (None, Some(not)) => exit != not,
        (None, None) => exit == 0,
    }
}

/// `at` (`YYYY-MM-DDTHH:MM:SSZ`) plus `minutes`, in the same form; `None` when `at` is not one.
fn instant_after(at: &str, minutes: i64) -> Option<String> {
    let bytes = at.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[10] != b'T' || bytes[19] != b'Z' {
        return None;
    }
    let number = |from: usize, to: usize| at.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    // Days from the civil date (Howard Hinnant's algorithm), proleptic Gregorian.
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let of_era = shifted - era * 400;
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second + minutes * 60;
    let (days, of_day) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let moved = days + 719_468;
    let era = moved.div_euclid(146_097);
    let of_era = moved - era * 146_097;
    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * of_year + 2) / 153;
    let day = of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60
    ))
}

/// Whether the scripted entry's `case` names `case`: its id, or the last part of it.
fn names_case(wanted: Option<&String>, case: &str) -> bool {
    wanted.is_none_or(|wanted| {
        case == wanted.as_str()
            || case
                .strip_suffix(wanted.as_str())
                .is_some_and(|head| head.ends_with('.'))
    })
}

/// The word after `--principal` in a printed command.
fn printed_principal(decision: &NextDecision) -> Option<&str> {
    decision
        .command
        .iter()
        .position(|word| word == "--principal")
        .and_then(|at| decision.command.get(at + 1))
        .map(String::as_str)
}

fn uses_left(entry: &ScriptedDecision, used: i64) -> bool {
    let times = match entry {
        ScriptedDecision::Approve(entry) => &entry.times,
        ScriptedDecision::Close(entry) => &entry.times,
        ScriptedDecision::Decide(entry) => &entry.times,
    };
    times.any || times.count.is_none_or(|count| used < count)
}

fn optional(entry: &ScriptedDecision) -> bool {
    match entry {
        ScriptedDecision::Approve(entry) => entry.optional,
        ScriptedDecision::Close(entry) => entry.optional,
        ScriptedDecision::Decide(entry) => entry.optional,
    }
}

fn take_name(entry: &ScriptedDecision) -> &'static str {
    match entry {
        ScriptedDecision::Approve(_) => "approve",
        ScriptedDecision::Close(_) => "close",
        ScriptedDecision::Decide(_) => "decide",
    }
}

impl SuiteRun<'_> {
    fn cancelled(&self) -> bool {
        self.cancel
            .is_some_and(|cancel| cancel.load(Ordering::SeqCst))
    }

    /// The difference the cycle ends with at the step `label` of `kind` when a flag stops it: the
    /// suite's (`StepFailed`, the suite was cancelled), else its own, set at its deadline
    /// (`CycleTimedOut`, found the seconds it ran); `None` when neither is set.
    fn interrupted(&self, running: &Cycle<'_>, label: &str, kind: &str) -> Option<CheckDifference> {
        if self.cancelled() {
            Some(difference(
                label,
                kind,
                DifferenceReason::StepFailed,
                "the step runs",
                Some("the suite was cancelled".to_owned()),
            ))
        } else if running.flag.load(Ordering::SeqCst) {
            Some(difference(
                label,
                kind,
                DifferenceReason::CycleTimedOut,
                format!("an end within {} s", self.settings.cycle_timeout.as_secs()),
                Some(format!("{:.1}", running.started.elapsed().as_secs_f64())),
            ))
        } else {
            None
        }
    }

    /// The cycles at the positions `chosen` names, on up to `settings.workers` threads, each
    /// thread taking the next of them in suite order until none is left or the cancel flag is set,
    /// beside one watcher thread that sets a running cycle's flag at its deadline (see the module
    /// documentation). The results come back in suite order, and `observer` is given each on this
    /// thread as soon as every result before it has ended; a cycle that never started has none.
    fn cycles(
        &self,
        cycles: &[SuiteCycle],
        chosen: &[usize],
        mut observer: Option<&mut dyn SuiteObserver>,
    ) -> Vec<CycleResult> {
        let threads = self.settings.workers.get().min(chosen.len());
        let next = AtomicUsize::new(0);
        let watcher = Watcher::new(threads);
        let mut ended: Vec<Option<CycleResult>> = vec![None; chosen.len()];
        let mut observed = 0;
        let mut observe = |result: &CycleResult| {
            if let Some(observer) = observer.as_deref_mut() {
                observer.cycle(result);
            }
        };
        std::thread::scope(|scope| {
            let (sender, receiver) = mpsc::channel();
            let watcher = &watcher;
            if threads > 0 {
                scope.spawn(move || watcher.watch(|| self.cancelled()));
            }
            for _ in 0..threads {
                let sender = sender.clone();
                let next = &next;
                scope.spawn(move || {
                    let _ended = WorkerEnd(watcher);
                    while !self.cancelled() {
                        let at = next.fetch_add(1, Ordering::SeqCst);
                        let Some(&index) = chosen.get(at) else {
                            break;
                        };
                        if self.cancelled() {
                            break;
                        }
                        let result = self.result(index, &cycles[index], at, watcher);
                        if sender.send((at, result)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(sender);
            for (at, result) in receiver {
                ended[at] = Some(result);
                while let Some(Some(result)) = ended.get(observed) {
                    observe(result);
                    observed += 1;
                }
            }
        });
        for result in ended.iter().skip(observed).flatten() {
            observe(result);
        }
        ended.into_iter().flatten().collect()
    }

    /// The result of `cycle`, the `index`th of the suite and the `at`th chosen: waiting when it
    /// uses a part that waits, not run when a `when` fails, else run under `watcher`.
    fn result(
        &self,
        index: usize,
        cycle: &SuiteCycle,
        at: usize,
        watcher: &Watcher,
    ) -> CycleResult {
        if let Some(waiting) = first_waiting(cycle, &self.settings.availability) {
            waiting_result(cycle, waiting)
        } else if let Some(failed) = cycle
            .when
            .iter()
            .position(|condition| !when_holds(condition, self.catalog, self.suite_dir))
        {
            not_run_result(cycle, failed)
        } else {
            self.cycle(index, cycle, at, watcher)
        }
    }

    /// Runs `cycle`, the `index`th of the suite and the `at`th chosen, to its result: the watcher
    /// holds its flag and deadline from its start until its steps and `expect` ended.
    fn cycle(&self, index: usize, cycle: &SuiteCycle, at: usize, watcher: &Watcher) -> CycleResult {
        let root = self.scratch.join((index + 1).to_string());
        let started = Instant::now();
        let flag = Arc::new(AtomicBool::new(false));
        let deadline = started
            .checked_add(self.settings.cycle_timeout)
            .unwrap_or(started);
        watcher.hold(at, Arc::clone(&flag), deadline);
        let mut running = Cycle {
            cycle,
            work: root.join("work"),
            store: root.join("store"),
            started,
            flag,
            outputs: BTreeMap::new(),
            captured: Captured::new(),
            uses: vec![0; cycle.decide.len()],
            last_run: None,
            run_exit: None,
        };
        let difference = self.steps(&mut running).err();
        watcher.release(at);
        let seconds = running.started.elapsed().as_secs_f64();
        CycleResult {
            name: cycle.name.clone(),
            pending: cycle.pending,
            waits_for: None,
            run: true,
            not_run_when: None,
            as_expected: difference.is_none(),
            difference,
            exit: running.run_exit,
            seconds: Decimal(format!("{seconds:.1}")),
        }
    }

    fn view<'v>(&'v self, running: &'v Cycle<'_>) -> CycleView<'v> {
        CycleView {
            work: &running.work,
            store: &running.store,
            suite: self.suite_dir,
            steps: &running.outputs,
            catalog: self.catalog,
        }
    }

    /// The seeds, every step, the scripted entries' uses, then `expect` and `tree verify`: the
    /// first difference.
    fn steps(&self, running: &mut Cycle<'_>) -> Result<(), CheckDifference> {
        let cycle = running.cycle;
        self.seed(running)?;
        let last_run = cycle
            .steps
            .iter()
            .rposition(|step| step.r#do == StepKind::Run);
        let expected_exit = cycle
            .expect
            .exit
            .or_else(|| cycle.expect.stop.as_ref().map(|stop| stop_exit(stop.stop)))
            .unwrap_or(0);
        let mut minute = -1i64;
        for (index, step) in cycle.steps.iter().enumerate() {
            let label = step
                .name
                .clone()
                .unwrap_or_else(|| format!("steps[{index}]"));
            if let Some(stopped) = self.interrupted(running, &label, step_name(step.r#do)) {
                return Err(stopped);
            }
            if step.r#do != StepKind::Write {
                minute = step.at.unwrap_or(minute + 1);
            }
            let time = match &self.settings.time {
                TreeTime::At(at) => {
                    TreeTime::At(instant_after(at, minute).unwrap_or_else(|| at.clone()))
                }
                TreeTime::Host => TreeTime::Host,
            };
            let ended = self.step(running, step, &label, &time)?;
            if step.r#do == StepKind::Run {
                running.run_exit = Some(ended.exit);
            }
            let expected = match &step.exit {
                Some(exit) => exit.clone(),
                None => ExitExpectation {
                    status: Some(if Some(index) == last_run {
                        expected_exit
                    } else if step.refusal.is_some() {
                        1
                    } else {
                        0
                    }),
                    not: None,
                },
            };
            if !exit_holds(&expected, ended.exit) {
                let found = match &ended.refusal {
                    Some(refusal) => format!("{}, refused {refusal}", ended.exit),
                    None => ended.exit.to_string(),
                };
                return Err(difference(
                    format!("{label}.exit"),
                    "exit",
                    DifferenceReason::ExitDiffers,
                    exit_text(&expected),
                    Some(found),
                ));
            }
            if let Some(refusal) = &step.refusal
                && ended.refusal.as_ref() != Some(refusal)
            {
                return Err(difference(
                    format!("{label}.refusal"),
                    "refusal",
                    DifferenceReason::RefusalDiffers,
                    refusal.clone(),
                    Some(ended.refusal.unwrap_or_else(|| "no refusal".to_owned())),
                ));
            }
            if let (Some(name), Some(output)) = (&step.name, ended.output) {
                running.outputs.insert(name.clone(), output);
            }
            let view = self.view(running);
            if let Some(difference) = evaluate_checks(
                &step.checks,
                &format!("steps[{index}]"),
                &view,
                &running.captured,
            ) {
                return Err(difference);
            }
            if let Some(name) = &step.name {
                let mut captured = std::mem::take(&mut running.captured);
                let taken = capture_after(name, cycle, &self.view(running), &mut captured);
                running.captured = captured;
                taken?;
            }
        }
        for (index, entry) in cycle.decide.iter().enumerate() {
            if running.uses[index] == 0 && !optional(entry) {
                return Err(difference(
                    format!("decide[{index}]"),
                    "decide",
                    DifferenceReason::ScriptUnused,
                    format!("the {} entry is used", take_name(entry)),
                    Some("never used".to_owned()),
                ));
            }
        }
        self.expect(running)
    }

    /// Copies each seed into the cycle's workspace.
    fn seed(&self, running: &Cycle<'_>) -> Result<(), CheckDifference> {
        fs::create_dir_all(&running.work).map_err(|error| {
            difference(
                "work",
                "seed",
                DifferenceReason::StepFailed,
                "the workspace is made",
                Some(error.kind().to_string()),
            )
        })?;
        for (index, seed) in running.cycle.seed.iter().enumerate() {
            let unusable = |detail: String| {
                difference(
                    format!("seed[{index}]"),
                    "seed",
                    DifferenceReason::SeedUnusable,
                    "a file or directory of the suite without a link",
                    Some(detail),
                )
            };
            let from = self.suite_dir.join(&seed.from);
            let mut files = Vec::new();
            seed_files(&from, &mut files).map_err(|_| unusable("unusable".to_owned()))?;
            let to = running.work.join(&seed.to);
            for file in files {
                let target = match file.strip_prefix(&from) {
                    Ok(relative) if relative.as_os_str().is_empty() => to.clone(),
                    Ok(relative) => to.join(relative),
                    Err(_) => return Err(unusable("outside the seed".to_owned())),
                };
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| unusable(error.kind().to_string()))?;
                }
                fs::copy(&file, &target).map_err(|error| unusable(error.kind().to_string()))?;
            }
        }
        Ok(())
    }

    /// The root case of the cycle's tree: the manifest's, else [`SUITE_ROOT_CASE`].
    fn root(&self, running: &Cycle<'_>) -> String {
        read_manifest(&running.store)
            .map(|manifest| manifest.root)
            .unwrap_or_else(|_| SUITE_ROOT_CASE.to_owned())
    }

    fn principal<'s>(&self, running: &'s Cycle<'_>, own: Option<&'s String>) -> &'s str {
        own.or(running.cycle.principal.as_ref())
            .map_or(SUITE_PRINCIPAL, String::as_str)
    }

    /// A `write` step: the bytes of `from` read again; each directory of `to` below the workspace
    /// a directory and no link, a missing one made; `to` absent or a regular file; then the bytes
    /// to a new file beside it, renamed over it. A link at any component of `to` is
    /// `PathOutsideCycle`; anything else that stops it is `StepFailed`, found as the entry's kind
    /// or the error's, never as the content. Nothing reaches the run log or the store.
    fn write(
        &self,
        running: &Cycle<'_>,
        step: &CycleStep,
        label: &str,
    ) -> Result<(), CheckDifference> {
        let failed = |reason: DifferenceReason, found: String| {
            difference(label, "write", reason, "the file is written", Some(found))
        };
        let (Some(from), Some(to)) = (&step.from, &step.to) else {
            return Err(failed(
                DifferenceReason::StepFailed,
                "no from or to".to_owned(),
            ));
        };
        let bytes = write_source(self.suite_dir, from)
            .map_err(|found| failed(DifferenceReason::StepFailed, format!("from {found}")))?;
        let parts: Vec<&str> = to.split('/').collect();
        let Some((last, parents)) = parts.split_last() else {
            return Err(failed(DifferenceReason::StepFailed, "no to".to_owned()));
        };
        let link = || {
            failed(
                DifferenceReason::PathOutsideCycle,
                "a symbolic link".to_owned(),
            )
        };
        let error =
            |error: io::Error| failed(DifferenceReason::StepFailed, error.kind().to_string());
        let mut dir = running.work.clone();
        for part in parents {
            dir.push(part);
            match fs::symlink_metadata(&dir) {
                Ok(entry) if entry.file_type().is_symlink() => return Err(link()),
                Ok(entry) if entry.file_type().is_dir() => {}
                Ok(entry) => {
                    return Err(failed(
                        DifferenceReason::StepFailed,
                        entry_kind(entry.file_type()).to_owned(),
                    ));
                }
                Err(missing) if missing.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&dir).map_err(error)?;
                }
                Err(other) => return Err(error(other)),
            }
        }
        let target = dir.join(last);
        match fs::symlink_metadata(&target) {
            Ok(entry) if entry.file_type().is_symlink() => return Err(link()),
            Ok(entry) if entry.file_type().is_file() => {}
            Ok(entry) => {
                return Err(failed(
                    DifferenceReason::StepFailed,
                    entry_kind(entry.file_type()).to_owned(),
                ));
            }
            Err(missing) if missing.kind() == io::ErrorKind::NotFound => {}
            Err(other) => return Err(error(other)),
        }
        let temporary = dir.join(format!(
            ".loom-write-{}-{}",
            std::process::id(),
            WRITES.fetch_add(1, Ordering::SeqCst)
        ));
        replace(&temporary, &target, &bytes).map_err(error)
    }

    /// Runs one step through its tree function.
    fn step(
        &self,
        running: &mut Cycle<'_>,
        step: &CycleStep,
        label: &str,
        time: &TreeTime,
    ) -> Result<StepEnd, CheckDifference> {
        let cycle = running.cycle;
        Ok(match step.r#do {
            StepKind::Open => match open_tree(&OpenRequest {
                store: running.store.clone(),
                workspace: running.work.clone(),
                protocol: cycle.protocol.clone().unwrap_or_default(),
                bindings: self
                    .suite_dir
                    .join(cycle.bindings.as_deref().unwrap_or_default()),
                revisions: cycle
                    .revisions
                    .iter()
                    .map(|revision| (revision.artifact.clone(), revision.revision.clone()))
                    .collect(),
                case: SUITE_ROOT_CASE.to_owned(),
                time: time.clone(),
                catalog: self.catalog,
                protocols: self.kept.to_vec(),
            }) {
                Ok(_) => StepEnd::done(None),
                Err(refusal) => StepEnd::refused(&refusal),
            },
            StepKind::Run => self.run(running, step, label, time)?,
            StepKind::Approve => {
                let case = step.case.clone().unwrap_or_else(|| self.root(running));
                let reasons = joined_reasons(step.reasons.as_deref());
                match crate::approve(&ApproveRequest {
                    store: &running.store,
                    case: &case,
                    capability: step.capability.as_deref(),
                    principal: self.principal(running, step.principal.as_ref()),
                    decision: decision_of(step.deny),
                    action: step.action.as_deref(),
                    result: step.result.as_deref(),
                    reasons: reasons.as_deref().map(Reasons::Text),
                    time: time.clone(),
                    catalog: self.catalog,
                    runner: Some(Arc::clone(&self.settings.runner)),
                }) {
                    Ok(_) => StepEnd::done(None),
                    Err(refusal) => StepEnd::refused(&refusal),
                }
            }
            StepKind::Close => {
                let Some(outcome) = &step.outcome else {
                    return Err(difference(
                        label,
                        "close",
                        DifferenceReason::StepFailed,
                        "the step names its outcome",
                        Some("no outcome".to_owned()),
                    ));
                };
                let case = step.case.clone().unwrap_or_else(|| self.root(running));
                match crate::close(&CloseRequest {
                    store: &running.store,
                    case: &case,
                    outcome,
                    time: time.clone(),
                    catalog: self.catalog,
                }) {
                    Ok(_) => StepEnd::done(None),
                    Err(refusal) => StepEnd::refused(&refusal),
                }
            }
            StepKind::Status => match tree_status(&running.store, self.catalog) {
                Ok(status) => StepEnd::done(Some(status_json(&status))),
                Err(refusal) => StepEnd::refused(&refusal),
            },
            StepKind::Verify => match verify_tree(&running.store, self.catalog) {
                Ok(verified) => StepEnd {
                    exit: if verified.failure.is_some() { 3 } else { 0 },
                    refusal: None,
                    output: Some(verify_json(&verified)),
                },
                Err(refusal) => StepEnd::refused(&refusal),
            },
            StepKind::Stop => {
                let reason = step
                    .reason
                    .clone()
                    .unwrap_or_else(|| format!("suite step {label}"));
                match crate::hold(&HoldRequest {
                    store: &running.store,
                    principal: self.principal(running, step.principal.as_ref()),
                    reason: &reason,
                    time: time.clone(),
                }) {
                    Ok(_) => StepEnd::done(None),
                    Err(refusal) => StepEnd::refused(&refusal),
                }
            }
            StepKind::Resume => self.resume(running, label, time)?,
            StepKind::Reopen => match crate::reopen(&ReopenRequest {
                store: &running.store,
                principal: self.principal(running, step.principal.as_ref()),
                time: time.clone(),
            }) {
                Ok(_) => StepEnd::done(None),
                Err(refusal) => StepEnd::refused(&refusal),
            },
            StepKind::Write => {
                self.write(running, step, label)?;
                StepEnd::done(None)
            }
            // As `tree replay`: 3 when a compared case differs, its object the step's output
            // (`story:tree-replay-command`).
            StepKind::Replay => {
                match replay_tree(&running.store, self.catalog, step.case.as_deref()) {
                    Ok(replayed) => StepEnd {
                        exit: if replay_difference(&replayed).is_some() {
                            3
                        } else {
                            0
                        },
                        refusal: None,
                        output: Some(replay_json(&replayed)),
                    },
                    Err(refusal) => StepEnd::refused(&refusal),
                }
            }
            other @ (StepKind::Decide | StepKind::Allow) => {
                return Err(difference(
                    label,
                    step_name(other),
                    DifferenceReason::StepFailed,
                    "a step kind this runner runs",
                    Some(step_name(other).to_owned()),
                ));
            }
        })
    }

    /// The settings of one round of `tree run` in `running`: no step bound, the wall time the
    /// cycle has left, the suite's runner and confinement.
    fn round_settings(&self, running: &Cycle<'_>, time: &TreeTime) -> RunSettings {
        let spent = running.started.elapsed().as_secs();
        RunSettings {
            max_steps: None,
            max_wall_seconds: Some(
                i64::try_from(CYCLE_WALL_SECONDS.saturating_sub(spent)).unwrap_or(0),
            ),
            time: time.clone(),
            runner: Arc::clone(&self.settings.runner),
            confinement: self.settings.confinement.clone(),
            cgroup_root: None,
        }
    }

    /// `resume`: exactly one `tree run`, as one round of a `run` step, holding the cycle's cancel
    /// flag and answering no wait. Its exit is the run's; a refused run (a held tree's
    /// `TreeStopped`) is exit 1 with the refusal.
    fn resume(
        &self,
        running: &mut Cycle<'_>,
        label: &str,
        time: &TreeTime,
    ) -> Result<StepEnd, CheckDifference> {
        let settings = self.round_settings(running, time);
        let ran = match run_tree(
            &running.store,
            self.catalog,
            &settings,
            None,
            Some(&running.flag),
        ) {
            Ok(ran) => ran,
            Err(refusal) => return Ok(StepEnd::refused(&refusal)),
        };
        let exit = stop_exit(ran.stop);
        let stopped = if ran.stop == TreeStop::Cancelled {
            self.interrupted(running, label, "resume")
        } else {
            None
        };
        running.last_run = Some(ran);
        if let Some(stopped) = stopped {
            return Err(stopped);
        }
        Ok(StepEnd {
            exit,
            refusal: None,
            output: None,
        })
    }

    /// Rounds of `tree run`, each exit 3 answered from the scripted entries, until another exit,
    /// a wait no entry answers, or the rounds the step or the cycle allows.
    fn run(
        &self,
        running: &mut Cycle<'_>,
        step: &CycleStep,
        label: &str,
        time: &TreeTime,
    ) -> Result<StepEnd, CheckDifference> {
        let limit = step.rounds.unwrap_or(running.cycle.max_rounds).max(1);
        let mut round = 0i64;
        loop {
            if round > 0
                && let Some(stopped) = self.interrupted(running, label, "run")
            {
                return Err(stopped);
            }
            let settings = self.round_settings(running, time);
            let ran = match run_tree(
                &running.store,
                self.catalog,
                &settings,
                None,
                Some(&running.flag),
            ) {
                Ok(ran) => ran,
                Err(refusal) => return Ok(StepEnd::refused(&refusal)),
            };
            round += 1;
            let exit = stop_exit(ran.stop);
            if ran.stop == TreeStop::Cancelled
                && let Some(stopped) = self.interrupted(running, label, "run")
            {
                running.run_exit = Some(exit);
                running.last_run = Some(ran);
                return Err(stopped);
            }
            let decisions = ran.decisions.clone();
            running.last_run = Some(ran);
            if exit != 3
                || (step.rounds.is_some() && round >= limit)
                || !self.answer(running, &decisions, time)?
            {
                return Ok(StepEnd {
                    exit,
                    refusal: None,
                    output: None,
                });
            }
            if round >= limit {
                return Err(difference(
                    label,
                    "run",
                    DifferenceReason::RoundsSpent,
                    format!("an end within {limit} rounds"),
                    Some(format!("{round} rounds")),
                ));
            }
        }
    }

    /// Answers each printed wait, in printed order, from the first scripted entry, in list order,
    /// that matches it and has a use left: whether every wait was answered.
    fn answer(
        &self,
        running: &mut Cycle<'_>,
        decisions: &[NextDecision],
        time: &TreeTime,
    ) -> Result<bool, CheckDifference> {
        let cycle = running.cycle;
        let mut all = true;
        let mut closed = BTreeSet::new();
        for decision in decisions {
            match decision.kind {
                DecisionKind::Approve | DecisionKind::ApproveAction => {
                    let found = cycle.decide.iter().enumerate().find(|(index, entry)| {
                        let ScriptedDecision::Approve(entry) = entry else {
                            return false;
                        };
                        uses_left(&cycle.decide[*index], running.uses[*index])
                            && names_case(entry.case.as_ref(), &decision.case)
                            && entry.capability.as_ref().is_none_or(|capability| {
                                decision.capability.as_ref() == Some(capability)
                            })
                            && entry
                                .action
                                .as_ref()
                                .is_none_or(|action| decision.action.as_ref() == Some(action))
                    });
                    let Some((index, ScriptedDecision::Approve(entry))) = found else {
                        all = false;
                        continue;
                    };
                    let principal = entry
                        .principal
                        .as_deref()
                        .or(cycle.principal.as_deref())
                        .or_else(|| printed_principal(decision))
                        .unwrap_or(SUITE_PRINCIPAL);
                    let action = match decision.kind {
                        DecisionKind::ApproveAction => decision.action.as_deref(),
                        _ => None,
                    };
                    let reasons = joined_reasons(entry.reasons.as_deref());
                    crate::approve(&ApproveRequest {
                        store: &running.store,
                        case: &decision.case,
                        capability: decision.capability.as_deref(),
                        principal,
                        decision: decision_of(entry.deny),
                        action,
                        result: entry.result.as_deref(),
                        reasons: reasons.as_deref().map(Reasons::Text),
                        time: time.clone(),
                        catalog: self.catalog,
                        runner: Some(Arc::clone(&self.settings.runner)),
                    })
                    .map_err(|refusal| {
                        difference(
                            format!("decide[{index}]"),
                            "decide",
                            DifferenceReason::StepFailed,
                            "the approval is recorded",
                            Some(format!("{:?}", refusal.reason())),
                        )
                    })?;
                    running.uses[index] += 1;
                }
                DecisionKind::Close => {
                    if !closed.insert(decision.case.clone()) {
                        continue;
                    }
                    let outcomes: Vec<&String> = decisions
                        .iter()
                        .filter(|other| {
                            other.kind == DecisionKind::Close && other.case == decision.case
                        })
                        .filter_map(|other| other.outcome.as_ref())
                        .collect();
                    let found = cycle.decide.iter().enumerate().find(|(index, entry)| {
                        let ScriptedDecision::Close(entry) = entry else {
                            return false;
                        };
                        uses_left(&cycle.decide[*index], running.uses[*index])
                            && names_case(entry.case.as_ref(), &decision.case)
                            && outcomes.contains(&&entry.outcome)
                    });
                    let Some((index, ScriptedDecision::Close(entry))) = found else {
                        all = false;
                        continue;
                    };
                    crate::close(&CloseRequest {
                        store: &running.store,
                        case: &decision.case,
                        outcome: &entry.outcome,
                        time: time.clone(),
                        catalog: self.catalog,
                    })
                    .map_err(|refusal| {
                        difference(
                            format!("decide[{index}]"),
                            "decide",
                            DifferenceReason::StepFailed,
                            "the case is closed",
                            Some(format!("{:?}", refusal.reason())),
                        )
                    })?;
                    running.uses[index] += 1;
                }
                DecisionKind::Handoff => all = false,
            }
        }
        Ok(all)
    }

    /// `expect`: the root's outcome, the last stop, the checks, then `tree verify`.
    fn expect(&self, running: &Cycle<'_>) -> Result<(), CheckDifference> {
        let expect = &running.cycle.expect;
        if let Some(outcome) = &expect.outcome {
            let found = tree_status(&running.store, self.catalog)
                .ok()
                .and_then(|status| {
                    status
                        .cases
                        .iter()
                        .find(|case| case.case == status.root)
                        .and_then(|root| root.outcome.clone())
                });
            if found.as_ref() != Some(outcome) {
                return Err(difference(
                    "expect.outcome",
                    "outcome",
                    DifferenceReason::OutcomeDiffers,
                    outcome.clone(),
                    Some(found.unwrap_or_else(|| "no outcome".to_owned())),
                ));
            }
        }
        if let Some(stop) = &expect.stop {
            let found = running
                .last_run
                .as_ref()
                .map(|ran| (ran.stop, ran.case.clone()));
            let holds = found.as_ref().is_some_and(|(found, case)| {
                *found == stop.stop && stop.case.as_ref().is_none_or(|wanted| wanted == case)
            });
            if !holds {
                let expected = match &stop.case {
                    Some(case) => format!("{:?} at {case}", stop.stop),
                    None => format!("{:?}", stop.stop),
                };
                return Err(difference(
                    "expect.stop",
                    "stop",
                    DifferenceReason::StopDiffers,
                    expected,
                    Some(match found {
                        Some((stop, case)) => format!("{stop:?} at {case}"),
                        None => "no run".to_owned(),
                    }),
                ));
            }
        }
        if let Some(difference) = evaluate_checks(
            &expect.checks,
            "expect",
            &self.view(running),
            &running.captured,
        ) {
            return Err(difference);
        }
        if read_manifest(&running.store).is_ok() {
            let verified = verify_tree(&running.store, self.catalog);
            let failed = match &verified {
                Ok(verified) => verified
                    .failure
                    .as_ref()
                    .map(|failure| format!("case {} line {}", failure.case, failure.line)),
                Err(refusal) => Some(format!("{:?}", refusal.reason())),
            };
            if let Some(found) = failed {
                return Err(difference(
                    "verify",
                    "verify",
                    DifferenceReason::VerifyFailed,
                    "every log verifies",
                    Some(found),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::instant_after;

    #[test]
    fn minutes_after_an_instant_cross_days_months_and_years() {
        assert_eq!(
            instant_after("2026-11-22T08:00:00Z", 0).as_deref(),
            Some("2026-11-22T08:00:00Z")
        );
        assert_eq!(
            instant_after("2026-12-31T23:59:30Z", 1).as_deref(),
            Some("2027-01-01T00:00:30Z")
        );
        assert_eq!(
            instant_after("2028-02-28T23:00:00Z", 60).as_deref(),
            Some("2028-02-29T00:00:00Z")
        );
        assert_eq!(instant_after("2026-11-22 08:00:00", 1), None);
    }
}
