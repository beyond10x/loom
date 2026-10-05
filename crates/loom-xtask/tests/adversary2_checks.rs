//! Adversary pass 2 on `story:agent-executor`: `no-hand-model` on edge cases of Rust source, and
//! the Taskfile's `generate` path (bootstrap, then `generate` twice) checked for idempotence.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time; copies land under
//! `CARGO_TARGET_TMPDIR`. Every case needs the `ess` binary on `PATH`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn workspace(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary2-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|e| panic!("create {}: {e}", to.display()));
    for entry in fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display())) {
        let entry = entry.unwrap_or_else(|e| panic!("read {}: {e}", from.display()));
        let target = to.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}

fn copy_of_repository(name: &str) -> PathBuf {
    let from = repository();
    let to = workspace(name);
    for part in ["ess", "generated/rust/loom", "crates/loom-executor/src"] {
        copy_tree(&from.join(part), &to.join(part));
    }
    to
}

fn xtask(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn append(file: &Path, text: &str) -> usize {
    let mut body =
        fs::read_to_string(file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let line = body.lines().count() + 1;
    body.push_str(text);
    body.push('\n');
    fs::write(file, body).unwrap_or_else(|e| panic!("write {}: {e}", file.display()));
    line
}

/// A `macro_rules!` that takes the type's name as a parameter defines `Selection` as surely as
/// `pub struct Selection` does, and the story's acceptance 5 is that exact type. The module doc of
/// `loom-xtask` says `no-hand-model` fails when a source file under `crates/loom-executor/src` defines a
/// specified type; the macro scan reads only a type keyword directly followed by a name, and here
/// the keyword is followed by `$name`.
#[test]
fn no_hand_model_refuses_a_model_type_defined_through_a_macro_parameter() {
    let root = copy_of_repository("macro-param");
    let lib = root.join("crates/loom-executor/src/lib.rs");
    append(
        &lib,
        "macro_rules! model {\n    ($name:ident) => {\n        pub struct $name {\n            pub action: String,\n        }\n    };\n}",
    );
    let line = append(&lib, "model!(Selection);");

    let found = xtask(&root, &["no-hand-model"]);
    let named = format!("{}:{line}:", lib.display());
    let _ = fs::remove_dir_all(&root);
    assert!(
        !found.status.success() && stderr(&found).contains(&named),
        "no-hand-model passed a `Selection` defined through `model!(Selection)` at {named} \
         (exit {:?}): {}",
        found.status.code(),
        stderr(&found)
    );
}

/// A type behind a `cfg` the build never enables, or inside a module file below `src/`, is still
/// a hand-written model type.
#[test]
fn no_hand_model_refuses_cfg_gated_and_nested_module_types() {
    let root = copy_of_repository("cfg-nested");
    let lib = root.join("crates/loom-executor/src/lib.rs");
    let cfg_line = append(&lib, "#[cfg(any())]\npub struct Session;") + 1;
    let nested = root.join("crates/loom-executor/src/hidden/deeper.rs");
    fs::create_dir_all(nested.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("create: {e}"));
    fs::write(&nested, "pub mod inner {\n    pub enum Turn { A }\n}\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", nested.display()));

    let found = xtask(&root, &["no-hand-model"]);
    let text = stderr(&found);
    let _ = fs::remove_dir_all(&root);
    assert!(!found.status.success(), "no-hand-model passed: {text}");
    assert!(
        text.contains(&format!("{}:{cfg_line}:", lib.display())),
        "the cfg-gated Session is not named: {text}"
    );
    assert!(
        text.contains(&format!("{}:2:", nested.display())),
        "the nested Turn is not named: {text}"
    );
}

/// The Taskfile's `generate` runs `generate-bootstrap` (the `ess generate synthesize` line, when
/// `generated/rust/loom/Cargo.toml` is absent) and then `loom-xtask generate`. From an absent tree,
/// bootstrap then `generate` twice leaves a tree `drift` accepts, and nothing beside it in
/// `generated/rust/`.
#[test]
fn bootstrap_then_generate_twice_leaves_no_drift_and_no_debris() {
    let taskfile = repository().join("Taskfile.yml");
    let text = fs::read_to_string(&taskfile)
        .unwrap_or_else(|e| panic!("read {}: {e}", taskfile.display()));
    let bootstrap: Vec<String> = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("- ess generate synthesize"))
        .map(|rest| {
            ["generate", "synthesize"]
                .into_iter()
                .map(str::to_owned)
                .chain(rest.split_whitespace().map(str::to_owned))
                .collect()
        })
        .unwrap_or_else(|| panic!("no bootstrap `ess generate synthesize` line in Taskfile.yml"));

    let root = copy_of_repository("bootstrap");
    fs::remove_dir_all(root.join("generated/rust/loom"))
        .unwrap_or_else(|e| panic!("remove generated: {e}"));
    let scratch = root.join("scratch");

    let boot = Command::new("ess")
        .args(&bootstrap)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("run ess: {e}"));
    assert!(boot.status.success(), "bootstrap failed: {}", stderr(&boot));

    for round in 1..=2 {
        let generated = Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
            .arg("--root")
            .arg(&root)
            .args(["generate", "--scratch"])
            .arg(&scratch)
            .output()
            .unwrap_or_else(|e| panic!("run loom-xtask: {e}"));
        assert!(
            generated.status.success(),
            "generate round {round} failed: {}",
            stderr(&generated)
        );
    }
    let drift = Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(&root)
        .args(["drift", "--scratch"])
        .arg(&scratch)
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"));
    let beside: Vec<String> = fs::read_dir(root.join("generated/rust"))
        .unwrap_or_else(|e| panic!("read generated/rust: {e}"))
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "loom")
        .collect();
    let committed = repository().join("generated/rust/loom");
    let same_as_committed = Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(&root)
        .args(["drift", "--scratch"])
        .arg(&scratch)
        .arg("--generated")
        .arg(&committed)
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"));
    let _ = fs::remove_dir_all(&root);
    assert!(
        drift.status.success(),
        "drift after generate: {}",
        stderr(&drift)
    );
    assert!(
        beside.is_empty(),
        "left beside generated/rust/loom: {beside:?}"
    );
    assert!(
        same_as_committed.status.success(),
        "a regeneration differs from the committed tree: {}",
        stderr(&same_as_committed)
    );
}
