//! The `b10x-loom` binary as a test starts it: every process one test starts shares one test
//! clock (`b10x_loom_intake_slice::clock::test_clock`), so an instant a later command reads is
//! never earlier than one an earlier command read, and no command sleeps for the next second.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::clock::test_clock::VARIABLE;

/// `b10x-loom`, its clock's offset held in this test process's file.
pub fn loom() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    command.env(VARIABLE, clock_file());
    command
}

/// The test clock file of this test process, under `CARGO_TARGET_TMPDIR`: nextest runs each test
/// in a process of its own.
fn clock_file() -> &'static PathBuf {
    static FILE: OnceLock<PathBuf> = OnceLock::new();
    FILE.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("test-clock");
        std::fs::create_dir_all(&dir).expect("the test clock directory");
        dir.join(format!("{}-{nanos}", std::process::id()))
    })
}
