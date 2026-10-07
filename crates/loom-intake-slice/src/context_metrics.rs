//! Payload-free measurements at the model port, including failed attempts and classification.

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

pub use intake_model::context::{ContextPolicy, ContextReport, ContextRequestMetric, RequestPhase};
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StreamSink, TurnObservation,
    TurnOutcome, TurnRequest,
};
use serde_json::{Value, json};

/// Ceiling on a complete serialized bounded request, including schemas and JSON escaping.
pub const REQUEST_CEILING: usize = 64 * 1024;

/// Shared run measurements. This holds counters, never prompts, arguments, or result contents.
#[derive(Debug, Clone)]
pub struct ContextMetrics {
    report: Arc<Mutex<ContextReport>>,
    started: Instant,
    elapsed_before_ms: i64,
}

impl ContextMetrics {
    pub fn new(policy: ContextPolicy) -> Self {
        Self {
            report: Arc::new(Mutex::new(ContextReport {
                policy,
                requests: Vec::new(),
                checkpoints: 0,
                retrievals: 0,
                model_calls: 0,
                elapsed_ms: 0,
            })),
            started: Instant::now(),
            elapsed_before_ms: 0,
        }
    }

    pub fn checkpoint(&self) {
        self.lock().checkpoints += 1;
    }

    pub fn retrieval(&self) {
        self.lock().retrievals += 1;
    }

    /// A snapshot remains available after a run fails.
    pub fn report(&self) -> ContextReport {
        let mut report = self.lock().clone();
        report.elapsed_ms = self
            .elapsed_before_ms
            .saturating_add(milliseconds(self.started));
        report
    }

    /// Writes only measurements. Unknown provider counters are represented by JSON null.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &self.snapshot()).map_err(io::Error::other)
    }

    /// Preserve the delegated child's counters while closing elapsed time with the original
    /// process's monotonic clock. This includes process startup and child initialization.
    pub fn finish_delegated_report(&self, path: &Path) -> io::Result<()> {
        let mut value: Value =
            serde_json::from_slice(&std::fs::read(path)?).map_err(io::Error::other)?;
        Self::restore(&value, self.lock().policy).map_err(io::Error::other)?;
        value["elapsed_ms"] = json!(self.report().elapsed_ms);
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &value).map_err(io::Error::other)
    }

    /// Payload-free state carried across the CLI's private delegation handoff.
    pub fn snapshot(&self) -> Value {
        let report = self.report();
        let requests: Vec<Value> = report
            .requests
            .iter()
            .map(|request| {
                json!({
                    "phase": phase_name(request.phase),
                    "request_bytes": request.request_bytes,
                    "elapsed_ms": request.elapsed_ms,
                    "final_usage": request.final_usage,
                    "input_tokens": request.input_tokens,
                    "cache_read_tokens": request.cache_read_tokens,
                    "cache_write_tokens": request.cache_write_tokens,
                    "output_tokens": request.output_tokens,
                })
            })
            .collect();
        json!({
            "policy": match report.policy { ContextPolicy::Legacy => "legacy", ContextPolicy::Bounded => "bounded" },
            "requests": requests,
            "checkpoints": report.checkpoints,
            "retrievals": report.retrievals,
            "model_calls": report.model_calls,
            "elapsed_ms": report.elapsed_ms,
        })
    }

    /// Restores measurements after a host-controlled process restart.
    pub fn restore(value: &Value, policy: ContextPolicy) -> Result<Self, String> {
        let invalid = || "invalid context measurement handoff".to_owned();
        let number = |v: &Value| v.as_i64().filter(|n| *n >= 0).ok_or_else(invalid);
        let expected = match policy {
            ContextPolicy::Legacy => "legacy",
            ContextPolicy::Bounded => "bounded",
        };
        if value["policy"].as_str() != Some(expected) {
            return Err(invalid());
        }
        let requests = value["requests"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|r| {
                let optional = |key: &str| -> Result<Option<i64>, String> {
                    if r[key].is_null() {
                        Ok(None)
                    } else {
                        number(&r[key]).map(Some)
                    }
                };
                Ok(ContextRequestMetric {
                    phase: match r["phase"].as_str() {
                        Some("classification") => RequestPhase::Classification,
                        Some("selection") => RequestPhase::Selection,
                        Some("arguments") => RequestPhase::Arguments,
                        _ => return Err(invalid()),
                    },
                    request_bytes: number(&r["request_bytes"])?,
                    elapsed_ms: number(&r["elapsed_ms"])?,
                    final_usage: r["final_usage"].as_bool().ok_or_else(invalid)?,
                    input_tokens: optional("input_tokens")?,
                    cache_read_tokens: optional("cache_read_tokens")?,
                    cache_write_tokens: optional("cache_write_tokens")?,
                    output_tokens: optional("output_tokens")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let report = ContextReport {
            policy,
            requests,
            checkpoints: number(&value["checkpoints"])?,
            retrievals: number(&value["retrievals"])?,
            model_calls: number(&value["model_calls"])?,
            elapsed_ms: number(&value["elapsed_ms"])?,
        };
        if report.model_calls != report.requests.len() as i64 {
            return Err(invalid());
        }
        Ok(Self {
            elapsed_before_ms: report.elapsed_ms,
            report: Arc::new(Mutex::new(report)),
            started: Instant::now(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, ContextReport> {
        self.report.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn milliseconds(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn phase_name(phase: RequestPhase) -> &'static str {
    match phase {
        RequestPhase::Classification => "classification",
        RequestPhase::Selection => "selection",
        RequestPhase::Arguments => "arguments",
    }
}

/// Decorates the existing model port without changing its binding, streaming, or errors.
pub(crate) struct MeasuredModel<'m> {
    inner: &'m dyn Model,
    metrics: ContextMetrics,
}

impl<'m> MeasuredModel<'m> {
    pub(crate) fn new(inner: &'m dyn Model, metrics: ContextMetrics) -> Self {
        Self { inner, metrics }
    }
}

impl Model for MeasuredModel<'_> {
    fn provenance(&self) -> &Provenance {
        self.inner.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.inner.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            let bytes = serde_json::to_vec(request)
                .map_err(|_| Error::invalid("context request cannot be serialized"))?
                .len();
            let phase = request
                .tools
                .iter()
                .find_map(|tool| match tool.name.as_str() {
                    "pick_protocol" => Some(RequestPhase::Classification),
                    "select_action" => Some(RequestPhase::Selection),
                    "action_arguments" => Some(RequestPhase::Arguments),
                    _ => None,
                })
                .ok_or_else(|| Error::invalid("context request has no recognized phase"))?;
            if self.metrics.lock().policy == ContextPolicy::Bounded && bytes > REQUEST_CEILING {
                return Err(Error::too_large(format!(
                    "bounded context request is {bytes} bytes; ceiling is {REQUEST_CEILING} bytes"
                )));
            }
            let started = Instant::now();
            // Count attempts at this port, including failures; local capacity refusal is no call.
            self.metrics.lock().model_calls += 1;
            let answer = self.inner.turn(request, sink, cancel).await;
            let observation = match &answer {
                Ok(outcome) => Some(&outcome.observation),
                Err(error) => error.observation.as_deref(),
            };
            self.metrics
                .lock()
                .requests
                .push(metric(phase, bytes, started, observation));
            answer
        })
    }
}

fn metric(
    phase: RequestPhase,
    bytes: usize,
    started: Instant,
    observation: Option<&TurnObservation>,
) -> ContextRequestMetric {
    let usage = observation.and_then(|observation| observation.usage.as_ref());
    // The contract uses signed integers; an unrepresentable provider count remains unknown.
    let count = |value: Option<u64>| value.and_then(|value| i64::try_from(value).ok());
    ContextRequestMetric {
        phase,
        request_bytes: i64::try_from(bytes).unwrap_or(i64::MAX),
        elapsed_ms: milliseconds(started),
        final_usage: observation.is_some_and(|observation| observation.final_usage),
        input_tokens: count(usage.and_then(|usage| usage.input_tokens)),
        cache_read_tokens: count(usage.and_then(|usage| usage.cached_input_tokens)),
        cache_write_tokens: count(usage.and_then(|usage| usage.cache_creation_input_tokens)),
        output_tokens: count(usage.and_then(|usage| usage.output_tokens)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_core::{
        Dispatch, ErrorCode, Id, Item, Protocol, ToolChoice, ToolName, ToolSpec, Usage, VecSink,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Recorded {
        provenance: Provenance,
        capabilities: Capabilities,
        calls: AtomicUsize,
        fail: bool,
    }

    impl Recorded {
        fn new(fail: bool) -> Self {
            let id = |value| Id::new(value).unwrap();
            Self {
                provenance: Provenance {
                    protocol: Protocol::Responses,
                    provider: id("recorded"),
                    account: id("fixture"),
                    endpoint: id("local"),
                    model: id("recorded-model"),
                    binding_revision: id("1"),
                },
                capabilities: Capabilities::text(128_000, 8192),
                calls: AtomicUsize::new(0),
                fail,
            }
        }
    }

    impl Model for Recorded {
        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }
        fn turn<'a>(
            &'a self,
            _: &'a TurnRequest,
            _: &'a mut dyn StreamSink,
            _: &'a Cancel,
        ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let observation = TurnObservation {
                    usage: Some(Usage {
                        input_tokens: Some(37),
                        cached_input_tokens: Some(11),
                        output_tokens: Some(5),
                        ..Usage::default()
                    }),
                    final_usage: !self.fail,
                    ..TurnObservation::new(self.provenance.clone())
                };
                if self.fail {
                    Err(Error::new(ErrorCode::Transport, "stream failed")
                        .with_dispatch(Dispatch::Accepted)
                        .with_observation(observation))
                } else {
                    Ok(TurnOutcome {
                        stop_reason: llm_core::StopReason::EndTurn,
                        items: vec![],
                        observation,
                    })
                }
            })
        }
    }

    fn request(tool: &str, text: &str) -> TurnRequest {
        let mut request = TurnRequest::new("recorded-model", vec![Item::user(text)]);
        let name = ToolName::new(tool).unwrap();
        request.tools.push(ToolSpec {
            name: name.clone(),
            description: "schema is included".into(),
            input_schema: json!({"type":"object","properties":{"value":{"type":"string"}}}),
        });
        request.tool_choice = ToolChoice::Named(name);
        request
    }

    fn turn(model: &dyn Model, request: &TurnRequest) -> Result<TurnOutcome, Error> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(model.turn(request, &mut VecSink::new(10, 1024), &Cancel::new()))
    }

    #[test]
    fn counts_full_serialized_requests_by_phase_and_preserves_unknown_counters() {
        let inner = Recorded::new(false);
        let metrics = ContextMetrics::new(ContextPolicy::Bounded);
        let model = MeasuredModel::new(&inner, metrics.clone());
        for tool in ["pick_protocol", "select_action", "action_arguments"] {
            let request = request(tool, "private source: 🦀\n\t\"\\");
            turn(&model, &request).unwrap();
            assert_eq!(
                metrics.report().requests.last().unwrap().request_bytes,
                serde_json::to_vec(&request).unwrap().len() as i64
            );
        }
        metrics.checkpoint();
        metrics.retrieval();
        let report = metrics.report();
        assert_eq!(report.model_calls, 3);
        assert_eq!(report.checkpoints, 1);
        assert_eq!(report.retrievals, 1);
        assert_eq!(
            report.requests.iter().map(|m| m.phase).collect::<Vec<_>>(),
            [
                RequestPhase::Classification,
                RequestPhase::Selection,
                RequestPhase::Arguments
            ]
        );
        assert_eq!(report.requests[0].input_tokens, Some(37));
        assert_eq!(report.requests[0].cache_read_tokens, Some(11));
        assert_eq!(report.requests[0].cache_write_tokens, None);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("metrics.json");
        metrics.write(&path).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("private source"));
        let value: Value = serde_json::from_str(&text).unwrap();
        assert!(value["requests"][0]["cache_write_tokens"].is_null());
    }

    #[test]
    fn delegation_preserves_classification_usage_and_unknown_counters() {
        let inner = Recorded::new(false);
        let metrics = ContextMetrics::new(ContextPolicy::Bounded);
        let model = MeasuredModel::new(&inner, metrics.clone());
        turn(&model, &request("pick_protocol", "private intent")).unwrap();
        let snapshot = metrics.snapshot();
        let restored = ContextMetrics::restore(&snapshot, ContextPolicy::Bounded).unwrap();
        assert_eq!(restored.report().requests, metrics.report().requests);
        assert_eq!(restored.report().model_calls, 1);
        assert!(restored.report().elapsed_ms >= snapshot["elapsed_ms"].as_i64().unwrap());
        assert!(!snapshot.to_string().contains("private intent"));
        assert!(ContextMetrics::restore(&snapshot, ContextPolicy::Legacy).is_err());
        let mut malformed = snapshot.clone();
        malformed["model_calls"] = json!(19);
        assert!(ContextMetrics::restore(&malformed, ContextPolicy::Bounded).is_err());
        malformed = snapshot;
        malformed["requests"][0]["input_tokens"] = json!(-1);
        assert!(ContextMetrics::restore(&malformed, ContextPolicy::Bounded).is_err());
    }

    #[test]
    fn delegated_report_includes_handoff_delay_without_losing_child_usage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let original = ContextMetrics::new(ContextPolicy::Bounded);
        let snapshot = original.snapshot();
        // A delay between snapshot and restore represents systemd and child startup.
        std::thread::sleep(std::time::Duration::from_millis(25));
        let child = ContextMetrics::restore(&snapshot, ContextPolicy::Bounded).unwrap();
        let inner = Recorded::new(false);
        turn(
            &MeasuredModel::new(&inner, child.clone()),
            &request("select_action", "private prompt"),
        )
        .unwrap();
        child.write(&path).unwrap();
        let before: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        original.finish_delegated_report(&path).unwrap();
        let after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(after["elapsed_ms"].as_i64().unwrap() >= 25);
        assert!(
            after["elapsed_ms"].as_i64().unwrap() >= before["elapsed_ms"].as_i64().unwrap() + 20
        );
        assert_eq!(after["requests"], before["requests"]);
        assert_eq!(after["model_calls"], 1);
        assert_eq!(after["requests"][0]["input_tokens"], 37);
        assert!(after["requests"][0]["cache_write_tokens"].is_null());
        assert!(!after.to_string().contains("private prompt"));
    }

    #[test]
    fn captures_partial_usage_on_failed_attempt() {
        let inner = Recorded::new(true);
        let metrics = ContextMetrics::new(ContextPolicy::Bounded);
        let model = MeasuredModel::new(&inner, metrics.clone());
        assert!(turn(&model, &request("pick_protocol", "intent")).is_err());
        let report = metrics.report();
        assert_eq!(report.model_calls, 1);
        assert_eq!(report.requests[0].output_tokens, Some(5));
        assert!(!report.requests[0].final_usage);
    }

    #[test]
    fn bounded_ceiling_rejects_escaped_payload_before_model_but_legacy_is_unchanged() {
        let inner = Recorded::new(false);
        let request = request("pick_protocol", &"\u{0001}".repeat(12_000));
        assert!(serde_json::to_vec(&request).unwrap().len() > REQUEST_CEILING);
        for policy in [ContextPolicy::Bounded, ContextPolicy::Legacy] {
            let metrics = ContextMetrics::new(policy);
            let model = MeasuredModel::new(&inner, metrics.clone());
            let result = turn(&model, &request);
            if policy == ContextPolicy::Bounded {
                assert_eq!(result.unwrap_err().code, ErrorCode::TooLarge);
                assert_eq!(metrics.report().model_calls, 0);
                assert_eq!(inner.calls.load(Ordering::SeqCst), 0);
            } else {
                result.unwrap();
                assert_eq!(metrics.report().model_calls, 1);
            }
        }
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use crate::executor::{TestCommand, UnconfinedRunner};
    use crate::run::{RunOptions, SliceError, SliceRequest, run_with_options};
    use loom_governor::{CanonGovernor, MemoryCaseStore};

    /// Classification rejects this input before invoking a model or opening the workspace.
    struct NeverCalled;
    impl Model for NeverCalled {
        fn provenance(&self) -> &Provenance {
            panic!("no model access")
        }
        fn capabilities(&self) -> &Capabilities {
            panic!("no model access")
        }
        fn turn<'a>(
            &'a self,
            _: &'a TurnRequest,
            _: &'a mut dyn StreamSink,
            _: &'a Cancel,
        ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
            panic!("no model access")
        }
    }

    #[test]
    fn failed_runs_write_reports_and_report_failures_preserve_the_original_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let request = SliceRequest {
            runner: Arc::new(UnconfinedRunner),
            intent: "private-intent".into(),
            workspace: directory.path().join("does-not-exist"),
            test: TestCommand::new("true", [] as [&str; 0]),
            max_steps: 1,
            threshold: 2.0,
        };
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let mut out = Vec::new();
        let result = run_with_options(
            &request,
            &governor,
            &governor,
            &NeverCalled,
            &NeverCalled,
            &mut out,
            &RunOptions {
                context_policy: ContextPolicy::Bounded,
                context_report: Some(path.clone()),
            },
        );
        assert!(matches!(result, Err(SliceError::Router(_))));
        let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["model_calls"], 0);
        assert_eq!(value["policy"], "bounded");
        assert!(!value.to_string().contains("private-intent"));
        let result = run_with_options(
            &request,
            &governor,
            &governor,
            &NeverCalled,
            &NeverCalled,
            &mut out,
            &RunOptions {
                context_policy: ContextPolicy::Legacy,
                context_report: Some(directory.path().to_owned()),
            },
        );
        assert!(matches!(
            result,
            Err(SliceError::ContextReport {
                run_error: Some(_),
                ..
            })
        ));
    }
}
