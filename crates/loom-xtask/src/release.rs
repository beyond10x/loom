//! `release-check`: a release tag names the workspace version and has a `CHANGELOG.md` entry.
//!
//! `.github/workflows/release.yml` runs it on every pushed tag. It fails, reporting every failure
//! at once, when the tag is not a bare version (`MAJOR.MINOR.PATCH`, digits only, no leading zero),
//! when it differs from `[workspace.package] version` in `<root>/Cargo.toml`, or when
//! `<root>/CHANGELOG.md` has no heading `## [<tag>]` or `## [<tag>] - YYYY-MM-DD`.

use std::fs;
use std::path::Path;

pub(crate) fn release_check(root: &Path, tag: &str) -> Result<(), String> {
    let mut failures = Vec::new();
    if !is_bare_version(tag) {
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
        Ok(text) if text.lines().any(|line| is_entry_heading(line, tag)) => {}
        Ok(_) => failures.push(format!(
            "CHANGELOG.md has no `## [{tag}]` entry: {}",
            changelog.display()
        )),
        Err(e) => failures.push(format!("read {}: {e}", changelog.display())),
    }

    if failures.is_empty() {
        println!("release-check: tag {tag} is the workspace version and has a CHANGELOG.md entry");
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

#[cfg(test)]
mod tests {
    use super::{is_bare_version, is_entry_heading};

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
}
