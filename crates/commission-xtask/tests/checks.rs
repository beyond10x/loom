//! Acceptance cases for the `drift` and `no-hand-model` checks, run against the binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MODEL_NAMES: [&str; 5] = [
    "AgentId",
    "AgentRevisionId",
    "CaseId",
    "CommissionId",
    "Commission",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

/// A fresh directory for one case, under cargo's per-target temporary directory.
fn case_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
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

fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commission-xtask"))
        .args(args)
        .output()
        .expect("run commission-xtask")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs `drift` against `generated`, regenerating from the repository's own specification.
fn drift(case: &Path, generated: &Path) -> Output {
    let root = repo_root();
    xtask(&[
        "--root",
        root.to_str().expect("utf-8 root"),
        "drift",
        "--scratch",
        case.join("scratch").to_str().expect("utf-8 scratch"),
        "--generated",
        generated.to_str().expect("utf-8 generated"),
    ])
}

fn committed_copy(case: &Path) -> PathBuf {
    let copy = case.join("committed");
    copy_tree(&repo_root().join("generated/rust/commission"), &copy);
    let state = copy.join(".ess-output");
    if state.exists() {
        fs::remove_dir_all(&state).expect("drop .ess-output from the copy");
    }
    copy
}

fn no_hand_model(src: &Path) -> Output {
    xtask(&["no-hand-model", "--src", src.to_str().expect("utf-8 src")])
}

#[test]
fn drift_passes_on_the_committed_tree() {
    let case = case_dir("drift_passes_on_the_committed_tree");
    let out = drift(&case, &repo_root().join("generated/rust/commission"));
    assert!(
        out.status.success(),
        "drift refused the committed tree:\n{}",
        text(&out.stderr)
    );
}

#[test]
fn drift_fails_naming_a_changed_byte_in_responsibility_rs() {
    let case = case_dir("drift_fails_naming_a_changed_byte_in_responsibility_rs");
    let copy = committed_copy(&case);
    let file = copy.join("src/responsibility.rs");
    let mut bytes = fs::read(&file).expect("read responsibility.rs");
    let middle = bytes.len() / 2;
    bytes[middle] = if bytes[middle] == b'x' { b'y' } else { b'x' };
    fs::write(&file, bytes).expect("write responsibility.rs");

    let out = drift(&case, &copy);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success(),
        "drift passed a changed byte:\n{stderr}"
    );
    assert!(
        stderr.contains("src/responsibility.rs"),
        "drift did not name the changed file:\n{stderr}"
    );
}

#[test]
fn drift_names_a_file_missing_from_the_committed_tree() {
    let case = case_dir("drift_names_a_file_missing_from_the_committed_tree");
    let copy = committed_copy(&case);
    fs::remove_file(copy.join("PLAN.md")).expect("remove PLAN.md");

    let out = drift(&case, &copy);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success(),
        "drift passed a missing file:\n{stderr}"
    );
    assert!(
        stderr.contains("PLAN.md"),
        "missing file not named:\n{stderr}"
    );
}

#[test]
fn drift_names_a_file_the_specification_does_not_produce() {
    let case = case_dir("drift_names_a_file_the_specification_does_not_produce");
    let copy = committed_copy(&case);
    fs::write(copy.join("src/extra.rs"), "pub struct Extra;\n").expect("write extra file");

    let out = drift(&case, &copy);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success(),
        "drift passed a stray file:\n{stderr}"
    );
    assert!(
        stderr.contains("src/extra.rs"),
        "stray file not named:\n{stderr}"
    );
}

#[test]
fn drift_ignores_ess_output() {
    let case = case_dir("drift_ignores_ess_output");
    let copy = committed_copy(&case);
    fs::create_dir_all(copy.join(".ess-output")).expect("create .ess-output");
    fs::write(
        copy.join(".ess-output/state.json"),
        "{\"anchor_id\":\"other\"}",
    )
    .expect("write state.json");

    let out = drift(&case, &copy);
    assert!(
        out.status.success(),
        "drift compared .ess-output:\n{}",
        text(&out.stderr)
    );
}

#[test]
fn drift_leaves_no_regeneration_tree_behind() {
    let case = case_dir("drift_leaves_no_regeneration_tree_behind");
    let copy = committed_copy(&case);
    let out = drift(&case, &copy);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let left: Vec<_> = fs::read_dir(case.join("scratch"))
        .map(|entries| entries.map(|e| e.expect("entry").path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "regeneration trees left behind: {left:?}");
}

#[test]
fn no_hand_model_passes_on_the_tree() {
    let out = no_hand_model(&repo_root().join("crates/commission/src"));
    assert!(
        out.status.success(),
        "no-hand-model refused the tree:\n{}",
        text(&out.stderr)
    );
}

#[test]
fn no_hand_model_names_a_hand_written_agent_id() {
    let case = case_dir("no_hand_model_names_a_hand_written_agent_id");
    let src = case.join("src");
    copy_tree(&repo_root().join("crates/commission/src"), &src);
    let lib = src.join("lib.rs");
    let mut body = fs::read_to_string(&lib).expect("read lib.rs");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("pub struct AgentId(pub String);\n");
    let line = body.lines().count();
    fs::write(&lib, body).expect("write lib.rs");

    let out = no_hand_model(&src);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success(),
        "no-hand-model passed a hand-written AgentId:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("lib.rs:{line}")) && stderr.contains("AgentId"),
        "no-hand-model did not name lib.rs:{line} and AgentId:\n{stderr}"
    );
}

#[test]
fn no_hand_model_names_every_model_type_in_any_source_file() {
    for name in MODEL_NAMES {
        for item in [
            "pub struct",
            "struct",
            "pub(crate) struct",
            "pub enum",
            "pub type",
        ] {
            let case = case_dir("no_hand_model_names_every_model_type_in_any_source_file");
            let src = case.join("src");
            fs::create_dir_all(src.join("nested")).expect("create nested");
            fs::write(src.join("lib.rs"), "pub mod nested;\n").expect("write lib.rs");
            let tail = if item.ends_with("type") {
                " = String;"
            } else if item.ends_with("enum") {
                " { Value }"
            } else {
                " { pub value: String }"
            };
            fs::write(
                src.join("nested/mod.rs"),
                format!("// header\n\n{item} {name}{tail}\n"),
            )
            .expect("write nested/mod.rs");

            let out = no_hand_model(&src);
            let stderr = text(&out.stderr);
            assert!(!out.status.success(), "`{item} {name}` passed:\n{stderr}");
            assert!(
                stderr.contains("nested/mod.rs:3") && stderr.contains(name),
                "`{item} {name}` not named at nested/mod.rs:3:\n{stderr}"
            );
        }
    }
}

/// `story:run-outcomes`: a hand-written `enum RunOutcome` in the module that derives outcomes is
/// refused, and so is a hand-written copy of each type the Run commands bring into the generated
/// crate — their ports, inputs, outcomes, events, error and view.
#[test]
fn no_hand_model_refuses_a_hand_written_run_outcome_and_run_command_types() {
    for (item, name) in [
        ("pub enum", "RunOutcome"),
        ("pub trait", "RunStorage"),
        ("pub trait", "Context"),
        ("pub struct", "Generated"),
        ("pub struct", "UnmetObligation"),
        ("pub struct", "StartRun"),
        ("pub enum", "StartRunOutcome"),
        ("pub struct", "SuspendRun"),
        ("pub enum", "SuspendRunOutcome"),
        ("pub struct", "ResumeRun"),
        ("pub enum", "ResumeRunOutcome"),
        ("pub struct", "RunStarted"),
        ("pub struct", "RunSuspended"),
        ("pub struct", "RunResumed"),
        ("pub struct", "RunStateConflict"),
        ("pub struct", "RunStates"),
    ] {
        let case =
            case_dir("no_hand_model_refuses_a_hand_written_run_outcome_and_run_command_types");
        let src = case.join("src");
        copy_tree(&repo_root().join("crates/commission/src"), &src);
        let outcome = src.join("outcome.rs");
        let mut body = fs::read_to_string(&outcome).expect("read outcome.rs");
        if !body.ends_with('\n') {
            body.push('\n');
        }
        let tail = if item.ends_with("trait") {
            " {}"
        } else {
            " { A }"
        };
        let tail = if item.ends_with("struct") { ";" } else { tail };
        body.push_str(&format!("{item} {name}{tail}\n"));
        let line = body.lines().count();
        fs::write(&outcome, body).expect("write outcome.rs");

        let out = no_hand_model(&src);
        let stderr = text(&out.stderr);
        let keyword = item.trim_start_matches("pub ");
        assert!(
            !out.status.success()
                && stderr.lines().any(|found| {
                    found.contains(&format!("outcome.rs:{line}"))
                        && found.contains(&format!("`{keyword} {name}`"))
                }),
            "no-hand-model passed a hand-written `{item} {name}` in outcome.rs:\n{stderr}"
        );
    }
}

/// The generated crate declares traits inside modules as well as at the top of a file:
/// `obligations::StartRunBehavior`, `obligations::RunStatesQuery`, `run_state::Marker`. A
/// hand-written trait with such a name is refused, naming its file and line.
#[test]
fn no_hand_model_refuses_a_trait_named_after_a_nested_generated_trait() {
    for name in [
        "StartRunBehavior",
        "SuspendRunBehavior",
        "ResumeRunBehavior",
        "RunStatesQuery",
        "Marker",
    ] {
        let case = case_dir("no_hand_model_refuses_a_trait_named_after_a_nested_generated_trait");
        let src = case.join("src");
        copy_tree(&repo_root().join("crates/commission/src"), &src);
        let outcome = src.join("outcome.rs");
        let mut body = fs::read_to_string(&outcome).expect("read outcome.rs");
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("pub trait {name} {{}}\n"));
        let line = body.lines().count();
        fs::write(&outcome, body).expect("write outcome.rs");

        let out = no_hand_model(&src);
        let stderr = text(&out.stderr);
        assert!(
            !out.status.success()
                && stderr.lines().any(|found| {
                    found.contains(&format!("outcome.rs:{line}"))
                        && found.contains(&format!("`trait {name}`"))
                }),
            "no-hand-model passed a hand-written `trait {name}` in outcome.rs:\n{stderr}"
        );
    }
}

#[test]
fn no_hand_model_does_not_flag_uses_comments_or_longer_names() {
    let case = case_dir("no_hand_model_does_not_flag_uses_comments_or_longer_names");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "use b10x_canon::CaseId;\n\
         // the hand-written struct Commission was deleted\n\
         /// pub struct AgentId(pub String);\n\
         pub struct CommissionRecord;\n\
         pub struct AgentIdentity;\n\
         pub fn takes(_: &CaseId) {}\n",
    )
    .expect("write lib.rs");

    let out = no_hand_model(&src);
    assert!(
        out.status.success(),
        "no-hand-model flagged a non-definition:\n{}",
        text(&out.stderr)
    );
}

/// Copies what a run of the xtask or the Taskfile reads into `<case>/root`.
fn repo_copy(case: &Path, with_generated: bool) -> PathBuf {
    let root = repo_root();
    let copy = case.join("root");
    for file in ["Cargo.toml", "Cargo.lock", "Taskfile.yml"] {
        fs::create_dir_all(&copy).expect("create root copy");
        fs::copy(root.join(file), copy.join(file)).expect("copy root file");
    }
    for dir in [
        "ess",
        "crates/commission",
        "crates/commission-docs",
        "crates/commission-testkit",
        "crates/commission-xtask",
        "crates/commission-conformance",
    ] {
        copy_tree(&root.join(dir), &copy.join(dir));
    }
    if with_generated {
        copy_tree(
            &root.join("generated/rust/commission"),
            &copy.join("generated/rust/commission"),
        );
        let state = copy.join("generated/rust/commission/.ess-output");
        if state.exists() {
            fs::remove_dir_all(&state).expect("drop .ess-output from the copy");
        }
    }
    copy
}

/// Every file under `dir`, relative to it, with its bytes; `.ess-output/` left out.
fn tree_bytes(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(base: &Path, relative: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(base.join(relative)).expect("read dir") {
            let entry = entry.expect("dir entry");
            let path = relative.join(entry.file_name());
            if path == Path::new(".ess-output") {
                continue;
            }
            if entry.file_type().expect("file type").is_dir() {
                walk(base, &path, out);
            } else {
                out.push((path.clone(), fs::read(base.join(&path)).expect("read file")));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, Path::new(""), &mut out);
    out.sort();
    out
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf-8 path")
}

fn xtask_at(root: &Path, args: &[&str]) -> Output {
    let mut all = vec!["--root", path_str(root)];
    all.extend_from_slice(args);
    xtask(&all)
}

#[test]
fn no_hand_model_scans_the_testkit_by_default() {
    let case = case_dir("no_hand_model_scans_the_testkit_by_default");
    let root = repo_copy(&case, true);
    let fake = root.join("crates/commission-testkit/src/fake_executor.rs");
    fs::write(
        &fake,
        "pub enum ExecutorOutcome {\n    NoUsefulAction,\n}\n",
    )
    .expect("write fake_executor.rs");

    let out = xtask_at(&root, &["no-hand-model"]);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success()
            && stderr.lines().any(|line| {
                line.contains("crates/commission-testkit/src/fake_executor.rs:1")
                    && line.contains("enum ExecutorOutcome")
            }),
        "no-hand-model passed a hand-written ExecutorOutcome in the testkit:\nstdout:\n{}\nstderr:\n{stderr}",
        text(&out.stdout)
    );
}

#[test]
fn no_hand_model_reads_model_names_from_the_generated_crate() {
    let case = case_dir("no_hand_model_reads_model_names_from_the_generated_crate");
    let generated = case.join("generated");
    fs::create_dir_all(generated.join("src")).expect("create generated src");
    fs::write(
        generated.join("src/lib.rs"),
        "pub mod zoo;\npub struct Zebra;\n",
    )
    .expect("write generated lib.rs");
    fs::write(generated.join("src/zoo.rs"), "pub enum Okapi { One }\n")
        .expect("write generated zoo.rs");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");

    for (body, name) in [
        ("pub struct Zebra;\n", Some("Zebra")),
        ("pub struct Okapi(pub u8);\n", Some("Okapi")),
        ("pub struct AgentId(pub String);\n", None),
    ] {
        fs::write(src.join("lib.rs"), body).expect("write lib.rs");
        let out = xtask(&[
            "no-hand-model",
            "--src",
            path_str(&src),
            "--generated",
            path_str(&generated),
        ]);
        let stderr = text(&out.stderr);
        match name {
            Some(name) => assert!(
                !out.status.success() && stderr.contains(name),
                "`{body}` passed although the generated crate declares {name}:\n{stderr}"
            ),
            None => assert!(
                out.status.success(),
                "`{body}` refused although this generated crate does not declare it:\n{stderr}"
            ),
        }
    }
}

#[test]
fn no_hand_model_refuses_a_generated_crate_that_declares_nothing() {
    let case = case_dir("no_hand_model_refuses_a_generated_crate_that_declares_nothing");
    let generated = case.join("generated");
    fs::create_dir_all(generated.join("src")).expect("create generated src");
    fs::write(generated.join("src/lib.rs"), "").expect("write generated lib.rs");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("lib.rs"), "pub struct Anything;\n").expect("write lib.rs");

    let out = xtask(&[
        "no-hand-model",
        "--src",
        path_str(&src),
        "--generated",
        path_str(&generated),
    ]);
    assert!(
        !out.status.success(),
        "no-hand-model passed with no model names to check:\n{}",
        text(&out.stdout)
    );
}

#[test]
fn no_hand_model_ignores_literals_and_block_comments() {
    let case = case_dir("no_hand_model_ignores_literals_and_block_comments");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "/* pub struct AgentId(pub String); */\n\
         pub const A: &str = \"pub struct Commission;\";\n\
         pub const B: &str = r#\"enum CaseId {}\"#;\n",
    )
    .expect("write lib.rs");
    let out = no_hand_model(&src);
    assert!(
        out.status.success(),
        "no-hand-model flagged a literal or block comment:\n{}",
        text(&out.stderr)
    );
}

#[test]
fn no_hand_model_names_a_use_rename_to_a_model_name() {
    let case = case_dir("no_hand_model_names_a_use_rename_to_a_model_name");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(
        src.join("lib.rs"),
        "mod hand { pub struct Handmade; }\n\npub use hand::{Handmade as CommissionData};\n",
    )
    .expect("write lib.rs");
    let out = no_hand_model(&src);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("lib.rs:3") && stderr.contains("CommissionData"),
        "no-hand-model did not name the rename at lib.rs:3:\n{stderr}"
    );
}

#[test]
fn no_hand_model_refuses_a_source_it_cannot_parse() {
    let case = case_dir("no_hand_model_refuses_a_source_it_cannot_parse");
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("lib.rs"), "pub struct AgentId(\n").expect("write lib.rs");
    let out = no_hand_model(&src);
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("lib.rs"),
        "no-hand-model passed a source it cannot read as Rust:\n{stderr}"
    );
}

#[test]
fn check_runs_drift_and_no_hand_model_before_any_cargo_step() {
    let taskfile = fs::read_to_string(repo_root().join("Taskfile.yml")).expect("read Taskfile");
    let check = taskfile
        .split("\n  check:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .expect("a check task");
    let position = |needle: &str| {
        check
            .find(needle)
            .unwrap_or_else(|| panic!("check does not run `{needle}`:\n{check}"))
    };
    let first_cargo = check.find("cargo ").expect("check runs cargo");
    for step in ["task: drift", "task: no-hand-model"] {
        assert!(
            position(step) < first_cargo,
            "`{step}` runs after a cargo step, so a generated file that does not compile stops \
             check before it:\n{check}"
        );
    }
}

#[test]
fn drift_fails_when_the_model_dependency_points_elsewhere() {
    let case = case_dir("drift_fails_when_the_model_dependency_points_elsewhere");
    let root = repo_copy(&case, true);
    let manifest = root.join("crates/commission/Cargo.toml");
    let body = fs::read_to_string(&manifest).expect("read manifest");
    fs::write(
        &manifest,
        body.replace("../../generated/rust/commission", "../hand-model"),
    )
    .expect("write manifest");

    let out = xtask_at(
        &root,
        &["drift", "--scratch", path_str(&case.join("scratch"))],
    );
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("crates/commission/Cargo.toml"),
        "drift passed a model dependency outside generated/rust/commission:\n{stderr}"
    );
}

#[test]
fn drift_fails_when_another_dependency_is_the_commission_package() {
    let case = case_dir("drift_fails_when_another_dependency_is_the_commission_package");
    let root = repo_copy(&case, true);
    let manifest = root.join("crates/commission/Cargo.toml");
    let mut body = fs::read_to_string(&manifest).expect("read manifest");
    body.push_str("hand = { package = \"commission\", path = \"../hand-model\" }\n");
    fs::write(&manifest, body).expect("write manifest");

    let out = xtask_at(
        &root,
        &["drift", "--scratch", path_str(&case.join("scratch"))],
    );
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("hand"),
        "drift passed a second `commission` package outside generated/rust/commission:\n{stderr}"
    );
}

#[test]
fn drift_fails_when_model_is_not_reexported_from_the_generated_crate() {
    let case = case_dir("drift_fails_when_model_is_not_reexported_from_the_generated_crate");
    let root = repo_copy(&case, true);
    let lib = root.join("crates/commission/src/lib.rs");
    let body = fs::read_to_string(&lib).expect("read lib.rs");
    fs::write(
        &lib,
        body.replace("pub use commission as model;", "pub mod model {}"),
    )
    .expect("write lib.rs");

    let out = xtask_at(
        &root,
        &["drift", "--scratch", path_str(&case.join("scratch"))],
    );
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("model"),
        "drift passed a b10x-commission whose `model` is not the generated crate:\n{stderr}"
    );
}

#[test]
fn generate_writes_the_tree_when_it_is_absent() {
    let case = case_dir("generate_writes_the_tree_when_it_is_absent");
    let root = repo_copy(&case, false);
    let out = xtask_at(
        &root,
        &["generate", "--scratch", path_str(&case.join("scratch"))],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    let written = root.join("generated/rust/commission");
    assert!(
        !written.join(".ess-output").exists(),
        "generate kept .ess-output"
    );
    assert_eq!(
        tree_bytes(&written),
        tree_bytes(&repo_root().join("generated/rust/commission")),
        "generate into an absent tree does not match the committed tree"
    );
}

#[test]
fn generate_replaces_a_stale_tree_and_removes_files_synthesis_no_longer_writes() {
    let case =
        case_dir("generate_replaces_a_stale_tree_and_removes_files_synthesis_no_longer_writes");
    let root = repo_copy(&case, true);
    let tree = root.join("generated/rust/commission");
    fs::write(tree.join("src/stale.rs"), "pub struct Stale;\n").expect("write stale file");
    fs::write(tree.join("PLAN.md"), "edited\n").expect("edit PLAN.md");

    let out = xtask_at(
        &root,
        &["generate", "--scratch", path_str(&case.join("scratch"))],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        tree_bytes(&tree),
        tree_bytes(&repo_root().join("generated/rust/commission")),
        "generate left a stale or edited file"
    );
    let siblings: Vec<_> = fs::read_dir(root.join("generated/rust"))
        .expect("read generated/rust")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(
        siblings,
        vec!["commission"],
        "generate left staging trees behind"
    );
}

#[test]
fn generate_keeps_the_committed_tree_when_synthesis_fails() {
    let case = case_dir("generate_keeps_the_committed_tree_when_synthesis_fails");
    let root = repo_copy(&case, true);
    fs::write(root.join("ess/system.yaml"), "format: ess/20\nsystem: [\n")
        .expect("break the specification");
    let tree = root.join("generated/rust/commission");
    let before = tree_bytes(&tree);

    let out = xtask_at(
        &root,
        &["generate", "--scratch", path_str(&case.join("scratch"))],
    );
    assert!(
        !out.status.success(),
        "generate passed over a broken specification"
    );
    assert_eq!(
        tree_bytes(&tree),
        before,
        "generate changed the tree it could not replace"
    );
    let siblings: Vec<_> = fs::read_dir(root.join("generated/rust"))
        .expect("read generated/rust")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(
        siblings,
        vec!["commission"],
        "generate left staging trees behind"
    );
}

/// `task generate` from a checkout with no generated tree: cargo cannot load the workspace
/// until the model crate's manifest exists, so the task must bootstrap it before the xtask runs.
#[test]
fn task_generate_writes_the_tree_when_it_is_absent() {
    let case = case_dir("task_generate_writes_the_tree_when_it_is_absent");
    let root = repo_copy(&case, false);
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("task-generate-target");
    let out = Command::new("task")
        .arg("generate")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("run task generate");
    assert!(
        out.status.success(),
        "task generate failed without a generated tree:\n{}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let written = root.join("generated/rust/commission");
    assert!(
        !written.join(".ess-output").exists(),
        "task generate kept .ess-output"
    );
    assert_eq!(
        tree_bytes(&written),
        tree_bytes(&repo_root().join("generated/rust/commission")),
        "task generate into an absent tree does not match the committed tree"
    );
}

/// Runs `no-hand-model` over a one-file crate whose `lib.rs` is `body`.
fn no_hand_model_on(case_name: &str, body: &str) -> Output {
    let case = case_dir(case_name);
    let src = case.join("src");
    fs::create_dir_all(&src).expect("create src");
    fs::write(src.join("lib.rs"), body).expect("write lib.rs");
    no_hand_model(&src)
}

fn assert_refused_at(out: &Output, line: usize, needles: &[&str]) {
    let stderr = text(&out.stderr);
    assert!(
        !out.status.success()
            && stderr.lines().any(|candidate| {
                candidate.contains(&format!("lib.rs:{line}:"))
                    && needles.iter().all(|needle| candidate.contains(needle))
            }),
        "no-hand-model did not refuse lib.rs:{line} naming {needles:?}:\nstatus: {}\nstdout:\n{}\nstderr:\n{stderr}",
        out.status,
        text(&out.stdout)
    );
}

#[test]
fn no_hand_model_refuses_a_model_type_in_a_macro_rules_body() {
    for keyword in ["struct", "enum", "type", "union", "trait"] {
        let out = no_hand_model_on(
            &format!("no_hand_model_macro_rules_{keyword}"),
            &format!(
                "macro_rules! stamp {{\n    () => {{\n        pub {keyword} r#Commission\n    }};\n}}\n"
            ),
        );
        assert_refused_at(&out, 3, &[keyword, "Commission"]);
    }
}

#[test]
fn no_hand_model_refuses_a_model_type_in_an_item_macro_invocation() {
    let out = no_hand_model_on(
        "no_hand_model_item_macro",
        "some_crate::declare! {\n    pub struct Hand;\n    pub enum CaseId { A }\n}\n",
    );
    assert_refused_at(&out, 3, &["enum", "CaseId"]);
}

#[test]
fn no_hand_model_refuses_a_model_type_in_a_statement_macro() {
    let out = no_hand_model_on(
        "no_hand_model_statement_macro",
        "fn f() {\n    stamp!(struct AgentId;);\n}\n",
    );
    assert_refused_at(&out, 2, &["struct", "AgentId"]);
}

#[test]
fn no_hand_model_refuses_include() {
    for (body, line) in [
        ("include!(\"hand.rs\");\n", 1),
        (
            "pub mod m {\n    std::include!(concat!(env!(\"OUT_DIR\"), \"/x.rs\"));\n}\n",
            2,
        ),
        ("fn f() {\n    core::include! { \"hand.rs\" }\n}\n", 2),
    ] {
        let out = no_hand_model_on("no_hand_model_include", body);
        assert_refused_at(&out, line, &["include!"]);
    }
}

#[test]
fn no_hand_model_refuses_a_path_attribute_on_a_module() {
    for (body, line) in [
        ("#[path = \"../elsewhere/hand.rs\"]\npub mod hand;\n", 1),
        (
            "mod outer {\n    #[path = \"x.rs\"]\n    mod inner {}\n}\n",
            2,
        ),
        ("#[cfg_attr(test, path = \"hand.rs\")]\nmod hand;\n", 1),
    ] {
        let out = no_hand_model_on("no_hand_model_path_attribute", body);
        assert_refused_at(&out, line, &["path"]);
    }
}

#[test]
fn no_hand_model_passes_macros_that_declare_no_model_type() {
    let out = no_hand_model_on(
        "no_hand_model_harmless_macros",
        "macro_rules! stamp {\n    () => { pub struct Handmade; };\n}\nstamp!();\n\
         fn f() -> String {\n    format!(\"struct Commission {}\", 1)\n}\n\
         #[cfg_attr(test, derive(Debug))]\nmod inline {}\n",
    );
    assert!(
        out.status.success(),
        "no-hand-model refused macros that declare no model type:\n{}",
        text(&out.stderr)
    );
}
