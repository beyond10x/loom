//! Deterministic extraction of an intent's key references (story intent-references).
//!
//! [`references`] finds, in text order and once each: Jira issue keys (bare, such as `DEV-630`,
//! and inside Jira `/browse/` URLs), Slack message permalinks, GitLab issue and merge-request
//! URLs, GitHub issue and pull-request URLs, and any other `http`/`https` URL. Extraction is
//! pattern matching only: no model call, no network.
//!
//! The rules:
//!
//! - A key's project part has at least two letters before any digit. It is not glued to a word
//!   on either side: a letter, digit or combining mark next to it glues it, and so does an `_`
//!   with a word character beyond it (so `foo_PR-1` is not a key, but the italic `_DEV-630_` is).
//!   Letters of the scripts written without spaces (listed below) may border a key. A key is not
//!   followed by `-<digit>` or `.<digit>`, so neither `CVE-2024-1234` nor `MPL-2.0` is cut into
//!   a key, and its project part is not one of a fixed list of standard names (`UTF-8`,
//!   `ISO-8601`, `SHA-256`, `HTTP-2`, ...).
//! - A URL's scheme is case-insensitive and it needs a host. It ends at white space, a quote,
//!   `<`, `>`, `]`, `|`, `*`, CJK or fullwidth punctuation (`。`, `、`, `）`, ...), and at the first
//!   character of a script written without spaces. Trailing sentence punctuation and `_` are
//!   dropped; a closing `)` or `}` is dropped only when it has no opening partner inside the URL.
//! - Values are canonical: the scheme and host of a URL, and a GitHub owner and repository, are
//!   lower-cased, so the same reference written in different case is found once.
//!
//! Scripts written without spaces: Thai, Lao, Myanmar, Khmer, Hangul, Hiragana, Katakana and the
//! CJK ideographs. Text in them may run straight into a reference, which is why they may border a
//! key and why they end a URL. The limit that follows: an internationalised URL (IRI) whose path
//! contains characters of these scripts is cut at the first such character; percent-encoded, it
//! is found whole.
//!
//! The types mirror `intake.routing.ReferenceKind` and `intake.routing.ExtractedReference` in
//! `ess/intake/domains/routing.yaml`. An [`ExtractedReference`] here carries only the specification's
//! `kind` and `value`. Its `reference_id` and `intent_id` are not assigned by this crate: an
//! extraction is a pure function of the text, and the identity is given where the intent and its
//! references are recorded.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};

/// What a first-level extractor recognises in an intent (`intake.routing.ReferenceKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    JiraIssue,
    SlackMessage,
    GitlabIssue,
    GitlabMergeRequest,
    GithubIssue,
    GithubPullRequest,
    Url,
}

/// A key element found in an intent's text, with its canonical value
/// (`intake.routing.ExtractedReference`, without its identity; see the crate docs).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtractedReference {
    pub kind: ReferenceKind,
    pub value: String,
}

/// Project parts that name a standard, not a tracker project.
const STANDARD_NAMES: &[&str] = &[
    "AES", "COVID", "CVE", "CWE", "ECMA", "GMT", "HTTP", "IEC", "IEEE", "ISO", "MD", "PEP", "RFC",
    "RSA", "SHA", "SSL", "TLS", "UCS", "UTC", "UTF",
];

/// Unicode blocks of the scripts written without spaces between words. They may border a key and
/// they end a URL.
const UNSPACED_SCRIPTS: &[(char, char)] = &[
    ('\u{0E00}', '\u{0E7F}'), // Thai
    ('\u{0E80}', '\u{0EFF}'), // Lao
    ('\u{1000}', '\u{109F}'), // Myanmar
    ('\u{1100}', '\u{11FF}'), // Hangul Jamo
    ('\u{1780}', '\u{17FF}'), // Khmer
    ('\u{19E0}', '\u{19FF}'), // Khmer Symbols
    ('\u{3040}', '\u{30FF}'), // Hiragana, Katakana
    ('\u{3400}', '\u{4DBF}'), // CJK Extension A
    ('\u{4E00}', '\u{9FFF}'), // CJK Unified Ideographs
    ('\u{A9E0}', '\u{A9FF}'), // Myanmar Extended-B
    ('\u{AA60}', '\u{AA7F}'), // Myanmar Extended-A
    ('\u{AC00}', '\u{D7AF}'), // Hangul Syllables
    ('\u{F900}', '\u{FAFF}'), // CJK Compatibility Ideographs
    ('\u{FF66}', '\u{FF9F}'), // Halfwidth Katakana
];

/// CJK and fullwidth punctuation blocks; they end a URL.
const WIDE_PUNCTUATION: &[(char, char)] = &[
    ('\u{3000}', '\u{303F}'), // CJK Symbols and Punctuation
    ('\u{FE10}', '\u{FE1F}'), // Vertical Forms
    ('\u{FE30}', '\u{FE4F}'), // CJK Compatibility Forms
    ('\u{FF00}', '\u{FF65}'), // Fullwidth forms and halfwidth CJK punctuation
];

/// One pass over the text: a URL, or a key candidate. A key inside a URL is consumed with the URL.
/// Key boundaries are checked in code, see [`bare_key`].
static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    let ends: String = UNSPACED_SCRIPTS
        .iter()
        .chain(WIDE_PUNCTUATION)
        .map(|(first, last)| format!("\\x{{{:X}}}-\\x{{{:X}}}", *first as u32, *last as u32))
        .collect();
    regex(&format!(
        r#"(?P<url>(?i-u:https?)://[^\s<>"'`\]|*{ends}]+)|(?P<key>[A-Z]{{2}}[A-Z0-9]*-[0-9]+)"#
    ))
});

static MARK: LazyLock<Regex> = LazyLock::new(|| regex(r"^\p{M}$"));

static GITHUB: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^https?://(?:www\.)?github\.com/([^/?#]+/[^/?#]+)/(pull|issues)/([0-9]+)(?:[/?#].*)?$")
});

static GITLAB: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^https?://[^/?#]+/([^?#]+?)/-/(merge_requests|issues)/([0-9]+)(?:[/?#].*)?$")
});

static SLACK: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^https?://(?:[^/?#]+\.)?slack\.com/archives/([A-Z0-9]+)/p([0-9]{10})([0-9]{6})/?(?:[?#].*)?$",
    )
});

static JIRA: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^https?://[^/?#]+(?:/[^?#]*)?/browse/([A-Z]{2}[A-Z0-9]*-[0-9]+)/?(?:[?#].*)?$")
});

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("built-in reference pattern compiles")
}

/// Extracts the key references of an intent, deduplicated and in the order they appear.
pub fn references(intent: &str) -> Vec<ExtractedReference> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for token in TOKEN.captures_iter(intent) {
        let reference = if let Some(url) = token.name("url") {
            canonical_url(trim_url(url.as_str())).map(|url| classify_url(&url))
        } else if let Some(key) = token.name("key") {
            bare_key(intent, key.start(), key.end())
        } else {
            None
        };
        let Some(reference) = reference else { continue };
        if seen.insert(reference.clone()) {
            found.push(reference);
        }
    }
    found
}

/// Drops sentence punctuation and `_` a URL picked up from the surrounding text, and a closing
/// `)` or `}` that has no opening partner inside the URL.
fn trim_url(mut url: &str) -> &str {
    while let Some(last) = url.chars().next_back() {
        let strip = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '_' => true,
            ')' => unbalanced(url, '(', ')'),
            '}' => unbalanced(url, '{', '}'),
            _ => false,
        };
        if !strip {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    url
}

fn unbalanced(url: &str, open: char, close: char) -> bool {
    url.matches(close).count() > url.matches(open).count()
}

/// Lower-cases the scheme and host; `None` when there is no host.
fn canonical_url(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let (userinfo, host) = match authority.rfind('@') {
        Some(at) => authority.split_at(at + 1),
        None => ("", authority),
    };
    if !host.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(format!(
        "{}://{userinfo}{}{tail}",
        scheme.to_ascii_lowercase(),
        host.to_lowercase()
    ))
}

/// A key candidate counts unless it is glued to a word or an address, is the head of a longer
/// `X-1-2` or `X-1.2` identifier, or names a standard.
fn bare_key(text: &str, start: usize, end: usize) -> Option<ExtractedReference> {
    let key = &text[start..end];
    let mut before = text[..start].chars().rev();
    let mut after = text[end..].chars();
    let (previous, next) = (before.next(), after.next());
    let beyond_next = after.next();
    if previous.is_some_and(|c| matches!(c, '@' | '.' | '-'))
        || glued(previous, before.next())
        || next == Some('@')
        || glued(next, beyond_next)
        || (matches!(next, Some('-' | '.')) && beyond_next.is_some_and(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let project = key.split_once('-').map_or(key, |(project, _)| project);
    if STANDARD_NAMES.contains(&project) {
        return None;
    }
    Some(reference(ReferenceKind::JiraIssue, key))
}

/// Whether the neighbour of a key glues it to a word: a word character does, and an `_` does when
/// a word character lies beyond it.
fn glued(neighbour: Option<char>, beyond: Option<char>) -> bool {
    match neighbour {
        Some('_') => beyond.is_some_and(word_character),
        Some(c) => word_character(c),
        None => false,
    }
}

/// A letter, digit or combining mark of a script that separates words with spaces.
fn word_character(c: char) -> bool {
    (c.is_alphanumeric() || MARK.is_match(c.encode_utf8(&mut [0; 4]))) && !unspaced_script(c)
}

fn unspaced_script(c: char) -> bool {
    UNSPACED_SCRIPTS
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&c))
}

fn classify_url(url: &str) -> ExtractedReference {
    if let Some(c) = GITHUB.captures(url) {
        let kind = if &c[2] == "pull" {
            ReferenceKind::GithubPullRequest
        } else {
            ReferenceKind::GithubIssue
        };
        return reference(kind, &format!("{}#{}", c[1].to_lowercase(), &c[3]));
    }
    if let Some(c) = GITLAB.captures(url) {
        return gitlab(&c);
    }
    if let Some(c) = SLACK.captures(url) {
        return reference(
            ReferenceKind::SlackMessage,
            &format!("{}/{}.{}", &c[1], &c[2], &c[3]),
        );
    }
    if let Some(c) = JIRA.captures(url) {
        return reference(ReferenceKind::JiraIssue, &c[1]);
    }
    reference(ReferenceKind::Url, url)
}

fn gitlab(c: &Captures<'_>) -> ExtractedReference {
    if &c[2] == "merge_requests" {
        reference(
            ReferenceKind::GitlabMergeRequest,
            &format!("{}!{}", &c[1], &c[3]),
        )
    } else {
        reference(ReferenceKind::GitlabIssue, &format!("{}#{}", &c[1], &c[3]))
    }
}

fn reference(kind: ReferenceKind, value: &str) -> ExtractedReference {
    ExtractedReference {
        kind,
        value: value.to_owned(),
    }
}
