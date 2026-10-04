//! The Harness module map (`docs/design/harness-map.md`) covers every Harness crate and every
//! responsibility `docs/design/loom-design.md` § Owns gives Loom.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

/// Every crate under `beyond10x/harness/crates` at `798325f0` (release 0.13.3). Each one's
/// `Cargo.toml` names its package `b10x-` followed by the directory name.
const HARNESS_CRATES: [&str; 14] = [
    "harness-app-server",
    "harness-cli",
    "harness-credential",
    "harness-flow",
    "harness-http",
    "harness-loop",
    "harness-mcp",
    "harness-messages",
    "harness-responses",
    "harness-substrate",
    "harness-toolchain",
    "harness-tools",
    "harness-wire",
    "harness-xtask",
];

const HARNESS_REVISION: &str = "798325f0";

const DISPOSITIONS: [&str; 3] = ["depend", "port", "not carried"];

/// Harness's licence today. Assembled from two halves so that no file under `crates/` holds the
/// identifier itself: ported source is checked for its absence.
const HARNESS_LICENCE: &str = concat!("LicenseRef-", "B10x-Proprietary");
const LOOM_LICENCE: &str = "Apache-2.0";

/// The cell of a column a row's disposition leaves empty.
const EMPTY: &str = "—";

const COLUMNS: [&str; 8] = [
    "crate",
    "package",
    "owns today",
    "serves",
    "disposition",
    "loom target",
    "licence",
    "owner instead",
];

/// Rust keywords, strict and reserved: a module path segment cannot be one of these.
const KEYWORDS: [&str; 51] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Reads a design document of the tree this test is run from. The manifest directory is taken at
/// run time: a test binary reused from a shared target directory must not read the tree it was
/// built in.
fn design_doc(name: &str) -> String {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    let path = PathBuf::from(manifest).join("../../docs/design").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The lines of the level-two section `heading`, up to the next level-two heading. The heading must
/// appear exactly once, so no second section of the same name sits on the page unchecked.
fn section<'a>(doc: &'a str, heading: &str) -> Vec<&'a str> {
    let marker = format!("## {heading}");
    let count = doc.lines().filter(|line| line.trim() == marker).count();
    assert_eq!(count, 1, "`{marker}` appears {count} times; expected once");
    let mut lines = doc.lines();
    lines
        .by_ref()
        .find(|line| line.trim() == format!("## {heading}"))
        .unwrap_or_else(|| panic!("no section `## {heading}`"));
    lines.take_while(|line| !line.starts_with("## ")).collect()
}

/// The text of each `- ` bullet, up to an explanatory ` — ` and without closing punctuation.
fn bullets(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|item| {
            let item = item.split(" — ").next().unwrap_or(item);
            item.trim().trim_end_matches([';', '.']).trim().to_owned()
        })
        .collect()
}

fn cells(line: &str) -> Vec<String> {
    let inner = line
        .trim()
        .strip_prefix('|')
        .and_then(|l| l.strip_suffix('|'))
        .unwrap_or_else(|| panic!("not a table row: {line}"));
    inner
        .split('|')
        .map(|cell| cell.trim().replace('`', ""))
        .collect()
}

/// The rows of the map table, each keyed by lower-case column name. Every `|` line of § Map must
/// belong to that one table, so no row can sit on the page unchecked.
fn map_rows(doc: &str) -> Vec<BTreeMap<&'static str, String>> {
    let lines = section(doc, "Map");
    let table: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with('|'))
        .map(|(i, line)| (i, *line))
        .collect();
    assert!(table.len() >= 2, "the Map section has no table");
    for pair in table.windows(2) {
        assert_eq!(
            pair[1].0,
            pair[0].0 + 1,
            "§ Map has a table line outside the map table: {}",
            pair[1].1
        );
    }
    let header: Vec<String> = cells(table[0].1)
        .into_iter()
        .map(|cell| cell.to_lowercase())
        .collect();
    assert_eq!(header, COLUMNS, "map table columns");
    let rule = table[1].1;
    assert!(
        rule.contains("---"),
        "second table line is not a rule: {rule}"
    );
    table[2..]
        .iter()
        .map(|(_, line)| {
            let row = cells(line);
            assert_eq!(row.len(), COLUMNS.len(), "row width: {line}");
            COLUMNS.iter().copied().zip(row).collect()
        })
        .collect()
}

/// The responsibilities of `loom-design.md` § Owns. Every non-blank line of the section must be a
/// `- item;` bullet at column 0 (the last may close with `.`), and no item may repeat: a line in
/// any other shape would be a responsibility the coverage check never reads.
fn owns(design: &str) -> BTreeSet<String> {
    let lines = section(design, "Owns");
    let mut owns = BTreeSet::new();
    for line in lines.iter().filter(|line| !line.trim().is_empty()) {
        let item = line
            .strip_prefix("- ")
            .and_then(|item| item.strip_suffix(';').or_else(|| item.strip_suffix('.')))
            .map(str::trim)
            .filter(|item| !item.is_empty() && !item.starts_with([' ', '-', '*', '+']))
            .unwrap_or_else(|| {
                panic!("loom-design.md § Owns line `{line}` is not a `- item;` bullet")
            });
        assert!(
            owns.insert(item.to_owned()),
            "loom-design.md § Owns repeats `{item}`"
        );
    }
    assert!(!owns.is_empty(), "loom-design.md § Owns lists nothing");
    owns
}

/// The Rust module a target under `crates/loom/src/` names, as its path segments below the crate
/// root: `x/`, `x.rs` and `x/mod.rs` are the same module. `None` when the target is not such a
/// path, names the crate root (`lib.rs`, `main.rs`), or has a segment that is not a Rust
/// identifier or is a keyword.
fn loom_module(target: &str) -> Option<Vec<String>> {
    let rest = target.strip_prefix("crates/loom/src/")?;
    let rest = rest
        .strip_suffix('/')
        .or_else(|| rest.strip_suffix(".rs"))?;
    let rest = rest.strip_suffix("/mod").unwrap_or(rest);
    if rest.is_empty() || ["lib", "main", "mod"].contains(&rest) {
        return None;
    }
    let segments: Vec<String> = rest.split('/').map(str::to_owned).collect();
    segments
        .iter()
        .all(|segment| {
            segment.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && !KEYWORDS.contains(&segment.as_str())
        })
        .then_some(segments)
}

fn is_one_repository(owner: &str) -> bool {
    owner.strip_prefix("beyond10x/").is_some_and(|repo| {
        repo.starts_with(|c: char| c.is_ascii_alphanumeric())
            && repo
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    })
}

#[test]
fn harness_map_covers_every_crate() {
    let map = design_doc("harness-map.md");
    let owns = owns(&design_doc("loom-design.md"));
    let rows = map_rows(&map);

    // 1. Exactly one row per Harness crate, and no other row.
    let mut seen = BTreeMap::<String, usize>::new();
    for row in &rows {
        *seen.entry(row["crate"].clone()).or_default() += 1;
    }
    let expected: BTreeMap<String, usize> = HARNESS_CRATES
        .iter()
        .map(|c| ((*c).to_owned(), 1))
        .collect();
    assert_eq!(seen, expected, "one row per Harness crate at 798325f0");

    let port_licence = format!("{HARNESS_LICENCE} → {LOOM_LICENCE}");
    let mut modules = BTreeMap::<Vec<String>, String>::new();
    let mut reached = BTreeSet::new();
    for row in &rows {
        let krate = &row["crate"];
        assert_eq!(row["package"], format!("b10x-{krate}"), "{krate}: package");

        // 2. One disposition out of three.
        let disposition = row["disposition"].as_str();
        assert!(
            DISPOSITIONS.contains(&disposition),
            "{krate}: disposition `{disposition}` is not one of {DISPOSITIONS:?}"
        );

        // 3. A § Owns responsibility, several separated by `;`, or `none`.
        let serves = row["serves"].as_str();
        let mut served = Vec::new();
        if serves != "none" {
            for item in serves.split(';').map(str::trim) {
                assert!(
                    owns.contains(item),
                    "{krate}: `{item}` is not a § Owns responsibility of loom-design.md"
                );
                served.push(item.to_owned());
            }
        }

        // 5. Each disposition fills exactly the columns its legend gives it.
        let target = row["loom target"].as_str();
        let licence = row["licence"].as_str();
        let owner = row["owner instead"].as_str();
        match disposition {
            "port" => {
                let module = loom_module(target).unwrap_or_else(|| {
                    panic!(
                        "{krate}: port target `{target}` is not one module below the crate root \
                         under crates/loom/src/"
                    )
                });
                if let Some(other) = modules.insert(module.clone(), krate.clone()) {
                    panic!(
                        "{krate} and {other} are both ported onto the module `{}`",
                        module.join("::")
                    );
                }
                assert_eq!(licence, port_licence, "{krate}: port licence");
                assert_eq!(owner, EMPTY, "{krate}: a port row names an owner instead");
                reached.extend(served);
            }
            "depend" => {
                assert!(
                    target.contains(HARNESS_REVISION),
                    "{krate}: a depend row names no pinned revision (`{target}`)"
                );
                assert_eq!(licence, EMPTY, "{krate}: a depend row names a licence");
                assert_eq!(owner, EMPTY, "{krate}: a depend row names an owner instead");
                reached.extend(served);
            }
            _ => {
                assert_eq!(target, EMPTY, "{krate}: not carried, yet a Loom target");
                assert_eq!(licence, EMPTY, "{krate}: not carried, yet a licence");
                assert!(
                    is_one_repository(owner),
                    "{krate}: owner instead `{owner}` is not exactly one beyond10x repository"
                );
            }
        }
    }

    // No ported module sits inside another ported module.
    for (outer, outer_crate) in &modules {
        for (inner, inner_crate) in &modules {
            assert!(
                outer == inner || !inner.starts_with(outer),
                "{inner_crate} (`{}`) is ported inside {outer_crate} (`{}`)",
                inner.join("::"),
                outer.join("::")
            );
        }
    }

    // 4. Every § Owns responsibility reaches Loom through a port or depend row, or is listed under
    //    "New in Loom"; that section lists exactly the ones no such row reaches.
    let new_in_loom: BTreeSet<String> =
        bullets(&section(&map, "New in Loom")).into_iter().collect();
    for item in &new_in_loom {
        assert!(
            owns.contains(item),
            "New in Loom: `{item}` is not a § Owns responsibility of loom-design.md"
        );
    }
    let unreached: BTreeSet<String> = owns.difference(&reached).cloned().collect();
    assert_eq!(
        new_in_loom, unreached,
        "New in Loom lists exactly the § Owns responsibilities no port or depend row reaches"
    );
}
