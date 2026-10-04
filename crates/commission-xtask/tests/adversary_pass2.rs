//! Adversary pass 2: the drift binding, a rename to a generated name and the drift step's
//! naming of a changed generated file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn case_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary2")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear case dir");
    }
    fs::create_dir_all(&dir).expect("create case dir");
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy dir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// What cargo and the xtask read, copied into `<case>/root`; `.ess-output/` left out.
fn repo_copy(case: &Path) -> PathBuf {
    let root = repo_root();
    let copy = case.join("root");
    fs::create_dir_all(&copy).expect("create root copy");
    for file in ["Cargo.toml", "Cargo.lock", "Taskfile.yml"] {
        fs::copy(root.join(file), copy.join(file)).expect("copy root file");
    }
    for dir in [
        "ess",
        "crates/commission",
        "crates/commission-xtask",
        "generated/rust/commission",
    ] {
        copy_tree(&root.join(dir), &copy.join(dir));
    }
    let state = copy.join("generated/rust/commission/.ess-output");
    if state.exists() {
        fs::remove_dir_all(&state).expect("drop .ess-output");
    }
    copy
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commission-xtask"))
        .args(args)
        .output()
        .expect("run commission-xtask")
}

/// `b10x-commission` re-exports some other crate as `model`, while the manifest still carries the
/// `commission` path dependency. `drift` promises to fail unless the crate root re-exports the
/// generated crate; no other case changes only the name being re-exported.
#[test]
fn adversary2_drift_refuses_model_reexported_from_another_crate() {
    let case = case_dir("drift_refuses_model_reexported_from_another_crate");
    let root = repo_copy(&case);
    let lib = root.join("crates/commission/src/lib.rs");
    let body = fs::read_to_string(&lib).expect("read lib.rs");
    assert!(body.contains("pub use commission as model;"));
    fs::write(
        &lib,
        body.replace(
            "pub use commission as model;",
            "pub use b10x_canon as model;",
        ),
    )
    .expect("write lib.rs");

    let scratch = case.join("scratch");
    let out = xtask(&[
        "--root",
        root.to_str().expect("utf-8 root"),
        "drift",
        "--scratch",
        scratch.to_str().expect("utf-8 scratch"),
    ]);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("crates/commission/src/lib.rs"),
        "drift passed `pub use b10x_canon as model;`:\n{stderr}"
    );
}

/// A rename of some other type to a generated type's name (here `ExecutorOutcome`) is refused.
#[test]
fn adversary2_no_hand_model_refuses_a_rename_to_a_generated_name() {
    let case = case_dir("no_hand_model_refuses_a_rename_to_a_generated_name");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "mod hand {\n    pub struct Handmade;\n}\n\npub use hand::Handmade as ExecutorOutcome;\n",
    )
    .expect("write lib.rs");
    let generated = repo_root().join("generated/rust/commission");
    let out = xtask(&[
        "no-hand-model",
        "--src",
        src.to_str().expect("utf-8 src"),
        "--generated",
        generated.to_str().expect("utf-8 generated"),
    ]);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("lib.rs:5"),
        "no-hand-model allowed `use Handmade as ExecutorOutcome`:\n{stderr}"
    );
}

/// Story Outcome, Drift: "It fails on any difference and names the file." The drift step is
/// `cargo run -q --locked -p commission-xtask -- drift` (Taskfile `drift`).
///
/// Known limit of story:generated-responsibility-model: a changed version in the generated crate's
/// manifest makes cargo's `--locked` check refuse before the drift step runs, so the step fails but
/// does not name the file. The assertion pins that the change is still caught; once drift runs
/// before cargo's lock check, this case should also assert that the step names `Cargo.toml`.
#[test]
fn adversary2_known_limit_drift_step_fails_on_a_changed_generated_manifest() {
    let case = case_dir("drift_step_names_a_changed_byte_in_the_generated_manifest");
    let root = repo_copy(&case);
    let manifest = root.join("generated/rust/commission/Cargo.toml");
    let body = fs::read_to_string(&manifest).expect("read generated Cargo.toml");
    assert!(body.contains("version = \"1.0.0\""));
    fs::write(
        &manifest,
        body.replace("version = \"1.0.0\"", "version = \"1.0.1\""),
    )
    .expect("write generated Cargo.toml");

    let scratch = case.join("scratch");
    let out = Command::new(option_env!("CARGO").unwrap_or("cargo"))
        .args(["run", "-q", "--locked", "-p", "commission-xtask", "--"])
        .args([
            "drift",
            "--scratch",
            scratch.to_str().expect("utf-8 scratch"),
        ])
        .current_dir(&root)
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_TARGET_TMPDIR")).join("adversary2-target"),
        )
        .output()
        .expect("run the drift step");
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success(),
        "story:generated-responsibility-model: the drift step passed a changed version in the \
         generated Cargo.toml. Today cargo's --locked check refuses it before drift runs; once \
         drift runs before that check, the step should fail and name `Cargo.toml`:\n{stderr}"
    );
}
