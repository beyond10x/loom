//! Interruption and recovery of a governed run (`loom.run.InterruptSession`,
//! `loom.run.ResumeSession`; Atlas ADR 0071).
//!
//! # Interruption
//!
//! [`Loom::interrupt`] cancels the run holding a session. The session moves `Active` to
//! `Interrupted` at once, so nothing more is recorded into it, and the run's loop is cancelled at
//! its next check. A call that reached the selection pipeline before the loop saw the cancel is
//! selected and recorded, and stopped before it is revalidated: the selection stays `Selected`,
//! in flight, and is not proposed. The run ends `NoUsefulAction`, and its session stays
//! `Interrupted`; it is not filed.
//!
//! Whether a run was interrupted is its own cancel's answer, never the session's state, which a
//! later run may have changed. A run acts on its session only while it holds it: once another run
//! resumed the session, the interrupted run records no turn into it, and when it ends it neither
//! files the session nor holds or drops a checkpoint for it.
//!
//! # Checkpoints
//!
//! A governed run that stops at the approval checkpoint of a call ([`crate::harness::governed`])
//! leaves that checkpoint held for its session, with the call's selection and the proposal it made
//! or would have made: a proposal returned to Commission, or a selection in flight when the run was
//! interrupted. The checkpoint is held by this Loom, in memory, and a process restart loses it. A
//! run that starts a conversation of its own in the session ([`Loom::run_loop`]) drops it, and so
//! does a run that ends any other way, except a resumed run that ended before its held call was
//! answered: one whose configuration was refused, whose governor could not answer the first
//! projection, or that was interrupted or failed before the call was put to the pipeline or the
//! model asked again. Its checkpoint is held again, unchanged, for the next resume.
//!
//! # Recovery
//!
//! [`Loom::resume_loop`] resumes a session by id, from `Filed` or `Interrupted`, and continues from
//! its held checkpoint through the loop's own resume ([`AgentLoop::resume_asking`]), trusting
//! nothing that was pending there. The turn environment is refreshed first, so the catalogue is
//! projected from the frontier current at resume. Then the held call is put to the selection
//! pipeline again, which never selects it again:
//!
//! - a selection in flight is revalidated against the case's current frontier
//!   (`loom.run.RevalidateSelection`): admitted, its proposal is returned; refused, the refusal is
//!   recorded ([`Loom::revalidations`]) and the model is told why;
//! - a selection already admitted, whose proposal Commission was handed (a run stopped at a merge
//!   awaiting its approval), is not revalidated twice: its proposal is returned again while the
//!   case is at the revision it was selected at and the frontier still admits its action, and the
//!   model is told it is stale otherwise.
//!
//! A model told its held call is stale names the revision the call was selected at and the
//! current one, and chooses again from the catalogue projected at resume; what it selects is
//! revalidated before anything is proposed. A held call the current catalogue no longer publishes
//! is refused to the model by the loop itself, and its selection is never proposed. Only the held
//! call itself is answered as held: a later call that reuses its call id is that call only when it
//! also calls the held action, and is selected like any other call otherwise.
//!
//! Authority stays Commission's. A proposal returned again is rechecked by Commission, whose
//! authority provider decides whether its approval was granted; Loom resumes the same way whatever
//! the provider answered. A session with no held checkpoint is resumed with a conversation of its
//! own, as [`Loom::run_loop`] resumes a filed one.
//!
//! [`AgentLoop::resume_asking`]: crate::harness::turn_loop::AgentLoop::resume_asking

use std::ops::Deref;

use b10x_loom_commission::model::responsibility::{
    CaseId, Commission, ExecutorOutcomeProposedAction, Frontier, commission_state, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;

use crate::harness::governed::{LoopPorts, LoopRun, Start};
use crate::harness::turn_loop::{ApprovalCheckpoint, LoopCancel};
use crate::harness::wire::ToolName;
use crate::model::behaviour::SessionStorage;
use crate::model::obligation::UnmetObligation;
use crate::model::run::obligations::InterruptSessionBehavior;
use crate::model::run::{InterruptSession, InterruptSessionOutcome, SelectionId, SessionId};
use crate::{Loom, outage};

/// What a Loom holds to interrupt and recover its runs: the run holding each session, by its run
/// number, with its cancel, and the checkpoint each session's last run stopped at.
#[derive(Debug, Default)]
pub(crate) struct Recovery {
    running: Vec<(SessionId, u64, LoopCancel)>,
    held: Vec<Held>,
}

/// The checkpoint a session's last run stopped at.
#[derive(Debug, Clone)]
pub(crate) struct Held {
    pub(crate) session: SessionId,
    pub(crate) checkpoint: ApprovalCheckpoint,
    pub(crate) pending: Pending,
    /// The case the run that stopped there worked on.
    pub(crate) case: CaseId,
    /// The narrowing of the run that stopped there (`LoopConfig::admits`): a resume may narrow it
    /// further, never widen it.
    pub(crate) admits: Option<Vec<ToolName>>,
}

impl Held {
    /// Why a run for `case`, narrowed by `admits`, may not resume from here, when it may not: the
    /// checkpoint was made for another case, or the narrowing admits a tool the run that stopped
    /// here did not (it removes the narrowing, or names a tool the narrowing did not).
    pub(crate) fn refuses(
        &self,
        case: &CaseId,
        admits: Option<&[ToolName]>,
    ) -> Option<&'static str> {
        if self.case != *case {
            return Some("the commission's case is not the case the checkpoint was made for");
        }
        let widened = match (self.admits.as_deref(), admits) {
            (None, _) => false,
            (Some(_), None) => true,
            (Some(held), Some(resumed)) => resumed.iter().any(|name| !held.contains(name)),
        };
        widened.then_some("the resumed run's narrowing admits a tool the stopped run's did not")
    }
}

/// The call a checkpoint stopped at, as the selection pipeline left it.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    /// The selection the call was recorded as.
    pub(crate) selection: SelectionId,
    /// What it is proposed as, once it may be.
    pub(crate) proposal: ExecutorOutcomeProposedAction,
    /// Whether it is still `Selected`: its run was interrupted before it was revalidated.
    pub(crate) in_flight: bool,
}

impl Recovery {
    /// Notes that run `run` holds `session` and is cancelled by `cancel`, in place of any run that
    /// held it before: an interrupted run that has not ended yet no longer holds a session another
    /// run resumed.
    pub(crate) fn start(&mut self, session: SessionId, run: u64, cancel: LoopCancel) {
        self.running.retain(|(held, _, _)| *held != session);
        self.running.push((session, run, cancel));
    }

    /// Whether run `run` holds `session`.
    pub(crate) fn holds(&self, session: &SessionId, run: u64) -> bool {
        self.running
            .iter()
            .any(|(held, holder, _)| held == session && *holder == run)
    }

    /// Notes that run `run` ended, and answers whether it still held `session`; a run that no
    /// longer held it changes nothing.
    pub(crate) fn stop(&mut self, session: &SessionId, run: u64) -> bool {
        let held = self.holds(session, run);
        if held {
            self.running.retain(|(holding, _, _)| holding != session);
        }
        held
    }

    /// Cancels the run holding `session`, if one does.
    fn cancel(&self, session: &SessionId) {
        self.running
            .iter()
            .filter(|(held, _, _)| held == session)
            .for_each(|(_, _, cancel)| cancel.cancel());
    }

    /// Holds `held` for its session, in place of any checkpoint held for it.
    pub(crate) fn hold(&mut self, held: Held) {
        self.release(&held.session);
        self.held.push(held);
    }

    /// The checkpoint held for `session`, no longer held.
    pub(crate) fn take(&mut self, session: &SessionId) -> Option<Held> {
        let at = self.held.iter().position(|held| &held.session == session)?;
        Some(self.held.remove(at))
    }

    /// Drops the checkpoint held for `session`.
    pub(crate) fn release(&mut self, session: &SessionId) {
        self.held.retain(|held| &held.session != session);
    }
}

impl<S, G, V> Loom<S, G, V> {
    /// Interrupts the run holding `session` (`loom.run.InterruptSession`): the session is
    /// `Interrupted` from now on, and the run's loop is cancelled. What the run had in flight is not
    /// proposed (see the module docs). Callable from the run's own sink and from another thread.
    ///
    /// # Errors
    ///
    /// [`UnmetObligation`] only if the generated behaviour could not decide, which it always can.
    /// A session that is not `Active` is answered `wrong-state`, and one this Loom never opened
    /// `wrong-state` for an unknown instance; neither cancels anything.
    pub fn interrupt(
        &self,
        session: &SessionId,
    ) -> Result<InterruptSessionOutcome, UnmetObligation> {
        let recovery = self.recovery();
        let outcome = self.turn_record().interrupt_session(InterruptSession {
            session_id: session.clone(),
        });
        if matches!(outcome, Ok(InterruptSessionOutcome::Interrupted { .. })) {
            recovery.cancel(session);
        }
        outcome
    }
}

impl<S, G, V> Loom<S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// Resumes `session` by id (`loom.run.ResumeSession`, from `Filed` or `Interrupted`) and
    /// continues its run from the checkpoint it stopped at, for `commission` on `frontier`, as the
    /// module docs say. With no checkpoint held, the resumed run starts a conversation of its own.
    ///
    /// `ports.config` must be the configuration the run was started with, except that its narrowing
    /// (`admits`) may be narrower, and `commission` must be for the case the run worked on: a
    /// checkpoint resumed under another configuration, under a narrowing that admits a tool the
    /// stopped run's did not, or for another case, fails with `LoopError::Config` and the session
    /// is filed as failed. The checkpoint stays held until a resumed run's held call is
    /// answered: a resume that ends before that (a refused configuration, a governor that cannot
    /// answer, an interrupt, a panic) holds it again for the next. The outcome is what
    /// [`Loom::run_loop`] answers, and a session this Loom never opened is refused as an outage
    /// before anything is sent.
    pub fn resume_loop(
        &self,
        session: &SessionId,
        ports: LoopPorts<'_>,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> LoopRun {
        let Some(held) = SessionStorage::get(&*self.turn_record(), session) else {
            return LoopRun {
                outcome: outage(format!(
                    "session `{}` was never opened by this Loom, so it cannot be resumed",
                    session.0.0
                )),
                run: None,
            };
        };
        self.governed_run(&held.data, ports, commission, frontier, Start::Checkpoint)
    }
}
