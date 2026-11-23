//! A delegated scope of the user's systemd manager for a test: whether one starts here, and one a
//! test holds while it moves processes into children of it. The fixtures are
//! `fixtures/tree-delegated/`: `scope.sh`, which holds a delegated scope of `systemd-run --user
//! --scope -p Delegate=yes`, and `enter.sh`, which moves a process into a child of it. A test that
//! needs bubblewrap and a user systemd manager that starts a delegated scope prints `skipped:
//! missing prerequisite: …` without them, unless `LOOM_REQUIRE_DELEGATION` is set: then it fails.

use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

/// The fixture directory of `scope.sh` and `enter.sh`.
pub const DELEGATED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-delegated");

/// Set, a missing prerequisite fails the test instead of skipping it.
pub const REQUIRE: &str = "LOOM_REQUIRE_DELEGATION";

/// The flags of a delegated scope of the user's systemd manager.
pub const SCOPE: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Skips the test naming `what`, or fails it when [`REQUIRE`] is set.
pub fn missing(what: &str) -> bool {
    assert!(
        std::env::var_os(REQUIRE).is_none(),
        "{REQUIRE} is set and a prerequisite is missing: {what}"
    );
    eprintln!("skipped: missing prerequisite: {what}");
    false
}

/// Whether bubblewrap is here and a delegated scope starts.
pub fn delegation_present() -> bool {
    if !Path::new("/usr/bin/bwrap").is_file() {
        return missing("executable /usr/bin/bwrap");
    }
    match Command::new("systemd-run").args(SCOPE).arg("true").output() {
        Ok(output) if output.status.success() => true,
        Ok(output) => missing(&format!(
            "a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr(&output).trim()
        )),
        Err(error) => missing(&format!("systemd-run ({error})")),
    }
}

/// A delegated scope a test holds: `scope.sh` waits in `keeper/` until it is dropped.
pub struct Scope {
    /// The scope's cgroup directory.
    pub root: PathBuf,
    keeper: Child,
}

impl Scope {
    /// A fresh delegated scope, cpu, memory and pids handed to its children, into which this
    /// test may move a process; `None` when a prerequisite is missing.
    pub fn start() -> Option<Self> {
        if !delegation_present() {
            return None;
        }
        let mut keeper = Command::new("systemd-run")
            .args(SCOPE)
            .arg("sh")
            .arg(Path::new(DELEGATED).join("scope.sh"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(keeper.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let root = PathBuf::from(line.trim());
        assert!(
            root.starts_with("/sys/fs/cgroup") && root.join("keeper").is_dir(),
            "scope.sh printed no scope: {line:?}"
        );
        let scope = Self { root, keeper };
        let probe = Command::new("sh")
            .arg(Path::new(DELEGATED).join("enter.sh"))
            .arg(scope.root.join("probe"))
            .arg("true")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir(scope.root.join("probe"));
        if !probe.status.success() {
            missing(&format!(
                "a test process that may move a process into a scope of the user's manager, as under systemd-run --user --scope ({})",
                stderr(&probe).trim()
            ));
            return None;
        }
        Some(scope)
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        drop(self.keeper.stdin.take());
        let _ = self.keeper.wait();
    }
}
