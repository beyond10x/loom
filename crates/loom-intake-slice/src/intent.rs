//! Catalog-based intent orchestration. Resource initialization follows the accepted route.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_references::references;
use b10x_loom_intake_router::{RouterError, classify_with_catalog};
use llm_core::Model;
use loom_governor::{CanonGovernor, CaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

use crate::case;
use crate::clock::Clock;
use crate::confinement::TestRunner;
use crate::context_metrics::{ContextMetrics, MeasuredModel};
use crate::executor::{TestCommand, backend_name};
use crate::run::{
    Execution, LOCAL_PROTOCOL, RunOptions, SliceError, SliceRun, StopReason, drive, printable,
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
    let result = runtime.block_on(classify_with_catalog(
        &request.intent,
        &model,
        request.threshold,
        catalog,
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
    })
}

/// Execute an accepted route with host-provided resources and the same immutable catalog.
#[allow(clippy::too_many_arguments)]
pub fn run_prepared_intent<S: CaseStore>(
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
        )
    }
}

/// Route and run using an injectable clock, preserving metrics on every failure.
#[allow(clippy::too_many_arguments)]
pub fn run_intent_with_options<S: CaseStore>(
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
}
