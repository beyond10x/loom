//! Adversary pass 2 (wave 2026-10-04-w3, unit commission/port-skeleton): what `no-hand-model`
//! reads.
//!
//! The check promises to refuse a source file of `b10x-commission` or `b10x-commission-testkit`
//! that defines a type the generated crate declares. It reads each `.rs` file under the scanned
//! directories as a syntax tree and visits items. Two ways to define such a type are not items
//! in that tree: an item a macro expands to, and a module file a `#[path]` attribute loads from
//! outside the scanned directory. Both compile into the crate; neither is refused.
//!
//! Each case runs the real `commission-xtask` binary of this build over a source directory it
//! writes, against the generated crate of the tree this test sits in (located through
//! `CARGO_MANIFEST_DIR`, read when the test runs).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo");
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn case_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary2_scanner_reach")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear case dir");
    }
    fs::create_dir_all(&dir).expect("create case dir");
    dir
}

fn no_hand_model(src: &Path) -> Output {
    let root = root();
    std::process::Command::new(env!("CARGO_BIN_EXE_commission-xtask"))
        .arg("--root")
        .arg(&root)
        .arg("no-hand-model")
        .arg("--src")
        .arg(src)
        .arg("--generated")
        .arg(root.join("generated/rust/commission"))
        .output()
        .expect("run commission-xtask")
}

fn assert_refused(out: &Output, item: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains(item),
        "no-hand-model passed a hand-written `{item}`:\nstatus: {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        out.status
    );
}

/// A `macro_rules!` whose expansion is `pub enum GovernorError { … }`, invoked once at item
/// position: the crate then exports a hand-written `GovernorError` beside the generated one.
#[test]
fn adversary2_no_hand_model_refuses_a_model_type_a_macro_expands_to() {
    let case = case_dir("macro_expansion");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "macro_rules! stamp {\n    () => {\n        pub enum GovernorError {\n            \
         UnknownCase,\n            GovernorUnavailable,\n        }\n    };\n}\n\nstamp!();\n",
    )
    .expect("write lib.rs");

    let out = no_hand_model(&src);
    let _ = fs::remove_dir_all(&case);
    assert_refused(&out, "GovernorError");
}

/// `#[path = "../elsewhere/hand.rs"] mod hand;` compiles `hand.rs` into the crate as a module, but
/// the file lies outside the scanned directory, so its `pub struct RunOutcome` is never read.
#[test]
fn adversary2_no_hand_model_follows_a_path_attribute_out_of_the_scanned_directory() {
    let case = case_dir("path_attribute");
    let src = case.join("src");
    let elsewhere = case.join("elsewhere");
    fs::create_dir_all(&src).expect("create src");
    fs::create_dir_all(&elsewhere).expect("create elsewhere");
    fs::write(
        src.join("lib.rs"),
        "#[path = \"../elsewhere/hand.rs\"]\npub mod hand;\n",
    )
    .expect("write lib.rs");
    fs::write(elsewhere.join("hand.rs"), "pub struct RunOutcome;\n").expect("write hand.rs");

    let out = no_hand_model(&src);
    let _ = fs::remove_dir_all(&case);
    // Coordinator decision (wave 2026-10-04-w3): the check refuses the `#[path]` module without
    // following it, so the refusal names the attribute in lib.rs, not the type in hand.rs.
    assert_refused(&out, "lib.rs:1");
    assert_refused(&out, "#[path");
}
