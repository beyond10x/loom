//! Adversary cases for `tests/ess_gate.rs` (story:ess-hard-gate, Atlas ADR 0076).
//!
//! Each case builds a std-only replica of the repository under `CARGO_TARGET_TMPDIR` — the root
//! `Cargo.toml`, `AGENTS.md`, `Taskfile.yml`, `ess/` and `crates/loom-executor/tests/ess_gate.rs` copied
//! verbatim, with an empty library in place of `crates/loom-executor/src` — mutates the replica, and runs the
//! real gate suite in it with `cargo test --test ess_gate`. The gate is never edited: what is
//! measured is the exit status of the suite CI runs, over a tree carrying the mutant.
//!
//! The repository root is read from `CARGO_MANIFEST_DIR` at run time, not from `env!`, so these
//! cases read the tree they run in.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PROBE: &str = "# UNMAPPED: probe\n";

fn repo_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

/// A fresh directory under `CARGO_TARGET_TMPDIR`, unique to the calling case.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary_ess_gate")
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

/// A std-only replica of the repository at `at`, in which the real `ess_gate.rs` runs unchanged.
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

/// `cargo test --test ess_gate` in `tree`, building into `target`.
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

fn append(file: &Path, line: &str) {
    let mut text = fs::read_to_string(file).expect("read file to mutate");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(line);
    fs::write(file, text).expect("write mutant");
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

// ADR 0076 / acceptance: `task ess-gate` fails on a tree carrying a marker. Each tree builds into
// its own `target/`, but a `CARGO_TARGET_DIR` set in the environment can still point two
// worktrees' `task check` at one build directory. The gate takes the repository root from
// `env!("CARGO_MANIFEST_DIR")`, which is compiled into the test binary, so when cargo judges
// another worktree's `ess_gate` binary fresh it runs that binary — and the gate checks the other
// worktree's `ess/`, not the one `task check` was run in.
#[test]
fn gate_checks_the_tree_it_runs_in_when_worktrees_share_a_build_directory() {
    let dir = scratch("shared_target");
    let clean = dir.join("worktree-a");
    let marked = dir.join("worktree-b");
    let target = dir.join("target");
    // Both worktrees exist before either builds, as in a wave whose units are created together.
    replica(&clean);
    replica(&marked);
    append(&marked.join("ess/domains/run.yaml"), PROBE);

    let a = gate_suite(&clean, &target);
    assert!(
        a.status.success(),
        "precondition: the gate holds on the clean tree\n{}",
        text(&a)
    );

    let b = gate_suite(&marked, &target);
    assert!(
        !b.status.success(),
        "the ESS gate passed in a worktree whose ess/domains/run.yaml ends with `# UNMAPPED: probe`: \
         it ran the binary built for {} and checked that tree's ess/\n{}",
        clean.display(),
        text(&b)
    );
}

// Marker scan: a nested directory and a file ESS never reads (not listed in ess-inputs.yaml).
#[test]
fn mutant_marker_in_a_nested_non_yaml_file_is_killed() {
    mutant_is_killed(
        "nested_non_yaml",
        |tree| {
            let dir = tree.join("ess/domains/notes");
            fs::create_dir_all(&dir).expect("create nested dir");
            fs::write(dir.join("open.md"), "a note\nUNMAPPED: nested\n").expect("write");
        },
        &[
            "gate_holds_on_the_specification ... FAILED",
            "domains/notes/open.md:2",
        ],
    );
}

// Marker scan: a hidden file, the marker mid-line.
#[test]
fn mutant_marker_mid_line_in_a_hidden_file_is_killed() {
    mutant_is_killed(
        "hidden_mid_line",
        |tree| {
            fs::write(tree.join("ess/.open"), "settled: no # UNMAPPED: hidden\n").expect("write");
        },
        &["gate_holds_on_the_specification ... FAILED", ".open:1"],
    );
}

// Marker scan: the marker mid-line in the specification itself.
#[test]
fn mutant_marker_mid_line_in_run_yaml_is_killed() {
    mutant_is_killed(
        "run_yaml_mid_line",
        |tree| {
            replace(
                &tree.join("ess/domains/run.yaml"),
                "        type: loom.run.CommissionRunId\n",
                "        type: loom.run.CommissionRunId # UNMAPPED: mid-line\n",
            );
        },
        &[
            "gate_holds_on_the_specification ... FAILED",
            "open question marked UNMAPPED: at domains/run.yaml:",
        ],
    );
}

// Item 2/3: confidence typed Binary64 is refused by synthesize and is not the decimal primitive.
// Every confidence is retyped: `SelectAction` copies its input into `Selection.confidence`, and
// retyping one side alone fails validation instead.
#[test]
fn mutant_confidence_binary64_is_killed() {
    mutant_is_killed(
        "confidence_binary64",
        |tree| {
            let run = tree.join("ess/domains/run.yaml");
            let text = fs::read_to_string(&run).expect("read file to mutate");
            assert!(
                text.contains("type: Optional<Decimal>"),
                "mutant does not apply: {} lacks the Decimal confidence",
                run.display()
            );
            fs::write(
                &run,
                text.replace("type: Optional<Decimal>", "type: Optional<Binary64>"),
            )
            .expect("write mutant");
        },
        &[
            "gate_holds_on_the_specification ... FAILED",
            "compiled_specification_declares_the_settled_relations ... FAILED",
            "UnsupportedPrimitive",
        ],
    );
}

// Item 2: the relation Turn.catalogue dropped.
#[test]
fn mutant_relation_dropped_is_killed() {
    mutant_is_killed(
        "relation_dropped",
        |tree| {
            replace(
                &tree.join("ess/domains/run.yaml"),
                "      - name: catalogue\n        kind: owns\n        target: loom.run.ActionCatalogue\n        cardinality: one\n        via: turn_id\n",
                "",
            );
            // A `relations:` key left with only comments is null; drop it with the relation.
            replace(
                &tree.join("ess/domains/run.yaml"),
                "    relations:\n      # A catalogue is projected once per turn",
                "      # A catalogue is projected once per turn",
            );
        },
        &["compiled_specification_declares_the_settled_relations ... FAILED"],
    );
}

// Item 2: the relation kept, its cardinality changed.
#[test]
fn mutant_relation_cardinality_many_is_killed() {
    mutant_is_killed(
        "relation_many",
        |tree| {
            replace(
                &tree.join("ess/domains/run.yaml"),
                "        target: loom.run.ActionCatalogue\n        cardinality: one\n",
                "        target: loom.run.ActionCatalogue\n        cardinality: many\n",
            );
        },
        &["compiled_specification_declares_the_settled_relations ... FAILED"],
    );
}

// Item 2: commission_run back to String, and CommissionRunId of String.
#[test]
fn mutant_commission_run_string_is_killed() {
    mutant_is_killed(
        "commission_run_string",
        |tree| {
            replace(
                &tree.join("ess/domains/run.yaml"),
                "        type: loom.run.CommissionRunId\n",
                "        type: String\n",
            );
        },
        &["compiled_specification_declares_the_settled_relations ... FAILED"],
    );
}

#[test]
fn mutant_commission_run_id_of_string_is_killed() {
    mutant_is_killed(
        "commission_run_id_string",
        |tree| {
            replace(
                &tree.join("ess/domains/run.yaml"),
                "  - name: loom.run.CommissionRunId\n    kind: newtype\n    of: Uuid\n",
                "  - name: loom.run.CommissionRunId\n    kind: newtype\n    of: String\n",
            );
        },
        &["compiled_specification_declares_the_settled_relations ... FAILED"],
    );
}

// Acceptance: `task check` runs `task ess-gate`.
#[test]
fn mutant_check_without_ess_gate_is_killed() {
    mutant_is_killed(
        "check_without_ess_gate",
        |tree| {
            replace(&tree.join("Taskfile.yml"), "      - task: ess-gate\n", "");
        },
        &["taskfile_check_runs_the_ess_gate ... FAILED"],
    );
}
