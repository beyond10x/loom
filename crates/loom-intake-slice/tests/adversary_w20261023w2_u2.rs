//! Adversary cases for `story:child-workdir`: a working directory the host's disk holds as a
//! directory, reached through no link, that the child still cannot enter (no search permission).
//! `check_workdir` asks only whether the directory is there; Substrate's own check opens it with
//! `O_PATH`, which needs no search permission on it either; so the launch goes ahead and
//! bubblewrap's `--chdir` fails with `Permission denied`, a reason Substrate does not read as
//! `exec.workdir-missing`. The program never ran, yet the result is its own exit.
//!
//! The delegated cases run against the real backend through `tests/support`, as the
//! confined-child cases do, and skip naming what is missing.

mod support;

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementError, ConfinementRefusal, SubstrateRunner, TestExecution,
};
use support::{bubblewrap_present, skip};

fn host_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("loom-adv-workdir-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let canonical = directory.path().canonicalize().unwrap();
    (directory, canonical)
}

/// A directory with read and write permission and no search permission, which no process
/// without `CAP_DAC_OVERRIDE` can `chdir` into.
fn locked_directory(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn past_every_check() -> ConfinementRefusal {
    if bubblewrap_present() {
        ConfinementRefusal::CgroupUndelegated
    } else {
        ConfinementRefusal::BackendMissing
    }
}

/// A working directory the child cannot enter is refused `ScopeInvalid` before launch, naming
/// it, as an absent one or one that is no directory is: the launch would otherwise start a child
/// that never runs. Observed: it passes every check and reaches the cgroup.
#[test]
fn adversary_a_workdir_the_child_cannot_enter_is_refused_before_launch() {
    let (_held, base) = host_directory();
    let workspace = base.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    locked_directory(&workspace.join("locked"));
    let tools = base.join("tools");
    locked_directory(&tools.join("locked"));
    let runner = SubstrateRunner::new(Some(base.clone()));
    for (workdir, child) in [
        (
            "/workspace/locked",
            ChildCommand::new("true", [] as [&str; 0], &workspace).with_workdir("/workspace/locked"),
        ),
        (
            "/opt/tools/locked",
            ChildCommand::new("true", [] as [&str; 0], &workspace)
                .with_read_only_root(&tools, "/opt/tools")
                .with_workdir("/opt/tools/locked"),
        ),
    ] {
        for error in [
            runner.run_child(&child).unwrap_err(),
            runner.spawn_child(&child).unwrap_err(),
        ] {
            assert_ne!(
                error.reason,
                past_every_check(),
                "{workdir}: a directory the child cannot enter passed every check before launch: {error}"
            );
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{workdir}: {error}");
            assert!(error.detail.contains(workdir), "{workdir}: {error}");
        }
    }
}

/// A read-only file given at its own host path is no working directory: the resolved file mount
/// refuses it, naming it.
#[test]
fn adversary_a_file_given_at_its_host_path_is_no_workdir() {
    let (_held, base) = host_directory();
    let workspace = base.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let file = base.join("policy.toml");
    std::fs::write(&file, "allow = true\n").unwrap();
    let text = file.to_str().unwrap().to_owned();
    let child = ChildCommand::new("true", [] as [&str; 0], &workspace)
        .at_host_path(&file)
        .with_workdir(&file);
    let runner = SubstrateRunner::new(Some(base.clone()));
    for error in [
        runner.run_child(&child).unwrap_err(),
        runner.spawn_child(&child).unwrap_err(),
    ] {
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
        assert!(error.detail.contains(&text), "{error}");
        assert!(error.detail.contains("read-only file"), "{error}");
    }
}

fn workdir_result(
    result: Result<TestExecution, ConfinementError>,
) -> Option<Result<TestExecution, ConfinementError>> {
    match result {
        Err(error)
            if error.reason == ConfinementRefusal::CapabilityUnserved
                && error.detail.contains("(`exec.workdir`)") =>
        {
            skip(&format!("Substrate working directory ({error})"));
            None
        }
        result => Some(result),
    }
}

/// Against the real backend: a child named with a directory it cannot enter never runs its
/// program, so its result must not be a program's exit. Observed: `run_child` returns `Ok` with
/// bubblewrap's diagnostic as the output and its exit status as the child's.
#[test]
fn adversary_a_child_that_cannot_enter_its_workdir_reports_no_exit_of_its_own() {
    support::delegated(
        "adversary_a_child_that_cannot_enter_its_workdir_reports_no_exit_of_its_own",
        &[],
        |runner, workspace| {
            locked_directory(&workspace.join("locked"));
            let child = ChildCommand::new("sh", ["-c", "echo ran"], workspace)
                .with_workdir("/workspace/locked");
            let Some(result) = workdir_result(runner.run_child(&child)) else {
                return;
            };
            std::fs::set_permissions(
                workspace.join("locked"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            match result {
                Ok(execution) => panic!(
                    "the program never ran, yet the result is its exit {:?}, output {:?}",
                    execution.exit_code, execution.output
                ),
                Err(error) => {
                    assert!(error.detail.contains("/workspace/locked"), "{error}");
                }
            }
        },
    );
}

/// Against the real backend, the exception the check makes: `/workspace/target`, absent before
/// the launch, is created by it and the child starts there.
#[test]
fn adversary_a_child_starts_in_the_target_the_launch_creates() {
    support::delegated(
        "adversary_a_child_starts_in_the_target_the_launch_creates",
        &[],
        |runner, workspace| {
            // The preflight created the given workspace's `target`; a fresh workspace has none.
            let fresh = workspace.parent().unwrap().join("fresh");
            std::fs::create_dir(&fresh).unwrap();
            assert!(!fresh.join("target").exists());
            let child = ChildCommand::new("sh", ["-c", "pwd"], &fresh)
                .with_workdir("/workspace/target");
            let Some(result) = workdir_result(runner.run_child(&child)) else {
                return;
            };
            let execution = result.expect("the child starts in the created target");
            assert_eq!(execution.exit_code, Some(0), "{}", execution.output);
            assert_eq!(execution.output, "/workspace/target\n");
            assert_eq!(execution.applied.workdir.as_deref(), Some("/workspace/target"));
        },
    );
}
