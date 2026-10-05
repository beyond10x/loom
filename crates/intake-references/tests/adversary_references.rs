//! Adversary cases for story intent-references: false positives, false negatives,
//! canonicalisation and input size.

use std::time::{Duration, Instant};

use intake_references::{ExtractedReference, ReferenceKind, references};

fn found(intent: &str) -> Vec<(ReferenceKind, String)> {
    references(intent)
        .into_iter()
        .map(|ExtractedReference { kind, value }| (kind, value))
        .collect()
}

fn one(kind: ReferenceKind, value: &str) -> Vec<(ReferenceKind, String)> {
    vec![(kind, value.to_owned())]
}

// ---- false positives -------------------------------------------------------------------------

#[test]
fn a_cve_identifier_is_not_truncated_into_a_jira_key() {
    assert_eq!(found("patch CVE-2024-1234 before Friday"), Vec::new());
}

#[test]
fn standard_names_are_not_jira_keys() {
    for text in [
        "decode it as UTF-8",
        "timestamps are ISO-8601",
        "hash with SHA-256",
        "serve HTTP-2",
    ] {
        assert_eq!(found(text), Vec::new(), "{text}");
    }
}

#[test]
fn key_shapes_inside_words_are_not_keys() {
    for text in [
        "see xPR-1 now",
        "see foo_PR-1 now",
        "see ÄPR-1 now",
        "see PR-1x now",
    ] {
        assert_eq!(found(text), Vec::new(), "{text}");
    }
}

#[test]
fn a_key_after_an_at_sign_is_not_a_key() {
    assert_eq!(found("mail ops@DEV-1 about it"), Vec::new());
}

#[test]
fn a_bare_scheme_is_not_a_url() {
    assert_eq!(found("every link must start with https://..."), Vec::new());
}

// ---- trailing punctuation and wrappers ------------------------------------------------------

#[test]
fn sentence_punctuation_and_brackets_after_a_url_are_dropped() {
    let pr = one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3");
    for text in [
        "see https://github.com/beyond10x/intake/pull/3.",
        "see https://github.com/beyond10x/intake/pull/3, then",
        "(see https://github.com/beyond10x/intake/pull/3)",
        "<https://github.com/beyond10x/intake/pull/3>",
        "[see https://github.com/beyond10x/intake/pull/3]",
        "\"https://github.com/beyond10x/intake/pull/3\"",
        "'https://github.com/beyond10x/intake/pull/3'",
        "[the PR](https://github.com/beyond10x/intake/pull/3)",
    ] {
        assert_eq!(found(text), pr, "{text}");
    }
}

#[test]
fn a_url_with_balanced_parentheses_keeps_its_closing_parenthesis() {
    assert_eq!(
        found("background: https://en.wikipedia.org/wiki/Rust_(programming_language) for context"),
        one(
            ReferenceKind::Url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        )
    );
}

#[test]
fn a_markdown_link_whose_label_is_its_url_yields_the_reference_once() {
    assert_eq!(
        found(
            "[https://github.com/beyond10x/intake/pull/3](https://github.com/beyond10x/intake/pull/3)"
        ),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

#[test]
fn markdown_emphasis_around_a_url_is_not_part_of_it() {
    assert_eq!(
        found("**https://github.com/beyond10x/intake/pull/3**"),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

#[test]
fn a_slack_formatted_link_with_a_label_yields_the_reference() {
    assert_eq!(
        found("<https://github.com/beyond10x/intake/pull/3|PR 3>"),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

#[test]
fn query_strings_and_fragments_stay_on_a_plain_url() {
    assert_eq!(
        found("see https://example.org/a?b=1&c=2#part."),
        one(ReferenceKind::Url, "https://example.org/a?b=1&c=2#part")
    );
}

// ---- false negatives -------------------------------------------------------------------------

#[test]
fn a_key_followed_by_a_colon_is_found() {
    assert_eq!(
        found("DEV-630: login fails"),
        one(ReferenceKind::JiraIssue, "DEV-630")
    );
}

#[test]
fn a_key_with_digits_in_its_project_is_found() {
    assert_eq!(found("see AB2-12"), one(ReferenceKind::JiraIssue, "AB2-12"));
}

#[test]
fn keys_separated_by_a_slash_are_both_found() {
    assert_eq!(
        found("fix DEV-1/DEV-2 together"),
        vec![
            (ReferenceKind::JiraIssue, "DEV-1".to_owned()),
            (ReferenceKind::JiraIssue, "DEV-2".to_owned()),
        ]
    );
}

#[test]
fn a_key_followed_by_cjk_text_is_found() {
    assert_eq!(
        found("DEV-630を直してください"),
        one(ReferenceKind::JiraIssue, "DEV-630")
    );
}

#[test]
fn a_slack_thread_reply_permalink_is_a_slack_message() {
    assert_eq!(
        found(
            "https://example.slack.com/archives/C01/p1700000000000100?thread_ts=1699999999.000050&cid=C01"
        ),
        one(ReferenceKind::SlackMessage, "C01/1700000000.000100")
    );
}

#[test]
fn gitlab_nested_groups_are_kept_in_the_value() {
    assert_eq!(
        found("https://gitlab.example.com/a/b/c/-/merge_requests/1/diffs"),
        one(ReferenceKind::GitlabMergeRequest, "a/b/c!1")
    );
}

#[test]
fn github_trailing_paths_and_fragments_keep_the_kind() {
    assert_eq!(
        found(
            "https://github.com/beyond10x/intake/pull/3/files and \
https://github.com/beyond10x/intake/issues/5#issuecomment-1"
        ),
        vec![
            (
                ReferenceKind::GithubPullRequest,
                "beyond10x/intake#3".to_owned()
            ),
            (ReferenceKind::GithubIssue, "beyond10x/intake#5".to_owned()),
        ]
    );
}

#[test]
fn a_jira_url_with_a_query_yields_its_key() {
    assert_eq!(
        found("https://acme.atlassian.net/browse/DEV-1?focusedCommentId=7"),
        one(ReferenceKind::JiraIssue, "DEV-1")
    );
}

#[test]
fn an_upper_case_scheme_is_a_url() {
    assert_eq!(
        found("see HTTPS://GITHUB.COM/beyond10x/intake/pull/3"),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

// ---- canonicalisation ------------------------------------------------------------------------

#[test]
fn a_github_reference_dedups_across_scheme_and_host_case() {
    assert_eq!(
        found(
            "http://github.com/beyond10x/intake/pull/3 https://GitHub.com/beyond10x/intake/pull/3 \
https://www.github.com/beyond10x/intake/pull/3"
        ),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

#[test]
fn a_jira_key_dedups_between_url_and_bare_form_in_either_order() {
    assert_eq!(
        found("https://acme.atlassian.net/browse/DEV-630 then DEV-630"),
        one(ReferenceKind::JiraIssue, "DEV-630")
    );
}

#[test]
fn a_github_repository_dedups_across_owner_and_name_case() {
    assert_eq!(
        found(
            "https://github.com/Beyond10x/Intake/pull/3 and https://github.com/beyond10x/intake/pull/3"
        ),
        one(ReferenceKind::GithubPullRequest, "beyond10x/intake#3")
    );
}

#[test]
fn a_plain_url_dedups_across_host_case() {
    assert_eq!(
        found("https://Example.org/design.pdf and https://example.org/design.pdf"),
        one(ReferenceKind::Url, "https://example.org/design.pdf")
    );
}

// ---- input size ------------------------------------------------------------------------------

#[test]
fn a_mebibyte_of_unicode_text_is_extracted_quickly() {
    let chunk = "Grüße 日本語のテキスト mit Umlauten DEV-1 und https://example.org/x ";
    let mut text = String::new();
    while text.len() < 1 << 20 {
        text.push_str(chunk);
    }
    let started = Instant::now();
    let refs = found(&text);
    let elapsed = started.elapsed();
    eprintln!("1 MiB unicode: {elapsed:?}");
    assert_eq!(
        refs,
        vec![
            (ReferenceKind::JiraIssue, "DEV-1".to_owned()),
            (ReferenceKind::Url, "https://example.org/x".to_owned()),
        ]
    );
    // 30 s, not 5: a debug build under machine load took 5.33 s (wave 2026-10-04-w16); linear time is what this guards, release takes about 0.1 s.
    assert!(elapsed < Duration::from_secs(30), "took {elapsed:?}");
}

#[test]
fn one_very_long_url_is_extracted_quickly() {
    let mut text = String::from("https://gitlab.example.com/");
    while text.len() < 1 << 20 {
        text.push_str("a/");
    }
    text.push_str("x/-/issues/9x");
    let started = Instant::now();
    let refs = found(&text);
    let elapsed = started.elapsed();
    eprintln!("1 MiB url: {elapsed:?}");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].0, ReferenceKind::Url);
    assert!(elapsed < Duration::from_secs(30), "took {elapsed:?}");
}
