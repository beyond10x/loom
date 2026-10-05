//! Every Loom crate carries a `loom-` name (`story:crate-names`).
//!
//! Reads `cargo metadata --no-deps --format-version 1` on the root manifest at run time and checks:
//!
//! 1. the workspace package set equals the expected set exactly, each package at its expected
//!    `crates/<dir>` manifest and, where it has a library, with its expected library name;
//! 2. no package named `b10x-commission*`, `b10x-governor` or `b10x-intake-*` remains;
//! 3. every product package is published as `b10x-loom-*`, and the only other packages are
//!    `loom-*-docs` / `loom-*-xtask` tooling;
//! 4. the ESS-generated crates outside the workspace keep the package names ESS emits from the
//!    system names (`loom`, `commission`): they are regenerated, never renamed by hand.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `(package, directory under crates/, library name)` for every workspace member.
const EXPECTED: [(&str, &str, Option<&str>); 14] = [
    (
        "b10x-loom-executor",
        "loom-executor",
        Some("b10x_loom_executor"),
    ),
    (
        "b10x-loom-commission",
        "loom-commission",
        Some("b10x_loom_commission"),
    ),
    (
        "b10x-loom-commission-testkit",
        "loom-commission-testkit",
        Some("b10x_loom_commission_testkit"),
    ),
    (
        "b10x-loom-commission-conformance",
        "loom-commission-conformance",
        Some("b10x_loom_commission_conformance"),
    ),
    ("loom-commission-docs", "loom-commission-docs", None),
    ("loom-commission-xtask", "loom-commission-xtask", None),
    ("b10x-loom-governor", "loom-governor", Some("loom_governor")),
    (
        "b10x-loom-intake-router",
        "loom-intake-router",
        Some("b10x_loom_intake_router"),
    ),
    (
        "b10x-loom-intake-references",
        "loom-intake-references",
        Some("b10x_loom_intake_references"),
    ),
    (
        "b10x-loom-intake-slice",
        "loom-intake-slice",
        Some("b10x_loom_intake_slice"),
    ),
    ("b10x-loom-cli", "loom-cli", Some("b10x_loom_cli")),
    ("b10x-loom-sdk", "loom-sdk", Some("loom_sdk")),
    ("loom-docs", "loom-docs", None),
    ("loom-xtask", "loom-xtask", None),
];

/// The ESS-generated crates, excluded from the workspace: `(directory under generated/rust/,
/// package name ESS emits)`.
const GENERATED: [(&str, &str); 2] = [("loom", "loom"), ("commission", "commission")];

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels below the repository root")
        .to_path_buf()
}

fn metadata(manifest: &Path) -> serde_json::Value {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("spawn cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata on {} failed ({}):\n{}",
        manifest.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON")
}

struct Package {
    manifest: PathBuf,
    lib: Option<String>,
}

fn workspace_packages() -> BTreeMap<String, Package> {
    let root = repo_root();
    let metadata = metadata(&root.join("Cargo.toml"));
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata has a packages array");
    packages
        .iter()
        .map(|p| {
            let name = p["name"].as_str().expect("package name").to_owned();
            let manifest = PathBuf::from(p["manifest_path"].as_str().expect("manifest_path"));
            let lib = p["targets"]
                .as_array()
                .expect("targets array")
                .iter()
                .find(|t| {
                    t["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|k| k == "lib"))
                })
                .map(|t| t["name"].as_str().expect("target name").to_owned());
            (name, Package { manifest, lib })
        })
        .collect()
}

fn expected() -> impl Iterator<Item = (&'static str, &'static str, Option<&'static str>)> {
    EXPECTED.into_iter()
}

#[test]
fn workspace_package_set_is_the_loom_names() {
    let actual = workspace_packages();
    let actual_names: Vec<&str> = actual.keys().map(String::as_str).collect();
    let mut expected_names: Vec<&str> = expected().map(|(name, _, _)| name).collect();
    expected_names.sort_unstable();
    assert_eq!(
        actual_names, expected_names,
        "the workspace package set is not the loom- names"
    );
}

#[test]
fn each_package_sits_in_its_loom_directory_with_its_library_name() {
    let root = repo_root();
    let actual = workspace_packages();
    let mut wrong = Vec::new();
    for (name, dir, lib) in expected() {
        let Some(package) = actual.get(name) else {
            wrong.push(format!("{name}: missing"));
            continue;
        };
        let want = root.join("crates").join(dir).join("Cargo.toml");
        if package.manifest != want {
            wrong.push(format!(
                "{name}: manifest {} is not {}",
                package.manifest.display(),
                want.display()
            ));
        }
        if package.lib.as_deref() != lib {
            wrong.push(format!(
                "{name}: library {:?} is not {:?}",
                package.lib, lib
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "packages out of place:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn no_commission_governor_or_intake_package_remains() {
    let left: Vec<String> = workspace_packages()
        .into_keys()
        .filter(|name| {
            name.starts_with("b10x-commission")
                || name == "b10x-governor"
                || name.starts_with("b10x-intake-")
        })
        .collect();
    assert!(
        left.is_empty(),
        "packages with pre-loom names remain: {left:?}"
    );
}

#[test]
fn product_packages_are_b10x_loom_and_the_rest_is_loom_tooling() {
    let stray: Vec<String> = workspace_packages()
        .into_keys()
        .filter(|name| {
            let product = name.starts_with("b10x-loom-");
            let tooling =
                name.starts_with("loom-") && (name.ends_with("-docs") || name.ends_with("-xtask"));
            !(product || tooling)
        })
        .collect();
    assert!(
        stray.is_empty(),
        "packages neither b10x-loom-* nor loom-*-docs/-xtask: {stray:?}"
    );
}

#[test]
fn generated_crates_keep_the_names_ess_emits() {
    let root = repo_root();
    for (dir, want) in GENERATED {
        let manifest = root.join("generated/rust").join(dir).join("Cargo.toml");
        let metadata = metadata(&manifest);
        let names: Vec<&str> = metadata["packages"]
            .as_array()
            .expect("cargo metadata has a packages array")
            .iter()
            .filter_map(|p| p["name"].as_str())
            .collect();
        assert_eq!(
            names,
            [want],
            "generated/rust/{dir} is not the ESS-emitted package {want}"
        );
    }
}
