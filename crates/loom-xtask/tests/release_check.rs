//! `story:release-process`: `loom-xtask release-check --tag <tag>` and the release workflow that
//! calls it.
//!
//! * `release-check` passes when the tag equals `[workspace.package] version` in `Cargo.toml` and
//!   `CHANGELOG.md` has a `## [<tag>]` heading, and fails, naming what differs, otherwise.
//! * The repository itself passes `release-check` at its own workspace version, and its README and
//!   site pin that version as a tag wherever they show a Loom dependency line.
//! * `.github/workflows/release.yml` runs on a bare-version tag with read-only permissions, calls
//!   only `release-check` in its own steps, and reuses `check.yml` on the tagged commit.
//!
//! `story:release-names-embedder-breaks`, as `story:changelog-keep-a-changelog` moved it to Keep a
//! Changelog: the tag's entry names each break in a `- **Breaking:** …` bullet or carries the line
//! `No breaking changes.`, never both, and no `**Breaking:**` bullet names nothing or says `none`;
//! `No breaking changes.` is refused when a stable surface (`consumers/pinned/`,
//! `crates/loom-sdk/tests/host_ports.rs`, `docs/contracts/run-events-v1.json`) changed since the
//! previous tag. A `Breaking:` sentence in prose no longer counts. The fixture repositories are
//! built with `git` in a temporary directory, with a local identity on each command and no global
//! or system configuration; nothing is tagged or committed in the working tree, and nothing
//! reaches the network.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time; fixtures land under
//! `CARGO_TARGET_TMPDIR`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-10-05\n\n### Added\n\n- **Breaking:** everything is new.\n";
/// The first release's entry in the fixture repositories.
const FIRST: &str = "### Added\n\n- **Breaking:** everything is new.";
/// An entry that names no break.
const NO_BREAKS: &str = "No breaking changes.\n\n### Changed\n\n- `Port::call` is faster.";
const RELEASE_TAGS: &str = "[0-9]+.[0-9]+.[0-9]+";

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// A fresh directory under `CARGO_TARGET_TMPDIR` holding a `Cargo.toml` at `version` and the given
/// `CHANGELOG.md`.
fn fixture(name: &str, version: &str, changelog: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("release-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let manifest = format!(
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"{version}\"\n\
         edition = \"2024\"\n"
    );
    write(&dir.join("Cargo.toml"), &manifest);
    write(&dir.join("CHANGELOG.md"), changelog);
    dir
}

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn release_check(root: &Path, tag: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .args(["release-check", "--tag", tag])
        .output()
        .unwrap_or_else(|e| panic!("run loom-xtask: {e}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The `[workspace.package] version` of the repository's own `Cargo.toml`.
fn workspace_version() -> String {
    let manifest = read(&repository().join("Cargo.toml"));
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
        } else if in_package && let Some(value) = line.strip_prefix("version") {
            let value = value.trim_start().trim_start_matches('=').trim();
            return value.trim_matches('"').to_owned();
        }
    }
    panic!("Cargo.toml has no [workspace.package] version");
}

/// The three stable surfaces `release-check` diffs between tags.
const SURFACES: [&str; 3] = [
    "consumers/pinned/src/main.rs",
    "crates/loom-sdk/tests/host_ports.rs",
    "docs/contracts/run-events-v1.json",
];

/// Runs `git` in `dir` with a local identity and no global or system configuration.
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
        stderr(&out)
    );
}

fn changelog_with(entries: &[(&str, &str)]) -> String {
    let mut text = String::from("# Changelog\n\n## [Unreleased]\n");
    for (version, body) in entries.iter().rev() {
        text.push_str(&format!("\n## [{version}] - 2026-10-05\n\n{body}\n"));
    }
    text
}

/// A git repository holding the three stable surfaces and a README, committed and tagged `0.1.0`
/// with a `**Breaking:**` bullet in its entry. `change` names the files to edit before the second
/// commit, which sets the workspace version to `0.2.0` with `second` as its entry; `tag_second`
/// tags that commit `0.2.0`.
fn two_tag_fixture(name: &str, change: &[&str], second: &str, tag_second: bool) -> PathBuf {
    let first = FIRST;
    let root = fixture(name, "0.1.0", &changelog_with(&[("0.1.0", first)]));
    for file in SURFACES.iter().chain(&["README.md"]) {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap_or(&root))
            .unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
        write(&path, &format!("{file}: first\n"));
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "first"]);
    git(&root, &["tag", "-a", "0.1.0", "-m", "Loom 0.1.0"]);

    for file in change {
        write(&root.join(file), &format!("{file}: second\n"));
    }
    write(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"0.2.0\"\n\
         edition = \"2024\"\n",
    );
    write(
        &root.join("CHANGELOG.md"),
        &changelog_with(&[("0.1.0", first), ("0.2.0", second)]),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "second"]);
    if tag_second {
        git(&root, &["tag", "-a", "0.2.0", "-m", "Loom 0.2.0"]);
    }
    root
}

/// The message `release-check` names an entry with neither form by.
const NEITHER: &str = "has no `**Breaking:**` bullet and no `No breaking changes.` line";

#[test]
fn release_check_names_an_entry_without_a_breaking_bullet() {
    // A `Breaking:` sentence in prose is the old form and no longer counts; a marker quoted in
    // backticks, "Breaking" without its colon and the words inside a sentence are no statement.
    let second = "Second. Breaking: `Port::call` takes a context.\n\n### Changed\n\n- A break is \
                  named in a `**Breaking:**` bullet.\n- Breaking for nobody, a habit.\n- The \
                  release has No breaking changes. Honest.";
    let changelog = changelog_with(&[("0.1.0", FIRST), ("0.2.0", second)]);
    let root = fixture("no-breaking", "0.2.0", &changelog);
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success(),
        "release-check passed an entry with neither a `**Breaking:**` bullet nor \
         `No breaking changes.`"
    );
    assert!(
        stderr(&out).contains(&format!("CHANGELOG.md entry [0.2.0] {NEITHER}")),
        "release-check did not name the entry with neither form: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_refuses_no_breaking_changes_when_a_stable_surface_changed_since_the_previous_tag()
{
    for (n, surface) in SURFACES.iter().enumerate() {
        let root = two_tag_fixture(
            &format!("surface-{n}"),
            &[surface, "README.md"],
            NO_BREAKS,
            true,
        );
        let out = release_check(&root, "0.2.0");
        let err = stderr(&out);
        assert!(
            !out.status.success(),
            "release-check passed `No breaking changes.` although {surface} changed since 0.1.0"
        );
        assert!(
            err.contains("CHANGELOG.md entry [0.2.0] says `No breaking changes.`")
                && err.contains("since 0.1.0")
                && err.contains(surface),
            "release-check did not name the entry and the changed surface {surface}: {err}"
        );
        assert!(
            !err.contains("README.md"),
            "release-check named a file that is no stable surface: {err}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn release_check_refuses_no_breaking_changes_before_the_tag_exists() {
    // On the release commit, before the tag is pushed, the diff runs to `HEAD`.
    let root = two_tag_fixture(
        "untagged",
        &["docs/contracts/run-events-v1.json"],
        NO_BREAKS,
        false,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success()
            && stderr(&out).contains("docs/contracts/run-events-v1.json")
            && stderr(&out).contains("since 0.1.0"),
        "release-check did not diff the untagged release commit against 0.1.0: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_accepts_no_breaking_changes_when_no_stable_surface_changed() {
    let root = two_tag_fixture("unchanged", &["README.md"], NO_BREAKS, true);
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused `No breaking changes.` although no stable surface changed: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_accepts_a_breaking_bullet_when_a_stable_surface_changed() {
    let root = two_tag_fixture(
        "named",
        &SURFACES,
        "### Changed\n\n- **Breaking:** `Port::call` takes a context (story:port-context).\n\
         - `Port::name` is new.",
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused an entry that names its break: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_refuses_a_breaking_bullet_that_names_no_break() {
    // `- **Breaking:** none.` would slip past the stable-surface check `No breaking changes.` gets;
    // an empty bullet names nothing. No stable surface changes here, so only the bullet can be
    // refused.
    for (n, bullet) in [
        "- **Breaking:** none.",
        "- **Breaking:** None",
        "- **Breaking:**",
    ]
    .iter()
    .enumerate()
    {
        let second = format!("### Changed\n\n{bullet}\n- `Port::name` is new.");
        let root = two_tag_fixture(&format!("empty-{n}"), &["README.md"], &second, true);
        let out = release_check(&root, "0.2.0");
        assert!(
            !out.status.success()
                && stderr(&out).contains(
                    "CHANGELOG.md entry [0.2.0] has a `**Breaking:**` bullet that names no break"
                )
                && stderr(&out).contains(bullet),
            "release-check did not refuse the bullet {bullet:?}: {}",
            stderr(&out)
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn release_check_refuses_an_entry_with_both_a_breaking_bullet_and_no_breaking_changes() {
    let second = "No breaking changes.\n\n### Removed\n\n- **Breaking:** `Port::call` is removed.";
    let root = two_tag_fixture("both", &["README.md"], second, true);
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success()
            && stderr(&out).contains(
                "CHANGELOG.md entry [0.2.0] says `No breaking changes.` beside a \
                 `**Breaking:**` bullet"
            ),
        "release-check passed an entry that both breaks and says it does not: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_passes_on_the_first_tag() {
    let root = two_tag_fixture("first", &[], NO_BREAKS, false);
    // Check out the first tag: it has no previous tag, so only the statement is required.
    git(&root, &["checkout", "-q", "0.1.0"]);
    write(
        &root.join("CHANGELOG.md"),
        &changelog_with(&[("0.1.0", NO_BREAKS)]),
    );
    let out = release_check(&root, "0.1.0");
    assert!(
        out.status.success(),
        "release-check refused `No breaking changes.` on the first tag: {}",
        stderr(&out)
    );

    write(
        &root.join("CHANGELOG.md"),
        &changelog_with(&[("0.1.0", "### Added\n\n- `Port` is new.")]),
    );
    let out = release_check(&root, "0.1.0");
    assert!(
        !out.status.success()
            && stderr(&out).contains(&format!("CHANGELOG.md entry [0.1.0] {NEITHER}")),
        "release-check passed the first tag with neither a `**Breaking:**` bullet nor \
         `No breaking changes.`: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

/// The SDK stability page of the fixture repositories, listing `loom_sdk::Port` and
/// `loom_sdk::Other`.
const STABILITY_PAGE: &str = "# SDK stability\n\n| Port | Path |\n|---|---|\n\
                              | `Port` | `loom_sdk::Port` |\n| `Other` | `loom_sdk::Other` |\n";

/// A git repository tagged `0.1.0` with `first` as its entry and the SDK stability page at
/// `website/docs/reference/sdk-stability.md`, then a second commit at the workspace version
/// `second_version` with `second` as its entry, tagged when `tag_second`.
fn release_pair(
    name: &str,
    first: &str,
    second_version: &str,
    second: &str,
    tag_second: bool,
) -> PathBuf {
    let root = fixture(name, "0.1.0", &changelog_with(&[("0.1.0", first)]));
    let page = root.join("website/docs/reference/sdk-stability.md");
    fs::create_dir_all(page.parent().unwrap_or(&root))
        .unwrap_or_else(|e| panic!("create {}: {e}", page.display()));
    write(&page, STABILITY_PAGE);
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "first"]);
    git(&root, &["tag", "-a", "0.1.0", "-m", "Loom 0.1.0"]);

    write(
        &root.join("Cargo.toml"),
        &format!(
            "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \
             \"{second_version}\"\nedition = \"2024\"\n"
        ),
    );
    write(
        &root.join("CHANGELOG.md"),
        &changelog_with(&[("0.1.0", first), (second_version, second)]),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "second"]);
    if tag_second {
        git(
            &root,
            &[
                "tag",
                "-a",
                second_version,
                "-m",
                &format!("Loom {second_version}"),
            ],
        );
    }
    root
}

#[test]
fn a_removal_without_a_deprecated_release_is_refused() {
    // `loom_sdk::Port` is listed on 0.1.0's stability page and no earlier entry deprecated it, so
    // removing it in 0.2.0 is refused, under `### Removed` and as a `**Breaking:**` removal
    // elsewhere, tagged or not yet tagged.
    for (n, (second, tag_second)) in [
        (
            "### Removed\n\n- **Breaking:** `loom_sdk::Port` is removed (story:drop-port)",
            true,
        ),
        (
            "### Removed\n\n- **Breaking:** `loom_sdk::Port` is removed (story:drop-port)",
            false,
        ),
        (
            "### Changed\n\n- **Breaking:** `loom_sdk::Port` is removed; use `loom_sdk::Other`",
            true,
        ),
    ]
    .iter()
    .enumerate()
    {
        let root = release_pair(&format!("removal-{n}"), FIRST, "0.2.0", second, *tag_second);
        let out = release_check(&root, "0.2.0");
        let err = stderr(&out);
        assert!(
            !out.status.success()
                && err.contains("CHANGELOG.md entry [0.2.0] removes `loom_sdk::Port`")
                && err.contains("### Deprecated"),
            "release-check did not refuse removing `loom_sdk::Port` without a deprecated \
             release ({second:?}): {err}"
        );
        assert!(
            !err.contains("`loom_sdk::Other`"),
            "release-check named an item the entry does not remove: {err}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    // Deprecated in an earlier release, the removal passes.
    let first = "### Added\n\n- **Breaking:** everything is new.\n\n### Deprecated\n\n\
                 - `loom_sdk::Port` is deprecated; use `loom_sdk::Other`";
    let root = release_pair(
        "removal-deprecated",
        first,
        "0.2.0",
        "### Removed\n\n- **Breaking:** `loom_sdk::Port` is removed",
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused removing `loom_sdk::Port`, deprecated in 0.1.0: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);

    // Deprecated only in the entry that removes it is no release under `### Deprecated`.
    let root = release_pair(
        "removal-same-entry",
        FIRST,
        "0.2.0",
        "### Deprecated\n\n- `loom_sdk::Port` is deprecated\n\n### Removed\n\n\
         - **Breaking:** `loom_sdk::Port` is removed",
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success()
            && stderr(&out).contains("CHANGELOG.md entry [0.2.0] removes `loom_sdk::Port`"),
        "release-check accepted a deprecation in the entry that removes the item: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);

    // An item the previous release's stability page does not list is outside the window.
    let root = release_pair(
        "removal-unlisted",
        FIRST,
        "0.2.0",
        "### Removed\n\n- **Breaking:** `Loom::run_loop` is removed",
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused removing an item the stability page does not list: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_breaking_entry_on_a_patch_tag_is_refused() {
    let breaking =
        "### Changed\n\n- **Breaking:** `Port::call` takes a context (story:port-context)";
    for tag_second in [true, false] {
        let root = release_pair(
            &format!("patch-{tag_second}"),
            FIRST,
            "0.1.1",
            breaking,
            tag_second,
        );
        let out = release_check(&root, "0.1.1");
        let err = stderr(&out);
        assert!(
            !out.status.success()
                && err.contains("tag 0.1.1 moves only the patch version from 0.1.0")
                && err.contains("**Breaking:**"),
            "release-check passed a `**Breaking:**` entry on the patch tag 0.1.1: {err}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    // A patch tag that breaks nothing passes, and so does a break on a minor tag.
    let root = release_pair("patch-none", FIRST, "0.1.1", NO_BREAKS, true);
    let out = release_check(&root, "0.1.1");
    assert!(
        out.status.success(),
        "release-check refused a patch tag with `No breaking changes.`: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);

    let root = release_pair("minor-break", FIRST, "0.2.0", breaking, true);
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused a `**Breaking:**` entry on the minor tag 0.2.0: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_passes_when_tag_version_and_changelog_agree() {
    let root = fixture("agree", "0.1.0", CHANGELOG);
    let out = release_check(&root, "0.1.0");
    assert!(
        out.status.success(),
        "release-check failed on an agreeing tag: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_names_a_tag_that_differs_from_the_workspace_version() {
    let changelog = format!("{CHANGELOG}\n## [0.2.0] - 2026-10-06\n\nSecond.\n");
    let root = fixture("differs", "0.1.0", &changelog);
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success(),
        "release-check passed tag 0.2.0 against workspace version 0.1.0"
    );
    assert!(
        stderr(&out).contains("tag 0.2.0 differs from the workspace version 0.1.0"),
        "release-check did not name both versions: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_names_a_missing_changelog_entry() {
    // Mentions of the version that are not its heading do not count, nor does a longer version.
    let changelog = "# Changelog\n\n## [Unreleased]\n\nPrepares [0.1.0].\n\n## [0.1.01] - 2026-10-05\n\
                     \n### [0.1.0]\n";
    let root = fixture("no-entry", "0.1.0", changelog);
    let out = release_check(&root, "0.1.0");
    assert!(
        !out.status.success(),
        "release-check passed without a `## [0.1.0]` heading"
    );
    assert!(
        stderr(&out).contains("CHANGELOG.md has no `## [0.1.0]` entry"),
        "release-check did not name the missing entry: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_reports_every_failure_at_once() {
    let root = fixture("both", "0.1.0", "# Changelog\n\n## [Unreleased]\n");
    let out = release_check(&root, "0.2.0");
    assert!(!out.status.success(), "release-check passed a wrong tag");
    let err = stderr(&out);
    assert!(
        err.contains("tag 0.2.0 differs from the workspace version 0.1.0")
            && err.contains("CHANGELOG.md has no `## [0.2.0]` entry"),
        "release-check did not report both failures: {err}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_refuses_a_tag_that_is_not_a_bare_version() {
    let changelog = "# Changelog\n\n## [v0.1.0] - 2026-10-05\n";
    let root = fixture("prefixed", "v0.1.0", changelog);
    let out = release_check(&root, "v0.1.0");
    assert!(!out.status.success(), "release-check passed tag v0.1.0");
    assert!(
        stderr(&out).contains("tag v0.1.0 is not a bare version"),
        "release-check did not refuse the prefixed tag: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn release_check_names_a_manifest_without_a_workspace_version() {
    let root = fixture("no-version", "0.1.0", CHANGELOG);
    write(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[package]\nversion = \"0.1.0\"\n",
    );
    let out = release_check(&root, "0.1.0");
    assert!(
        !out.status.success(),
        "release-check passed without [workspace.package] version"
    );
    assert!(
        stderr(&out).contains("has no [workspace.package] version"),
        "release-check did not name the missing workspace version: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_repository_passes_release_check_at_its_workspace_version() {
    let version = workspace_version();
    assert_ne!(
        version, "0.0.0",
        "the workspace version is still the unreleased 0.0.0"
    );
    let out = release_check(&repository(), &version);
    assert!(
        out.status.success(),
        "release-check --tag {version} failed on the repository: {}",
        stderr(&out)
    );
}

/// Every Loom dependency line on the README and the site pins the workspace version as a tag.
#[test]
fn readme_and_site_pin_the_workspace_version_as_a_tag() {
    let version = workspace_version();
    let root = repository();
    let mut pages = vec![root.join("README.md")];
    let mut dirs = vec![root.join("website/docs")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
            let path = entry
                .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                .path();
            if path.is_dir() {
                dirs.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "mdx")
            {
                pages.push(path);
            }
        }
    }
    let pin = format!("tag = \"{version}\"");
    let mut lines = 0;
    let mut wrong = Vec::new();
    for page in &pages {
        for (n, line) in read(page).lines().enumerate() {
            if line.contains("git = \"https://github.com/beyond10x/loom\"") {
                lines += 1;
                if !line.contains(&pin) {
                    wrong.push(format!("{}:{}: {line}", page.display(), n + 1));
                }
            }
        }
    }
    assert!(
        lines >= 3,
        "expected the README, embed and move pages to show a Loom dependency line; found {lines}"
    );
    assert!(
        wrong.is_empty(),
        "dependency lines that do not pin {pin}:\n{}",
        wrong.join("\n")
    );
}

fn workflow(name: &str) -> serde_yaml_ng::Value {
    let path = repository().join(".github/workflows").join(name);
    serde_yaml_ng::from_str(&read(&path))
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn release_workflow_verifies_the_tag_and_reuses_the_check() {
    let release = workflow("release.yml");

    let tags = release["on"]["push"]["tags"]
        .as_sequence()
        .unwrap_or_else(|| panic!("release.yml has no on.push.tags"));
    assert_eq!(
        tags,
        &vec![serde_yaml_ng::Value::from(RELEASE_TAGS)],
        "release.yml must run on bare-version tags only"
    );
    assert!(
        release["on"]["push"]["branches"].is_null() && release["on"]["pull_request"].is_null(),
        "release.yml runs on something other than a tag"
    );

    let read_only: serde_yaml_ng::Value = serde_yaml_ng::from_str("contents: read").unwrap();
    assert_eq!(
        release["permissions"], read_only,
        "release.yml must be read-only"
    );

    let jobs = release["jobs"]
        .as_mapping()
        .unwrap_or_else(|| panic!("release.yml has no jobs"));
    let mut verifies = false;
    let mut checks = false;
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or_default();
        if let Some(permissions) = job.get("permissions") {
            assert_eq!(permissions, &read_only, "job {name} widens permissions");
        }
        assert!(job.get("secrets").is_none(), "job {name} passes secrets");
        if job["uses"].as_str() == Some("./.github/workflows/check.yml") {
            checks = true;
        }
        for step in job["steps"].as_sequence().into_iter().flatten() {
            let Some(run) = step["run"].as_str() else {
                continue;
            };
            let run = run.trim();
            assert!(
                !run.contains('\n')
                    && run.starts_with("cargo run --locked -q -p loom-xtask -- release-check "),
                "job {name} runs something other than the Rust release check: {run}"
            );
            if run.contains("--tag \"$TAG\"")
                && step["env"]["TAG"].as_str() == Some("${{ github.ref_name }}")
            {
                verifies = true;
            }
        }
    }
    assert!(
        verifies,
        "no job runs `loom-xtask release-check --tag` on the pushed tag"
    );
    assert!(checks, "no job reuses ./.github/workflows/check.yml");

    // `release-check` diffs the stable surfaces against the previous tag, so the job that runs it
    // checks out the full history with its tags; a shallow clone has no previous tag to diff.
    let verify_steps = jobs
        .values()
        .find(|job| {
            job["steps"]
                .as_sequence()
                .into_iter()
                .flatten()
                .any(|step| {
                    step["run"]
                        .as_str()
                        .is_some_and(|run| run.contains("release-check"))
                })
        })
        .and_then(|job| job["steps"].as_sequence())
        .unwrap_or_else(|| panic!("no job of release.yml runs release-check"));
    let checkout = verify_steps
        .iter()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/checkout@"))
        })
        .unwrap_or_else(|| panic!("the release-check job has no actions/checkout step"));
    assert!(
        checkout["with"]["fetch-depth"].as_i64() == Some(0)
            && checkout["with"]["fetch-tags"].as_bool() == Some(true),
        "the release-check job must check out with `fetch-depth: 0` and `fetch-tags: true` so \
         the previous tag is there to diff against: {:?}",
        checkout["with"]
    );

    let check = workflow("check.yml");
    assert!(
        check["on"]
            .as_mapping()
            .is_some_and(|on| on.contains_key("workflow_call")),
        "check.yml cannot be called by release.yml: no workflow_call trigger"
    );
}
