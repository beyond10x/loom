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
//! entry fails for each path the previous tag's [`STABILITY_PAGE`] lists that it removes and that
//! no entry of a release below the tag deprecates, naming the path. The page lists each backticked
//! `loom_sdk::` path in a table row outside a code fence that is not marked generated; anything
//! else (a variable, a generated row, prose) is outside the window. A removal is any bullet under
//! `### Removed`, and any `**Breaking:**` bullet in another section that says `removed`,
//! `deleted`, `renamed` or `moves to`. It removes every backticked span before its first `;`,
//! wherever the verb stands (`` Removed `a` ``, `` `a` is removed, and so is `b` ``); when it says
//! `renamed` or `moves to`, only the first span before that verb (the first after it when none
//! precedes it), the rest naming the new place. A bullet under `### Deprecated` deprecates every
//! `loom_sdk::` span before its first `;`. A removed or deprecated item covers a path it equals or
//! is a module of, compared by `::` component, so removing `loom_sdk::ports` removes the listed
//! `loom_sdk::ports::Gate`, and deprecating it deprecates that path.
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

/// The text before the first `;` outside a backticked span.
fn clause(text: &str) -> &str {
    let mut in_span = false;
    for (at, c) in text.char_indices() {
        match c {
            '`' => in_span = !in_span,
            ';' if !in_span => return &text[..at],
            _ => {}
        }
    }
    text
}

/// A word of a bullet's prose or one of its backticked spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    Word(&'a str),
    Span(&'a str),
}

/// The words outside backticked spans and the spans themselves, in order.
fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut found = Vec::new();
    for (n, part) in text.split('`').enumerate() {
        if n % 2 == 1 {
            found.push(Token::Span(part));
        } else {
            found.extend(
                part.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .filter(|w| !w.is_empty())
                    .map(Token::Word),
            );
        }
    }
    found
}

/// Whether the token is the prose word `word`, ignoring ASCII case.
fn is_word(token: Option<&Token<'_>>, word: &str) -> bool {
    matches!(token, Some(Token::Word(w)) if w.eq_ignore_ascii_case(word))
}

/// Where the prose says `renamed` or `moves to`, as a token index.
fn rename_at(tokens: &[Token<'_>]) -> Option<usize> {
    (0..tokens.len()).find(|&at| {
        is_word(tokens.get(at), "renamed")
            || (is_word(tokens.get(at), "moves") && is_word(tokens.get(at + 1), "to"))
    })
}

/// Whether the prose of a bullet says it takes something away: `removed`, `deleted`, `renamed`
/// or `moves to`.
fn says_removal(text: &str) -> bool {
    let found = tokens(text);
    rename_at(&found).is_some()
        || found
            .iter()
            .any(|t| is_word(Some(t), "removed") || is_word(Some(t), "deleted"))
}

/// The items a removal bullet names: every span before its first `;`, or, when that clause says
/// `renamed` or `moves to`, only the first span before the verb (the first after it when none
/// precedes it); the other spans name the new place.
fn removal_items(text: &str) -> Vec<&str> {
    let found = tokens(clause(text));
    match rename_at(&found) {
        Some(at) => {
            let before = token_spans(&found[..at]);
            let after = token_spans(&found[at..]);
            before
                .first()
                .or(after.first())
                .copied()
                .into_iter()
                .collect()
        }
        None => token_spans(&found),
    }
}

/// The spans among the tokens, in order.
fn token_spans<'a>(part: &[Token<'a>]) -> Vec<&'a str> {
    part.iter()
        .filter_map(|t| match t {
            Token::Span(s) => Some(*s),
            Token::Word(_) => None,
        })
        .collect()
}

/// Whether `item` is `path` or a module of it, compared by `::` component.
fn covers(item: &str, path: &str) -> bool {
    path == item
        || path
            .strip_prefix(item)
            .is_some_and(|rest| rest.starts_with("::"))
}

/// The prefix of a path the stability page's window covers.
const SDK_PATH: &str = "loom_sdk::";

/// The items an entry removes: those of each bullet under `### Removed`, and of each
/// `**Breaking:**` bullet elsewhere that says `removed`, `deleted`, `renamed` or `moves to`.
fn removed_items(entry: &str) -> Vec<String> {
    let mut items = Vec::new();
    for (section, text) in bullets_by_section(entry) {
        let breaking_removal = text.starts_with(BREAKING) && says_removal(&text);
        if section != "Removed" && !breaking_removal {
            continue;
        }
        let text = text.strip_prefix(BREAKING).unwrap_or(&text);
        for item in removal_items(text) {
            if !item.is_empty() && !items.iter().any(|i| i == item) {
                items.push(item.to_owned());
            }
        }
    }
    items
}

/// The items deprecated in the entries of releases below the tag: every `loom_sdk::` span before
/// the first `;` of each bullet under `### Deprecated`.
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
                items.extend(
                    spans(clause(&text))
                        .into_iter()
                        .filter(|s| s.starts_with(SDK_PATH))
                        .map(str::to_owned),
                );
            }
        }
    }
    items
}

/// One failure per path the previous tag's SDK stability page lists that an item the entry
/// removes covers and that no deprecation of a release below the tag covers.
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
    let mut failures = Vec::new();
    let mut named: Vec<&str> = Vec::new();
    for item in removed {
        for path in listed.iter().filter(|path| covers(item, path)) {
            if named.contains(&path.as_str()) || deprecated.iter().any(|d| covers(d, path)) {
                continue;
            }
            named.push(path);
            let what = if item == path {
                format!("`{item}`")
            } else {
                format!("`{item}`, and with it `{path}`")
            };
            failures.push(format!(
                "CHANGELOG.md entry [{tag}] removes {what}, which the SDK stability page of \
                 {previous} lists ({STABILITY_PAGE}), but no earlier release lists it under \
                 `### Deprecated`; deprecate it for at least one release before removing it"
            ));
        }
    }
    failures
}

/// Whether a table row is marked generated: one of its cells reads `generated` or `*generated*`.
fn marked_generated(row: &str) -> bool {
    row.split('|').any(|cell| {
        cell.trim()
            .trim_matches('*')
            .eq_ignore_ascii_case("generated")
    })
}

/// The `loom_sdk::` paths the SDK stability page at `refs/tags/<previous>` lists: the backticked
/// ones in its table rows outside code fences, rows marked generated left out; empty when the page
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
    Ok(listed_paths(&page))
}

/// The paths a stability page lists, as [`listed_at`] reads them.
fn listed_paths(page: &str) -> Vec<String> {
    let mut fenced = false;
    let mut listed = Vec::new();
    for line in page.lines() {
        let row = line.trim_start();
        if row.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && row.starts_with('|') && !marked_generated(row) {
            for span in spans(row) {
                if span.starts_with(SDK_PATH) && !listed.iter().any(|l| l == span) {
                    listed.push(span.to_owned());
                }
            }
        }
    }
    listed
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
        Breaks, breaks, covers, deprecated_before, entry, is_bare_version, is_entry_heading,
        listed_paths, moves_only_patch, removal_items, removed_items, version_key,
    };

    #[test]
    fn a_removal_names_every_span_before_the_first_semicolon() {
        assert_eq!(
            removal_items("`a` and `b` are removed; use `c`"),
            ["a", "b"]
        );
        assert_eq!(
            removal_items("the harness `R`, `p::R`, is removed, see `x`"),
            ["R", "p::R", "x"]
        );
        assert_eq!(removal_items("Removed `a` (story:s)"), ["a"]);
        assert_eq!(removal_items("`a` is removed, and so is `b`"), ["a", "b"]);
        assert_eq!(removal_items("`a;b` is removed; use `c`"), ["a;b"]);
    }

    #[test]
    fn a_rename_removes_only_its_old_name() {
        assert_eq!(
            removal_items("`A` is renamed `B`, an llm `M` only; `C` is removed"),
            ["A"]
        );
        assert_eq!(removal_items("`A` moves to `p::A`"), ["A"]);
        assert_eq!(removal_items("Renamed `A` to `B`"), ["A"]);
        assert_eq!(removal_items("`renamed` is renamed `x`"), ["renamed"]);
    }

    #[test]
    fn removals_are_removed_bullets_and_breaking_removals() {
        let entry = "### Changed\n\n- **Breaking:** `a` is removed\n- `b` is removed\n\
                     - **Breaking:** `c` takes a context\n\n### Removed\n\n- `d` is gone\n\
                     - **Breaking:**\n  `e` is removed\n\n### Changed\n\n\
                     - **Breaking:** `f` is deleted\n- **Breaking:** `g` is renamed `h`\n\
                     - **Breaking:** `i` moves to `j`\n- **Breaking:** `removed` takes `k`";
        assert_eq!(removed_items(entry), ["a", "d", "e", "f", "g", "i"]);
    }

    #[test]
    fn deprecations_count_only_below_the_tag() {
        let changelog = "## [Unreleased]\n\n### Deprecated\n\n- `loom_sdk::u` is deprecated\n\n\
                         ## [0.3.0] - 2026-10-07\n\n### Deprecated\n\n\
                         - `loom_sdk::c` is deprecated\n\n\
                         ## [0.2.0] - 2026-10-06\n\n### Deprecated\n\n\
                         - `loom_sdk::b` is deprecated; use `loom_sdk::x`\n\
                         - Deprecated `loom_sdk::d` and `NOT_A_PATH`\n\n\
                         ## [0.1.0]\n\n### Added\n\n- `loom_sdk::a`\n";
        assert_eq!(
            deprecated_before(changelog, "0.3.0"),
            ["loom_sdk::b", "loom_sdk::d"]
        );
        assert!(moves_only_patch("0.3.1", "0.3.0") && !moves_only_patch("0.4.0", "0.3.9"));
        assert!(!moves_only_patch("1.0.0", "0.9.9"));
    }

    #[test]
    fn an_item_covers_itself_and_the_paths_of_its_module() {
        assert!(covers("loom_sdk::ports", "loom_sdk::ports"));
        assert!(covers("loom_sdk::ports", "loom_sdk::ports::Gate"));
        assert!(!covers("loom_sdk::ports", "loom_sdk::portsx::Gate"));
        assert!(!covers("loom_sdk::ports::Gate", "loom_sdk::ports"));
    }

    #[test]
    fn the_page_lists_sdk_paths_of_table_rows_not_marked_generated() {
        let page = "Prose names `loom_sdk::Prose`.\n\n| A | B |\n|---|---|\n\
                    | `loom_sdk::A` | names `CURL_HOME` and `loom_sdk::B` |\n\
                    | `loom_sdk::G` | generated |\n| `loom_sdk::H` | *generated* |\n\n\
                    ```text\n| `loom_sdk::Fenced` | type |\n```\n";
        assert_eq!(listed_paths(page), ["loom_sdk::A", "loom_sdk::B"]);
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
