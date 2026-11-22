//! The release gate's marker check (`binaries.yml`: `loom-xtask unsynced-store`) is only worth
//! something if a build that holds the test sync switch carries the marker it refuses.

use std::fs;

/// The bytes `loom-xtask unsynced-store` refuses (`crates/loom-xtask/src/unsynced_store.rs`).
const MARKER: &[u8] = b"b10x-loom-test-unsynced-store-marker";

/// The `b10x-loom` these tests start holds the switch (`b10x-loom-cli`'s dev-dependencies enable
/// the feature), so it carries the marker.
#[test]
fn a_build_with_the_sync_switch_carries_the_marker_the_release_check_refuses() {
    let bytes = fs::read(env!("CARGO_BIN_EXE_b10x-loom")).unwrap();
    assert!(
        bytes.windows(MARKER.len()).any(|window| window == MARKER),
        "the test build of b10x-loom holds the test sync switch but not its marker: \
         `loom-xtask unsynced-store` would pass a binary that can skip fsync"
    );
}
