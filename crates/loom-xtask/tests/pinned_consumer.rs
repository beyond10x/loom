//! `story:pinned-consumer-check`: `consumers/pinned/` is a crate outside Loom's workspace that
//! depends on `b10x-loom-sdk` by path and nothing else, and compiles the documented embedding
//! (`crates/loom-sdk/examples/software_change.rs`) as an external crate would.
//!
//! * Its manifest is its own workspace, with exactly one dependency, `b10x-loom-sdk`, by `path`
//!   alone, and no `[patch]`.
//! * It resolves one copy of each Loom and Canon crate (`cargo tree --duplicates`, offline, over
//!   the root `Cargo.lock`, which `task consumer-check` copies in the same way).
//! * Its `src/main.rs` includes the SDK example by `#[path]` and has no embedding code of its own.
//!
//! `task consumer-check` builds and runs the consumer itself
//! (`consumers/pinned/tests/embedding.rs`). The repository is read from `CARGO_MANIFEST_DIR` at
//! run time.

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
