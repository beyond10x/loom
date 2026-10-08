//! The run store a run command is answered against.
//!
//! `StartRun` and `SuspendRun` declare `storage-failed` as `external:` in the specification: the
//! run store's ability to write decides it, not the input. A scenario forces it by
//! `configure_external_outcome`, and the target answers by putting the store in the state that
//! external condition names: the next write of the armed command cannot be made. Every other
//! answer comes from `b10x_loom_commission::outcome::RunStore`, which holds the runs.

use b10x_loom_commission::model::behaviour::RunStorage;
use b10x_loom_commission::model::obligation::UnmetObligation;
use b10x_loom_commission::model::responsibility::obligations::{
    StartRunBehavior, SuspendRunBehavior,
};
use b10x_loom_commission::model::responsibility::{
    RunId, RunSnapshot, RunStorageFailed, StartRun, StartRunOutcome, SuspendRun, SuspendRunOutcome,
};
use b10x_loom_commission::outcome::RunStore;

/// The reason the store gives for a write a scenario made fail.
pub const UNWRITABLE: &str = "the conformance run store cannot write";

/// The run command whose next write cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unwritable {
    /// `StartRun`: the store cannot hold the new run.
    StartRun,
    /// `SuspendRun`: the store cannot record the suspension.
    SuspendRun,
}

/// [`RunStore`], whose next write of one run command can be made to fail.
#[derive(Debug)]
pub struct ScenarioStore {
    runs: RunStore,
    unwritable: Option<Unwritable>,
}

impl ScenarioStore {
    /// A store over `runs` that can write.
    pub fn new(runs: RunStore) -> Self {
        Self {
            runs,
            unwritable: None,
        }
    }

    /// Makes the next write of `command` fail. The condition lapses after that invocation.
    pub fn fail_next(&mut self, command: Unwritable) {
        self.unwritable = Some(command);
    }

    /// Whether `command`'s write fails now; a condition that fires lapses.
    fn fails(&mut self, command: Unwritable) -> bool {
        if self.unwritable == Some(command) {
            self.unwritable = None;
            true
        } else {
            false
        }
    }
}

fn unwritable() -> RunStorageFailed {
    RunStorageFailed {
        reason: UNWRITABLE.to_owned(),
    }
}

impl RunStorage for ScenarioStore {
    fn get(&self, identity: &RunId) -> Option<RunSnapshot> {
        self.runs.get(identity)
    }

    fn put(&mut self, snapshot: RunSnapshot) {
        self.runs.put(snapshot);
    }

    fn delete(&mut self, identity: &RunId) {
        self.runs.delete(identity);
    }

    fn list(&self) -> Vec<RunSnapshot> {
        self.runs.list()
    }
}

impl StartRunBehavior for ScenarioStore {
    fn start_run(&mut self, input: StartRun) -> Result<StartRunOutcome, UnmetObligation> {
        if self.fails(Unwritable::StartRun) {
            return Ok(StartRunOutcome::StorageFailed {
                error: unwritable(),
            });
        }
        self.runs.start_run(input)
    }
}

impl SuspendRunBehavior for ScenarioStore {
    fn suspend_run(&mut self, input: SuspendRun) -> Result<SuspendRunOutcome, UnmetObligation> {
        if self.fails(Unwritable::SuspendRun) {
            return Ok(SuspendRunOutcome::StorageFailed {
                error: unwritable(),
            });
        }
        self.runs.suspend_run(input)
    }
}
