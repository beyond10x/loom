// generated from commission v1
// model digest 24f933215e98a20ff4758a4c398276a46849806e95ade4524297a92f95fe25fc
// contract digest 079c52a5c30f36a2d41adc343a508d804fd1658d2217aeeefa310d5094f46d2c
// do not edit: regenerate with `ess synthesize --layout crate`

//! What the specification fully determines, generated: the behaviour of every command the plan
//! lists as generated, written against ports the implementor supplies.
//!
//! Storage is a port: one trait per entity, get, put and delete of a snapshot by identity. ess
//! preserves the trait; generated network entries supply an ephemeral store. `Context` carries the caller's attributes,
//! every identity and value the model says the implementation assigns, and the answer to each
//! `external:` branch. [`Generated`] implements every generated `…Behavior` trait over those ports
//! and forwards every behaviour and query the plan still owes to them, so it is a complete bundle
//! for every component port. To replace one generated behaviour, write a bundle of your own that
//! implements that trait and delegates the rest to a `Generated`.
//!
//! An `Err` from a generated behaviour is the typed refusal naming the command: the model declares
//! no outcome for the request (a guard is undecidable over it, or no declared branch answers it),
//! or — as `entity invariant` — the declared outcome would leave an entity breaking an invariant.

use crate::obligation::UnmetObligation;

/// Where `commission.responsibility.Run` is stored — a port the implementor provides.
///
/// Keyed by the identity `run_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait RunStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::responsibility::RunId) -> Option<crate::responsibility::RunSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::responsibility::RunSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::responsibility::RunId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::responsibility::RunSnapshot>;
}

/// Where `commission.responsibility.Suspension` is stored — a port the implementor provides.
///
/// Keyed by the identity `suspension`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SuspensionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::responsibility::SuspensionKey) -> Option<crate::responsibility::SuspensionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::responsibility::SuspensionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::responsibility::SuspensionKey);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::responsibility::SuspensionSnapshot>;
}

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes, and
/// every `…Behavior` and `…Query` trait the plan still owes; `Generated<P>` forwards those to it.
pub struct Generated<P> {
    /// The storage ports, and every behaviour or query still owed.
    pub ports: P,
}

impl<P> Generated<P> {
    /// The generated behaviours, over `ports`.
    pub fn new(ports: P) -> Self {
        Self { ports }
    }
}

impl<P: crate::responsibility::obligations::HoldSuspensionBehavior> crate::responsibility::obligations::HoldSuspensionBehavior for Generated<P> {
    fn hold_suspension(&mut self, input: crate::responsibility::HoldSuspension) -> Result<crate::responsibility::HoldSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::HoldSuspensionBehavior::hold_suspension(&mut self.ports, input)
    }
}

/// `commission.responsibility.JudgeBudgetStop`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::JudgeBudgetStopBehavior for Generated<P>
{
    fn judge_budget_stop(&mut self, input: crate::responsibility::JudgeBudgetStop) -> Result<crate::responsibility::JudgeBudgetStopOutcome, UnmetObligation> {
        let _ = &input;
        // `waiting`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(compare_numbers(Some(&input.pending).map(|value| value.len().to_string()), Some("1".to_owned()), core::cmp::Ordering::is_ge), "commission.responsibility.JudgeBudgetStop")? {
            return Ok(crate::responsibility::JudgeBudgetStopOutcome::Waiting { error: crate::responsibility::RequestsWaiting { action_request_ids: input.pending.clone(), reason: input.reason.clone() } });
        }
        // `suspended`: the default.
        return Ok(crate::responsibility::JudgeBudgetStopOutcome::Suspended);
    }
}

/// `commission.responsibility.JudgeEffectRefusal`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::JudgeEffectRefusalBehavior for Generated<P>
{
    fn judge_effect_refusal(&mut self, input: crate::responsibility::JudgeEffectRefusal) -> Result<crate::responsibility::JudgeEffectRefusalOutcome, UnmetObligation> {
        let _ = &input;
        // `final`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.ends_run).map(|value| *value), Some(true)), "commission.responsibility.JudgeEffectRefusal")? {
            return Ok(crate::responsibility::JudgeEffectRefusalOutcome::Final { error: crate::responsibility::EffectRefusedFinal { action_request_id: input.action_request_id.clone(), reason: input.reason.clone() } });
        }
        // `ordinary`: the default.
        return Ok(crate::responsibility::JudgeEffectRefusalOutcome::Ordinary);
    }
}

/// `commission.responsibility.JudgeIdleIteration`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::JudgeIdleIterationBehavior for Generated<P>
{
    fn judge_idle_iteration(&mut self, input: crate::responsibility::JudgeIdleIteration) -> Result<crate::responsibility::JudgeIdleIterationOutcome, UnmetObligation> {
        let _ = &input;
        // `bound`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(compare_numbers(Some(&input.idle_iterations).map(|value| value.to_string()), Some(&input.bound).map(|value| value.to_string()), core::cmp::Ordering::is_ge), "commission.responsibility.JudgeIdleIteration")? {
            return Ok(crate::responsibility::JudgeIdleIterationOutcome::Bound { error: crate::responsibility::IdleBoundReached { idle_iterations: input.idle_iterations.clone() } });
        }
        // `below`: the default.
        return Ok(crate::responsibility::JudgeIdleIterationOutcome::Below);
    }
}

/// `commission.responsibility.JudgePerformableActions`, generated: every outcome is one the specification fully determines.
impl<P> crate::responsibility::obligations::JudgePerformableActionsBehavior for Generated<P>
{
    fn judge_performable_actions(&mut self, input: crate::responsibility::JudgePerformableActions) -> Result<crate::responsibility::JudgePerformableActionsOutcome, UnmetObligation> {
        let _ = &input;
        // `unperformable`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[compare_numbers(Some(&input.performed).map(|value| value.to_string()), Some("0".to_owned()), core::cmp::Ordering::is_eq), compare_numbers(Some(&input.unperformed).map(|value| value.len().to_string()), Some("1".to_owned()), core::cmp::Ordering::is_ge)]), "commission.responsibility.JudgePerformableActions")? {
            return Ok(crate::responsibility::JudgePerformableActionsOutcome::Unperformable { error: crate::responsibility::ActionsUnperformable { actions: input.unperformed.clone(), open_claims: input.open_claims.clone() } });
        }
        // `performable`: the default.
        return Ok(crate::responsibility::JudgePerformableActionsOutcome::Performable);
    }
}

impl<P: crate::responsibility::obligations::ReleaseSuspensionBehavior> crate::responsibility::obligations::ReleaseSuspensionBehavior for Generated<P> {
    fn release_suspension(&mut self, input: crate::responsibility::ReleaseSuspension) -> Result<crate::responsibility::ReleaseSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::ReleaseSuspensionBehavior::release_suspension(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::ResumeRunBehavior> crate::responsibility::obligations::ResumeRunBehavior for Generated<P> {
    fn resume_run(&mut self, input: crate::responsibility::ResumeRun) -> Result<crate::responsibility::ResumeRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::ResumeRunBehavior::resume_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::RevalidateActionRequestBehavior> crate::responsibility::obligations::RevalidateActionRequestBehavior for Generated<P> {
    fn revalidate_action_request(&mut self, input: crate::responsibility::RevalidateActionRequest) -> Result<crate::responsibility::RevalidateActionRequestOutcome, UnmetObligation> {
        crate::responsibility::obligations::RevalidateActionRequestBehavior::revalidate_action_request(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::StartRunBehavior> crate::responsibility::obligations::StartRunBehavior for Generated<P> {
    fn start_run(&mut self, input: crate::responsibility::StartRun) -> Result<crate::responsibility::StartRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::StartRunBehavior::start_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::SuspendRunBehavior> crate::responsibility::obligations::SuspendRunBehavior for Generated<P> {
    fn suspend_run(&mut self, input: crate::responsibility::SuspendRun) -> Result<crate::responsibility::SuspendRunOutcome, UnmetObligation> {
        crate::responsibility::obligations::SuspendRunBehavior::suspend_run(&mut self.ports, input)
    }
}

impl<P: crate::responsibility::obligations::TakeSuspensionBehavior> crate::responsibility::obligations::TakeSuspensionBehavior for Generated<P> {
    fn take_suspension(&mut self, input: crate::responsibility::TakeSuspension) -> Result<crate::responsibility::TakeSuspensionOutcome, UnmetObligation> {
        crate::responsibility::obligations::TakeSuspensionBehavior::take_suspension(&mut self.ports, input)
    }
}

/// `commission.responsibility.RunStates`, generated: every row is one the specification fully determines from the stored `commission.responsibility.Run`s.
impl<P> crate::responsibility::obligations::RunStatesQuery for Generated<P>
where
    P: RunStorage,
{
    fn run_states(&self) -> Result<Vec<crate::responsibility::RunStates>, UnmetObligation> {
        let admitted = RunStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::responsibility::RunStates {
                run_id: held.data.run_id,
                commission_id: held.data.commission_id,
                case_revision: held.data.case_revision,
                state: held.state,
            })
            .collect())
    }
}

/// `commission.responsibility.SuspensionStates`, generated: every row is one the specification fully determines from the stored `commission.responsibility.Suspension`s.
impl<P> crate::responsibility::obligations::SuspensionStatesQuery for Generated<P>
where
    P: SuspensionStorage,
{
    fn suspension_states(&self) -> Result<Vec<crate::responsibility::SuspensionStates>, UnmetObligation> {
        let admitted = SuspensionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::responsibility::SuspensionStates {
                suspension: held.data.suspension,
                commission_id: held.data.commission_id,
                run_id: held.data.run_id,
                action_request_id: held.data.action_request_id,
                state: held.state,
            })
            .collect())
    }
}

/// The typed refusal of a request the model declares no outcome for.
fn undeclared(source: &'static str) -> UnmetObligation {
    let capability = "command behaviour";
    UnmetObligation { capability, source }
}

/// A guard's truth, where it has one: Unknown selects no branch, so the model declares no outcome.
fn decided(truth: Option<bool>, command: &'static str) -> Result<bool, UnmetObligation> {
    truth.ok_or_else(|| undeclared(command))
}

/// Three-valued conjunction: false wins, then Unknown.
fn all(truths: &[Option<bool>]) -> Option<bool> {
    if truths.contains(&Some(false)) {
        Some(false)
    } else if truths.contains(&None) {
        None
    } else {
        Some(true)
    }
}

/// Equality of two read values; an unread one is Unknown.
fn equal<T: PartialEq>(left: Option<T>, right: Option<T>) -> Option<bool> {
    Some(left? == right?)
}

/// A decimal rendering as its sign, its whole digits and its fraction digits, without the zeros
/// that do not change its value; `None` where it is not a plain decimal.
fn number_parts(text: &str) -> Option<(bool, String, String)> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().chain(fraction.bytes()).all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole = whole.trim_start_matches('0').to_owned();
    let fraction = fraction.trim_end_matches('0').to_owned();
    let zero = whole.is_empty() && fraction.is_empty();
    Some((negative && !zero, whole, fraction))
}

/// Compares two decimal renderings exactly; an unread or unparsable one is Unknown.
fn compare_numbers(
    left: Option<String>,
    right: Option<String>,
    accepts: fn(core::cmp::Ordering) -> bool,
) -> Option<bool> {
    let (left, right) = (number_parts(&left?)?, number_parts(&right?)?);
    let magnitude = left
        .1
        .len()
        .cmp(&right.1.len())
        .then_with(|| left.1.cmp(&right.1))
        .then_with(|| left.2.cmp(&right.2));
    let ordering = match (left.0, right.0) {
        (false, false) => magnitude,
        (true, true) => magnitude.reverse(),
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
    };
    Some(accepts(ordering))
}
