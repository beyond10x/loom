//! The governor crate lives in Loom and builds against Loom's own Commission
//! (`story:import-governor`, Atlas ADR 0090).
//!
//! Three checks, each reading the tree at run time from `CARGO_MANIFEST_DIR`:
//!
//! 1. no `Cargo.toml` in the workspace names `github.com/beyond10x/commission`, and
//!    `crates/loom-governor/Cargo.toml` takes `b10x-loom-commission` and `b10x-loom-commission-testkit` by path;
//! 2. `Cargo.lock` resolves exactly one `b10x-loom-commission`, and no package from the Commission git
//!    repository;
//! 3. `cargo metadata --format-version 1 --no-deps` on the root manifest lists `b10x-loom-governor` as a
//!    workspace member whose manifest is `crates/loom-governor/Cargo.toml`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const COMMISSION_GIT: &str = "github.com/beyond10x/commission";

const GOVERNOR_MANIFEST: &str = "crates/loom-governor/Cargo.toml";

/// Each Commission crate the governor depends on, with the path it must be taken from.
const GOVERNOR_COMMISSION_DEPS: [(&str, &str); 2] = [
    ("b10x-loom-commission", "../loom-commission"),
    ("b10x-loom-commission-testkit", "../loom-commission-testkit"),
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

/// The value after `=` of the first line in `manifest` whose key is `name`, trimmed.
fn dependency_line<'a>(manifest: &'a str, name: &str) -> Option<&'a str> {
    manifest.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == name).then(|| value.trim())
    })
}

#[test]
fn governor_takes_commission_by_path_and_no_manifest_names_its_git_repository() {
    let root = repo_root();
    let governor = root.join(GOVERNOR_MANIFEST);
    assert!(governor.is_file(), "{GOVERNOR_MANIFEST} is not in the tree");

    let mut manifests = Vec::new();
    manifests_under(&root, &mut manifests);
    assert!(
        manifests.iter().any(|m| m == &root.join("Cargo.toml")),
        "the walk did not reach the root Cargo.toml"
    );
    assert!(
        manifests.iter().any(|m| m == &governor),
        "the walk did not reach {GOVERNOR_MANIFEST}"
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

    let text = fs::read_to_string(&governor).expect("read the governor's Cargo.toml");
    for (name, path) in GOVERNOR_COMMISSION_DEPS {
        let wanted = format!("path = \"{path}\"");
        match dependency_line(&text, name) {
            Some(value) if value.contains(&wanted) => {}
            Some(value) => offenders.push(format!(
                "{GOVERNOR_MANIFEST}: {name} = {value} (wanted {wanted})"
            )),
            None => offenders.push(format!("{GOVERNOR_MANIFEST}: no {name} dependency")),
        }
    }

    assert!(
        offenders.is_empty(),
        "the governor does not build against Loom's own Commission:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn lock_resolves_one_commission_and_nothing_from_its_git_repository() {
    let root = repo_root();
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");

    let mut commissions = Vec::new();
    let mut git_packages = Vec::new();
    for block in lock.split("[[package]]").skip(1) {
        let field = |key: &str| {
            block.lines().find_map(|line| {
                let (k, v) = line.split_once('=')?;
                (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
            })
        };
        let name = field("name").unwrap_or_default();
        let source = field("source");
        if name == "b10x-loom-commission" {
            commissions.push(source.clone().unwrap_or_else(|| "path".to_string()));
        }
        if let Some(source) = source
            && source.contains(COMMISSION_GIT)
        {
            git_packages.push(format!("{name} from {source}"));
        }
    }

    assert_eq!(
        commissions.len(),
        1,
        "Cargo.lock resolves b10x-loom-commission {} times: {commissions:?}",
        commissions.len()
    );
    assert!(
        git_packages.is_empty(),
        "Cargo.lock resolves packages from {COMMISSION_GIT}:\n{}",
        git_packages.join("\n")
    );
}

#[test]
fn cargo_metadata_lists_b10x_governor_as_a_member() {
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
        .find(|p| p["name"] == "b10x-loom-governor")
        .unwrap_or_else(|| {
            let names: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
            panic!("cargo metadata lists no b10x-loom-governor; packages: {names:?}")
        });

    let manifest_path = PathBuf::from(
        package["manifest_path"]
            .as_str()
            .expect("package has a manifest_path"),
    );
    let expected = root.join(GOVERNOR_MANIFEST);
    assert_eq!(
        fs::canonicalize(&manifest_path).expect("canonicalize manifest_path"),
        fs::canonicalize(&expected).expect("canonicalize crates/loom-governor/Cargo.toml"),
        "b10x-loom-governor is not built from crates/loom-governor"
    );

    let id = package["id"].as_str().expect("package has an id");
    let members = metadata["workspace_members"]
        .as_array()
        .expect("cargo metadata has workspace_members");
    assert!(
        members.iter().any(|m| m == id),
        "b10x-loom-governor ({id}) is not a workspace member"
    );
}
