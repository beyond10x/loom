//! Adversary pass 1 on `story:harness-crate-port`: what the port says about itself in comments,
//! and what Harness's crate boundaries enforced that one Loom crate no longer does.
//!
//! The ported source is read at run time through `CARGO_MANIFEST_DIR`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PORTED_MODULES: [&str; 5] = ["wire", "http", "responses", "messages", "turn_loop"];

fn crate_dir() -> PathBuf {
    PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo test"))
}

fn workspace_dir() -> PathBuf {
    crate_dir().join("../..")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap_or_else(|e| panic!("{}: {e}", next.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every ported source file, with its path relative to `crates/loom/src/harness`.
fn ported_sources() -> Vec<(String, String)> {
    let root = crate_dir().join("src/harness");
    PORTED_MODULES
        .iter()
        .flat_map(|module| rust_files(&root.join(module)))
        .map(|path| {
            let rel = path
                .strip_prefix(&root)
                .expect("below src/harness")
                .display()
                .to_string();
            let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
            (rel, text)
        })
        .collect()
}

/// Every backticked span on a line.
fn backticked(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// A backticked span that is a repository path to a Rust test file or a Markdown document. A
/// pattern (`agents/*.md`, `agents/<name>.md`) is a path in a run's workspace, not in this one.
fn is_repo_path(span: &str) -> bool {
    !span.contains(|c: char| c.is_whitespace() || matches!(c, '*' | '<' | '{'))
        && (span.contains('/') || span == "ROADMAP.md")
        && (span.ends_with(".rs") || span.ends_with(".md"))
}

fn resolves(span: &str) -> bool {
    workspace_dir().join(span).exists() || crate_dir().join(span).exists()
}

/// `messages/mod.rs:113` and `responses/mod.rs:86` say the transport the two wires share is held by
/// a named test file that "fails if the two wires ever stop agreeing". That file is Harness's
/// `crates/harness-messages/tests/transport.rs`, which the port did not carry: in Loom the comment
/// names a guard that does not exist, so a reader trusts a check nothing runs.
#[test]
fn every_test_file_a_ported_comment_names_as_a_guard_exists_in_loom() {
    let mut dangling = Vec::new();
    for (rel, text) in ported_sources() {
        for (n, line) in text.lines().enumerate() {
            for span in backticked(line) {
                if is_repo_path(span)
                    && span.ends_with(".rs")
                    && span.contains("tests/")
                    && !resolves(span)
                {
                    dangling.push(format!("{rel}:{}: `{span}`", n + 1));
                }
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "ported comments name test files Loom does not have:\n{}",
        dangling.join("\n")
    );
}

/// The ported comments cite Harness's design record and its numbered `AGENTS.md` invariants as the
/// reason for a rule. Loom has neither `docs/design/0002-…`, nor `ROADMAP.md`, nor a numbered
/// invariant in its `AGENTS.md`, so in this repository each citation points at nothing, or at a
/// different file of the same name.
#[test]
fn every_document_a_ported_comment_cites_resolves_in_loom() {
    let agents = fs::read_to_string(workspace_dir().join("AGENTS.md")).expect("Loom AGENTS.md");
    let mut dangling = Vec::new();
    for (rel, text) in ported_sources() {
        for (n, line) in text.lines().enumerate() {
            for span in backticked(line) {
                if is_repo_path(span) && span.ends_with(".md") && !resolves(span) {
                    dangling.push(format!("{rel}:{}: `{span}`", n + 1));
                }
            }
            // `AGENTS.md invariant 5`, `` `AGENTS.md` invariant 7 ``.
            let flat = line.replace('`', "");
            if let Some(at) = flat.find("AGENTS.md invariant ") {
                let number: String = flat[at + "AGENTS.md invariant ".len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if !number.is_empty() && !agents.contains(&format!("invariant {number}")) {
                    dangling.push(format!("{rel}:{}: AGENTS.md invariant {number}", n + 1));
                }
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "ported comments cite {} documents or invariants Loom does not have:\n{}",
        dangling.len(),
        dangling.join("\n")
    );
}

/// What each Harness crate could not name, because its manifest did not depend on it. In Harness a
/// violation is a compile error; in one Loom crate every module sees every other, so only a check
/// like this one holds the line. `harness-messages/Cargo.toml` at `798325f0`: "Nothing in `src/`
/// may import [`harness-responses`] — the two projections stay independent"; the loop depends only
/// on the wire crate; the wire crate performs no I/O; the two projections "speak no HTTP of
/// [their] own".
const FORBIDDEN: [(&str, &[&str]); 5] = [
    (
        "wire",
        &[
            "crate::harness::http",
            "crate::harness::responses",
            "crate::harness::messages",
            "crate::harness::turn_loop",
            "reqwest",
            "tokio",
            "jsonschema",
            "sha2",
        ],
    ),
    (
        "http",
        &[
            "crate::harness::responses",
            "crate::harness::messages",
            "crate::harness::turn_loop",
        ],
    ),
    (
        "responses",
        &[
            "crate::harness::messages",
            "crate::harness::turn_loop",
            "reqwest",
            "tokio",
        ],
    ),
    (
        "messages",
        &[
            "crate::harness::responses",
            "crate::harness::turn_loop",
            "reqwest",
            "tokio",
        ],
    ),
    (
        "turn_loop",
        &[
            "crate::harness::http",
            "crate::harness::responses",
            "crate::harness::messages",
            "reqwest",
            "tokio",
        ],
    ),
];

/// Code lines (not comments) of `text` that name a forbidden path, as `line: token`.
fn violations(text: &str, forbidden: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let code = code.split(" //").next().unwrap_or(code);
        for token in forbidden {
            let hit = code.match_indices(token).any(|(at, _)| {
                let before = code[..at].chars().next_back();
                let after = code[at + token.len()..].chars().next();
                let ident = |c: char| c.is_alphanumeric() || c == '_';
                !before.is_some_and(ident) && !after.is_some_and(ident)
            });
            if hit {
                found.push(format!("{}: {token}", n + 1));
            }
        }
    }
    found
}

#[test]
fn the_boundaries_harness_enforced_by_crate_still_hold_between_modules() {
    // The scan can fail: a messages module that reaches for the other projection, and a loop
    // that reaches for a client, are both found.
    let planted = "use crate::harness::responses::WIRE;\nlet c = reqwest::Client::new();\n";
    assert_eq!(
        violations(planted, &["crate::harness::responses", "reqwest"]),
        vec![
            "1: crate::harness::responses".to_owned(),
            "2: reqwest".to_owned()
        ]
    );
    assert!(violations("// reqwest in a comment\n", &["reqwest"]).is_empty());

    let root = crate_dir().join("src/harness");
    let mut crossings = Vec::new();
    for (module, forbidden) in FORBIDDEN {
        for path in rust_files(&root.join(module)) {
            let text = fs::read_to_string(&path).expect("readable source");
            for v in violations(&text, forbidden) {
                crossings.push(format!(
                    "{}:{v}",
                    path.strip_prefix(&root).expect("below").display()
                ));
            }
        }
    }
    assert!(
        crossings.is_empty(),
        "a ported module names what its Harness crate could not depend on:\n{}",
        crossings.join("\n")
    );
}
