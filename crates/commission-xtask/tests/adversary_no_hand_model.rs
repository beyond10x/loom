//! Adversary cases for `no-hand-model`: hand-written model types the check must see, the one it
//! still does not, and a non-definition it must not flag.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

fn case_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear case dir");
    }
    fs::create_dir_all(&dir).expect("create case dir");
    dir
}

/// Writes `files` (relative path, body) under a fresh case dir and returns `<case>/src`.
fn sources(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let case = case_dir(name);
    for (relative, body) in files {
        let path = case.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, body).expect("write source");
    }
    case.join("src")
}

fn no_hand_model(src: &Path) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_commission-xtask"))
        .args(["no-hand-model", "--src", src.to_str().expect("utf-8 src")])
        .output()
        .expect("run commission-xtask")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A hand-written type exported under a model name is a hand-written model type: every caller of
/// `b10x_commission::AgentId` gets the hand-written one.
#[test]
fn adversary_flags_a_hand_written_type_reexported_under_a_model_name() {
    let src = sources(
        "reexported_under_a_model_name",
        &[(
            "src/lib.rs",
            "pub struct Handmade(pub String);\npub use self::Handmade as AgentId;\n",
        )],
    );
    let out = no_hand_model(&src);
    assert!(
        !out.status.success(),
        "no-hand-model passed a hand-written type re-exported as AgentId:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// An id-newtype macro is the ordinary way to write several id types by hand.
///
/// Known limit of story:generated-responsibility-model: `no-hand-model` reads source as written
/// and does not expand macros, so today it passes this case. The assertion pins that behaviour; it
/// must flip to a refusal when the check learns macro expansion.
#[test]
fn adversary_known_limit_model_type_defined_through_a_macro_passes() {
    let src = sources(
        "defined_through_a_macro",
        &[(
            "src/lib.rs",
            "macro_rules! id {\n    ($name:ident) => {\n        pub struct $name(pub String);\n    };\n}\n\nid!(AgentId);\nid!(CommissionId);\n",
        )],
    );
    let out = no_hand_model(&src);
    assert!(
        out.status.success(),
        "known limit of story:generated-responsibility-model changed: no-hand-model now refuses \
         AgentId and CommissionId defined through a macro. Flip this case to assert the refusal \
         now that the check expands macros:\n{}",
        stderr(&out)
    );
}

/// `r#AgentId` is the identifier `AgentId`.
#[test]
fn adversary_flags_a_raw_identifier_model_type() {
    let src = sources(
        "raw_identifier",
        &[("src/lib.rs", "pub struct r#AgentId(pub String);\n")],
    );
    let out = no_hand_model(&src);
    assert!(
        !out.status.success(),
        "no-hand-model passed `pub struct r#AgentId`:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// The story's Outcome: Agent, AgentRevision, the Case entity and Commission come from the
/// generated model; acceptance 4 adds PrincipalId and AuthorityContext. Each, hand-written in
/// `crates/commission/src`, is a hand-written model type.
#[test]
fn adversary_flags_every_entity_the_story_says_is_generated() {
    let mut passed = Vec::new();
    for name in [
        "Agent",
        "AgentRevision",
        "Case",
        "PrincipalId",
        "AuthorityContext",
        "CommissionData",
    ] {
        let body = format!("pub struct {name} {{\n    pub id: String,\n}}\n");
        let src = sources(&format!("entity_{name}"), &[("src/lib.rs", &body)]);
        if no_hand_model(&src).status.success() {
            passed.push(name);
        }
    }
    assert!(
        passed.is_empty(),
        "no-hand-model passed hand-written {passed:?}"
    );
}

/// A `#[path]` module compiles into `b10x-commission` from outside `src/`.
///
/// `no-hand-model` does not follow the attribute; since `story:port-skeleton` it refuses the
/// `#[path]` module itself, naming the file and line (this case was a pinned known limit before).
#[test]
fn adversary_path_module_outside_src_is_refused() {
    let src = sources(
        "path_module_outside_src",
        &[
            (
                "src/lib.rs",
                "#[path = \"../model/hand.rs\"]\npub mod hand;\n",
            ),
            ("model/hand.rs", "pub struct AgentId(pub String);\n"),
        ],
    );
    let out = no_hand_model(&src);
    let stderr = stderr(&out);
    assert!(
        !out.status.success()
            && stderr
                .lines()
                .any(|line| line.contains("lib.rs:1:") && line.contains("#[path")),
        "no-hand-model passed a #[path] module compiled in from outside src/:\n{stderr}"
    );
}

/// A string literal is not a definition.
#[test]
fn adversary_does_not_flag_a_string_literal() {
    let src = sources(
        "string_literal",
        &[(
            "src/lib.rs",
            "pub const HINT: &str = \"struct Commission comes from the generated model\";\n",
        )],
    );
    let out = no_hand_model(&src);
    assert!(
        out.status.success(),
        "no-hand-model flagged a string literal:\n{}",
        stderr(&out)
    );
}
