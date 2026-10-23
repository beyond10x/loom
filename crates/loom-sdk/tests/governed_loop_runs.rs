//! The SDK's looping example, `examples/governed_loop.rs`: an agent that loops on a failing check
//! until the frontier offers the merge, whose early merge is refused, whose merge stops for a
//! human, and which a tight step budget suspends.
//!
//! The example's logic is compiled into this test, so the runs asserted here are the runs
//! `cargo run --example governed_loop` makes. Its scratch repositories are under
//! `CARGO_TARGET_TMPDIR`. No case reaches a model or the network.

#[allow(dead_code)]
#[path = "../examples/governed_loop.rs"]
mod example;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use loom_sdk::commission::model::responsibility::{
    EffectOutcome, RevalidateActionRequestOutcome, RunOutcome,
};

#[test]
fn the_agent_loops_until_the_check_passes_and_stops_at_the_merge_approval() {
    let workspace = scratch("roomy");

    let end = example::run(&workspace, example::ROOMY).expect("the example runs until it stops");

    assert_eq!(
        example::stop_reason(&end),
        "ApprovalRequired (repository.merge)",
        "{end:#?}"
    );
    let admitted: Vec<&str> = end
        .admitted
        .iter()
        .map(|request| request.action.as_str())
        .collect();
    assert_eq!(
        admitted,
        [
            "implementation.edit",
            "tests.run",
            "implementation.edit",
            "tests.run",
            "implementation.edit",
            "tests.run",
        ],
        "three attempts and three checks are admitted, the merge never is: {end:#?}"
    );
    assert!(
        end.effects
            .iter()
            .all(|effect| matches!(effect, EffectOutcome::Performed(_))),
        "{:?}",
        end.effects
    );
    // The early merge was refused at selection: it is no request at all. The one merge request is
    // the last, after the passing check, and it needs authority.
    let needing: Vec<&str> = end
        .requests
        .iter()
        .filter_map(|made| match &made.outcome {
            RevalidateActionRequestOutcome::NeedsAuthority { error } => Some(error.action.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(needing, ["repository.merge"], "{end:#?}");
    assert_eq!(end.requests.len(), 7, "{end:#?}");

    // The third attempt is at HEAD, it passes the check, and nothing was merged or branched.
    assert_eq!(
        git(&workspace, &["show", "HEAD:fizzbuzz.sh"]),
        example::ATTEMPTS[2].1
    );
    assert!(sh(&workspace, "check.sh"), "check.sh passes at HEAD");
    assert_eq!(
        git(&workspace, &["rev-list", "--count", "HEAD"]).trim(),
        "4"
    );
    assert_eq!(git(&workspace, &["branch", "--list"]).trim(), "* main");

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn a_tight_budget_suspends_the_agent_before_it_fixes_anything() {
    let workspace = scratch("tight");

    let end = example::run(&workspace, example::TIGHT).expect("the example runs until it stops");

    assert_eq!(example::stop_reason(&end), "Budget", "{end:#?}");
    assert!(
        matches!(end.outcome, RunOutcome::Suspended(_)),
        "{:?}",
        end.outcome
    );
    assert_eq!(end.steps, example::TIGHT, "{end:#?}");
    assert_eq!(
        git(&workspace, &["show", "HEAD:fizzbuzz.sh"]),
        example::ATTEMPTS[1].1
    );
    assert!(!sh(&workspace, "check.sh"), "check.sh still fails at HEAD");

    let _ = std::fs::remove_dir_all(&workspace);
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "loom-sdk-governed-loop-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn sh(workspace: &Path, script: &str) -> bool {
    Command::new("sh")
        .arg(script)
        .current_dir(workspace)
        .status()
        .expect("run sh")
        .success()
}

fn git(workspace: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}
