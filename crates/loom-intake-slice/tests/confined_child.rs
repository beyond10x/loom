//! A child process the host names runs confined through Substrate (`SubstrateRunner::run_child`),
//! with the test runner's guarantees: a missing guarantee is a typed refusal and nothing runs, the
//! child's environment holds only what the host named, and the applied confinement comes back with
//! the result.
//!
//! The delegated cases need bubblewrap and a delegated cgroup v2 scope. Each starts its own test
//! binary again under `systemd-run --user --scope -p Delegate=yes`, as `b10x-loom` starts itself,
//! and runs the case there. Where bubblewrap, a user systemd manager or the Substrate driver is
//! absent, the case prints `skipped: missing prerequisite: …` and returns; such a skip establishes
//! no qualification.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_intake_slice::confinement::{
    Backend, ChildCommand, ConfinementRefusal, ExecEnvironment, NetworkMode, SandboxProfile,
    SubstrateRunner, TestRunner as _, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;

/// Set in the test binary started inside the delegated scope, so it runs the case instead of
/// starting itself again.
const DELEGATED: &str = "LOOM_CONFINED_CHILD_DELEGATED";
/// The file the delegated binary writes: `started`, then `ran` or the missing prerequisite.
const REPORT: &str = "LOOM_CONFINED_CHILD_REPORT";
/// A variable the process that launches the child holds and the host never names.
const UNNAMED: &str = "LOOM_CONFINED_CHILD_UNNAMED";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";

fn bubblewrap_present() -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(BUBBLEWRAP).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The two variables the host names.
fn named() -> ExecEnvironment {
    ExecEnvironment {
        allow: Vec::new(),
        set: BTreeMap::from([
            ("FIRST".to_owned(), "one".to_owned()),
            ("SECOND".to_owned(), "two".to_owned()),
        ]),
    }
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
        .env(UNNAMED, "leak")
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

#[test]
fn a_named_child_runs_confined_and_reports_what_was_applied() {
    delegated(
        "a_named_child_runs_confined_and_reports_what_was_applied",
        |runner, workspace| {
            let child = ChildCommand::new(
                "sh",
                [
                    "-c",
                    "printf '%s %s ' \"$FIRST\" \"$SECOND\"; pwd; touch target/ran",
                ],
                workspace,
            )
            .with_environment(named())
            .with_profile(SandboxProfile::Workspace);
            assert_eq!(child.network(), NetworkMode::None);
            let result = runner.run_child(&child).expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert!(!result.timed_out);
            assert_eq!(result.output, "one two /workspace\n");
            assert!(workspace.join("target/ran").is_file());

            assert_eq!(result.applied.backend, Backend::Substrate);
            assert_eq!(result.applied.profile, "workspace");
            assert_eq!(result.applied.network, "none");
            assert_eq!(result.applied.writable_scopes, ["target"]);
            assert!(result.applied.read_only_roots.is_empty());
            assert!(!result.applied.cgroup.is_empty());
            assert_eq!(result.applied.profile_digest.len(), 64);
            let record = result.driver_record.as_ref().expect("the driver's record");
            assert_eq!(record.profile, SandboxProfile::Workspace);
            assert_eq!(record.network, substrate_wire::AppliedNetwork::None);
            assert_eq!(record.cgroup, result.applied.cgroup);
            assert_eq!(
                record.capability_snapshot,
                result.applied.capability_snapshot
            );
            assert!(record.read_only_roots.is_empty());
            assert!(record.secret_slots.is_empty());
        },
    );
}

#[test]
fn a_missing_guarantee_is_a_typed_refusal() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let outside = fixture.path().join("ran-outside");
    let child = ChildCommand::new(
        "sh",
        [
            "-c".to_owned(),
            format!("touch target/ran; touch '{}'", outside.display()),
        ],
        &workspace,
    )
    .with_environment(named());
    // A directory that is no cgroup: Substrate's driver is never established, so no backend
    // serves the launch, whatever this machine could otherwise delegate.
    let error = SubstrateRunner::new(Some(fixture.path().to_owned()))
        .run_child(&child)
        .unwrap_err();
    let expected = if bubblewrap_present() {
        ConfinementRefusal::CgroupUndelegated
    } else {
        ConfinementRefusal::BackendMissing
    };
    assert_eq!(error.reason, expected, "{error}");
    assert!(
        !workspace.join("target/ran").exists(),
        "nothing ran confined"
    );
    assert!(!outside.exists(), "nothing ran unconfined");
}

#[test]
fn the_environment_holds_only_what_the_host_named() {
    delegated(
        "the_environment_holds_only_what_the_host_named",
        |runner, workspace| {
            assert_eq!(
                std::env::var(UNNAMED).as_deref(),
                Ok("leak"),
                "the process that launches the child holds a variable the host did not name"
            );
            assert!(std::env::var_os("HOME").is_some());
            let child =
                ChildCommand::new("env", [] as [&str; 0], workspace).with_environment(named());
            let result = runner.run_child(&child).expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.output, "FIRST=one\nSECOND=two\n");
        },
    );
}

#[test]
fn a_program_that_reads_as_an_assignment_is_refused_before_anything_starts() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let child = ChildCommand::new("GH_TOKEN=synthetic-value", ["/usr/bin/env"], &workspace);
    let error = runner.run_child(&child).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains("`GH_TOKEN=…`"), "{error}");
    assert!(!error.detail.contains("synthetic-value"), "{error}");
    let test = TestCommand::new("RUST_LOG=debug", ["cargo", "test"]);
    let error = runner.run(&test, &workspace).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(
        !workspace.join("target").exists(),
        "the workspace is untouched and no driver was opened"
    );
}

/// A child running in one workspace directory while the runner is asked for a driver in another:
/// opening it would end the running child, so the second launch is refused and the first runs on.
#[test]
fn a_launch_needing_another_driver_is_refused_while_a_child_runs() {
    delegated(
        "a_launch_needing_another_driver_is_refused_while_a_child_runs",
        |runner, workspace| {
            let elsewhere = workspace.parent().unwrap().join("other/elsewhere");
            std::fs::create_dir_all(&elsewhere).unwrap();
            let started = workspace.join("target/started");
            let first = ChildCommand::new(
                "sh",
                ["-c", "touch target/started; sleep 3; printf first-done"],
                workspace,
            );
            let first_runner = runner.clone();
            let handle = std::thread::spawn(move || first_runner.run_child(&first));
            wait_for(&started, &handle);
            let other = runner.run_child(&ChildCommand::new("true", [] as [&str; 0], &elsewhere));
            let error = other.expect_err("no driver is opened over a running child");
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{error}"
            );
            let first = handle.join().unwrap().expect("the first child runs on");
            assert_eq!(
                (first.exit_code, first.output.as_str()),
                (Some(0), "first-done")
            );
            let after = runner
                .run_child(&ChildCommand::new("true", [] as [&str; 0], &elsewhere))
                .expect("with nothing running, the other driver opens");
            assert_eq!(after.exit_code, Some(0), "{}", after.output);
        },
    );
}

/// The story's case: a test command runs beside a running child, through one runner.
#[test]
fn a_test_runs_beside_a_running_child_through_one_runner() {
    delegated(
        "a_test_runs_beside_a_running_child_through_one_runner",
        |runner, workspace| {
            let started = workspace.join("target/started");
            let child = ChildCommand::new(
                "sh",
                ["-c", "touch target/started; sleep 3; printf child-done"],
                workspace,
            );
            let child_runner = runner.clone();
            let handle = std::thread::spawn(move || child_runner.run_child(&child));
            wait_for(&started, &handle);
            let test = runner
                .run(
                    &TestCommand::new("sh", ["-c", "printf test-done"]),
                    workspace,
                )
                .expect("the test runs beside the child");
            assert_eq!(
                (test.exit_code, test.output.as_str()),
                (Some(0), "test-done")
            );
            let child = handle.join().unwrap().expect("the child runs on");
            assert_eq!(
                (child.exit_code, child.output.as_str()),
                (Some(0), "child-done")
            );
        },
    );
}

/// A child its deadline ended has no exit status and is still a result, marked timed out: only a
/// child ended by something other than itself or its deadline is refused (unit-tested in
/// `confinement.rs`, since no child can provoke that end on purpose).
#[test]
fn a_timed_out_child_is_a_result_with_no_exit_status() {
    delegated(
        "a_timed_out_child_is_a_result_with_no_exit_status",
        |runner, workspace| {
            let timed = runner
                .run_child(
                    &ChildCommand::new("sleep", ["30"], workspace)
                        .with_timeout(std::time::Duration::from_millis(250)),
                )
                .expect("a timed-out child is a result");
            assert!(timed.timed_out, "{}", timed.output);
            assert_eq!(timed.exit_code, None);
        },
    );
}

/// Wait until the child running on `handle` created `marker`.
fn wait_for<T: std::fmt::Debug>(marker: &Path, handle: &std::thread::JoinHandle<T>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !marker.exists() {
        assert!(
            !handle.is_finished(),
            "the child returned before it created {}",
            marker.display()
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the child started within 30 s"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
