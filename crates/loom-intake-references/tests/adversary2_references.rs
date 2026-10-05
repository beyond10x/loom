//! Adversary pass 2 for story intent-references: the pass-1 fixes (standard names, script-aware
//! key boundaries, parenthesis balance, URL terminators and case folding) against their own docs.

use b10x_loom_intake_references::{ExtractedReference, ReferenceKind, references};

fn found(intent: &str) -> Vec<(ReferenceKind, String)> {
    references(intent)
        .into_iter()
        .map(|ExtractedReference { kind, value }| (kind, value))
        .collect()
}

fn one(kind: ReferenceKind, value: &str) -> Vec<(ReferenceKind, String)> {
    vec![(kind, value.to_owned())]
}

fn pr3() -> Vec<(ReferenceKind, String)> {
    one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
}

// ---- standard names --------------------------------------------------------------------------

#[test]
fn a_project_that_only_starts_with_a_standard_name_is_a_key() {
    for key in ["RFCX-1", "ISO2-5", "UTFS-3", "SHAPE-9", "MDM-4"] {
        assert_eq!(
            found(&format!("see {key} today")),
            one(ReferenceKind::JiraIssue, key),
            "{key}"
        );
    }
}

#[test]
fn a_jira_url_is_authoritative_for_a_standard_like_project() {
    assert_eq!(
        found("see https://acme.atlassian.net/browse/RFC-12"),
        one(ReferenceKind::JiraIssue, "RFC-12")
    );
    assert_eq!(
        found("see https://acme.atlassian.net/browse/UTF-8 and UTF-8"),
        one(ReferenceKind::JiraIssue, "UTF-8")
    );
}

#[test]
fn a_key_followed_by_a_version_decimal_is_not_a_key() {
    for text in [
        "relicense under MPL-2.0 this week",
        "the GPL-3.0-only notice",
        "LGPL-2.1 applies",
    ] {
        assert_eq!(found(text), Vec::new(), "{text}");
    }
}

// ---- script-aware key boundaries -------------------------------------------------------------

#[test]
fn a_decomposed_letter_before_a_key_glues_it_like_the_composed_one() {
    // "ÄPR-1" written as A + U+0308 COMBINING DIAERESIS (NFD), which macOS text often is.
    assert_eq!(found("A\u{0308}PR-1"), Vec::new());
    assert_eq!(found("cafe\u{0301}DEV-1"), Vec::new());
}

#[test]
fn a_key_glued_to_a_non_ascii_letter_or_digit_is_not_a_key() {
    for text in [
        "яDEV-1",
        "λDEV-1",
        "\u{0663}DEV-1",
        "1DEV-1",
        "DEV-1\u{00B2}",
    ] {
        assert_eq!(found(text), Vec::new(), "{text}");
    }
}

#[test]
fn keys_next_to_emoji_and_symbols_are_found() {
    for (text, key) in [
        ("\u{1F680}DEV-1", "DEV-1"),
        ("DEV-2\u{1F389} shipped", "DEV-2"),
        ("\u{FF08}DEV-3\u{FF09}", "DEV-3"),
        ("DEV-4\u{200B}", "DEV-4"),
        ("課題DEV-5、", "DEV-5"),
    ] {
        assert_eq!(found(text), one(ReferenceKind::JiraIssue, key), "{text}");
    }
}

#[test]
fn a_key_bordered_by_lao_text_is_found() {
    // Lao, like Thai, is written without spaces between words; the crate docs promise such
    // scripts may border a key.
    assert_eq!(
        found("ແກ້DEV-630ດ່ວນ"),
        one(ReferenceKind::JiraIssue, "DEV-630")
    );
}

#[test]
fn slack_italic_underscores_around_a_key_do_not_glue_it() {
    assert_eq!(
        found("_DEV-630_ is blocked"),
        one(ReferenceKind::JiraIssue, "DEV-630")
    );
}

// ---- parenthesis balance and URL terminators -------------------------------------------------

#[test]
fn nested_parentheses_in_a_url_are_kept() {
    assert_eq!(
        found("(see https://en.wikipedia.org/wiki/A_(b_(c)))."),
        one(
            ReferenceKind::Url,
            "https://en.wikipedia.org/wiki/A_(b_(c))"
        )
    );
}

#[test]
fn a_url_in_parentheses_inside_a_sentence_drops_only_the_sentence_parenthesis() {
    assert_eq!(
        found("Fix it (see https://github.com/beyond10x/intake/pull/3) today."),
        pr3()
    );
    assert_eq!(
        found("(https://en.wikipedia.org/wiki/Rust_(programming_language)), then"),
        one(
            ReferenceKind::Url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        )
    );
}

#[test]
fn a_url_template_keeps_its_closing_brace() {
    assert_eq!(
        found("the endpoint https://api.example.org/v1/items/{id} returns 500"),
        one(ReferenceKind::Url, "https://api.example.org/v1/items/{id}")
    );
}

#[test]
fn cjk_sentence_punctuation_after_a_url_is_dropped() {
    for text in [
        "PRは https://github.com/beyond10x/intake/pull/3。",
        "https://github.com/beyond10x/intake/pull/3、DEV-1",
        "（https://github.com/beyond10x/intake/pull/3）",
    ] {
        assert_eq!(found(text).first(), pr3().first(), "{text}");
    }
}

#[test]
fn japanese_text_glued_after_a_url_is_not_part_of_it() {
    assert_eq!(
        found("https://github.com/beyond10x/intake/pull/3を見てください"),
        pr3()
    );
}

#[test]
fn slack_italic_underscores_around_a_url_are_not_part_of_it() {
    assert_eq!(found("_https://github.com/beyond10x/intake/pull/3_"), pr3());
}

// ---- case folding ----------------------------------------------------------------------------

#[test]
fn path_case_is_kept_outside_github_owner_and_repository() {
    assert_eq!(
        found("https://GitLab.Example.com/Team/App/-/issues/9"),
        one(ReferenceKind::GitlabIssue, "Team/App#9")
    );
    assert_eq!(
        found("https://Example.org/Docs/Design.PDF?Q=A#Part"),
        one(
            ReferenceKind::Url,
            "https://example.org/Docs/Design.PDF?Q=A#Part"
        )
    );
    assert_eq!(
        found(
            "https://GitLab.Example.com/Team/App/-/issues/9 and https://gitlab.example.com/team/app/-/issues/9"
        ),
        vec![
            (ReferenceKind::GitlabIssue, "Team/App#9".to_owned()),
            (ReferenceKind::GitlabIssue, "team/app#9".to_owned()),
        ]
    );
}
