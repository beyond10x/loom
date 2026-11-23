//! A refusal of `b10x-loom tree test` under its default confinement, `substrate`, ends with exit 2
//! before any cycle, writes nothing, and names a remedy `tree test` accepts (`--confinement local`,
//! `tree test --cgroup-root`), never a flag it rejects or another command's behaviour. The two
//! refusals here need no delegation: a `--cgroup-root` that is no cgroup, and a start again with
//! no `systemd-run` on `PATH`.
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
/// `extra` after the common flags; with `PATH` at an empty directory when `no_programs`.
fn tree_test(dir: &Path, no_programs: bool, extra: &[&str]) -> Output {
    let mut command = command(&dir.join("home"));
    if no_programs {
        std::fs::create_dir_all(dir.join("no-programs")).unwrap();
        command.env("PATH", dir.join("no-programs"));
    }
    command
        .args([
            "tree",
            "test",
            "--at",
            AT,
            "--suite",
            SUITE,
            "--protocols",
            PROTOCOLS,
        ])
        .arg("--scratch")
        .arg(dir.join("scratch"))
        .args(extra)
        .output()
        .unwrap()
}

/// A fresh directory holding an empty `home/` and an empty directory `plain/`, which is no cgroup.
fn fresh(label: &str) -> PathBuf {
    let dir = fresh_scratch(label);
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::create_dir_all(dir.join("plain")).unwrap();
    dir
}

/// The refusals of a hold and of a start again, under the default confinement, in `dir`.
fn refusals(dir: &Path, extra: &[&str]) -> [(&'static str, Output); 2] {
    let plain = dir.join("plain");
    let mut held: Vec<&str> = vec!["--cgroup-root", plain.to_str().unwrap()];
    held.extend_from_slice(extra);
    [
        ("refused hold", tree_test(dir, false, &held)),
        ("failed start again", tree_test(dir, true, extra)),
    ]
}

/// Each refusal is exit 2 before any cycle, and the remedy it prints is one `tree test`
/// accepts: following it must not end in clap rejecting the flag it advised.
#[test]
fn the_substrate_refusal_names_a_remedy_tree_test_accepts() {
    let started = Instant::now();
    let dir = fresh("substrate-remedy");

    for (what, refused) in refusals(&dir, &[]) {
        let said = String::from_utf8_lossy(&refused.stderr).into_owned();
        assert_eq!(refused.status.code(), Some(2), "{what}: {said}");
        assert!(said.contains("ConfinementRefused"), "{what}: {said}");
        assert!(said.contains("--confinement local"), "{what}: {said}");
        assert!(said.contains("tree test --cgroup-root"), "{what}: {said}");
        assert!(
            refused.stdout.is_empty(),
            "{what}: a refusal printed a cycle"
        );
        assert!(
            !dir.join("scratch").exists(),
            "{what}: the refused run made --scratch"
        );
        assert!(
            !said.contains("tree run"),
            "{what}: the `tree test` refusal describes `tree run`, another command: {said}"
        );
    }

    // The flag the refusal advises, given to `tree test`, as a user would follow it.
    let followed = tree_test(&dir, true, &["--cgroup-root", "/sys/fs/cgroup"]);
    let rejected = String::from_utf8_lossy(&followed.stderr).into_owned();
    assert!(
        !rejected.contains("unexpected argument '--cgroup-root'"),
        "`tree test` rejects the `--cgroup-root` its refusal advises: {rejected}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    within_bound(started);
}

/// A refused `tree test --confinement substrate` (the default) writes nothing: the scratch
/// directory stays absent, no report is written and nothing is written into `HOME`.
#[test]
fn the_substrate_refusal_writes_nothing() {
    let started = Instant::now();
    let dir = fresh("substrate-nothing");
    let report = dir.join("report.json");
    for (what, output) in refusals(&dir, &["--report", report.to_str().unwrap()]) {
        let said = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{what}: {said}");
        assert!(said.contains("ConfinementRefused"), "{what}: {said}");
        assert!(
            !dir.join("scratch").exists(),
            "{what}: the scratch directory was made: {said}"
        );
        assert!(!report.exists(), "{what}: a report was written: {said}");
        assert!(
            dir.join("home").read_dir().unwrap().next().is_none(),
            "{what}: something was written into HOME"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
    within_bound(started);
}
