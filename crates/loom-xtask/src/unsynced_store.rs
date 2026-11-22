//! `unsynced-store <binary>`: a released `b10x-loom` syncs every durable write
//! (`story:tree-run-tests-fit-the-budget`).
//!
//! A build of `b10x-loom-governor` with the feature `test-unsynced-store` can skip its syncs while
//! `B10X_LOOM_TEST_UNSYNCED_STORE=1` is in the process's environment, and carries the bytes
//! [`MARKER`]. The command reads the binary and fails, naming it, when it holds them.

use std::fs;
use std::path::Path;

/// The bytes a build holding the test sync switch carries (`loom_governor::durable`).
pub const MARKER: &[u8] = b"b10x-loom-test-unsynced-store-marker";

/// Fails, naming `binary`, when it holds [`MARKER`].
pub fn unsynced_store(binary: &Path) -> Result<(), String> {
    let bytes = fs::read(binary).map_err(|e| format!("read `{}`: {e}", binary.display()))?;
    if holds_marker(&bytes) {
        Err(format!(
            "`{}` holds the test sync switch (the feature `test-unsynced-store` of \
             b10x-loom-governor), which a released binary must not: build it without test targets",
            binary.display()
        ))
    } else {
        println!("`{}` holds no test sync switch", binary.display());
        Ok(())
    }
}

/// Whether `bytes` hold [`MARKER`].
pub fn holds_marker(bytes: &[u8]) -> bool {
    bytes.windows(MARKER.len()).any(|window| window == MARKER)
}
