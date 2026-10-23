//! A child given several egress apertures (`story:several-egress-apertures`): each refusal a set
//! of apertures gets before anything starts, and what the request asks Substrate for.

use super::*;

/// The aperture `name`, naming `host` and pinned at `pinned`, at the pinned port.
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

/// The refusal `run_child` gives `apertures`, in a workspace that exists and with a cgroup root
/// that is no cgroup, so a launch that got past every check before launch would be refused for
/// that instead; the workspace is untouched.
fn refusal(apertures: &[EgressAperture]) -> ConfinementError {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let child = apertures.iter().cloned().fold(
        ChildCommand::new("true", [] as [&str; 0], &workspace),
        ChildCommand::with_egress,
    );
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let error = runner.run_child(&child).unwrap_err();
    assert!(
        !workspace.join("target").exists(),
        "refused before anything starts: {error}"
    );
    error
}

/// Assert `apertures` are refused `ScopeInvalid`, the refusal containing `named`.
fn assert_refused(apertures: &[EgressAperture], named: &str) {
    let error = refusal(apertures);
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains(named), "{named}: {error}");
}

#[test]
fn more_than_four_apertures_are_refused_naming_the_count() {
    let five: Vec<_> = (1..=5)
        .map(|n| {
            aperture(
                &format!("api{n}"),
                &format!("api{n}.example.test"),
                &format!("10.0.0.{n}:443"),
            )
        })
        .collect();
    assert_refused(&five, "5 egress apertures");
    assert_refused(&five, &format!("at most {MAX_EGRESS_APERTURES}"));
    for n in 1..=5 {
        assert_refused(&five, &format!("`api{n}` (api{n}.example.test)"));
    }
}

#[test]
fn one_aperture_name_given_twice_is_refused_naming_it() {
    let again = EgressAperture {
        host: "other.example.test".to_owned(),
        ..model()
    };
    assert_refused(&[model(), again.clone()], "`model` is named twice");
    assert_refused(
        &[model(), again],
        "the hosts api.example.test and other.example.test",
    );
}

/// Every spelling `inet_aton` reads as an IPv4 address, which glibc dials before it reads the
/// child's `/etc/hosts`: decimal, hexadecimal, octal and mixed, in one to four labels.
const NUMERIC_HOSTS: [&str; 12] = [
    "0x7f000001",
    "0X7F000001",
    "0x7f.0x1",
    "0x7f.0.0.0x1",
    "0177.0.0.1",
    "0177.1",
    "0x7f.0.01",
    "2130706433",
    "127.1",
    "127.0.1",
    "0x0a000002",
    "0x7f000001.",
];

#[test]
fn a_numeric_host_in_any_spelling_inet_aton_reads_is_numeric() {
    for host in NUMERIC_HOSTS {
        assert!(numeric_host(host), "{host}");
    }
    for host in [
        "api.example.test",
        "0x7g000001",
        "0xdead.example",
        "1.2.3.4.5",
        "",
        "1..2",
        "x0x7f",
    ] {
        assert!(!numeric_host(host), "{host}");
    }
}

#[test]
fn a_later_aperture_naming_a_numeric_host_in_any_spelling_is_refused_naming_it() {
    for host in NUMERIC_HOSTS {
        let numeric = aperture("search", host, "10.0.0.2:443");
        assert_refused(&[model(), numeric], &format!("names the host {host}"));
    }
}

/// On the first aperture the only numeric host is `127.0.0.1` spelled so: `0x7f000001` names the
/// same address, and `0x0a000001` the pinned one, yet neither is allowed.
#[test]
fn the_first_aperture_allows_no_numeric_host_but_127_0_0_1_spelled_so() {
    for (host, pinned) in [
        ("0x7f000001", "127.0.0.1:443"),
        ("0177.0.0.1", "127.0.0.1:443"),
        ("127.1", "127.0.0.1:443"),
        ("0x0a000001", "10.0.0.1:443"),
        ("0x7f.0.0.0x1", "127.0.0.1:443"),
    ] {
        assert_refused(
            &[aperture("model", host, pinned)],
            &format!("names the host {host}"),
        );
    }
    check_network(
        NetworkMode::Aperture,
        &[aperture("model", "127.0.0.1", "127.0.0.1:443")],
    )
    .unwrap();
}

#[test]
fn a_later_aperture_naming_an_ipv4_literal_or_localhost_is_refused_naming_the_host() {
    for host in ["127.0.0.1", "10.0.0.2"] {
        let literal = aperture("search", host, &format!("{host}:443"));
        assert_refused(&[model(), literal], &format!("names the host {host}"));
    }
    for host in ["localhost", "LocalHost"] {
        let local = aperture("search", host, "127.0.0.1:443");
        assert_refused(&[model(), local], &format!("names the host {host}"));
    }
    // The first aperture keeps its rule: 127.0.0.1 pinned there, and `localhost`, are allowed.
    let first = aperture("model", "127.0.0.1", "127.0.0.1:443");
    check_network(NetworkMode::Aperture, &[first.clone(), search()]).unwrap();
    let first = aperture("model", "localhost", "127.0.0.1:443");
    check_network(NetworkMode::Aperture, &[first, search()]).unwrap();
}

#[test]
fn two_apertures_naming_one_host_are_refused_naming_it() {
    for host in ["api.example.test", "API.Example.Test"] {
        let same = aperture("search", host, "10.0.0.2:443");
        assert_refused(&[model(), same], "api.example.test");
    }
    let third = aperture("third", "Search.Example.Test", "10.0.0.3:443");
    assert_refused(&[model(), search(), third], "search.example.test");
}

#[test]
fn each_aperture_of_several_is_held_to_the_rules_of_one() {
    let blank = EgressAperture {
        host: " ".to_owned(),
        ..search()
    };
    assert_refused(&[model(), blank], "`search` names no host");
    let unnamed = EgressAperture {
        name: "Search".to_owned(),
        ..search()
    };
    assert_refused(&[model(), unnamed], "`Search`");
    let off_port = EgressAperture {
        port: 8443,
        ..search()
    };
    assert_refused(&[model(), off_port], "not the port it names");
}

#[test]
fn a_child_given_several_apertures_asks_for_each_by_name_in_order() {
    let one = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
    let request = one.input("space", "snapshot".to_owned()).sandbox;
    assert_eq!(request.aperture.as_deref(), Some("model"));
    assert!(request.apertures.is_empty(), "one aperture keeps its form");

    let two = one.with_egress(search());
    assert_eq!(two.egress(), [model(), search()]);
    let request = two.input("space", "snapshot".to_owned()).sandbox;
    assert_eq!(request.network, NetworkMode::Aperture);
    assert_eq!(request.aperture, None);
    assert_eq!(request.apertures, ["model", "search"]);
    substrate_wire::validate_aperture_selection(&request).unwrap();
    let live = two
        .session_input("space", "snapshot".to_owned())
        .exec
        .sandbox;
    assert_eq!(live, request);
}

/// `declared` as Substrate applies it, listening at `listen`.
fn applied(declared: &EgressAperture, listen: Option<&str>) -> substrate_wire::AppliedAperture {
    substrate_wire::AppliedAperture {
        mode: ApertureMode::Aperture,
        name: declared.name.clone(),
        destination: declared.pinned.to_string(),
        mechanism: ApertureMechanism::LoopbackForwarder,
        bytes: substrate_wire::ApertureBytes {
            to_destination: 7,
            from_destination: 11,
        },
        max_bytes: declared.max_bytes,
        listen: listen.map(str::to_owned),
    }
}

/// A record of exactly what `input` asked for, with `network` applied.
fn record(input: &ExecStartInput, network: AppliedNetwork) -> substrate_wire::AppliedConfinement {
    substrate_wire::AppliedConfinement {
        capability_snapshot: input.sandbox.capability_snapshot.clone(),
        cgroup: "substrate-ex_loom_1_0".to_owned(),
        filesystem: substrate_wire::AppliedFilesystem::WorkspaceReadWriteSystemReadOnly,
        network,
        profile: input.sandbox.profile,
        workspace_access: input.workspace_access.clone(),
        capsule: None,
        read_only_roots: input.read_only_roots.clone(),
        read_only_files: input.read_only_files.clone(),
        secret_slots: Vec::new(),
        workdir: None,
        writable_roots: Vec::new(),
    }
}

/// A record of several apertures is accepted only when it lists exactly the declared ones, in
/// order, each listening where Substrate puts the aperture at its place; any other set, order,
/// address or form is refused.
#[test]
fn a_record_must_list_exactly_the_apertures_requested_in_order() {
    let egress = [model(), search()];
    let child = egress.iter().cloned().fold(
        ChildCommand::new("true", [] as [&str; 0], "/work/space"),
        ChildCommand::with_egress,
    );
    let input = child.input("space", "snapshot".to_owned());
    let list = |apertures: Vec<substrate_wire::AppliedAperture>| {
        record(&input, AppliedNetwork::Apertures(apertures))
    };
    let exact = list(vec![
        applied(&model(), Some("127.0.0.1:443")),
        applied(&search(), Some("127.0.0.2:443")),
    ]);
    check_record(&input, &egress, &exact).unwrap();

    let third = aperture("third", "third.example.test", "10.0.0.3:443");
    let refused = [
        (
            "another order",
            list(vec![
                applied(&search(), Some("127.0.0.1:443")),
                applied(&model(), Some("127.0.0.2:443")),
            ]),
        ),
        (
            "one missing",
            list(vec![applied(&model(), Some("127.0.0.1:443"))]),
        ),
        (
            "one more",
            list(vec![
                applied(&model(), Some("127.0.0.1:443")),
                applied(&search(), Some("127.0.0.2:443")),
                applied(&third, Some("127.0.0.3:443")),
            ]),
        ),
        (
            "another listen address",
            list(vec![
                applied(&model(), Some("127.0.0.1:443")),
                applied(&search(), Some("127.0.0.3:443")),
            ]),
        ),
        (
            "no listen address",
            list(vec![applied(&model(), None), applied(&search(), None)]),
        ),
        (
            "another destination",
            list(vec![
                applied(&model(), Some("127.0.0.1:443")),
                applied(
                    &aperture("search", "search.example.test", "10.0.0.9:443"),
                    Some("127.0.0.2:443"),
                ),
            ]),
        ),
        (
            "one aperture alone",
            record(&input, AppliedNetwork::Aperture(applied(&model(), None))),
        ),
        ("no network", record(&input, AppliedNetwork::None)),
    ];
    for (case, record) in refused {
        let error = check_record(&input, &egress, &record).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::CapabilityUnserved,
            "{case}"
        );
    }

    // The request must name the declared set too: a record of what another request asked for is
    // not what this one required.
    let one = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .with_egress(model())
        .input("space", "snapshot".to_owned());
    assert!(check_record(&one, &egress, &record(&one, exact.network.clone())).is_err());
    let swapped = [search(), model()];
    assert!(check_record(&input, &swapped, &exact).is_err());
}

/// A shared driver is keyed by the whole set of apertures, in order, and declares each.
#[test]
fn a_driver_is_keyed_by_the_whole_set_and_declares_each_aperture() {
    let key = |apertures: Vec<EgressAperture>| DriverEgress {
        apertures,
        ca_bundle: None,
    };
    assert_ne!(key(vec![model()]), key(vec![model(), search()]));
    assert_ne!(key(vec![model(), search()]), key(vec![search(), model()]));
    assert_eq!(key(vec![model(), search()]), key(vec![model(), search()]));
    assert_ne!(key(Vec::new()), key(vec![model()]));

    let state = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let config = driver_config(parent.path(), state.path(), &key(vec![model(), search()])).unwrap();
    assert_eq!(config.egress_apertures, [model(), search()]);
}

/// One aperture keeps its profile network and digest; several are named each, in order, and their
/// digest depends on the set and its order.
#[test]
fn the_profile_names_every_aperture_in_order() {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space")
        .with_egress(model())
        .with_egress(search());
    let input = child.input("space", "snapshot".to_owned());
    let profile = requested_profile(&input, child.egress());
    assert_eq!(
        profile.network,
        "aperture:model@10.0.0.1:443,aperture:search@10.0.0.2:443"
    );
    let digest = |egress: &[EgressAperture]| profile_digest(&profile, &[], egress, None, &[], None);
    assert_ne!(digest(&[model(), search()]), digest(&[search(), model()]));
    assert_ne!(digest(&[model(), search()]), digest(&[model()]));
    let single = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
    let single = requested_profile(&single.input("space", "snapshot".to_owned()), &[model()]);
    assert_eq!(single.network, "aperture:model@10.0.0.1:443");
}

/// A child a byte ceiling ended is a result when any aperture of a list declares the ceiling.
#[test]
fn a_ceiling_of_any_aperture_of_a_list_explains_an_ended_child() {
    let bounded = EgressAperture {
        max_bytes: Some(1024),
        ..search()
    };
    let list = |second: &EgressAperture| {
        AppliedNetwork::Apertures(vec![
            applied(&model(), Some("127.0.0.1:443")),
            applied(second, Some("127.0.0.2:443")),
        ])
    };
    assert!(declares_a_ceiling(&list(&bounded)));
    assert!(!declares_a_ceiling(&list(&search())));
    assert!(!declares_a_ceiling(&AppliedNetwork::None));
}

/// The applied confinement lists every aperture of a list with the address it listens on.
#[test]
fn the_applied_confinement_lists_every_aperture() {
    let network = AppliedNetwork::Apertures(vec![
        applied(&model(), Some("127.0.0.1:443")),
        applied(&search(), Some("127.0.0.2:443")),
    ]);
    assert_eq!(
        applied_apertures(&network),
        [
            AppliedAperture {
                name: "model".to_owned(),
                destination: "10.0.0.1:443".to_owned(),
                listen: Some("127.0.0.1:443".to_owned()),
            },
            AppliedAperture {
                name: "search".to_owned(),
                destination: "10.0.0.2:443".to_owned(),
                listen: Some("127.0.0.2:443".to_owned()),
            },
        ]
    );
    assert_eq!(
        aperture_listen(3, &model()),
        Some("127.0.0.4:443".parse().unwrap())
    );
    assert_eq!(aperture_listen(4, &model()), None);
}
