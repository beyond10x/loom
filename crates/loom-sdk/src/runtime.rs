//! [`Runtime`]: the parts of a run held together, so a host builds them once and runs, suspends
//! and resumes a commission with one value (`story:runtime-wrapper`).
//!
//! It adds no behaviour. [`Runtime::run`], [`Runtime::run_suspending`] and [`Runtime::resume`]
//! call [`run_until_blocked`], [`run_with_suspension`] and [`resume_after_grant`] with the parts
//! it holds and return what they return; the free functions stay public and unchanged, and a host
//! may call them directly instead. The governor, executor, authority provider and effect port are
//! held by reference, because a host usually shares one governor between them (an effect port
//! that submits evidence, an executor that reads the frontier); the run store and the loop
//! context are owned, because each call takes them mutably.

use b10x_loom_commission::model::behaviour::RunStorage;
use b10x_loom_commission::model::responsibility::obligations::{
    ResumeRunBehavior, StartRunBehavior, SuspendRunBehavior,
};
use b10x_loom_commission::model::responsibility::{Commission, commission_state};
use b10x_loom_commission::ports::authority::AuthorityProvider;
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, ResumeError, resume_after_grant, run_until_blocked,
    run_with_suspension,
};

/// A run's governor, executor, authority provider and effect port, by reference, and its run
/// store and loop context, owned.
pub struct Runtime<'p, G: ?Sized, E: ?Sized, A: ?Sized, F: ?Sized, R, C> {
    governor: &'p G,
    executor: &'p E,
    authority: &'p A,
    effects: &'p F,
    runs: R,
    context: C,
}

impl<'p, G: ?Sized, E: ?Sized, A: ?Sized, F: ?Sized, R, C> Runtime<'p, G, E, A, F, R, C> {
    /// Holds the parts a run needs.
    pub fn new(
        governor: &'p G,
        executor: &'p E,
        authority: &'p A,
        effects: &'p F,
        runs: R,
        context: C,
    ) -> Self {
        Self {
            governor,
            executor,
            authority,
            effects,
            runs,
            context,
        }
    }

    /// The run store, holding every Run this runtime started or resumed.
    pub fn runs(&self) -> &R {
        &self.runs
    }

    /// The run store, mutably.
    pub fn runs_mut(&mut self) -> &mut R {
        &mut self.runs
    }

    /// The loop context.
    pub fn context(&self) -> &C {
        &self.context
    }

    /// The loop context, mutably.
    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    /// The run store and the loop context, giving up the runtime.
    pub fn into_parts(self) -> (R, C) {
        (self.runs, self.context)
    }
}

impl<G, E, A, F, R, C> Runtime<'_, G, E, A, F, R, C>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    C: LoopContext,
{
    /// [`run_until_blocked`] with the parts this runtime holds.
    pub fn run(
        &mut self,
        commission: &Commission<commission_state::Assigned>,
    ) -> Result<LoopEnd, LoopError>
    where
        R: StartRunBehavior + SuspendRunBehavior,
    {
        run_until_blocked(
            self.governor,
            self.executor,
            self.authority,
            self.effects,
            commission,
            &mut self.runs,
            &mut self.context,
        )
    }

    /// [`run_with_suspension`] with the parts this runtime holds and `suspensions`.
    pub fn run_suspending(
        &mut self,
        commission: &Commission<commission_state::Assigned>,
        suspensions: &dyn SuspensionPort,
    ) -> Result<LoopEnd, LoopError>
    where
        R: StartRunBehavior + SuspendRunBehavior + ResumeRunBehavior,
    {
        run_with_suspension(
            self.governor,
            self.executor,
            self.authority,
            self.effects,
            commission,
            &mut self.runs,
            suspensions,
            &mut self.context,
        )
    }

    /// [`resume_after_grant`] with the parts this runtime holds and `suspensions`.
    pub fn resume(
        &mut self,
        commission: &Commission<commission_state::Assigned>,
        suspensions: &dyn SuspensionPort,
    ) -> Result<LoopEnd, ResumeError>
    where
        R: RunStorage + SuspendRunBehavior + ResumeRunBehavior,
    {
        resume_after_grant(
            self.governor,
            self.executor,
            self.authority,
            self.effects,
            commission,
            &mut self.runs,
            suspensions,
            &mut self.context,
        )
    }
}
