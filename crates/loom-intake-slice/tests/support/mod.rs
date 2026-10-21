//! The delegated harness the confinement tests share. A case runs against the real Substrate
//! backend in a delegated cgroup v2 scope: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes`, as `b10x-loom` starts itself, and runs the case
//! there. Where bubblewrap, a user systemd manager or the Substrate driver is absent, the case
//! prints `skipped: missing prerequisite: …` and returns; such a skip establishes no
//! qualification. Anything else fails the parent: a refusal that names no missing prerequisite,
//! a delegated binary that fails, or one that never ran exactly the case.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use b10x_loom_intake_slice::confinement::{
    ConfinementError, ConfinementRefusal, SubstrateRunner, prepare_delegated_cgroup,
};

/// Set in the test binary started inside the delegated scope, so it runs the case instead of
/// starting itself again.
const DELEGATED: &str = "LOOM_DELEGATED_CASE";
/// The file the delegated binary writes: `started`, then `ran` or [`MISSING`] and the missing
/// prerequisite.
const REPORT: &str = "LOOM_DELEGATED_CASE_REPORT";
/// The prefix of a report naming a missing prerequisite.
const MISSING: &str = "missing: ";
pub const BUBBLEWRAP: &str = "/usr/bin/bwrap";
const SYSTEMD_RUN: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];

pub fn bubblewrap_present() -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(BUBBLEWRAP).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Run the test `name`'s `case` against the real backend in a delegated scope, or skip naming
/// what is missing. The binary the scope starts holds `environment` beside the harness's own
/// variables. There `case` gets a runner on the scope's cgroup and an empty workspace, `target/`
/// created, in a directory of its own that the runner's driver already adopted.
pub fn delegated(
    name: &str,
    environment: &[(&str, &str)],
    case: impl FnOnce(&SubstrateRunner, &Path),
) {
    if std::env::var_os(DELEGATED).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        std::fs::write(&report, "started").unwrap();
        let cgroup = match prepare_delegated_cgroup() {
            Ok(cgroup) => cgroup,
            Err(error) => return refused("a delegated cgroup v2 scope", &error),
        };
        let runner = SubstrateRunner::new(Some(cgroup));
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        if let Err(error) = runner.preflight(&workspace) {
            return refused("the Substrate driver", &error);
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
        .args(SYSTEMD_RUN)
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED, "1")
        .env(REPORT, &report)
        .envs(environment.iter().copied())
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
        // The case was never reached. Only a scope that never started is a missing prerequisite:
        // systemd-run failed, no test binary wrote anything, and a bare scope does not start
        // either (as on a host with no user systemd manager).
        None => {
            assert!(
                !output.status.success(),
                "the delegated binary never reached the case {name}, yet systemd-run exited {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                !stdout.contains("running "),
                "the delegated binary started and never reached the case {name}: {}\n{stdout}{stderr}",
                output.status
            );
            if let Some(started) = scope_starts() {
                panic!(
                    "a delegated scope starts on this host ({started}), yet the case {name} was never reached: systemd-run {}\n{stdout}{stderr}",
                    output.status
                );
            }
            eprintln!(
                "skipped: missing prerequisite: a user systemd manager that starts a delegated scope (systemd-run {}: {})",
                output.status,
                stderr.trim()
            );
        }
        Some("started") => panic!(
            "the delegated case {name} stopped before it ran or named a missing prerequisite: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            ran_exactly(name, &output);
            eprintln!("ran delegated: {name}");
        }
        Some(report) => {
            let Some(missing) = report.strip_prefix(MISSING) else {
                panic!("the delegated case {name} wrote no report the harness reads: {report:?}");
            };
            ran_exactly(name, &output);
            eprintln!("skipped: missing prerequisite: {missing}");
        }
    }
}

/// Inside the delegated binary: the refusal names a prerequisite this machine lacks and the case
/// skips, or it names none and the case fails.
fn refused(prerequisite: &str, error: &ConfinementError) {
    match error.reason {
        ConfinementRefusal::BackendMissing
        | ConfinementRefusal::CgroupUndelegated
        | ConfinementRefusal::CapabilityUnserved => skip(&format!("{prerequisite} ({error})")),
        _ => panic!("{prerequisite} refused, naming no missing prerequisite: {error}"),
    }
}

/// Whether a bare delegated scope starts on this host; `Some` holds how it ended.
fn scope_starts() -> Option<String> {
    let output = Command::new("systemd-run")
        .args(SYSTEMD_RUN)
        .arg("true")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| format!("systemd-run true: {}", output.status))
}

/// The delegated binary exited 0 having run the one test `name`, which passed.
fn ran_exactly(name: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the delegated case {name} failed: {}\n{stdout}{stderr}",
        output.status
    );
    let passed = format!("test {name} ... ok");
    assert!(
        stdout.lines().any(|line| line == passed)
            && stdout.contains("test result: ok. 1 passed; 0 failed;"),
        "the delegated binary did not run exactly the case {name}: {}\n{stdout}{stderr}",
        output.status
    );
}

/// Inside the delegated binary: the case skips, naming what this machine lacks.
pub fn skip(missing: &str) {
    let report = std::env::var_os(REPORT).expect("a case skips only inside the delegated binary");
    std::fs::write(report, format!("{MISSING}{missing}")).unwrap();
}
