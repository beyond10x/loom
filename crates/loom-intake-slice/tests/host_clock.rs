//! The real host clock refuses invalid timezone configuration, in isolated child processes.
use b10x_loom_intake_slice::clock::{Clock, HostClock};
use chrono::Utc;
use std::process::Command;

#[test]
fn host_clock_child() {
    let Ok(expectation) = std::env::var("LOOM_CLOCK_TEST_EXPECT") else {
        return;
    };
    let before = Utc::now();
    let result = HostClock.read();
    let after = Utc::now();
    if expectation == "error" {
        let error = result.expect_err("invalid timezone must not silently produce UTC");
        assert!(error.contains("timezone"), "{error}");
    } else {
        let instant = result.unwrap();
        assert!(
            instant >= before && instant <= after,
            "clock reading must be sampled during this call"
        );
        if expectation != "system" {
            assert_eq!(
                instant.offset().local_minus_utc(),
                expectation.parse::<i32>().unwrap()
            );
        }
    }
}

fn child(timezone: Option<&std::ffi::OsStr>, expectation: &str) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "host_clock_child", "--nocapture"])
        .env("LOOM_CLOCK_TEST_EXPECT", expectation)
        .env_remove("TZ");
    if let Some(timezone) = timezone {
        command.env("TZ", timezone);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn host_timezone_resolution_is_fallible_and_preserves_the_sampled_instant() {
    child(Some("UTC0".as_ref()), "0");
    child(Some("EST5".as_ref()), "-18000");
    child(Some("".as_ref()), "0");
    child(Some("Loom/DefinitelyMissingTimezone".as_ref()), "error");
    child(None, "system");
    let temporary = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temporary.path(), "not a TZif file").unwrap();
    child(Some(temporary.path().as_os_str()), "error");
}

#[cfg(unix)]
#[test]
fn non_unicode_timezone_does_not_silently_use_the_system_default() {
    use std::os::unix::ffi::OsStrExt;
    child(
        Some(std::ffi::OsStr::from_bytes(b"\xff-invalid-timezone")),
        "error",
    );
}
