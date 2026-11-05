//! Security review, wave 2026-11-04-w1, unit U2: the same read as `AdmitLoomAgent`'s in the
//! conformance target's `CheckAgentSpec` (`crates/loom-conformance/src/agent.rs`). Its module
//! documentation reads "a `path` naming a regular file under the repository root is read as it
//! is"; AGENTS.md § Never: never read a credential file in Loom code or tests. A `path` outside the
//! root names no file under it, so the target checks `examples/agents/operator.yaml` and answers
//! `serveable`; the stand-in outside the root is a copy of the `malformed` fixture, which would
//! answer `malformed` if the target read it.

use std::path::{Path, PathBuf};

use b10x_loom_conformance::LoomTarget;
use ess_conformance::target::{ConformanceTarget, SemanticCommandRequest};
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn check(path: &str) -> String {
    let target = LoomTarget::default();
    let result = target
        .execute_command(SemanticCommandRequest {
            command: "loom.agent.CheckAgentSpec"
                .parse()
                .expect("a command reference"),
            actor: None,
            caller: None,
            input: [("path".to_owned(), Node::Text(path.to_owned()))]
                .into_iter()
                .collect(),
            correlation: CorrelationId::new("security-w20261104w1-agent")
                .expect("a correlation id"),
        })
        .unwrap_or_else(|error| panic!("CheckAgentSpec: {error}"));
    result
        .outcome
        .as_ref()
        .map(ToString::to_string)
        .expect("a declared outcome")
}

#[test]
fn an_agent_document_outside_the_repository_is_not_read() {
    let dir = std::env::temp_dir().join(format!("loom-agent-path-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the outside directory");
    let dir = dir
        .canonicalize()
        .expect("the outside directory, canonical");
    assert!(
        !dir.starts_with(root()),
        "the stand-in must lie outside the repository root"
    );
    let file = dir.join("outside.yaml");
    std::fs::copy(
        root().join("ess/scenarios/agent-check/specs/malformed.yaml"),
        &file,
    )
    .expect("the stand-in document");

    let outcome = check(file.to_str().expect("a UTF-8 path"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        outcome.ends_with("serveable"),
        "the target read {} outside the repository root: outcome {outcome}",
        file.display()
    );
}
