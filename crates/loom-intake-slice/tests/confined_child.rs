//! A child process the host names runs confined through Substrate (`SubstrateRunner::run_child`),
//! with the test runner's guarantees: a missing guarantee is a typed refusal and nothing runs, the
//! child's environment holds only what the host named, it writes only `target` and the workspace
//! paths the host named, and the applied confinement comes back with the result. A child also runs
//! live (`SubstrateRunner::spawn_child`): the host writes its standard input, reads its two
//! streams as they arrive, signals it and waits for its end, under the same checks.
//!
//! The delegated cases need bubblewrap and a delegated cgroup v2 scope. Each starts its own test
//! binary again under `systemd-run --user --scope -p Delegate=yes`, as `b10x-loom` starts itself,
//! and runs the case there, through the harness in `tests/support`. Where bubblewrap, a user
//! systemd manager or the Substrate driver is absent, the case prints
//! `skipped: missing prerequisite: …` and returns; such a skip establishes no qualification. The
//! egress cases also skip naming what is missing where `bash` is absent or
//! Substrate does not serve an egress aperture on this machine, and the trust-anchor case where
//! `openssl` or `curl` is. Their hosts are loopback listeners the test starts; nothing connects
//! past loopback.

mod support;

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    Backend, ChildCommand, ChildError, ChildExit, ChildSignal, ChildStream, ConfinedChild,
    ConfinementError, ConfinementRefusal, EgressAperture, ExecEnvironment, NetworkMode,
    SandboxProfile, SubstrateRunner, TestExecution, TestRunner as _,
};
use b10x_loom_intake_slice::executor::TestCommand;
use substrate_wire::{APERTURE_CA_BUNDLE_PATH, ApertureMechanism, AppliedNetwork};
use support::{bubblewrap_present, skip};

/// A variable the process that launches the child holds and the host never names.
const UNNAMED: &str = "LOOM_CONFINED_CHILD_UNNAMED";

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

/// Run `case` against the real backend in a delegated scope, or skip naming what is missing; the
/// process that launches the child holds [`UNNAMED`].
fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
    support::delegated(name, &[(UNNAMED, "leak")], case);
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

/// The machine's TLS tools the trust-anchor case needs: `openssl` makes its test CAs and serves its
/// host, and `curl` is the child that verifies that host's certificate.
const OPENSSL: &str = "/usr/bin/openssl";
const CURL: &str = "/usr/bin/curl";

/// Make, in `directory`, two test CAs, `trusted.crt` and `other.crt`, each a CA bundle of one
/// certificate, and `host.crt` with `host.key`, a certificate for [`NAMED_HOST`] signed by
/// `trusted`. Each is valid for a day, and no key leaves the directory.
fn make_test_cas(directory: &Path) {
    let openssl = |args: &[&str]| {
        let output = Command::new(OPENSSL)
            .args(args)
            .current_dir(directory)
            .stdin(Stdio::null())
            .output()
            .expect("openssl starts");
        assert!(
            output.status.success(),
            "openssl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    // A self-signed certificate with a new P-256 key, valid for a day.
    let new = [
        "req",
        "-x509",
        "-nodes",
        "-days",
        "1",
        "-newkey",
        "ec",
        "-pkeyopt",
        "ec_paramgen_curve:prime256v1",
    ];
    for ca in ["trusted", "other"] {
        let (key, certificate) = (format!("{ca}.key"), format!("{ca}.crt"));
        let subject = format!("/CN=loom test CA {ca}");
        let mut args = new.to_vec();
        args.extend([
            "-keyout",
            key.as_str(),
            "-out",
            certificate.as_str(),
            "-subj",
            subject.as_str(),
        ]);
        args.extend(["-addext", "basicConstraints=critical,CA:TRUE"]);
        args.extend(["-addext", "keyUsage=critical,keyCertSign"]);
        openssl(&args);
    }
    let subject = format!("/CN={NAMED_HOST}");
    let names = format!("subjectAltName=DNS:{NAMED_HOST}");
    let mut args = new.to_vec();
    args.extend([
        "-keyout",
        "host.key",
        "-out",
        "host.crt",
        "-subj",
        subject.as_str(),
    ]);
    args.extend(["-addext", names.as_str()]);
    args.extend(["-addext", "basicConstraints=critical,CA:FALSE"]);
    args.extend(["-addext", "extendedKeyUsage=serverAuth"]);
    // Signed by `trusted` instead of by itself.
    args.extend(["-CA", "trusted.crt", "-CAkey", "trusted.key"]);
    openssl(&args);
}

/// `openssl s_server` answering HTTPS with `host.crt` on the loopback address `ip`, at the port the
/// kernel picked and the server reports; killed and reaped when dropped.
struct TlsHost {
    server: std::process::Child,
    address: SocketAddr,
}

impl TlsHost {
    fn start(directory: &Path, ip: Ipv4Addr) -> Self {
        let mut server = Command::new(OPENSSL)
            .args(["s_server", "-www", "-cert", "host.crt", "-key", "host.key"])
            .arg("-accept")
            .arg(format!("{ip}:0"))
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("openssl s_server starts");
        let stdout = server.stdout.take().expect("the server's output is piped");
        let (port, reported) = std::sync::mpsc::channel();
        let accept = format!("ACCEPT {ip}:");
        // Read on until the server ends, so it never blocks on a full pipe.
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(number) = line.strip_prefix(&accept).and_then(|p| p.parse().ok()) {
                    let _ = port.send(number);
                }
            }
        });
        match reported.recv_timeout(Duration::from_secs(30)) {
            Ok(port) => Self {
                server,
                address: SocketAddr::from((ip, port)),
            },
            Err(error) => {
                let _ = server.kill();
                let _ = server.wait();
                panic!("openssl s_server reported no port it accepts on: {error}");
            }
        }
    }
}

impl Drop for TlsHost {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}

/// A `curl` child fetching `https://model.example.test:<port>/`, verifying the host's certificate
/// against whatever bundle it trusts and printing the HTTP status, `000` when there was none.
fn curl_child(workspace: &Path, port: u16) -> ChildCommand {
    let url = format!("https://{NAMED_HOST}:{port}/");
    let args = [
        "--silent",
        "--show-error",
        "--max-time",
        "20",
        "--output",
        "/dev/null",
        "--write-out",
        "status:%{http_code}\n",
        url.as_str(),
    ];
    ChildCommand::new(CURL, args, workspace)
}

/// The story's case: a child under an egress aperture verifies its host's certificate against the
/// CA bundle the host names, and against no other. The host, `model.example.test` on 127.0.0.2,
/// serves a certificate a test CA signed; `curl` reaches it through the aperture and verifies it
/// when the host names that CA's bundle, and fails certificate verification (curl's exit status
/// 60) when the host names another CA's bundle. Every run is in one workspace directory through one
/// runner, so the second can fail only on a driver of its own: two children naming different
/// bundles never share a driver. A child naming no bundle has no trust anchor and fails too, and
/// the first bundle, named again, is trusted again.
#[test]
fn a_child_under_an_aperture_trusts_the_named_ca_bundle() {
    delegated(
        "a_child_under_an_aperture_trusts_the_named_ca_bundle",
        |runner, workspace| {
            for tool in [OPENSSL, CURL] {
                if !Path::new(tool).is_file() {
                    return skip(&format!("executable {tool}"));
                }
            }
            let anchors = tempfile::tempdir().unwrap();
            make_test_cas(anchors.path());
            let trusted = anchors.path().join("trusted.crt");
            let other = anchors.path().join("other.crt");
            let host = TlsHost::start(anchors.path(), Ipv4Addr::new(127, 0, 0, 2));
            let aperture = EgressAperture {
                host: NAMED_HOST.to_owned(),
                ..model_aperture(host.address)
            };
            let child = curl_child(workspace, host.address.port()).with_egress(aperture);

            let named = child.clone().with_ca_bundle(&trusted);
            assert_eq!(named.ca_bundle(), Some(trusted.as_path()));
            let Some(verified) = egress_result(runner.run_child(&named)) else {
                return;
            };
            assert_eq!(verified.exit_code, Some(0), "{}", verified.output);
            assert_eq!(verified.output, "status:200\n");

            let refused = runner
                .run_child(&child.clone().with_ca_bundle(&other))
                .expect("the child runs confined with its egress");
            assert_eq!(
                refused.exit_code,
                Some(60),
                "certificate verification fails: {}",
                refused.output
            );
            assert!(
                refused.output.starts_with("status:000\ncurl: (60) "),
                "{}",
                refused.output
            );

            assert_eq!(child.ca_bundle(), None);
            let unanchored = runner
                .run_child(&child)
                .expect("the child runs confined with its egress");
            // 77: no bundle to read; 60: none that verifies the host.
            assert!(
                matches!(unanchored.exit_code, Some(60 | 77)),
                "a child with no trust anchor fails to verify: {:?} {}",
                unanchored.exit_code,
                unanchored.output
            );
            assert!(
                unanchored.output.starts_with("status:000\n"),
                "{}",
                unanchored.output
            );

            let again = runner
                .run_child(&child.with_ca_bundle(&trusted))
                .expect("the child runs confined with its egress");
            assert_eq!(
                (again.exit_code, again.output.as_str()),
                (Some(0), "status:200\n")
            );
        },
    );
}

/// A child naming a CA bundle finds it where `SSL_CERT_FILE` says, the one variable it gets beside
/// the environment the host named: a read-only snapshot of the host's file, taken when the child
/// starts, so the next child sees the file as it is then and the host's file is never written.
#[test]
fn a_child_finds_the_snapshot_of_its_ca_bundle_through_ssl_cert_file() {
    delegated(
        "a_child_finds_the_snapshot_of_its_ca_bundle_through_ssl_cert_file",
        |runner, workspace| {
            let (_host, address) = loopback_host();
            let anchors = tempfile::tempdir().unwrap();
            let bundle = anchors.path().join("bundle.pem");
            std::fs::write(&bundle, "first anchor\n").unwrap();
            let anchored = |child: ChildCommand| {
                child
                    .with_environment(named())
                    .with_egress(model_aperture(address))
                    .with_ca_bundle(&bundle)
            };
            let env = anchored(ChildCommand::new("env", [] as [&str; 0], workspace));
            let Some(environment) = egress_result(runner.run_child(&env)) else {
                return;
            };
            assert_eq!(environment.exit_code, Some(0), "{}", environment.output);
            assert_eq!(
                environment.output,
                format!("FIRST=one\nSECOND=two\nSSL_CERT_FILE={APERTURE_CA_BUNDLE_PATH}\n")
            );

            let script = "cat \"$SSL_CERT_FILE\"; \
                          if (printf changed >> \"$SSL_CERT_FILE\") 2>/dev/null; \
                          then echo writable; else echo read-only; fi";
            let read = anchored(ChildCommand::new("sh", ["-c", script], workspace));
            let first = runner.run_child(&read).expect("the child runs confined");
            assert_eq!(
                (first.exit_code, first.output.as_str()),
                (Some(0), "first anchor\nread-only\n")
            );
            std::fs::write(&bundle, "second anchor\n").unwrap();
            let second = runner.run_child(&read).expect("the child runs confined");
            assert_eq!(
                (second.exit_code, second.output.as_str()),
                (Some(0), "second anchor\nread-only\n")
            );
            assert_eq!(std::fs::read_to_string(&bundle).unwrap(), "second anchor\n");
        },
    );
}

/// The variables through which a TLS client takes another trust store than the bundle Substrate
/// points `SSL_CERT_FILE` at, replacing or widening it; a child naming a bundle is refused when its
/// environment names any of them.
const TRUST_STORE_VARIABLES: [&str; 13] = [
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "CURL_CA_BUNDLE",
    "CURL_CA_PATH",
    "CURL_HOME",
    "REQUESTS_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "GIT_SSL_CAPATH",
    "GIT_SSL_NO_VERIFY",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_PARAMETERS",
];

/// Git configuration keys a `GIT_CONFIG_KEY_<n>` may name beside a bundle and be refused for:
/// `http.sslCAInfo`, `http.sslCAPath` and `http.sslVerify`, section and key in any case, with
/// or without a URL subsection.
const GIT_TRUST_KEYS: [&str; 5] = [
    "http.sslCAInfo",
    "HTTP.SSLCAPATH",
    "http.sslverify",
    "http.https://model.example.test/.sslCAInfo",
    "Http.https://model.example.test:443/a.b.SslVerify",
];

/// Substrate's ceiling on a certificate bundle, 1 MiB.
const BUNDLE_CEILING: usize = 1024 * 1024;

/// A CA bundle the host cannot name is refused `ScopeInvalid` before anything starts, naming the
/// path: a relative path, one that does not exist, a symbolic link (to a regular file too, as
/// Substrate's daemon refuses one), a directory, a socket, a FIFO (refused without waiting for a
/// writer), an empty file, one larger than Substrate's 1 MiB and one this process cannot read; a
/// bundle named without an egress aperture, which would trust nothing the child can reach; and a
/// bundle beside an environment naming any variable that replaces or widens a TLS client's trust
/// store ([`TRUST_STORE_VARIABLES`]), or a `GIT_CONFIG_KEY_<n>` naming one of Git's
/// ([`GIT_TRUST_KEYS`]), the refusal naming the variable. Each refusal leaves the workspace as it
/// was, without `target`. A well-named bundle beside an aperture, one of exactly 1 MiB included,
/// and one beside Git configuration naming other keys, get past every check, to the missing
/// backend.
#[test]
fn a_ca_bundle_the_host_cannot_name_is_refused_before_anything_starts() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let anchors = fixture.path().join("anchors");
    std::fs::create_dir(&anchors).unwrap();
    let bundle = anchors.join("bundle.pem");
    std::fs::write(&bundle, "anchor\n").unwrap();
    let largest = anchors.join("largest.pem");
    std::fs::write(&largest, vec![b'a'; BUNDLE_CEILING]).unwrap();
    std::fs::write(anchors.join("large.pem"), vec![b'a'; BUNDLE_CEILING + 1]).unwrap();
    let fifo = anchors.join("fifo.pem");
    let fifo_made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|status| status.success());
    std::fs::write(anchors.join("empty.pem"), "").unwrap();
    std::os::unix::fs::symlink(&bundle, anchors.join("link.pem")).unwrap();
    let _socket = UnixListener::bind(anchors.join("socket")).unwrap();
    let unreadable = anchors.join("unreadable.pem");
    std::fs::write(&unreadable, "anchor\n").unwrap();
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
    // A process that reads it anyway (root) has nothing to refuse.
    let unreadable_here = std::fs::File::open(&unreadable).is_err();

    let ran = fixture.path().join("ran");
    let script = format!("touch '{}'", ran.display());
    let plain = || ChildCommand::new("sh", ["-c", script.as_str()], &workspace);
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 9));
    let child = || plain().with_egress(model_aperture(address));
    // A directory that is no cgroup: a launch that got past the refusal would be refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let mut refused = vec![
        (PathBuf::from("anchors/bundle.pem"), child(), "relative"),
        (anchors.join("absent.pem"), child(), "does not exist"),
        (anchors.join("link.pem"), child(), "symbolic link"),
        (anchors.clone(), child(), "no regular file"),
        (anchors.join("socket"), child(), "no regular file"),
        (anchors.join("empty.pem"), child(), "empty"),
        (anchors.join("large.pem"), child(), "more than"),
        (bundle.clone(), plain(), "no egress aperture"),
    ];
    for variable in TRUST_STORE_VARIABLES {
        let environment = ExecEnvironment {
            allow: Vec::new(),
            set: BTreeMap::from([(variable.to_owned(), "/workspace/ca.pem".to_owned())]),
        };
        refused.push((
            bundle.clone(),
            child().with_environment(environment),
            variable,
        ));
    }
    let git_config = |pairs: &[(&str, &str)]| ExecEnvironment {
        allow: Vec::new(),
        set: pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    };
    for key in GIT_TRUST_KEYS {
        let environment = git_config(&[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", key),
            ("GIT_CONFIG_VALUE_0", "/workspace/ca.pem"),
        ]);
        refused.push((
            bundle.clone(),
            child().with_environment(environment),
            "GIT_CONFIG_KEY_0",
        ));
    }
    if fifo_made {
        refused.push((fifo.clone(), child(), "no regular file"));
    }
    if unreadable_here {
        refused.push((unreadable.clone(), child(), "cannot be read"));
    }
    for (path, child, named) in refused {
        let error = runner.run_child(&child.with_ca_bundle(&path)).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{path:?}: {error}"
        );
        assert!(
            error.detail.contains(&format!("`{}`", path.display())),
            "{path:?}: {error}"
        );
        assert!(error.detail.contains(named), "{path:?}: {error}");
        assert!(
            !workspace.join("target").exists(),
            "{path:?}: a refused bundle leaves the workspace as it was"
        );
    }
    assert!(!ran.exists(), "nothing ran");

    let expected = if bubblewrap_present() {
        ConfinementRefusal::CgroupUndelegated
    } else {
        ConfinementRefusal::BackendMissing
    };
    for accepted in [&bundle, &largest] {
        let error = runner
            .run_child(&child().with_ca_bundle(accepted))
            .unwrap_err();
        assert_eq!(error.reason, expected, "{accepted:?}: {error}");
    }
    // Git configuration naming other keys stays allowed, a value naming a trust key included.
    let other_keys = git_config(&[
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "safe.directory"),
        ("GIT_CONFIG_VALUE_0", "http.sslCAInfo"),
        ("GIT_CONFIG_KEY_1", "http.sslBackend"),
        ("GIT_CONFIG_VALUE_1", "openssl"),
    ]);
    let error = runner
        .run_child(&child().with_environment(other_keys).with_ca_bundle(&bundle))
        .unwrap_err();
    assert_eq!(error.reason, expected, "{error}");
    assert!(!ran.exists(), "nothing ran");
}

/// A bundle path the host named and then swapped for a symbolic link before the launch is never
/// followed: the launch reads the bundle once, through an open with its last component not
/// followed, so it is refused `ScopeInvalid` and nothing starts. The child never sees the link's target, either in
/// that launch or in the next one, which reads the regular file put back and trusts its bytes as
/// read then.
#[test]
fn a_bundle_swapped_for_a_link_after_it_was_named_is_never_followed() {
    delegated(
        "a_bundle_swapped_for_a_link_after_it_was_named_is_never_followed",
        |runner, workspace| {
            let (_host, address) = loopback_host();
            let anchors = tempfile::tempdir().unwrap();
            let bundle = anchors.path().join("bundle.pem");
            let elsewhere = anchors.path().join("elsewhere.pem");
            std::fs::write(&bundle, "named anchor\n").unwrap();
            std::fs::write(&elsewhere, "link target\n").unwrap();
            let read = ChildCommand::new("sh", ["-c", "cat \"$SSL_CERT_FILE\""], workspace)
                .with_egress(model_aperture(address))
                .with_ca_bundle(&bundle);
            let Some(first) = egress_result(runner.run_child(&read)) else {
                return;
            };
            assert_eq!(
                (first.exit_code, first.output.as_str()),
                (Some(0), "named anchor\n")
            );

            std::fs::remove_file(&bundle).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &bundle).unwrap();
            let marker = workspace.join("target/ran");
            let swapped = ChildCommand::new(
                "sh",
                ["-c", "cat \"$SSL_CERT_FILE\"; touch target/ran"],
                workspace,
            )
            .with_egress(model_aperture(address))
            .with_ca_bundle(&bundle);
            let error = runner
                .run_child(&swapped)
                .expect_err("a bundle swapped for a link is refused");
            assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
            assert!(error.detail.contains("symbolic link"), "{error}");
            assert!(!marker.exists(), "nothing started");

            std::fs::remove_file(&bundle).unwrap();
            std::fs::write(&bundle, "put back\n").unwrap();
            let after = runner.run_child(&read).expect("the child runs confined");
            assert_eq!(
                (after.exit_code, after.output.as_str()),
                (Some(0), "put back\n")
            );
        },
    );
}

/// Read `child`'s frames until both its streams closed, within 60 s: what each stream carried.
fn read_to_end(child: &mut ConfinedChild) -> (Vec<u8>, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    loop {
        assert!(
            Instant::now() < deadline,
            "the child's streams closed within 60 s"
        );
        match child.read(Duration::from_secs(1)) {
            Ok(Some(frame)) => match frame.stream {
                ChildStream::Stdout => stdout.extend(frame.bytes),
                ChildStream::Stderr => stderr.extend(frame.bytes),
            },
            Ok(None) => return (stdout, stderr),
            Err(ChildError::ReadTimedOut) => {}
            Err(error) => panic!("the read failed: {error}"),
        }
    }
}

/// Read `child`'s standard output until it holds `expected`, within 30 s; nothing arrives on its
/// standard error meanwhile.
fn read_stdout(child: &mut ConfinedChild, expected: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut stdout = Vec::new();
    while stdout.len() < expected.len() {
        assert!(
            Instant::now() < deadline,
            "{expected:?} came back within 30 s, read {stdout:?}"
        );
        match child.read(Duration::from_secs(1)) {
            Ok(Some(frame)) => {
                assert_eq!(frame.stream, ChildStream::Stdout, "{frame:?}");
                stdout.extend(frame.bytes);
            }
            Ok(None) => panic!("the child's output closed after {stdout:?}"),
            Err(ChildError::ReadTimedOut) => {}
            Err(error) => panic!("the read failed: {error}"),
        }
    }
    assert_eq!(stdout, expected);
}

/// The story's case: `cat` started live reads what the host writes to its standard input and
/// writes it back, frame by frame, as it arrives; closing its standard input ends it with exit 0,
/// and its result carries the confinement applied and both streams whole.
#[test]
fn a_live_child_echoes_what_it_is_written() {
    delegated(
        "a_live_child_echoes_what_it_is_written",
        |runner, workspace| {
            let mut child = runner
                .spawn_child(&ChildCommand::new("cat", [] as [&str; 0], workspace))
                .expect("cat starts live, confined");
            child.write_stdin(b"first\n").unwrap();
            read_stdout(&mut child, b"first\n");
            child.write_stdin(b"second\n").unwrap();
            read_stdout(&mut child, b"second\n");
            child.close_stdin().unwrap();
            assert_eq!(read_to_end(&mut child), (Vec::new(), Vec::new()));
            let result = child.wait().expect("the child's result");
            assert_eq!(
                result.exit,
                ChildExit {
                    exit_code: Some(0),
                    signal: None,
                    timed_out: false
                }
            );
            assert_eq!(result.output.stdout, b"first\nsecond\n");
            assert!(result.output.stderr.is_empty());
            assert!(!result.output.stdout_truncated && !result.output.stderr_truncated);
            assert_eq!(result.applied.backend, Backend::Substrate);
            assert_eq!(result.applied.network, "none");
            assert_eq!(result.applied.writable_scopes, ["target"]);
            assert_eq!(result.applied.profile_digest.len(), 64);
            assert_eq!(result.driver_record.network, AppliedNetwork::None);
            assert_eq!(result.driver_record.cgroup, result.applied.cgroup);
        },
    );
}

/// The story's case: a live child's standard output and standard error arrive as frames of their
/// own stream, never merged, and come back apart in its result.
#[test]
fn a_live_child_keeps_its_streams_apart() {
    delegated(
        "a_live_child_keeps_its_streams_apart",
        |runner, workspace| {
            let mut child = runner
                .spawn_child(&ChildCommand::new(
                    "sh",
                    [
                        "-c",
                        "printf out; printf err >&2; printf more; printf less >&2",
                    ],
                    workspace,
                ))
                .expect("the shell starts live");
            assert_eq!(
                read_to_end(&mut child),
                (b"outmore".to_vec(), b"errless".to_vec())
            );
            let result = child.wait().expect("the child's result");
            assert_eq!(result.exit.exit_code, Some(0));
            assert_eq!(result.output.stdout, b"outmore");
            assert_eq!(result.output.stderr, b"errless");
        },
    );
}

/// The story's case: a live child the host kills ends at once with signal 9, well before its
/// timeout, and is a result.
#[test]
fn a_live_child_can_be_killed() {
    delegated("a_live_child_can_be_killed", |runner, workspace| {
        let mut child = runner
            .spawn_child(&ChildCommand::new("sleep", ["600"], workspace))
            .expect("sleep starts live");
        let started = Instant::now();
        child
            .signal(ChildSignal::Kill)
            .expect("the child is killed");
        let result = child.wait().expect("a child the host killed is a result");
        assert_eq!(
            result.exit,
            ChildExit {
                exit_code: None,
                signal: Some(9),
                timed_out: false
            }
        );
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "killed in {:?}",
            started.elapsed()
        );
    });
}

/// A live child is bounded by its timeout as a child run to its end is: past it, the child is
/// ended and its result is marked timed out, with no exit status.
#[test]
fn a_live_child_is_ended_at_its_timeout() {
    delegated(
        "a_live_child_is_ended_at_its_timeout",
        |runner, workspace| {
            let started = Instant::now();
            let child = runner
                .spawn_child(
                    &ChildCommand::new("sleep", ["600"], workspace)
                        .with_timeout(Duration::from_millis(500)),
                )
                .expect("sleep starts live");
            let result = child.wait().expect("a timed-out child is a result");
            assert!(result.exit.timed_out, "{:?}", result.exit);
            assert_eq!(result.exit.exit_code, None);
            assert!(started.elapsed() < Duration::from_secs(30));
        },
    );
}

/// A live child dropped before it ended is killed: it never reaches what it would have done next,
/// and the runner then opens a driver for another directory, which it refuses while a launch still
/// runs under the same cgroup root.
#[test]
fn a_dropped_live_child_is_killed() {
    delegated("a_dropped_live_child_is_killed", |runner, workspace| {
        let started = workspace.join("target/started");
        let child = runner
            .spawn_child(&ChildCommand::new(
                "sh",
                ["-c", "touch target/started; sleep 2; touch target/survived"],
                workspace,
            ))
            .expect("the shell starts live");
        let deadline = Instant::now() + Duration::from_secs(30);
        while !started.exists() {
            assert!(Instant::now() < deadline, "the child started within 30 s");
            std::thread::sleep(Duration::from_millis(50));
        }
        drop(child);
        std::thread::sleep(Duration::from_secs(3));
        assert!(
            !workspace.join("target/survived").exists(),
            "the dropped child was killed"
        );
        // A driver opening on the cgroup root kills what runs there, so it opens only now.
        let elsewhere = workspace.parent().unwrap().join("other/elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let after = runner
            .run_child(&ChildCommand::new("true", [] as [&str; 0], &elsewhere))
            .expect("the dropped child no longer counts as a launch running there");
        assert_eq!(after.exit_code, Some(0), "{}", after.output);
    });
}

/// The story's case: a child run to its end gets the bytes the host gave it on its standard input
/// once, then end of file.
#[test]
fn run_child_passes_standard_input_once() {
    delegated(
        "run_child_passes_standard_input_once",
        |runner, workspace| {
            let result = runner
                .run_child(&ChildCommand::new("cat", [] as [&str; 0], workspace).with_stdin(b"abc"))
                .expect("cat runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.output, "abc");
            assert_eq!(result.child_output.stdout, b"abc");
            assert!(result.child_output.stderr.is_empty());
            assert_eq!(result.applied.writable_scopes, ["target"]);
            let empty = runner
                .run_child(&ChildCommand::new("cat", [] as [&str; 0], workspace))
                .expect("cat runs confined");
            assert_eq!(
                (empty.exit_code, empty.child_output.stdout.as_slice()),
                (Some(0), b"".as_slice()),
                "without stdin the child reads an empty, closed input"
            );
        },
    );
}

/// The story's case: a child run to its end returns its standard output and standard error whole
/// and apart, each with its own truncation flag; `output` keeps the tail of each, as before.
#[test]
fn run_child_returns_both_streams_whole() {
    delegated(
        "run_child_returns_both_streams_whole",
        |runner, workspace| {
            let script = "i=0; while [ $i -lt 2000 ]; do printf oooooooooo; printf eeeeeeeeee >&2; i=$((i+1)); done";
            let result = runner
                .run_child(&ChildCommand::new("sh", ["-c", script], workspace))
                .expect("the shell runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.child_output.stdout, vec![b'o'; 20_000]);
            assert_eq!(result.child_output.stderr, vec![b'e'; 20_000]);
            assert!(!result.child_output.stdout_truncated);
            assert!(!result.child_output.stderr_truncated);
            assert_eq!(
                result.output,
                format!("{}{}", "o".repeat(8192), "e".repeat(8192)),
                "`output` is the tail of each stream, as before"
            );
        },
    );
}

/// More standard input than a pipe session admits is refused before anything starts, as a live
/// child given standard input through `with_stdin` is (it is written through `write_stdin`); at
/// the limit the launch goes on to the driver. A live child is checked before launch as a child
/// run to its end is.
#[test]
fn stdin_over_the_input_limit_is_refused_before_launch() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    // A directory that is no cgroup: no driver is established, so a launch past the checks is
    // refused naming the missing cgroup or backend, never `ScopeInvalid`.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let limit = ChildCommand::MAX_INPUT_BYTES;
    let over =
        ChildCommand::new("cat", [] as [&str; 0], &workspace).with_stdin(vec![b'x'; limit + 1]);
    let error = runner.run_child(&over).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains(&limit.to_string()), "{error}");
    assert!(
        !workspace.join("target").exists(),
        "the workspace is untouched and no driver was opened"
    );
    let at = ChildCommand::new("cat", [] as [&str; 0], &workspace).with_stdin(vec![b'x'; limit]);
    let error = runner.run_child(&at).unwrap_err();
    assert_ne!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");

    let error = runner
        .spawn_child(&ChildCommand::new("cat", [] as [&str; 0], &workspace).with_stdin(b"abc"))
        .expect_err("a live child is written through write_stdin");
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    let error = runner
        .spawn_child(&ChildCommand::new(
            "GH_TOKEN=synthetic-value",
            ["/usr/bin/env"],
            &workspace,
        ))
        .expect_err("a program env reads as an assignment is refused");
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(!error.detail.contains("synthetic-value"), "{error}");
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
