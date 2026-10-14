//! The host's test-path patterns (`intake.routing.SliceRun.test_paths`): the paths `tests.write`
//! may change.
//!
//! `software.change/1` gives `tests.write` no precondition, so a bugfix could otherwise change its
//! implementation through `tests.write` before any test failed. The slice therefore performs
//! `tests.write` only when every path it changes matches one of these patterns, and refuses it
//! otherwise, naming the first path outside them, before anything is written or committed
//! ([`crate::executor::ExecuteError::NotATestPath`]). The rule holds for every change kind;
//! `implementation.edit` is not restricted by it.
//!
//! The patterns are the host's trusted context, never a model's: `b10x-loom run --test-path`
//! (repeatable), or the embedder's [`crate::run::SliceRequest::test_paths`]. Without one,
//! [`TestPaths::DEFAULTS`] apply.
//!
//! # Pattern syntax
//!
//! A pattern is matched against a path relative to the workspace root, as the executor resolves
//! it (`./src/../` never survives: such a path is refused before), segment by segment, each
//! separated by `/`:
//!
//! - `**`, a whole segment, matches any number of segments, none included, except as the last
//!   segment, where it matches one or more: `tests/**` matches what is inside `tests`, never a
//!   file named `tests`;
//! - `*` matches any run of characters within one segment, none included;
//! - `?` matches exactly one character within one segment;
//! - any other character matches itself, case included.
//!
//! A pattern is refused when it is empty, starts with `/`, has an empty segment, a `.` or `..`
//! segment, a NUL, or `**` within a segment rather than as a whole one.

use std::fmt;
use std::path::{Component, Path};

/// The host's test-path patterns (see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPaths {
    patterns: Vec<String>,
}

/// Why a list of test-path patterns was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestPathsError {
    /// The list names no pattern.
    Empty,
    /// `pattern` is not a pattern (see the module documentation, "Pattern syntax").
    Invalid { pattern: String, problem: String },
}

impl fmt::Display for TestPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("no test-path pattern is given"),
            Self::Invalid { pattern, problem } => {
                write!(f, "test-path pattern `{pattern}` {problem}")
            }
        }
    }
}

impl std::error::Error for TestPathsError {}

impl TestPaths {
    /// The patterns that apply when the host gives none.
    pub const DEFAULTS: [&'static str; 8] = [
        "tests/**",
        "**/tests/**",
        "test/**",
        "**/test/**",
        "**/*_test.*",
        "**/test_*.*",
        "**/*.test.*",
        "**/*.spec.*",
    ];

    /// `patterns`, each checked.
    ///
    /// # Errors
    /// [`TestPathsError::Empty`] for no pattern, [`TestPathsError::Invalid`] naming the first
    /// pattern that is not one.
    pub fn new<P: Into<String>>(
        patterns: impl IntoIterator<Item = P>,
    ) -> Result<Self, TestPathsError> {
        let patterns: Vec<String> = patterns.into_iter().map(Into::into).collect();
        if patterns.is_empty() {
            return Err(TestPathsError::Empty);
        }
        for pattern in &patterns {
            checked(pattern)?;
        }
        Ok(Self { patterns })
    }

    /// The patterns, in the order given.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Whether `relative`, a path relative to the workspace root made only of normal components,
    /// matches one of the patterns. A path that is not UTF-8 matches none.
    pub fn matches(&self, relative: &Path) -> bool {
        let mut segments = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(name) => match name.to_str() {
                    Some(name) => segments.push(name),
                    None => return false,
                },
                Component::CurDir => {}
                _ => return false,
            }
        }
        !segments.is_empty()
            && self.patterns.iter().any(|pattern| {
                let pattern: Vec<&str> = pattern.split('/').collect();
                path_matches(&pattern, &segments)
            })
    }
}

impl Default for TestPaths {
    /// [`TestPaths::DEFAULTS`].
    fn default() -> Self {
        Self {
            patterns: Self::DEFAULTS.iter().map(|&p| p.to_owned()).collect(),
        }
    }
}

fn checked(pattern: &str) -> Result<(), TestPathsError> {
    let invalid = |problem: &str| TestPathsError::Invalid {
        pattern: pattern.to_owned(),
        problem: problem.to_owned(),
    };
    if pattern.is_empty() {
        return Err(invalid("is empty"));
    }
    if pattern.contains('\0') {
        return Err(invalid("holds a NUL"));
    }
    if pattern.starts_with('/') {
        return Err(invalid("is absolute"));
    }
    for segment in pattern.split('/') {
        match segment {
            "" => return Err(invalid("has an empty segment")),
            "." | ".." => return Err(invalid("has a `.` or `..` segment")),
            "**" => {}
            other if other.contains("**") => {
                return Err(invalid("has `**` within a segment"));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Whether the pattern's segments match the path's.
fn path_matches(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        // A trailing `**` matches what is inside a directory, never the directory itself.
        Some((&"**", [])) => !path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| path_matches(rest, &path[skip..])),
        Some((first, rest)) => match path.split_first() {
            Some((segment, others)) => {
                segment_matches(first, segment) && path_matches(rest, others)
            }
            None => false,
        },
    }
}

/// Whether one pattern segment matches one path segment (`?` takes one character).
fn segment_matches(pattern: &str, text: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    chars_match(&pattern, &text)
}

fn chars_match(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| chars_match(rest, &text[skip..])),
        Some(('?', rest)) => !text.is_empty() && chars_match(rest, &text[1..]),
        Some((c, rest)) => text.first() == Some(c) && chars_match(rest, &text[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> TestPaths {
        TestPaths::default()
    }

    #[test]
    fn the_defaults_match_test_files_and_no_implementation() {
        for path in [
            "tests/regression.txt",
            "tests/a/b.rs",
            "crates/x/tests/it.rs",
            "test/x.py",
            "pkg/test/x.py",
            "src/parser_test.go",
            "parser_test.go",
            "test_parser.py",
            "pkg/test_parser.py",
            "web/app.test.ts",
            "web/app.spec.js",
        ] {
            assert!(defaults().matches(Path::new(path)), "{path} is a test path");
        }
        for path in [
            "check.txt",
            "src/lib.rs",
            "tests",
            "src/testsuite/x.rs",
            "contest/x.rs",
            "Tests/x.rs",
            "src/parser_test",
            "latest.txt",
        ] {
            assert!(!defaults().matches(Path::new(path)), "{path} is not");
        }
    }

    #[test]
    fn a_pattern_is_matched_segment_by_segment() {
        let paths = TestPaths::new(["spec/*.rb", "a/**/z", "?.t"]).expect("patterns");
        assert!(paths.matches(Path::new("spec/x.rb")));
        assert!(!paths.matches(Path::new("spec/sub/x.rb")));
        assert!(paths.matches(Path::new("a/z")));
        let inside = TestPaths::new(["spec/**"]).expect("a pattern");
        assert!(inside.matches(Path::new("spec/x")));
        assert!(
            !inside.matches(Path::new("spec")),
            "a trailing ** needs a segment"
        );
        assert!(paths.matches(Path::new("a/b/c/z")));
        assert!(paths.matches(Path::new("x.t")));
        assert!(!paths.matches(Path::new("xy.t")));
    }

    #[test]
    fn a_list_that_is_not_one_of_patterns_is_refused() {
        assert_eq!(
            TestPaths::new(Vec::<String>::new()),
            Err(TestPathsError::Empty)
        );
        for pattern in ["", "/tests/**", "tests//x", "../x", "./x", "a**/x", "x\0"] {
            assert!(
                matches!(
                    TestPaths::new([pattern]),
                    Err(TestPathsError::Invalid { .. })
                ),
                "`{pattern}`"
            );
        }
    }
}
