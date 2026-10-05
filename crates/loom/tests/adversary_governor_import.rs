//! Adversary pass 1, wave 2026-10-05-w24, `story:import-governor`.
//!
//! The story's acceptance: "`epic:governor` and its story exist in Loom's store, citing their old
//! ids." The governor's store held `epic:governor` and `story:canon-governor` (governor `81fcc1e`);
//! the layout commit dropped its `.engineering/` saying "the governor's open plan is re-filed in
//! Loom's store". This case asserts the re-filing: an epic and a story in Loom's store whose `refs`
//! cite `provider: governor` with those two references.
//!
//! The store is read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/loom sits two levels below the repository root")
        .to_path_buf()
}

/// The YAML front matter of a planning artifact: the text between the first two `---` lines.
fn front_matter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

/// Each `(provider, reference)` pair in the `refs:` list of `front`.
fn refs(front: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut provider: Option<String> = None;
    let mut in_refs = false;
    for line in front.lines() {
        if !line.starts_with(' ') && !line.starts_with('-') {
            in_refs = line.trim_end() == "refs:";
            continue;
        }
        if !in_refs {
            continue;
        }
        let item = line.trim_start().trim_start_matches("- ").trim();
        if let Some(value) = item.strip_prefix("provider:") {
            provider = Some(value.trim().to_string());
        } else if let Some(value) = item.strip_prefix("reference:")
            && let Some(p) = provider.take()
        {
            out.push((p, value.trim().to_string()));
        }
    }
    out
}

/// Every artifact of `kind` (its directory under `.engineering/planning/`) that cites
/// `provider: governor` with `reference`, as a relative path.
fn citing(root: &Path, kind: &str, reference: &str) -> Vec<String> {
    let dir = root.join(".engineering/planning").join(kind);
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return found;
    };
    for entry in entries {
        let path = entry.expect("store entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read planning artifact");
        let Some(front) = front_matter(&text) else {
            continue;
        };
        if refs(front)
            .iter()
            .any(|(p, r)| p == "governor" && r == reference)
        {
            found.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }
    found
}

#[test]
fn the_parser_reads_a_provider_reference_pair() {
    let front = "id: epic:x\nkind: epic\nrefs:\n- provider: governor\n  reference: epic:governor\n- provider: atlas\n  reference: epic:ga\nrelations:\n- serves: vision:O2\n";
    assert_eq!(
        refs(front),
        vec![
            ("governor".to_string(), "epic:governor".to_string()),
            ("atlas".to_string(), "epic:ga".to_string()),
        ]
    );
}

#[test]
fn the_governor_epic_and_its_story_are_re_filed_citing_their_old_ids() {
    let root = repo_root();
    let epic = citing(&root, "epic", "epic:governor");
    let story = citing(&root, "story", "story:canon-governor");
    assert!(
        epic.len() == 1 && story.len() == 1,
        "Loom's store must hold one epic citing governor `epic:governor` and one story citing \
         governor `story:canon-governor` (story:import-governor acceptance); found epic {epic:?}, \
         story {story:?}"
    );
}
