//! Adversary cases for `story:stability-version-rule`.
//!
//! `website/docs/reference/sdk-stability.md` states a deprecation window: a path the page lists is
//! deprecated for at least one release (a bullet under `### Deprecated`) before it is removed, and
//! `loom-xtask release-check` holds it. These cases drive `release-check` with entries worded as
//! `CHANGELOG.md` already words removals and deprecations (0.31.0 is the source of most of them),
//! and hold that a removal is refused whatever verb or ordering names it, and that what the entry
//! does not remove, or the page puts outside the promise, is not refused.
//!
//! Each fixture is a git repository under `CARGO_TARGET_TMPDIR`, tagged `0.1.0` with its own SDK
//! stability page, then a second commit at `0.2.0`; `git` runs with a local identity and no global
//! or system configuration, and nothing reaches the network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

/// The first release's entry.
const FIRST: &str = "### Added\n\n- **Breaking:** everything is new.";

/// The page path `release-check` reads at the previous tag.
const PAGE: &str = "website/docs/reference/sdk-stability.md";

/// A stability page listing `loom_sdk::Port`, the module path `loom_sdk::ports::Gate`, the llm
/// `Model` a host hands Loom, a generated model path the page puts outside the promise, and a
/// trust-store variable that is no path.
const STABILITY: &str = "# SDK stability\n\n## The promise\n\n\
    - Outside the promise: generated model types (`loom_sdk::loom::model`).\n\
    - Deprecation window: a path this page lists is deprecated for at least one release before \
    it is removed.\n\n\
    | Port | Path |\n|---|---|\n\
    | `Port` | `loom_sdk::Port` |\n\
    | `Gate` | `loom_sdk::ports::Gate` |\n\n\
    | Item | What it is |\n|---|---|\n\
    | `loom_sdk::models::from_config` | The llm `Model` of each role |\n\
    | `loom_sdk::ChildCommand` | A child; the environment may not name `CURL_HOME` |\n\n\
    ## Used by control-plane\n\n\
    Rows marked *generated* are generated model types or modules, outside the promise above.\n\n\
    | Path | Kind |\n|---|---|\n\
    | `loom_sdk::loom::model::run::SessionId` | generated |\n";

fn fixture_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary-u4-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| panic!("create {}: {e}", parent.display()));
    }
    fs::write(path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Loom Release Fixture",
            "-c",
            "user.email=release-fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn manifest(version: &str) -> String {
    format!(
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"{version}\"\n\
         edition = \"2024\"\n"
    )
}

fn changelog(entries: &[(&str, &str)]) -> String {
    let mut text = String::from("# Changelog\n\n## [Unreleased]\n");
    for (version, body) in entries.iter().rev() {
        text.push_str(&format!("\n## [{version}] - 2026-10-23\n\n{body}\n"));
    }
    text
}

/// `0.1.0` tagged with `first` and [`STABILITY`], then `0.2.0` committed and tagged with `second`.
fn pair(name: &str, first: &str, second: &str) -> PathBuf {
    let root = fixture_dir(name);
    write(&root.join("Cargo.toml"), &manifest("0.1.0"));
    write(&root.join("CHANGELOG.md"), &changelog(&[("0.1.0", first)]));
    write(&root.join(PAGE), STABILITY);
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "first"]);
    git(&root, &["tag", "-a", "0.1.0", "-m", "Loom 0.1.0"]);
    write(&root.join("Cargo.toml"), &manifest("0.2.0"));
    write(
        &root.join("CHANGELOG.md"),
        &changelog(&[("0.1.0", first), ("0.2.0", second)]),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "second"]);
    git(&root, &["tag", "-a", "0.2.0", "-m", "Loom 0.2.0"]);
    root
}

fn release_check(root: &Path, tag: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .args(["release-check", "--tag", tag])
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Runs `release-check --tag 0.2.0` on a fresh pair and says whether it refused removing `item`.
fn refuses_removing(name: &str, first: &str, second: &str, item: &str) -> (bool, String) {
    let root = pair(name, first, second);
    let out = release_check(&root, "0.2.0");
    let err = stderr(&out);
    let _ = fs::remove_dir_all(&root);
    (
        !out.status.success() && err.contains(&format!("removes `{item}`")),
        err,
    )
}

/// Runs `release-check --tag 0.2.0` on a fresh pair and returns whether it passed, and stderr.
fn passes(name: &str, first: &str, second: &str) -> (bool, String) {
    let root = pair(name, first, second);
    let out = release_check(&root, "0.2.0");
    let err = stderr(&out);
    let _ = fs::remove_dir_all(&root);
    (out.status.success(), err)
}

/// A bullet that leads with the verb, as Keep a Changelog writes it (`Removed …`), removes the
/// items after it.
#[test]
fn adversary_a_removal_led_by_its_verb_is_refused() {
    let (refused, err) = refuses_removing(
        "verb-first",
        FIRST,
        "### Removed\n\n- **Breaking:** Removed `loom_sdk::Port` (story:drop-port)",
        "loom_sdk::Port",
    );
    assert!(
        refused,
        "release-check passed `Removed `loom_sdk::Port`` under `### Removed` with no deprecated \
         release: {err}"
    );
}

/// 0.31.0's own wording: `` `a` is removed, and so are `b` and `c` `` removes all three.
#[test]
fn adversary_every_item_a_removal_names_after_the_verb_is_refused() {
    let (refused, err) = refuses_removing(
        "and-so-are",
        FIRST,
        "### Removed\n\n- **Breaking:** `loom_sdk::Gone` is removed, and so is `loom_sdk::Port`",
        "loom_sdk::Port",
    );
    assert!(
        refused,
        "release-check passed removing `loom_sdk::Port` named after the verb (`… is removed, and \
         so is `loom_sdk::Port``): {err}"
    );
}

/// Removing a module removes every listed path under it (0.31.0: `the module
/// `loom_sdk::loom::harness::wire` … are removed`).
#[test]
fn adversary_removing_the_module_of_a_listed_path_is_refused() {
    let (refused, err) = refuses_removing(
        "module",
        FIRST,
        "### Removed\n\n- **Breaking:** the module `loom_sdk::ports` is removed",
        "loom_sdk::ports",
    );
    let names_gate = err.contains("loom_sdk::ports::Gate");
    assert!(
        refused || names_gate,
        "release-check passed removing the module `loom_sdk::ports`, which holds the listed \
         `loom_sdk::ports::Gate`, with no deprecated release: {err}"
    );
}

/// A listed path that is renamed, moved or deleted is gone from the old path just as a removed
/// one is (0.31.0: `MeteredModelPort` is renamed; `Loom's port of Harness is deleted`).
#[test]
fn adversary_a_listed_path_gone_under_another_verb_is_refused() {
    let mut passed = Vec::new();
    for (n, bullet) in [
        "- **Breaking:** `loom_sdk::Port` is renamed `loom_sdk::Gate2`",
        "- **Breaking:** `loom_sdk::Port` moves to `loom_sdk::ports::Port`",
        "- **Breaking:** `loom_sdk::Port` is deleted (story:drop-port)",
    ]
    .iter()
    .enumerate()
    {
        let (refused, _) = refuses_removing(
            &format!("verb-{n}"),
            FIRST,
            &format!("### Changed\n\n{bullet}"),
            "loom_sdk::Port",
        );
        if !refused {
            passed.push(*bullet);
        }
    }
    assert!(
        passed.is_empty(),
        "release-check passed these entries taking the listed `loom_sdk::Port` away with no \
         deprecated release: {passed:#?}"
    );
}

/// A deprecation led by its verb (`Deprecated `x`; use `y``) deprecates `x`, so a later release may
/// remove it.
#[test]
fn adversary_a_deprecation_led_by_its_verb_opens_the_window() {
    let first = format!(
        "{FIRST}\n\n### Deprecated\n\n- Deprecated `loom_sdk::Port`; use `loom_sdk::ports::Gate`"
    );
    let (ok, err) = passes(
        "deprecated-first",
        &first,
        "### Removed\n\n- **Breaking:** `loom_sdk::Port` is removed",
    );
    assert!(
        ok,
        "release-check refused removing `loom_sdk::Port`, which 0.1.0 listed as `Deprecated \
         `loom_sdk::Port``: {err}"
    );
}

/// A deprecated module deprecates every listed path under it.
#[test]
fn adversary_a_deprecated_module_opens_the_window_for_its_paths() {
    let first =
        format!("{FIRST}\n\n### Deprecated\n\n- the module `loom_sdk::ports` is deprecated");
    let (ok, err) = passes(
        "deprecated-module",
        &first,
        "### Removed\n\n- **Breaking:** `loom_sdk::ports::Gate` is removed",
    );
    assert!(
        ok,
        "release-check refused removing `loom_sdk::ports::Gate`, whose module 0.1.0 deprecated: \
         {err}"
    );
}

/// 0.31.0's rename bullet under `### Removed` names the llm `Model`, which it does not remove;
/// `release-check` must not read it as a removal of `Model`.
#[test]
fn adversary_a_span_the_removal_only_mentions_is_not_removed() {
    let (ok, err) = passes(
        "mention",
        FIRST,
        "### Removed\n\n- **Breaking:** `MeteredModelPort` is renamed `MeteredModel`, an llm \
         `Model` only; its harness `ModelPort` impl is removed",
    );
    assert!(
        ok && !err.contains("removes `Model`"),
        "release-check read 0.31.0's rename bullet as removing the llm `Model` it only mentions: \
         {err}"
    );
}

/// The page puts generated model types outside the promise, the control-plane rows marked
/// *generated* included, and the window covers paths: a generated path, or a span that is no
/// path, is outside it.
#[test]
fn adversary_what_the_page_puts_outside_the_promise_is_outside_the_window() {
    let mut refused = Vec::new();
    for (n, bullet) in [
        "- **Breaking:** `loom_sdk::loom::model::run::SessionId` is removed",
        "- **Breaking:** `CURL_HOME` is removed from the trust-store variables",
    ]
    .iter()
    .enumerate()
    {
        let (ok, err) = passes(
            &format!("outside-{n}"),
            FIRST,
            &format!("### Removed\n\n{bullet}"),
        );
        if !ok {
            refused.push(format!("{bullet}: {err}"));
        }
    }
    assert!(
        refused.is_empty(),
        "release-check refused removing what the stability page puts outside the promise or is no \
         path: {refused:#?}"
    );
}

/// The upcoming minor release: a `**Breaking:**` bullet for the Substrate move and a
/// `### Deprecated` bullet pass on a tag that moves the minor version.
#[test]
fn adversary_the_upcoming_minor_release_passes() {
    let (ok, err) = passes(
        "upcoming-minor",
        FIRST,
        "### Deprecated\n\n- `loom_sdk::Port` is deprecated; use `loom_sdk::ports::Gate`\n\n\
         ### Changed\n\n- **Breaking:** Loom links Substrate 0.10.0 \
         (`rev = \"105cf718bc6a5314486c78f5fd93fabed290e30e\"`); `TestExecution::driver_record` \
         gains `workdir`, `writable_roots` and `AppliedNetwork::Apertures` (story:substrate-0-8-pin)",
    );
    assert!(
        ok,
        "release-check refused the upcoming minor release: {err}"
    );
}
