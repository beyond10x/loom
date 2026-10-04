//! Adversary pass 2 (wave 2026-10-04-w2, unit commission/ess-hard-gate).
//!
//! The gate's functions are private to `ess_gate.rs`, so this binary compiles that file as a module
//! and drives its real `ess_gate` test end to end: it re-runs itself, selecting only
//! `ess_gate_src::ess_gate`, with `CARGO_MANIFEST_DIR` pointing at a scratch repository whose
//! `ess/` is a mutated copy of this one. `ess_gate.rs` reads that variable when it runs, so the
//! child checks the copy, with the code as it stands in this tree.
//!
//! A side effect: the cases of `ess_gate.rs` also run once more inside this binary.

#[path = "ess_gate.rs"]
mod ess_gate_src;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo");
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(purpose: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary2-{purpose}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
        Self { dir }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display())) {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}

/// A scratch repository: `<dir>/ess` (a copy of this repository's `ess/`) and an empty
/// `<dir>/crates/commission`, so `CARGO_MANIFEST_DIR/../..` resolves to `<dir>`.
fn scratch_repository(purpose: &str) -> Scratch {
    let scratch = Scratch::new(purpose);
    copy_tree(&repository().join("ess"), &scratch.dir.join("ess"));
    fs::create_dir_all(scratch.dir.join("crates/commission")).unwrap();
    scratch
}

/// Runs this tree's real `ess_gate` test against the scratch repository `repo`.
fn run_gate_on(repo: &Path) -> (Output, String) {
    let exe = std::env::current_exe().expect("current test binary");
    let output = Command::new(exe)
        .args([
            "ess_gate_src::ess_gate",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CARGO_MANIFEST_DIR", repo.join("crates/commission"))
        .output()
        .expect("re-run this test binary");
    let both = format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        both.contains("running 1 test") && both.contains("ess_gate_src::ess_gate"),
        "the child did not select exactly the ess_gate case:\n{both}"
    );
    (output, both)
}

const ISSUING_COMMAND: &str = "  - name: commission.responsibility.IssueFrontier
    input:
      - name: case_id
        type: commission.responsibility.CaseId
      - name: case_revision
        type: Integer
    outcomes:
      - name: issued
        creates: commission.responsibility.Frontier
        instance: frontier_id
        emits: [commission.responsibility.FrontierIssued]
        payload:
          commission.responsibility.FrontierIssued:
            frontier_id: {generated: true}
        sets:
          case_id: input.case_id
          case_revision: input.case_revision
";

const ISSUED_EVENT: &str = "  - name: commission.responsibility.FrontierIssued
    fields:
      - name: frontier_id
        type: commission.responsibility.FrontierId
";

/// `text` with `item` added to its top-level `section` list, opening the section at the end when
/// the domain has none. A second top-level key of the same name would not be YAML ess accepts.
fn add_to_section(text: &str, section: &str, item: &str) -> String {
    let header = format!("\n{section}:\n");
    match text.matches(&header).count() {
        0 => format!("{text}{header}{item}"),
        1 => text.replacen(&header, &format!("{header}{item}"), 1),
        n => panic!("the domain opens `{section}:` {n} times"),
    }
}

/// An invariant ESS cannot witness (no view publishes `case_id`), whose own text — which ess 0.52.0
/// quotes verbatim on the `refused:` detail line, before its summary — reads `0 refusal(s)`.
const INVARIANT_QUOTING_A_ZERO_COUNT: &str =
    "    invariants:\n      - 'case_id != \"see 0 refusal(s) here\"'\n";

/// Expectation 3 / ADR 0076 item 3: synthesis "with 0 refusals". The specification below makes
/// ess 0.52.0 report `1 refusal(s)` on its summary line and exit 0; the refused detail line, printed
/// first, quotes the author's invariant, which says `0 refusal(s)`. The gate must fail at step 3.
#[test]
fn adversary2_gate_reads_the_summary_count_not_a_quoted_one() {
    let scratch = scratch_repository("quoted-count");
    let domain = scratch.dir.join("ess/domains/responsibility.yaml");
    let text = fs::read_to_string(&domain).unwrap();
    let lifecycle_end = "      terminal: [Issued]\n";
    assert_eq!(text.matches(lifecycle_end).count(), 1, "Frontier lifecycle");
    let text = text.replacen(
        lifecycle_end,
        &format!("{lifecycle_end}{INVARIANT_QUOTING_A_ZERO_COUNT}"),
        1,
    );
    let text = add_to_section(&text, "commands", ISSUING_COMMAND);
    let text = add_to_section(&text, "events", ISSUED_EVENT);
    fs::write(&domain, &text).unwrap();

    // Precondition, observed from ess itself: one refusal, exit 0.
    let suite = scratch.dir.join("probe-suite.json");
    let probe = Command::new("ess")
        .args(["verify", "conform", "synthesize", "--path"])
        .arg(scratch.dir.join("ess"))
        .arg("--out")
        .arg(&suite)
        .output()
        .expect("ess on PATH");
    let report = String::from_utf8_lossy(&probe.stdout);
    assert!(
        probe.status.success()
            && report.contains("refused: refusal[ESS-SYNTH-011]")
            && report.contains("see 0 refusal(s) here")
            && report
                .lines()
                .rfind(|line| line.contains(" scenario(s) "))
                .is_some_and(|summary| summary.contains(" (0 authored), 1 refusal(s), written to")),
        "precondition: ess did not refuse one scenario with exit 0:\n{report}"
    );

    let (output, both) = run_gate_on(&scratch.dir);
    assert!(
        !output.status.success() && both.contains("step 3 (synthesize)"),
        "the gate passed a specification ess refused one scenario of:\n{both}"
    );
}

/// ADR 0076 item 1 / expectation 1: `--strict-requires`. A specification that requires an older
/// ess passes `validate` without the flag (a warning, exit 0) and every later step, so only the flag
/// fails it. Kills the mutant that drops `--strict-requires` from step 1, which the suite misses.
#[test]
fn adversary2_gate_refuses_an_older_required_ess() {
    let scratch = scratch_repository("older-requires");
    let inputs = scratch.dir.join("ess/ess-inputs.yaml");
    let text = fs::read_to_string(&inputs).unwrap();
    assert_eq!(
        text.matches("requires: ess 0.52.0").count(),
        1,
        "ess-inputs"
    );
    fs::write(
        &inputs,
        text.replace("requires: ess 0.52.0", "requires: ess 0.51.0"),
    )
    .unwrap();

    let (output, both) = run_gate_on(&scratch.dir);
    assert!(
        !output.status.success()
            && both.contains("step 1 (validate)")
            && both.contains("--strict-requires refuses a newer release"),
        "the gate passed a specification that requires an older ess:\n{both}"
    );
}
