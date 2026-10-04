//! Adversary pass 1 (wave 2026-10-05-w19, unit loom/import-commission).
//!
//! Each case reads the tree at run time through `CARGO_MANIFEST_DIR`, so the same binary can be
//! pointed at a mutated copy of the repository to show the case can fail.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    PathBuf::from(manifest).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The lines of the top-level task `name` in a Taskfile, up to the next task.
fn task_block(taskfile: &str, name: &str) -> Vec<String> {
    let header = format!("  {name}:");
    let mut lines = taskfile.lines().skip_while(|line| *line != header);
    assert!(lines.next().is_some(), "Taskfile.yml has no task `{name}`");
    lines
        .take_while(|line| line.is_empty() || line.starts_with("    "))
        .map(|line| line.trim().to_owned())
        .collect()
}

/// Commission's own tests (`ess_gate.rs` expectation 8, `checks.rs`
/// `check_runs_drift_and_no_hand_model_before_any_cargo_step`, `adversary2_executor_guard.rs`)
/// were repointed at `Taskfile.commission.yml`'s `check`, which neither the root `task check` nor
/// CI (`check.yml` runs `task check`) ever invokes. Nothing then pins that the root `check` runs
/// Commission's non-cargo-test steps. This case pins it: every Commission gate step is a step of
/// the root `check` (or the root `check` runs `commission:check`), and Commission's drift and
/// no-hand-model run before the first cargo build/lint/test step, as Commission's own check had it.
#[test]
fn adversary_root_check_runs_every_commission_gate_step() {
    let taskfile = read("Taskfile.yml");
    assert!(
        taskfile.contains("\n  commission: ./Taskfile.commission.yml\n"),
        "Taskfile.yml does not include Taskfile.commission.yml under `commission:`"
    );
    let check = task_block(&taskfile, "check");
    if check.iter().any(|line| line == "- task: commission:check") {
        return;
    }
    let steps = [
        "commission:spec",
        "commission:ess-gate",
        "commission:drift",
        "commission:no-hand-model",
        "commission:conform",
        "commission:deps-guard",
        "commission:docs-drift",
    ];
    let position = |step: &str| {
        check
            .iter()
            .position(|line| *line == format!("- task: {step}"))
    };
    let missing: Vec<_> = steps
        .iter()
        .filter(|step| position(step).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "root `task check` does not run {missing:?}:\n{}",
        check.join("\n")
    );
    let first_cargo = check
        .iter()
        .position(|line| line.starts_with("- cargo "))
        .unwrap_or_else(|| panic!("root `task check` has no cargo step:\n{}", check.join("\n")));
    for step in ["commission:drift", "commission:no-hand-model"] {
        assert!(
            position(step).unwrap() < first_cargo,
            "root `task check` runs {step} after a cargo step:\n{}",
            check.join("\n")
        );
    }
}

/// Loom's Docusaurus site sets `markdown.format: 'detect'`, which parses a `.md` file as CommonMark:
/// no `import`, no `{expression}`, no JSX. Commission's site did not set it, so its generator writes
/// MDX into `.md` files (`domain-model.md` imports `./domain-graph.json` and renders
/// `<DomainGraph data={domainGraph} />` under a `{/* … */}` marker). Loom's own generator writes its
/// MDX page as `index.mdx` and marks `.md` pages with an HTML comment. Moved into Loom's site,
/// Commission's pages must be MDX files, or the site must not parse them as CommonMark.
#[test]
fn adversary_commission_reference_pages_are_parsed_as_mdx() {
    let config = read("website/docusaurus.config.ts");
    let detect = config
        .lines()
        .any(|line| line.trim().trim_end_matches(',') == "format: 'detect'");
    if !detect {
        return;
    }
    let dir = repo_root().join("website/docs/reference/commission");
    let mut pages: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == "md" || ext == "mdx")
        })
        .collect();
    pages.sort();
    assert!(
        pages.len() >= 2,
        "{}: expected the generated Commission pages, found {pages:?}",
        dir.display()
    );
    let mut offenders = Vec::new();
    for page in &pages {
        if page.extension().is_some_and(|ext| ext == "mdx") {
            continue;
        }
        let text = fs::read_to_string(page)
            .unwrap_or_else(|error| panic!("read {}: {error}", page.display()));
        for (n, line) in text.lines().enumerate() {
            let mdx_only = line.starts_with("import ")
                || line.starts_with("export ")
                || line.contains("{/*")
                || line.trim_start().starts_with("<DomainGraph");
            if mdx_only {
                offenders.push(format!(
                    "{}:{}: {line}",
                    Path::new(page.file_name().unwrap()).display(),
                    n + 1
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "website/docusaurus.config.ts parses .md as CommonMark (format: 'detect'), and these \
         Commission .md pages carry MDX-only syntax that will render as text:\n{}",
        offenders.join("\n")
    );
}

/// Commission's code and tests cite `AGENTS.md` § Rules for rules Loom's § Rules does not carry:
/// `adversary_admission_semantics.rs:153` ("fail toward less authority, never silently broaden
/// capability") and `adversary_evidence_governor_refusal.rs:7-8` ("failing toward more explicit
/// uncertainty"). Commission's own AGENTS.md was not imported, so the citations now point at a file
/// that does not hold them.
#[test]
fn adversary_agents_rules_carry_the_rules_commission_cites() {
    let agents = read("AGENTS.md");
    let rules = agents
        .split("\n## ")
        .find(|section| section.starts_with("Rules\n"))
        .unwrap_or_else(|| panic!("AGENTS.md has no section `## Rules`"));
    let rules = rules
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let missing: Vec<_> = [
        "fail toward less authority",
        "more explicit uncertainty",
        "never silently broaden capability",
    ]
    .into_iter()
    .filter(|phrase| !rules.contains(phrase))
    .collect();
    assert!(
        missing.is_empty(),
        "AGENTS.md § Rules does not carry {missing:?}, which Commission's code cites as `AGENTS.md` \
         § Rules"
    );
}
