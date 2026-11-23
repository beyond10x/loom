//! The one probe of what the delegated tests need, `loom-xtask delegation-probe`: the test step
//! runs it before its timed run, unless its caller set `LOOM_REQUIRE_DELEGATION`, and CI's step
//! `Probe a delegated user scope` runs it before `task check`. Where it passes,
//! `LOOM_REQUIRE_DELEGATION=1` turns a delegated test that would skip into a failure; where it
//! fails, the variable stays unset and those tests skip, named by the test step.
//!
//! It checks, in order, what the tests' harnesses check, with their own fixtures and library calls,
//! and stops at the first missing prerequisite ([`Record`], [`decide`]): bubblewrap; a scope of the
//! user's systemd manager that `scope.sh` of `crates/loom-cli/tests/fixtures/tree-delegated/` holds
//! (`crates/loom-cli/tests/common/delegated_cgroup.rs`); `enter.sh` there, started by the probing
//! process as a test process starts it, moving a process into a child of that scope; and the
//! example `delegation_probe` of `b10x-loom-intake-slice`, which, in a delegated scope of its own,
//! moves itself into a leaf and opens Substrate's driver on the scope as the confinement tests do
//! (`crates/loom-intake-slice/tests/support/mod.rs`). A scope that only starts is not enough: on
//! the runner of CI run 45194449550 one started, `enter.sh` failed and Substrate served no
//! `limits`, and 75 delegated tests failed under `LOOM_REQUIRE_DELEGATION=1`.

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

/// The bubblewrap the delegated tests and Substrate's driver start.
pub const BUBBLEWRAP: &str = "/usr/bin/bwrap";
/// The fixtures of the delegated scope, relative to the repository root.
pub const FIXTURES: &str = "crates/loom-cli/tests/fixtures/tree-delegated";
/// The flags of a delegated scope of the user's systemd manager, as the tests give them.
const SCOPE: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];
/// The package and the example that run inside a delegated scope.
const EXAMPLE_PACKAGE: &str = "b10x-loom-intake-slice";
const EXAMPLE: &str = "delegation_probe";
/// The line the example prints when Substrate's driver serves every family Loom needs.
const SERVED: &str = "served";

/// What one run of the probe saw, step by step, in the order it runs them; a step after the first
/// that failed is not run (`None`).
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// `/usr/bin/bwrap` is an executable file.
    pub bubblewrap: bool,
    /// `systemd-run --user --scope -p Delegate=yes` started `scope.sh`, which printed its scope.
    pub scope: Option<Result<(), String>>,
    /// `enter.sh`, started by the probing process, moved a process into a child of that scope.
    pub enter: Option<Result<(), String>>,
    /// The example `delegation_probe` of `b10x-loom-intake-slice`, in a delegated scope of its own,
    /// moved itself into a leaf and found Substrate's driver serving every family Loom needs.
    pub substrate: Option<Result<(), String>>,
}

/// Whether the delegated tests can run here, given what the probe saw: `Err` names the first
/// prerequisite that is missing and what it said. A step not run proves nothing and fails.
pub fn decide(record: &Record) -> Result<(), String> {
    if !record.bubblewrap {
        return Err(format!("executable {BUBBLEWRAP} is missing"));
    }
    let steps = [
        (
            "a delegated scope of the user's systemd manager (systemd-run --user --scope -p \
             Delegate=yes, scope.sh)",
            &record.scope,
        ),
        (
            "a probing process that may move a process into a child of that scope (enter.sh)",
            &record.enter,
        ),
        (
            "Substrate's driver serving every family Loom needs in a delegated scope (example \
             delegation_probe of b10x-loom-intake-slice)",
            &record.substrate,
        ),
    ];
    for (what, step) in steps {
        match step {
            Some(Ok(())) => {}
            Some(Err(why)) => return Err(format!("{what}: {why}")),
            None => return Err(format!("{what}: not run")),
        }
    }
    Ok(())
}

/// Runs the probe in the repository `root`. `Err` when the probe itself cannot run: the example
/// does not build.
pub fn probe(root: &Path) -> Result<Record, String> {
    let mut record = Record {
        bubblewrap: executable(Path::new(BUBBLEWRAP)),
        scope: None,
        enter: None,
        substrate: None,
    };
    if !record.bubblewrap {
        return Ok(record);
    }
    let fixtures = root.join(FIXTURES);
    let held = match Held::start(&fixtures) {
        Ok(held) => held,
        Err(why) => {
            record.scope = Some(Err(why));
            return Ok(record);
        }
    };
    record.scope = Some(Ok(()));
    let entered = enter(&fixtures, &held.root);
    drop(held);
    let entered_ok = entered.is_ok();
    record.enter = Some(entered);
    if !entered_ok {
        return Ok(record);
    }
    let example = build_example(root)?;
    record.substrate = Some(in_own_scope(&example));
    Ok(record)
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_owned()
}

/// A delegated scope `scope.sh` holds until the value is dropped, as a test holds one.
struct Held {
    root: PathBuf,
    keeper: Child,
}

impl Held {
    fn start(fixtures: &Path) -> Result<Self, String> {
        let mut keeper = Command::new("systemd-run")
            .args(SCOPE)
            .arg("sh")
            .arg(fixtures.join("scope.sh"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("systemd-run ({e})"))?;
        let mut line = String::new();
        let read = keeper
            .stdout
            .take()
            .map(|stdout| BufReader::new(stdout).read_line(&mut line));
        let root = PathBuf::from(line.trim());
        if matches!(read, Some(Ok(_)))
            && root.starts_with("/sys/fs/cgroup")
            && root.join("keeper").is_dir()
        {
            return Ok(Self { root, keeper });
        }
        drop(keeper.stdin.take());
        match keeper.wait_with_output() {
            Ok(output) => Err(format!(
                "systemd-run {}, printed no scope ({line:?}): {}",
                output.status,
                said(&output)
            )),
            Err(error) => Err(format!("systemd-run ({error})")),
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        drop(self.keeper.stdin.take());
        let _ = self.keeper.wait();
    }
}

/// `enter.sh` moves a process into `<scope>/probe` and runs `true` there.
fn enter(fixtures: &Path, scope: &Path) -> Result<(), String> {
    let probe = scope.join("probe");
    let output = Command::new("sh")
        .arg(fixtures.join("enter.sh"))
        .arg(&probe)
        .arg("true")
        .output();
    let _ = std::fs::remove_dir(&probe);
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!("enter.sh {}: {}", output.status, said(&output))),
        Err(error) => Err(format!("sh ({error})")),
    }
}

/// Builds the example and answers its path.
fn build_example(root: &Path) -> Result<PathBuf, String> {
    let args = [
        "build",
        "--locked",
        "-q",
        "-p",
        EXAMPLE_PACKAGE,
        "--example",
        EXAMPLE,
    ];
    let status = crate::test_step::cargo_command(root)
        .args(args)
        .status()
        .map_err(|e| format!("run cargo {}: {e}", args.join(" ")))?;
    if !status.success() {
        return Err(format!("cargo {} exited {status}", args.join(" ")));
    }
    Ok(crate::test_step::target_dir(root)
        .join("debug/examples")
        .join(EXAMPLE))
}

/// The example in a delegated scope of its own: `Ok` when it printed [`SERVED`] and exited 0.
fn in_own_scope(example: &Path) -> Result<(), String> {
    match Command::new("systemd-run")
        .args(SCOPE)
        .arg(example)
        .output()
    {
        Ok(output)
            if output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line == SERVED) =>
        {
            Ok(())
        }
        Ok(output) => Err(format!("{}: {}", output.status, said(&output))),
        Err(error) => Err(format!("systemd-run ({error})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The refusal of Substrate's driver, as CI run 45194449550 printed it.
    const UNSERVED: &str = "CgroupUndelegated: confinement.family-unserved: Substrate's linux \
        adapter does not serve `limits` on this machine; Substrate proves it only once bubblewrap \
        runs with user namespaces and the cpu, memory and pids controllers are delegated";

    /// The facts of the `ubuntu-26.04` runner of CI run 45194449550 (PR 166): bubblewrap installed,
    /// `systemd-run --user --scope -p Delegate=yes true` exited 0 (`delegation probe passed`), the
    /// tests' `enter.sh` failed with `echo: I/O error`, and Substrate's driver refused `limits`.
    fn runner() -> Record {
        Record {
            bubblewrap: true,
            scope: Some(Ok(())),
            enter: Some(Err(
                "crates/loom-cli/tests/fixtures/tree-delegated/enter.sh: 6: echo: echo: I/O error"
                    .to_owned(),
            )),
            substrate: Some(Err(UNSERVED.to_owned())),
        }
    }

    /// The facts of a slot pool job on the workstation, where every delegated test passed.
    fn pool_job() -> Record {
        Record {
            bubblewrap: true,
            scope: Some(Ok(())),
            enter: Some(Ok(())),
            substrate: Some(Ok(())),
        }
    }

    /// What the probe it replaces read: `systemd-run --user --scope -p Delegate=yes true` exited 0.
    fn weak_probe_passes(record: &Record) -> bool {
        matches!(record.scope, Some(Ok(())))
    }

    #[test]
    fn the_runner_of_pr_166_does_not_pass_where_the_weak_probe_did() {
        let runner = runner();
        assert!(
            weak_probe_passes(&runner),
            "the replaced probe passed on the runner"
        );
        let why = decide(&runner).expect_err("the delegated tests cannot run on the runner");
        assert!(
            why.contains("enter.sh") && why.contains("I/O error"),
            "{why}"
        );
    }

    #[test]
    fn substrate_refusing_limits_fails_the_probe_where_a_process_may_enter_the_scope() {
        let record = Record {
            enter: Some(Ok(())),
            ..runner()
        };
        assert!(weak_probe_passes(&record));
        let why = decide(&record).expect_err("Substrate serves no limits");
        assert!(why.contains("Substrate") && why.contains(UNSERVED), "{why}");
    }

    #[test]
    fn a_plain_session_scope_fails_the_probe() {
        // This workstation, from `session-3.scope`: the scope starts, and the test process may not
        // move a process into it, its nearest common ancestor being root's `user-1000.slice`.
        let record = Record {
            bubblewrap: true,
            scope: Some(Ok(())),
            enter: Some(Err(
                "enter.sh: line 6: echo: write error: Permission denied".to_owned(),
            )),
            substrate: None,
        };
        assert!(weak_probe_passes(&record));
        let why = decide(&record).expect_err("a session scope moves no process into a scope");
        assert!(why.contains("Permission denied"), "{why}");
    }

    #[test]
    fn a_pool_job_passes_the_probe() {
        assert_eq!(decide(&pool_job()), Ok(()));
    }

    #[test]
    fn a_missing_bubblewrap_or_a_step_not_run_fails_the_probe() {
        let why = decide(&Record {
            bubblewrap: false,
            ..pool_job()
        })
        .expect_err("no bubblewrap");
        assert!(why.contains("/usr/bin/bwrap"), "{why}");
        for record in [
            Record {
                scope: None,
                ..pool_job()
            },
            Record {
                enter: None,
                ..pool_job()
            },
            Record {
                substrate: None,
                ..pool_job()
            },
        ] {
            let why = decide(&record).expect_err("a step not run proves nothing");
            assert!(why.contains("not run"), "{record:?}: {why}");
        }
    }
}
