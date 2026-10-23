//! A host declares a child's confinement as one `ConfinementSpec` (`story:confinement-spec`), and
//! `ChildCommand::confined` applies it through the builders: the spec and the same builder calls
//! give equal commands, a field the pinned Substrate cannot honour is refused naming it, and the
//! builders' limits hold through the spec.
//!
//! Nothing here needs bubblewrap: the cases compare commands, and the refusals they read happen
//! before anything starts, on a runner whose cgroup root is no cgroup, so a launch past every check
//! is refused for that instead and never `ScopeInvalid`.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use b10x_loom_intake_slice::confinement::{
    BaselineEnvironment, BaselineVariable, ChildAperture, ChildCommand, ChildEnvironment,
    ChildNetwork, ChildProfile, ConfinementError, ConfinementRefusal, ConfinementSpec,
    EgressAperture, EnvironmentEntry, ExecEnvironment, MAX_EGRESS_APERTURES, NetworkMode,
    ReadOnlyFile, SandboxProfile, SubstrateRunner, ToolchainRoot, WritableRoot,
};

/// A spec that names nothing: the confinement `ChildCommand::new` already has.
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

/// The aperture `name`, naming `host` and pinned at `pinned`, as the spec declares it.
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

/// The same aperture as Substrate declares it.
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

/// The refusal `run_child` gives `command`, on a runner whose cgroup root is no cgroup.
fn launch_refusal(base: &Path, command: &ChildCommand) -> ConfinementError {
    let runner = SubstrateRunner::new(Some(base.to_owned()));
    runner.run_child(command).unwrap_err()
}

/// A canonical directory under the test target, so mounts and host paths are canonical.
fn host_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("loom-spec-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let canonical = directory.path().canonicalize().unwrap();
    (directory, canonical)
}

fn text(path: &Path) -> String {
    path.to_str().expect("a test path is UTF-8").to_owned()
}

#[test]
fn a_spec_equals_its_builders() {
    let workspace = PathBuf::from("/srv/workspace");
    assert_eq!(
        child(&workspace).confined(&empty()).unwrap(),
        child(&workspace),
        "a spec naming nothing leaves the command as new made it"
    );

    let spec = ConfinementSpec {
        read_only_roots: vec![
            ToolchainRoot {
                host_path: "/srv/rust".to_owned(),
                mount: "/opt/rust".to_owned(),
            },
            ToolchainRoot {
                host_path: "/srv/cache".to_owned(),
                mount: "/opt/cache".to_owned(),
            },
        ],
        read_only_files: vec![ReadOnlyFile {
            host_path: "/srv/config.toml".to_owned(),
            mount: "/etc/tool/config.toml".to_owned(),
        }],
        host_paths: vec!["/srv/checkout".to_owned()],
        writable: vec!["scratch".to_owned(), ".git".to_owned()],
        writable_roots: Vec::new(),
        workdir: Some("/workspace/scratch".to_owned()),
        environment: ChildEnvironment {
            allow: vec![BaselineVariable::Path, BaselineVariable::Lang],
            set: vec![
                EnvironmentEntry {
                    name: "HOME".to_owned(),
                    value: "/workspace".to_owned(),
                },
                EnvironmentEntry {
                    name: "MODE".to_owned(),
                    value: "spec".to_owned(),
                },
            ],
        },
        profile: ChildProfile::Workspace,
        network: ChildNetwork::None,
        egress: vec![
            spec_aperture("model", "api.example.test", "10.0.0.1:443"),
            ChildAperture {
                max_bytes: Some(4096),
                ..spec_aperture("search", "search.example.test", "10.0.0.2:8443")
            },
        ],
        ca_bundle: Some("/srv/ca.pem".to_owned()),
        timeout_ms: Some(1500),
        stdin: Some(b"input".to_vec()),
    };
    let built = child(&workspace)
        .with_read_only_root("/srv/rust", "/opt/rust")
        .with_read_only_root("/srv/cache", "/opt/cache")
        .with_read_only_file("/srv/config.toml", "/etc/tool/config.toml")
        .at_host_path("/srv/checkout")
        .with_writable("scratch")
        .with_writable(".git")
        .with_environment(ExecEnvironment {
            allow: vec![BaselineEnvironment::Path, BaselineEnvironment::Lang],
            set: BTreeMap::from([
                ("HOME".to_owned(), "/workspace".to_owned()),
                ("MODE".to_owned(), "spec".to_owned()),
            ]),
        })
        .with_profile(SandboxProfile::Workspace)
        .with_network(NetworkMode::None)
        .with_egress(substrate_aperture(
            "model",
            "api.example.test",
            "10.0.0.1:443",
        ))
        .with_egress(EgressAperture {
            max_bytes: Some(4096),
            ..substrate_aperture("search", "search.example.test", "10.0.0.2:8443")
        })
        .with_ca_bundle("/srv/ca.pem")
        .with_timeout(Duration::from_millis(1500))
        .with_stdin(b"input".to_vec())
        .with_workdir("/workspace/scratch");
    let confined = child(&workspace).confined(&spec).unwrap();
    assert_eq!(confined, built, "the spec is its builder calls");
    assert_eq!(confined.network(), NetworkMode::Aperture);
    assert_eq!(confined.egress().len(), 2);

    // Each variant of the enumerations maps to its Substrate counterpart, one to one.
    let every_variable = ConfinementSpec {
        environment: ChildEnvironment {
            allow: vec![
                BaselineVariable::Lang,
                BaselineVariable::LcAll,
                BaselineVariable::Path,
                BaselineVariable::Term,
                BaselineVariable::Tz,
            ],
            set: Vec::new(),
        },
        network: ChildNetwork::Aperture,
        ..empty()
    };
    assert_eq!(
        child(&workspace).confined(&every_variable).unwrap(),
        child(&workspace)
            .with_environment(ExecEnvironment {
                allow: vec![
                    BaselineEnvironment::Lang,
                    BaselineEnvironment::LcAll,
                    BaselineEnvironment::Path,
                    BaselineEnvironment::Term,
                    BaselineEnvironment::Tz,
                ],
                set: BTreeMap::new(),
            })
            .with_network(NetworkMode::Aperture),
    );
}

#[test]
fn a_field_the_pin_cannot_honour_is_refused_naming_it() {
    let workspace = PathBuf::from("/srv/workspace");
    let spec = ConfinementSpec {
        writable_roots: vec![WritableRoot {
            host_path: "/srv/shared".to_owned(),
            mount: "/mnt/shared".to_owned(),
        }],
        ..empty()
    };
    let error = child(&workspace).confined(&spec).unwrap_err();
    assert_eq!(
        error.reason,
        ConfinementRefusal::CapabilityUnserved,
        "{error}"
    );
    assert!(error.detail.contains("`writable_roots`"), "{error}");
}

/// A field whose value the builder's own type cannot carry is refused naming the field, never
/// narrowed or dropped: a port outside 0 to 65535, a pinned address that is no socket address, a
/// negative byte ceiling or timeout, and one variable set twice, which one environment cannot hold.
#[test]
fn a_value_the_builder_type_cannot_carry_is_refused_naming_its_field() {
    let workspace = PathBuf::from("/srv/workspace");
    let aperture = || spec_aperture("model", "api.example.test", "10.0.0.1:443");
    let cases = [
        (
            ConfinementSpec {
                egress: vec![
                    aperture(),
                    ChildAperture {
                        port: 65_536,
                        ..spec_aperture("search", "search.example.test", "10.0.0.2:443")
                    },
                ],
                ..empty()
            },
            "`egress[1].port`",
        ),
        (
            ConfinementSpec {
                egress: vec![ChildAperture {
                    port: -1,
                    ..aperture()
                }],
                ..empty()
            },
            "`egress[0].port`",
        ),
        (
            ConfinementSpec {
                egress: vec![ChildAperture {
                    pinned: "api.example.test:443".to_owned(),
                    ..aperture()
                }],
                ..empty()
            },
            "`egress[0].pinned`",
        ),
        (
            ConfinementSpec {
                egress: vec![ChildAperture {
                    max_bytes: Some(-1),
                    ..aperture()
                }],
                ..empty()
            },
            "`egress[0].max_bytes`",
        ),
        (
            ConfinementSpec {
                timeout_ms: Some(-1),
                ..empty()
            },
            "`timeout_ms`",
        ),
        (
            ConfinementSpec {
                environment: ChildEnvironment {
                    allow: Vec::new(),
                    set: vec![
                        EnvironmentEntry {
                            name: "MODE".to_owned(),
                            value: "a".to_owned(),
                        },
                        EnvironmentEntry {
                            name: "MODE".to_owned(),
                            value: "b".to_owned(),
                        },
                    ],
                },
                ..empty()
            },
            "`environment.set`",
        ),
    ];
    for (spec, named) in cases {
        let error = child(&workspace).confined(&spec).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{named}: {error}"
        );
        assert!(error.detail.contains(named), "{named}: {error}");
    }

    // What the builder's type carries goes to the builder, whose launch decides it: a port of 0
    // and a byte ceiling of 0 are the launch's to refuse, not the spec's.
    let zero = ConfinementSpec {
        egress: vec![ChildAperture {
            port: 0,
            max_bytes: Some(0),
            ..aperture()
        }],
        ..empty()
    };
    let confined = child(&workspace).confined(&zero).unwrap();
    assert_eq!(confined.egress()[0].port, 0);
    assert_eq!(confined.egress()[0].max_bytes, Some(0));
}

/// The builders' limits hold through the spec: at most 4 read-only roots and 16 files, at most 16
/// MiB of standard input and none unless named, and at most 4 egress apertures. Past each the
/// launch refuses the spec's command `ScopeInvalid` before anything starts, as it refuses the
/// builders'; at each it gets past every check.
#[test]
fn the_builders_limits_hold_through_the_spec() {
    let (_held, base) = host_directory();
    let workspace = base.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let roots: Vec<ToolchainRoot> = (0..=ChildCommand::MAX_READ_ONLY_ROOTS)
        .map(|n| {
            let root = base.join(format!("root{n}"));
            std::fs::create_dir(&root).unwrap();
            ToolchainRoot {
                host_path: text(&root),
                mount: format!("/opt/root{n}"),
            }
        })
        .collect();
    let files: Vec<ReadOnlyFile> = (0..=ChildCommand::MAX_READ_ONLY_FILES)
        .map(|n| {
            let file = base.join(format!("file{n}.toml"));
            std::fs::write(&file, "a file").unwrap();
            ReadOnlyFile {
                host_path: text(&file),
                mount: format!("/opt/file{n}.toml"),
            }
        })
        .collect();
    let apertures: Vec<ChildAperture> = (1..=MAX_EGRESS_APERTURES + 1)
        .map(|n| {
            spec_aperture(
                &format!("api{n}"),
                &format!("api{n}.example.test"),
                &format!("10.0.0.{n}:443"),
            )
        })
        .collect();
    let limit = ChildCommand::MAX_INPUT_BYTES;
    let with = |spec: ConfinementSpec| child(&workspace).confined(&spec).unwrap();
    let roots_of = |count: usize| ConfinementSpec {
        read_only_roots: roots[..count].to_vec(),
        ..empty()
    };
    let files_of = |count: usize| ConfinementSpec {
        read_only_files: files[..count].to_vec(),
        ..empty()
    };
    let apertures_of = |count: usize| ConfinementSpec {
        egress: apertures[..count].to_vec(),
        ..empty()
    };
    let stdin_of = |bytes: usize| ConfinementSpec {
        stdin: Some(vec![b'x'; bytes]),
        ..empty()
    };
    let max_apertures = usize::try_from(MAX_EGRESS_APERTURES).unwrap();

    assert_eq!(
        with(empty()).stdin(),
        None,
        "standard input is absent by default"
    );

    let past = [
        (
            roots_of(ChildCommand::MAX_READ_ONLY_ROOTS + 1),
            roots[ChildCommand::MAX_READ_ONLY_ROOTS].host_path.clone(),
        ),
        (
            files_of(ChildCommand::MAX_READ_ONLY_FILES + 1),
            files[ChildCommand::MAX_READ_ONLY_FILES].host_path.clone(),
        ),
        (stdin_of(limit + 1), limit.to_string()),
        (
            apertures_of(max_apertures + 1),
            format!("{} egress apertures", max_apertures + 1),
        ),
    ];
    for (spec, named) in past {
        let command = with(spec);
        let error = launch_refusal(&base, &command);
        assert!(
            !workspace.join("target").exists(),
            "{named}: refused before anything starts: {error}"
        );
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{named}: {error}"
        );
        assert!(error.detail.contains(&named), "{named}: {error}");
    }
    for spec in [
        roots_of(ChildCommand::MAX_READ_ONLY_ROOTS),
        files_of(ChildCommand::MAX_READ_ONLY_FILES),
        stdin_of(limit),
        apertures_of(max_apertures),
    ] {
        let command = with(spec);
        let error = launch_refusal(&base, &command);
        assert_ne!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    }
}
