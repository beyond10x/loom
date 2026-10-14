//! `story:pinned-consumer-check`: the consumer's binary, built outside Loom's workspace against
//! `b10x-loom-sdk` alone, runs the documented embedding to `ApprovalRequired (repository.merge)`
//! and merges nothing.
//!
//! The binary runs in a fresh directory under `CARGO_TARGET_TMPDIR`; its scratch repository is
//! `target/loom-sdk-example` there. No case reaches a model or the network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn the_pinned_consumer_stops_at_approval_required() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pinned-consumer-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&directory)
        .unwrap_or_else(|e| panic!("create {}: {e}", directory.display()));

    let output = Command::new(env!("CARGO_BIN_EXE_loom-pinned-consumer"))
        .current_dir(&directory)
        .output()
        .unwrap_or_else(|e| panic!("run the consumer: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the consumer exits 0: {}\n{stdout}\n{stderr}",
        output.status
    );

    let last = stdout.lines().last().unwrap_or_default();
    assert_eq!(
        last.strip_prefix("stopped: "),
        Some("ApprovalRequired (repository.merge)"),
        "the consumer's last line is the stop at the merge approval:\n{stdout}"
    );

    // The edit was committed in the scratch repository, and nothing was merged or branched.
    let workspace = directory.join("target/loom-sdk-example");
    assert_eq!(git(&workspace, &["show", "HEAD:check.txt"]), "fixed\n");
    assert_eq!(
        git(&workspace, &["rev-list", "--count", "HEAD"]).trim(),
        "2",
        "the failing check's commit and the edit's, and no merge commit"
    );
    assert_eq!(git(&workspace, &["branch", "--list"]).trim(), "* main");
    assert_eq!(
        git(&workspace, &["rev-list", "--merges", "--count", "HEAD"]).trim(),
        "0",
        "nothing was merged"
    );

    let _ = fs::remove_dir_all(&directory);
}

fn git(workspace: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("run git {args:?}: {e}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}
