//! Caller-owned retries around single-attempt model ports. Never retry an effect.
use b10x_llm_tool_call::ModelError;
use b10x_loom_intake_router::{ProtocolPick, RouterError, classify, classify_with_catalog};
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StreamEvent, StreamSink,
    TurnOutcome, TurnRequest,
};
use llm_routing::RetryPolicy;
use loom_protocols::ProtocolCatalog;
use std::time::Duration;

const POLICY: RetryPolicy = RetryPolicy {
    max_attempts: 3,
    ..RetryPolicy::DEFAULT
};

pub(crate) async fn classify_with_retries(
    intent: &str,
    model: &dyn Model,
    threshold: f64,
    catalog: Option<&ProtocolCatalog>,
) -> Result<ProtocolPick, RouterError> {
    let model = ObservedModel(model);
    let cancel = Cancel::new();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut answer = if let Some(catalog) = catalog {
            classify_with_catalog(intent, &model, threshold, catalog).await
        } else {
            classify(intent, &model, threshold).await
        };
        if let Err(RouterError::Model(ModelError::Model(error) | ModelError::Transport(error))) =
            &mut answer
            && wait_to_retry(error, attempt, &cancel).await
        {
            continue;
        }
        return answer;
    }
}

pub(crate) async fn turn_with_retries(
    model: &dyn Model,
    request: &TurnRequest,
    sink: &mut dyn StreamSink,
    cancel: &Cancel,
) -> Result<TurnOutcome, Error> {
    let model = ObservedModel(model);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut answer = model.turn(request, sink, cancel).await;
        if let Err(error) = &mut answer
            && wait_to_retry(error, attempt, cancel).await
        {
            continue;
        }
        return answer;
    }
}

async fn wait_to_retry(error: &mut Error, attempt: u32, cancel: &Cancel) -> bool {
    if !error.may_retry() {
        return false;
    }
    if attempt >= POLICY.max_attempts {
        error.retriable = false;
        error.message = format!("{} (after {attempt} attempts)", error.message);
        return false;
    }
    let delay = POLICY.delay(attempt, error.retry_after_ms.map(Duration::from_millis));
    tokio::select! {
        biased;
        () = cancel.cancelled() => {
            // Preserve the evidence of the last completed attempt.
            error.code = llm_core::ErrorCode::Cancelled;
            error.message = "the caller cancelled during model retry backoff".into();
            error.retriable = false;
            false
        }
        () = tokio::time::sleep(delay) => true,
    }
}

/// Still exactly one attempt. Keep sink visibility beside the provider's retry classification.
struct ObservedModel<'a>(&'a dyn Model);
impl Model for ObservedModel<'_> {
    fn provenance(&self) -> &Provenance {
        self.0.provenance()
    }
    fn capabilities(&self) -> &Capabilities {
        self.0.capabilities()
    }
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(Error::cancelled());
            }
            let mut observed = ObservedSink {
                inner: sink,
                offered: false,
            };
            self.0
                .turn(request, &mut observed, cancel)
                .await
                .map_err(|mut error| {
                    if let Err(invalid) = error.validate_for(self.provenance()) {
                        return invalid;
                    }
                    if observed.offered {
                        error.retriable = false;
                    }
                    error
                })
        })
    }
}
struct ObservedSink<'a> {
    inner: &'a mut dyn StreamSink,
    offered: bool,
}
impl StreamSink for ObservedSink<'_> {
    fn emit(&mut self, event: StreamEvent) -> BoxFuture<'_, Result<(), Error>> {
        self.offered = true;
        self.inner.emit(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_core::{Dispatch, ErrorCode, Id, Item, Protocol, TurnObservation, VecSink};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Failing {
        provenance: Provenance,
        capabilities: Capabilities,
        calls: AtomicUsize,
        event: Option<StreamEvent>,
        code: ErrorCode,
        retry_after_ms: Option<u64>,
    }
    impl Failing {
        fn new() -> Self {
            let id = |value| Id::new(value).unwrap();
            Self {
                provenance: Provenance {
                    protocol: Protocol::Responses,
                    provider: id("fixture"),
                    account: id("fixture"),
                    endpoint: id("fixture"),
                    model: id("fixture"),
                    binding_revision: id("fixture"),
                },
                capabilities: Capabilities::text(4096, 1024),
                calls: AtomicUsize::new(0),
                event: None,
                code: ErrorCode::Unavailable,
                retry_after_ms: None,
            }
        }
    }
    impl Model for Failing {
        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }
        fn turn<'a>(
            &'a self,
            _: &'a TurnRequest,
            sink: &'a mut dyn StreamSink,
            _: &'a Cancel,
        ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if let Some(event) = &self.event {
                    sink.emit(event.clone()).await?;
                }
                let mut error = Error::new(self.code, "fixture overload")
                    .with_dispatch(Dispatch::Accepted)
                    .with_retriable(true)
                    .with_observation(TurnObservation::new(self.provenance.clone()));
                error.retry_after_ms = self.retry_after_ms;
                Err(error)
            })
        }
    }
    fn request() -> TurnRequest {
        TurnRequest::new("fixture", vec![Item::user("fixture")])
    }

    #[tokio::test(start_paused = true)]
    async fn exhaustion_waits_one_then_two_seconds_and_preserves_final_evidence() {
        let model = Failing::new();
        let start = tokio::time::Instant::now();
        let error = turn_with_retries(
            &model,
            &request(),
            &mut VecSink::new(10, 4096),
            &Cancel::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(model.calls.load(Ordering::SeqCst), 3);
        assert_eq!(start.elapsed(), Duration::from_secs(3));
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert_eq!(error.dispatch, Dispatch::Accepted);
        assert_eq!(error.observation.unwrap().binding, model.provenance);
        assert!(!error.retriable);
        assert!(error.message.contains("after 3 attempts"));
    }

    #[tokio::test(start_paused = true)]
    async fn provider_delay_is_capped_without_shortening_local_backoff() {
        for (server, elapsed) in [(1, 3), (u64::MAX, 60)] {
            let mut model = Failing::new();
            model.retry_after_ms = Some(server);
            let start = tokio::time::Instant::now();
            turn_with_retries(
                &model,
                &request(),
                &mut VecSink::new(10, 4096),
                &Cancel::new(),
            )
            .await
            .unwrap_err();
            assert_eq!(start.elapsed(), Duration::from_secs(elapsed));
            assert_eq!(model.calls.load(Ordering::SeqCst), 3);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_interrupts_backoff_without_another_attempt() {
        let model = Failing::new();
        let cancel = Cancel::new();
        let start = tokio::time::Instant::now();
        let request = request();
        let mut sink = VecSink::new(10, 4096);
        let (answer, ()) = tokio::join!(
            turn_with_retries(&model, &request, &mut sink, &cancel),
            async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                cancel.cancel();
            }
        );
        let error = answer.unwrap_err();
        assert_eq!(error.code, ErrorCode::Cancelled);
        assert!(error.observation.is_some());
        assert!(!error.retriable);
        assert_eq!(start.elapsed(), Duration::from_millis(100));
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
        turn_with_retries(&model, &request, &mut sink, &cancel)
            .await
            .unwrap_err();
        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn every_offered_event_prevents_retry_even_when_the_sink_rejects_it() {
        for event in [
            StreamEvent::TextDelta {
                text: "partial".into(),
            },
            StreamEvent::Warning {
                code: "fixture".into(),
                message: "warning".into(),
            },
            StreamEvent::ReasoningDelta {
                text: "partial".into(),
            },
        ] {
            for limit in [0, 10] {
                let mut model = Failing::new();
                model.event = Some(event.clone());
                let mut sink = VecSink::new(limit, 4096);
                let error = turn_with_retries(&model, &request(), &mut sink, &Cancel::new())
                    .await
                    .unwrap_err();
                assert_eq!(model.calls.load(Ordering::SeqCst), 1);
                assert!(!error.may_retry());
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn denied_classes_are_not_retried_even_with_a_retriable_flag() {
        for code in [
            ErrorCode::Refused,
            ErrorCode::Unauthorized,
            ErrorCode::InvalidRequest,
            ErrorCode::Unsupported,
            ErrorCode::TooLarge,
            ErrorCode::Cancelled,
            ErrorCode::Deadline,
        ] {
            let mut model = Failing::new();
            model.code = code;
            let error = turn_with_retries(
                &model,
                &request(),
                &mut VecSink::new(10, 4096),
                &Cancel::new(),
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(model.calls.load(Ordering::SeqCst), 1);
        }
    }
}
