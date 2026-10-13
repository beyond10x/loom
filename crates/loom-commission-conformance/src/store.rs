//! The run store a run command is answered against, and the suspension store a Suspension command
//! is answered against.
//!
//! `StartRun`, `SuspendRun` and `ResumeRun` declare `storage-failed` as `external:` in the
//! specification: the run store's ability to write decides it, not the input. A scenario forces it
//! by `configure_external_outcome`, and the target answers by putting the store in the state that
//! external condition names: the next write of the armed command cannot be made. Every other
//! answer comes from `b10x_loom_commission::outcome::RunStore`, which holds the runs.
//!
//! `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension` declare `storage-failed` the same
//! way, and are forced the same way on [`ScenarioSuspensions`]; every other answer comes from
//! `b10x_loom_commission::ports::suspension::SuspensionStore`, which holds the Suspensions.

use b10x_loom_commission::model::behaviour::{RunStorage, SuspensionStorage};
use b10x_loom_commission::model::obligation::UnmetObligation;
use b10x_loom_commission::model::responsibility::obligations::{
    HoldSuspensionBehavior, ReleaseSuspensionBehavior, ResumeRunBehavior, StartRunBehavior,
    SuspendRunBehavior, TakeSuspensionBehavior,
};
use b10x_loom_commission::model::responsibility::{
    HoldSuspension, HoldSuspensionOutcome, ReleaseSuspension, ReleaseSuspensionOutcome, ResumeRun,
    ResumeRunOutcome, RunId, RunSnapshot, RunStorageFailed, StartRun, StartRunOutcome, SuspendRun,
    SuspendRunOutcome, SuspensionKey, SuspensionSnapshot, SuspensionStorageFailed, TakeSuspension,
    TakeSuspensionOutcome,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::suspension::SuspensionStore;

/// The reason the store gives for a write a scenario made fail.
pub const UNWRITABLE: &str = "the conformance run store cannot write";

/// The run command whose next write cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unwritable {
    /// `StartRun`: the store cannot hold the new run.
    StartRun,
    /// `SuspendRun`: the store cannot record the suspension.
    SuspendRun,
    /// `ResumeRun`: the store cannot record the resumption.
    ResumeRun,
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

impl ResumeRunBehavior for ScenarioStore {
    fn resume_run(&mut self, input: ResumeRun) -> Result<ResumeRunOutcome, UnmetObligation> {
        if self.fails(Unwritable::ResumeRun) {
            return Ok(ResumeRunOutcome::StorageFailed {
                error: unwritable(),
            });
        }
        self.runs.resume_run(input)
    }
}

/// The reason the suspension store gives for a write a scenario made fail.
pub const SUSPENSIONS_UNWRITABLE: &str = "the conformance suspension store cannot write";

/// The Suspension command whose next write cannot be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspensionUnwritable {
    /// `HoldSuspension`: the port cannot hold the Suspension.
    Hold,
    /// `TakeSuspension`: the port cannot record the take.
    Take,
    /// `ReleaseSuspension`: the port cannot remove the Suspension.
    Release,
}

/// [`SuspensionStore`], whose next write of one Suspension command can be made to fail.
#[derive(Debug, Default)]
pub struct ScenarioSuspensions {
    held: SuspensionStore,
    unwritable: Option<SuspensionUnwritable>,
}

impl ScenarioSuspensions {
    /// An empty store that can write.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the next write of `command` fail. The condition lapses after that invocation.
    pub fn fail_next(&mut self, command: SuspensionUnwritable) {
        self.unwritable = Some(command);
    }

    /// Whether `command`'s write fails now; a condition that fires lapses.
    fn fails(&mut self, command: SuspensionUnwritable) -> bool {
        if self.unwritable == Some(command) {
            self.unwritable = None;
            true
        } else {
            false
        }
    }
}

fn suspensions_unwritable() -> SuspensionStorageFailed {
    SuspensionStorageFailed {
        reason: SUSPENSIONS_UNWRITABLE.to_owned(),
    }
}

impl SuspensionStorage for ScenarioSuspensions {
    fn get(&self, identity: &SuspensionKey) -> Option<SuspensionSnapshot> {
        SuspensionStorage::get(&self.held, identity)
    }

    fn put(&mut self, snapshot: SuspensionSnapshot) {
        SuspensionStorage::put(&mut self.held, snapshot);
    }

    fn delete(&mut self, identity: &SuspensionKey) {
        SuspensionStorage::delete(&mut self.held, identity);
    }

    fn list(&self) -> Vec<SuspensionSnapshot> {
        SuspensionStorage::list(&self.held)
    }
}

impl HoldSuspensionBehavior for ScenarioSuspensions {
    fn hold_suspension(
        &mut self,
        input: HoldSuspension,
    ) -> Result<HoldSuspensionOutcome, UnmetObligation> {
        if self.fails(SuspensionUnwritable::Hold) {
            return Ok(HoldSuspensionOutcome::StorageFailed {
                error: suspensions_unwritable(),
            });
        }
        self.held.hold_suspension(input)
    }
}

impl TakeSuspensionBehavior for ScenarioSuspensions {
    fn take_suspension(
        &mut self,
        input: TakeSuspension,
    ) -> Result<TakeSuspensionOutcome, UnmetObligation> {
        if self.fails(SuspensionUnwritable::Take) {
            return Ok(TakeSuspensionOutcome::StorageFailed {
                error: suspensions_unwritable(),
            });
        }
        self.held.take_suspension(input)
    }
}

impl ReleaseSuspensionBehavior for ScenarioSuspensions {
    fn release_suspension(
        &mut self,
        input: ReleaseSuspension,
    ) -> Result<ReleaseSuspensionOutcome, UnmetObligation> {
        if self.fails(SuspensionUnwritable::Release) {
            return Ok(ReleaseSuspensionOutcome::StorageFailed {
                error: suspensions_unwritable(),
            });
        }
        self.held.release_suspension(input)
    }
}
