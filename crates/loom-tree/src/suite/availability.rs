//! [`SUITE_AVAILABILITY`]: what of the format this binary runs, and the story each other part
//! waits for. A cycle that uses a part that waits, in a step, a scripted entry, a selection or a
//! cycle member, is pending and never run ([`first_waiting`]).

use loom::tree::{
    CycleStep, ScriptedDecision, SelectItem, StepKind, SuiteCheck, SuiteCycle, WaitingCycle,
};

/// Where a part of the format is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindUse {
    /// A step kind (`do`).
    Step,
    /// A scripted decision (`take`).
    Entry,
    /// A source of a selection.
    Source,
    /// A run log event kind a `log` source names.
    Event,
    /// A member of a cycle, a step or a scripted entry.
    Member,
}

/// Whether this binary runs a part of the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// It runs.
    Available,
    /// It waits for the story named, which adds it.
    Waits(&'static str),
}

/// One part of the format and whether it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvailabilityEntry {
    /// Where it is used.
    pub used: KindUse,
    /// Its name as the suite writes it; a member of a kind is `<kind>.<member>`.
    pub name: &'static str,
    /// Whether it runs.
    pub availability: Availability,
}

const fn entry(used: KindUse, name: &'static str, availability: Availability) -> AvailabilityEntry {
    AvailabilityEntry {
        used,
        name,
        availability,
    }
}

use Availability::{Available, Waits};
use KindUse::{Entry, Event, Member, Source, Step};

const STOP_AND_LABELS: &str = "story:tree-stop-record-and-labels";
const REPLAY: &str = "story:tree-replay-command";
const TEST_COMMAND: &str = "story:tree-test-command";
const CALLS: &str = "story:agent-node-run-records";
const ENGINE: &str = "story:engine-configuration-document";
const AGENTS: &str = "story:engine-agents-section";

/// Every step kind, scripted entry, source, run log event kind and waiting member of
/// `loom.tree-suite/1`, and whether this binary runs it. A name the table does not hold is one
/// the format does not know.
pub const SUITE_AVAILABILITY: &[AvailabilityEntry] = &[
    entry(Step, "open", Available),
    entry(Step, "run", Available),
    entry(Step, "approve", Available),
    entry(Step, "close", Available),
    entry(Step, "status", Available),
    entry(Step, "verify", Available),
    entry(Step, "write", Available),
    entry(Step, "stop", Waits(STOP_AND_LABELS)),
    entry(Step, "resume", Waits(STOP_AND_LABELS)),
    entry(Step, "allow", Waits(STOP_AND_LABELS)),
    entry(Step, "reopen", Waits(STOP_AND_LABELS)),
    entry(Step, "replay", Waits(REPLAY)),
    entry(Step, "decide", Waits(TEST_COMMAND)),
    entry(Entry, "approve", Available),
    entry(Entry, "close", Available),
    entry(Entry, "decide", Waits(TEST_COMMAND)),
    entry(Member, "approve.result", Waits(STOP_AND_LABELS)),
    entry(Member, "approve.reasons", Waits(STOP_AND_LABELS)),
    entry(Member, "approve.deny", Waits(STOP_AND_LABELS)),
    entry(Member, "engine", Waits(ENGINE)),
    entry(Member, "fixtures", Waits(AGENTS)),
    entry(Member, "when", Available),
    entry(Source, "log", Available),
    entry(Source, "records", Available),
    entry(Source, "runs", Available),
    entry(Source, "cases", Available),
    entry(Source, "doc", Available),
    entry(Source, "files", Available),
    entry(Source, "step", Available),
    entry(Source, "suite", Available),
    entry(Source, "requests", Waits(TEST_COMMAND)),
    entry(Source, "calls", Waits(CALLS)),
    entry(Event, "CaseOpened", Available),
    entry(Event, "Evaluated", Available),
    entry(Event, "ActionStarted", Available),
    entry(Event, "ActionFinished", Available),
    entry(Event, "Discarded", Available),
    entry(Event, "ChildOpened", Available),
    entry(Event, "ChildClosed", Available),
    entry(Event, "AuthorityRecorded", Available),
    entry(Event, "Stopped", Available),
    entry(Event, "Terminated", Available),
    entry(Event, "StopRecorded", Waits(STOP_AND_LABELS)),
    entry(Event, "StopLifted", Waits(STOP_AND_LABELS)),
    entry(Event, "Labelled", Waits(STOP_AND_LABELS)),
    entry(Event, "RunSettings", Waits(AGENTS)),
];

/// Whether the format knows `name` where it is `used`.
pub(crate) fn known(used: KindUse, name: &str) -> bool {
    SUITE_AVAILABILITY
        .iter()
        .any(|entry| entry.used == used && entry.name == name)
}

pub(crate) fn step_name(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Open => "open",
        StepKind::Run => "run",
        StepKind::Approve => "approve",
        StepKind::Close => "close",
        StepKind::Decide => "decide",
        StepKind::Stop => "stop",
        StepKind::Resume => "resume",
        StepKind::Allow => "allow",
        StepKind::Reopen => "reopen",
        StepKind::Replay => "replay",
        StepKind::Status => "status",
        StepKind::Verify => "verify",
        StepKind::Write => "write",
    }
}

/// The first part `cycle` uses that waits in `table`, as the waiting cycle it makes; `None` when
/// every part it uses runs. A part `table` does not hold runs. Read in this order: the cycle's
/// members, its scripted entries, its steps (kind, members, checks), then its expectation.
pub fn first_waiting(cycle: &SuiteCycle, table: &[AvailabilityEntry]) -> Option<WaitingCycle> {
    let waits = |used: KindUse, name: &str| -> Option<WaitingCycle> {
        table
            .iter()
            .find(|entry| entry.used == used && entry.name == name)
            .and_then(|entry| match entry.availability {
                Available => None,
                Waits(story) => Some(WaitingCycle {
                    name: cycle.name.clone(),
                    story: story.to_owned(),
                    kind: name.to_owned(),
                }),
            })
    };
    let mut found = None;
    let mut look = |used: KindUse, name: &str| {
        if found.is_none() {
            found = waits(used, name);
        }
    };
    if cycle.engine.is_some() {
        look(Member, "engine");
    }
    if cycle.fixtures.is_some() {
        look(Member, "fixtures");
    }
    if !cycle.when.is_empty() {
        look(Member, "when");
    }
    for decision in &cycle.decide {
        match decision {
            ScriptedDecision::Approve(approval) => {
                look(Entry, "approve");
                if approval.result.is_some() {
                    look(Member, "approve.result");
                }
                if approval.reasons.is_some() {
                    look(Member, "approve.reasons");
                }
                if approval.deny.is_some() {
                    look(Member, "approve.deny");
                }
            }
            ScriptedDecision::Close(_) => look(Entry, "close"),
            ScriptedDecision::Decide(_) => look(Entry, "decide"),
        }
    }
    for step in &cycle.steps {
        step_parts(step, &mut look);
    }
    for check in &cycle.expect.checks {
        check_parts(check, &mut look);
    }
    found
}

fn step_parts(step: &CycleStep, look: &mut impl FnMut(KindUse, &str)) {
    look(Step, step_name(step.r#do));
    if step.r#do == StepKind::Approve {
        if step.result.is_some() {
            look(Member, "approve.result");
        }
        if step.reasons.is_some() {
            look(Member, "approve.reasons");
        }
        if step.deny.is_some() {
            look(Member, "approve.deny");
        }
    }
    if step.engine.is_some() {
        look(Member, "engine");
    }
    for check in &step.checks {
        check_parts(check, look);
    }
}

fn check_parts(check: &SuiteCheck, look: &mut impl FnMut(KindUse, &str)) {
    selection_parts(&check.select, look);
    for operand in [&check.left, &check.right].into_iter().flatten() {
        if let Some(select) = &operand.select {
            selection_parts(select, look);
        }
    }
}

fn selection_parts(items: &[SelectItem], look: &mut impl FnMut(KindUse, &str)) {
    let Some(first) = items.first() else { return };
    let source = [
        (first.log.is_some(), "log"),
        (first.records.is_some(), "records"),
        (first.runs.is_some(), "runs"),
        (first.cases.is_some(), "cases"),
        (first.doc.is_some(), "doc"),
        (first.files.is_some(), "files"),
        (first.step.is_some(), "step"),
        (first.suite.is_some(), "suite"),
        (first.requests.is_some(), "requests"),
        (first.calls.is_some(), "calls"),
    ]
    .into_iter()
    .find_map(|(present, name)| present.then_some(name));
    if let Some(source) = source {
        look(Source, source);
    }
    if let Some(log) = &first.log {
        for event in &log.event {
            look(Event, event);
        }
    }
}
