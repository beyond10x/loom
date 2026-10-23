//! Conformance checks of `check_record` against the confinement Loom requests on Substrate 0.10.0:
//! a record naming anything the request did not ask for is not the confinement required.

use super::*;

const PINNED: &str = "10.0.0.1:443";

fn model() -> EgressAperture {
    EgressAperture {
        name: "model".to_owned(),
        host: "api.example.test".to_owned(),
        port: 443,
        pinned: PINNED.parse().unwrap(),
        max_bytes: None,
    }
}

fn aperture(listen: Option<&str>) -> substrate_wire::AppliedAperture {
    substrate_wire::AppliedAperture {
        mode: ApertureMode::Aperture,
        name: "model".to_owned(),
        destination: PINNED.to_owned(),
        mechanism: ApertureMechanism::LoopbackForwarder,
        bytes: substrate_wire::ApertureBytes {
            to_destination: 7,
            from_destination: 11,
        },
        max_bytes: None,
        listen: listen.map(str::to_owned),
    }
}

fn exact(input: &ExecStartInput, network: AppliedNetwork) -> substrate_wire::AppliedConfinement {
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

/// Substrate 0.10.0 added `listen` to an applied aperture, set only on the elements of the list
/// form; a single requested aperture records none. A single-aperture record naming a listen
/// address is not the aperture Loom requested and is refused, as any other field Loom did not ask
/// for is.
#[test]
fn security_a_single_aperture_record_naming_a_listen_address_is_refused() {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space").with_egress(model());
    let input = child.input("space", "snapshot".to_owned());
    check_record(
        &input,
        &[model()],
        &exact(&input, AppliedNetwork::Aperture(aperture(None))),
    )
    .unwrap();
    for listen in ["127.0.0.2:443", "0.0.0.0:443"] {
        let record = exact(&input, AppliedNetwork::Aperture(aperture(Some(listen))));
        let refused = check_record(&input, &[model()], &record);
        assert!(
            refused.is_err(),
            "a record whose single aperture listened on {listen} was accepted as the requested aperture"
        );
    }
}

/// Loom never requests an execution capsule (`capsule: None`). A record reporting a capsule mounted
/// read-only, with the capsule filesystem, is a mount Loom did not ask for and is refused.
#[test]
fn security_a_record_naming_an_unrequested_capsule_is_refused() {
    let child = ChildCommand::new("true", [] as [&str; 0], "/work/space");
    let input = child.input("space", "snapshot".to_owned());
    assert!(input.capsule.is_none());
    check_record(&input, &[], &exact(&input, AppliedNetwork::None)).unwrap();
    let record = substrate_wire::AppliedConfinement {
        filesystem:
            substrate_wire::AppliedFilesystem::WorkspaceReadWriteCapsuleReadOnlySystemReadOnly,
        capsule: Some(substrate_wire::AppliedExecutionCapsule {
            manifest_sha256: "0".repeat(64),
            entrypoint: "bin/tool".to_owned(),
            mount: "/capsule".to_owned(),
            file_count: 1,
            total_bytes: 1,
        }),
        ..exact(&input, AppliedNetwork::None)
    };
    assert!(
        check_record(&input, &[], &record).is_err(),
        "a record reporting a capsule Loom did not request was accepted"
    );
}
