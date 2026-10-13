//! Catalog-based intent orchestration. Resource initialization follows the accepted route.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::model_retry::classify_with_retries;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    AgentRevisionId, CaseId, CommissionData, CommissionId,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{ResumeError, ResumeRefusal};
use b10x_loom_intake_references::references;
use b10x_loom_intake_router::RouterError;
use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::primitives::{Decimal, Uuid as IntakeUuid};
use llm_core::Model;
use loom_governor::{CanonGovernor, FallibleCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

use crate::case;
use crate::clock::Clock;
use crate::confinement::TestRunner;
use crate::context_metrics::{ContextMetrics, MeasuredModel};
use crate::executor::{TestCommand, backend_name};
use crate::grant::{GrantAuthority, GrantStore};
use crate::handoff::MergeHandoffStore;
use crate::resume::{
    self, AcceptedRoute, CatalogDigest, DropResumeContext, DropResumeContextOutcome,
    ResumeContextData, ResumeContextStore,
};
use crate::run::{
    Execution, Holding, LOCAL_PROTOCOL, Mode, Resuming, RunOptions, SliceError, SliceRun,
    StopReason, drive, operator_commission, printable,
};
use crate::selector::Briefing;

/// General intent input. A workspace and test runner are needed only for software changes.
#[derive(Clone)]
pub struct IntentRequest {
    pub intent: String,
    pub workspace: Option<PathBuf>,
    pub test: TestCommand,
    pub runner: Option<Arc<dyn TestRunner>>,
    pub max_steps: usize,
    pub threshold: f64,
}

/// A route accepted for this exact intent, catalog and context policy; never authority.
#[derive(Debug)]
pub struct PreparedIntent {
    protocol: String,
    binding: Value,
}

impl PreparedIntent {
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// For a private, host-created delegation file only; never accept this from model arguments.
    pub fn handoff(&self) -> Value {
        json!({"protocol": self.protocol, "binding": self.binding})
    }

    /// Restore a host handoff only after its containing file has been authenticated by the host.
    pub fn restore(
        value: &Value,
        request: &IntentRequest,
        catalog: &ProtocolCatalog,
        options: &RunOptions,
    ) -> Result<Self, SliceError> {
        let protocol = value["protocol"]
            .as_str()
            .ok_or_else(|| SliceError::Context("invalid routing handoff".into()))?;
        if value["binding"] != binding(request, catalog, options) || catalog.get(protocol).is_none()
        {
            return Err(SliceError::Context(
                "routing handoff does not match intent, catalog or options".into(),
            ));
        }
        Ok(Self {
            protocol: protocol.into(),
            binding: value["binding"].clone(),
        })
    }

    /// The route as the resume context keeps it (`intake.routing.AcceptedRoute`): the handoff's
    /// protocol and binding, typed.
    ///
    /// # Errors
    /// [`SliceError::State`] when the binding is not the one this module makes.
    pub fn accepted_route(&self) -> Result<AcceptedRoute, SliceError> {
        let invalid = || SliceError::State("the accepted route's binding is not readable".into());
        let text = |value: &Value| value.as_str().map(str::to_owned).ok_or_else(invalid);
        let catalog = self.binding["catalog"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|entry| {
                Ok(CatalogDigest {
                    name: text(&entry["name"])?,
                    digest: text(&entry["digest"])?,
                })
            })
            .collect::<Result<_, SliceError>>()?;
        let context_policy = match self.binding["policy"].as_str() {
            Some("Legacy") => ContextPolicy::Legacy,
            Some("Bounded") => ContextPolicy::Bounded,
            _ => return Err(invalid()),
        };
        let threshold = match &self.binding["threshold"] {
            Value::Number(number) => Decimal(number.to_string()),
            _ => return Err(invalid()),
        };
        Ok(AcceptedRoute {
            protocol: self.protocol.clone(),
            intent_revision: text(&self.binding["intent"])?,
            catalog,
            context_policy,
            threshold,
        })
    }

    /// Restores a route a resume context kept, as [`PreparedIntent::restore`] restores a handoff:
    /// only when it was accepted for exactly `request`'s intent and threshold, `catalog` and
    /// `options`' context policy.
    ///
    /// # Errors
    /// [`SliceError::Context`] when it was not, or names a protocol `catalog` does not hold.
    pub fn restore_route(
        route: &AcceptedRoute,
        request: &IntentRequest,
        catalog: &ProtocolCatalog,
        options: &RunOptions,
    ) -> Result<Self, SliceError> {
        let threshold = route
            .threshold
            .0
            .parse::<f64>()
            .map_err(|_| SliceError::Context("invalid routing handoff".into()))?;
        let entries: Vec<Value> = route
            .catalog
            .iter()
            .map(|entry| json!({"name": entry.name, "digest": entry.digest}))
            .collect();
        let policy = match route.context_policy {
            ContextPolicy::Legacy => "Legacy",
            ContextPolicy::Bounded => "Bounded",
        };
        let handoff = json!({
            "protocol": route.protocol,
            "binding": {
                "intent": route.intent_revision,
                "catalog": entries,
                "policy": policy,
                "threshold": threshold,
            },
        });
        Self::restore(&handoff, request, catalog, options)
    }
}

pub enum Preparation {
    Ready(PreparedIntent),
    Stopped(SliceRun),
}

fn binding(request: &IntentRequest, catalog: &ProtocolCatalog, options: &RunOptions) -> Value {
    let entries: Vec<_> = catalog
        .iter()
        .map(|entry| json!({"name":entry.name(),"digest":entry.definition.sha256}))
        .collect();
    json!({"intent":case::intent_revision(&request.intent),"catalog":entries,"policy":format!("{:?}",options.context_policy), "threshold":request.threshold})
}

/// Classify once before creating protocol-specific resources.
pub fn prepare_intent(
    request: &IntentRequest,
    catalog: &ProtocolCatalog,
    classifier: &dyn Model,
    out: &mut dyn Write,
    options: &RunOptions,
    metrics: &ContextMetrics,
) -> Result<Preparation, SliceError> {
    if metrics.report().policy != options.context_policy {
        return Err(SliceError::Context(
            "measurement policy differs from context policy".into(),
        ));
    }
    for reference in references(&request.intent) {
        writeln!(
            out,
            "reference: {:?} {}",
            reference.kind,
            printable(&reference.value)
        )?;
    }
    let model = MeasuredModel::new(classifier, metrics.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(SliceError::Runtime)?;
    let result = runtime.block_on(classify_with_retries(
        &request.intent,
        &model,
        request.threshold,
        Some(catalog),
    ));
    drop(runtime);
    let pick = match result {
        Ok(pick) => pick,
        Err(error @ RouterError::OutsideRegistry { .. })
        | Err(error @ RouterError::Unsure { .. }) => {
            let (protocol, detail) = match &error {
                RouterError::OutsideRegistry { protocol } => (protocol.clone(), "outside registry"),
                RouterError::Unsure { protocol, .. } => (protocol.clone(), "unsure"),
                _ => unreachable!(),
            };
            writeln!(out, "refused: {}", printable(&error.to_string()))?;
            return stop(out, protocol, StopReason::Refused, detail).map(Preparation::Stopped);
        }
        Err(error) => return Err(SliceError::Router(error)),
    };
    writeln!(
        out,
        "picked {} (confidence {})",
        printable(&pick.protocol),
        pick.confidence
    )?;
    for reason in pick.reasons {
        writeln!(out, "  reason: {}", printable(&reason))?;
    }
    let entry = catalog
        .get(&pick.protocol)
        .expect("router validates catalog membership");
    if pick.protocol != LOCAL_PROTOCOL
        && let Err(reason) = entry.clock_compatible()
    {
        return stop(out, pick.protocol, StopReason::NoLocalExecutor, &reason)
            .map(Preparation::Stopped);
    }
    Ok(Preparation::Ready(PreparedIntent {
        protocol: pick.protocol,
        binding: binding(request, catalog, options),
    }))
}

fn stop(
    out: &mut dyn Write,
    protocol: String,
    reason: StopReason,
    detail: &str,
) -> Result<SliceRun, SliceError> {
    writeln!(out, "stopped: {reason:?} ({})", printable(detail))?;
    Ok(SliceRun {
        protocol,
        steps: 0,
        stop_reason: reason,
        approvals: Vec::new(),
    })
}

/// Execute an accepted route with host-provided resources and the same immutable catalog.
#[allow(clippy::too_many_arguments)]
pub fn run_prepared_intent<S: FallibleCaseStore>(
    request: &IntentRequest,
    prepared: &PreparedIntent,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    clock: &dyn Clock,
    out: &mut dyn Write,
    options: &RunOptions,
    metrics: &ContextMetrics,
) -> Result<SliceRun, SliceError> {
    run_prepared(
        request, prepared, catalog, governor, frontiers, agent, clock, out, options, metrics, None,
    )
}

/// What `run --state` keeps a commission in, beside the case its governor's store holds: the
/// suspension store a withheld approval's Suspension is held in, the resume context store, and
/// what the host knows of the run that the slice does not.
pub struct RunState<'s> {
    /// The suspension store `run_with_suspension` holds the commission's Suspension in.
    pub suspensions: &'s dyn SuspensionPort,
    /// Where the commission's resume context is kept before it runs.
    pub contexts: &'s ResumeContextStore,
    /// The host's part of the resume context.
    pub host: ResumeHost,
}

/// What the host knows of a run that a resume needs and the slice does not: how the test command
/// is confined, and which agent model runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeHost {
    /// The confinement the operator asked for.
    pub confinement: Backend,
    /// The delegated cgroup root the operator gave, if any.
    pub cgroup_root: Option<PathBuf>,
    /// The agent model: a Codex model name, or with `catalog` a route alias of that llm catalog.
    pub model: String,
    /// The llm catalog the agent model's alias names a route of, if any.
    pub catalog: Option<PathBuf>,
}

impl ResumeHost {
    /// Checks that a resume context of `request` run on this host can be kept: every path and
    /// word it keeps as text (the workspace, made absolute, the test command, the cgroup root and
    /// the catalog path) is UTF-8, and `max_steps` fits its integer. A host calls it before it
    /// calls any model or opens any store, so a run that could not keep its context starts
    /// nothing.
    ///
    /// # Errors
    /// [`SliceError::State`], naming the path or word that cannot be kept.
    pub fn check(&self, request: &IntentRequest) -> Result<(), SliceError> {
        HostText::of(request, self).map(|_| ())
    }
}

/// What a resume context keeps of a request and its host as text, converted in one place
/// for [`ResumeHost::check`] and for the context itself, so the two never disagree.
struct HostText {
    workspace: Option<String>,
    test_command: Vec<String>,
    max_steps: i64,
    cgroup_root: Option<String>,
    catalog: Option<String>,
}

impl HostText {
    fn of(request: &IntentRequest, host: &ResumeHost) -> Result<Self, SliceError> {
        let test_command = std::iter::once(request.test.program())
            .chain(request.test.args().iter().map(OsString::as_os_str))
            .map(|word| {
                word.to_str().map(str::to_owned).ok_or_else(|| {
                    SliceError::State(format!("the test command word {word:?} is not UTF-8"))
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            workspace: request
                .workspace
                .as_deref()
                .map(|path| absolute_text("the workspace", path))
                .transpose()?,
            test_command,
            max_steps: i64::try_from(request.max_steps)
                .map_err(|_| SliceError::State("max_steps does not fit a 64-bit integer".into()))?,
            cgroup_root: host
                .cgroup_root
                .as_deref()
                .map(|path| absolute_text("the cgroup root", path))
                .transpose()?,
            catalog: host
                .catalog
                .as_deref()
                .map(|path| absolute_text("the catalog", path))
                .transpose()?,
        })
    }
}

/// Executes an accepted route as [`run_prepared_intent`] does, holding the commission in `state`:
/// its resume context is kept before it runs, and it runs through Commission's
/// `run_with_suspension`, so a withheld approval leaves a Suspension in `state.suspensions`.
/// When the run ends, however it ends, without the commission's Suspension held, the context is
/// removed (`intake.routing.DropResumeContext`): nothing can resume that commission. A process
/// that dies before the run ends leaves the context behind (see [`crate::resume`]).
///
/// # Errors
/// As [`run_prepared_intent`], plus [`SliceError::State`] when the resume context cannot be kept
/// (nothing runs then), and [`SliceError::Loop`] when the suspension store cannot be used. A run
/// that ended without failing is [`SliceError::State`] when the suspension store cannot say
/// whether the commission holds its Suspension (the context is kept then) or its context cannot be
/// removed; a run that failed reports its own failure.
#[allow(clippy::too_many_arguments)]
pub fn run_prepared_intent_with_state<S: FallibleCaseStore>(
    request: &IntentRequest,
    prepared: &PreparedIntent,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    clock: &dyn Clock,
    out: &mut dyn Write,
    options: &RunOptions,
    metrics: &ContextMetrics,
    state: &RunState<'_>,
) -> Result<SliceRun, SliceError> {
    let kept = RefCell::new(None);
    let keep = |commission: &CommissionData| {
        let context = resume_context(request, prepared, &state.host, commission)?;
        state
            .contexts
            .keep(&context)
            .map_err(|error| SliceError::State(error.to_string()))?;
        *kept.borrow_mut() = Some(commission.commission_id.clone());
        Ok(())
    };
    let holding = Holding {
        suspensions: state.suspensions,
        keep: &keep,
    };
    let ran = run_prepared(
        request,
        prepared,
        catalog,
        governor,
        frontiers,
        agent,
        clock,
        out,
        options,
        metrics,
        Some(&holding),
    );
    let Some(commission) = kept.into_inner() else {
        return ran;
    };
    let settled = settle(state.suspensions, state.contexts, &commission);
    match ran {
        Ok(run) => settled.map(|()| run),
        Err(failure) => Err(failure),
    }
}

/// Keeps `commission`'s resume context when the commission holds its Suspension, and removes it
/// otherwise. When the suspension store cannot say, the context is kept.
fn settle(
    suspensions: &dyn SuspensionPort,
    contexts: &ResumeContextStore,
    commission: &CommissionId,
) -> Result<(), SliceError> {
    let held = suspensions.get(commission).map_err(|error| {
        SliceError::State(format!(
            "whether commission {} holds a Suspension cannot be read, so its resume context is \
             kept: {error}",
            commission.0.0
        ))
    })?;
    if held.is_some() {
        return Ok(());
    }
    let input = DropResumeContext {
        commission_id: resume::CommissionId(IntakeUuid(commission.0.0.clone())),
    };
    match contexts.remove(&input) {
        // Gone either way: none is left for a commission nothing can resume.
        DropResumeContextOutcome::Dropped | DropResumeContextOutcome::NotFound { .. } => Ok(()),
        DropResumeContextOutcome::StorageFailed { error } => Err(SliceError::State(format!(
            "commission {} holds no Suspension, and its resume context cannot be removed: {}",
            commission.0.0, error.reason
        ))),
    }
}

/// What `resume` continues a commission from, in the directory `run --state` kept it in: the
/// suspension store its Suspension is held in, its resume context, the grant `approve` wrote for
/// it, and where a granted `repository.merge` is handed off.
pub struct ResumeState<'s> {
    /// The suspension store the commission's Suspension is held in.
    pub suspensions: &'s dyn SuspensionPort,
    /// Where the commission's resume context is kept.
    pub contexts: &'s ResumeContextStore,
    /// Where `approve` wrote the commission's grant.
    pub grants: &'s GrantStore,
    /// Where a granted `repository.merge` is handed off.
    pub handoffs: &'s MergeHandoffStore,
}

/// The resume context of `commission`, read only when the commission holds a Suspension: a
/// context alone, which a run that died early leaves behind, resumes nothing.
///
/// # Errors
/// [`SliceError::Resume`] refusing `NothingSuspended` when the commission holds no Suspension, or
/// with `Store` when the suspension store cannot be read; [`SliceError::State`] when no context is
/// kept for the commission or it cannot be read.
pub fn held_resume_context(
    state: &ResumeState<'_>,
    commission: &str,
) -> Result<ResumeContextData, SliceError> {
    let held = state
        .suspensions
        .get(&CommissionId(CommissionUuid(commission.to_owned())))
        .map_err(|error| SliceError::Resume(ResumeError::Store(error)))?;
    if held.is_none() {
        return Err(SliceError::Resume(ResumeError::Refused(
            ResumeRefusal::NothingSuspended,
        )));
    }
    state
        .contexts
        .get(&resume::CommissionId(IntakeUuid(commission.to_owned())))
        .map_err(|error| SliceError::State(error.to_string()))?
        .ok_or_else(|| {
            SliceError::State(format!(
                "commission {} holds a Suspension, but no resume context is kept for it",
                printable(commission)
            ))
        })
}

/// The request `context` keeps, as the run that kept it was given it: the intent, the
/// workspace, the test command, the step budget and the threshold, with `runner`.
///
/// # Errors
/// [`SliceError::State`] when the context keeps no test program, a negative step budget or a
/// threshold that is not a number.
pub fn resumed_request(
    context: &ResumeContextData,
    runner: Option<Arc<dyn TestRunner>>,
) -> Result<IntentRequest, SliceError> {
    let invalid = |what: &str| SliceError::State(format!("the resume context keeps {what}"));
    let (program, args) = context
        .test_command
        .split_first()
        .ok_or_else(|| invalid("no test program"))?;
    Ok(IntentRequest {
        intent: context.intent.clone(),
        workspace: context.workspace.as_ref().map(PathBuf::from),
        test: TestCommand::new(program, args),
        runner,
        max_steps: usize::try_from(context.max_steps)
            .map_err(|_| invalid("a step budget that is not a count"))?,
        threshold: context
            .route
            .threshold
            .0
            .parse()
            .map_err(|_| invalid("a threshold that is not a number"))?,
    })
}

/// Continues `context`'s commission after the grant, through Commission's `resume_after_grant`:
/// the route is restored only for the intent, the catalog and the policy it was accepted for, the
/// workspace is validated again (`case::validate_workspace`), the grant `approve` wrote is read
/// through [`GrantAuthority`], and a granted `repository.merge` is handed off into
/// `state.handoffs`, moving no Git ref. The suspended request is performed once, the Suspension is
/// released, and the run goes on until it stops, as a run does. When it ends, however it ends,
/// without the commission's Suspension held, the context is removed
/// (`intake.routing.DropResumeContext`).
///
/// Only a `software-change@1` commission is resumed: no other protocol's run holds a Suspension.
///
/// # Errors
/// [`SliceError::Resume`] when Commission refused the resume (nothing was performed) or its loop
/// stopped without a run outcome; [`SliceError::Context`] when the route no longer matches the
/// intent, the catalog or the policy, or the protocol is not `software-change@1`;
/// [`SliceError::Case`] when the workspace no longer validates; [`SliceError::State`] when the
/// grant cannot be read, or the context cannot be removed; and every failure of a run.
#[allow(clippy::too_many_arguments)]
pub fn resume_with_state<S: FallibleCaseStore>(
    context: &ResumeContextData,
    request: &IntentRequest,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    out: &mut dyn Write,
    state: &ResumeState<'_>,
) -> Result<SliceRun, SliceError> {
    let ran = resume_prepared(
        context, request, catalog, governor, frontiers, agent, out, state,
    );
    let commission = CommissionId(CommissionUuid(context.commission_id.0.0.clone()));
    let settled = settle(state.suspensions, state.contexts, &commission);
    match ran {
        Ok(run) => settled.map(|()| run),
        Err(failure) => Err(failure),
    }
}

#[allow(clippy::too_many_arguments)]
fn resume_prepared<S: FallibleCaseStore>(
    context: &ResumeContextData,
    request: &IntentRequest,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    out: &mut dyn Write,
    state: &ResumeState<'_>,
) -> Result<SliceRun, SliceError> {
    let options = RunOptions {
        context_policy: context.route.context_policy,
        context_report: None,
    };
    let metrics = ContextMetrics::new(options.context_policy);
    let prepared = PreparedIntent::restore_route(&context.route, request, catalog, &options)?;
    governor
        .validate_catalog(catalog)
        .map_err(|error| SliceError::Context(error.to_string()))?;
    if prepared.protocol != LOCAL_PROTOCOL {
        return Err(SliceError::Context(format!(
            "resume continues {LOCAL_PROTOCOL} runs only, not {}",
            printable(&prepared.protocol)
        )));
    }
    let (Some(workspace), Some(kept)) = (request.workspace.as_deref(), &context.workspace) else {
        return Err(SliceError::State(
            "the resume context keeps no workspace".into(),
        ));
    };
    let runner = request
        .runner
        .as_ref()
        .ok_or_else(|| SliceError::Context("software-change@1 requires a test runner".into()))?;
    case::validate_workspace(workspace).map_err(SliceError::Case)?;
    writeln!(out, "confinement: {}", backend_name(runner.backend()))?;
    let grant = state
        .grants
        .get(&context.commission_id)
        .map_err(|error| SliceError::State(error.to_string()))?;
    let authority = GrantAuthority::new(grant, state.suspensions);
    let resuming = Resuming {
        commission: operator_commission(
            CommissionId(CommissionUuid(context.commission_id.0.0.clone())),
            AgentRevisionId(CommissionUuid(context.agent_revision_id.0.0.clone())),
            CaseId(context.case_id.0.clone()),
        ),
        suspensions: state.suspensions,
        authority: &authority,
        handoffs: state.handoffs,
        workspace: kept.clone(),
    };
    // The briefing starts empty: the model does not see the steps before the Suspension.
    let briefing = Briefing::with_options(
        request.intent.clone(),
        references(&request.intent),
        options.context_policy,
        metrics.clone(),
    );
    let agent = MeasuredModel::new(agent, metrics);
    drive(
        &request.intent,
        &prepared.protocol,
        CaseId(context.case_id.0.clone()),
        request.max_steps,
        governor,
        frontiers,
        &agent,
        out,
        briefing,
        Execution::Software {
            workspace,
            test: &request.test,
            runner: Arc::clone(runner),
        },
        Mode::Resuming(&resuming),
    )
}

/// The resume context of `commission`, run on `request` over `prepared`'s route.
fn resume_context(
    request: &IntentRequest,
    prepared: &PreparedIntent,
    host: &ResumeHost,
    commission: &CommissionData,
) -> Result<ResumeContextData, SliceError> {
    let text = HostText::of(request, host)?;
    Ok(ResumeContextData {
        commission_id: resume::CommissionId(IntakeUuid(commission.commission_id.0.0.clone())),
        case_id: resume::CaseId(commission.case_id.0.clone()),
        agent_revision_id: resume::AgentRevisionId(IntakeUuid(
            commission.agent_revision_id.0.0.clone(),
        )),
        intent: request.intent.clone(),
        route: prepared.accepted_route()?,
        workspace: text.workspace,
        test_command: text.test_command,
        max_steps: text.max_steps,
        confinement: host.confinement,
        cgroup_root: text.cgroup_root,
        model: host.model.clone(),
        catalog: text.catalog,
    })
}

/// `path` made absolute against the current directory, as text, so a resume from elsewhere finds
/// the same file.
fn absolute_text(what: &str, path: &Path) -> Result<String, SliceError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| SliceError::State(format!("{what} {}: {error}", path.display())))?;
    absolute
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| SliceError::State(format!("{what} {} is not UTF-8", path.display())))
}

#[allow(clippy::too_many_arguments)]
fn run_prepared<S: FallibleCaseStore>(
    request: &IntentRequest,
    prepared: &PreparedIntent,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    clock: &dyn Clock,
    out: &mut dyn Write,
    options: &RunOptions,
    metrics: &ContextMetrics,
    holding: Option<&Holding<'_>>,
) -> Result<SliceRun, SliceError> {
    let mode = holding.map_or(Mode::Once, Mode::Holding);
    if metrics.report().policy != options.context_policy {
        return Err(SliceError::Context(
            "measurement policy differs from context policy".into(),
        ));
    }
    if prepared.binding != binding(request, catalog, options) {
        return Err(SliceError::Context(
            "accepted route no longer matches the run".into(),
        ));
    }
    governor
        .validate_catalog(catalog)
        .map_err(|error| SliceError::Context(error.to_string()))?;
    let entry = catalog
        .get(&prepared.protocol)
        .ok_or_else(|| SliceError::Context("accepted protocol disappeared".into()))?;
    let briefing = Briefing::with_options(
        request.intent.clone(),
        references(&request.intent),
        options.context_policy,
        metrics.clone(),
    );
    let agent = MeasuredModel::new(agent, metrics.clone());
    if prepared.protocol == LOCAL_PROTOCOL {
        let workspace = request.workspace.as_deref().ok_or_else(|| SliceError::Case(case::CaseError::Workspace {problem:"software-change@1 requires --workspace with an existing Git worktree and commit".into()}))?;
        let runner = request.runner.as_ref().ok_or_else(|| {
            SliceError::Context("software-change@1 requires a test runner".into())
        })?;
        case::validate_workspace(workspace).map_err(SliceError::Case)?;
        writeln!(out, "confinement: {}", backend_name(runner.backend()))?;
        let case = case::open_with_catalog(
            governor,
            &prepared.protocol,
            &request.intent,
            workspace,
            catalog,
        )
        .map_err(SliceError::Case)?;
        drive(
            &request.intent,
            &prepared.protocol,
            case,
            request.max_steps,
            governor,
            frontiers,
            &agent,
            out,
            briefing,
            Execution::Software {
                workspace,
                test: &request.test,
                runner: Arc::clone(runner),
            },
            mode,
        )
    } else {
        entry.clock_compatible().map_err(SliceError::Context)?;
        let revision = case::intent_revision(&request.intent);
        let case = governor
            .open(
                &prepared.protocol,
                BTreeMap::from([("intent".into(), revision.clone())]),
            )
            .map_err(|e| SliceError::Case(case::CaseError::Open(e)))?;
        drive(
            &request.intent,
            &prepared.protocol,
            case,
            request.max_steps,
            governor,
            frontiers,
            &agent,
            out,
            briefing.for_query(),
            Execution::Clock {
                clock,
                intent_revision: &revision,
            },
            mode,
        )
    }
}

/// Route and run using an injectable clock, preserving metrics on every failure.
#[allow(clippy::too_many_arguments)]
pub fn run_intent_with_options<S: FallibleCaseStore>(
    request: &IntentRequest,
    catalog: &ProtocolCatalog,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    clock: &dyn Clock,
    out: &mut dyn Write,
    options: &RunOptions,
) -> Result<SliceRun, SliceError> {
    let metrics = ContextMetrics::new(options.context_policy);
    let result = (|| {
        governor
            .validate_catalog(catalog)
            .map_err(|error| SliceError::Context(error.to_string()))?;
        match prepare_intent(request, catalog, classifier, out, options, &metrics)? {
            Preparation::Stopped(run) => Ok(run),
            Preparation::Ready(prepared) => run_prepared_intent(
                request, &prepared, catalog, governor, frontiers, agent, clock, out, options,
                &metrics,
            ),
        }
    })();
    write_report(result, options, &metrics)
}

/// Finalize a run's measurements without hiding an earlier failure.
pub fn write_report(
    result: Result<SliceRun, SliceError>,
    options: &RunOptions,
    metrics: &ContextMetrics,
) -> Result<SliceRun, SliceError> {
    if let Some(path) = &options.context_report
        && let Err(error) = metrics.write(path)
    {
        return Err(SliceError::ContextReport {
            error,
            run_error: result.err().map(Box::new),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_metrics::ContextPolicy;

    #[test]
    fn delegation_handoff_is_bound_to_intent_catalog_threshold_and_policy() {
        let catalog = ProtocolCatalog::bundled().unwrap();
        let mut request = IntentRequest {
            intent: "current time".into(),
            workspace: None,
            test: TestCommand::new("", [] as [&str; 0]),
            runner: None,
            max_steps: 4,
            threshold: 0.5,
        };
        let mut options = RunOptions::default();
        let prepared = PreparedIntent {
            protocol: "system-query@1".into(),
            binding: binding(&request, &catalog, &options),
        };
        let value = prepared.handoff();
        assert_eq!(
            PreparedIntent::restore(&value, &request, &catalog, &options)
                .unwrap()
                .protocol(),
            "system-query@1"
        );
        options.context_policy = ContextPolicy::Bounded;
        assert!(PreparedIntent::restore(&value, &request, &catalog, &options).is_err());
        options.context_policy = ContextPolicy::Legacy;
        request.threshold = 0.9;
        assert!(PreparedIntent::restore(&value, &request, &catalog, &options).is_err());
        request.threshold = 0.5;
        request.intent = "write a file".into();
        assert!(PreparedIntent::restore(&value, &request, &catalog, &options).is_err());
        request.intent = "current time".into();
        assert!(
            PreparedIntent::restore(
                &value,
                &request,
                &ProtocolCatalog::engineering().unwrap(),
                &options
            )
            .is_err()
        );
    }

    /// A resume context's route restores only for the intent, catalog, threshold and policy it
    /// was accepted for, as the delegation handoff does.
    #[test]
    fn a_kept_route_restores_as_the_handoff_it_types() {
        let catalog = ProtocolCatalog::bundled().unwrap();
        let mut request = IntentRequest {
            intent: "current time".into(),
            workspace: None,
            test: TestCommand::new("", [] as [&str; 0]),
            runner: None,
            max_steps: 4,
            threshold: 0.35,
        };
        let options = RunOptions {
            context_policy: ContextPolicy::Bounded,
            context_report: None,
        };
        let prepared = PreparedIntent {
            protocol: "system-query@1".into(),
            binding: binding(&request, &catalog, &options),
        };
        let route = prepared.accepted_route().unwrap();
        assert_eq!(route.protocol, "system-query@1");
        assert_eq!(route.context_policy, ContextPolicy::Bounded);
        assert_eq!(route.threshold.0, "0.35");
        assert_eq!(route.catalog.len(), catalog.iter().count());
        let restored = PreparedIntent::restore_route(&route, &request, &catalog, &options).unwrap();
        assert_eq!(restored.handoff(), prepared.handoff());
        assert!(
            PreparedIntent::restore_route(&route, &request, &catalog, &RunOptions::default())
                .is_err()
        );
        request.intent = "write a file".into();
        assert!(PreparedIntent::restore_route(&route, &request, &catalog, &options).is_err());
    }
}
