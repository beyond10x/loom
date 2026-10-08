//! Adversary pass 1 for the engineering-protocols 0.3.0 pin.
//!
//! The pin is written in three places that nothing compares: the manifests (`tag = "…"`), the
//! lockfile, and the literal `revision` that `ProtocolCatalog::engineering` stamps on every entry
//! (`src/lib.rs`). These cases tie them together, so the next pin cannot move one and leave the
//! others, and hold the catalog to what the CHANGELOG says callers now see.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use loom_protocols::{ProtocolCatalog, SourceKind};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `tag = "…"` every workspace manifest gives `dependency`, one entry per manifest naming it.
fn manifest_tags(dependency: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let crates = std::fs::read_dir(workspace().join("crates")).expect("crates");
    for entry in crates {
        let manifest = entry.expect("entry").path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix(dependency) else {
                continue;
            };
            if !rest.trim_start().starts_with('=') {
                continue;
            }
            let reference = rest
                .split("tag = \"")
                .nth(1)
                .and_then(|tail| tail.split('"').next())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("not a tag: {line}"));
            found.push((manifest.display().to_string(), reference));
        }
    }
    assert!(!found.is_empty(), "no manifest names {dependency}");
    found
}

/// The `source` of every `[[package]]` named `name` in `Cargo.lock`.
fn locked_sources(name: &str) -> Vec<String> {
    let lock = std::fs::read_to_string(workspace().join("Cargo.lock")).expect("Cargo.lock");
    lock.split("[[package]]")
        .filter(|block| {
            block
                .lines()
                .any(|line| line.trim() == format!("name = \"{name}\""))
        })
        .map(|block| {
            block
                .lines()
                .find_map(|line| line.trim().strip_prefix("source = "))
                .unwrap_or("no source")
                .trim_matches('"')
                .to_owned()
        })
        .collect()
}

fn one_tag(dependency: &str) -> String {
    let tags = manifest_tags(dependency);
    let distinct: BTreeSet<&str> = tags.iter().map(|(_, tag)| tag.as_str()).collect();
    assert_eq!(
        distinct.len(),
        1,
        "{dependency} is named by more than one reference: {tags:?}"
    );
    distinct.into_iter().next().expect("one").to_owned()
}

#[test]
fn every_engineering_entry_carries_the_tag_the_manifest_pins() {
    let tag = one_tag("b10x-canon-engineering");
    let catalog = ProtocolCatalog::engineering().expect("the engineering catalog");
    let mut seen = 0;
    for entry in catalog.iter() {
        assert_eq!(
            entry.definition.source.kind,
            SourceKind::Engineering,
            "{}",
            entry.name()
        );
        assert_eq!(
            entry.definition.source.revision,
            tag,
            "{} says it comes from engineering-protocols {} while the manifests pin {tag}",
            entry.name(),
            entry.definition.source.revision
        );
        seen += 1;
    }
    assert!(seen > 0, "the engineering catalog is empty");
}

#[test]
fn the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags() {
    for (package, repository) in [
        ("b10x-canon-engineering", "beyond10x/engineering-protocols"),
        ("b10x-canon", "beyond10x/canon"),
    ] {
        let tag = one_tag(package);
        let sources = locked_sources(package);
        assert_eq!(sources.len(), 1, "{package}: one copy, found {sources:?}");
        let prefix = format!("git+https://github.com/{repository}?tag={tag}#");
        assert!(
            sources[0].starts_with(&prefix),
            "{package}: locked from {} and not from tag {tag}",
            sources[0]
        );
    }
}

/// The CHANGELOG (Unreleased): the engineering registry, and so `ProtocolCatalog::engineering` and
/// `::bundled`, now lists `support-triage@1`; `incident.response/1` declares `investigate_cause`.
/// The names are the release's `protocols/` directory at engineering-protocols 0.3.0.
#[test]
fn the_catalog_lists_what_the_changelog_says_callers_now_see() {
    let names = |catalog: &ProtocolCatalog| -> BTreeSet<String> {
        catalog
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect()
    };
    let engineering = ProtocolCatalog::engineering().expect("engineering");
    assert_eq!(
        names(&engineering),
        BTreeSet::from([
            "incident-response@1".to_owned(),
            "software-change@1".to_owned(),
            "support-triage@1".to_owned(),
        ])
    );
    let bundled = ProtocolCatalog::bundled().expect("bundled");
    assert!(names(&bundled).contains("support-triage@1"));
    let incident = engineering
        .get("incident-response@1")
        .expect("incident-response@1");
    assert!(
        incident.definition.yaml.contains("investigate_cause:"),
        "incident.response/1 declares investigate_cause"
    );
}
