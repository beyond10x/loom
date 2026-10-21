//! The delegated harness the confinement tests share. A case runs against the real Substrate
//! backend in a delegated cgroup v2 scope: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes`, as `b10x-loom` starts itself, and runs the case
//! there. Where bubblewrap, a user systemd manager or the Substrate driver is absent, the case
//! prints `skipped: missing prerequisite: …` and returns; such a skip establishes no
//! qualification.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_intake_slice::confinement::{SubstrateRunner, prepare_delegated_cgroup};

/// Set in the test binary started inside the delegated scope, so it runs the case instead of
/// starting itself again.
const DELEGATED: &str = "LOOM_DELEGATED_CASE";
/// The file the delegated binary writes: `started`, then `ran` or the missing prerequisite.
const REPORT: &str = "LOOM_DELEGATED_CASE_REPORT";
pub const BUBBLEWRAP: &str = "/usr/bin/bwrap";

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

/// Inside the delegated binary: the case skips, naming what this machine lacks.
pub fn skip(missing: &str) {
    let report = std::env::var_os(REPORT).expect("a case skips only inside the delegated binary");
    std::fs::write(report, missing).unwrap();
}
