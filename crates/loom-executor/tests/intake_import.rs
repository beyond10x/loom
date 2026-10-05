//! Intake's crates live in Loom and build against Loom's own crates by path
//! (`story:import-intake`).
//!
//! Three checks, each reading the tree at run time from `CARGO_MANIFEST_DIR`:
//!
//! 1. no `Cargo.toml` in the workspace names `github.com/beyond10x/commission`, `/governor`,
//!    `/loom` or `/intake` (the root manifest's own `repository = ".../loom"` field excepted:
//!    it names this repository, not a dependency);
//! 2. `Cargo.lock` resolves exactly one copy each of `b10x-loom-commission`, `b10x-loom-governor`,
//!    `b10x-loom-executor`, `b10x-canon` and `b10x-canon-engineering`; the Commission, governor
//!    and Loom copies are the path ones (no `source`), and no package comes from one of the four
//!    repositories above;
//! 3. `cargo metadata --format-version 1 --no-deps` on the root manifest lists the four intake
//!    packages as workspace members, each built from its `crates/loom-intake-*` directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repositories whose crates now live in Loom: none may be named as a dependency source.
const ABSORBED_REPOSITORIES: [&str; 4] = ["commission", "governor", "loom", "intake"];

const GITHUB_ORG: &str = "github.com/beyond10x/";

/// The packages `Cargo.lock` must resolve exactly once, and whether that copy must be the path one.
const SINGLE_COPY: [(&str, bool); 5] = [
    ("b10x-loom-commission", true),
    ("b10x-loom-governor", true),
    ("b10x-loom-executor", true),
    ("b10x-canon", false),
    ("b10x-canon-engineering", false),
];

/// The intake packages and the directory each must be built from.
const INTAKE_PACKAGES: [(&str, &str); 4] = [
    ("b10x-loom-intake-router", "crates/loom-intake-router"),
    (
        "b10x-loom-intake-references",
        "crates/loom-intake-references",
    ),
    ("b10x-loom-intake-slice", "crates/loom-intake-slice"),
    ("b10x-loom-cli", "crates/loom-cli"),
];

/// Directories never walked for manifests: build output, VCS data, node packages, the store.
const SKIPPED_DIRS: [&str; 4] = ["target", ".git", "node_modules", ".engineering"];

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/loom-executor sits two levels below the repository root")
        .to_path_buf()
}

fn manifests_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            let name = entry.file_name();
            if SKIPPED_DIRS.iter().any(|s| name == *s) {
                continue;
            }
            manifests_under(&path, out);
        } else if file_type.is_file() && entry.file_name() == "Cargo.toml" {
            out.push(path);
        }
    }
}

/// The absorbed repositories `text` names: every `github.com/beyond10x/<name>` occurrence whose
/// `<name>` (up to the next character that cannot continue a repository name) is absorbed.
fn absorbed_names_in(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(GITHUB_ORG) {
        let after = &rest[start + GITHUB_ORG.len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(after.len());
        let name = &after[..end];
        if let Some(absorbed) = ABSORBED_REPOSITORIES.iter().find(|a| **a == name) {
            found.push(*absorbed);
        }
        rest = &after[end..];
    }
    found
}

#[test]
fn absorbed_name_scan_can_fail() {
    assert_eq!(
        absorbed_names_in(
            r#"b10x-loom = { git = "https://github.com/beyond10x/loom", rev = "x" }"#
        ),
        vec!["loom"]
    );
    assert_eq!(
        absorbed_names_in(r#"x = { git = "https://github.com/beyond10x/intake.git" }"#),
        vec!["intake"]
    );
    assert!(
        absorbed_names_in(r#"x = { git = "https://github.com/beyond10x/loom-extra" }"#).is_empty()
    );
    assert!(absorbed_names_in(r#"els = { git = "https://github.com/beyond10x/els" }"#).is_empty());
}

#[test]
fn no_manifest_names_an_absorbed_repository() {
    let root = repo_root();
    let root_manifest = root.join("Cargo.toml");
    let mut manifests = Vec::new();
    manifests_under(&root, &mut manifests);
    assert!(
        manifests.iter().any(|m| m == &root_manifest),
        "the walk did not reach the root Cargo.toml"
    );
    for (_, dir) in INTAKE_PACKAGES {
        let manifest = root.join(dir).join("Cargo.toml");
        assert!(
            manifests.iter().any(|m| m == &manifest),
            "the walk did not reach {dir}/Cargo.toml"
        );
    }

    let mut offenders = Vec::new();
    for manifest in &manifests {
        let text = fs::read_to_string(manifest)
            .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
        for (index, line) in text.lines().enumerate() {
            let names = absorbed_names_in(line);
            if names.is_empty() {
                continue;
            }
            let own_repository_field = manifest == &root_manifest
                && names == ["loom"]
                && line
                    .split_once('=')
                    .is_some_and(|(key, _)| key.trim() == "repository");
            if own_repository_field {
                continue;
            }
            let relative = manifest.strip_prefix(&root).unwrap_or(manifest);
            offenders.push(format!(
                "{}:{}: {}",
                relative.display(),
                index + 1,
                line.trim()
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "Cargo.toml files still name an absorbed repository ({ABSORBED_REPOSITORIES:?}):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn lock_resolves_one_path_copy_of_each_absorbed_crate() {
    let root = repo_root();
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");

    let mut copies: Vec<(String, Option<String>)> = Vec::new();
    let mut absorbed_sources = Vec::new();
    for block in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block.lines().find_map(|line| {
                let (k, v) = line.split_once('=')?;
                (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
            })
        };
        let name = field("name").unwrap_or_default();
        let source = field("source");
        if let Some(source) = &source
            && !absorbed_names_in(source).is_empty()
        {
            absorbed_sources.push(format!("{name} from {source}"));
        }
        copies.push((name, source));
    }

    let mut problems = Vec::new();
    for (name, must_be_path) in SINGLE_COPY {
        let found: Vec<&Option<String>> = copies
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, s)| s)
            .collect();
        if found.len() != 1 {
            problems.push(format!(
                "{name}: {} copies {:?}",
                found.len(),
                found
                    .iter()
                    .map(|s| s.as_deref().unwrap_or("path"))
                    .collect::<Vec<_>>()
            ));
        } else if must_be_path && let Some(source) = found[0] {
            problems.push(format!("{name}: resolved from {source}, not by path"));
        }
    }
    problems.extend(absorbed_sources);
    assert!(
        problems.is_empty(),
        "Cargo.lock does not resolve Loom's own crates once, by path:\n{}",
        problems.join("\n")
    );
}

#[test]
fn cargo_metadata_lists_the_four_intake_packages_as_members() {
    let root = repo_root();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("spawn cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let packages = metadata["packages"]
        .as_array()
        .expect("cargo metadata has a packages array");
    let members = metadata["workspace_members"]
        .as_array()
        .expect("cargo metadata has workspace_members");

    let mut problems = Vec::new();
    for (name, dir) in INTAKE_PACKAGES {
        let Some(package) = packages.iter().find(|p| p["name"] == name) else {
            problems.push(format!("{name}: not listed"));
            continue;
        };
        let manifest_path = PathBuf::from(
            package["manifest_path"]
                .as_str()
                .expect("package has a manifest_path"),
        );
        let expected = root.join(dir).join("Cargo.toml");
        if fs::canonicalize(&manifest_path).ok() != fs::canonicalize(&expected).ok() {
            problems.push(format!(
                "{name}: built from {}, not {dir}",
                manifest_path.display()
            ));
        }
        let id = package["id"].as_str().expect("package has an id");
        if !members.iter().any(|m| m == id) {
            problems.push(format!("{name}: not a workspace member"));
        }
    }
    assert!(
        problems.is_empty(),
        "cargo metadata does not list intake's packages as members:\n{}",
        problems.join("\n")
    );
}
