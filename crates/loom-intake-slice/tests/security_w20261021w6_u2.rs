//! Security review of the delegated harness in `tests/support` (story:confinement-adversary-runs).
//!
//! The harness promises that a delegated case either runs in its scope or skips naming a missing
//! prerequisite: bubblewrap, a user systemd manager or the Substrate driver. These cases hold it
//! to that promise on a host where all three are present: a case the scope never reached, and a
//! refusal that names no missing prerequisite, must fail the parent instead of passing as a skip.

mod support;

use std::path::Path;
use std::process::Command;

use b10x_loom_intake_slice::confinement::SubstrateRunner;

/// The variable the harness sets in the binary it starts inside the scope.
const DELEGATED: &str = "LOOM_DELEGATED_CASE";

/// Whether this host starts a delegated user scope at all; where it does not, a skip is the
/// promised answer and these cases have nothing to hold.
fn delegated_scope_available() -> bool {
    support::bubblewrap_present()
        && Command::new("systemd-run")
            .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
            .arg("/bin/true")
            .output()
            .is_ok_and(|output| output.status.success())
}

/// A name that matches no test of this binary: the scope starts, the binary runs zero tests, and
/// the case is never reached. That is no missing prerequisite, so the parent must not pass.
#[test]
fn security_a_case_the_scope_never_reached_is_not_a_missing_prerequisite() {
    if !delegated_scope_available() {
        eprintln!("skipped: missing prerequisite: a delegated user scope");
        return;
    }
    let outcome = std::panic::catch_unwind(|| {
        support::delegated(
            "security_a_name_no_test_of_this_binary_has",
            &[],
            |_: &SubstrateRunner, _: &Path| {},
        );
    });
    assert!(
        outcome.is_err(),
        "the delegated binary ran no test and never reached the case, yet the parent passed as a \
         skip naming a missing user systemd manager"
    );
}

/// A refusal of the workspace the harness itself made (its temporary directory reached through a
/// symbolic link, which `validate_workspace` refuses `ScopeInvalid`) names no missing
/// prerequisite: bubblewrap, the user systemd manager and the driver are all present. The parent
/// must not pass it as a skip naming the Substrate driver.
#[test]
fn security_a_refusal_naming_no_missing_prerequisite_fails_the_parent() {
    if std::env::var_os(DELEGATED).is_some() {
        support::delegated(
            "security_a_refusal_naming_no_missing_prerequisite_fails_the_parent",
            &[],
            |_: &SubstrateRunner, _: &Path| {},
        );
        return;
    }
    if !delegated_scope_available() {
        eprintln!("skipped: missing prerequisite: a delegated user scope");
        return;
    }
    let base = tempfile::tempdir().unwrap();
    let real = base.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = base.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let outcome = std::panic::catch_unwind(|| {
        support::delegated(
            "security_a_refusal_naming_no_missing_prerequisite_fails_the_parent",
            &[("TMPDIR", link.to_str().unwrap())],
            |_: &SubstrateRunner, _: &Path| {},
        );
    });
    assert!(
        outcome.is_err(),
        "a ScopeInvalid refusal of the harness's own workspace passed as a skip naming the \
         Substrate driver as missing"
    );
}
