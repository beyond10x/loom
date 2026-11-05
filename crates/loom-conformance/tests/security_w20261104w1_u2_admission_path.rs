//! Security review, wave 2026-11-04-w1, unit U2 (`story:operator-constraints-in-ess`): the
//! conformance target's `AdmitLoomAgent` reads the resource its input `path` names.
//!
//! `crates/loom-conformance/src/admission.rs` documents the contract: "a `path` naming a regular
//! file under the repository root is read as it is; a synthesized path names none, and the
//! target reads the fixture of the branch the scenario forced ... or, forced nothing,
//! `resources/admitted.yaml`". AGENTS.md § Never: never read a credential file in Loom code or
//! tests. A `path` that leaves the repository root (absolute, or through `..`) names no file
//! under the root, so the target must answer from `resources/admitted.yaml`, never from the file
//! outside. The stand-in outside the root is a copy of the `secret-not-allowed` fixture, written
//! by this test into its own temporary directory: if the target read it, the outcome is
//! `secret-not-allowed`; if it kept to the root, the outcome is `admitted`.

use std::path::{Component, Path, PathBuf};

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

/// A directory outside the repository root holding a copy of the `secret-not-allowed` fixture.
struct Outside {
    dir: PathBuf,
    file: PathBuf,
}

impl Outside {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("loom-u2sec-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the outside directory");
        let dir = dir
            .canonicalize()
            .expect("the outside directory, canonical");
        assert!(
            !dir.starts_with(root()),
            "the stand-in must lie outside the repository root"
        );
        let fixture =
            root().join("ess/scenarios/resource-admission/resources/secret-not-allowed.yaml");
        let file = dir.join("outside.yaml");
        std::fs::copy(&fixture, &file).expect("the stand-in resource");
        Self { dir, file }
    }
}

impl Drop for Outside {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn admit(path: &str) -> String {
    let target = LoomTarget::default();
    let result = target
        .execute_command(SemanticCommandRequest {
            command: "loom.resource.AdmitLoomAgent"
                .parse()
                .expect("a command reference"),
            actor: None,
            caller: None,
            input: [("path".to_owned(), Node::Text(path.to_owned()))]
                .into_iter()
                .collect(),
            correlation: CorrelationId::new("security-w20261104w1-u2").expect("a correlation id"),
        })
        .unwrap_or_else(|error| panic!("AdmitLoomAgent: {error}"));
    result
        .outcome
        .as_ref()
        .map(ToString::to_string)
        .expect("a declared outcome")
}

#[test]
fn an_absolute_path_outside_the_repository_is_not_read() {
    let outside = Outside::new("abs");
    let outcome = admit(outside.file.to_str().expect("a UTF-8 path"));
    assert!(
        outcome.ends_with("admitted"),
        "the target read {} outside the repository root: outcome {outcome}",
        outside.file.display()
    );
}

#[test]
fn a_dot_dot_path_leaving_the_repository_is_not_read() {
    let outside = Outside::new("dotdot");
    let depth = root()
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    let up = "../".repeat(depth);
    let below = outside
        .file
        .strip_prefix("/")
        .expect("an absolute path")
        .to_str()
        .expect("a UTF-8 path");
    let outcome = admit(&format!("{up}{below}"));
    assert!(
        outcome.ends_with("admitted"),
        "the target read {} through `..`: outcome {outcome}",
        outside.file.display()
    );
}
