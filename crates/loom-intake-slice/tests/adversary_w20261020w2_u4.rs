//! Adversary cases for `story:sdk-confines-any-child` (wave 2026-10-20-w2, U4).
//!
//! Both cases run against the real backend in a delegated cgroup v2 scope, the way
//! `tests/confined_child.rs` does: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes` and runs the case there. Where bubblewrap, a user
//! systemd manager or the Substrate driver is absent, the case prints
//! `skipped: missing prerequisite: …` and returns.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ExecEnvironment, SubstrateRunner, prepare_delegated_cgroup,
};

const DELEGATED: &str = "LOOM_ADV_W20261020W2_U4_DELEGATED";
const REPORT: &str = "LOOM_ADV_W20261020W2_U4_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";

fn bubblewrap_present() -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(BUBBLEWRAP).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Run `case` against the real backend in a delegated scope, or skip naming what is missing.
fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
    if std::env::var_os(DELEGATED).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        std::fs::write(&report, "started").unwrap();
        let cgroup = match prepare_delegated_cgroup() {
            Ok(cgroup) => cgroup,
            Err(error) => {
                std::fs::write(&report, format!("a delegated cgroup v2 scope ({error})")).unwrap();
                return;
            }
        };
        let runner = SubstrateRunner::new(Some(cgroup));
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        if let Err(error) = runner.preflight(&workspace) {
            std::fs::write(&report, format!("the Substrate driver ({error})")).unwrap();
            return;
        }
        std::fs::write(&report, "ran").unwrap();
        case(&runner, &workspace);
        return;
    }
    if !bubblewrap_present() {
        eprintln!("skipped: missing prerequisite: executable {BUBBLEWRAP}");
        return;
    }
    let state = tempfile::tempdir().unwrap();
    let report = state.path().join("report");
    let started = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED, "1")
        .env(REPORT, &report)
        .output();
    let output = match started {
        Ok(output) => output,
        Err(error) => {
            eprintln!("skipped: missing prerequisite: systemd-run ({error})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match std::fs::read_to_string(&report).ok().as_deref() {
        None => eprintln!(
            "skipped: missing prerequisite: a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr.trim()
        ),
        Some("started") => panic!(
            "the delegated case stopped before it decided its prerequisites: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            assert!(
                output.status.success(),
                "the delegated case failed: {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                stdout.contains("1 passed"),
                "the delegated binary ran the case: {stdout}{stderr}"
            );
            eprintln!("ran delegated: {name}");
        }
        Some(missing) => eprintln!("skipped: missing prerequisite: {missing}"),
    }
}

/// A host runs one child beside another (the story's Why: a harness child, `git`, a forge command
/// and `aep` beside a test command). Launching the second child must not end the first: each
/// `run_child` call is its own confined run, and the first one's result is its own.
#[test]
fn adv_a_second_child_launch_leaves_a_running_child_running() {
    delegated(
        "adv_a_second_child_launch_leaves_a_running_child_running",
        |runner, workspace| {
            let second = workspace.parent().unwrap().join("second");
            std::fs::create_dir(&second).unwrap();
            let started = workspace.join("target/started");
            let finished = workspace.join("target/finished");
            let first = ChildCommand::new(
                "sh",
                [
                    "-c",
                    "touch target/started; sleep 4; touch target/finished; printf first-done",
                ],
                workspace,
            )
            .with_timeout(Duration::from_secs(60));
            let first_runner = runner.clone();
            let handle = std::thread::spawn(move || first_runner.run_child(&first));
            let deadline = Instant::now() + Duration::from_secs(30);
            while !started.exists() && !handle.is_finished() {
                assert!(
                    Instant::now() < deadline,
                    "the first child started within 30 s"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(
                started.exists(),
                "the first child started before it returned: {:?}",
                handle.join().unwrap()
            );

            let other = runner.run_child(&ChildCommand::new("true", [] as [&str; 0], &second));
            let first = handle.join().unwrap();
            eprintln!("first child: {first:?}");
            eprintln!("second child: {other:?}");

            let first = first.expect(
                "the first child, running when the second was launched, returns its own result",
            );
            assert_eq!(
                (first.exit_code, first.timed_out, first.output.as_str()),
                (Some(0), false, "first-done"),
                "the first child ran to its end"
            );
            assert!(finished.is_file(), "the first child ran to its end");
            assert_eq!(
                other.expect("the second child runs").exit_code,
                Some(0),
                "the second child ran"
            );
        },
    );
}

/// The program a host names reaches the child unchanged and is never read as a shell line
/// (`ChildCommand` rustdoc, CHANGELOG, the stability page). A program that reads as a shell
/// assignment must not set a variable in the child, least of all a credential-like name Substrate
/// refuses when the host names it as a variable.
#[test]
fn adv_a_program_that_reads_as_an_assignment_sets_no_variable() {
    delegated(
        "adv_a_program_that_reads_as_an_assignment_sets_no_variable",
        |runner, workspace| {
            let as_variable = ExecEnvironment {
                allow: Vec::new(),
                set: BTreeMap::from([(
                    "GH_TOKEN".to_owned(),
                    "loom-adversary-synthetic".to_owned(),
                )]),
            };
            let refused = runner.run_child(
                &ChildCommand::new("/usr/bin/env", [] as [&str; 0], workspace)
                    .with_environment(as_variable),
            );
            assert!(
                refused.is_err(),
                "Substrate refuses GH_TOKEN as a variable the host names: {refused:?}"
            );

            let child = ChildCommand::new(
                "GH_TOKEN=loom-adversary-synthetic",
                ["/usr/bin/env"],
                workspace,
            );
            assert_eq!(child.program(), "GH_TOKEN=loom-adversary-synthetic");
            match runner.run_child(&child) {
                Err(error) => eprintln!("refused: {error}"),
                Ok(result) => assert!(
                    !result.output.contains("GH_TOKEN"),
                    "the program `GH_TOKEN=loom-adversary-synthetic` ran `/usr/bin/env` with \
                     GH_TOKEN set: exit {:?}, output {:?}",
                    result.exit_code,
                    result.output
                ),
            }
        },
    );
}
