//! Security review of `story:several-egress-apertures`: conformance checks of the launch refusals
//! a set of egress apertures gets before anything starts, and of what the applied confinement
//! projection says about a run's network.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;

use b10x_loom_intake_slice::confinement::{
    Backend, ChildCommand, ConfinementError, ConfinementRefusal, EgressAperture, ExecEnvironment,
    MAX_EGRESS_APERTURES, SubstrateRunner, TestRunner as _,
};
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};

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

fn model() -> EgressAperture {
    aperture("model", "api.example.test", "10.0.0.1:443")
}

fn search() -> EgressAperture {
    aperture("search", "search.example.test", "10.0.0.2:443")
}

/// What `run_child` answers for `child(workspace)`, with a cgroup root that is no cgroup, so a
/// launch past every check before launch is refused for that; and whether `target/` was created.
fn launch(child: impl FnOnce(&Path) -> ChildCommand) -> (ConfinementError, bool) {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let child = child(&workspace);
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let error = runner.run_child(&child).unwrap_err();
    (error, workspace.join("target").exists())
}

fn with_all(workspace: &Path, apertures: &[EgressAperture]) -> ChildCommand {
    apertures.iter().cloned().fold(
        ChildCommand::new("true", [] as [&str; 0], workspace),
        ChildCommand::with_egress,
    )
}

fn assert_scope_refused(apertures: &[EgressAperture], named: &str) {
    let (error, started) = launch(|workspace| with_all(workspace, apertures));
    assert_eq!(
        error.reason,
        ConfinementRefusal::ScopeInvalid,
        "{named}: {error}"
    );
    assert!(error.detail.contains(named), "{named}: {error}");
    assert!(!started, "refused before anything starts: {error}");
}

/// A later aperture may not name its host by a literal or `localhost` however it is spelled: a
/// trailing `.` or another case is the same host to the resolver.
#[test]
fn security_a_later_aperture_naming_a_loopback_host_in_another_spelling_is_refused() {
    for host in ["127.0.0.1.", "LOCALHOST.", "localhost.", "127.0.0.2"] {
        let later = aperture("search", host, "127.0.0.1:443");
        assert_scope_refused(&[model(), later], &format!("names the host {host}"));
    }
}

/// Two apertures naming one host are refused when only case or a trailing `.` tells them apart,
/// so no host name maps to two forwarders.
#[test]
fn security_two_apertures_naming_one_host_in_another_spelling_are_refused() {
    let same = aperture("search", "API.example.TEST.", "10.0.0.2:443");
    let (error, started) = launch(|workspace| with_all(workspace, &[model(), same]));
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(!started, "{error}");
    let fourth = aperture("fourth", "SEARCH.example.test", "10.0.0.4:443");
    let third = aperture("third", "third.example.test", "10.0.0.3:443");
    assert_scope_refused(&[model(), search(), third, fourth], "search.example.test");
}

/// A later aperture naming a numeric host that resolvers read as an IPv4 address is refused too.
#[test]
fn security_a_later_aperture_naming_a_numeric_host_is_refused() {
    for host in ["2130706434", "127.2", "0x7f.0.0.2"] {
        let later = aperture("search", host, "10.0.0.2:443");
        let (error, started) = launch(|workspace| with_all(workspace, &[model(), later]));
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{host}: {error}"
        );
        assert!(!started, "{host}: {error}");
    }
}

/// Exactly `MAX_EGRESS_APERTURES` distinct, well-formed apertures pass every check before launch
/// (refused only for the cgroup root that is no cgroup), and one more is refused before launch.
#[test]
fn security_the_aperture_bound_is_exactly_substrates() {
    let most = usize::try_from(MAX_EGRESS_APERTURES).unwrap();
    let set: Vec<_> = (1..=most + 1)
        .map(|n| {
            aperture(
                &format!("api{n}"),
                &format!("api{n}.example.test"),
                &format!("10.0.0.{n}:443"),
            )
        })
        .collect();
    let (error, _) = launch(|workspace| with_all(workspace, &set[..most]));
    assert_ne!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert_scope_refused(&set, &format!("{} egress apertures", most + 1));
}

/// The certificate bundle rules hold for a child given several apertures: a trust-store variable
/// named beside the bundle is refused before anything starts.
#[test]
fn security_a_trust_store_variable_beside_a_bundle_is_refused_with_several_apertures() {
    let anchors = tempfile::tempdir().unwrap();
    let bundle = anchors.path().join("bundle.pem");
    std::fs::write(&bundle, b"-----BEGIN CERTIFICATE-----\n").unwrap();
    for variable in ["SSL_CERT_DIR", "GIT_SSL_NO_VERIFY", "CURL_CA_BUNDLE"] {
        let environment = ExecEnvironment {
            allow: Vec::new(),
            set: BTreeMap::from([(variable.to_owned(), "/workspace/ca.pem".to_owned())]),
        };
        let (error, started) = launch(|workspace| {
            with_all(workspace, &[model(), search()])
                .with_environment(environment)
                .with_ca_bundle(&bundle)
        });
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{variable}: {error}"
        );
        assert!(error.detail.contains(variable), "{variable}: {error}");
        assert!(!started, "{variable}: {error}");
    }
}

/// An unconfined run (`--confinement none`, `UnconfinedRunner`) reaches any network. Its applied
/// confinement must not report the network a confined run with no network reports: the
/// specification reads an empty list as no network (`ess/intake/domains/confinement.yaml`,
/// `intake.confinement.AppliedAperture`), and 0.36.0 reported `"unconfined"` here. Compared by
/// its debug form, so the check holds whatever type a fix gives the field.
#[test]
fn security_an_unconfined_run_does_not_report_no_network() {
    let workspace = tempfile::tempdir().unwrap();
    let result = UnconfinedRunner
        .run(&TestCommand::new("true", [] as [&str; 0]), workspace.path())
        .expect("the unconfined runner runs `true`");
    assert_eq!(result.applied.backend, Backend::None);
    let no_network: Vec<b10x_loom_intake_slice::confinement::AppliedAperture> = Vec::new();
    assert_ne!(
        format!("{:?}", result.applied.network),
        format!("{no_network:?}"),
        "an unconfined run's network is reported as no network"
    );
}
