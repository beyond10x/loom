//! Adversary case for `story:docs-agent-stack-layers`: no current documentation page presents
//! Loom as a harness Metaharness compares (Atlas ADR 0096, which superseded ADR 0071; ADR 0098).
//! A dated record under `docs/qualification` or `docs/research` is history and is not read.

use std::path::{Path, PathBuf};

/// Phrases, whitespace-normalized, that place Loom among the harnesses Metaharness compares.
const FORBIDDEN: [&str; 5] = [
    "with Loom as the native one",
    "Loom is the native executor Metaharness compares",
    "the native executor it compares",
    "Metaharness compares Loom",
    "compares Loom with other harnesses",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pages(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            pages(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "md" || ext == "mdx")
        {
            out.push(path);
        }
    }
}

/// Phrases that put Metaharness above Loom: comparing Loom's selectors with harnesses, or reading
/// Loom's records, when metaharness is L2 below Loom and never depends on it (ADR 0096 Decision
/// 1; ADR 0098 Decision 3). Trials, not Metaharness, read Loom's decision records (ADR 0101).
const ABOVE_LOOM: [&str; 3] = [
    "Loom + reasoning selector external coding harness",
    "Metaharness reads the records through `Loom::selection_records`",
    "Selection telemetry is available to **Metaharness** for evaluation",
];

fn found_in_pages(phrases: &[&str]) -> Vec<String> {
    let root = root();
    let mut files = vec![root.join("README.md"), root.join("AGENTS.md")];
    pages(&root.join("website/docs"), &mut files);
    pages(&root.join("docs/design"), &mut files);
    pages(&root.join("docs/contracts"), &mut files);
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a readable page");
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in phrases {
            if flat.contains(phrase) {
                found.push(format!(
                    "{}: {phrase:?}",
                    file.strip_prefix(&root).unwrap_or(file).display()
                ));
            }
        }
    }
    found
}

#[test]
fn adv_q13_no_page_puts_metaharness_above_loom() {
    let found = found_in_pages(&ABOVE_LOOM);
    assert!(
        found.is_empty(),
        "pages still put Metaharness above Loom (ADRs 0096, 0098): {found:#?}"
    );
}

#[test]
fn adv_q13_no_page_presents_loom_as_a_compared_harness() {
    let root = root();
    let mut files = vec![root.join("README.md"), root.join("AGENTS.md")];
    pages(&root.join("website/docs"), &mut files);
    pages(&root.join("docs/design"), &mut files);
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("a readable page");
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in FORBIDDEN {
            if flat.contains(phrase) {
                found.push(format!(
                    "{}: {phrase:?}",
                    file.strip_prefix(&root).unwrap_or(file).display()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "pages still present Loom as a harness Metaharness compares (ADR 0096): {found:#?}"
    );
}
