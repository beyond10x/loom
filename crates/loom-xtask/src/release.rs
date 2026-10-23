//! `release-check`: a release tag names the workspace version and has a `CHANGELOG.md` entry that
//! says what breaks for a consumer.
//!
//! `CHANGELOG.md` follows Keep a Changelog: an entry names each break for a consumer in a bullet
//! that starts `**Breaking:**`, or carries the line `No breaking changes.`.
//!
//! `.github/workflows/release.yml` runs it on every pushed tag. It fails, reporting every failure
//! at once, when the tag is not a bare version (`MAJOR.MINOR.PATCH`, digits only, no leading zero),
//! when it differs from `[workspace.package] version` in `<root>/Cargo.toml`, when
//! `<root>/CHANGELOG.md` has no heading `## [<tag>]` or `## [<tag>] - YYYY-MM-DD`, when that entry
//! has neither a `**Breaking:**` bullet nor the line `No breaking changes.`, when it has both, when
//! one of its `**Breaking:**` bullets names nothing or says `none`, or when it says
//! `No breaking changes.` while a stable surface ([`STABLE_SURFACES`]) differs between the previous
//! tag and the tag. A `Breaking:` sentence in prose is no statement.
//!
//! It also holds the SDK stability page's two rules (`story:stability-version-rule`). The tag
//! rule: an entry with a `**Breaking:**` bullet on a tag that moves only the patch version from
//! the previous tag (the same major and minor) fails, naming both tags. The deprecation window: an
//! entry fails for each item it removes that the previous tag's [`STABILITY_PAGE`] lists (as a
//! backticked span outside a code fence) and that no entry of a release below the tag lists under
//! `### Deprecated`, naming the item. An entry removes the subjects of each bullet under
//! `### Removed` and of each `**Breaking:**` bullet elsewhere that says `removed`; a bullet's
//! subjects are its backticked spans before its first `;` and before the word (`removed`,
//! `deprecated`), so `` `a` is removed; use `b` `` removes `a` alone. An item the page does not
//! list is outside the window.
//!
//! The previous tag is the highest bare-version tag of `<root>`'s git repository below the tag;
//! with none, only the statement is required. The tag's side of the diff is the tag when it exists
//! and `HEAD` otherwise, so the check also runs on the release commit before it is tagged. Whether
//! a change is a break is the release author's to say: the check refuses only a missing,
//! contradictory or empty statement, `No breaking changes.` beside a changed surface, a break on
//! a patch tag, or a removal the window does not allow.

use std::fs;
use std::path::Path;
use std::process::Command;

/// The files a consumer pinned to a tag sees, as `git diff` pathspecs: the pinned consumer, the
/// host-port witness and the run-events contract snapshot.
const STABLE_SURFACES: [&str; 3] = [
    "consumers/pinned/",
    "crates/loom-sdk/tests/host_ports.rs",
    "docs/contracts/run-events-v1.json",
];

pub(crate) fn release_check(root: &Path, tag: &str) -> Result<(), String> {
    let mut failures = Vec::new();
    let bare = is_bare_version(tag);
    if !bare {
        failures.push(format!(
            "tag {tag} is not a bare version (MAJOR.MINOR.PATCH, digits only, no leading zero)"
        ));
    }

    let manifest = root.join("Cargo.toml");
    match workspace_version(&manifest) {
        Ok(version) if version == tag => {}
        Ok(version) => failures.push(format!(
            "tag {tag} differs from the workspace version {version} in {}",
            manifest.display()
        )),
        Err(failure) => failures.push(failure),
    }

    let changelog = root.join("CHANGELOG.md");
    match fs::read_to_string(&changelog) {
        Ok(text) => match entry(&text, tag) {
            None => failures.push(format!(
                "CHANGELOG.md has no `## [{tag}]` entry: {}",
                changelog.display()
            )),
            Some(body) => {
                let breaks = breaks(&body);
                for bullet in &breaks.empty {
                    failures.push(format!(
                        "CHANGELOG.md entry [{tag}] has a `**Breaking:**` bullet that names no \
                         break (`{bullet}`); name the break, or write `{NO_BREAKING_CHANGES}` \
                         when nothing breaks: {}",
                        changelog.display()
                    ));
                }
                if breaks.named > 0 && breaks.none {
                    failures.push(format!(
                        "CHANGELOG.md entry [{tag}] says `{NO_BREAKING_CHANGES}` beside a \
                         `{BREAKING}` bullet; keep one: {}",
                        changelog.display()
                    ));
                } else if breaks.named == 0 && !breaks.none && breaks.empty.is_empty() {
                    failures.push(format!(
                        "CHANGELOG.md entry [{tag}] has no `{BREAKING}` bullet and no \
                         `{NO_BREAKING_CHANGES}` line; name each break for a consumer in a \
                         `- {BREAKING} …` bullet, or write `{NO_BREAKING_CHANGES}`: {}",
                        changelog.display()
                    ));
                } else if breaks.none && breaks.named == 0 && bare {
                    match changed_surfaces(root, tag) {
                        Ok(Some((previous, changed))) if !changed.is_empty() => {
                            failures.push(format!(
                                "CHANGELOG.md entry [{tag}] says `{NO_BREAKING_CHANGES}` but \
                                 stable surfaces changed since {previous}: {}",
                                changed.join(", ")
                            ));
                        }
                        Ok(_) => {}
                        Err(failure) => failures.push(failure),
                    }
                }
                let removed = removed_items(&body);
                if bare && (breaks.named > 0 || !removed.is_empty()) {
                    match previous_tag(root, tag) {
                        Ok(Some(previous)) => {
                            if breaks.named > 0 && moves_only_patch(tag, &previous) {
                                failures.push(format!(
                                    "tag {tag} moves only the patch version from {previous} but \
                                     its CHANGELOG.md entry has a `{BREAKING}` bullet; a break \
                                     moves the minor version before 1.0: {}",
                                    changelog.display()
                                ));
                            }
                            if !removed.is_empty() {
                                failures.extend(undeprecated_removals(
                                    root, &text, tag, &previous, &removed,
                                ));
                            }
                        }
                        Ok(None) => {}
                        Err(failure) => failures.push(failure),
                    }
                }
            }
        },
        Err(e) => failures.push(format!("read {}: {e}", changelog.display())),
    }

    if failures.is_empty() {
        println!(
            "release-check: tag {tag} is the workspace version and its CHANGELOG.md entry says \
             what breaks"
        );
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn workspace_version(manifest: &Path) -> Result<String, String> {
    let text =
        fs::read_to_string(manifest).map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", manifest.display()))?;
    table
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} has no [workspace.package] version", manifest.display()))
}

fn is_bare_version(tag: &str) -> bool {
    let parts: Vec<&str> = tag.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (*part == "0" || !part.starts_with('0'))
        })
}

/// A bare version as numbers, for ordering tags.
fn version_key(tag: &str) -> Option<(u64, u64, u64)> {
    if !is_bare_version(tag) {
        return None;
    }
    let mut parts = tag.split('.').map(str::parse::<u64>);
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch))) => Some((major, minor, patch)),
        _ => None,
    }
}

/// `## [<tag>]`, or `## [<tag>] - YYYY-MM-DD`.
fn is_entry_heading(line: &str, tag: &str) -> bool {
    let Some(rest) = line.trim_end().strip_prefix(&format!("## [{tag}]")) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let Some(date) = rest.strip_prefix(" - ") else {
        return false;
    };
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// The lines of the tag's entry, from its heading to the next `## ` heading, or `None` when the
/// changelog has no heading for the tag.
fn entry(changelog: &str, tag: &str) -> Option<String> {
    let mut lines = changelog.lines();
    lines.find(|line| is_entry_heading(line, tag))?;
    Some(
        lines
            .take_while(|line| !line.starts_with("## "))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// The marker a bullet naming a break starts with.
const BREAKING: &str = "**Breaking:**";
/// The line of an entry that names no break.
const NO_BREAKING_CHANGES: &str = "No breaking changes.";

/// What an entry says breaks for a consumer.
#[derive(Debug, Default, PartialEq, Eq)]
struct Breaks {
    /// The `**Breaking:**` bullets that name a break.
    named: usize,
    /// The `**Breaking:**` bullets that name nothing or say `none`, as written.
    empty: Vec<String>,
    /// Whether the entry has the line `No breaking changes.`.
    none: bool,
}

/// Reads the entry's `**Breaking:**` bullets (`- ` or `* `, at any depth, with a bullet's
/// indented continuation lines) and its `No breaking changes.` line. A marker that does not start
/// a bullet's text (quoted in backticks, inside a sentence) is a mention, and a `Breaking:`
/// sentence in prose, the form before Keep a Changelog, is no statement.
fn breaks(entry: &str) -> Breaks {
    let lines: Vec<&str> = entry.lines().collect();
    let mut found = Breaks::default();
    for (at, line) in lines.iter().enumerate() {
        if line.trim() == NO_BREAKING_CHANGES {
            found.none = true;
            continue;
        }
        let Some(rest) = bullet(line).and_then(|text| text.strip_prefix(BREAKING)) else {
            continue;
        };
        let mut text = rest.trim().to_owned();
        for next in &lines[at + 1..] {
            if next.trim().is_empty()
                || !next.starts_with(char::is_whitespace)
                || bullet(next).is_some()
            {
                break;
            }
            text.push(' ');
            text.push_str(next.trim());
        }
        if names_nothing(&text) {
            found.empty.push(line.trim().to_owned());
        } else {
            found.named += 1;
        }
    }
    found
}

/// The text of a `- ` or `* ` bullet, at any depth.
fn bullet(line: &str) -> Option<&str> {
    let item = line.trim_start();
    item.strip_prefix("- ")
        .or_else(|| item.strip_prefix("* "))
        .map(str::trim_start)
}

/// Whether a break's text is empty or its first word is `none`.
fn names_nothing(text: &str) -> bool {
    let word: String = text
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    text.is_empty() || word.eq_ignore_ascii_case("none")
}

/// The highest bare-version tag of `<root>`'s git repository below the tag; `None` when there is
/// none or the tag is no bare version.
fn previous_tag(root: &Path, tag: &str) -> Result<Option<String>, String> {
    let Some(key) = version_key(tag) else {
        return Ok(None);
    };
    let tags = git(root, &["tag", "--list"])?;
    Ok(tags
        .lines()
        .filter_map(|line| version_key(line.trim()).map(|k| (k, line.trim())))
        .filter(|(k, _)| *k < key)
        .max()
        .map(|(_, name)| name.to_owned()))
}

/// The SDK stability page, whose listed paths the deprecation window covers.
const STABILITY_PAGE: &str = "website/docs/reference/sdk-stability.md";

/// Whether `tag` moves only the patch version from `previous` (the same major and minor).
fn moves_only_patch(tag: &str, previous: &str) -> bool {
    match (version_key(tag), version_key(previous)) {
        (Some((major, minor, _)), Some((p_major, p_minor, _))) => {
            major == p_major && minor == p_minor
        }
        _ => false,
    }
}

/// The top-level and nested bullets of an entry (`- ` or `* `, with a bullet's indented
/// continuation lines joined), each with the `### ` section it stands under (empty before any).
fn bullets_by_section(entry: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = entry.lines().collect();
    let mut section = String::new();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if let Some(heading) = line.strip_prefix("### ") {
            section = heading.trim().to_owned();
            continue;
        }
        let Some(text) = bullet(line) else {
            continue;
        };
        let mut text = text.trim().to_owned();
        for next in &lines[at + 1..] {
            if next.trim().is_empty()
                || !next.starts_with(char::is_whitespace)
                || bullet(next).is_some()
            {
                break;
            }
            text.push(' ');
            text.push_str(next.trim());
        }
        found.push((section.clone(), text));
    }
    found
}

/// The backticked spans of a text, in order.
fn spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// Whether `word` is a whole word of `text`, ignoring ASCII case.
fn has_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|w| w.eq_ignore_ascii_case(word))
}

/// The backticked spans a bullet names as the subject of `verb` (`removed`, `deprecated`): the
/// spans before its first `;`, and before the first whole word `verb` in that clause, so
/// `` `a` is removed; use `b` `` names `a` alone.
fn subjects<'a>(text: &'a str, verb: &str) -> Vec<&'a str> {
    let clause = text.split(';').next().unwrap_or_default();
    let mut end = clause.len();
    let mut at = 0;
    for word in clause.split_inclusive(|c: char| !(c.is_alphanumeric() || c == '_')) {
        let bare = word.trim_end_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
        if bare.eq_ignore_ascii_case(verb) && clause[..at].matches('`').count().is_multiple_of(2) {
            end = at;
            break;
        }
        at += word.len();
    }
    spans(&clause[..end])
}

/// The items an entry removes: the subjects of each bullet under `### Removed`, and of each
/// `**Breaking:**` bullet elsewhere that says `removed`.
fn removed_items(entry: &str) -> Vec<String> {
    let mut items = Vec::new();
    for (section, text) in bullets_by_section(entry) {
        let breaking_removal = text.starts_with(BREAKING) && has_word(&text, "removed");
        if section != "Removed" && !breaking_removal {
            continue;
        }
        let text = text.strip_prefix(BREAKING).unwrap_or(&text);
        for item in subjects(text, "removed") {
            if !item.is_empty() && !items.iter().any(|i| i == item) {
                items.push(item.to_owned());
            }
        }
    }
    items
}

/// The items listed under `### Deprecated` in the entries of releases below the tag: each such
/// bullet's subjects.
fn deprecated_before(changelog: &str, tag: &str) -> Vec<String> {
    let Some(key) = version_key(tag) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut lines = changelog.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(version) = line
            .strip_prefix("## [")
            .and_then(|rest| rest.split(']').next())
        else {
            continue;
        };
        if !version_key(version).is_some_and(|k| k < key) || !is_entry_heading(line, version) {
            continue;
        }
        let mut body = Vec::new();
        while let Some(next) = lines.peek() {
            if next.starts_with("## ") {
                break;
            }
            body.push(*next);
            lines.next();
        }
        for (section, text) in bullets_by_section(&body.join("\n")) {
            if section == "Deprecated" {
                items.extend(subjects(&text, "deprecated").into_iter().map(str::to_owned));
            }
        }
    }
    items
}

/// One failure per item the entry removes that the previous tag's SDK stability page lists and no
/// release below the tag listed under `### Deprecated`.
fn undeprecated_removals(
    root: &Path,
    changelog: &str,
    tag: &str,
    previous: &str,
    removed: &[String],
) -> Vec<String> {
    let listed = match listed_at(root, previous) {
        Ok(listed) => listed,
        Err(failure) => return vec![failure],
    };
    let deprecated = deprecated_before(changelog, tag);
    removed
        .iter()
        .filter(|item| listed.contains(item) && !deprecated.contains(item))
        .map(|item| {
            format!(
                "CHANGELOG.md entry [{tag}] removes `{item}`, which the SDK stability page of \
                 {previous} lists ({STABILITY_PAGE}), but no earlier release lists it under \
                 `### Deprecated`; deprecate it for at least one release before removing it"
            )
        })
        .collect()
}

/// The backticked spans of the SDK stability page at `refs/tags/<previous>`, empty when the page
/// is not in that tag.
fn listed_at(root: &Path, previous: &str) -> Result<Vec<String>, String> {
    let tree = format!("refs/tags/{previous}");
    if git(
        root,
        &["ls-tree", "--name-only", &tree, "--", STABILITY_PAGE],
    )?
    .trim()
    .is_empty()
    {
        return Ok(Vec::new());
    }
    let page = git(root, &["show", &format!("{tree}:{STABILITY_PAGE}")])?;
    let mut fenced = false;
    let mut listed = Vec::new();
    for line in page.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            listed.extend(spans(line).into_iter().map(str::to_owned));
        }
    }
    Ok(listed)
}

/// The previous tag and the stable-surface files that differ between it and the tag (or `HEAD`
/// when the tag does not exist yet); `None` when no bare-version tag precedes the tag.
fn changed_surfaces(root: &Path, tag: &str) -> Result<Option<(String, Vec<String>)>, String> {
    let Some(previous) = previous_tag(root, tag)? else {
        return Ok(None);
    };
    let tag_ref = format!("refs/tags/{tag}");
    let end = if git(
        root,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("{tag_ref}^{{commit}}"),
        ],
    )
    .is_ok()
    {
        tag_ref
    } else {
        "HEAD".to_owned()
    };
    let mut args = vec![
        "diff".to_owned(),
        "--name-only".to_owned(),
        format!("refs/tags/{previous}"),
        end,
        "--".to_owned(),
    ];
    args.extend(STABLE_SURFACES.iter().map(|s| (*s).to_owned()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let changed = git(root, &args)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(Some((previous, changed)))
}

/// Runs `git -C <root> <args>` and returns its standard output; the caller's `GIT_DIR` and
/// work-tree overrides are dropped so the repository read is `<root>`'s, and neither system,
/// global nor repository configuration can start a hook, a monitor or a prompt.
fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("run git {} in {}: {e}", args.join(" "), root.display()))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Breaks, breaks, deprecated_before, entry, is_bare_version, is_entry_heading,
        moves_only_patch, removed_items, subjects, version_key,
    };

    #[test]
    fn a_bullet_names_its_subjects_before_the_verb_and_the_first_semicolon() {
        assert_eq!(
            subjects("`a` and `b` are removed; use `c`", "removed"),
            ["a", "b"]
        );
        assert_eq!(
            subjects("the harness `R`, `p::R`, is removed, see `x`", "removed"),
            ["R", "p::R"]
        );
        assert_eq!(subjects("`removed` is removed", "removed"), ["removed"]);
        assert_eq!(subjects("`a` drops `b`", "removed"), ["a", "b"]);
    }

    #[test]
    fn removals_are_removed_bullets_and_breaking_removals() {
        let entry = "### Changed\n\n- **Breaking:** `a` is removed\n- `b` is removed\n\
                     - **Breaking:** `c` takes a context\n\n### Removed\n\n- `d` is gone\n\
                     - **Breaking:**\n  `e` is removed";
        assert_eq!(removed_items(entry), ["a", "d", "e"]);
    }

    #[test]
    fn deprecations_count_only_below_the_tag() {
        let changelog = "## [Unreleased]\n\n### Deprecated\n\n- `u` is deprecated\n\n\
                         ## [0.3.0] - 2026-10-07\n\n### Deprecated\n\n- `c` is deprecated\n\n\
                         ## [0.2.0] - 2026-10-06\n\n### Deprecated\n\n\
                         - `b` is deprecated; use `x`\n\n## [0.1.0]\n\n### Added\n\n- `a`\n";
        assert_eq!(deprecated_before(changelog, "0.3.0"), ["b"]);
        assert!(moves_only_patch("0.3.1", "0.3.0") && !moves_only_patch("0.4.0", "0.3.9"));
        assert!(!moves_only_patch("1.0.0", "0.9.9"));
    }

    #[test]
    fn bare_versions() {
        for tag in ["0.1.0", "1.20.3", "10.0.0"] {
            assert!(is_bare_version(tag), "{tag}");
        }
        for tag in [
            "v0.1.0",
            "0.1",
            "0.1.0.0",
            "0.1.0-rc.1",
            "01.0.0",
            "0..1",
            "",
        ] {
            assert!(!is_bare_version(tag), "{tag}");
        }
    }

    #[test]
    fn entry_headings() {
        assert!(is_entry_heading("## [0.1.0] - 2026-10-05", "0.1.0"));
        assert!(is_entry_heading("## [0.1.0]", "0.1.0"));
        for line in [
            "## [0.1.01] - 2026-10-05",
            "### [0.1.0]",
            "## [0.1.0] 2026-10-05",
            "## [0.1.0] - yesterday",
            " ## [0.1.0]",
        ] {
            assert!(!is_entry_heading(line, "0.1.0"), "{line}");
        }
    }

    #[test]
    fn versions_order_numerically() {
        assert!(version_key("0.10.0") > version_key("0.9.0"));
        assert_eq!(version_key("v0.1.0"), None);
    }

    #[test]
    fn an_entry_ends_at_the_next_version_heading() {
        let changelog = "## [0.2.0] - 2026-10-06\n\n### Added\n\n- x\n\n\
                         ## [0.1.0]\n\nNo breaking changes.\n";
        let body = entry(changelog, "0.2.0").unwrap_or_default();
        assert!(
            body.contains("### Added") && !body.contains("No breaking"),
            "{body}"
        );
        assert_eq!(entry(changelog, "0.3.0"), None);
    }

    #[test]
    fn breaking_bullets() {
        let none = Breaks {
            none: true,
            ..Breaks::default()
        };
        for text in [
            "No breaking changes.",
            "No breaking changes.\n\n### Changed\n\n- `x` is faster.",
            "  No breaking changes.  ",
        ] {
            assert_eq!(breaks(text), none, "{text}");
        }
        for text in [
            "- **Breaking:** `LoopStop` gains a variant.",
            "* **Breaking:** `Port::call` takes a context (story:port-context).",
            "- `x` is new.\n  - **Breaking:** nested under another bullet.",
            "- **Breaking:**\n  `Port::call` takes a context.",
            "- **Breaking:** nonexhaustive matches on `Outcome` break.",
        ] {
            assert_eq!(breaks(text).named, 1, "{text}");
            assert!(
                breaks(text).empty.is_empty() && !breaks(text).none,
                "{text}"
            );
        }
        for text in [
            "- **Breaking:** none.",
            "- **Breaking:** None",
            "- **Breaking:**",
            "- **Breaking:**\n- `x` is new.",
        ] {
            let found = breaks(text);
            assert_eq!((found.named, found.empty.len()), (0, 1), "{text}");
        }
        for text in [
            "Breaking: `Port::call` takes a context.",
            "**Breaking:** `Port::call` takes a context.",
            "- A break is named in a `**Breaking:**` bullet.",
            "- Breaking for exhaustive matches.",
            "- No breaking changes.",
            "The release has No breaking changes. Honest.",
        ] {
            assert_eq!(breaks(text), Breaks::default(), "{text}");
        }
    }
}
