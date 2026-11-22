//! `b10x-loom tree test` under its default confinement, `substrate`: refused before any cycle with
//! exit 2, nothing written, and the refusal names a remedy `tree test` accepts
//! (`--confinement local`), never a flag it rejects or another command's behaviour.
//!
//! Nothing runs a cycle; each test ends within its 5 s bound.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Instant;

use super::{AT, PROTOCOLS, command, fresh_scratch, within_bound};

const SUITE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/tree-suite/suite.yaml"
);

/// `tree test` of the fixture suite in `dir` (home `dir/home`, scratch `dir/scratch`), with
/// `extra` after the common flags.
fn tree_test(dir: &Path, extra: &[&str]) -> Output {
    command(&dir.join("home"))
        .args(["tree", "test", "--at", AT, "--suite", SUITE, "--protocols", PROTOCOLS])
        .arg("--scratch")
        .arg(dir.join("scratch"))
        .args(extra)
        .output()
        .unwrap()
}

/// A fresh directory holding an empty `home/`.
fn fresh(label: &str) -> PathBuf {
    let dir = fresh_scratch(label);
    std::fs::create_dir_all(dir.join("home")).unwrap();
    dir
}

/// The default `tree test` is refused before any cycle, and the remedy it prints is one `tree
/// test` accepts: following it must not end in clap rejecting the flag it advised.
#[test]
fn the_substrate_refusal_names_a_remedy_tree_test_accepts() {
    let started = Instant::now();
    let dir = fresh("substrate-remedy");

    let refused = tree_test(&dir, &[]);
    let said = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert_eq!(refused.status.code(), Some(2), "{said}");
    assert!(said.contains("ConfinementRefused"), "{said}");
    assert!(said.contains("--confinement local"), "{said}");
    assert!(refused.stdout.is_empty(), "a refusal printed a cycle");
    assert!(!dir.join("scratch").exists(), "the refused run made --scratch");

    // The flag a `tree run` refusal advises, given to `tree test`, as a user would follow it.
    let followed = tree_test(&dir, &["--cgroup-root", "/sys/fs/cgroup"]);
    let rejected = String::from_utf8_lossy(&followed.stderr).into_owned();
    let flag_unknown = rejected.contains("unexpected argument '--cgroup-root'");
    assert!(
        !(said.contains("--cgroup-root") && flag_unknown),
        "the default `tree test` refusal advises `--cgroup-root`, which `tree test` rejects:\n\
         refusal: {said}\nfollowing it: {rejected}"
    );
    assert!(
        !said.contains("tree run"),
        "the `tree test` refusal describes `tree run`, another command: {said}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    within_bound(started);
}

/// `tree test --confinement substrate` (the default) writes nothing: the scratch directory stays
/// absent, no report is written and nothing is written into `HOME`.
#[test]
fn the_substrate_refusal_writes_nothing() {
    let started = Instant::now();
    let dir = fresh("substrate-nothing");
    let report = dir.join("report.json");
    let output = tree_test(&dir, &["--report", report.to_str().unwrap()]);
    let said = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(said.contains("ConfinementRefused"), "{said}");
    assert!(!dir.join("scratch").exists(), "the scratch directory was made: {said}");
    assert!(!report.exists(), "a report was written: {said}");
    assert!(
        dir.join("home").read_dir().unwrap().next().is_none(),
        "something was written into HOME"
    );
    let _ = std::fs::remove_dir_all(&dir);
    within_bound(started);
}
