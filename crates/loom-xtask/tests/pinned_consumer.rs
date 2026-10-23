//! `story:pinned-consumer-check`: `consumers/pinned/` is a crate outside Loom's workspace that
//! depends on `b10x-loom-sdk` by path and nothing else, and compiles the documented embedding
//! (`crates/loom-sdk/examples/software_change.rs`) as an external crate would.
//!
//! * Its manifest is its own workspace, with exactly one dependency, `b10x-loom-sdk`, by `path`
//!   alone, and no `[patch]`.
//! * It resolves one copy of each Loom and Canon crate (`cargo tree --duplicates`, offline, over
//!   the root `Cargo.lock`, which `task consumer-check` copies in the same way).
//! * Its `src/main.rs` includes the SDK example by `#[path]` and has no embedding code of its own.
//! * Its library is a second consumer (`story:pinned-consumers-agency-shape`): `src/agency.rs`
//!   imports from `std` and `loom_sdk` alone, runs the `loom.agent/1` spec in `agents/` through a
//!   `loom_sdk::Runtime` with `NoEvents` and a memory case store, and keeps the spec's limits as the
//!   agent's `Limits`.
//!
//! `task consumer-check` builds and runs the consumer itself
//! (`consumers/pinned/tests/embedding.rs`, `consumers/pinned/tests/agent_spec.rs`). The repository
//! is read from `CARGO_MANIFEST_DIR` at run time.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The SDK example, from the repository root.
const EXAMPLE: &str = "crates/loom-sdk/examples/software_change.rs";

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn consumer() -> PathBuf {
    repository().join("consumers/pinned")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|e| panic!("resolve {}: {e}", path.display()))
}

fn manifest() -> toml::Table {
    let path = consumer().join("Cargo.toml");
    read(&path)
        .parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn the_pinned_consumer_depends_on_the_sdk_alone_by_path() {
    let manifest = manifest();

    assert!(
        manifest.get("workspace").is_some_and(toml::Value::is_table),
        "consumers/pinned/Cargo.toml declares its own [workspace], so the root workspace neither \
         includes nor rejects it"
    );
    assert!(
        !manifest.contains_key("patch"),
        "consumers/pinned/Cargo.toml has no [patch] section"
    );
    for section in ["dev-dependencies", "build-dependencies", "target"] {
        assert!(
            !manifest.contains_key(section),
            "consumers/pinned/Cargo.toml declares no [{section}]: the SDK is its only dependency"
        );
    }

    let dependencies = manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("consumers/pinned/Cargo.toml has a [dependencies] table"));
    let names: Vec<&String> = dependencies.keys().collect();
    assert_eq!(
        names,
        ["b10x-loom-sdk"],
        "the consumer depends on b10x-loom-sdk and nothing else"
    );

    let sdk = dependencies["b10x-loom-sdk"]
        .as_table()
        .unwrap_or_else(|| panic!("b10x-loom-sdk is an inline table with a path"));
    for key in [
        "git", "tag", "rev", "branch", "version", "registry", "package",
    ] {
        assert!(
            !sdk.contains_key(key),
            "b10x-loom-sdk is pinned by path alone, without `{key}`: {sdk:?}"
        );
    }
    let path = sdk
        .get("path")
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("b10x-loom-sdk has a `path`: {sdk:?}"));
    assert_eq!(
        canonical(&consumer().join(path)),
        canonical(&repository().join("crates/loom-sdk")),
        "the path names this tree's crates/loom-sdk"
    );
}

#[test]
fn the_pinned_consumer_resolves_one_copy_of_each_loom_crate() {
    // As `task consumer-check` does: the consumer resolves the versions the workspace locked.
    let root = repository();
    let lockfile = consumer().join("Cargo.lock");
    fs::copy(root.join("Cargo.lock"), &lockfile)
        .unwrap_or_else(|e| panic!("copy Cargo.lock to {}: {e}", lockfile.display()));

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["tree", "--manifest-path"])
        .arg(consumer().join("Cargo.toml"))
        // Depth 0 lists the duplicated packages alone, not the crates that depend on them.
        .args([
            "--offline",
            "--duplicates",
            "--depth",
            "0",
            "--prefix",
            "none",
        ])
        .args(["--target", "all"])
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("run cargo tree: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo tree over consumers/pinned succeeds offline:\n{stderr}"
    );

    let duplicated: Vec<&str> = stdout
        .lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or_default();
            name.starts_with("b10x-loom-")
                || name.starts_with("loom-")
                || name == "b10x-canon"
                || name == "b10x-canon-engineering"
        })
        .collect();
    assert!(
        duplicated.is_empty(),
        "the consumer resolves a second copy of a Loom or Canon crate:\n{}\n\ncargo tree \
         --duplicates printed:\n{stdout}",
        duplicated.join("\n")
    );
}

#[test]
fn the_pinned_consumer_compiles_the_sdk_example_source() {
    let main = consumer().join("src/main.rs");
    let source = read(&main);

    let included: Vec<PathBuf> = source
        .lines()
        .filter_map(|line| {
            let attribute = line.trim().strip_prefix("#[path = \"")?;
            let (path, _) = attribute.split_once('"')?;
            Some(canonical(&main.parent().expect("src/").join(path)))
        })
        .collect();
    assert_eq!(
        included,
        [canonical(&repository().join(EXAMPLE))],
        "consumers/pinned/src/main.rs includes {EXAMPLE} by #[path], and nothing else"
    );

    // Code, without comments: the embedding is the example's, so main.rs names no SDK item and
    // declares no type, trait or implementation of its own.
    let code: String = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("loom_sdk"),
        "consumers/pinned/src/main.rs names nothing from loom_sdk itself; the example does"
    );
    let own: Vec<&str> = code
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| ["impl", "struct", "enum", "trait", "static", "const"].contains(word))
        .collect();
    assert!(
        own.is_empty(),
        "consumers/pinned/src/main.rs declares no embedding code of its own: {own:?}"
    );
}

/// `source` without its `//` comments.
fn code(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The crate each `use` declaration of `code` starts from, in order.
fn use_roots(code: &str) -> Vec<String> {
    code.lines()
        .filter_map(|line| {
            let path = line.trim().strip_prefix("use ")?;
            let root: String = path
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            Some(root)
        })
        .collect()
}

#[test]
fn the_agent_consumer_runs_a_spec_through_the_sdk_alone() {
    // `story:pinned-consumers-agency-shape`: the crate's library is the second consumer. It
    // declares the `agency` module and nothing else, and includes no other source by #[path].
    let lib = read(&consumer().join("src/lib.rs"));
    let lib_code = code(&lib);
    assert_eq!(
        lib_code
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>(),
        ["pub mod agency;"],
        "consumers/pinned/src/lib.rs declares the agency module alone"
    );

    let agency = read(&consumer().join("src/agency.rs"));
    let agency_code = code(&agency);
    assert!(
        !agency_code.contains("#[path"),
        "consumers/pinned/src/agency.rs includes no source by #[path]"
    );
    let roots = use_roots(&agency_code);
    assert!(
        roots.iter().any(|root| root == "loom_sdk"),
        "consumers/pinned/src/agency.rs imports from loom_sdk: {roots:?}"
    );
    let other: Vec<&String> = roots
        .iter()
        .filter(|root| !["std", "loom_sdk", "crate"].contains(&root.as_str()))
        .collect();
    assert!(
        other.is_empty(),
        "consumers/pinned/src/agency.rs imports from std and loom_sdk alone: {other:?}"
    );
    for (part, why) in [
        ("Runtime::new(", "builds a loom_sdk::Runtime"),
        ("NoEvents", "hands the runtime NoEvents"),
        ("MemoryCaseStore", "keeps its case in a memory case store"),
        (
            "impl Limits for AgentLimits",
            "keeps the spec's limits as the agent's Limits",
        ),
        (
            "include_str!(\"../agents/clock-reader.yaml\")",
            "runs the spec in consumers/pinned/agents/",
        ),
    ] {
        assert!(
            agency_code.contains(part),
            "consumers/pinned/src/agency.rs {why} (`{part}`)"
        );
    }

    // The spec is a `loom.agent/1` document whose session declares the budget and stop policy the
    // run is held to.
    let path = consumer().join("agents/clock-reader.yaml");
    let spec: serde_yaml_ng::Value = serde_yaml_ng::from_str(&read(&path))
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    assert_eq!(
        spec["format"].as_str(),
        Some("loom.agent/1"),
        "{} is a loom.agent/1 document",
        path.display()
    );
    for member in ["budget", "stop"] {
        assert!(
            spec["session"][member].is_mapping(),
            "{} declares session.{member}",
            path.display()
        );
    }

    // `task consumer-check` runs the test that runs it.
    let test = read(&consumer().join("tests/agent_spec.rs"));
    assert!(
        test.contains("fn an_agent_defined_as_data_runs()"),
        "consumers/pinned/tests/agent_spec.rs holds an_agent_defined_as_data_runs"
    );
}

#[test]
fn task_check_runs_the_consumer_check() {
    let taskfile = read(&repository().join("Taskfile.yml"));
    let body = |name: &str| -> Vec<String> {
        let header = format!("\n  {name}:\n");
        let start = taskfile
            .find(&header)
            .unwrap_or_else(|| panic!("Taskfile.yml has task `{name}`"));
        taskfile[start + header.len()..]
            .lines()
            .take_while(|line| line.is_empty() || line.starts_with("    "))
            .map(|line| line.trim().to_owned())
            .collect()
    };

    assert!(
        body("check")
            .iter()
            .any(|line| line == "- task: consumer-check"),
        "task check runs consumer-check as its own step"
    );
    let steps = body("consumer-check");
    for command in [
        "- cp Cargo.lock consumers/pinned/Cargo.lock",
        "- cargo test --manifest-path consumers/pinned/Cargo.toml --offline",
    ] {
        assert!(
            steps.iter().any(|line| line == command),
            "task consumer-check runs `{command}`: {steps:?}"
        );
    }
    assert!(
        !taskfile.contains("CARGO_TARGET_DIR"),
        "the consumer builds into its own consumers/pinned/target/"
    );
}
