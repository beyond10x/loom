//! Adversary cases for `story:sdk-confines-any-child` (wave 2026-10-20-w2, U4).
//!
//! Every case runs against the real backend in a delegated cgroup v2 scope, the way
//! `tests/confined_child.rs` does: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes` and runs the case there. Where bubblewrap, a user
//! systemd manager or the Substrate driver is absent, the case prints
//! `skipped: missing prerequisite: …` and returns.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementRefusal, ExecEnvironment, SubstrateRunner, TestRunner as _,
    discover_cgroup_root, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;

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

/// Pass 2. A runner and its clones now keep one Substrate driver per workspace directory for as
/// long as they live (CHANGELOG, `SubstrateRunner` rustdoc). A host that keeps its runner, as
/// `b10x-loom` keeps one for a whole run, launches in the same workspace many times: every one of
/// those launches runs, the 129th, a test, as much as the first.
#[test]
fn adv_a_runner_serves_more_than_128_launches_in_one_workspace() {
    delegated(
        "adv_a_runner_serves_more_than_128_launches_in_one_workspace",
        |runner, workspace| {
            for index in 0..128 {
                let result = runner
                    .run_child(&ChildCommand::new("true", [] as [&str; 0], workspace))
                    .unwrap_or_else(|error| panic!("launch {} is refused: {error}", index + 1));
                assert_eq!(result.exit_code, Some(0), "launch {}", index + 1);
            }
            let test = runner.run(
                &TestCommand::new("sh", ["-c", "printf test-done"]),
                workspace,
            );
            let test = test.expect("launch 129, a test through TestRunner::run, runs");
            assert_eq!(
                (test.exit_code, test.output.as_str()),
                (Some(0), "test-done")
            );
        },
    );
}

/// Pass 2. "No driver is opened on a root while a launch of this process runs under it"
/// (CHANGELOG, `SubstrateRunner` rustdoc). The count is the process's, over every runner; a second
/// runner naming the same cgroup root by another path must not open a driver that ends the first
/// runner's running child.
#[test]
fn adv_a_root_named_another_way_opens_no_driver_over_a_running_child() {
    delegated(
        "adv_a_root_named_another_way_opens_no_driver_over_a_running_child",
        |runner, workspace| {
            let root = discover_cgroup_root().expect("the delegated root");
            let same_root = root
                .join("..")
                .join(root.file_name().expect("the root has a name"));
            let elsewhere = workspace.parent().unwrap().join("other/elsewhere");
            std::fs::create_dir_all(&elsewhere).unwrap();
            let started = workspace.join("target/started");
            let first = ChildCommand::new(
                "sh",
                ["-c", "touch target/started; sleep 3; printf first-done"],
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
                "the first child started: {:?}",
                handle.join().unwrap()
            );

            let other = SubstrateRunner::new(Some(same_root.clone()))
                .run_child(&ChildCommand::new("true", [] as [&str; 0], &elsewhere));
            let first = handle.join().unwrap();
            eprintln!("root {}, named {}", root.display(), same_root.display());
            eprintln!("first child: {first:?}");
            eprintln!("second runner: {other:?}");

            let first = first.expect("the first child runs on");
            assert_eq!(
                (first.exit_code, first.output.as_str()),
                (Some(0), "first-done")
            );
            assert_eq!(
                other
                    .expect_err("no driver is opened over the running child")
                    .reason,
                ConfinementRefusal::CapabilityUnserved
            );
        },
    );
}

/// Pass 2. The program a host names is the program that runs, never read by Substrate's
/// `/usr/bin/env` as one of its own operands (`ChildCommand` rustdoc: refused programs are those
/// env would reinterpret). A lone `-` is such an operand: env takes it as `-i` and runs the next
/// word instead.
#[test]
fn adv_a_program_env_reads_as_an_operand_is_not_replaced_by_its_first_argument() {
    delegated(
        "adv_a_program_env_reads_as_an_operand_is_not_replaced_by_its_first_argument",
        |runner, workspace| {
            let child = ChildCommand::new("-", ["/usr/bin/printf", "replaced"], workspace);
            assert_eq!(child.program(), "-");
            match runner.run_child(&child) {
                Err(error) => eprintln!("refused: {error}"),
                Ok(result) => assert_ne!(
                    result.output, "replaced",
                    "the program `-` ran its first argument `/usr/bin/printf`: exit {:?}",
                    result.exit_code
                ),
            }
        },
    );
}
