//! Adversary cases for `CanonGovernor::decision_document` (story
//! `governor-exposes-canon-decision`, wave 2026-10-20-w1).
//!
//! The story's first acceptance line: `decision_document` returns the document whose rendering
//! gives the id of the frontier `frontier` issued at the same revision. The unit's own case holds
//! it with a trusted time that never moves. The governor `b10x-loom run` and `resume` build
//! (`loom_intake_slice::run::slice_governor`) evaluates at `HostTime`, the system clock to the
//! second, read afresh for every evaluation; and every `canon-decision/1` document Canon writes
//! with a time names it (`computed_from.at`). `decision_document` evaluates the case again, so a
//! host that reads the frontier and then its document across a second gets the document of a
//! second evaluation, not the one that frontier was issued from, while the case has not moved.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::responsibility::{CaseId, GovernorError};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::json;
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use serde_json::Value;

const PROTOCOL: &str = "software-change@1";
const CASE: &str = "case-1";

/// A case opened on `software-change@1` and left alone: the frontier is read at 12:00:00 and the
/// document a second later, at the same case revision, with nothing recorded in between. The
/// stand-in clock is `HostTime`'s format (UTC, to the second) moving one second between the two
/// reads, as the system clock does whenever a host's two reads straddle a second.
#[test]
fn the_document_read_after_a_frontier_is_the_one_that_frontier_was_issued_from_under_a_host_clock()
{
    let readings = Arc::new(AtomicUsize::new(0));
    let clock = Arc::clone(&readings);
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&ProtocolCatalog::bundled().expect("the bundled catalog"))
        .expect("the catalog is admitted")
        .with_evaluation_time(move |_: &CaseId| -> Result<Option<String>, GovernorError> {
            let second = clock.fetch_add(1, Ordering::SeqCst);
            Ok(Some(format!("2026-10-04T12:00:{second:02}Z")))
        });
    let case = CaseId(CASE.into());
    governor
        .open_case(
            case.clone(),
            PROTOCOL,
            fixture_revisions(),
            fixture_inputs(),
        )
        .expect("the case opens");

    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    let document = governor.decision_document(&case).expect("a document");

    assert_eq!(
        governor.current_revision(&case),
        Ok(frontier.case_revision),
        "the case has not moved since the frontier was issued"
    );
    assert_eq!(
        frontier_id(CASE, frontier.case_revision, &canonical(&document)),
        frontier.frontier_id.0.0,
        "the document read at revision {} is the one the frontier issued at that revision was \
         issued from; it was decided at {:?}",
        frontier.case_revision,
        document
            .member("explanation")
            .and_then(|explanation| explanation.member("computed_from"))
            .and_then(|computed| computed.member("at")),
    );
}

fn fixture_revisions() -> BTreeMap<String, String> {
    fixture()["case"]["artifacts"]
        .as_object()
        .expect("artifacts")
        .iter()
        .map(|(artifact, entry)| {
            (
                artifact.clone(),
                entry["revision"].as_str().expect("revision").to_owned(),
            )
        })
        .collect()
}

fn fixture_inputs() -> CaseInputs {
    let inputs: Vec<(String, String)> = fixture()["case"]["inputs"]
        .as_object()
        .expect("inputs")
        .iter()
        .map(|(input, value)| {
            let value = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (input.clone(), value)
        })
        .collect();
    CaseInputs::try_from_pairs(inputs).expect("distinct inputs")
}

/// `tests/fixtures/chg-1842.fixture.yaml`.
fn fixture() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    serde_yaml_ng::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("YAML")
}

/// `document` written exactly as it was read.
fn compact(document: &json::Value) -> String {
    let mut out = String::new();
    json::push_value(&mut out, document);
    out
}

/// `document` as Canon renders a `canon-decision/1` document (`b10x_canon::eval::render`): object
/// keys in code-point order, members indented by two spaces, and one newline at the end.
fn canonical(document: &json::Value) -> String {
    let value: Value = serde_json::from_str(&compact(document)).expect("the document is JSON");
    let mut rendered = serde_json::to_string_pretty(&value).expect("JSON renders");
    rendered.push('\n');
    rendered
}

/// The frontier id the governor issues for `case` at `revision` from Canon's rendering `rendered`
/// of its decision: a version 8 UUID from two 64-bit FNV-1a hashes of
/// `<case>\n<revision>\n<rendered>`.
fn frontier_id(case: &str, revision: i64, rendered: &str) -> String {
    let fnv1a = |basis: u64, bytes: &[u8]| {
        bytes.iter().fold(basis, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let input = format!("{case}\n{revision}\n{rendered}");
    let high = fnv1a(0xcbf2_9ce4_8422_2325, input.as_bytes());
    let low = fnv1a(0x6c62_272e_07bb_0142, input.as_bytes());
    let bits = (u128::from(high) << 64 | u128::from(low)) & !(0xf << 76) & !(0x3 << 62)
        | (0x8 << 76)
        | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
