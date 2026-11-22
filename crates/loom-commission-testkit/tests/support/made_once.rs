//! Products several test processes of this crate read, made once a test run.
//!
//! nextest starts each test in its own process, so a `OnceLock` makes a product once for every
//! test. These are kept on disk under `CARGO_TARGET_TMPDIR` instead, keyed by the digest of
//! everything they are made from ([`ess_suite::made_once`]): the first test to ask makes the
//! product, one that asks while it is being made waits, and the rest read it. A changed input
//! makes a new key.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../../../loom-executor/tests/support/ess_suite.rs"]
mod ess_suite;

/// What `ess verify conform synthesize --path <root>/ess/commission --out <suite>` returned.
pub struct Synthesized {
    /// Exit code, `None` when a signal ended the run.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// The suite file it wrote; read it, never write it, since other tests share it.
    pub suite: PathBuf,
}

/// The synthesis of `<root>/ess/commission` as the tree holds it now
/// ([`ess_suite::plain_synthesis_once`]): made once for every process of the test run, keyed by
/// the digest of every file under `ess/commission/` and the version line of the `ess` that runs.
/// `task test-step` makes it beside its build, so the tests find it made.
pub fn commission_synthesis(root: &Path) -> Synthesized {
    let dir = ess_suite::plain_synthesis_once(
        &ess_suite::ess_program(root),
        root,
        &root.join(ess_suite::PLAIN_SYNTHESIS_SPECIFICATION),
        &Path::new(env!("CARGO_TARGET_TMPDIR")).join(ess_suite::PLAIN_SYNTHESIS_CACHE),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let read = |name: &str| {
        let path = dir.join(name);
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    };
    Synthesized {
        code: read("code").trim().parse().ok(),
        stdout: read("stdout"),
        stderr: read("stderr"),
        suite: dir.join("suite.json"),
    }
}

/// `cargo <args>` in `root`, with colour off; its standard output. It is made once for every
/// process of the test run, keyed by the arguments, the version line of `cargo`, and the bytes of
/// `Cargo.lock` and of every manifest the resolution reads: the workspace's, each member's under
/// `crates/` and each generated crate's under `generated/rust/`. Only for a command whose output
/// follows from those alone, such as `cargo tree --locked`.
pub fn cargo_output(root: &Path, args: &[&str]) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let command = || {
        let mut command = Command::new(&cargo);
        command.env("CARGO_TERM_COLOR", "never").current_dir(root);
        command
    };
    let version = command()
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("run `cargo --version`: {e}"));
    assert!(version.status.success(), "`cargo --version` failed");
    let mut stream = format!(
        "loom-cargo-output/1\n{}\n{}\n",
        String::from_utf8_lossy(&version.stdout).trim(),
        args.join("\u{0}")
    )
    .into_bytes();
    let mut manifests = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    for dir in ["crates", "generated/rust"] {
        let Ok(entries) = fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("Cargo.toml"))
            .filter(|path| path.is_file())
            .collect();
        found.sort();
        manifests.extend(found);
    }
    for path in manifests {
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        stream.extend_from_slice(format!("{relative}\n{}\n", bytes.len()).as_bytes());
        stream.extend_from_slice(&bytes);
        stream.push(b'\n');
    }
    let key = format!(
        "cargo-{}",
        ess_suite::digest(&stream).trim_start_matches("sha256:")
    );
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cargo-output");
    let dir = ess_suite::made_once(&cache, &key, |dir| {
        let output = command()
            .args(args)
            .output()
            .map_err(|e| format!("run `cargo {}`: {e}", args.join(" ")))?;
        if !output.status.success() {
            return Err(format!(
                "`cargo {}` failed:\n{}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        fs::write(dir.join("stdout"), &output.stdout)
            .map_err(|e| format!("write under {}: {e}", dir.display()))
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let path = dir.join("stdout");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}
