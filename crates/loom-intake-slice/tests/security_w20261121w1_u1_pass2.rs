//! Security review of wave 2026-11-21-w1, unit U1 (story:substrate-0-21-pin), pass 2.
//!
//! Integrity conformance of the `/etc/alternatives` owned mount the correction added: every way
//! a launch can name a mount there is refused `ScopeInvalid` as the other owned mounts are,
//! naming the mount and "Substrate owns", through `run_child` and `spawn_child`, before
//! `target/` is created. That includes `at_host_path`, whose mount is its host path and which
//! is judged before the disk is read, so it holds on a host without `/etc/alternatives`. A
//! mount that only shares a text prefix still gets past every check.

use std::path::{Path, PathBuf};

use b10x_loom_intake_slice::confinement::{ChildCommand, ConfinementRefusal, SubstrateRunner};

fn host_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("loom-host-")
        .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap();
    let canonical = directory.path().canonicalize().unwrap();
    (directory, canonical)
}

fn assert_owned(runner: &SubstrateRunner, workspace: &Path, refused: &ChildCommand, named: &str) {
    for error in [
        runner.run_child(refused).unwrap_err(),
        runner.spawn_child(refused).unwrap_err(),
    ] {
        assert_eq!(
            error.reason,
            ConfinementRefusal::ScopeInvalid,
            "{named}: {error}"
        );
        assert!(error.detail.contains(named), "{named}: {error}");
        assert!(error.detail.contains("Substrate owns"), "{named}: {error}");
        assert!(
            error.detail.contains("`/etc/alternatives`") || error.detail.contains("`/etc/hosts`"),
            "{named}: names the owned mount: {error}"
        );
        assert!(!workspace.join("target").exists(), "{named}");
    }
}

#[test]
fn security_w20261121w1_u1_pass2_every_mount_on_etc_alternatives_is_owned() {
    let (_held, base) = host_directory();
    let workspace = base.join("workspace");
    let host = base.join("host");
    let file = base.join("host.toml");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::create_dir(&host).unwrap();
    std::fs::write(&file, "a file").unwrap();
    let runner = SubstrateRunner::new(Some(base.clone()));
    let child = || ChildCommand::new("true", [] as [&str; 0], &workspace);
    for mount in [
        "/etc/alternatives",
        "/etc/alternatives/cc",
        "/etc/alternatives/",
        "/etc/alternatives/../x",
    ] {
        assert_owned(
            &runner,
            &workspace,
            &child().with_read_only_root(&host, mount),
            mount,
        );
        assert_owned(
            &runner,
            &workspace,
            &child().with_read_only_file(&file, mount),
            mount,
        );
    }
    for host_path in ["/etc/alternatives", "/etc/alternatives/cc"] {
        assert_owned(
            &runner,
            &workspace,
            &child().at_host_path(host_path),
            host_path,
        );
    }
    for placed in [
        child().with_read_only_root(&host, "/etc/alternatives2"),
        child().with_read_only_root(&host, "/etc/alt"),
        child().with_read_only_file(&file, "/etc/alternatives.toml"),
    ] {
        let error = runner.run_child(&placed).unwrap_err();
        assert_eq!(
            error.reason,
            ConfinementRefusal::CgroupUndelegated,
            "{placed:?}: {error}"
        );
    }
}
