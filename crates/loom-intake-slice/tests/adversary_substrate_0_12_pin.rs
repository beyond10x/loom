//! Adversary, story:substrate-0-12-pin.
//!
//! A real Substrate driver derives `limits` and the confinement floor (`process_run`, `wait`,
//! `output_read`) from one probe result: when the delegated cgroup is unusable, every one of them
//! is absent together. Admission walks `NEEDED_FAMILIES` with `limits` last, so a driver whose
//! cgroup is not delegated is refused `CapabilityUnserved` naming `process_run`, never
//! `CgroupUndelegated`. Before the change that driver was refused `CgroupUndelegated`, the reason
//! `b10x-loom` re-executes itself in a delegated systemd scope on.

use b10x_loom_intake_slice::confinement::{ConfinementRefusal, SubstrateRunner};

fn current_cgroup() -> std::path::PathBuf {
    let line = std::fs::read_to_string("/proc/self/cgroup").unwrap();
    let path = line
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .expect("unified cgroup membership");
    std::path::Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'))
}

/// Run inside a delegated scope:
/// systemd-run --user --scope -q -p Delegate=yes <this-test-binary> --ignored
///
/// It models the root `discover_cgroup_root` picks in a session whose ancestor cgroup does not
/// delegate cpu, memory and pids: an empty, writable cgroup above this process, its controllers
/// not enabled.
#[test]
#[ignore = "story:substrate-0-12-pin: requires a delegated systemd user scope; run the test binary there with --ignored"]
fn adversary_substrate_0_12_pin_an_undelegated_cgroup_is_still_refused_cgroup_undelegated() {
    let scope = current_cgroup();
    let ancestor = scope.join("adv-undelegated");
    let leaf = ancestor.join("leaf");
    std::fs::create_dir_all(&leaf).expect("a delegated scope is required");
    std::fs::write(leaf.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    assert_eq!(
        std::fs::read_to_string(ancestor.join("cgroup.controllers"))
            .unwrap()
            .trim(),
        "",
        "the ancestor delegates no controller"
    );

    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let outcome = SubstrateRunner::new(None).preflight(&workspace);

    std::fs::write(scope.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let _ = std::fs::remove_dir(&leaf);
    let _ = std::fs::remove_dir(&ancestor);

    let error = outcome.expect_err("an undelegated cgroup serves no confined run");
    assert_eq!(
        error.reason,
        ConfinementRefusal::CgroupUndelegated,
        "a driver whose cgroup is not delegated must be refused CgroupUndelegated, as before \
         the change, so the CLI re-executes in a delegated scope: {error}"
    );
}
