use b10x_loom_intake_slice::confinement::{ConfinementRefusal, SubstrateRunner, TestRunner};
use b10x_loom_intake_slice::executor::TestCommand;

#[test]
fn symlinked_target_is_refused_before_launch() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::os::unix::fs::symlink(root.path(), workspace.join("target")).unwrap();
    let error = SubstrateRunner::new(None)
        .run(&TestCommand::new("true", [] as [&str; 0]), &workspace)
        .unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid);
}

#[test]
fn undelegated_root_produces_a_typed_refusal_without_running_code() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let error = SubstrateRunner::new(Some(root.path().to_owned()))
        .run(
            &TestCommand::new("sh", ["-c", "touch target/ran"]),
            &workspace,
        )
        .unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::CgroupUndelegated);
    assert!(!workspace.join("target/ran").exists());
}

#[test]
fn file_target_and_invalid_workspace_name_are_named_refusals() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("target"), "not a directory").unwrap();
    assert_eq!(
        SubstrateRunner::new(None)
            .preflight(&workspace)
            .unwrap_err()
            .reason,
        ConfinementRefusal::ScopeInvalid
    );
    let invalid = root.path().join("invalid.name");
    std::fs::create_dir(&invalid).unwrap();
    assert_eq!(
        SubstrateRunner::new(None)
            .preflight(&invalid)
            .unwrap_err()
            .reason,
        ConfinementRefusal::WorkspaceNameInvalid
    );
}

/// Run explicitly in a dedicated scope:
/// systemd-run --user --scope -p Delegate=yes <this-test-binary> --ignored --nocapture
/// An unavailable prerequisite is a failure here, never successful qualification.
#[test]
#[ignore = "requires a dedicated delegated systemd user scope; run the test binary there with --ignored"]
fn real_confinement_rust_system_tools_and_cleanup() {
    use b10x_loom_intake_slice::confinement::{Backend, prepare_delegated_cgroup};
    use std::time::Duration;
    let cgroup = prepare_delegated_cgroup().expect("dedicated delegated user scope is required");
    let runner = SubstrateRunner::new(Some(cgroup.clone()));
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(workspace.join(".git/hooks")).unwrap();
    std::fs::write(workspace.join("source"), "unchanged").unwrap();
    let credential = root.path().join("credential");
    std::fs::write(&credential, "private").unwrap();
    runner
        .preflight(&workspace)
        .expect("real backend must serve confinement");
    let check = |script: &str| {
        let result = runner
            .run(&TestCommand::new("sh", ["-c", script]), &workspace)
            .unwrap();
        assert_eq!(result.exit_code, Some(0), "{}", result.output);
        assert_eq!(result.applied.backend, Backend::Substrate);
        assert_eq!(result.applied.network, "none");
        assert_eq!(result.applied.writable_scopes, ["target"]);
        let record = result.driver_record.as_ref().expect("actual driver record");
        assert_eq!(record.cgroup, result.applied.cgroup);
        assert!(
            !record
                .read_only_roots
                .iter()
                .any(|r| r.host_path.ends_with("/.cargo"))
        );
        assert!(record.secret_slots.is_empty());
        result
    };
    check(
        "set -eu; if echo changed >source; then exit 90; fi; if touch .git/hooks/attack; then exit 91; fi; if touch /toolchain/rust/forbidden; then exit 92; fi; touch target/allowed; test -f target/allowed",
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("source")).unwrap(),
        "unchanged"
    );
    assert!(!workspace.join(".git/hooks/attack").exists());
    let credential = credential.to_str().unwrap();
    check(&format!(
        "set -e; test ! -r '{credential}'; if touch '{credential}'; then exit 93; fi; test \"$HOME\" = /tmp; test -z \"$SSH_AUTH_SOCK\"; test -z \"$AWS_ACCESS_KEY_ID\"; test ! -f \"$CARGO_HOME/credentials.toml\""
    ));
    check("if bash -c 'echo forbidden >/dev/tcp/1.1.1.1/443'; then exit 94; fi");
    // Inspect real kernel bounds while an execution is alive. No request value
    // or observation string can substitute for these cgroup control files.
    std::thread::scope(|scope| {
        let running = scope.spawn(|| runner.run(&TestCommand::new("sleep", ["2"]), &workspace));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let execution_group = loop {
            let found = std::fs::read_dir(&cgroup)
                .unwrap()
                .filter_map(Result::ok)
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("substrate-ex_loom_")
                        && std::fs::read_to_string(entry.path().join("cgroup.procs"))
                            .is_ok_and(|s| !s.trim().is_empty())
                });
            if let Some(entry) = found {
                break entry.path();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "execution cgroup never appeared"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            std::fs::read_to_string(execution_group.join("pids.max"))
                .unwrap()
                .trim(),
            "2048"
        );
        assert_eq!(
            std::fs::read_to_string(execution_group.join("memory.max"))
                .unwrap()
                .trim(),
            "8589934592"
        );
        assert_eq!(
            std::fs::read_to_string(execution_group.join("memory.swap.max"))
                .unwrap()
                .trim(),
            "0"
        );
        assert_eq!(
            std::fs::read_to_string(execution_group.join("memory.oom.group"))
                .unwrap()
                .trim(),
            "1"
        );
        assert_ne!(
            std::fs::read_to_string(execution_group.join("cpu.max"))
                .unwrap()
                .trim(),
            "max 100000"
        );
        let result = running.join().unwrap().unwrap();
        assert_eq!(result.exit_code, Some(0), "{}", result.output);
        assert_eq!(cgroup.join(&result.applied.cgroup), execution_group);
        assert!(
            !execution_group.exists(),
            "successful execution must retire its cgroup"
        );
    });
    let timed = runner
        .run(
            &TestCommand::new("sh", ["-c", "setsid sleep 60 & wait"])
                .with_timeout(Duration::from_millis(250)),
            &workspace,
        )
        .unwrap();
    assert!(timed.timed_out, "{}", timed.output);
    assert_eq!(timed.exit_code, None);
    assert!(
        !cgroup.join(&timed.applied.cgroup).exists(),
        "timeout must retire its cgroup"
    );
    // Offline Cargo compiles with a private Cargo home and direct installed compiler.
    std::fs::create_dir(workspace.join("src")).unwrap();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname='confined-fixture'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nitoa='1'\n",
    )
    .unwrap();
    std::fs::write(
        workspace.join("src/lib.rs"),
        "#[test] fn fixture() { assert_eq!(itoa::Buffer::new().format(4), \"4\"); }\n",
    )
    .unwrap();
    assert!(
        std::process::Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&workspace)
            .status()
            .unwrap()
            .success()
    );
    let cargo = runner
        .run(
            &TestCommand::new("cargo", ["test", "--offline"]),
            &workspace,
        )
        .unwrap();
    assert_eq!(cargo.exit_code, Some(0), "{}", cargo.output);
    assert!(cargo.output.contains("1 passed"), "{}", cargo.output);
    std::fs::write(workspace.join("Cargo.toml"), "[package]\nname='confined-fixture'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nloom-deliberately-unavailable-dependency='=99.0.0'\n").unwrap();
    let missing = runner
        .run(
            &TestCommand::new("cargo", ["test", "--offline"]),
            &workspace,
        )
        .unwrap();
    assert_ne!(missing.exit_code, Some(0));
    assert!(missing.output.contains("cargo fetch"), "{}", missing.output);
}
