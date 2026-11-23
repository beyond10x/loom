//! The one probe of what the delegated tests need, and its one decision ([`decide`]). Std only,
//! so every user reaches this file through `#[path]` (or `mod`) without a dependency:
//! `loom-xtask delegation-probe` (`crates/loom-xtask/src/delegation.rs`), which the test step and
//! CI's step `Probe a delegated user scope` run; the guards of the delegated tests of
//! `b10x-loom-cli` (`delegated_cgroup.rs` beside this file, `tree_run/handoff_when.rs`); and the
//! embedder fixture's (`crates/loom-tree/tests/embedder/tests/main.rs`). Where the decision says
//! no, a guard skips naming the missing prerequisite, or fails under `LOOM_REQUIRE_DELEGATION`.
//!
//! It checks, in order, and stops at the first missing prerequisite ([`Record`]): bubblewrap; a
//! scope of the user's systemd manager that `fixtures/tree-delegated/scope.sh` holds; `enter.sh`
//! there, started by the probing process, moving a process into a child of that scope; and the
//! example `delegation_probe` of `b10x-loom-intake-slice`, which, in a delegated scope of its own,
//! moves itself into a leaf and opens Substrate's driver on the scope as the confinement tests do.
//! A scope that only starts is not enough: on the runner of CI run 45194449550 one started,
//! `enter.sh` failed and Substrate served no `limits`.
#![allow(dead_code)]

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
pub const EXAMPLE_PACKAGE: &str = "b10x-loom-intake-slice";
pub const EXAMPLE: &str = "delegation_probe";
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
        return Err(format!("executable {BUBBLEWRAP}"));
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

/// Runs the probe with the fixtures in `fixtures`; `example` answers the path of the example,
/// asked only once the steps before it passed. `Err` only when `example` does.
pub fn probe(
    fixtures: &Path,
    example: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<Record, String> {
    let mut record = Record {
        bubblewrap: executable(Path::new(BUBBLEWRAP)),
        scope: None,
        enter: None,
        substrate: None,
    };
    if !record.bubblewrap {
        return Ok(record);
    }
    let held = match Held::start(fixtures) {
        Ok(held) => held,
        Err(why) => {
            record.scope = Some(Err(why));
            return Ok(record);
        }
    };
    record.scope = Some(Ok(()));
    let entered = enter(fixtures, &held.root);
    drop(held);
    let entered_ok = entered.is_ok();
    record.enter = Some(entered);
    if !entered_ok {
        return Ok(record);
    }
    let example = example()?;
    record.substrate = Some(in_own_scope(&example));
    Ok(record)
}

/// The probe as a test runs it, from the test binary `test_binary` (`target/<profile>/deps/…`):
/// the example is the one the workspace's test build made beside it
/// (`target/<profile>/examples/delegation_probe`), never built here.
pub fn probe_from_test(fixtures: &Path, test_binary: &Path) -> Record {
    let example = test_binary
        .parent()
        .and_then(Path::parent)
        .map(|profile| profile.join("examples").join(EXAMPLE));
    let found = probe(fixtures, || {
        Ok(example.clone().unwrap_or_else(|| PathBuf::from(EXAMPLE)))
    });
    match found {
        Ok(record) => record,
        Err(why) => Record {
            bubblewrap: true,
            scope: Some(Ok(())),
            enter: Some(Ok(())),
            substrate: Some(Err(why)),
        },
    }
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

/// The example in a delegated scope of its own: `Ok` when it printed [`SERVED`] and exited 0.
fn in_own_scope(example: &Path) -> Result<(), String> {
    if !executable(example) {
        return Err(format!(
            "{} is not built: build it with `cargo build -p {EXAMPLE_PACKAGE} --example {EXAMPLE}` \
             or the workspace's tests",
            example.display()
        ));
    }
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

/// The refusal of Substrate's driver, as CI run 45194449550 printed it.
#[cfg(test)]
pub const UNSERVED: &str = "CgroupUndelegated: confinement.family-unserved: Substrate's linux \
    adapter does not serve `limits` on this machine; Substrate proves it only once bubblewrap runs \
    with user namespaces and the cpu, memory and pids controllers are delegated";

/// The facts of the `ubuntu-26.04` runner of CI run 45194449550 (PR 166): bubblewrap installed,
/// `systemd-run --user --scope -p Delegate=yes true` exited 0 (`delegation probe passed`), the
/// tests' `enter.sh` failed with `echo: I/O error`, and Substrate's driver refused `limits`.
#[cfg(test)]
pub fn runner_of_pr_166() -> Record {
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

/// The facts of a build-pool job on the workstation, where every delegated test passed.
#[cfg(test)]
pub fn pool_job() -> Record {
    Record {
        bubblewrap: true,
        scope: Some(Ok(())),
        enter: Some(Ok(())),
        substrate: Some(Ok(())),
    }
}
