//! Acceptance for `story:http-read-transport-failure-goes-back`: an `http` read whose transport
//! fails (the reader answers `HttpReadError::Unavailable`) is asked once more; a read that fails
//! twice is a refused step naming the source, the path and the error, which the model sees on its
//! next request; and a read the reader refuses is never asked again. Before the story one
//! transport failure ended the run, and the turn reported it as `PluginError::Unavailable`.

mod support;

use std::collections::VecDeque;
use std::sync::Mutex;

use b10x_loom_plugin::datasource::SourceName;
use b10x_loom_plugin::effects::{HttpAnswer, HttpReadError, HttpReader, HttpSources};
use b10x_loom_plugin::project::{PROPOSE, READ};
use b10x_loom_plugin::turn::{TurnOptions, turn_with};
use b10x_loom_plugin::{DataSourceEffects, Intent, Projection, RecordOutcome};
use loom::agent::HttpBinding;
use loom_sdk::commission::model::responsibility::EffectOutcome;
use serde_json::json;
use support::{
    Fixture, Proposes, Scripted, classification, governed, item, request_text, run_once,
};

/// The error a reader answers for a connection that could not be made.
const DOWN: &str = "the request failed: connection refused";

/// A reader answering each read with the next scripted answer, keeping how often it was asked.
struct Sequence {
    answers: Mutex<VecDeque<Result<HttpAnswer, HttpReadError>>>,
    asked: Mutex<usize>,
}

impl Sequence {
    fn new(answers: Vec<Result<HttpAnswer, HttpReadError>>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
            asked: Mutex::new(0),
        }
    }

    fn asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}

impl HttpReader for Sequence {
    fn read(
        &self,
        _binding: &HttpBinding,
        _path: &str,
        _query: &[(String, String)],
    ) -> Result<HttpAnswer, HttpReadError> {
        *self.asked.lock().unwrap() += 1;
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(HttpReadError::Unavailable(DOWN.to_owned())))
    }
}

fn logs() -> HttpBinding {
    HttpBinding {
        name: SourceName("logs".to_owned()),
        base_url: "http://127.0.0.1:9/".to_owned(),
        allow_paths: vec!["/api/v1/query".to_owned()],
        method: "GET".to_owned(),
        bearer_env: None,
        max_bytes: 100,
    }
}

fn down() -> Result<HttpAnswer, HttpReadError> {
    Err(HttpReadError::Unavailable(DOWN.to_owned()))
}

fn whole(body: &str) -> Result<HttpAnswer, HttpReadError> {
    Ok(HttpAnswer {
        status: 200,
        body: body.to_owned(),
        cut: false,
    })
}

fn reads_logs() -> Projection {
    Projection {
        actions: vec![READ.to_owned(), PROPOSE.to_owned()],
        sources: vec![SourceName("logs".to_owned())],
    }
}

fn read_logs() -> serde_json::Value {
    json!({"source": "logs", "kind": "get", "input": {"path": "/api/v1/query"}})
}

/// A transport failure, then an answer: the read is performed, and the reader was asked twice.
#[test]
fn a_read_whose_transport_fails_once_is_retried_and_performed() {
    let fixture = Fixture::new("http_read_transport", "retried");
    let config = fixture.config(Vec::new());
    let host = fixture.host(&config);
    let item = item("item-1");
    let projection = reads_logs();
    let (governor, case) = governed();
    let reader = Sequence::new(vec![down(), whole("up 1")]);
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    )
    .with_http(HttpSources {
        bindings: vec![logs()],
        reader: &reader,
    });

    let end = run_once(
        &governor,
        &case,
        &Proposes::new(vec![(READ, read_logs())]),
        &effects,
    );

    assert!(
        matches!(end.effects.as_slice(), [EffectOutcome::Performed(_)]),
        "story:http-read-transport-failure-goes-back: the retry answered: {:?}",
        end.effects
    );
    assert_eq!(reader.asked(), 2, "one read, asked twice");
    assert_eq!(effects.unavailable(), None, "nothing stopped the run");
    assert_eq!(effects.reads().len(), 1);
}

/// A read that fails twice goes back to the model as a refusal naming the source, the path and
/// the error; the model reads again on its next step, the endpoint answers, and the turn
/// proposes.
#[test]
fn a_read_that_fails_twice_goes_back_to_the_model() {
    let fixture = Fixture::new("http_read_transport", "goes-back");
    let config = fixture.config(Vec::new());
    let host = fixture.host(&config);
    let item = item("item-1");
    let classified = classification(Intent::Ask, &["logs"], "0.9");
    let reader = Sequence::new(vec![down(), down(), whole("up 1")]);
    let (model, seen) = Scripted::new(vec![
        ("source_read", read_logs()),
        ("source_read", read_logs()),
        ("reply_propose", json!({"text": "The service is up."})),
    ]);

    let turned = turn_with(
        &host,
        model.into_turn_model(),
        &item,
        &classified,
        &reads_logs(),
        TurnOptions {
            http: Some(HttpSources {
                bindings: vec![logs()],
                reader: &reader,
            }),
            ..TurnOptions::defaults()
        },
    )
    .expect("story:http-read-transport-failure-goes-back: a failed read does not end the turn");

    assert_eq!(
        turned.result.outcome,
        RecordOutcome::Proposed,
        "{:?}",
        turned.result.detail
    );
    assert_eq!(
        reader.asked(),
        3,
        "the first read was asked twice, then given up; the second answered"
    );
    assert_eq!(turned.result.reads.len(), 1, "a read given up is no read");
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests.len(), 6, "three steps, two requests each");
    assert!(
        request_text(&requests[2]).contains(&format!(
            "refused: the read of `logs` (path `/api/v1/query`) found the endpoint unavailable, \
             also when retried: {DOWN}"
        )),
        "the selection after the failed read tells the model why"
    );
}

/// A read the reader refuses (a failing status) is a refused step, and is not asked again.
#[test]
fn a_refused_read_is_not_retried() {
    let fixture = Fixture::new("http_read_transport", "refused");
    let config = fixture.config(Vec::new());
    let host = fixture.host(&config);
    let item = item("item-1");
    let projection = reads_logs();
    let (governor, case) = governed();
    let reader = Sequence::new(vec![Err(HttpReadError::Refused(
        "FailedStatus: the endpoint answered 503".to_owned(),
    ))]);
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &projection,
    )
    .with_http(HttpSources {
        bindings: vec![logs()],
        reader: &reader,
    });

    let end = run_once(
        &governor,
        &case,
        &Proposes::new(vec![(READ, read_logs())]),
        &effects,
    );

    assert!(
        matches!(end.effects.as_slice(), [EffectOutcome::Refused(r)] if !r.ends_run),
        "{:?}",
        end.effects
    );
    assert_eq!(reader.asked(), 1, "a refusal is not retried");
}
