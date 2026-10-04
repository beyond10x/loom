//! Adversary pass 2 on `story:commission-ess-conformance`: numbers through the codec.
//!
//! Pass 1 drove integers above `2^53`. This file drives what it did not: non-integer numbers, the
//! binary64-only magnitudes, negative zero, the `decimal_literal` fallback in `codec::number_of`,
//! and the input-decoding contract in the `codec` module doc.

use b10x_commission_conformance::codec::{self, Input};
use b10x_commission_conformance::run_suite;
use ess_primitives::facts::Number;
use ess_primitives::node::Node;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn synthesized(tag: &str) -> Value {
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary2-codec-{tag}-{}.json",
        std::process::id()
    ));
    let output = Command::new("ess")
        .current_dir(root())
        .args(["verify", "conform", "synthesize", "--path", "ess", "--out"])
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "synthesize exited {}",
        output.status
    );
    let text = std::fs::read_to_string(&out).expect("read suite");
    let _ = std::fs::remove_file(&out);
    serde_json::from_str(&text).expect("suite is JSON")
}

/// Replaces the input literal and the expected payload of `key` in `node`.
fn retarget(node: &mut Value, key: &str, new: &Value, bare: &dyn Fn(&Value) -> bool) {
    match node {
        Value::Object(members) => {
            for (name, value) in members.iter_mut() {
                if name == key {
                    if value.get("kind") == Some(&json!("literal")) {
                        value["value"] = new.clone();
                        continue;
                    }
                    if bare(value) {
                        *value = new.clone();
                        continue;
                    }
                }
                retarget(value, key, new, bare);
            }
        }
        Value::Array(items) => {
            for item in items {
                retarget(item, key, new, bare);
            }
        }
        _ => {}
    }
}

fn assert_passes(suite: &Value, scenario: &str) {
    let executed = run_suite(&suite.to_string()).unwrap_or_else(|e| panic!("did not run: {e}"));
    let report: Value = serde_json::from_str(&executed.report).expect("report is JSON");
    let passed = report["outcomes"]["passed"]
        .as_array()
        .expect("outcomes.passed")
        .iter()
        .any(|id| id == scenario);
    if !passed {
        let at = executed.diagnostics.find(scenario).unwrap_or(0);
        let end = (at + 2500).min(executed.diagnostics.len());
        panic!(
            "`{scenario}` did not pass; report outcomes: {}\ndiagnostics: {}",
            report["outcomes"],
            &executed.diagnostics[at..end]
        );
    }
}

fn suspend_with_authority_value(tag: &str, value: &Value) {
    let scenario = "commission.responsibility.SuspendRun/outcome/suspended";
    let mut suite = synthesized(tag);
    let reason = json!({"kind": "Authority", "value": value});
    retarget(
        &mut suite["scenarios"][scenario],
        "reason",
        &reason,
        &|value| value.get("kind") == Some(&json!("Authority")),
    );
    assert!(
        suite["scenarios"][scenario]
            .to_string()
            .contains(&reason.to_string()),
        "the edit did not land"
    );
    assert_passes(&suite, scenario);
}

/// `RunSuspended.reason` is `input.reason`, so every non-integer `Json` number in it must come back
/// as the value it was: the short decimals, the binary64-only magnitudes (`1e-300`, `1e300`,
/// `1e40`, `f64::MAX`, the smallest subnormal) and an integral float.
#[test]
fn adversary2_suspend_run_echoes_non_integer_numbers() {
    suspend_with_authority_value(
        "non-integer",
        &json!({
            "half": 1.5,
            "tenth": 0.1,
            "negative": -2.25,
            "third": 1.0_f64 / 3.0,
            "tiny": 1e-300,
            "huge": 1e300,
            "wide": 1e40,
            "max": f64::MAX,
            "subnormal": 5e-324,
            "integral_float": 3.0,
            "min_i64": i64::MIN,
            "max_u64": u64::MAX,
            "list": [0.5, -0.5, 1e-7, 123_456.789],
        }),
    );
}

/// Negative zero in a `Json` reason: ESS compares numbers by value, and `-0` and `0` are one value
/// (`ess_primitives::facts::Number::cmp`), so the scenario must pass.
#[test]
fn adversary2_suspend_run_echoes_negative_zero() {
    suspend_with_authority_value("negative-zero", &json!({"zero": -0.0, "list": [-0.0, 0.0]}));
}

/// A fixed-seed generator, so a failure names the same inputs on every machine.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

/// The numbers the property runs over: random finite binary64 bit patterns (most are the
/// binary64-only magnitudes), short decimals, integers near `2^53` and `2^63`, and the edges.
fn numbers() -> Vec<Number> {
    let mut rng = Lcg(0x5EED_2026_1004);
    let mut out = Vec::new();
    while out.len() < 20_000 {
        let value = f64::from_bits(rng.next());
        if value.is_finite() {
            out.push(Number::new(value).expect("finite"));
        }
    }
    for _ in 0..5_000 {
        #[allow(clippy::cast_possible_wrap)]
        let units = rng.next() as i64 >> (rng.next() % 60);
        let places = i32::try_from(rng.next() % 12).expect("small");
        #[allow(clippy::cast_precision_loss)]
        let value = units as f64 / 10_f64.powi(places);
        out.push(Number::new(value).expect("finite"));
        out.push(Number::from(units));
    }
    for base in [1_i64 << 53, i64::MAX, i64::MIN] {
        for delta in -3_i64..=3 {
            if let Some(value) = base.checked_add(delta) {
                out.push(Number::from(value));
            }
        }
    }
    for value in [
        0.0,
        -0.0,
        f64::MIN_POSITIVE,
        5e-324,
        -5e-324,
        f64::MAX,
        f64::MIN,
        1e-300,
        1e300,
        1e40,
        -1e40,
        9_223_372_036_854_775_808.0,
        18_446_744_073_709_551_616.0,
    ] {
        out.push(Number::new(value).expect("finite"));
    }
    out
}

/// The codec's round trip keeps every number exactly, binary64 included, and never takes the
/// `serde_json` fallback in `number_of`: `Number::decimal_literal` reads back every spelling
/// `to_json` writes, the binary64-only magnitudes included. The one bit pattern allowed to change
/// is the sign of zero, which `Number` compares equal.
#[test]
fn adversary2_codec_round_trip_is_exact_and_never_falls_back() {
    let mut wrong = Vec::new();
    for number in numbers() {
        let spelling = number.exact_text();
        if Number::decimal_literal(&spelling) != Some(number) {
            wrong.push(format!("decimal_literal refuses or changes `{spelling}`"));
        }
        let back = codec::from_json(
            &codec::to_json(&Node::Number(number)).expect("a number is a Json value"),
        );
        match back {
            Node::Number(read) if read == number => {
                let zero = number.get() == 0.0;
                if !zero && read.get().to_bits() != number.get().to_bits() {
                    wrong.push(format!(
                        "`{spelling}`: binary64 {:e} came back as {:e}",
                        number.get(),
                        read.get()
                    ));
                }
            }
            other => wrong.push(format!("`{spelling}` came back as {other:?}")),
        }
        if wrong.len() > 20 {
            break;
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The `codec` module doc: decoding "answers `None` when the value cannot be held by that type".
/// The generated model carries a `Uuid` "as its canonical textual rendering", and the generated
/// wire reader (`model::json::uuid_at`) refuses any other spelling; `codec::uuid` accepts any text.
#[test]
fn adversary2_uuid_refuses_text_that_is_not_a_canonical_uuid() {
    for spelling in [
        "not-a-uuid",
        "",
        "{00000000-0000-4000-8000-000000000001}",
        "urn:uuid:00000000-0000-4000-8000-000000000001",
        " 00000000-0000-4000-8000-000000000001",
    ] {
        let input = Input::from([("run_id".to_owned(), Node::Text(spelling.to_owned()))]);
        let decoded = codec::uuid(&input, "run_id");
        assert!(
            decoded.is_none(),
            "codec::uuid decoded `{spelling}` as {decoded:?}, which the generated Uuid cannot hold"
        );
    }
}
