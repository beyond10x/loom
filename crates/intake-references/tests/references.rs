use intake_references::{ExtractedReference, ReferenceKind, references};

fn found(intent: &str) -> Vec<(ReferenceKind, String)> {
    references(intent)
        .into_iter()
        .map(|ExtractedReference { kind, value }| (kind, value))
        .collect()
}

#[test]
fn intent_references_are_extracted_in_order() {
    let intent = "fix DEV-630, see https://acme.atlassian.net/browse/DEV-630 and \
https://gitlab.example.com/team/app/-/merge_requests/42 and the thread \
https://example.slack.com/archives/C01/p1700000000000100, also https://acme.atlassian.net/browse/OPS-7, \
https://gitlab.example.com/team/app/-/issues/9, https://github.com/beyond10x/intake/pull/3 and \
https://example.org/design.pdf";

    let expected = vec![
        (ReferenceKind::JiraIssue, "DEV-630".to_owned()),
        (ReferenceKind::GitlabMergeRequest, "team/app!42".to_owned()),
        (
            ReferenceKind::SlackMessage,
            "C01/1700000000.000100".to_owned(),
        ),
        (ReferenceKind::JiraIssue, "OPS-7".to_owned()),
        (ReferenceKind::GitlabIssue, "team/app#9".to_owned()),
        (
            ReferenceKind::GithubPullRequest,
            "beyond10x/intake#3".to_owned(),
        ),
        (
            ReferenceKind::Url,
            "https://example.org/design.pdf".to_owned(),
        ),
    ];

    assert_eq!(found(intent), expected);
}

#[test]
fn lower_case_keys_and_email_addresses_yield_nothing() {
    assert_eq!(found("fix dev-630 please"), Vec::new());
    assert_eq!(found("write to ops.team@example.org about it"), Vec::new());
}

#[test]
fn github_issue_urls_are_extracted() {
    assert_eq!(
        found("see https://github.com/beyond10x/intake/issues/5."),
        vec![(ReferenceKind::GithubIssue, "beyond10x/intake#5".to_owned())]
    );
}

#[test]
fn a_project_part_needs_two_letters_before_any_digit() {
    assert_eq!(found("use X-1 or A1-5 bolts"), Vec::new());
}

#[test]
fn a_longer_dashed_identifier_is_not_cut_into_a_key() {
    assert_eq!(found("ticket REQ-2024-1234 is closed"), Vec::new());
}

#[test]
fn key_shaped_email_local_parts_yield_nothing() {
    assert_eq!(found("write to DEV-630@example.org"), Vec::new());
}
