//! Adversarial cases for `story:writable-dirs-mode-0700`: what `b10x-loom agent prepare-dirs`
//! finds in place of a directory, and the mode bits a rerun meets.

use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adv_cmdv_u5_prepare_dirs")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// Runs `prepare-dirs` over `dirs`, killing it after 10 s: `None` when it had to be killed.
fn prepare(dirs: &[&Path]) -> Option<i32> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
    command
        .args(["agent", "prepare-dirs"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for dir in dirs {
        command.arg("--dir").arg(dir);
    }
    let mut child = command.spawn().expect("b10x-loom runs");
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().expect("kill");
            child.wait().expect("reaped");
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o7777
}

/// A FIFO in place of the directory: an open for reading would wait for a writer, so the
/// command must refuse it rather than hang the init container.
#[test]
fn adv_a_fifo_in_place_of_a_dir_is_refused_without_blocking() {
    let base = scratch("fifo");
    let fifo = base.join("tmp");
    let path = CString::new(fifo.as_os_str().as_bytes()).expect("a C path");
    // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
    let made = unsafe { libc::mkfifo(path.as_ptr(), 0o600) };
    assert_eq!(made, 0, "mkfifo");
    assert_eq!(prepare(&[&fifo]), Some(1), "refused, and in time");
}

/// An existing directory of this user with setuid, setgid, sticky and every permission bit set
/// leaves exactly 0700; naming it twice is no error.
#[test]
fn adv_every_special_bit_is_cleared_and_a_repeated_dir_is_no_error() {
    let base = scratch("bits");
    let dir = base.join("home");
    fs::create_dir(&dir).expect("dir");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o7777)).expect("all bits");
    assert_eq!(prepare(&[&dir, &dir]), Some(0));
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(
        fs::metadata(&dir).expect("dir").uid(),
        fs::metadata(&base).expect("base").uid()
    );
}

/// A dangling link in place of the directory is refused and nothing is made where it points.
#[test]
fn adv_a_dangling_link_is_refused_and_not_followed() {
    let base = scratch("dangling");
    let target = base.join("made-through-the-link");
    let link = base.join("tmp");
    std::os::unix::fs::symlink(&target, &link).expect("a dangling link");
    assert_eq!(prepare(&[&link]), Some(1));
    assert!(!target.exists(), "nothing made through the link");
}
