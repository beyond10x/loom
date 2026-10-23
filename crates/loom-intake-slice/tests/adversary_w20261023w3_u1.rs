//! Adversary cases for `story:confinement-spec`: `ChildCommand::confined` against the builders it
//! stands for and against what its documentation says of it.
//!
//! `ChildCommand::confined` and the SDK stability page both say "a spec naming nothing leaves the
//! command as it was". The spec's list fields and optional fields are applied only when named, but
//! `environment`, `profile` and `network` are applied always, so a command that already named an
//! environment or an egress aperture comes back changed by a spec naming nothing.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use b10x_loom_intake_slice::confinement::{
    BaselineEnvironment, ChildAperture, ChildCommand, ChildEnvironment, ChildNetwork, ChildProfile,
    ConfinementRefusal, ConfinementSpec, EgressAperture, EnvironmentEntry, ExecEnvironment,
    NetworkMode, SubstrateRunner,
};

fn empty() -> ConfinementSpec {
    ConfinementSpec {
        read_only_roots: Vec::new(),
        read_only_files: Vec::new(),
        host_paths: Vec::new(),
        writable: Vec::new(),
        writable_roots: Vec::new(),
        workdir: None,
        environment: ChildEnvironment {
            allow: Vec::new(),
            set: Vec::new(),
        },
        profile: ChildProfile::Workspace,
        network: ChildNetwork::None,
        egress: Vec::new(),
        ca_bundle: None,
        timeout_ms: None,
        stdin: None,
    }
}

fn child(workspace: &Path) -> ChildCommand {
    ChildCommand::new("true", ["--flag"], workspace)
}

fn substrate_aperture(name: &str, host: &str, pinned: &str) -> EgressAperture {
    let pinned: SocketAddr = pinned.parse().unwrap();
    EgressAperture {
        name: name.to_owned(),
        host: host.to_owned(),
        port: pinned.port(),
        pinned,
        max_bytes: None,
    }
}

fn host_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("loom-adv-spec-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let canonical = directory.path().canonicalize().unwrap();
    (directory, canonical)
}

/// The documented promise, on a command whose environment was named by its builder: a spec naming
/// nothing must leave it as it was. `confined` replaces it with the spec's empty environment.
#[test]
fn adversary_a_spec_naming_nothing_leaves_an_environment_already_named() {
    let workspace = PathBuf::from("/srv/workspace");
    let named = child(&workspace).with_environment(ExecEnvironment {
        allow: vec![BaselineEnvironment::Path],
        set: BTreeMap::from([("MODE".to_owned(), "check".to_owned())]),
    });
    let confined = named.clone().confined(&empty()).unwrap();
    assert_eq!(
        confined.environment(),
        named.environment(),
        "a spec naming nothing leaves the command as it was (ChildCommand::confined, sdk-stability.md)"
    );
}

/// The same promise on a command that already reaches an aperture: `confined` keeps its egress
/// list but sets its network back to `None`, a command no single builder call produces.
#[test]
fn adversary_a_spec_naming_nothing_leaves_an_aperture_already_named() {
    let workspace = PathBuf::from("/srv/workspace");
    let named = child(&workspace).with_egress(substrate_aperture(
        "model",
        "api.example.test",
        "10.0.0.1:443",
    ));
    let confined = named.clone().confined(&empty()).unwrap();
    assert_eq!(
        (confined.network(), confined.egress().len()),
        (NetworkMode::Aperture, 1),
        "a spec naming nothing leaves the command as it was: network {:?} beside {} aperture(s)",
        confined.network(),
        confined.egress().len()
    );
}

/// Held: values the builder's types carry go to the builder unchanged, and the launch refuses the
/// spec's command exactly as it refuses the builders'. An IPv6 `pinned`, a timeout of zero, two
/// variable names differing only in case, and `network: Aperture` with no aperture.
#[test]
fn adversary_values_the_builders_carry_are_judged_by_the_launch_alike() {
    let (_held, base) = host_directory();
    let workspace = base.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let runner = SubstrateRunner::new(Some(base.clone()));

    let v6 = ConfinementSpec {
        egress: vec![ChildAperture {
            name: "model".to_owned(),
            host: "api.example.test".to_owned(),
            port: 443,
            pinned: "[::1]:443".to_owned(),
            max_bytes: None,
        }],
        timeout_ms: Some(0),
        environment: ChildEnvironment {
            allow: Vec::new(),
            set: vec![
                EnvironmentEntry {
                    name: "Mode".to_owned(),
                    value: "a".to_owned(),
                },
                EnvironmentEntry {
                    name: "MODE".to_owned(),
                    value: "b".to_owned(),
                },
            ],
        },
        ..empty()
    };
    let spec_command = child(&workspace).confined(&v6).unwrap();
    let built = child(&workspace)
        .with_environment(ExecEnvironment {
            allow: Vec::new(),
            set: BTreeMap::from([
                ("Mode".to_owned(), "a".to_owned()),
                ("MODE".to_owned(), "b".to_owned()),
            ]),
        })
        .with_egress(EgressAperture {
            name: "model".to_owned(),
            host: "api.example.test".to_owned(),
            port: 443,
            pinned: "[::1]:443".parse().unwrap(),
            max_bytes: None,
        })
        .with_timeout(Duration::ZERO);
    assert_eq!(spec_command, built);
    let from_spec = runner.run_child(&spec_command).unwrap_err();
    let from_builders = runner.run_child(&built).unwrap_err();
    assert_eq!(
        from_spec.reason,
        ConfinementRefusal::ScopeInvalid,
        "{from_spec}"
    );
    assert_eq!(from_spec.detail, from_builders.detail);

    let bare_aperture = ConfinementSpec {
        network: ChildNetwork::Aperture,
        ..empty()
    };
    let spec_command = child(&workspace).confined(&bare_aperture).unwrap();
    let error = runner.run_child(&spec_command).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(!workspace.join("target").exists());
}
