//! Commission's crates live in Loom's workspace and are built by path (`story:import-commission`).
//!
//! Three checks, each reading the tree at run time from `CARGO_MANIFEST_DIR`:
//!
//! 1. the root manifest's `[workspace]` members (with `crates/*`-style globs expanded against the
//!    tree, `exclude` applied) cover the five Commission crates;
//! 2. no `Cargo.toml` in the workspace names `github.com/beyond10x/commission`;
//! 3. `cargo metadata --format-version 1 --no-deps` on the root manifest lists `b10x-commission` as a
//!    workspace member whose manifest is `crates/commission/Cargo.toml`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const COMMISSION_CRATES: [&str; 5] = [
    "crates/commission",
    "crates/commission-conformance",
    "crates/commission-docs",
    "crates/commission-testkit",
    "crates/commission-xtask",
];

const COMMISSION_GIT: &str = "github.com/beyond10x/commission";

/// Directories never walked for manifests: build output, VCS data and node packages.
const SKIPPED_DIRS: [&str; 4] = ["target", ".git", "node_modules", ".engineering"];

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/loom sits two levels below the repository root")
        .to_path_buf()
}

/// The quoted strings of the array `key = [ ... ]` inside the `[workspace]` table of `manifest`.
/// The array may span several lines. Returns an empty list when the key is absent.
fn workspace_array(manifest: &str, key: &str) -> Vec<String> {
    let mut in_workspace = false;
    let mut collecting = false;
    let mut buffer = String::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if !collecting && trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            in_workspace = trimmed == "[workspace]";
            continue;
        }
        if !in_workspace {
            continue;
        }
        if !collecting {
            let Some((name, value)) = trimmed.split_once('=') else {
                continue;
            };
            if name.trim() != key {
                continue;
            }
            collecting = true;
            buffer.push_str(value);
        } else {
            buffer.push('\n');
            buffer.push_str(trimmed);
        }
        if buffer.contains(']') {
            break;
        }
    }
    let mut out = Vec::new();
    let mut rest = buffer.as_str();
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

/// Whether the `/`-separated relative path `dir` matches the member pattern `pattern`. A pattern
/// component `*` matches any one component; a component with a trailing `*` matches by prefix.
fn pattern_matches(pattern: &str, dir: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let dir_parts: Vec<&str> = dir.split('/').collect();
    pattern_parts.len() == dir_parts.len()
        && pattern_parts
            .iter()
            .zip(&dir_parts)
            .all(|(p, d)| match p.strip_suffix('*') {
                Some(prefix) => d.starts_with(prefix),
                None => p == d,
            })
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

#[test]
fn root_manifest_lists_the_five_commission_crates_as_members() {
    let root = repo_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read root Cargo.toml");
    let members = workspace_array(&manifest, "members");
    let exclude = workspace_array(&manifest, "exclude");
    assert!(
        !members.is_empty(),
        "the root Cargo.toml has no [workspace] members"
    );

    let mut missing = Vec::new();
    for krate in COMMISSION_CRATES {
        assert!(
            root.join(krate).join("Cargo.toml").is_file(),
            "{krate}/Cargo.toml is not in the tree"
        );
        let member = members.iter().any(|m| pattern_matches(m, krate));
        let excluded = exclude.iter().any(|e| pattern_matches(e, krate));
        if !member || excluded {
            missing.push(krate);
        }
    }
    assert!(
        missing.is_empty(),
        "root [workspace] does not make these Commission crates members: {missing:?} \
         (members = {members:?}, exclude = {exclude:?})"
    );
}

#[test]
fn no_manifest_names_the_commission_git_repository() {
    let root = repo_root();
    let mut manifests = Vec::new();
    manifests_under(&root, &mut manifests);
    assert!(
        manifests.iter().any(|m| m == &root.join("Cargo.toml")),
        "the walk did not reach the root Cargo.toml"
    );

    let mut offenders = Vec::new();
    for manifest in &manifests {
        let text = fs::read_to_string(manifest)
            .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
        for (index, line) in text.lines().enumerate() {
            if line.contains(COMMISSION_GIT) {
                let relative = manifest.strip_prefix(&root).unwrap_or(manifest);
                offenders.push(format!(
                    "{}:{}: {}",
                    relative.display(),
                    index + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "Cargo.toml files still name {COMMISSION_GIT}:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn cargo_metadata_lists_b10x_commission_as_a_path_member() {
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
    let package = packages
        .iter()
        .find(|p| p["name"] == "b10x-commission")
        .unwrap_or_else(|| {
            let names: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
            panic!("cargo metadata lists no b10x-commission; packages: {names:?}")
        });

    let manifest_path = PathBuf::from(
        package["manifest_path"]
            .as_str()
            .expect("package has a manifest_path"),
    );
    let expected = root.join("crates/commission/Cargo.toml");
    assert_eq!(
        fs::canonicalize(&manifest_path).expect("canonicalize manifest_path"),
        fs::canonicalize(&expected).expect("canonicalize crates/commission/Cargo.toml"),
        "b10x-commission is not built from crates/commission"
    );

    let id = package["id"].as_str().expect("package has an id");
    let members = metadata["workspace_members"]
        .as_array()
        .expect("cargo metadata has workspace_members");
    assert!(
        members.iter().any(|m| m == id),
        "b10x-commission ({id}) is not a workspace member"
    );
}
