//! Adversary pass 1 (wave 2026-10-05-w19, unit loom/import-commission): the story's first
//! acceptance line, "`git log --follow` on a moved file shows commits from `beyond10x/commission`".
//!
//! Needs the full history (a shallow clone cannot pass it). It is a probe of the acceptance, not a
//! gate to merge as is.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

/// Every one of these files was first committed in Commission's bootstrap commit, at `ess/…` or
/// `docs/…`, and moved by the import.
#[test]
fn adversary_moved_commission_files_follow_into_commission_history() {
    let mut lost = Vec::new();
    for file in [
        "ess/commission/system.yaml",
        "ess/commission/domains/responsibility.yaml",
        "docs/commission/contracts/frontier.md",
    ] {
        let out = Command::new("git")
            .current_dir(repo_root())
            .args(["log", "--follow", "--format=%s", "--", file])
            .output()
            .unwrap_or_else(|error| panic!("run git log: {error}"));
        assert!(out.status.success(), "git log --follow -- {file} failed");
        let subjects = String::from_utf8_lossy(&out.stdout);
        if !subjects
            .lines()
            .any(|s| s == "Bootstrap Commission from the Governed Autonomy build pack")
        {
            lost.push(format!(
                "{file}: {} commit(s) followed: {:?}",
                subjects.lines().count(),
                subjects.lines().collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        lost.is_empty(),
        "`git log --follow` does not reach Commission's history for moved files:\n{}",
        lost.join("\n")
    );
}
