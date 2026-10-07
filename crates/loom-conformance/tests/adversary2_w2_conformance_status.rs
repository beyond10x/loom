//! Adversary pass 2, wave 2026-10-07-w2: named unsupported scenarios keep `task conform` green
//! while ESS rates the run `failed`.
//!
//! The Rust producer reports a scenario the target cannot answer `unsupported`, and ESS's own rule
//! (ess-conformance `counts.rs`, `execution`) makes any `unsupported` scenario an
//! `execution_status: failed` and so a `conformance_status: failed`. `tests/conform.rs` accepts an
//! `unsupported` scenario named in `ess/SKIPPED.md`, and reads the report's two statuses only to
//! check they agree with its counts, so a report whose every unsupported scenario is named passes
//! the check with both statuses `failed`. That behaviour was decided to stay (correction round 2
//! of the wave): acceptance item 4 admits a named scenario, and the `conform` task description and
//! the `ess/SKIPPED.md` header say so. This case holds the decided behaviour.
//!
//! The case builds that report the documented way: a copy of `ess/` whose `ReleaseSession` is sent
//! by an actor with an attribute (so every step invoking it carries a caller, which `LoomTarget`
//! answers `unsupported`), and an `ess/SKIPPED.md` naming each such scenario with a reason, as its
//! header says to for "a scenario Loom cannot answer". It checks that ESS's report rates that run
//! `failed`, then runs the real `conform` test binary against the copy, by pointing
//! `CARGO_MANIFEST_DIR` (read at run time, `conform.rs` `root()`) at it, and requires the check to
//! pass.

use b10x_loom_conformance::run_suite;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// The actor block inserted ahead of `commands:` in the copy's `ess/domains/run.yaml`.
const ACTOR: &str = "actors:\n  - name: loom.run.Releaser\n    attributes:\n      - {name: releaser, type: String}\n    may: [loom.run.ReleaseSession]\n\n";

fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
    for entry in
        fs::read_dir(from).unwrap_or_else(|error| panic!("read {}: {error}", from.display()))
    {
        let entry = entry.expect("a directory entry");
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", path.display()));
        }
    }
}

/// The `conform` integration-test binary cargo built beside this one, the newest if several.
fn conform_binary() -> PathBuf {
    let exe = std::env::current_exe().expect("this test's own path");
    let deps = exe.parent().expect("the deps directory");
    fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_prefix("conform-").is_some_and(|hash| {
                !hash.contains('.') && hash.chars().all(|c| c.is_ascii_hexdigit())
            })
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max()
        .map(|(_, path)| path)
        .unwrap_or_else(|| {
            panic!(
                "no `conform-<hash>` test binary in {}: build it first with \
                 `cargo test --locked -p b10x-loom-conformance --no-run`",
                deps.display()
            )
        })
}

#[test]
fn adversary2_w2_named_unsupported_scenarios_keep_task_conform_green_while_ess_rates_the_run_failed()
 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let fake = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary2-status-{}-{nanos}", std::process::id()));
    fs::create_dir_all(fake.join("crates/loom-conformance")).expect("create the copy's crate dir");
    copy_tree(&root().join("ess"), &fake.join("ess"));

    let run_yaml = fake.join("ess/domains/run.yaml");
    let spec = fs::read_to_string(&run_yaml).expect("read the copy's run.yaml");
    let at = spec
        .find("\ncommands:\n")
        .expect("run.yaml has a top-level `commands:` line");
    fs::write(
        &run_yaml,
        format!("{}\n{ACTOR}{}", &spec[..at], &spec[at + 1..]),
    )
    .expect("write the copy's run.yaml");

    let suite_path = fake.join("suite.json");
    let synthesized = Command::new("ess")
        .current_dir(&fake)
        .args(["verify", "conform", "synthesize", "--path", "ess", "--out"])
        .arg(&suite_path)
        .output()
        .expect("`ess` on PATH");
    assert!(
        synthesized.status.success(),
        "synthesize the copy: {}",
        String::from_utf8_lossy(&synthesized.stderr)
    );
    let suite = fs::read_to_string(&suite_path).expect("read the copy's suite");
    let report: Value = serde_json::from_str(
        &run_suite(&suite)
            .unwrap_or_else(|error| panic!("the copy's suite did not run: {error}"))
            .report,
    )
    .expect("the report is JSON");

    // The state the case is about: nothing failed or errored, some scenarios are unsupported, and
    // ESS itself rates the run failed.
    let unsupported: Vec<String> = report["outcomes"]["unsupported"]
        .as_array()
        .expect("outcomes.unsupported")
        .iter()
        .map(|id| id.as_str().expect("an id").to_owned())
        .collect();
    assert_eq!(report["counts"]["failed"], 0, "report: {report}");
    assert_eq!(report["counts"]["error"], 0, "report: {report}");
    assert!(!unsupported.is_empty(), "report: {report}");
    assert_eq!(report["execution_status"], "failed", "report: {report}");
    assert_eq!(report["conformance_status"], "failed", "report: {report}");

    let mut skipped = String::from(
        "# Skipped conformance scenarios\n\nEach line below names one conformance scenario this \
         repository skips, and why.\n\n",
    );
    for id in &unsupported {
        skipped.push_str(&format!(
            "- {id}: Loom holds no credential to send loom.run.ReleaseSession as a caller\n"
        ));
    }
    fs::write(fake.join("ess/SKIPPED.md"), skipped).expect("write the copy's SKIPPED.md");

    let ran = Command::new(conform_binary())
        .args(["--exact", "ess_conformance_report", "--test-threads=1"])
        .env("CARGO_MANIFEST_DIR", fake.join("crates/loom-conformance"))
        .output()
        .expect("run the conform test binary");
    let stdout = String::from_utf8_lossy(&ran.stdout).into_owned();
    assert!(
        stdout.contains("running 1 test"),
        "the filter selected no test:\n{stdout}"
    );
    let _ = fs::remove_dir_all(&fake);
    assert!(
        ran.status.success(),
        "task conform's check failed a report whose {} unsupported scenario(s) are all named in \
         ess/SKIPPED.md: a named scenario keeps it green while ESS rates the run `failed` (decided \
         behaviour): {unsupported:?}\n{stdout}{}",
        unsupported.len(),
        String::from_utf8_lossy(&ran.stderr)
    );
}
