//! Security review of the live confined child and the child's read-only roots (wave
//! 2026-10-22-w1). Each case states a boundary the stories claim and is red while the boundary
//! does not hold.
//!
//! The delegated cases run through the harness in `tests/support`, as `confined_child.rs` does:
//! the test binary starts itself again under `systemd-run --user --scope -p Delegate=yes` and
//! skips only naming a missing prerequisite.

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{ChildCommand, ChildSignal, SubstrateRunner};

fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
    support::delegated(name, &[], case);
}

/// A host-supplied timeout is a value to refuse or bound, never a panic: a live child named with
/// the largest timeout either starts (and is killed here) or is refused, and the call returns.
#[test]
fn security_a_live_child_with_the_largest_timeout_returns_instead_of_panicking() {
    delegated(
        "security_a_live_child_with_the_largest_timeout_returns_instead_of_panicking",
        |runner, workspace| {
            let named = ChildCommand::new("sleep", ["600"], workspace).with_timeout(Duration::MAX);
            let started = catch_unwind(AssertUnwindSafe(|| runner.spawn_child(&named)));
            let Ok(started) = started else {
                panic!("spawn_child panicked on a timeout of Duration::MAX instead of returning");
            };
            if let Ok(mut child) = started {
                child
                    .signal(ChildSignal::Kill)
                    .expect("the child is killed");
                let _ = child.wait();
            }
        },
    );
}

/// The same for a child run to its end with standard input: the call returns, a result or a
/// refusal, and never panics on the timeout the host named.
#[test]
fn security_run_child_with_stdin_and_the_largest_timeout_returns_instead_of_panicking() {
    delegated(
        "security_run_child_with_stdin_and_the_largest_timeout_returns_instead_of_panicking",
        |runner, workspace| {
            let named = ChildCommand::new("cat", [] as [&str; 0], workspace)
                .with_stdin(b"input\n".to_vec())
                .with_timeout(Duration::MAX);
            let ran = catch_unwind(AssertUnwindSafe(|| runner.run_child(&named)));
            assert!(
                ran.is_ok(),
                "run_child with standard input panicked on a timeout of Duration::MAX instead of returning"
            );
        },
    );
}

/// A live child dropped where its host happens to run async code is still killed and released,
/// and the drop does not panic: `Drop` is not a call the host can keep outside a runtime.
#[test]
fn security_a_live_child_dropped_inside_an_async_runtime_is_killed_without_a_panic() {
    delegated(
        "security_a_live_child_dropped_inside_an_async_runtime_is_killed_without_a_panic",
        |runner, workspace| {
            let started = workspace.join("target/started");
            let child = runner
                .spawn_child(&ChildCommand::new(
                    "sh",
                    ["-c", "touch target/started; sleep 3; touch target/survived"],
                    workspace,
                ))
                .expect("the shell starts live");
            let deadline = Instant::now() + Duration::from_secs(30);
            while !started.exists() {
                assert!(Instant::now() < deadline, "the child started within 30 s");
                std::thread::sleep(Duration::from_millis(50));
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let dropped = catch_unwind(AssertUnwindSafe(|| {
                runtime.block_on(async move { drop(child) });
            }));
            std::thread::sleep(Duration::from_secs(5));
            let survived = workspace.join("target/survived").exists();
            assert!(
                dropped.is_ok() && !survived,
                "dropping a live child inside an async runtime: panicked {}, child survived {survived}",
                dropped.is_err()
            );
        },
    );
}
