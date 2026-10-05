//! Adversary pass 1 on `story:harness-module-map`: stricter readings of the story's Scope that
//! `harness_map.rs` does not check. Each case names the mutant of `docs/design/harness-map.md` it
//! kills and `harness_map_covers_every_crate` misses.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const HARNESS_LICENCE: &str = concat!("LicenseRef-", "B10x-Proprietary");
const LOOM_LICENCE: &str = "Apache-2.0";
const HARNESS_REVISION: &str = "798325f0";
const NONE_CELL: &str = "—";

fn design_doc(name: &str) -> String {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset; run this test through cargo test");
    let path = PathBuf::from(manifest).join("../../docs/design").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn section<'a>(doc: &'a str, heading: &str) -> Vec<&'a str> {
    let mut lines = doc.lines();
    lines
        .by_ref()
        .find(|line| line.trim() == format!("## {heading}"))
        .unwrap_or_else(|| panic!("no section `## {heading}`"));
    lines.take_while(|line| !line.starts_with("## ")).collect()
}

fn cells(line: &str) -> Vec<String> {
    let inner = line
        .trim()
        .strip_prefix('|')
        .and_then(|l| l.strip_suffix('|'))
        .unwrap_or_else(|| panic!("not a table row: {line}"));
    inner
        .split('|')
        .map(|cell| cell.replace('`', "").trim().to_owned())
        .collect()
}

/// Every table-looking line of § Map, with its index in the section.
fn pipe_lines<'a>(lines: &[&'a str]) -> Vec<(usize, &'a str)> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with('|'))
        .map(|(i, line)| (i, *line))
        .collect()
}

fn rows(doc: &str) -> Vec<BTreeMap<String, String>> {
    let lines = section(doc, "Map");
    let table = pipe_lines(&lines);
    let header: Vec<String> = cells(table[0].1)
        .into_iter()
        .map(|c| c.to_lowercase())
        .collect();
    table[2..]
        .iter()
        .map(|(_, line)| header.iter().cloned().zip(cells(line)).collect())
        .collect()
}

fn owns() -> BTreeSet<String> {
    section(&design_doc("loom-design.md"), "Owns")
        .iter()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|item| item.trim().trim_end_matches([';', '.']).trim().to_owned())
        .collect()
}

/// Mutant killed: a 15th row (or any row) placed after a blank line inside § Map. The unit's parser
/// stops at the first non-`|` line, so the stray row is rendered on the page and never checked.
#[test]
fn adversary_map_section_holds_one_contiguous_table() {
    let doc = design_doc("harness-map.md");
    let lines = section(&doc, "Map");
    let table = pipe_lines(&lines);
    assert!(!table.is_empty(), "§ Map holds no table");
    for pair in table.windows(2) {
        assert_eq!(
            pair[1].0,
            pair[0].0 + 1,
            "§ Map has a table line outside the map table, which harness_map.rs never reads: {}",
            pair[1].1
        );
    }
}

/// Mutant killed: licence cell reversed (`Apache-2.0 → LicenseRef-…`), or naming both licences in
/// any other relation. Story § Scope: the licence the source carries today, then the one it is
/// carried under in Loom.
#[test]
fn adversary_port_licence_runs_from_harness_to_loom() {
    let expected = format!("{HARNESS_LICENCE} → {LOOM_LICENCE}");
    for row in rows(&design_doc("harness-map.md")) {
        if row["disposition"] == "port" {
            assert_eq!(
                row["licence"], expected,
                "{}: licence direction",
                row["crate"]
            );
        }
    }
}

/// Mutants killed: a port target that merely contains `crates/loom-executor/` (`not crates/loom-executor/`), and two
/// crates ported onto the same Loom module.
#[test]
fn adversary_port_targets_are_distinct_loom_paths() {
    let mut seen = BTreeMap::<String, String>::new();
    for row in rows(&design_doc("harness-map.md")) {
        if row["disposition"] != "port" {
            continue;
        }
        let target = row["loom target"]
            .split([',', ' '])
            .next()
            .unwrap_or_default()
            .to_owned();
        assert!(
            target.starts_with("crates/loom-executor/src/") && !target.contains(".."),
            "{}: port target `{}` is not a path under crates/loom-executor/src/",
            row["crate"],
            row["loom target"]
        );
        if let Some(other) = seen.insert(target.clone(), row["crate"].clone()) {
            panic!(
                "{} and {other} are both ported onto `{target}`",
                row["crate"]
            );
        }
    }
}

/// Mutants killed: a not-carried row that also names a Loom target or licence, and an owner cell
/// that only contains `beyond10x/…` somewhere (`not beyond10x/harness`).
#[test]
fn adversary_not_carried_rows_name_only_their_owner() {
    for row in rows(&design_doc("harness-map.md")) {
        if row["disposition"] != "not carried" {
            continue;
        }
        let krate = &row["crate"];
        assert_eq!(
            row["loom target"], NONE_CELL,
            "{krate}: not carried, yet a target"
        );
        assert_eq!(
            row["licence"], NONE_CELL,
            "{krate}: not carried, yet a licence"
        );
        let repo = row["owner instead"].strip_prefix("beyond10x/");
        assert!(
            repo.is_some_and(|r| !r.is_empty()
                && r.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')),
            "{krate}: owner instead `{}` is not exactly one beyond10x repository",
            row["owner instead"]
        );
    }
}

/// One disposition per crate (story § Outcome). The doc's own legend gives "owner instead" to
/// `not carried` only; a `port` row that also names an owner instead is a crate in two
/// dispositions, which `harness_map.rs` cannot tell from a whole port.
#[test]
fn adversary_port_rows_name_no_owner_instead() {
    for row in rows(&design_doc("harness-map.md")) {
        if row["disposition"] == "port" {
            assert_eq!(
                row["owner instead"], NONE_CELL,
                "{}: a port row that also names an owner instead",
                row["crate"]
            );
        }
    }
}

/// Mutant killed: a `depend` row with no pinned revision. Story § Scope defines `depend` as a git
/// dependency at a pinned revision; `harness_map.rs` checks nothing on a depend row.
#[test]
fn adversary_depend_rows_name_the_pinned_revision() {
    for row in rows(&design_doc("harness-map.md")) {
        if row["disposition"] == "depend" {
            let text: String = row.values().cloned().collect::<Vec<_>>().join(" ");
            assert!(
                text.contains(HARNESS_REVISION),
                "{}: a depend row names no pinned revision",
                row["crate"]
            );
        }
    }
}

/// Mutant killed: a § Owns responsibility named only by a `not carried` row. It is then neither
/// supplied by anything Loom takes nor listed under "New in Loom", and `harness_map.rs` counts it
/// as covered.
#[test]
fn adversary_every_owned_responsibility_reaches_loom() {
    let doc = design_doc("harness-map.md");
    let mut reached = BTreeSet::new();
    for row in rows(&doc) {
        if row["disposition"] == "not carried" || row["serves"] == "none" {
            continue;
        }
        for item in row["serves"].split(';') {
            reached.insert(item.trim().to_owned());
        }
    }
    let new_in_loom: BTreeSet<String> = section(&doc, "New in Loom")
        .iter()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|item| item.split(" — ").next().unwrap_or(item).trim().to_owned())
        .collect();
    for item in owns() {
        assert!(
            reached.contains(&item) || new_in_loom.contains(&item),
            "§ Owns `{item}` is served only by a not-carried row and is not New in Loom"
        );
    }
}
