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
    SubstrateRunner, prepare_delegated_cgroup,
};

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
            assert_eq!(record.capability_snapshot, result.applied.capability_snapshot);
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
