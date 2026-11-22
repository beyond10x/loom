//! Acceptance for `story:tree-replay-command`, command half: `b10x-loom tree replay --store S
//! [--case C]` prints, as JSON, what `replay_tree` found and exits 0 when every compared case
//! matches its log, 3 when one differs (naming it on stderr) and 1 when it is refused.
//!
//! The cases live in the crate's shared test binary, not in `tree_run`'s, whose starts of
//! `b10x-loom` are bounded (`START_BUDGETS` in `crates/loom-xtask/src/test_step.rs`). The stores
//! are the library's ([`fixture`], `crates/loom-tree/tests/replay/fixture.rs`), made once per test
//! run under this crate's `CARGO_TARGET_TMPDIR`; each start reads a copy of its own. A local
//! runner and POSIX sh stubs; no network, no model, no credential; the test ends within 60 s.

#[path = "../../../loom-tree/tests/replay/fixture.rs"]
mod fixture;

use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use b10x_loom_tree::{replay_json, replay_tree};
use serde_json::Value;

use fixture::{
    ROOT, Trees, append_a_late_child_closed, catalog, lines, move_outside_the_log,
    receive_outside_the_log,
};

const BINARY: &str = env!("CARGO_BIN_EXE_b10x-loom");

/// `b10x-loom tree replay --store <store>`, with `--case <case>` when given.
fn replay(store: &Path, case: Option<&str>) -> Output {
    let mut command = Command::new(BINARY);
    command.args(["tree", "replay", "--store"]).arg(store);
    if let Some(case) = case {
        command.args(["--case", case]);
    }
    command.output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The leg of the stored `wait` tree whose log holds no `terminated` line.
fn open_leg(store: &Path) -> &'static str {
    let open = |case: &str| {
        !lines(store, case).iter().any(|line| {
            matches!(
                line.event,
                b10x_loom_tree::runlog::RunLogEvent::Terminated(_)
            )
        })
    };
    match (open(fixture::NORTH), open(fixture::SOUTH)) {
        (true, false) => fixture::NORTH,
        (false, true) => fixture::SOUTH,
        other => panic!("at the wait one leg is open: {other:?}"),
    }
}

/// The closed leg of the stored `wait` tree.
fn closed_leg(store: &Path) -> &'static str {
    if open_leg(store) == fixture::NORTH {
        fixture::SOUTH
    } else {
        fixture::NORTH
    }
}

/// Asserts `output` exited 3 with one stderr line naming `case`, `what` and, when given, `line`.
fn differs(output: &Output, case: &str, what: &str, line: Option<i64>) {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    let at = line.map_or(String::new(), |seq| format!(" at line {seq}"));
    assert_eq!(
        stderr,
        format!("b10x-loom: tree replay: case {case} differs from its log: {what}{at}\n")
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap();
}

#[test]
fn tree_replay_prints_the_cases_and_exits_by_what_it_found() {
    let started = Instant::now();
    let trees = Trees::made();

    // A tree run to its end: exit 0, and stdout is the library's replay, as JSON.
    let ended = trees.copy("ended", "cli-ended");
    let output = replay(&ended, None);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = replay_json(&replay_tree(&ended, &catalog(&ended), None).unwrap());
    assert_eq!(printed, expected);

    // A tree only opened: no case was decided, nothing differs.
    let opened = trees.copy("opened", "cli-opened");
    let output = replay(&opened, None);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));

    // A record received outside the log: a decision difference, exit 3.
    let received = trees.copy("wait", "cli-received");
    let leg = open_leg(&received);
    receive_outside_the_log(&received, leg);
    differs(&replay(&received, None), leg, "decision", None);

    // A revision moved outside the log: a state difference, exit 3.
    let moved = trees.copy("wait", "cli-moved");
    let leg = open_leg(&moved);
    move_outside_the_log(&moved, leg, "route");
    differs(&replay(&moved, None), leg, "state", None);

    // A `child closed` whose `child opened` precedes the last evaluation: named by its line.
    let late = trees.copy("wait", "cli-late");
    let seq = append_a_late_child_closed(&late, ROOT, closed_leg(&late));
    differs(&replay(&late, None), ROOT, "state", Some(seq));

    // A directory that holds no tree: refused NotATree, exit 1.
    let empty = fixture::scratch("cli-empty");
    std::fs::create_dir_all(&empty).unwrap();
    let output = replay(&empty, None);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    assert!(
        text(&output.stderr).starts_with("b10x-loom: tree refused: NotATree"),
        "{}",
        text(&output.stderr)
    );

    // A case that is not in the tree: refused CaseNotInTree, exit 1, nothing on stdout.
    let output = replay(&ended, Some("root.nowhere"));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("b10x-loom: tree refused: CaseNotInTree"),
        "{stderr}"
    );
    assert!(
        stderr.contains("the tree rooted at root holds no such case"),
        "{stderr}"
    );

    assert!(
        started.elapsed() < Duration::from_secs(60),
        "{:?}",
        started.elapsed()
    );
}
