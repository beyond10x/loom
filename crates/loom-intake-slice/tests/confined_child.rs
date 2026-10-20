//! A child process the host names runs confined through Substrate (`SubstrateRunner::run_child`),
//! with the test runner's guarantees: a missing guarantee is a typed refusal and nothing runs, the
//! child's environment holds only what the host named, it writes only `target` and the workspace
//! paths the host named, and the applied confinement comes back with the result.
//!
//! The delegated cases need bubblewrap and a delegated cgroup v2 scope. Each starts its own test
//! binary again under `systemd-run --user --scope -p Delegate=yes`, as `b10x-loom` starts itself,
//! and runs the case there. Where bubblewrap, a user systemd manager or the Substrate driver is
//! absent, the case prints `skipped: missing prerequisite: …` and returns; such a skip establishes
//! no qualification. The egress cases also skip naming what is missing where `bash` is absent or
//! Substrate does not serve an egress aperture on this machine. Their hosts are loopback listeners
//! the test starts; nothing connects past loopback.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    Backend, ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, ExecEnvironment,
    NetworkMode, SandboxProfile, SubstrateRunner, TestExecution, TestRunner as _,
    prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;
use substrate_wire::{ApertureMechanism, AppliedNetwork};

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

/// A child its deadline ended has no exit status and is still a result, marked timed out, as one
/// its egress aperture's byte ceiling ended is (`adversary_w20261020w2_u5.rs`,
/// `adv_a_child_its_byte_ceiling_stopped_is_reported_as_such`): only a child with no exit status
/// that something other than itself, its deadline or that ceiling ended is refused, which
/// `confinement.rs` unit-tests (`a_child_result_with_no_exit_status_and_no_timeout_is_refused`,
/// `a_child_its_declared_byte_ceiling_ended_is_a_result_and_no_other_end_is`).
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

/// Try to write each path and say, per path, whether the write was `written` or `refused`.
fn write_each(paths: &[&str]) -> String {
    format!(
        "for path in {}; do if (printf written > \"$path\") 2>/dev/null; \
         then echo \"$path:written\"; else echo \"$path:refused\"; fi; done",
        paths.join(" ")
    )
}

/// The story's case: a child writes in the workspace directory the host names, a file it edits
/// and one it creates, and nowhere else in the workspace: not in another directory, not in a file
/// at its root, and not in `.git`, which the child writes only once the host names it. The
/// applied confinement lists the named paths beside `target`, and a test run in the same
/// workspace through the same runner still writes only under `target`.
#[test]
fn a_child_writes_only_the_paths_the_host_named() {
    delegated(
        "a_child_writes_only_the_paths_the_host_named",
        |runner, workspace| {
            for directory in ["scratch", "other", ".git"] {
                std::fs::create_dir(workspace.join(directory)).unwrap();
            }
            std::fs::write(workspace.join("scratch/edited"), "before").unwrap();
            std::fs::write(workspace.join("README"), "kept\n").unwrap();
            let script = write_each(&[
                "scratch/edited",
                "scratch/created",
                "other/written",
                ".git/written",
                "README",
            ]);
            let child = ChildCommand::new("sh", ["-c", script.as_str()], workspace)
                .with_writable("scratch");
            assert_eq!(child.writable(), [Path::new("scratch")]);
            let result = runner.run_child(&child).expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(
                result.output,
                "scratch/edited:written\nscratch/created:written\nother/written:refused\n\
                 .git/written:refused\nREADME:refused\n"
            );
            for written in ["scratch/edited", "scratch/created"] {
                assert_eq!(
                    std::fs::read_to_string(workspace.join(written)).unwrap(),
                    "written",
                    "{written}"
                );
            }
            for refused in ["other/written", ".git/written"] {
                assert!(!workspace.join(refused).exists(), "{refused}");
            }
            assert_eq!(
                std::fs::read_to_string(workspace.join("README")).unwrap(),
                "kept\n"
            );
            assert_eq!(result.applied.writable_scopes, ["scratch", "target"]);
            let record = result.driver_record.as_ref().expect("the driver's record");
            assert_eq!(
                record.workspace_access.writable_subtrees(),
                Some(["scratch".to_owned(), "target".to_owned()].as_slice())
            );

            // `.git` is the host's to name; named alone, it is written and `scratch` is not.
            let script = write_each(&[".git/written", "scratch/again"]);
            let git = runner
                .run_child(
                    &ChildCommand::new("sh", ["-c", script.as_str()], workspace)
                        .with_writable(".git"),
                )
                .expect("the child runs confined");
            assert_eq!(git.exit_code, Some(0), "{}", git.output);
            assert_eq!(git.output, ".git/written:written\nscratch/again:refused\n");
            assert!(workspace.join(".git/written").is_file());
            assert!(!workspace.join("scratch/again").exists());
            assert_eq!(git.applied.writable_scopes, [".git", "target"]);
            assert_ne!(git.applied.profile_digest, result.applied.profile_digest);

            // A test in the same workspace, through the same runner, writes only under `target`.
            let script = write_each(&["scratch/by-test", "target/by-test"]);
            let test = runner
                .run(&TestCommand::new("sh", ["-c", script.as_str()]), workspace)
                .expect("the test runs confined");
            assert_eq!(test.exit_code, Some(0), "{}", test.output);
            assert_eq!(
                test.output,
                "scratch/by-test:refused\ntarget/by-test:written\n"
            );
            assert!(!workspace.join("scratch/by-test").exists());
            assert_eq!(test.applied.writable_scopes, ["target"]);
        },
    );
}

/// A writable path the child could leave the workspace through, or that names no directory of it,
/// is refused `ScopeInvalid` before anything starts, naming the path: an absolute path, one that
/// climbs out with `..`, one through a symbolic link (to outside the workspace or inside it), an
/// absent path, a file, a socket, a directory holding a socket or a file hard-linked from outside
/// the writable paths, and a path inside `target` or inside another named path. Each refusal
/// leaves the workspace as it was, without `target`. The same workspace with a well-named path
/// gets past every check, to the missing backend.
#[test]
fn a_writable_path_outside_the_workspace_or_through_a_link_is_refused() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    let outside = fixture.path().join("outside");
    std::fs::create_dir_all(workspace.join("scratch/inner")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, workspace.join("outward")).unwrap();
    std::os::unix::fs::symlink("scratch", workspace.join("alias")).unwrap();
    std::fs::write(workspace.join("file"), "a file").unwrap();
    let _socket = UnixListener::bind(workspace.join("socket")).unwrap();
    std::fs::create_dir(workspace.join("sockets")).unwrap();
    let _held = UnixListener::bind(workspace.join("sockets/held")).unwrap();
    std::fs::create_dir(workspace.join("linked")).unwrap();
    std::fs::write(workspace.join("source"), "source").unwrap();
    std::fs::hard_link(workspace.join("source"), workspace.join("linked/alias")).unwrap();
    let ran = fixture.path().join("ran");
    let script = format!("touch '{}'", ran.display());
    let child = || ChildCommand::new("sh", ["-c", script.as_str()], &workspace);
    // A directory that is no cgroup: a launch that got past the refusal would be refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let absolute = outside.display().to_string();
    for (path, refused) in [
        (absolute.as_str(), "absolute"),
        ("../outside", "climbs out"),
        ("scratch/../../outside", "climbs out"),
        ("outward", "symbolic link"),
        ("outward/inner", "symbolic link"),
        ("alias", "symbolic link"),
        ("absent", "does not exist"),
        ("file", "no directory"),
        ("socket", "no directory"),
        ("sockets", "socket"),
        ("linked", "hard link"),
        ("target/debug", "overlap"),
    ] {
        let error = runner.run_child(&child().with_writable(path)).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{path}: {error}"
        );
        assert!(error.detail.contains(path), "{path}: {error}");
        assert!(error.detail.contains(refused), "{path}: {error}");
        assert!(
            !workspace.join("target").exists(),
            "{path}: a refused path leaves the workspace as it was"
        );
    }
    let nested = child()
        .with_writable("scratch")
        .with_writable("scratch/inner");
    let error = runner.run_child(&nested).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains("scratch/inner"), "{error}");
    assert!(!ran.exists(), "nothing ran");
    assert!(
        !workspace.join("target").exists(),
        "the workspace is as it was"
    );

    let error = runner
        .run_child(&child().with_writable("scratch"))
        .unwrap_err();
    let expected = if bubblewrap_present() {
        ConfinementRefusal::CgroupUndelegated
    } else {
        ConfinementRefusal::BackendMissing
    };
    assert_eq!(error.reason, expected, "{error}");
    assert!(!ran.exists(), "nothing ran");
}

/// The shell the egress cases connect with, through its `/dev/tcp` redirections.
const BASH: &str = "/usr/bin/bash";
/// The start of the refusal `run_child` gives when Substrate cannot serve the egress aperture.
const EGRESS_UNSERVED: &str = "Substrate does not serve the egress aperture";

/// A loopback listener the test starts on 127.0.0.1, standing in for a host, and its address.
fn loopback_host() -> (TcpListener, SocketAddr) {
    loopback_host_at(Ipv4Addr::LOCALHOST)
}

/// A listener the test starts on the loopback address `ip` (any of 127.0.0.0/8), and its address.
fn loopback_host_at(ip: Ipv4Addr) -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind((ip, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    (listener, address)
}

/// The egress aperture `model`, naming the host at `address` and pinned there.
fn model_aperture(address: SocketAddr) -> EgressAperture {
    EgressAperture {
        name: "model".to_owned(),
        host: address.ip().to_string(),
        port: address.port(),
        pinned: address,
        max_bytes: None,
    }
}

/// Inside the delegated binary: the case skips, naming what this machine lacks.
fn skip(missing: &str) {
    let report = std::env::var_os(REPORT).expect("a case skips only inside the delegated binary");
    std::fs::write(report, missing).unwrap();
}

/// The result of a child with an egress aperture, or `None` once the case skipped because
/// Substrate does not serve the aperture here; any other refusal fails the case.
fn egress_result(result: Result<TestExecution, ConfinementError>) -> Option<TestExecution> {
    match result {
        Err(error)
            if error.reason == ConfinementRefusal::CapabilityUnserved
                && error.detail.starts_with(EGRESS_UNSERVED) =>
        {
            skip(&format!("Substrate egress apertures ({error})"));
            None
        }
        result => Some(result.expect("the child runs confined with its egress")),
    }
}

/// Answer one connection to `listener` with `answer`, waiting at most 30 s for it.
fn answer_once(listener: TcpListener, answer: &'static [u8]) -> std::thread::JoinHandle<bool> {
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream.write_all(answer).unwrap();
                    return true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("the named host accepts: {error}"),
            }
        }
        false
    })
}

/// The host name the story's case names its host by.
const NAMED_HOST: &str = "model.example.test";

/// The story's case: under an egress profile the child reaches the host the profile names, by its
/// name, and fails to reach one it does not name, here another loopback port of this machine. The
/// named host listens on 127.0.0.2, so only the aperture, never the address the child dials by
/// name, can carry the connection; dialling that address directly fails too.
#[test]
fn a_child_reaches_only_the_named_egress() {
    delegated(
        "a_child_reaches_only_the_named_egress",
        |runner, workspace| {
            if !Path::new(BASH).is_file() {
                return skip(&format!("executable {BASH}"));
            }
            let (named, named_at) = loopback_host_at(Ipv4Addr::new(127, 0, 0, 2));
            let (unnamed, unnamed_at) = loopback_host();
            unnamed.set_nonblocking(true).unwrap();
            let answered = answer_once(named, b"named-host\n");
            let script = format!(
                "read -r line < /dev/tcp/{NAMED_HOST}/{named}; printf 'named:%s\\n' \"$line\"; \
                 if : 2>/dev/null 3<>/dev/tcp/127.0.0.1/{unnamed}; \
                 then echo unnamed:reached; else echo unnamed:refused; fi; \
                 if : 2>/dev/null 4<>/dev/tcp/127.0.0.2/{named}; \
                 then echo pinned:reached; else echo pinned:refused; fi",
                named = named_at.port(),
                unnamed = unnamed_at.port(),
            );
            let aperture = EgressAperture {
                host: NAMED_HOST.to_owned(),
                ..model_aperture(named_at)
            };
            let child =
                ChildCommand::new(BASH, ["-c", script.as_str()], workspace).with_egress(aperture);
            assert_eq!(child.network(), NetworkMode::Aperture);
            let Some(result) = egress_result(runner.run_child(&child)) else {
                return;
            };
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(
                result.output,
                "named:named-host\nunnamed:refused\npinned:refused\n"
            );
            assert!(answered.join().unwrap(), "the named host was reached");
            assert_eq!(
                unnamed.accept().map(|_| ()).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "nothing reached the host the profile does not name"
            );
        },
    );
}

/// An egress profile that names no host, by asking for an aperture without one or naming one with
/// no host, is refused before anything starts: no driver opens and the workspace is untouched.
#[test]
fn an_egress_profile_with_no_named_host_is_refused() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let outside = fixture.path().join("ran-outside");
    let script = format!("touch target/ran; touch '{}'", outside.display());
    let child = || ChildCommand::new("sh", ["-c", script.as_str()], &workspace);
    // A directory that is no cgroup: a launch that got past the refusal would be refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 9));
    for (child, case) in [
        (child().with_network(NetworkMode::Aperture), "no aperture"),
        (
            child().with_egress(EgressAperture {
                host: String::new(),
                ..model_aperture(address)
            }),
            "an aperture with an empty host",
        ),
        (
            child().with_egress(EgressAperture {
                host: " ".to_owned(),
                ..model_aperture(address)
            }),
            "an aperture with a blank host",
        ),
    ] {
        let error = runner.run_child(&child).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{case}: {error}"
        );
        assert!(error.detail.contains("no host"), "{case}: {error}");
    }
    assert!(
        !workspace.join("target").exists(),
        "the workspace is untouched and no driver was opened"
    );
    assert!(!outside.exists(), "nothing ran unconfined");
}

/// The applied confinement names the aperture the child got, and the driver's record is the
/// aperture declared; the same runner's child with no egress still reports no network. A record
/// reporting a wider network than the request is refused as a test's is: no child can make
/// Substrate report one, so `confinement.rs` holds that refusal
/// (`a_record_reporting_another_network_than_requested_is_refused`).
#[test]
fn the_applied_record_names_the_egress() {
    delegated(
        "the_applied_record_names_the_egress",
        |runner, workspace| {
            let (_named, address) = loopback_host();
            let child = ChildCommand::new("true", [] as [&str; 0], workspace)
                .with_egress(model_aperture(address));
            assert_eq!(child.egress(), Some(&model_aperture(address)));
            let Some(result) = egress_result(runner.run_child(&child)) else {
                return;
            };
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.applied.network, format!("aperture:model@{address}"));
            assert_eq!(result.applied.profile, "workspace");
            assert_eq!(result.applied.writable_scopes, ["target"]);
            assert!(result.applied.read_only_roots.is_empty());
            let record = result.driver_record.as_ref().expect("the driver's record");
            let AppliedNetwork::Aperture(applied) = &record.network else {
                panic!("the driver applied an aperture: {:?}", record.network);
            };
            assert_eq!(applied.name, "model");
            assert_eq!(applied.destination, address.to_string());
            assert_eq!(applied.mechanism, ApertureMechanism::LoopbackForwarder);
            assert_eq!(applied.max_bytes, None);
            assert_eq!(record.cgroup, result.applied.cgroup);

            let plain = runner
                .run_child(&ChildCommand::new("true", [] as [&str; 0], workspace))
                .expect("a child with no egress runs beside it");
            assert_eq!(plain.applied.network, "none");
            assert_eq!(
                plain.driver_record.as_ref().map(|record| &record.network),
                Some(&AppliedNetwork::None)
            );
            assert_ne!(plain.applied.profile_digest, result.applied.profile_digest);
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
