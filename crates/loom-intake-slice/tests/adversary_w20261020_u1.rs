//! Adversary pass on `story:engineering-protocols-090` (wave 2026-10-20-w1, U1).
//!
//! `b10x-loom resume` uses a commission's kept route only when "the protocol catalog ... [is] still
//! the one it was accepted for" (`website/docs/reference/cli.md`, `b10x-loom resume`):
//! `resume_with_state` calls `PreparedIntent::restore_route` first, which compares the name and
//! digest of every catalog entry. Loom 0.26.0 kept every route on its bundled catalog, which is
//! this release's bundled catalog without `eval-gated-change@1`: engineering-protocols 0.9.0 changes
//! no existing protocol (byte-identical YAML) and `protocols/system-query/1.yaml` is untouched, so
//! every other digest is the same. This change therefore refuses the resume of every commission
//! `run --state` suspended under 0.26.0. The CHANGELOG entry that announces the new bundled protocol
//! must name that break in its `Breaking:` sentence, as README's `0.20.0` entry named the state
//! directories it stopped resuming.

use b10x_loom_intake_slice::case::{RUN_DEFAULTS, intent_revision};
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_intake_slice::intent::{IntentRequest, PreparedIntent};
use b10x_loom_intake_slice::resume::{AcceptedRoute, CatalogDigest};
use b10x_loom_intake_slice::run::RunOptions;
use b10x_loom_intake_slice::test_paths::TestPaths;
use intake_model::primitives::Decimal;
use loom_protocols::ProtocolCatalog;

const INTENT: &str = "fix the failing parser test";
const THRESHOLD: f64 = 0.5;
/// The definition this change adds to the bundled catalog.
const ADDED: &str = "eval-gated-change@1";

fn request() -> IntentRequest {
    IntentRequest {
        intent: INTENT.into(),
        workspace: None,
        runner: None,
        test: TestCommand::new("must-never-run", std::iter::empty::<&str>()),
        max_steps: 2,
        threshold: THRESHOLD,
        inputs: RUN_DEFAULTS,
        test_paths: TestPaths::default(),
        bindings: None,
    }
}

/// A `software-change@1` route accepted on `catalog`'s entries, leaving out `without`: what a
/// resume context keeps (`intake.routing.AcceptedRoute`).
fn route(catalog: &ProtocolCatalog, without: Option<&str>) -> AcceptedRoute {
    AcceptedRoute {
        protocol: "software-change@1".into(),
        intent_revision: intent_revision(INTENT),
        catalog: catalog
            .iter()
            .filter(|entry| Some(entry.name()) != without)
            .map(|entry| CatalogDigest {
                name: entry.name().to_owned(),
                digest: entry.definition.sha256.clone(),
            })
            .collect(),
        context_policy: ContextPolicy::Legacy,
        threshold: Decimal(serde_json::json!(THRESHOLD).to_string()),
    }
}

/// The paragraph that names `protocol` and carries a `Breaking:` sentence, in the newest CHANGELOG
/// section that has one: `[Unreleased]` while the change is unreleased, the release's own section
/// once a release commit moved the entries under it.
fn newest_paragraph_naming(protocol: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md");
    let text = std::fs::read_to_string(&path).expect("CHANGELOG.md");
    text.split("\n## [")
        .skip(1)
        .find_map(|section| {
            section
                .split("\n\n")
                .find(|paragraph| paragraph.contains(protocol) && paragraph.contains("Breaking:"))
        })
        .unwrap_or_else(|| {
            panic!("no CHANGELOG paragraph names {protocol} with a Breaking sentence")
        })
        .to_owned()
}

#[test]
fn adversary_a_commission_suspended_before_the_new_bundled_protocol_is_refused_by_resume_and_the_changelog_names_it()
 {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    assert!(catalog.get(ADDED).is_some(), "{ADDED} is bundled");
    let options = RunOptions::default();
    let request = request();

    // Control: a route kept on this release's catalog restores, so the construction is right.
    PreparedIntent::restore_route(&route(&catalog, None), &request, &catalog, &options)
        .expect("a route accepted on this release's bundled catalog restores");

    // The route as Loom 0.26.0 kept it: the same entries and digests, without the added one.
    let Err(refusal) =
        PreparedIntent::restore_route(&route(&catalog, Some(ADDED)), &request, &catalog, &options)
    else {
        // The upgrade does not refuse the resume: nothing breaks, so nothing has to be named.
        return;
    };

    let paragraph = newest_paragraph_naming(ADDED);
    let breaking = paragraph
        .split_once("Breaking:")
        .map(|(_, sentence)| sentence)
        .unwrap_or_default();
    assert!(
        breaking.contains("resume"),
        "a route kept before {ADDED} was bundled is refused on resume ({refusal}), and the \
         CHANGELOG's Breaking sentence for this change does not name it:\nBreaking:{breaking}"
    );
}
