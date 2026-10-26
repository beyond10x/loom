// generated from loom v1
// model digest ab3a9045b7503e022390ae7585a5f18f93fa2b65e8d482b33126c9b72b6c8557
// contract digest 5ff5ff482ca56a9d17ca259eb07404322585c5fcfd49553a38ba3a03930af806
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

/// Where `loom.resource.LoomAgent` is stored — a port the implementor provides.
///
/// Keyed by the identity `name`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait LoomAgentStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::resource::ResourceName) -> Option<crate::resource::LoomAgentSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::resource::LoomAgentSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::resource::ResourceName);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::resource::LoomAgentSnapshot>;
}

/// Where `loom.run.ActionCatalogue` is stored — a port the implementor provides.
///
/// Keyed by the identity `catalogue_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait ActionCatalogueStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::CatalogueId) -> Option<crate::run::ActionCatalogueSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::ActionCatalogueSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::CatalogueId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::ActionCatalogueSnapshot>;
}

/// Where `loom.run.ArgumentRequest` is stored — a port the implementor provides.
///
/// Keyed by the identity `argument_request_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait ArgumentRequestStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::ArgumentRequestId) -> Option<crate::run::ArgumentRequestSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::ArgumentRequestSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::ArgumentRequestId);
}

/// Where `loom.run.Selection` is stored — a port the implementor provides.
///
/// Keyed by the identity `selection_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SelectionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::SelectionId) -> Option<crate::run::SelectionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::SelectionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::SelectionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::SelectionSnapshot>;
}

/// Where `loom.run.SelectionRecord` is stored — a port the implementor provides.
///
/// Keyed by the identity `selection_record_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait SelectionRecordStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::run::SelectionRecordId) -> Option<crate::run::SelectionRecordSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::run::SelectionRecordSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::run::SelectionRecordId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::run::SelectionRecordSnapshot>;
}

/// Where `loom.session.AgentSession` is stored — a port the implementor provides.
///
/// Keyed by the identity `session_id`. Generated network entries supply an ephemeral implementation; durable storage remains a port.
pub trait AgentSessionStorage {
    /// The instance with this identity, or `None` where none is stored.
    fn get(&self, identity: &crate::session::SessionId) -> Option<crate::session::AgentSessionSnapshot>;

    /// Stores this instance under its identity, replacing what was held.
    fn put(&mut self, snapshot: crate::session::AgentSessionSnapshot);

    /// Removes the instance with this identity.
    fn delete(&mut self, identity: &crate::session::SessionId);

    /// Every stored instance, in the order the store keeps them: the order a generated query
    /// answers an unordered view in.
    fn list(&self) -> Vec<crate::session::AgentSessionSnapshot>;
}

/// The exact executing command input supplied to an external decision.
///
/// This supplies facts, not authority: the context must verify its request-bound proof.
#[derive(Debug, Clone, Copy)]
pub enum ExternalCommand<'a> {
    /// The executing `loom.run.RevalidateSelection` input.
    LoomRunRevalidateSelection(&'a crate::run::RevalidateSelection),
}

impl ExternalCommand<'_> {
    /// The canonical qualified identity of this command.
    pub fn name(&self) -> &'static str {
        match self {
            Self::LoomRunRevalidateSelection(_) => "loom.run.RevalidateSelection",
        }
    }
}

/// What the specification leaves to the implementor's context — a port the implementor provides.
///
/// The caller's attributes, the values the model says the implementation assigns, and the answer
/// to each `external:` branch.
pub trait Context {
    /// Whether the external branch `outcome` of `command` is taken on this invocation.
    ///
    /// Asked in declaration order, before the branch's input guard is read; the first branch
    /// answered `true` whose guard holds is taken. A test forces a branch by answering `true`
    /// for it alone; a deployment asks whatever decides it.
    fn external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> bool;
}

/// Context answers that may be unavailable, without fabricated values.
/// Existing `Context` implementations receive the blanket adapter.
pub trait TryContext {
/// Decides the named external branch, or names the unavailable answer.
fn try_external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> Result<bool, UnmetObligation>;
}

impl<T: Context + ?Sized> TryContext for T {
fn try_external(&mut self, command: ExternalCommand<'_>, outcome: &'static str) -> Result<bool, UnmetObligation> { Ok(Context::external(self, command, outcome)) }
}

/// An unavailable runtime context answer, rather than a new planned capability.
pub fn unmet_context(source: &'static str) -> UnmetObligation { UnmetObligation { capability: "context answer", source } }

/// Every generated behaviour of this workspace, over the ports `P` supplies.
///
/// `P` implements the storage trait of each entity a generated behaviour reads or writes,
/// `TryContext` (or its legacy `Context` blanket adapter) where one asks it anything, and every `…Behavior` and `…Query` trait the plan still
/// owes; `Generated<P>` forwards those to it.
pub struct Generated<P> {
    /// The storage and context ports, and every behaviour or query still owed.
    pub ports: P,
}

impl<P> Generated<P> {
    /// The generated behaviours, over `ports`.
    pub fn new(ports: P) -> Self {
        Self { ports }
    }
}

/// `loom.agent.AdmitA2aRequest`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::AdmitA2aRequestBehavior for Generated<P>
{
    fn admit_a2a_request(&mut self, input: crate::agent::AdmitA2aRequest) -> Result<crate::agent::AdmitA2aRequestOutcome, UnmetObligation> {
        let _ = &input;
        // `missing-bearer`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.probe).map(|value| *value), Some(false)), equal(Some(&input.bearer_presented).map(|value| *value), Some(false))]), "loom.agent.AdmitA2aRequest")? {
            return Ok(crate::agent::AdmitA2aRequestOutcome::MissingBearer { error: crate::agent::A2aRefused { reason: crate::agent::A2aRefusalReason::MissingBearer } });
        }
        // `wrong-bearer`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.probe).map(|value| *value), Some(false)), equal(Some(&input.bearer_matches).map(|value| *value), Some(false))]), "loom.agent.AdmitA2aRequest")? {
            return Ok(crate::agent::AdmitA2aRequestOutcome::WrongBearer { error: crate::agent::A2aRefused { reason: crate::agent::A2aRefusalReason::WrongBearer } });
        }
        // `probe`: an accepting branch, in declaration order.
        if decided(equal(Some(&input.probe).map(|value| *value), Some(true)), "loom.agent.AdmitA2aRequest")? {
            return Ok(crate::agent::AdmitA2aRequestOutcome::Probe);
        }
        // `admitted`: the default.
        return Ok(crate::agent::AdmitA2aRequestOutcome::Admitted);
    }
}

/// `loom.agent.AdmitHttpRead`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::AdmitHttpReadBehavior for Generated<P>
{
    fn admit_http_read(&mut self, input: crate::agent::AdmitHttpRead) -> Result<crate::agent::AdmitHttpReadOutcome, UnmetObligation> {
        let _ = &input;
        // `method-not-get`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.method).map(|value| value.clone()), Some("GET".to_owned())).map(|value| !value), "loom.agent.AdmitHttpRead")? {
            return Ok(crate::agent::AdmitHttpReadOutcome::MethodNotGet { error: crate::agent::HttpReadRefused { reason: crate::agent::HttpReadRefusalReason::MethodNotGet } });
        }
        // `path-not-allowed`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.path_allowed).map(|value| *value), Some(false)), "loom.agent.AdmitHttpRead")? {
            return Ok(crate::agent::AdmitHttpReadOutcome::PathNotAllowed { error: crate::agent::HttpReadRefused { reason: crate::agent::HttpReadRefusalReason::PathNotAllowed } });
        }
        // `bearer-missing`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.bearer_missing).map(|value| *value), Some(true)), "loom.agent.AdmitHttpRead")? {
            return Ok(crate::agent::AdmitHttpReadOutcome::BearerMissing { error: crate::agent::HttpReadRefused { reason: crate::agent::HttpReadRefusalReason::BearerMissing } });
        }
        // `admitted`: the default.
        return Ok(crate::agent::AdmitHttpReadOutcome::Admitted);
    }
}

/// `loom.agent.AdmitHttpTrust`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::AdmitHttpTrustBehavior for Generated<P>
{
    fn admit_http_trust(&mut self, input: crate::agent::AdmitHttpTrust) -> Result<crate::agent::AdmitHttpTrustOutcome, UnmetObligation> {
        let _ = &input;
        // `unreadable`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.named).map(|value| *value), Some(true)), equal(Some(&input.read).map(|value| *value), Some(false))]), "loom.agent.AdmitHttpTrust")? {
            return Ok(crate::agent::AdmitHttpTrustOutcome::Unreadable { error: crate::agent::HttpTrustRefused { reason: crate::agent::HttpTrustRefusalReason::Unreadable } });
        }
        // `too-large`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.named).map(|value| *value), Some(true)), compare_numbers(Some(&input.bytes).map(|value| value.to_string()), Some("1048576".to_owned()), core::cmp::Ordering::is_gt)]), "loom.agent.AdmitHttpTrust")? {
            return Ok(crate::agent::AdmitHttpTrustOutcome::TooLarge { error: crate::agent::HttpTrustRefused { reason: crate::agent::HttpTrustRefusalReason::TooLarge } });
        }
        // `no-certificate`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.named).map(|value| *value), Some(true)), compare_numbers(Some(&input.certificates).map(|value| value.to_string()), Some("0".to_owned()), core::cmp::Ordering::is_le)]), "loom.agent.AdmitHttpTrust")? {
            return Ok(crate::agent::AdmitHttpTrustOutcome::NoCertificate { error: crate::agent::HttpTrustRefused { reason: crate::agent::HttpTrustRefusalReason::NoCertificate } });
        }
        // `webpki-only`: an accepting branch, in declaration order.
        if decided(equal(Some(&input.named).map(|value| *value), Some(false)), "loom.agent.AdmitHttpTrust")? {
            return Ok(crate::agent::AdmitHttpTrustOutcome::WebpkiOnly);
        }
        // `extended`: the default.
        return Ok(crate::agent::AdmitHttpTrustOutcome::Extended);
    }
}

/// `loom.agent.AdmitSend`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::AdmitSendBehavior for Generated<P>
{
    fn admit_send(&mut self, input: crate::agent::AdmitSend) -> Result<crate::agent::AdmitSendOutcome, UnmetObligation> {
        let _ = &input;
        // `writes-none`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.mode).map(|value| match value { crate::agent::WriteMode::Auto => "auto", crate::agent::WriteMode::Approval => "approval", crate::agent::WriteMode::None => "none" }.to_owned()), Some("none".to_owned())), "loom.agent.AdmitSend")? {
            return Ok(crate::agent::AdmitSendOutcome::WritesNone { error: crate::agent::SendRefused { reason: crate::agent::SendRefusalReason::WritesNone, r#final: true } });
        }
        // `outside-allow-list`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.channel_allowed).map(|value| *value), Some(false)), "loom.agent.AdmitSend")? {
            return Ok(crate::agent::AdmitSendOutcome::OutsideAllowList { error: crate::agent::SendRefused { reason: crate::agent::SendRefusalReason::OutsideAllowList, r#final: true } });
        }
        // `no-proposal`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(equal(Some(&input.proposed).map(|value| *value), Some(false)), "loom.agent.AdmitSend")? {
            return Ok(crate::agent::AdmitSendOutcome::NoProposal { error: crate::agent::SendRefused { reason: crate::agent::SendRefusalReason::NoProposal, r#final: false } });
        }
        // `rate-limited`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(compare_numbers(Some(&input.sent_in_window).map(|value| value.to_string()), Some(&input.max_per_hour).map(|value| value.to_string()), core::cmp::Ordering::is_ge), "loom.agent.AdmitSend")? {
            return Ok(crate::agent::AdmitSendOutcome::RateLimited { error: crate::agent::SendRefused { reason: crate::agent::SendRefusalReason::RateLimited, r#final: true } });
        }
        // `held`: an accepting branch, in declaration order.
        if decided(all(&[equal(Some(&input.mode).map(|value| match value { crate::agent::WriteMode::Auto => "auto", crate::agent::WriteMode::Approval => "approval", crate::agent::WriteMode::None => "none" }.to_owned()), Some("approval".to_owned())), equal(Some(&input.granted).map(|value| *value), Some(false))]), "loom.agent.AdmitSend")? {
            return Ok(crate::agent::AdmitSendOutcome::Held);
        }
        // `admitted`: the default.
        return Ok(crate::agent::AdmitSendOutcome::Admitted);
    }
}

/// `loom.agent.AdmitServe`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::AdmitServeBehavior for Generated<P>
{
    fn admit_serve(&mut self, input: crate::agent::AdmitServe) -> Result<crate::agent::AdmitServeOutcome, UnmetObligation> {
        let _ = &input;
        // `nothing-to-serve`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[equal(Some(&input.channel).map(|value| *value), Some(false)), equal(Some(&input.a2a).map(|value| *value), Some(false))]), "loom.agent.AdmitServe")? {
            return Ok(crate::agent::AdmitServeOutcome::NothingToServe { error: crate::agent::ServeRefused { reason: crate::agent::ServeRefusalReason::NothingToServe } });
        }
        // `polls-and-answers`: an accepting branch, in declaration order.
        if decided(all(&[equal(Some(&input.channel).map(|value| *value), Some(true)), equal(Some(&input.a2a).map(|value| *value), Some(true))]), "loom.agent.AdmitServe")? {
            return Ok(crate::agent::AdmitServeOutcome::PollsAndAnswers);
        }
        // `polls`: an accepting branch, in declaration order.
        if decided(equal(Some(&input.channel).map(|value| *value), Some(true)), "loom.agent.AdmitServe")? {
            return Ok(crate::agent::AdmitServeOutcome::Polls);
        }
        // `answers`: the default.
        return Ok(crate::agent::AdmitServeOutcome::Answers);
    }
}

/// `loom.agent.BoundHttpAnswer`, generated: every outcome is one the specification fully determines.
impl<P> crate::agent::obligations::BoundHttpAnswerBehavior for Generated<P>
{
    fn bound_http_answer(&mut self, input: crate::agent::BoundHttpAnswer) -> Result<crate::agent::BoundHttpAnswerOutcome, UnmetObligation> {
        let _ = &input;
        // `redirected`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(all(&[compare_numbers(Some(&input.status).map(|value| value.to_string()), Some("300".to_owned()), core::cmp::Ordering::is_ge), compare_numbers(Some(&input.status).map(|value| value.to_string()), Some("399".to_owned()), core::cmp::Ordering::is_le)]), "loom.agent.BoundHttpAnswer")? {
            return Ok(crate::agent::BoundHttpAnswerOutcome::Redirected { error: crate::agent::HttpReadRefused { reason: crate::agent::HttpReadRefusalReason::Redirected } });
        }
        // `failed-status`: an input-guarded refusal, before the addressed subject is loaded.
        if decided(any(&[compare_numbers(Some(&input.status).map(|value| value.to_string()), Some("199".to_owned()), core::cmp::Ordering::is_le), compare_numbers(Some(&input.status).map(|value| value.to_string()), Some("400".to_owned()), core::cmp::Ordering::is_ge)]), "loom.agent.BoundHttpAnswer")? {
            return Ok(crate::agent::BoundHttpAnswerOutcome::FailedStatus { error: crate::agent::HttpReadRefused { reason: crate::agent::HttpReadRefusalReason::FailedStatus } });
        }
        // `cut`: an accepting branch, in declaration order.
        if decided(compare_numbers(Some(&input.bytes).map(|value| value.to_string()), Some(&input.max_bytes).map(|value| value.to_string()), core::cmp::Ordering::is_gt), "loom.agent.BoundHttpAnswer")? {
            return Ok(crate::agent::BoundHttpAnswerOutcome::Cut);
        }
        // `whole`: the default.
        return Ok(crate::agent::BoundHttpAnswerOutcome::Whole);
    }
}

impl<P: crate::agent::obligations::CheckAgentSpecBehavior> crate::agent::obligations::CheckAgentSpecBehavior for Generated<P> {
    fn check_agent_spec(&mut self, input: crate::agent::CheckAgentSpec) -> Result<crate::agent::CheckAgentSpecOutcome, UnmetObligation> {
        crate::agent::obligations::CheckAgentSpecBehavior::check_agent_spec(&mut self.ports, input)
    }
}

/// `loom.resource.ApplyLoomAgent`, generated: every outcome is one the specification fully determines.
impl<P> crate::resource::obligations::ApplyLoomAgentBehavior for Generated<P>
where
    P: LoomAgentStorage,
{
    fn apply_loom_agent(&mut self, input: crate::resource::ApplyLoomAgent) -> Result<crate::resource::ApplyLoomAgentOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            let identity: crate::resource::ResourceName = input.name.clone();
            let data = crate::resource::LoomAgentData {
                name: identity.clone(),
                observed_generation: input.generation.clone(),
                check: None,
                deployment: None,
                variant: None,
                run: None,
                conditions: None,
            };
            let answer = crate::resource::ApplyLoomAgentOutcome::Created { loom_agent_applied: crate::resource::LoomAgentApplied { name: identity.clone(), generation: input.generation.clone() } };
            LoomAgentStorage::put(&mut self.ports, crate::resource::AnyLoomAgent::Pending(crate::resource::LoomAgent::new(data)).snapshot());
            return Ok(answer);
        };
        let _ = &held;
        // `stale`: selected by the addressed row.
        if decided(compare_numbers(Some(&held.data.observed_generation).map(|value| value.to_string()), Some(&input.generation).map(|value| value.to_string()), core::cmp::Ordering::is_gt), "loom.resource.ApplyLoomAgent")? {
            return Ok(crate::resource::ApplyLoomAgentOutcome::Stale { error: crate::resource::StaleGeneration { name: input.name.clone(), generation: input.generation.clone(), observed_generation: held.data.observed_generation.clone() } });
        }
        // `unchanged`: selected by the addressed row.
        if decided(compare_numbers(Some(&held.data.observed_generation).map(|value| value.to_string()), Some(&input.generation).map(|value| value.to_string()), core::cmp::Ordering::is_eq), "loom.resource.ApplyLoomAgent")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                let identity: crate::resource::ResourceName = input.name.clone();
                let data = crate::resource::LoomAgentData {
                    name: identity.clone(),
                    observed_generation: input.generation.clone(),
                    check: None,
                    deployment: None,
                    variant: None,
                    run: None,
                    conditions: None,
                };
                let answer = crate::resource::ApplyLoomAgentOutcome::Created { loom_agent_applied: crate::resource::LoomAgentApplied { name: identity.clone(), generation: input.generation.clone() } };
                LoomAgentStorage::put(&mut self.ports, crate::resource::AnyLoomAgent::Pending(crate::resource::LoomAgent::new(data)).snapshot());
                return Ok(answer);
            };
            let _ = &held;
            let answer = crate::resource::ApplyLoomAgentOutcome::Unchanged;
            return Ok(answer);
        }
        // `respecified`: the default.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            let identity: crate::resource::ResourceName = input.name.clone();
            let data = crate::resource::LoomAgentData {
                name: identity.clone(),
                observed_generation: input.generation.clone(),
                check: None,
                deployment: None,
                variant: None,
                run: None,
                conditions: None,
            };
            let answer = crate::resource::ApplyLoomAgentOutcome::Created { loom_agent_applied: crate::resource::LoomAgentApplied { name: identity.clone(), generation: input.generation.clone() } };
            LoomAgentStorage::put(&mut self.ports, crate::resource::AnyLoomAgent::Pending(crate::resource::LoomAgent::new(data)).snapshot());
            return Ok(answer);
        };
        let _ = &held;
        let moved = match held.refine() {
            crate::resource::AnyLoomAgent::Completed(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
            crate::resource::AnyLoomAgent::Degraded(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
            crate::resource::AnyLoomAgent::Failed(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
            crate::resource::AnyLoomAgent::Pending(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
            crate::resource::AnyLoomAgent::Refused(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
            crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Pending(instance.respecify()),
        };
        let mut next = moved.snapshot();
        next.data.observed_generation = input.generation.clone();
        next.data.check = None;
        next.data.deployment = None;
        next.data.variant = None;
        next.data.run = None;
        let answer = crate::resource::ApplyLoomAgentOutcome::Respecified { loom_agent_applied: crate::resource::LoomAgentApplied { name: input.name.clone(), generation: input.generation.clone() } };
        LoomAgentStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.resource.DeleteLoomAgent`, generated: every outcome is one the specification fully determines.
impl<P> crate::resource::obligations::DeleteLoomAgentBehavior for Generated<P>
where
    P: LoomAgentStorage,
{
    fn delete_loom_agent(&mut self, input: crate::resource::DeleteLoomAgent) -> Result<crate::resource::DeleteLoomAgentOutcome, UnmetObligation> {
        let _ = &input;
        // `deleted`: the default.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            return Ok(crate::resource::DeleteLoomAgentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
        };
        let _ = &held;
        let answer = crate::resource::DeleteLoomAgentOutcome::Deleted { loom_agent_deleted: crate::resource::LoomAgentDeleted { name: input.name.clone() } };
        LoomAgentStorage::delete(&mut self.ports, &input.name);
        return Ok(answer);
    }
}

/// `loom.resource.ObserveDeployment`, generated: every outcome is one the specification fully determines.
impl<P> crate::resource::obligations::ObserveDeploymentBehavior for Generated<P>
where
    P: LoomAgentStorage,
{
    fn observe_deployment(&mut self, input: crate::resource::ObserveDeployment) -> Result<crate::resource::ObserveDeploymentOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            return Ok(crate::resource::ObserveDeploymentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
        };
        let _ = &held;
        // `degraded`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Running)), equal(Some(&input.availability).map(|value| match value { crate::resource::DeploymentAvailability::Available => "Available", crate::resource::DeploymentAvailability::Unavailable => "Unavailable" }.to_owned()), Some("Unavailable".to_owned()))]), "loom.resource.ObserveDeployment")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveDeploymentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Degraded(instance.degrade()),
                _ => return Err(undeclared("loom.resource.ObserveDeployment")),
            };
            let mut next = moved.snapshot();
            next.data.deployment = Some(crate::resource::DeploymentStatus { availability: input.availability.clone() });
            let answer = crate::resource::ObserveDeploymentOutcome::Degraded { loom_agent_degraded: crate::resource::LoomAgentDegraded { name: input.name.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `still-running`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Running)), equal(Some(&input.availability).map(|value| match value { crate::resource::DeploymentAvailability::Available => "Available", crate::resource::DeploymentAvailability::Unavailable => "Unavailable" }.to_owned()), Some("Available".to_owned()))]), "loom.resource.ObserveDeployment")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveDeploymentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Running(instance.observe_running()),
                _ => return Err(undeclared("loom.resource.ObserveDeployment")),
            };
            let mut next = moved.snapshot();
            next.data.deployment = Some(crate::resource::DeploymentStatus { availability: input.availability.clone() });
            let answer = crate::resource::ObserveDeploymentOutcome::StillRunning { loom_agent_observed: crate::resource::LoomAgentObserved { name: input.name.clone(), availability: input.availability.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `recovered`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Degraded)), equal(Some(&input.availability).map(|value| match value { crate::resource::DeploymentAvailability::Available => "Available", crate::resource::DeploymentAvailability::Unavailable => "Unavailable" }.to_owned()), Some("Available".to_owned()))]), "loom.resource.ObserveDeployment")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveDeploymentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Degraded(instance) => crate::resource::AnyLoomAgent::Running(instance.recover()),
                _ => return Err(undeclared("loom.resource.ObserveDeployment")),
            };
            let mut next = moved.snapshot();
            next.data.deployment = Some(crate::resource::DeploymentStatus { availability: input.availability.clone() });
            let answer = crate::resource::ObserveDeploymentOutcome::Recovered { loom_agent_recovered: crate::resource::LoomAgentRecovered { name: input.name.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `still-degraded`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Degraded)), equal(Some(&input.availability).map(|value| match value { crate::resource::DeploymentAvailability::Available => "Available", crate::resource::DeploymentAvailability::Unavailable => "Unavailable" }.to_owned()), Some("Unavailable".to_owned()))]), "loom.resource.ObserveDeployment")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveDeploymentOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Degraded(instance) => crate::resource::AnyLoomAgent::Degraded(instance.observe_degraded()),
                _ => return Err(undeclared("loom.resource.ObserveDeployment")),
            };
            let mut next = moved.snapshot();
            next.data.deployment = Some(crate::resource::DeploymentStatus { availability: input.availability.clone() });
            let answer = crate::resource::ObserveDeploymentOutcome::StillDegraded { loom_agent_observed: crate::resource::LoomAgentObserved { name: input.name.clone(), availability: input.availability.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `no-deployment`: the default.
        return Ok(crate::resource::ObserveDeploymentOutcome::NoDeployment { error: crate::resource::LoomAgentStateConflict { name: input.name.clone() } });
    }
}

/// `loom.resource.ObserveRun`, generated: every outcome is one the specification fully determines.
impl<P> crate::resource::obligations::ObserveRunBehavior for Generated<P>
where
    P: LoomAgentStorage,
{
    fn observe_run(&mut self, input: crate::resource::ObserveRun) -> Result<crate::resource::ObserveRunOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            return Ok(crate::resource::ObserveRunOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
        };
        let _ = &held;
        // `still-active`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Running)), equal(Some(&input.phase).map(|value| match value { crate::resource::RunPhase::Active => "Active", crate::resource::RunPhase::Succeeded => "Succeeded", crate::resource::RunPhase::Failed => "Failed" }.to_owned()), Some("Active".to_owned()))]), "loom.resource.ObserveRun")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveRunOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Running(instance.observe_running()),
                _ => return Err(undeclared("loom.resource.ObserveRun")),
            };
            let next = moved.snapshot();
            let answer = crate::resource::ObserveRunOutcome::StillActive { loom_agent_run_observed: crate::resource::LoomAgentRunObserved { name: input.name.clone(), generation: before.observed_generation.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `completed`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Running)), equal(Some(&input.phase).map(|value| match value { crate::resource::RunPhase::Active => "Active", crate::resource::RunPhase::Succeeded => "Succeeded", crate::resource::RunPhase::Failed => "Failed" }.to_owned()), Some("Succeeded".to_owned()))]), "loom.resource.ObserveRun")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveRunOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Completed(instance.complete()),
                _ => return Err(undeclared("loom.resource.ObserveRun")),
            };
            let mut next = moved.snapshot();
            next.data.run = Some(crate::resource::RunEnd { exit_code: input.exit_code.clone(), finished_at: input.finished_at.clone(), reason: input.reason.clone() });
            let answer = crate::resource::ObserveRunOutcome::Completed { loom_agent_completed: crate::resource::LoomAgentCompleted { name: input.name.clone(), generation: before.observed_generation.clone(), exit_code: input.exit_code.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `failed`: selected by the addressed row.
        if decided(all(&[Some(matches!(held.state, crate::resource::LoomAgentState::Running)), equal(Some(&input.phase).map(|value| match value { crate::resource::RunPhase::Active => "Active", crate::resource::RunPhase::Succeeded => "Succeeded", crate::resource::RunPhase::Failed => "Failed" }.to_owned()), Some("Failed".to_owned()))]), "loom.resource.ObserveRun")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveRunOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Running(instance) => crate::resource::AnyLoomAgent::Failed(instance.fail()),
                _ => return Err(undeclared("loom.resource.ObserveRun")),
            };
            let mut next = moved.snapshot();
            next.data.run = Some(crate::resource::RunEnd { exit_code: input.exit_code.clone(), finished_at: input.finished_at.clone(), reason: input.reason.clone() });
            let answer = crate::resource::ObserveRunOutcome::Failed { loom_agent_failed: crate::resource::LoomAgentFailed { name: input.name.clone(), generation: before.observed_generation.clone(), exit_code: input.exit_code.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `finished`: selected by the addressed row.
        if decided(Some(matches!(held.state, crate::resource::LoomAgentState::Completed | crate::resource::LoomAgentState::Failed)), "loom.resource.ObserveRun")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::ObserveRunOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let answer = crate::resource::ObserveRunOutcome::Finished;
            return Ok(answer);
        }
        // `no-run`: the default.
        return Ok(crate::resource::ObserveRunOutcome::NoRun { error: crate::resource::LoomAgentStateConflict { name: input.name.clone() } });
    }
}

/// `loom.resource.RecordCheck`, generated: every outcome is one the specification fully determines.
impl<P> crate::resource::obligations::RecordCheckBehavior for Generated<P>
where
    P: LoomAgentStorage,
{
    fn record_check(&mut self, input: crate::resource::RecordCheck) -> Result<crate::resource::RecordCheckOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            return Ok(crate::resource::RecordCheckOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
        };
        let _ = &held;
        // `stale`: selected by the addressed row.
        if decided(compare_numbers(Some(&held.data.observed_generation).map(|value| value.to_string()), Some(&input.generation).map(|value| value.to_string()), core::cmp::Ordering::is_ne), "loom.resource.RecordCheck")? {
            return Ok(crate::resource::RecordCheckOutcome::Stale { error: crate::resource::StaleGeneration { name: input.name.clone(), generation: input.generation.clone(), observed_generation: held.data.observed_generation.clone() } });
        }
        // `running`: an accepting branch, in declaration order.
        if decided(compare_numbers(Some(&input.exit_code).map(|value| value.to_string()), Some("0".to_owned()), core::cmp::Ordering::is_eq), "loom.resource.RecordCheck")? {
            let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
                return Ok(crate::resource::RecordCheckOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
            };
            let _ = &held;
            let moved = match held.refine() {
                crate::resource::AnyLoomAgent::Pending(instance) => crate::resource::AnyLoomAgent::Running(instance.run()),
                _ => return Ok(crate::resource::RecordCheckOutcome::OutOfOrder { error: crate::resource::LoomAgentStateConflict { name: input.name.clone() } }),
            };
            let mut next = moved.snapshot();
            next.data.check = Some(crate::resource::CheckResult { exit_code: input.exit_code.clone(), message: input.message.clone() });
            next.data.variant = Some(input.variant.clone());
            let answer = crate::resource::RecordCheckOutcome::Running { loom_agent_running: crate::resource::LoomAgentRunning { name: input.name.clone(), generation: input.generation.clone() } };
            LoomAgentStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `refused`: the default.
        let Some(held) = LoomAgentStorage::get(&self.ports, &input.name) else {
            return Ok(crate::resource::RecordCheckOutcome::Unknown { error: crate::resource::LoomAgentNotFound { name: input.name.clone() } });
        };
        let _ = &held;
        let moved = match held.refine() {
            crate::resource::AnyLoomAgent::Pending(instance) => crate::resource::AnyLoomAgent::Refused(instance.refuse()),
            _ => return Ok(crate::resource::RecordCheckOutcome::OutOfOrder { error: crate::resource::LoomAgentStateConflict { name: input.name.clone() } }),
        };
        let mut next = moved.snapshot();
        next.data.check = Some(crate::resource::CheckResult { exit_code: input.exit_code.clone(), message: input.message.clone() });
        next.data.variant = Some(input.variant.clone());
        let answer = crate::resource::RecordCheckOutcome::Refused { loom_agent_refused: crate::resource::LoomAgentRefused { name: input.name.clone(), generation: input.generation.clone(), message: input.message.clone() } };
        LoomAgentStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.OverruleSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::OverruleSelectionBehavior for Generated<P>
where
    P: SelectionStorage,
{
    fn overrule_selection(&mut self, input: crate::run::OverruleSelection) -> Result<crate::run::OverruleSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // `overruled`: the default.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::OverruleSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Overruled(instance.overrule()),
            _ => return Ok(crate::run::OverruleSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
        };
        let mut next = moved.snapshot();
        next.data.replaced_by = Some(input.replacement_id.clone());
        let answer = crate::run::OverruleSelectionOutcome::Overruled { selection_overruled: crate::run::SelectionOverruled { selection_id: input.selection_id.clone(), replacement_id: input.replacement_id.clone() } };
        SelectionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

/// `loom.run.ProjectCatalogue`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::ProjectCatalogueBehavior for Generated<P>
where
    P: ActionCatalogueStorage,
{
    fn project_catalogue(&mut self, input: crate::run::ProjectCatalogue) -> Result<crate::run::ProjectCatalogueOutcome, UnmetObligation> {
        let _ = &input;
        // `catalogue-exists`: an identity a record already carries, before any branch is taken.
        if ActionCatalogueStorage::get(&self.ports, &input.catalogue_id).is_some() {
            return Ok(crate::run::ProjectCatalogueOutcome::CatalogueExists { error: crate::run::CatalogueExists { catalogue_id: input.catalogue_id.clone() } });
        }
        // `projected`: the default.
        let identity: crate::run::CatalogueId = input.catalogue_id.clone();
        let data = crate::run::ActionCatalogueData {
            catalogue_id: identity.clone(),
            turn_id: input.turn_id.clone(),
            frontier: input.frontier.clone(),
            case_revision: input.case_revision.clone(),
            entries: input.entries.clone(),
        };
        let answer = crate::run::ProjectCatalogueOutcome::Projected { catalogue_projected: crate::run::CatalogueProjected { catalogue_id: identity.clone(), turn_id: input.turn_id.clone(), case_revision: input.case_revision.clone() } };
        ActionCatalogueStorage::put(&mut self.ports, crate::run::AnyActionCatalogue::Projected(crate::run::ActionCatalogue::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.RecordSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RecordSelectionBehavior for Generated<P>
where
    P: SelectionStorage + SelectionRecordStorage,
{
    fn record_selection(&mut self, input: crate::run::RecordSelection) -> Result<crate::run::RecordSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // `record-exists`: an identity a record already carries, before any branch is taken.
        if SelectionRecordStorage::get(&self.ports, &input.selection_record_id).is_some() {
            return Ok(crate::run::RecordSelectionOutcome::RecordExists { error: crate::run::SelectionRecordExists { selection_record_id: input.selection_record_id.clone() } });
        }
        // `when_related:` reads the `loom.run.Selection` row `input.selection_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.selection_id);
        let related = reference.and_then(|identity| SelectionStorage::get(&self.ports, identity));
        // `selection-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RecordSelectionOutcome::SelectionUnknown { error: crate::run::SelectionNotFound { selection_id: input.selection_id.clone() } });
        }
        let _ = &related;
        // `recorded`: the default.
        let identity: crate::run::SelectionRecordId = input.selection_record_id.clone();
        let data = crate::run::SelectionRecordData {
            selection_record_id: identity.clone(),
            selection_id: input.selection_id.clone(),
            strategy: input.strategy.clone(),
            candidate_count: input.candidate_count.clone(),
            chosen_action: input.chosen_action.clone(),
            confidence: input.confidence.clone(),
            fell_back_to: input.fell_back_to.clone(),
            latency_ms: input.latency_ms.clone(),
            input_tokens: input.input_tokens.clone(),
            output_tokens: input.output_tokens.clone(),
        };
        let answer = crate::run::RecordSelectionOutcome::Recorded { selection_recorded: crate::run::SelectionRecorded { selection_record_id: identity.clone(), selection_id: input.selection_id.clone(), strategy: input.strategy.clone(), fell_back_to: input.fell_back_to.clone() } };
        SelectionRecordStorage::put(&mut self.ports, crate::run::AnySelectionRecord::Recorded(crate::run::SelectionRecord::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.RequestArguments`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RequestArgumentsBehavior for Generated<P>
where
    P: ArgumentRequestStorage + SelectionStorage,
{
    fn request_arguments(&mut self, input: crate::run::RequestArguments) -> Result<crate::run::RequestArgumentsOutcome, UnmetObligation> {
        let _ = &input;
        // `when_related:` reads the `loom.run.Selection` row `input.selection_id` names, through its storage port; an absent
        // reference reads no row and selects no related branch.
        let reference = Some(&input.selection_id);
        let related = reference.and_then(|identity| SelectionStorage::get(&self.ports, identity));
        // `selection-unknown`: the reference names an identity no row carries.
        if reference.is_some() && related.is_none() {
            return Ok(crate::run::RequestArgumentsOutcome::SelectionUnknown { error: crate::run::SelectionNotFound { selection_id: input.selection_id.clone() } });
        }
        let _ = &related;
        // `selection-not-selected`: selected by the present related row, in declaration order.
        if let Some(related) = &related {
        if decided(equal(Some(&related.state).map(|value| match value { crate::run::SelectionState::Admitted => "Admitted", crate::run::SelectionState::Overruled => "Overruled", crate::run::SelectionState::Refused => "Refused", crate::run::SelectionState::Selected => "Selected" }.to_owned()), Some("Selected".to_owned())).map(|value| !value), "loom.run.RequestArguments")? {
            return Ok(crate::run::RequestArgumentsOutcome::SelectionNotSelected { error: crate::run::SelectionNotSelected { selection_id: input.selection_id.clone() } });
        }
        }
        // `requested`: the default.
        let identity: crate::run::ArgumentRequestId = input.argument_request_id.clone();
        let data = crate::run::ArgumentRequestData {
            argument_request_id: identity.clone(),
            selection_id: input.selection_id.clone(),
        };
        let answer = crate::run::RequestArgumentsOutcome::Requested { arguments_requested: crate::run::ArgumentsRequested { argument_request_id: identity.clone(), selection_id: input.selection_id.clone() } };
        ArgumentRequestStorage::put(&mut self.ports, crate::run::AnyArgumentRequest::Requested(crate::run::ArgumentRequest::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.run.RevalidateSelection`, generated: every outcome is one the specification fully determines.
impl<P> crate::run::obligations::RevalidateSelectionBehavior for Generated<P>
where
    P: TryContext + SelectionStorage,
{
    fn revalidate_selection(&mut self, input: crate::run::RevalidateSelection) -> Result<crate::run::RevalidateSelectionOutcome, UnmetObligation> {
        let _ = &input;
        // The addressed row, read before the branches that select by it.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        // `stale-revision`: selected by the addressed row.
        if decided(compare_numbers(Some(&held.data.case_revision).map(|value| value.to_string()), Some(&input.case_revision).map(|value| value.to_string()), core::cmp::Ordering::is_ne), "loom.run.RevalidateSelection")? {
            let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
                return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
            };
            let _ = &held;
            let held_state = held.state;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Refused(instance.refuse()),
                _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
            };
            let next = moved.snapshot();
            let answer = crate::run::RevalidateSelectionOutcome::StaleRevision { selection_stale: crate::run::SelectionStale { selection_id: input.selection_id.clone(), catalogue_revision: before.case_revision.clone(), case_revision: input.case_revision.clone() } };
            SelectionStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `not-in-frontier`: an external branch, where the context takes it.
        if self.ports.try_external(ExternalCommand::LoomRunRevalidateSelection(&input), "not-in-frontier")? {
            let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
                return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
            };
            let _ = &held;
            let held_state = held.state;
            let before = held.data.clone();
            let moved = match held.refine() {
                crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Refused(instance.refuse()),
                _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
            };
            let next = moved.snapshot();
            let answer = crate::run::RevalidateSelectionOutcome::NotInFrontier { selection_not_in_frontier: crate::run::SelectionNotInFrontier { selection_id: input.selection_id.clone(), action: before.action.clone() } };
            SelectionStorage::put(&mut self.ports, next);
            return Ok(answer);
        }
        // `admitted`: the default.
        let Some(held) = SelectionStorage::get(&self.ports, &input.selection_id) else {
            return Ok(crate::run::RevalidateSelectionOutcome::WrongStateUnknownInstance);
        };
        let _ = &held;
        let held_state = held.state;
        let moved = match held.refine() {
            crate::run::AnySelection::Selected(instance) => crate::run::AnySelection::Admitted(instance.admit()),
            _ => return Ok(crate::run::RevalidateSelectionOutcome::WrongState { error: crate::run::SelectionStateConflict { state: held_state } }),
        };
        let next = moved.snapshot();
        let answer = crate::run::RevalidateSelectionOutcome::Admitted { selection_admitted: crate::run::SelectionAdmitted { selection_id: input.selection_id.clone() } };
        SelectionStorage::put(&mut self.ports, next);
        return Ok(answer);
    }
}

impl<P: crate::run::obligations::SelectActionBehavior> crate::run::obligations::SelectActionBehavior for Generated<P> {
    fn select_action(&mut self, input: crate::run::SelectAction) -> Result<crate::run::SelectActionOutcome, UnmetObligation> {
        crate::run::obligations::SelectActionBehavior::select_action(&mut self.ports, input)
    }
}

/// `loom.session.ProposeSessionChange`, generated: every outcome is one the specification fully determines.
impl<P> crate::session::obligations::ProposeSessionChangeBehavior for Generated<P>
where
    P: AgentSessionStorage,
{
    fn propose_session_change(&mut self, input: crate::session::ProposeSessionChange) -> Result<crate::session::ProposeSessionChangeOutcome, UnmetObligation> {
        let _ = &input;
        // `proposed`: the default.
        let identity: crate::session::SessionId = input.session_id.clone();
        let data = crate::session::AgentSessionData {
            session_id: identity.clone(),
            kind: input.kind.clone(),
            commission_id: input.commission_id.clone(),
            frontier_id: input.frontier_id.clone(),
            vendor_session_id: input.vendor_session_id.clone(),
            agent: input.agent.clone(),
            continues: input.continues.clone(),
            action: input.action.clone(),
            files: input.files.clone(),
        };
        let answer = crate::session::ProposeSessionChangeOutcome::Proposed { session_change_proposed: crate::session::SessionChangeProposed { session_id: identity.clone(), commission_id: input.commission_id.clone(), frontier_id: input.frontier_id.clone(), continues: input.continues.clone(), action: input.action.clone(), files: input.files.clone() } };
        AgentSessionStorage::put(&mut self.ports, crate::session::AnyAgentSession::Ended(crate::session::AgentSession::new(data)).snapshot());
        return Ok(answer);
    }
}

/// `loom.resource.LoomAgents`, generated: every row is one the specification fully determines from the stored `loom.resource.LoomAgent`s.
impl<P> crate::resource::obligations::LoomAgentsQuery for Generated<P>
where
    P: LoomAgentStorage,
{
    fn loom_agents(&self) -> Result<Vec<crate::resource::LoomAgents>, UnmetObligation> {
        let admitted = LoomAgentStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::resource::LoomAgents {
                name: held.data.name,
                observed_generation: held.data.observed_generation,
                check: held.data.check,
                deployment: held.data.deployment,
                variant: held.data.variant,
                run: held.data.run,
                conditions: held.data.conditions,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.run.Catalogues`, generated: every row is one the specification fully determines from the stored `loom.run.ActionCatalogue`s.
impl<P> crate::run::obligations::CataloguesQuery for Generated<P>
where
    P: ActionCatalogueStorage,
{
    fn catalogues(&self) -> Result<Vec<crate::run::Catalogues>, UnmetObligation> {
        let admitted = ActionCatalogueStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::Catalogues {
                catalogue_id: held.data.catalogue_id,
                case_revision: held.data.case_revision,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.run.SelectionRecords`, generated: every row is one the specification fully determines from the stored `loom.run.SelectionRecord`s.
impl<P> crate::run::obligations::SelectionRecordsQuery for Generated<P>
where
    P: SelectionRecordStorage,
{
    fn selection_records(&self) -> Result<Vec<crate::run::SelectionRecords>, UnmetObligation> {
        let admitted = SelectionRecordStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::SelectionRecords {
                selection_record_id: held.data.selection_record_id,
                selection_id: held.data.selection_id,
                strategy: held.data.strategy,
                candidate_count: held.data.candidate_count,
                chosen_action: held.data.chosen_action,
                fell_back_to: held.data.fell_back_to,
                latency_ms: held.data.latency_ms,
                input_tokens: held.data.input_tokens,
                output_tokens: held.data.output_tokens,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.run.Selections`, generated: every row is one the specification fully determines from the stored `loom.run.Selection`s.
impl<P> crate::run::obligations::SelectionsQuery for Generated<P>
where
    P: SelectionStorage,
{
    fn selections(&self) -> Result<Vec<crate::run::Selections>, UnmetObligation> {
        let admitted = SelectionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::run::Selections {
                selection_id: held.data.selection_id,
                catalogue_id: held.data.catalogue_id,
                action: held.data.action,
                strategy: held.data.strategy,
                case_revision: held.data.case_revision,
                replaced_by: held.data.replaced_by,
                state: held.state,
            })
            .collect())
    }
}

/// `loom.session.AgentSessions`, generated: every row is one the specification fully determines from the stored `loom.session.AgentSession`s.
impl<P> crate::session::obligations::AgentSessionsQuery for Generated<P>
where
    P: AgentSessionStorage,
{
    fn agent_sessions(&self) -> Result<Vec<crate::session::AgentSessions>, UnmetObligation> {
        let admitted = AgentSessionStorage::list(&self.ports);
        Ok(admitted
            .into_iter()
            .map(|held| crate::session::AgentSessions {
                session_id: held.data.session_id,
                kind: held.data.kind,
                commission_id: held.data.commission_id,
                frontier_id: held.data.frontier_id,
                vendor_session_id: held.data.vendor_session_id,
                agent: held.data.agent,
                continues: held.data.continues,
                action: held.data.action,
                files: held.data.files,
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

/// Three-valued disjunction: true wins, then Unknown.
fn any(truths: &[Option<bool>]) -> Option<bool> {
    if truths.contains(&Some(true)) {
        Some(true)
    } else if truths.contains(&None) {
        None
    } else {
        Some(false)
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
