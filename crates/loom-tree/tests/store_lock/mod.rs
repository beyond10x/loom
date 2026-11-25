//! The store lock is let go when its `StoreLock` is dropped, even while a forked child still holds
//! a duplicate of the lock file's open description.
//!
//! flock(2) ties the lock to the open file description: it is released by `LOCK_UN` on any
//! duplicate, or else only when every duplicate is closed. A process forked while the lock is held
//! (a host running another run's step) holds a duplicate until it execs. The child here is held
//! between fork and exec by a `pre_exec` that blocks on a pipe the test closes; no sleep, no
//! timing. Scratch is under `CARGO_TARGET_TMPDIR`.

#![cfg(unix)]

use std::fs;
use std::io::{self, Read as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::process::CommandExt as _;
use std::process::Command;

use b10x_loom_tree::{LOCK_FILE, StoreLock};

#[test]
fn a_dropped_lock_is_free_while_a_forked_child_still_holds_its_file() {
    let store = super::scratch("store-lock-fork");
    fs::write(store.join(LOCK_FILE), b"").unwrap();

    // `gate`: the child waits on it between fork and exec until the test closes its write end.
    // `ready`: the child writes one byte to it once it is forked, holding the lock's file.
    let (gate_read, gate_write) = io::pipe().unwrap();
    let (mut ready_read, ready_write) = io::pipe().unwrap();
    let gate_read_fd = gate_read.as_raw_fd();
    let gate_write_fd = gate_write.as_raw_fd();
    let ready_write_fd = ready_write.as_raw_fd();

    let lock = StoreLock::acquire(&store).unwrap();

    let mut command = Command::new("true");
    // SAFETY: the closure calls only `close`, `write` and `read`, which are async-signal-safe, on
    // descriptors the parent keeps open until the child is reaped.
    unsafe {
        command.pre_exec(move || {
            libc::close(gate_write_fd);
            libc::write(ready_write_fd, b"x".as_ptr().cast(), 1);
            let mut byte = 0u8;
            loop {
                let read = libc::read(gate_read_fd, (&raw mut byte).cast(), 1);
                if read == 0 {
                    break;
                }
                if read < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    break;
                }
            }
            Ok(())
        });
    }
    // `spawn` returns only once the child has exec'd, so it runs beside the test.
    let child = std::thread::spawn(move || command.spawn().unwrap().wait().unwrap());

    let mut byte = [0u8; 1];
    ready_read.read_exact(&mut byte).unwrap();
    // The child is forked and holds a duplicate of the lock file's description until it execs.
    drop(lock);
    let again = StoreLock::acquire(&store);

    drop(gate_write);
    let status = child.join().unwrap();
    drop((gate_read, ready_write));

    assert!(status.success(), "{status}");
    let again = again.expect("the dropped lock is free while the forked child holds its file");
    drop(again);
    let _ = fs::remove_dir_all(&store);
}
