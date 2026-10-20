//! Adversary cases for `story:harness-child-egress-profile` (wave 2026-10-20-w2, U5).
//!
//! The delegated cases run against the real backend in a delegated cgroup v2 scope, the way
//! `tests/confined_child.rs` does: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes` and runs the case there. Where bubblewrap, a user
//! systemd manager, the Substrate driver, `bash` or Substrate's egress aperture is absent, the case
//! prints `skipped: missing prerequisite: …` and returns. Every host is a loopback listener the
//! test starts; nothing connects past loopback.

use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, SubstrateRunner,
    TestExecution, prepare_delegated_cgroup,
};
use substrate_wire::AppliedNetwork;

const DELEGATED: &str = "LOOM_ADV_W20261020W2_U5_DELEGATED";
const REPORT: &str = "LOOM_ADV_W20261020W2_U5_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";
const BASH: &str = "/usr/bin/bash";
/// The start of the refusal `run_child` gives when Substrate cannot serve the egress aperture.
const EGRESS_UNSERVED: &str = "Substrate does not serve the egress aperture";

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
        if !Path::new(BASH).is_file() {
            std::fs::write(&report, format!("executable {BASH}")).unwrap();
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

/// Inside the delegated binary: `None` once the case skipped because Substrate does not serve the
/// egress aperture on this machine; otherwise the result as `run_child` returned it.
fn egress(
    result: Result<TestExecution, ConfinementError>,
) -> Option<Result<TestExecution, ConfinementError>> {
    match result {
        Err(error)
            if error.reason == ConfinementRefusal::CapabilityUnserved
                && error.detail.starts_with(EGRESS_UNSERVED) =>
        {
            let report = std::env::var_os(REPORT).expect("a case skips only when delegated");
            std::fs::write(report, format!("Substrate egress apertures ({error})")).unwrap();
            None
        }
        result => Some(result),
    }
}

/// A loopback listener the test starts, standing in for a host, and its address.
fn loopback_host() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
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

/// Answer up to `connections` connections to `listener` with `answer`, for at most 60 s; the
/// thread returns how many it answered. A write the peer cut short is not an error here.
fn serve(
    listener: TcpListener,
    answer: Vec<u8>,
    connections: usize,
) -> std::thread::JoinHandle<usize> {
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut answered = 0;
        while answered < connections && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    let _ = stream.write_all(&answer);
                    answered += 1;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("the named host accepts: {error}"),
            }
        }
        answered
    })
}

/// A declared byte ceiling is part of the aperture `with_egress` names (`max_bytes`), and Substrate
/// ends the run that reaches it with `exec.aperture-byte-limit`, as it ends one that reaches its
/// deadline with `exec.timeout`. A child its timeout ended is a result marked timed out
/// (`confined_child.rs::a_timed_out_child_is_a_result_with_no_exit_status`); a child its byte
/// ceiling ended must likewise be a result that says so, not a `CapabilityUnserved` refusal that
/// reads as a guarantee Substrate did not serve and drops the record of the bytes that crossed.
#[test]
fn adv_a_child_its_byte_ceiling_stopped_is_reported_as_such() {
    delegated(
        "adv_a_child_its_byte_ceiling_stopped_is_reported_as_such",
        |runner, workspace| {
            let (named, address) = loopback_host();
            let served = serve(named, vec![b'x'; 65_536], 1);
            let script = format!(
                "/usr/bin/cat < /dev/tcp/127.0.0.1/{} > /dev/null; echo drained",
                address.port()
            );
            let child = ChildCommand::new(BASH, ["-c", script.as_str()], workspace)
                .with_egress(EgressAperture {
                    max_bytes: Some(1024),
                    ..model_aperture(address)
                })
                .with_timeout(Duration::from_secs(60));
            let Some(result) = egress(runner.run_child(&child)) else {
                return;
            };
            let result = result.unwrap_or_else(|error| {
                panic!(
                    "a child its byte ceiling stopped is a result naming the ceiling, as a timed-out child is a result: {error}"
                )
            });
            assert_eq!(served.join().unwrap(), 1, "the named host was reached");
            assert_eq!(result.exit_code, None, "{}", result.output);
            assert!(!result.timed_out, "{}", result.output);
            assert!(
                result.output.contains("exec.aperture-byte-limit"),
                "the result names the ceiling that ended it: {}",
                result.output
            );
            let record = result.driver_record.as_ref().expect("the driver's record");
            let AppliedNetwork::Aperture(applied) = &record.network else {
                panic!("the driver applied the aperture: {:?}", record.network);
            };
            assert_eq!(applied.max_bytes, Some(1024));
            assert!(
                applied.bytes.to_destination + applied.bytes.from_destination >= 1024,
                "{applied:?}"
            );
        },
    );
}

/// A listener on the loopback address `ip` of this machine (any of 127.0.0.0/8), and its address.
fn host_at(ip: Ipv4Addr) -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind((ip, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    (listener, address)
}

/// The story's acceptance (`a_child_reaches_only_the_named_egress`): under the egress profile a
/// connection to the named host is allowed. `with_egress` admits an IPv4 literal as the host when
/// it is the pinned address (`check_host`), and Substrate maps only a host *name* to its in-sandbox
/// listener on 127.0.0.1; so a child that dials the literal it names reaches it only when that
/// literal happens to be 127.0.0.1, the one address the story's case uses. Here the named host is
/// 127.0.0.2, still loopback: the declaration is either refused before anything starts, or the
/// child reaches the host at the address it names.
#[test]
fn adv_a_child_reaches_an_ipv4_literal_host_at_the_address_it_names() {
    delegated(
        "adv_a_child_reaches_an_ipv4_literal_host_at_the_address_it_names",
        |runner, workspace| {
            let (named, address) = host_at(Ipv4Addr::new(127, 0, 0, 2));
            let served = serve(named, b"named-host\n".to_vec(), 1);
            let script = format!(
                "read -r line < /dev/tcp/127.0.0.2/{}; printf 'named:%s\\n' \"$line\"",
                address.port()
            );
            let child = ChildCommand::new(BASH, ["-c", script.as_str()], workspace)
                .with_egress(model_aperture(address))
                .with_timeout(Duration::from_secs(60));
            let Some(result) = egress(runner.run_child(&child)) else {
                return;
            };
            match result {
                Err(error) if error.reason == ConfinementRefusal::ScopeInvalid => {}
                Err(error) => panic!("refused, but not before anything started: {error}"),
                Ok(result) => {
                    assert_eq!(
                        result.output, "named:named-host\n",
                        "the child reached the IPv4 literal it names (exit {:?})",
                        result.exit_code
                    );
                    assert_eq!(served.join().unwrap(), 1, "the named host was reached");
                }
            }
        },
    );
}

/// `AGENTS.md` and the CHANGELOG promise that an aperture naming no host or port, or one
/// Substrate's daemon would not declare, is refused `ScopeInvalid` before anything starts. The
/// daemon declares an aperture only from `name=host:port/tcp` whose host resolves, and pins it at
/// that port; the embedded driver writes `host` verbatim into the child's `/etc/hosts`. A host
/// that is no host name, and a destination pinned to port 0, are declarations it never makes.
#[test]
fn adv_an_aperture_the_daemon_would_never_declare_is_refused_before_anything_starts() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    // A directory that is no cgroup: a launch that got past the refusal is refused for it instead.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 443));
    let child = |aperture: EgressAperture| {
        ChildCommand::new("true", [] as [&str; 0], &workspace).with_egress(aperture)
    };
    let cases = [
        (
            "a host carrying a second hosts line",
            EgressAperture {
                host: "model.example.test\n127.0.0.1\tother.example.test".to_owned(),
                ..model_aperture(address)
            },
        ),
        (
            "a host with a space",
            EgressAperture {
                host: "model example test".to_owned(),
                ..model_aperture(address)
            },
        ),
        (
            "a host carrying its port",
            EgressAperture {
                host: "model.example.test:443".to_owned(),
                ..model_aperture(address)
            },
        ),
        (
            "a host that is a URL",
            EgressAperture {
                host: "https://model.example.test/".to_owned(),
                ..model_aperture(address)
            },
        ),
        (
            "a destination pinned to port 0",
            EgressAperture {
                pinned: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                ..model_aperture(address)
            },
        ),
    ];
    let answered: Vec<(&str, Option<ConfinementRefusal>, String)> = cases
        .into_iter()
        .map(
            |(case, aperture)| match runner.run_child(&child(aperture)) {
                Ok(result) => (case, None, format!("ran: {result:?}")),
                Err(error) => (case, Some(error.reason), error.detail),
            },
        )
        .collect();
    let not_refused: Vec<_> = answered
        .iter()
        .filter(|(_, reason, _)| *reason != Some(ConfinementRefusal::ScopeInvalid))
        .collect();
    assert!(
        not_refused.is_empty(),
        "refused ScopeInvalid before anything starts, not: {not_refused:#?}"
    );
    assert!(!workspace.join("target").exists(), "nothing ran");
}
