//! Adversary pass 2 for `tests/ess_gate.rs` (story:ess-hard-gate, Atlas ADR 0076).
//!
//! Each case builds a std-only replica of the repository under `CARGO_TARGET_TMPDIR` (the root
//! `Cargo.toml`, `AGENTS.md`, `Taskfile.yml`, `ess/` and `crates/loom-executor/tests/ess_gate.rs` copied
//! verbatim, an empty library in place of `crates/loom-executor/src`), applies one mutant to the replica,
//! and runs the real gate suite there with `cargo test --test ess_gate`. A mutant of the gate code
//! is applied to the replica's copy of `ess_gate.rs`; the file under review is never edited.
//!
//! The repository root is read from `CARGO_MANIFEST_DIR` at run time, and the scratch directory is
//! keyed by that root, so a binary another worktree built into a shared build directory still
//! reads and writes for the tree it runs in.

use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A command whose `never` outcome no input satisfies. ess 0.54.0 validates and compiles it, and
/// `ess verify conform synthesize` refuses the outcome (`ESS-SYNTH-003`) and still exits 0, so the
/// refusal count on the summary line is the only signal that step 3 does not hold. Observed
/// 2026-10-04 on a copy of `ess/`: `1 scenario(s) (0 authored), 1 refusal(s), written to …`, exit 0.
/// It is a file of its own in domain `loom.run`, so it does not depend on which sections
/// `run.yaml` already declares.
const UNSATISFIABLE_COMMAND: &str = "domain: loom.run

commands:
  - name: loom.run.Probe
    naming:
      wire: probe
      display: Probe
    input:
      - name: strategy
        type: loom.run.SelectionStrategy
    outcomes:
      - name: never
        when:
          all:
            - strategy == Rule
            - strategy == Hybrid
        emits: [loom.run.Probed]
        payload:
          loom.run.Probed:
            strategy: input.strategy
      - name: always
        emits: [loom.run.Probed]
        payload:
          loom.run.Probed:
            strategy: input.strategy

events:
  - name: loom.run.Probed
    naming:
      wire: probed
      display: Probed
    fields:
      - name: strategy
        type: loom.run.SelectionStrategy
";

fn repo_root() -> PathBuf {
    PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

/// A fresh directory under `CARGO_TARGET_TMPDIR`, unique to the calling case and to the tree.
fn scratch(name: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    repo_root().hash(&mut hasher);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary2_ess_gate")
        .join(format!("{:016x}", hasher.finish()))
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    fs::create_dir_all(&dir).expect("create scratch");
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy");
    for entry in fs::read_dir(from).expect("read source") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// A std-only replica of the repository at `at`, in which the real `ess_gate.rs` runs.
fn replica(at: &Path) {
    let root = repo_root();
    fs::create_dir_all(at.join("crates/loom-executor/src")).expect("create replica src");
    fs::create_dir_all(at.join("crates/loom-executor/tests")).expect("create replica tests");
    for file in ["Cargo.toml", "AGENTS.md", "Taskfile.yml"] {
        fs::copy(root.join(file), at.join(file)).expect("copy root file");
    }
    copy_dir(&root.join("ess"), &at.join("ess"));
    fs::copy(
        root.join("crates/loom-executor/tests/ess_gate.rs"),
        at.join("crates/loom-executor/tests/ess_gate.rs"),
    )
    .expect("copy ess_gate.rs");
    fs::write(
        at.join("crates/loom-executor/Cargo.toml"),
        "[package]\nname = \"b10x-loom-executor\"\nversion.workspace = true\nedition.workspace = true\n\
         license.workspace = true\nrepository.workspace = true\n",
    )
    .expect("write replica manifest");
    fs::write(at.join("crates/loom-executor/src/lib.rs"), "").expect("write replica lib");
}

fn gate_suite(tree: &Path, target: &Path) -> Output {
    Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .args(["test", "--offline", "--test", "ess_gate"])
        .current_dir(tree)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .output()
        .expect("run cargo test --test ess_gate")
}

fn text(output: &Output) -> String {
    format!(
        "{}\n--- stdout\n{}\n--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn replace(file: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(file).expect("read file to mutate");
    assert!(
        text.contains(from),
        "mutant does not apply: {} lacks {from:?}",
        file.display()
    );
    fs::write(file, text.replacen(from, to, 1)).expect("write mutant");
}

/// Adds `UNSATISFIABLE_COMMAND` to the replica's specification as `domains/probe.yaml`.
fn add_unsatisfiable_command(tree: &Path) {
    fs::write(tree.join("ess/domains/probe.yaml"), UNSATISFIABLE_COMMAND)
        .expect("write probe.yaml");
    replace(
        &tree.join("ess/ess-inputs.yaml"),
        "  - domains/run.yaml\n",
        "  - domains/run.yaml\n  - domains/probe.yaml\n",
    );
}

/// Builds a replica, applies `mutate`, runs the gate suite in it with its own build directory, and
/// asserts the suite fails with every string in `expect` in its output.
fn mutant_is_killed(name: &str, mutate: impl FnOnce(&Path), expect: &[&str]) {
    let dir = scratch(name);
    let tree = dir.join("tree");
    replica(&tree);
    mutate(&tree);
    let output = gate_suite(&tree, &dir.join("target"));
    let report = text(&output);
    assert!(
        !output.status.success(),
        "mutant `{name}` survives: the ESS gate suite passed\n{report}"
    );
    for needle in expect {
        assert!(
            report.contains(needle),
            "mutant `{name}`: the suite failed, but not naming {needle:?}\n{report}"
        );
    }
}

// Gate code mutant, step 3. ess 0.54.0 exits 0 while refusing (see UNSATISFIABLE_COMMAND), so the
// refusal count is what holds step 3 for every refusal that is not an admission error. No case in
// ess_gate.rs runs the gate over a specification that synthesize refuses with exit 0: the only
// refusing copy (Optional<Binary64>) exits 1. A gate that accepts any count passes the suite.
#[test]
fn mutant_gate_accepting_any_refusal_count_is_killed() {
    mutant_is_killed(
        "accept_any_refusal_count",
        |tree| {
            replace(
                &tree.join("crates/loom-executor/tests/ess_gate.rs"),
                "        Some(0) => {}\n",
                "        Some(_) => {}\n",
            );
        },
        &["refusal(s)"],
    );
}

// Gate code mutant, step 1. Acceptance item 1 names `--strict-requires`. Nothing in ess_gate.rs runs
// the gate over a specification whose `requires` differs from the ess on PATH, so dropping the flag
// passes the suite: without it ess 0.54.0 warns about `requires: ess 0.51.0` and exits 0.
#[test]
fn mutant_validate_without_strict_requires_is_killed() {
    mutant_is_killed(
        "validate_without_strict_requires",
        |tree| {
            replace(
                &tree.join("crates/loom-executor/tests/ess_gate.rs"),
                "        path_arg(dir),\n        \"--strict-requires\",\n    ]);",
                "        path_arg(dir),\n    ]);",
            );
        },
        &["--strict-requires"],
    );
}

// Specification mutant, step 3: the unchanged gate holds a specification synthesize refuses with
// exit 0. Pins the behaviour the code mutant above removes.
#[test]
fn mutant_spec_refused_by_synthesize_with_exit_0_is_killed() {
    mutant_is_killed(
        "spec_refused_exit_0",
        add_unsatisfiable_command,
        &[
            "gate_holds_on_the_specification ... FAILED",
            "1 refusal(s)",
            "ESS-SYNTH-003",
        ],
    );
}

// Specification mutant, step 1: the unchanged gate holds a `requires` older than the ess on PATH.
#[test]
fn mutant_spec_requiring_an_older_ess_is_killed() {
    mutant_is_killed(
        "spec_requires_older_ess",
        |tree| {
            replace(
                &tree.join("ess/ess-inputs.yaml"),
                "requires: ess 0.54.0",
                "requires: ess 0.51.0",
            );
        },
        &[
            "gate_holds_on_the_specification ... FAILED",
            "--strict-requires refuses",
        ],
    );
}
