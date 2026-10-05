//! Acceptance for `story:loom-sdk`: the SDK's example opens a case on `software-change@1`, runs
//! Commission's loop over fake models and stops at `ApprovalRequired (repository.merge)`,
//! depending on `b10x-loom-sdk` alone.
//!
//! The example's logic is compiled into this test from `examples/software_change.rs`, so the run
//! asserted here is the run `cargo run --example software_change` makes. Its scratch repository
//! is under `CARGO_TARGET_TMPDIR`. No case reaches a model or the network.
//!
//! "Alone" is checked on the example's source, read at run time: no path in it starts at a crate
//! the workspace's crates depend on, or at one of those crates, except through `loom_sdk`.

#[allow(dead_code)]
#[path = "../examples/software_change.rs"]
mod example;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use loom_sdk::commission::model::responsibility::{
    EffectOutcome, RevalidateActionRequestOutcome, RunOutcome,
};

#[test]
fn the_example_stops_at_the_merge_approval() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let workspace = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loom-sdk-example-{}-{nanos}", std::process::id()));

    let end = example::run(&workspace).expect("the example runs until the loop stops");

    assert_eq!(
        example::stop_reason(&end),
        "ApprovalRequired (repository.merge)",
        "{end:#?}"
    );
    assert!(
        matches!(end.outcome, RunOutcome::NeedsAuthority(_)),
        "the run needs authority: {:?}",
        end.outcome
    );
    let needing: Vec<&str> = end
        .requests
        .iter()
        .filter_map(|made| match &made.outcome {
            RevalidateActionRequestOutcome::NeedsAuthority { error } => Some(error.action.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(needing, ["repository.merge"], "{end:#?}");

    let admitted: Vec<&str> = end
        .admitted
        .iter()
        .map(|request| request.action.as_str())
        .collect();
    assert_eq!(
        admitted,
        ["repository.edit", "tests.run"],
        "the edit and the test run are admitted, the merge is not: {end:#?}"
    );
    assert_eq!(end.effects.len(), 2, "{end:#?}");
    assert!(
        end.effects
            .iter()
            .all(|effect| matches!(effect, EffectOutcome::Performed(_))),
        "both admitted requests were performed: {:?}",
        end.effects
    );
    assert_eq!(end.steps, 2, "{end:#?}");

    // The edit was committed in the scratch repository, and nothing was merged or branched.
    assert_eq!(git(&workspace, &["show", "HEAD:check.txt"]), "fixed\n");
    assert_eq!(
        git(&workspace, &["rev-list", "--count", "HEAD"]).trim(),
        "2"
    );
    assert_eq!(git(&workspace, &["branch", "--list"]).trim(), "* main");

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn the_example_depends_on_the_sdk_alone() {
    let manifest = manifest_dir();
    let source = read(&manifest.join("examples/software_change.rs"));
    let crates = workspace_crate_names(&manifest.join("../.."));
    assert!(
        crates.contains("b10x_loom_commission") && crates.contains("loom_governor"),
        "the workspace's crate names were read: {crates:?}"
    );

    let roots = path_roots(&source);
    assert!(
        roots.uses.contains("loom_sdk"),
        "the example imports the SDK: {:?}",
        roots.uses
    );
    let foreign_uses: Vec<&String> = roots
        .uses
        .iter()
        .filter(|root| !["std", "core", "alloc", "loom_sdk"].contains(&root.as_str()))
        .collect();
    assert!(
        foreign_uses.is_empty(),
        "the example imports only from std and loom_sdk: {foreign_uses:?}"
    );
    let foreign_paths: Vec<&String> = roots
        .paths
        .iter()
        .filter(|root| crates.contains(*root))
        .collect();
    assert!(
        foreign_paths.is_empty(),
        "no path in the example starts at another crate: {foreign_paths:?}"
    );
}

#[test]
fn the_dependency_check_catches_a_direct_import() {
    // The check above, on two mutants of the example: a direct import and a direct path.
    let manifest = manifest_dir();
    let source = read(&manifest.join("examples/software_change.rs"));
    let crates = workspace_crate_names(&manifest.join("../.."));

    let imported = format!("{source}\nuse b10x_loom_commission::runtime::run_until_blocked;\n");
    assert!(
        path_roots(&imported).uses.contains("b10x_loom_commission"),
        "a direct import is seen"
    );

    let pathed =
        format!("{source}\nfn direct() {{ let _ = loom_governor::MemoryCaseStore::default(); }}\n");
    let roots = path_roots(&pathed);
    assert!(
        roots.paths.contains("loom_governor") && crates.contains("loom_governor"),
        "a direct path is seen: {:?}",
        roots.paths
    );
    assert!(
        !path_roots(&source).paths.contains("loom_governor"),
        "`loom_sdk::governor` is not a direct path"
    );
}

/// The roots of the paths in a Rust source: the first segment of every `use` declaration, and the
/// first segment of every other path (`a::b` where `a` follows no `::`).
struct Roots {
    uses: BTreeSet<String>,
    paths: BTreeSet<String>,
}

fn path_roots(source: &str) -> Roots {
    let code: String = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");

    let mut uses = BTreeSet::new();
    let mut rest = String::new();
    let mut remaining = code.as_str();
    while let Some(at) = find_word(remaining, "use") {
        rest.push_str(&remaining[..at]);
        let after = &remaining[at + "use".len()..];
        let end = after.find(';').map_or(after.len(), |end| end + 1);
        let declaration = after[..end].trim_start().trim_start_matches("::");
        let root: String = declaration
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !root.is_empty() {
            uses.insert(root);
        }
        remaining = &after[end..];
    }
    rest.push_str(remaining);

    let mut paths = BTreeSet::new();
    let bytes = rest.as_bytes();
    let mut start = None;
    for (index, &byte) in bytes.iter().enumerate() {
        let ident = byte.is_ascii_alphanumeric() || byte == b'_';
        match (ident, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                let follows_path = from >= 1 && bytes[from - 1] == b':';
                if !follows_path && rest[index..].starts_with("::") {
                    paths.insert(rest[from..index].to_owned());
                }
                start = None;
            }
            _ => {}
        }
    }
    Roots { uses, paths }
}

/// Where `word` starts in `text` as a whole word followed by whitespace.
fn find_word(text: &str, word: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = text[from..].find(word) {
        let at = from + found;
        let before = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let after = bytes
            .get(at + word.len())
            .is_some_and(u8::is_ascii_whitespace);
        if before && after {
            return Some(at);
        }
        from = at + word.len();
    }
    None
}

/// The crate names Rust code in the workspace can name: every member's library, and every
/// dependency its manifest declares, each with `-` as `_`. `loom_sdk` is not among them.
fn workspace_crate_names(root: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let members = std::fs::read_dir(root.join("crates")).expect("read crates/");
    for member in members {
        let manifest = member.expect("a crates/ entry").path().join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let mut section = String::new();
        for line in read(&manifest).lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line.trim_matches(['[', ']']).to_owned();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if section.ends_with("dependencies") {
                names.insert(key.replace('-', "_"));
            } else if (section == "lib" || section == "package") && key == "name" {
                names.insert(value.replace('-', "_"));
            }
        }
    }
    names.remove("loom_sdk");
    names.remove("b10x_loom_sdk");
    names
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn git(workspace: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}
