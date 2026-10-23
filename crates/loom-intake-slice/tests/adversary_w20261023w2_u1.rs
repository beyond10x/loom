//! Adversary cases for `story:several-egress-apertures`: a second, third or fourth aperture whose
//! host is an IPv4 address in a numeric spelling the C resolver reads as one (`inet_aton`'s hex
//! forms, as `0x7f000001`), which neither `Ipv4Addr`'s parser nor the host-name rule sees as an
//! address. Substrate maps such a host to its own forwarder in `/etc/hosts`, but glibc parses the
//! numeric form before it reads `/etc/hosts`, so the child dials the address itself: `127.0.0.1`
//! is the first aperture's forwarder.
//!
//! The delegated case runs against the real backend through `tests/support`, as the
//! confined-child cases do, and skips naming what is missing; its hosts are loopback listeners.

mod support;

use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::Path;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, SubstrateRunner,
};
use support::skip;

fn aperture(name: &str, host: &str, pinned: &str) -> EgressAperture {
    let pinned: SocketAddr = pinned.parse().unwrap();
    EgressAperture {
        name: name.to_owned(),
        host: host.to_owned(),
        port: pinned.port(),
        pinned,
        max_bytes: None,
    }
}

/// The refusal `run_child` gives `apertures` with a cgroup root that is no cgroup, so nothing
/// launches whichever way the checks go; `Err` holds any other refusal, past the checks.
fn scope_refusal(apertures: &[EgressAperture]) -> Result<ConfinementError, ConfinementError> {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let child = apertures.iter().cloned().fold(
        ChildCommand::new("true", [] as [&str; 0], &workspace),
        ChildCommand::with_egress,
    );
    let error = SubstrateRunner::new(Some(fixture.path().to_owned()))
        .run_child(&child)
        .unwrap_err();
    if error.reason == ConfinementRefusal::ScopeInvalid {
        Ok(error)
    } else {
        Err(error)
    }
}

/// Every one of these is an IPv4 address to glibc's `getaddrinfo` (`getent ahostsv4` answers
/// `127.0.0.1` for the first three and `10.0.0.2` for the last), and none has an all-digit last
/// label, so the host-name rule takes each as a name.
const NUMERIC_HOSTS: [&str; 4] = ["0x7f000001", "0x7f.0x1", "0x7f.0.0.0x1", "0x0a000002"];

#[test]
fn adversary_a_later_aperture_naming_its_host_by_a_hex_ipv4_form_is_refused_naming_the_host() {
    let mut passed = Vec::new();
    for host in NUMERIC_HOSTS {
        let apertures = [
            aperture("model", "api.example.test", "10.0.0.1:443"),
            aperture("search", host, "10.0.0.2:443"),
        ];
        match scope_refusal(&apertures) {
            Ok(error) => assert!(error.detail.contains(host), "{host}: {error}"),
            Err(error) => passed.push(format!("{host} -> {:?}: {error}", error.reason)),
        }
    }
    assert!(
        passed.is_empty(),
        "a later aperture naming an IPv4 literal is refused ScopeInvalid before launch, yet these got past every check:\n{}",
        passed.join("\n")
    );
}

/// Answer one connection to `listener` with `answer`, waiting at most 30 s.
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
                Err(error) => panic!("the host accepts: {error}"),
            }
        }
        false
    })
}

const BASH: &str = "/usr/bin/bash";
const EGRESS_UNSERVED: &str = "Substrate does not serve the egress aperture";

/// The consequence, live: two apertures on one port, the second naming its host `0x7f000001`.
/// A child dialling the second aperture's host by its name reaches the first aperture's
/// destination. Passes when the set is refused `ScopeInvalid` before launch, or when the child
/// reaches the second aperture's destination.
#[test]
fn adversary_a_child_dialling_a_hex_named_aperture_never_reaches_the_first_apertures_host() {
    support::delegated(
        "adversary_a_child_dialling_a_hex_named_aperture_never_reaches_the_first_apertures_host",
        &[],
        |runner, workspace| {
            if !Path::new(BASH).is_file() {
                return skip(&format!("executable {BASH}"));
            }
            let first = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 2), 0)).unwrap();
            let first_at = first.local_addr().unwrap();
            let Ok(second) = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 3), first_at.port()))
            else {
                return skip("a free port shared by 127.0.0.2 and 127.0.0.3");
            };
            let second_at = second.local_addr().unwrap();
            let first_answered = answer_once(first, b"first-host\n");
            let second_answered = answer_once(second, b"second-host\n");
            let script = format!(
                "read -r line < /dev/tcp/0x7f000001/{port}; printf 'dialled:%s\\n' \"$line\"",
                port = second_at.port()
            );
            let child = ChildCommand::new(BASH, ["-c", script.as_str()], workspace)
                .with_egress(aperture(
                    "model",
                    "model.example.test",
                    &first_at.to_string(),
                ))
                .with_egress(aperture("search", "0x7f000001", &second_at.to_string()));
            let result = match runner.run_child(&child) {
                Err(error) if error.reason == ConfinementRefusal::ScopeInvalid => return,
                Err(error)
                    if error.reason == ConfinementRefusal::CapabilityUnserved
                        && error.detail.starts_with(EGRESS_UNSERVED) =>
                {
                    return skip(&format!("Substrate egress apertures ({error})"));
                }
                result => result.expect("the child runs confined with its egress"),
            };
            let first_reached = first_answered.join().unwrap();
            let second_reached = second_answered.join().unwrap();
            assert!(
                !first_reached && second_reached,
                "the second aperture's host `0x7f000001` carried the child to the first aperture's destination: output {:?}, first reached {first_reached}, second reached {second_reached}",
                result.output
            );
        },
    );
}
