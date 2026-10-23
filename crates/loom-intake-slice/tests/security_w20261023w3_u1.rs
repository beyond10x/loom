//! Security review of `story:confinement-spec`: a `ConfinementSpec` grants a child nothing the
//! builders would refuse, and the checks a launch makes for builder calls run, with the same
//! refusal kind, for a command `ChildCommand::confined` built.
//!
//! Nothing here needs bubblewrap: every refusal read happens before anything starts, on a runner
//! whose cgroup root is no cgroup, so a launch past every check is refused for that instead and
//! never `ScopeInvalid`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use b10x_loom_intake_slice::confinement::{
    BaselineEnvironment, ChildAperture, ChildCommand, ChildEnvironment, ChildNetwork, ChildProfile,
    ConfinementError, ConfinementRefusal, ConfinementSpec, EgressAperture, EnvironmentEntry,
    ExecEnvironment, SubstrateRunner, WritableRoot,
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

fn spec_aperture(name: &str, host: &str, pinned: &str) -> ChildAperture {
    let address: SocketAddr = pinned.parse().unwrap();
    ChildAperture {
        name: name.to_owned(),
        host: host.to_owned(),
        port: i64::from(address.port()),
        pinned: pinned.to_owned(),
        max_bytes: None,
    }
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

fn launch_refusal(base: &Path, command: &ChildCommand) -> ConfinementError {
    SubstrateRunner::new(Some(base.to_owned()))
        .run_child(command)
        .unwrap_err()
}

fn workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("loom-spec-sec-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let base = directory.path().canonicalize().unwrap();
    let workspace = base.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    (directory, base, workspace)
}

fn entry(name: &str, value: &str) -> EnvironmentEntry {
    EnvironmentEntry {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

/// `ChildCommand::confined`'s own contract: "A spec naming nothing leaves the command as it was."
/// A command a host already confined through the builders (an environment, an aperture) and then
/// given a spec naming nothing must be that same command.
#[test]
fn security_a_spec_naming_nothing_leaves_a_built_command_as_it_was() {
    let (_held, _base, workspace) = workspace();
    let built = ChildCommand::new("true", Vec::<String>::new(), &workspace)
        .with_environment(ExecEnvironment {
            allow: vec![BaselineEnvironment::Path],
            set: BTreeMap::from([("LANGUAGE".to_owned(), "C".to_owned())]),
        })
        .with_egress(substrate_aperture(
            "api",
            "api.example.test",
            "10.0.0.1:443",
        ));
    let confined = built.clone().confined(&empty()).unwrap();
    assert_eq!(
        confined, built,
        "a spec naming nothing changed the command it was applied to"
    );
}

/// A trust-store variable beside a certificate bundle is refused at launch for a spec-built
/// command as for the builders, naming the variable.
#[test]
fn security_a_spec_cannot_override_its_bundle_through_the_environment() {
    let (_held, base, workspace) = workspace();
    for (name, value, named) in [
        ("SSL_CERT_FILE", "/elsewhere.pem", "SSL_CERT_FILE"),
        ("GIT_SSL_NO_VERIFY", "1", "GIT_SSL_NO_VERIFY"),
        ("GIT_CONFIG_KEY_0", "HTTP.sslVerify", "GIT_CONFIG_KEY_0"),
    ] {
        let spec = ConfinementSpec {
            environment: ChildEnvironment {
                allow: Vec::new(),
                set: vec![entry(name, value)],
            },
            egress: vec![spec_aperture("api", "api.example.test", "10.0.0.1:443")],
            ca_bundle: Some("/srv/ca.pem".to_owned()),
            ..empty()
        };
        let command = ChildCommand::new("true", Vec::<String>::new(), &workspace)
            .confined(&spec)
            .unwrap();
        let error = launch_refusal(&base, &command);
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{name}: {error}"
        );
        assert!(error.detail.contains(named), "{name}: {error}");
        assert!(
            !workspace.join("target").exists(),
            "{name}: refused before start"
        );
    }
}

/// A later aperture naming `localhost` or a numeric host in any spelling `inet_aton` reads is
/// refused at launch for a spec-built command, naming the host.
#[test]
fn security_a_spec_cannot_smuggle_a_numeric_or_localhost_later_aperture() {
    let (_held, base, workspace) = workspace();
    for (host, pinned) in [
        ("localhost", "127.0.0.1:443"),
        ("0x7f000001", "127.0.0.1:443"),
        ("127.1", "127.0.0.1:443"),
        ("10.0.0.2", "10.0.0.2:443"),
    ] {
        let spec = ConfinementSpec {
            egress: vec![
                spec_aperture("api", "api.example.test", "10.0.0.1:443"),
                spec_aperture("later", host, pinned),
            ],
            ..empty()
        };
        let command = ChildCommand::new("true", Vec::<String>::new(), &workspace)
            .confined(&spec)
            .unwrap();
        let error = launch_refusal(&base, &command);
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{host}: {error}"
        );
        assert!(error.detail.contains(host), "{host}: {error}");
    }
}

/// `network: Aperture` with no aperture is refused at launch, as `with_network` alone is: a spec
/// requests no network but none or the named apertures.
#[test]
fn security_a_spec_asking_for_an_aperture_without_one_is_refused() {
    let (_held, base, workspace) = workspace();
    let spec = ConfinementSpec {
        network: ChildNetwork::Aperture,
        ..empty()
    };
    let command = ChildCommand::new("true", Vec::<String>::new(), &workspace)
        .confined(&spec)
        .unwrap();
    let error = launch_refusal(&base, &command);
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
}

/// A program `/usr/bin/env` would read as an assignment or an option stays refused for a
/// spec-built command, and a `writable_roots` entry is refused before the spec is applied
/// whatever else it names.
#[test]
fn security_program_and_writable_root_refusals_hold_through_the_spec() {
    let (_held, base, workspace) = workspace();
    for program in ["-i", "PATH=/evil"] {
        let command = ChildCommand::new(program, Vec::<String>::new(), &workspace)
            .confined(&empty())
            .unwrap();
        let error = launch_refusal(&base, &command);
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{program}: {error}"
        );
    }
    let spec = ConfinementSpec {
        writable_roots: vec![WritableRoot {
            host_path: workspace.to_str().unwrap().to_owned(),
            mount: "/opt/w".to_owned(),
        }],
        egress: vec![spec_aperture("api", "api.example.test", "10.0.0.1:443")],
        ..empty()
    };
    let error = ChildCommand::new("true", Vec::<String>::new(), &workspace)
        .confined(&spec)
        .unwrap_err();
    assert_eq!(
        error.reason,
        ConfinementRefusal::CapabilityUnserved,
        "{error}"
    );
    assert!(error.detail.contains("`writable_roots`"), "{error}");
}
