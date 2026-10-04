//! Adversary pass 2 on `story:run-outcomes`: what the widened `no-hand-model` reserves.
//!
//! The unit widened the reserved names to traits declared in the generated crate's inline modules,
//! to catch a hand-written `StartRunBehavior` or `run_state::Marker`. The recursion also reaches
//! each state module's private `mod sealed { pub trait Sealed {} }`: a trait nobody outside the
//! generated crate can name, so a hand-written `Sealed` cannot be a copy of a model type. The
//! sealed-trait idiom is ordinary Rust, and the check refuses it as "a hand-written model type".
//!
//! Runs the real `commission-xtask` binary of this build over a source directory it writes,
//! against the generated crate of the tree this test sits in (located through
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
        .join("adversary2_run_nested_traits")
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

/// A private sealed trait in hand-written code names nothing the generated crate exports.
#[test]
fn adversary2_run_a_hand_written_sealed_trait_is_not_a_model_type() {
    let generated = root().join("generated/rust/commission/src/responsibility.rs");
    let source = fs::read_to_string(&generated).expect("read generated responsibility.rs");
    assert!(
        source.contains("    mod sealed {\n") && source.contains("pub trait Sealed {}"),
        "the generated crate no longer declares a private `sealed::Sealed`; this case is moot"
    );

    let case = case_dir("sealed");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "mod sealed {\n    pub trait Sealed {}\n}\n\n\
         /// Closed over this crate's own kinds.\n\
         pub trait Kind: sealed::Sealed {}\n",
    )
    .expect("write lib.rs");

    let out = no_hand_model(&src);
    assert!(
        out.status.success(),
        "no-hand-model refused a private sealed trait that copies no generated type:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
