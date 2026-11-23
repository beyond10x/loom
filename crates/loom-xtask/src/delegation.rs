//! `loom-xtask delegation-probe`: the one probe of what the delegated tests need, and its one
//! decision, live in `crates/loom-cli/tests/common/delegation_probe.rs`, which the tests' guards
//! read too; this module builds the probe's example first. The test step runs it before its timed
//! run, unless its caller set `LOOM_REQUIRE_DELEGATION`, and CI's step `Probe a delegated user
//! scope` runs it before `task check`. Where it passes, `LOOM_REQUIRE_DELEGATION=1` turns a
//! delegated test that would skip into a failure; where it fails, the variable stays unset and
//! those tests skip, named by the test step.

use std::path::{Path, PathBuf};

#[path = "../../loom-cli/tests/common/delegation_probe.rs"]
mod shared;

pub use shared::{Record, decide};

/// Runs the probe in the repository `root`, building the example once the steps before it
/// passed. `Err` when the probe itself cannot run: the example does not build.
pub fn probe(root: &Path) -> Result<Record, String> {
    shared::probe(&root.join(shared::FIXTURES), || build_example(root))
}

/// Builds the example and answers its path.
fn build_example(root: &Path) -> Result<PathBuf, String> {
    let args = [
        "build",
        "--locked",
        "-q",
        "-p",
        shared::EXAMPLE_PACKAGE,
        "--example",
        shared::EXAMPLE,
    ];
    let status = crate::test_step::cargo_command(root)
        .args(args)
        .status()
        .map_err(|e| format!("run cargo {}: {e}", args.join(" ")))?;
    if !status.success() {
        return Err(format!("cargo {} exited {status}", args.join(" ")));
    }
    Ok(crate::test_step::target_dir(root)
        .join("debug/examples")
        .join(shared::EXAMPLE))
}

#[cfg(test)]
mod tests {
    use super::shared::{UNSERVED, pool_job, runner_of_pr_166 as runner};
    use super::*;

    /// What the probe it replaces read: `systemd-run --user --scope -p Delegate=yes true` exited 0.
    fn weak_probe_passes(record: &Record) -> bool {
        matches!(record.scope, Some(Ok(())))
    }

    #[test]
    fn the_runner_of_pr_166_does_not_pass_where_the_weak_probe_did() {
        let runner = runner();
        assert!(
            weak_probe_passes(&runner),
            "the replaced probe passed on the runner"
        );
        let why = decide(&runner).expect_err("the delegated tests cannot run on the runner");
        assert!(
            why.contains("enter.sh") && why.contains("I/O error"),
            "{why}"
        );
    }

    #[test]
    fn substrate_refusing_limits_fails_the_probe_where_a_process_may_enter_the_scope() {
        let record = Record {
            enter: Some(Ok(())),
            ..runner()
        };
        assert!(weak_probe_passes(&record));
        let why = decide(&record).expect_err("Substrate serves no limits");
        assert!(why.contains("Substrate") && why.contains(UNSERVED), "{why}");
    }

    #[test]
    fn a_plain_session_scope_fails_the_probe() {
        // This workstation, from `session-3.scope`: the scope starts, and the test process may not
        // move a process into it, its nearest common ancestor being root's `user-1000.slice`.
        let record = Record {
            bubblewrap: true,
            scope: Some(Ok(())),
            enter: Some(Err(
                "enter.sh: line 6: echo: write error: Permission denied".to_owned(),
            )),
            substrate: None,
        };
        assert!(weak_probe_passes(&record));
        let why = decide(&record).expect_err("a session scope moves no process into a scope");
        assert!(why.contains("Permission denied"), "{why}");
    }

    #[test]
    fn a_pool_job_passes_the_probe() {
        assert_eq!(decide(&pool_job()), Ok(()));
    }

    #[test]
    fn a_missing_bubblewrap_or_a_step_not_run_fails_the_probe() {
        let why = decide(&Record {
            bubblewrap: false,
            ..pool_job()
        })
        .expect_err("no bubblewrap");
        assert!(why.contains("/usr/bin/bwrap"), "{why}");
        for record in [
            Record {
                scope: None,
                ..pool_job()
            },
            Record {
                enter: None,
                ..pool_job()
            },
            Record {
                substrate: None,
                ..pool_job()
            },
        ] {
            let why = decide(&record).expect_err("a step not run proves nothing");
            assert!(why.contains("not run"), "{record:?}: {why}");
        }
    }
}
