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
//! `story:release-publishes-images`: after `check.yml` passes on the tag, `release.yml` calls
//! `images.yml` with `push: true`, the one job granted `packages: write`; `images.yml` refuses a
//! version the registry holds, pushes the `loom-agent` and `loom-operator` images and the
//! `loom-agents` chart, and on a pull request builds the images without pushing. Every action is
//! pinned by commit, and each Dockerfile builds its locked release binary onto a digest-pinned
//! base that runs it as user 10001.
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
//! `story:release-binaries`: after `check.yml` passes on the tag, `release.yml` calls
//! `binaries.yml` read-only with the tag as `version`. It builds the locked `b10x-loom` from the
//! tagged commit in the agent image's builder, packs it with `LICENSE` as
//! `b10x-loom-<version>-x86_64-unknown-linux-gnu.tar.gz` beside its `.sha256`, and keeps both as
//! the workflow artifact `b10x-loom-<version>`; no workflow holds `contents: write`, reads a
//! secret or uploads to a release, because the bot attaches the files (AGENTS.md § Releases). The
//! README and the getting-started page download that archive and check it with `sha256sum -c`.
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
    release_pair_with_page(
        name,
        STABILITY_PAGE,
        first,
        second_version,
        second,
        tag_second,
    )
}

/// [`release_pair`] with `page` as the SDK stability page of both commits.
fn release_pair_with_page(
    name: &str,
    page: &str,
    first: &str,
    second_version: &str,
    second: &str,
    tag_second: bool,
) -> PathBuf {
    let root = fixture(name, "0.1.0", &changelog_with(&[("0.1.0", first)]));
    let stability = root.join("website/docs/reference/sdk-stability.md");
    fs::create_dir_all(stability.parent().unwrap_or(&root))
        .unwrap_or_else(|e| panic!("create {}: {e}", stability.display()));
    write(&stability, page);
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

/// The variant `story:engineering-registry-calls-replaced` removes.
const ROUTER_REGISTRY: &str = "`loom_sdk::intake::router::RouterError::Registry`";

/// The bullets under `### Removed` in the `## [<version>]` section of `changelog`; the date after
/// the `]` is ignored.
fn section_removed(changelog: &str, version: &str) -> Vec<String> {
    let heading = format!("## [{version}]");
    let section = changelog
        .split_once(heading.as_str())
        .map(|(_, rest)| rest.split("\n## [").next().unwrap_or(rest))
        .unwrap_or_else(|| panic!("CHANGELOG.md has no `{heading}` section"));
    let mut bullets = Vec::new();
    let mut in_removed = false;
    for line in section.lines() {
        if let Some(heading) = line.strip_prefix("### ") {
            in_removed = heading.trim() == "Removed";
        } else if in_removed && line.starts_with("- ") {
            bullets.push(line.to_owned());
        }
    }
    bullets
}

#[test]
fn the_router_registry_removal_passes_release_check() {
    let changelog = read(&repository().join("CHANGELOG.md"));
    let bullet = section_removed(&changelog, "0.58.0")
        .into_iter()
        .find(|bullet| bullet.contains(ROUTER_REGISTRY))
        .unwrap_or_else(|| {
            panic!("CHANGELOG.md [0.58.0] has no `### Removed` bullet naming {ROUTER_REGISTRY}")
        });
    // 0.1.0's page lists the module the variant lives in, as the repository's page does.
    let page = "# SDK stability\n\n| Path | What |\n|---|---|\n\
                | `loom_sdk::intake::router` | The re-exported crate |\n";
    let root = release_pair_with_page(
        "router-registry",
        page,
        FIRST,
        "0.2.0",
        &format!("### Removed\n\n{bullet}"),
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        out.status.success(),
        "release-check refused the repository's bullet {bullet:?}: {}",
        stderr(&out)
    );
    let _ = fs::remove_dir_all(&root);

    // The same page refuses removing the listed module itself, so the page was read.
    let root = release_pair_with_page(
        "router-module",
        page,
        FIRST,
        "0.2.0",
        "### Removed\n\n- **Breaking:** `loom_sdk::intake::router` is removed",
        true,
    );
    let out = release_check(&root, "0.2.0");
    assert!(
        !out.status.success()
            && stderr(&out)
                .contains("CHANGELOG.md entry [0.2.0] removes `loom_sdk::intake::router`"),
        "release-check passed removing the listed module `loom_sdk::intake::router`: {}",
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
    let publish: serde_yaml_ng::Value =
        serde_yaml_ng::from_str("contents: read\npackages: write").unwrap();
    let mut verifies = false;
    let mut checks = false;
    let mut publishes = false;
    let mut builds = false;
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or_default();
        if job["uses"].as_str() == Some("./.github/workflows/binaries.yml") {
            // `story:release-binaries`: the binaries are built read-only, after the tagged commit
            // passed check.yml, for the version the tag names.
            assert_eq!(
                job.get("permissions"),
                Some(&read_only),
                "job {name} must be granted exactly contents: read"
            );
            assert_eq!(
                job["needs"].as_str(),
                Some("check"),
                "job {name} builds before the tagged commit passed check.yml"
            );
            let version: serde_yaml_ng::Value =
                serde_yaml_ng::from_str("version: ${{ github.ref_name }}").unwrap();
            assert_eq!(
                job["with"], version,
                "job {name} must call binaries.yml with exactly version: ${{{{ github.ref_name }}}}"
            );
            assert!(job.get("secrets").is_none(), "job {name} passes secrets");
            builds = true;
            continue;
        }
        if job["uses"].as_str() == Some("./.github/workflows/images.yml") {
            // `story:release-publishes-images`: the one job that may write packages, and only
            // after the tag is verified and checked.
            assert_eq!(
                job["permissions"], publish,
                "job {name} must be granted exactly contents: read and packages: write"
            );
            assert_eq!(
                job["needs"].as_str(),
                Some("check"),
                "job {name} publishes before the tagged commit passed check.yml"
            );
            assert_eq!(
                job["with"]["push"].as_bool(),
                Some(true),
                "job {name} must call images.yml with push: true"
            );
            assert!(job.get("secrets").is_none(), "job {name} passes secrets");
            publishes = true;
            continue;
        }
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
    assert!(
        publishes,
        "no job of release.yml calls ./.github/workflows/images.yml"
    );
    assert!(
        builds,
        "no job of release.yml calls ./.github/workflows/binaries.yml"
    );

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

/// The images a release publishes, each built from `deploy/images/<image>/Dockerfile`.
const IMAGES: [&str; 2] = ["loom-agent", "loom-operator"];

/// Every `uses:` of a workflow's jobs and steps.
fn uses(workflow: &serde_yaml_ng::Value) -> Vec<String> {
    let mut out = Vec::new();
    for job in workflow["jobs"]
        .as_mapping()
        .into_iter()
        .flat_map(|jobs| jobs.values())
    {
        out.extend(job["uses"].as_str().map(str::to_owned));
        for step in job["steps"].as_sequence().into_iter().flatten() {
            out.extend(step["uses"].as_str().map(str::to_owned));
        }
    }
    out
}

#[test]
fn every_action_of_the_release_and_images_workflows_is_pinned_by_commit() {
    for name in ["release.yml", "images.yml"] {
        for action in uses(&workflow(name)) {
            if action.starts_with("./") {
                continue;
            }
            let pin = action.rsplit_once('@').map(|(_, pin)| pin).unwrap_or("");
            assert!(
                pin.len() == 40 && pin.bytes().all(|b| b.is_ascii_hexdigit()),
                "{name}: {action} is not pinned by a commit SHA"
            );
        }
    }
}

#[test]
fn images_workflow_builds_on_pull_requests_and_publishes_only_when_called_to_push() {
    // `story:release-publishes-images`.
    let images = workflow("images.yml");
    let paths: Vec<&str> = images["on"]["pull_request"]["paths"]
        .as_sequence()
        .unwrap_or_else(|| panic!("images.yml has no on.pull_request.paths"))
        .iter()
        .filter_map(serde_yaml_ng::Value::as_str)
        .collect();
    for path in [
        "deploy/images/**",
        ".github/workflows/images.yml",
        "Cargo.lock",
    ] {
        assert!(
            paths.contains(&path),
            "images.yml does not build on a change to {path}"
        );
    }
    assert_eq!(
        images["on"]["workflow_call"]["inputs"]["push"]["type"].as_str(),
        Some("boolean"),
        "images.yml takes no boolean input push"
    );
    assert!(
        images["on"]["push"].is_null(),
        "images.yml runs on a push of its own"
    );
    assert_eq!(
        images["env"]["REGISTRY"].as_str(),
        Some("ghcr.io/beyond10x")
    );
    assert_eq!(images["env"]["CHART"].as_str(), Some("charts/loom-agents"));

    let jobs = &images["jobs"];
    let absent = jobs["absent"]["steps"]
        .as_sequence()
        .and_then(|steps| steps.iter().find_map(|step| step["run"].as_str()))
        .unwrap_or_else(|| panic!("images.yml has no job `absent` that runs a check"));
    for package in IMAGES.iter().copied().chain(["charts/loom-agents"]) {
        assert!(
            absent.contains(package),
            "the job `absent` does not check that {package} lacks the version"
        );
    }
    assert_eq!(jobs["absent"]["if"].as_str(), Some("${{ inputs.push }}"));

    let image = &jobs["image"];
    assert_eq!(image["needs"].as_str(), Some("absent"));
    let matrix: Vec<&str> = image["strategy"]["matrix"]["image"]
        .as_sequence()
        .unwrap_or_else(|| panic!("the job `image` has no matrix of images"))
        .iter()
        .filter_map(serde_yaml_ng::Value::as_str)
        .collect();
    assert_eq!(matrix, IMAGES, "the images a release publishes");
    let build = image["steps"]
        .as_sequence()
        .and_then(|steps| {
            steps.iter().find(|step| {
                step["uses"]
                    .as_str()
                    .is_some_and(|uses| uses.starts_with("docker/build-push-action@"))
            })
        })
        .unwrap_or_else(|| panic!("the job `image` has no docker/build-push-action step"));
    assert_eq!(build["with"]["platforms"].as_str(), Some("linux/amd64"));
    assert_eq!(
        build["with"]["push"].as_str(),
        Some("${{ inputs.push == true }}"),
        "an image is pushed only when images.yml is called with push: true"
    );
    assert_eq!(
        build["with"]["file"].as_str(),
        Some("deploy/images/${{ matrix.image }}/Dockerfile")
    );

    let chart = &jobs["chart"];
    assert_eq!(chart["needs"].as_str(), Some("image"));
    assert_eq!(chart["if"].as_str(), Some("${{ inputs.push }}"));
    let push = chart["steps"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|step| step["run"].as_str())
        .find(|run| run.contains("helm push"))
        .unwrap_or_else(|| panic!("the job `chart` runs no helm push"));
    assert!(
        push.contains("helm package charts/loom-agents")
            && push.contains("helm push \"$archive\" \"oci://$REGISTRY/charts\""),
        "the chart is pushed to oci://ghcr.io/beyond10x/charts: {push}"
    );
}

#[test]
fn each_image_runs_its_locked_release_binary_as_a_non_root_user_on_pinned_bases() {
    // `story:release-publishes-images`.
    for (image, binary, package) in [
        ("loom-agent", "b10x-loom", "b10x-loom-cli"),
        ("loom-operator", "b10x-loom-operator", "b10x-loom-operator"),
    ] {
        let path = repository()
            .join("deploy/images")
            .join(image)
            .join("Dockerfile");
        let text = read(&path);
        for line in text.lines().filter(|line| line.starts_with("FROM ")) {
            assert!(
                line.contains("@sha256:"),
                "{image}: base not pinned by digest: {line}"
            );
        }
        assert!(
            text.contains(&format!(
                "cargo build --release --locked -p {package} --bin {binary}"
            )),
            "{image}: does not build {binary} from {package} with --locked"
        );
        assert!(
            text.contains(&format!("ENTRYPOINT [\"/usr/local/bin/{binary}\"]")),
            "{image}: does not run {binary}"
        );
        assert!(
            text.contains(&format!("/usr/local/bin/{binary} --version")),
            "{image}: does not run {binary} --version at build time"
        );
        assert!(
            text.contains("grep 'not found'"),
            "{image}: does not fail on an unresolved shared library"
        );
        let user = text
            .lines()
            .filter_map(|line| line.strip_prefix("USER "))
            .next_back();
        assert_eq!(
            user,
            Some("10001:10001"),
            "{image}: must end as the numeric user the chart and the controller's pods run as"
        );
    }
}

/// The one target a release builds `b10x-loom` for (`story:release-binaries`).
const TARGET: &str = "x86_64-unknown-linux-gnu";

/// The file names of every workflow under `.github/workflows/`.
fn workflow_names() -> Vec<String> {
    let dir = repository().join(".github/workflows");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".yml") || name.ends_with(".yaml"))
        .collect();
    names.sort();
    names
}

/// The one job of `binaries.yml`.
fn binaries_job(binaries: &serde_yaml_ng::Value) -> &serde_yaml_ng::Value {
    let jobs = binaries["jobs"]
        .as_mapping()
        .unwrap_or_else(|| panic!("binaries.yml has no jobs"));
    assert_eq!(
        jobs.len(),
        1,
        "binaries.yml must hold one job, binaries / {TARGET}"
    );
    jobs.values().next().unwrap()
}

fn steps(job: &serde_yaml_ng::Value) -> &[serde_yaml_ng::Value] {
    job["steps"]
        .as_sequence()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("the job has no steps"))
}

/// The index of the first step whose `uses` starts with `action@`.
fn step_using(steps: &[serde_yaml_ng::Value], action: &str) -> usize {
    steps
        .iter()
        .position(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with(&format!("{action}@")))
        })
        .unwrap_or_else(|| panic!("no step uses {action}"))
}

/// The index of the first step whose `run` holds `fragment`.
fn step_running(steps: &[serde_yaml_ng::Value], fragment: &str) -> usize {
    steps
        .iter()
        .position(|step| {
            step["run"]
                .as_str()
                .is_some_and(|run| run.contains(fragment))
        })
        .unwrap_or_else(|| panic!("no step runs {fragment}"))
}

#[test]
fn binaries_workflow_builds_read_only_and_keeps_the_archive_and_its_checksum() {
    let binaries = workflow("binaries.yml");
    let triggers: Vec<&str> = binaries["on"]
        .as_mapping()
        .unwrap_or_else(|| panic!("binaries.yml has no `on` mapping"))
        .keys()
        .filter_map(serde_yaml_ng::Value::as_str)
        .collect();
    assert_eq!(
        triggers,
        ["workflow_call"],
        "binaries.yml must run only when called"
    );
    let input = &binaries["on"]["workflow_call"]["inputs"]["version"];
    assert_eq!(input["type"].as_str(), Some("string"));
    assert_eq!(input["required"].as_bool(), Some(true));

    let job = binaries_job(&binaries);
    assert_eq!(
        job["name"].as_str(),
        Some("binaries / x86_64-unknown-linux-gnu")
    );
    assert_eq!(job["runs-on"].as_str(), Some("ubuntu-24.04"));
    assert_eq!(
        job["env"]["VERSION"].as_str(),
        Some("${{ inputs.version }}"),
        "the job reads the version from its input through the environment"
    );
    let steps = steps(job);

    // The tagged commit, as the caller's run checked it out: no `ref` names another.
    let checkout = step_using(steps, "actions/checkout");
    assert_eq!(
        steps[checkout]["with"]["persist-credentials"].as_bool(),
        Some(false)
    );
    assert!(
        steps[checkout]["with"].get("ref").is_none(),
        "the checkout must build the commit the release run was started for"
    );

    let build = step_running(
        steps,
        "cargo build --release --locked -p b10x-loom-cli --bin b10x-loom",
    );
    // story:drop-the-substrate-git-feature: the built binary links no libgit2, libssh2, OpenSSL
    // or zlib, checked before it is packed.
    let linked = step_running(
        steps,
        "cargo run -q --locked -p loom-xtask -- linked-libraries target/release/b10x-loom",
    );
    let package = step_running(steps, "sha256sum");
    assert!(
        build < linked && linked < package,
        "binaries.yml must check the linked libraries after the build and before packing"
    );
    let run = steps[package]["run"].as_str().unwrap();
    for fragment in [
        r#"[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]"#,
        r#"test "$(target/release/b10x-loom --version)" = "b10x-loom $VERSION""#,
        r#"name="b10x-loom-$VERSION-x86_64-unknown-linux-gnu""#,
        r#"install -m 0755 target/release/b10x-loom "$name/b10x-loom""#,
        r#"install -m 0644 LICENSE "$name/LICENSE""#,
        r#"tar -czf "$name.tar.gz" "$name""#,
        r#"sha256sum "$name.tar.gz" > "$name.tar.gz.sha256""#,
        r#"cat "$name.tar.gz.sha256""#,
    ] {
        assert!(
            run.contains(fragment),
            "the packaging step does not run `{fragment}`:\n{run}"
        );
    }

    let upload = step_using(steps, "actions/upload-artifact");
    assert!(
        checkout < build && build < package && package < upload,
        "binaries.yml must check out, build, package and then upload, in that order"
    );
    let with = &steps[upload]["with"];
    assert_eq!(
        with["name"].as_str(),
        Some("b10x-loom-${{ inputs.version }}")
    );
    let archive = format!("b10x-loom-${{{{ inputs.version }}}}-{TARGET}.tar.gz");
    let paths: Vec<&str> = with["path"]
        .as_str()
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        paths,
        [archive.clone(), format!("{archive}.sha256")],
        "the artifact must hold the archive and its checksum, and nothing else"
    );
    assert_eq!(with["if-no-files-found"].as_str(), Some("error"));
    assert_eq!(with["retention-days"].as_i64(), Some(30));
}

#[test]
fn binaries_workflow_reads_no_secret_uploads_to_no_release_and_puts_no_expression_in_a_shell() {
    let path = repository().join(".github/workflows/binaries.yml");
    let text = read(&path);
    for word in ["secrets", "github.token", "GITHUB_TOKEN", "GH_TOKEN"] {
        assert!(!text.contains(word), "binaries.yml names {word}");
    }
    let binaries = workflow("binaries.yml");
    let mut actions: Vec<String> = uses(&binaries)
        .iter()
        .map(|action| action.split('@').next().unwrap_or_default().to_owned())
        .collect();
    actions.sort();
    assert_eq!(
        actions,
        ["actions/checkout", "actions/upload-artifact"],
        "binaries.yml may only check out and keep a workflow artifact"
    );
    for step in steps(binaries_job(&binaries)) {
        let Some(run) = step["run"].as_str() else {
            continue;
        };
        // A tag name reaches a shell only through the environment, quoted.
        assert!(
            !run.contains("${{"),
            "a step puts an expression into its shell line: {run}"
        );
        for word in ["gh ", "curl", "wget", "github.com"] {
            assert!(!run.contains(word), "a step reaches GitHub ({word}): {run}");
        }
    }
}

#[test]
fn every_action_of_the_binaries_workflow_is_pinned_as_the_other_workflows_pin_it() {
    let mut pinned = Vec::new();
    for name in workflow_names() {
        if name != "binaries.yml" {
            pinned.extend(uses(&workflow(&name)));
        }
    }
    let actions = uses(&workflow("binaries.yml"));
    assert!(!actions.is_empty(), "binaries.yml uses no action");
    for action in actions {
        let pin = action.rsplit_once('@').map(|(_, pin)| pin).unwrap_or("");
        assert!(
            pin.len() == 40 && pin.bytes().all(|b| b.is_ascii_hexdigit()),
            "binaries.yml: {action} is not pinned by a commit SHA"
        );
        assert!(
            pinned.contains(&action),
            "binaries.yml: {action} is pinned to a commit no other workflow uses"
        );
    }
}

#[test]
fn no_workflow_is_granted_contents_write() {
    let read_only: serde_yaml_ng::Value = serde_yaml_ng::from_str("contents: read").unwrap();
    let names = workflow_names();
    assert!(
        names.contains(&"binaries.yml".to_owned()),
        "binaries.yml is missing"
    );
    for name in &names {
        let file = workflow(name);
        let mut blocks = vec![("the workflow".to_owned(), &file["permissions"])];
        for (job, body) in file["jobs"].as_mapping().into_iter().flatten() {
            blocks.push((
                format!("job {}", job.as_str().unwrap_or_default()),
                &body["permissions"],
            ));
        }
        for (owner, permissions) in &blocks {
            assert_ne!(
                permissions.as_str(),
                Some("write-all"),
                "{name}: {owner} is granted write-all"
            );
            assert_ne!(
                permissions["contents"].as_str(),
                Some("write"),
                "{name}: {owner} is granted contents: write"
            );
        }
        if name == "binaries.yml" {
            assert_eq!(blocks.len(), 2, "binaries.yml holds one job");
            for (owner, permissions) in &blocks {
                assert_eq!(
                    *permissions, &read_only,
                    "binaries.yml: {owner} must be granted exactly contents: read"
                );
            }
        }
    }
}

#[test]
fn the_binaries_build_uses_the_agent_images_builder() {
    let dockerfile = read(&repository().join("deploy/images/loom-agent/Dockerfile"));
    let builder = dockerfile
        .lines()
        .find_map(|line| {
            line.strip_prefix("FROM ")
                .and_then(|rest| rest.strip_suffix(" AS builder"))
        })
        .unwrap_or_else(|| panic!("the loom-agent Dockerfile has no `FROM … AS builder` line"));
    assert!(
        builder.contains("@sha256:"),
        "the builder is not pinned by digest: {builder}"
    );
    assert_eq!(
        binaries_job(&workflow("binaries.yml"))["container"].as_str(),
        Some(builder),
        "binaries.yml must build in the loom-agent image's builder"
    );
}

/// The body of the `## <heading>` section of `text`, up to the next `## ` heading, its words
/// joined by single spaces.
fn section(text: &str, heading: &str) -> String {
    let marker = format!("\n## {heading}\n");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("no section `## {heading}`"))
        + marker.len();
    let rest = &text[start..];
    let body = &rest[..rest.find("\n## ").unwrap_or(rest.len())];
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn agents_md_releases_names_the_attach_and_verify_commands() {
    let agents = read(&repository().join("AGENTS.md"));
    let releases = section(&agents, "Releases");
    for command in [
        "./.github/workflows/binaries.yml",
        "gh run download",
        "b10x-gates gh -- release upload",
        "gh release download",
        "sha256sum -c",
    ] {
        assert!(
            releases.contains(command),
            "AGENTS.md § Releases does not name `{command}`"
        );
    }
    let finished = releases
        .split_once("A release is finished only once")
        .map(|(_, rest)| rest.split(". ").next().unwrap_or_default())
        .unwrap_or_else(|| panic!("AGENTS.md § Releases does not say when a release is finished"));
    assert!(
        finished.contains("binary assets and their checksums are verified"),
        "AGENTS.md § Releases does not finish a release on its verified assets: {finished}"
    );
    let writes = section(&agents, "Commits, pushes and GitHub writes");
    assert!(
        writes.contains("b10x-gates gh -- release upload"),
        "AGENTS.md § Commits, pushes and GitHub writes does not name the asset upload"
    );
}

#[test]
fn readme_and_site_download_the_release_binary_with_its_checksum() {
    let version = workspace_version();
    let archive = format!("b10x-loom-{version}-{TARGET}.tar.gz");
    let url = format!("https://github.com/beyond10x/loom/releases/download/{version}/{archive}");
    for page in ["README.md", "website/docs/getting-started.md"] {
        let text = read(&repository().join(page));
        assert!(
            text.lines().any(|line| line.contains(&url)),
            "{page} does not download {url}"
        );
        assert!(
            text.lines()
                .any(|line| line.contains(&format!("{url}.sha256"))),
            "{page} does not download the checksum of {archive}"
        );
        assert!(
            text.lines()
                .any(|line| line.trim().starts_with("sha256sum -c") && line.contains(&archive)),
            "{page} has no `sha256sum -c` line for {archive}"
        );
    }
}
