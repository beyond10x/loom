use b10x_loom_intake_slice::confinement::{
    Backend, ConfinementRefusal, SubstrateRunner, TestRunner, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;

fn scratch() -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.engineering/drafts/confinement-adversary");
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

#[test]
#[ignore = "requires a dedicated delegated systemd user scope"]
fn target_hardlinks_cannot_mutate_source_git_or_external_files() {
    let cgroup = prepare_delegated_cgroup().expect("dedicated delegated scope required");
    let fixture = tempfile::Builder::new()
        .prefix("hardlinks-")
        .tempdir_in(scratch())
        .unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir_all(workspace.join("target")).unwrap();
    std::fs::create_dir(workspace.join(".git")).unwrap();
    let victims = [
        workspace.join("source.rs"),
        workspace.join(".git/config"),
        fixture.path().join("external"),
    ];
    for (index, victim) in victims.iter().enumerate() {
        std::fs::write(victim, "unchanged").unwrap();
        std::fs::hard_link(victim, workspace.join(format!("target/alias-{index}"))).unwrap();
    }
    let result = SubstrateRunner::new(Some(cgroup)).run(
        &TestCommand::new(
            "sh",
            [
                "-c",
                "for path in target/alias-*; do printf changed > \"$path\"; done",
            ],
        ),
        &workspace,
    );
    eprintln!("runner result: {result:?}");
    if let Ok(execution) = &result {
        assert_eq!(execution.applied.backend, Backend::Substrate);
        assert_eq!(execution.applied.writable_scopes, ["target"]);
    } else {
        assert_eq!(result.unwrap_err().reason, ConfinementRefusal::ScopeInvalid);
    }
    let contents: Vec<_> = victims
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect();
    assert_eq!(
        contents,
        ["unchanged", "unchanged", "unchanged"],
        "target aliases bypassed the promised source/Git/external write boundary"
    );
}

#[test]
#[ignore = "requires a dedicated delegated systemd user scope and seeded synthetic environment"]
fn inherited_synthetic_credentials_are_cleared() {
    assert_eq!(
        std::env::var("AWS_ACCESS_KEY_ID").unwrap(),
        "loom-synthetic-aws"
    );
    assert_eq!(
        std::env::var("SSH_AUTH_SOCK").unwrap(),
        "/synthetic/loom-agent"
    );
    let cgroup = prepare_delegated_cgroup().expect("dedicated delegated scope required");
    let fixture = tempfile::Builder::new()
        .prefix("environment-")
        .tempdir_in(scratch())
        .unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let result = SubstrateRunner::new(Some(cgroup)).run(
        &TestCommand::new("sh", ["-c", "test -z \"${AWS_ACCESS_KEY_ID+x}\" && test -z \"${SSH_AUTH_SOCK+x}\" && test \"$HOME\" = /tmp && printf cleared"]),
        &workspace,
    ).unwrap();
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    assert_eq!(result.output, "cleared");
    assert_eq!(result.applied.backend, Backend::Substrate);
}

#[test]
#[ignore = "requires a dedicated delegated systemd user scope"]
fn workspace_rust_toolchain_override_never_executes_on_the_host() {
    use std::os::unix::fs::PermissionsExt as _;
    let cgroup = prepare_delegated_cgroup().expect("dedicated delegated scope required");
    let fixture = tempfile::Builder::new()
        .prefix("override-")
        .tempdir_in(scratch())
        .unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir_all(workspace.join("evil/bin")).unwrap();
    std::fs::write(
        workspace.join("evil/bin/rustc"),
        "#!/bin/sh\nprintf escaped > host-executed\n",
    )
    .unwrap();
    std::fs::set_permissions(
        workspace.join("evil/bin/rustc"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::write(
        workspace.join("rust-toolchain.toml"),
        format!(
            "[toolchain]\npath = '{}'\n",
            workspace.join("evil").display()
        ),
    )
    .unwrap();
    let result = SubstrateRunner::new(Some(cgroup))
        .run(&TestCommand::new("rustc", ["--version"]), &workspace);
    assert!(!workspace.join("host-executed").exists());
    assert_eq!(result.unwrap_err().reason, ConfinementRefusal::ScopeInvalid);
}

#[test]
#[ignore = "fixture child invoked only inside Substrate"]
fn fixture_socket_client() {
    use std::io::Write as _;
    let mut stream =
        std::os::unix::net::UnixStream::connect("/workspace/target/host-socket").unwrap();
    stream.write_all(b"escaped-host-ipc").unwrap();
}

#[test]
#[ignore = "requires a dedicated delegated systemd user scope"]
fn target_socket_cannot_reach_an_unconfined_host_service() {
    use std::io::Read as _;
    use std::os::fd::AsRawFd as _;
    let cgroup = prepare_delegated_cgroup().expect("dedicated delegated scope required");
    let fixture = tempfile::Builder::new()
        .prefix("socket-")
        .tempdir_in(scratch())
        .unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir_all(workspace.join("target")).unwrap();
    let directory = std::fs::File::open(&workspace).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(format!(
        "/proc/self/fd/{}/target/host-socket",
        directory.as_raw_fd()
    ))
    .unwrap();
    listener.set_nonblocking(true).unwrap();
    std::fs::copy(
        std::env::current_exe().unwrap(),
        workspace.join("target/probe"),
    )
    .unwrap();
    let result = SubstrateRunner::new(Some(cgroup)).run(
        &TestCommand::new(
            "/workspace/target/probe",
            [
                "--exact",
                "fixture_socket_client",
                "--ignored",
                "--nocapture",
            ],
        ),
        &workspace,
    );
    let received = match listener.accept() {
        Ok((mut stream, _)) => {
            let mut message = String::new();
            stream.read_to_string(&mut message).unwrap();
            Some(message)
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
        Err(error) => panic!("unexpected listener error: {error}"),
    };
    eprintln!(
        "runner exit: {:?}; host received: {received:?}",
        result.as_ref().map(|r| r.exit_code)
    );
    if let Err(error) = result {
        assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
    }
    assert_eq!(
        received, None,
        "confined code reached an unconfined host socket despite no-network confinement"
    );
}
