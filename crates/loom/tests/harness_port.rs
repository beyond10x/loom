//! Acceptance of `story:harness-crate-port`: the Harness crates `docs/design/harness-map.md` marks
//! `port` from `harness-wire` to `harness-loop` are carried into `b10x-loom`, held to the two
//! provider-wire contracts copied from Harness, and relicensed Apache-2.0.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time, so a test binary reused from a
//! shared build directory reads the tree it is run from, not the one it was built in.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom::harness::wire::{
    Approval, CallId, Envelope, Item, Sampling, ToolCall, ToolChoice, ToolName, ToolOutcome,
    ToolSpec, TurnRequest, WireId,
};
use b10x_loom::harness::{http, messages, responses};
use serde_json::{Value, json};

/// The wires `crates/loom/tests/fixtures/provider-wires/` holds, each with the version Harness's
/// own contract tests pin at `798325f0`.
const WIRES: [(&str, &str); 2] = [
    ("anthropic-messages", "2026-08-31"),
    ("openai-responses", "2026-08-31.1"),
];

/// The Harness revision the port and the fixtures are taken from.
const HARNESS_REVISION: &str = "798325f03cf5a18df8fadb346d31b314826136ec";

/// Where each `port` row from `harness-wire` to `harness-loop` lands, relative to
/// `crates/loom/src/harness`.
const PORTED_MODULES: [&str; 5] = ["wire", "http", "responses", "messages", "turn_loop"];

/// Harness's licence. Assembled from two halves so that no file under `crates/` holds the
/// identifier itself, this one included.
const HARNESS_LICENCE: &str = concat!("LicenseRef-", "B10x-Proprietary");
const LOOM_LICENCE: &str = "Apache-2.0";
const SPDX: &str = "SPDX-License-Identifier:";

fn crate_dir() -> PathBuf {
    PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test"),
    )
}

fn workspace_dir() -> PathBuf {
    crate_dir().join("../..")
}

fn wires_dir() -> PathBuf {
    crate_dir().join("tests/fixtures/provider-wires")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn request_fixture(wire: &str, version: &str) -> Vec<u8> {
    let path = wires_dir()
        .join(wire)
        .join(version)
        .join("fixtures/turn-request.json");
    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn cargo() -> Command {
    let mut command = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    command.current_dir(workspace_dir());
    command
}

fn stdout_of(command: &mut Command) -> String {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("cannot run {command:?}: {e}"));
    assert!(
        output.status.success(),
        "{command:?} failed: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("cargo prints UTF-8")
}

/// The fixed one-turn input both wires are projected from: an instruction, a person's input, a
/// replayed reasoning item of the wire's own, one call and its result, one tool, sampling set and a
/// held tool choice. It is Harness's canonical contract turn, so every field a wire sends appears.
fn turn(wire: &str, opaque: Value, assistant_text: Option<&str>, call: &str) -> TurnRequest {
    let call_id = || CallId::new(call).expect("valid call id");
    let tool = || ToolName::new("workspace_read").expect("valid tool name");
    let mut items = vec![
        Item::user("read the readme"),
        Item::Opaque {
            wire: WireId::new(wire).expect("valid wire id"),
            payload: opaque,
        },
    ];
    if let Some(text) = assistant_text {
        items.push(Item::assistant(text));
    }
    items.push(Item::ToolCall(ToolCall {
        call_id: call_id(),
        name: tool(),
        arguments: json!({"path": "README.md"}),
    }));
    items.push(Item::result(
        call_id(),
        ToolOutcome::ok(json!({"text": "hello harness"})),
    ));
    TurnRequest {
        model: "b10x-emulated".to_owned(),
        instructions: "be useful".to_owned(),
        items,
        tools: vec![ToolSpec {
            name: tool(),
            description: "Read one text file inside the workspace.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
                "additionalProperties": false,
            }),
            approval: Approval::NotRequired,
            envelope: Envelope::default(),
        }],
        max_output_tokens: None,
        sampling: Sampling {
            temperature: Some(0.2),
            top_p: Some(0.95),
            reasoning_effort: Some("medium".to_owned()),
        },
        tool_choice: ToolChoice::Named(tool()),
    }
}

/// The request body the ported adapter of `wire` builds for the fixed turn, as the bytes the
/// ported transport would send.
fn built_request(wire: &str) -> Vec<u8> {
    let body = match wire {
        messages::WIRE => {
            let request = turn(
                wire,
                json!({
                    "type": "thinking",
                    "thinking": "OPAQUE-REASONING-BLOB",
                    "signature": "OPAQUE-SIGNATURE",
                }),
                Some("Reading the readme."),
                "toolu_1",
            );
            // This route requires an output bound; the caller passes it beside the turn.
            messages::request_body(&request, 4096, None)
        }
        responses::WIRE => {
            let mut request = turn(
                wire,
                json!({
                    "id": "rs_1",
                    "type": "reasoning",
                    "summary": [],
                    "encrypted_content": "OPAQUE",
                }),
                None,
                "call_1",
            );
            request.max_output_tokens = Some(4096);
            // Fixed so the pinned fixture stays byte-stable; a real run's key is per-conversation.
            responses::request_body("b10x-session-fixture", &request)
        }
        other => panic!("no ported adapter for the wire `{other}`"),
    };
    http::encode_json_body(&body).expect("the request body encodes")
}

/// The packages `docs/design/harness-map.md` § Map gives the disposition `port`.
fn ported_packages() -> BTreeSet<String> {
    let map = read(&workspace_dir().join("docs/design/harness-map.md"));
    let mut in_map = false;
    let mut packages = BTreeSet::new();
    for line in map.lines() {
        if line.starts_with("## ") {
            in_map = line.trim() == "## Map";
            continue;
        }
        if !in_map || !line.trim_start().starts_with("| `") {
            continue;
        }
        let cells: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().replace('`', ""))
            .collect();
        if cells.get(4).map(String::as_str) == Some("port") {
            packages.insert(cells[1].clone());
        }
    }
    packages
}

/// Every regular file below `dir`, recursively.
fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in
            fs::read_dir(&next).unwrap_or_else(|e| panic!("cannot list {}: {e}", next.display()))
        {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn harness_port_wires_and_licence() {
    // 1a. Exactly two wires, and the copy names the Harness revision it came from.
    let found: BTreeSet<String> = fs::read_dir(wires_dir())
        .expect("crates/loom/tests/fixtures/provider-wires exists")
        .map(|entry| entry.expect("a directory entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let expected: BTreeSet<String> = WIRES.iter().map(|(w, _)| (*w).to_owned()).collect();
    assert_eq!(found, expected, "the provider wires under fixtures");
    let provenance = read(&wires_dir().join("README.md"));
    assert!(
        provenance.contains(HARNESS_REVISION),
        "the provider-wire README does not name Harness {HARNESS_REVISION}"
    );

    // 1b. Each ported adapter builds exactly its wire's pinned request.
    assert_eq!(messages::WIRE, WIRES[0].0);
    assert_eq!(responses::WIRE, WIRES[1].0);
    for (wire, version) in WIRES {
        let expected = request_fixture(wire, version);
        let actual = built_request(wire);
        assert!(
            actual == expected,
            "{wire} {version}: the built request differs from the pinned fixture\n\
             built:  {}\npinned: {}",
            String::from_utf8_lossy(&actual),
            String::from_utf8_lossy(&expected)
        );
    }

    // 2a. No file under `crates/` carries Harness's licence.
    let carrying: Vec<PathBuf> = files_below(&workspace_dir().join("crates"))
        .into_iter()
        .filter(|path| {
            fs::read(path)
                .map(|bytes| {
                    bytes
                        .windows(HARNESS_LICENCE.len())
                        .any(|w| w == HARNESS_LICENCE.as_bytes())
                })
                .unwrap_or(false)
        })
        .collect();
    assert!(
        carrying.is_empty(),
        "files under crates/ carry {HARNESS_LICENCE}: {carrying:?}"
    );

    // 2b. Each ported file states Apache-2.0: an SPDX line of its own must name it, and a file
    // without one takes the licence of the crate that holds it, which must be Apache-2.0.
    let metadata: Value = serde_json::from_str(&stdout_of(cargo().args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--offline",
    ])))
    .expect("cargo metadata prints JSON");
    let licences: BTreeMap<&str, &str> = metadata["packages"]
        .as_array()
        .expect("cargo metadata lists packages")
        .iter()
        .map(|p| {
            (
                p["name"].as_str().expect("a package name"),
                p["license"].as_str().unwrap_or(""),
            )
        })
        .collect();
    let crate_licence = licences
        .get("b10x-loom")
        .copied()
        .expect("cargo metadata names b10x-loom");
    let harness_src = crate_dir().join("src/harness");
    let mut ported = 0;
    for module in PORTED_MODULES {
        let dir = harness_src.join(module);
        let sources: Vec<PathBuf> = files_below(&dir)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .collect();
        assert!(!sources.is_empty(), "{}: no ported source", dir.display());
        for source in sources {
            ported += 1;
            let text = read(&source);
            let stated: Vec<&str> = text
                .lines()
                .filter_map(|line| line.split_once(SPDX).map(|(_, id)| id.trim()))
                .collect();
            if stated.is_empty() {
                assert_eq!(
                    crate_licence,
                    LOOM_LICENCE,
                    "{} states no licence and b10x-loom's manifest is not {LOOM_LICENCE}",
                    source.display()
                );
            } else {
                assert!(
                    stated.iter().all(|id| *id == LOOM_LICENCE),
                    "{} states {stated:?}, not {LOOM_LICENCE}",
                    source.display()
                );
            }
        }
    }
    assert!(ported > 0, "no ported file was checked");

    // 2c. No crate the map ports is still a dependency of `b10x-loom`.
    let port_rows = ported_packages();
    assert!(
        port_rows.contains("b10x-harness-wire") && port_rows.contains("b10x-harness-loop"),
        "the map's port rows were not read: {port_rows:?}"
    );
    let tree = stdout_of(cargo().args([
        "tree",
        "-p",
        "b10x-loom",
        "-e",
        "normal",
        "--prefix",
        "none",
        "--locked",
    ]));
    let named: BTreeSet<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert!(
        named.contains("b10x-loom"),
        "cargo tree did not name b10x-loom:\n{tree}"
    );
    let still: Vec<&String> = port_rows
        .iter()
        .filter(|p| named.contains(p.as_str()))
        .collect();
    assert!(
        still.is_empty(),
        "b10x-loom still depends on ported Harness crates: {still:?}"
    );
}
