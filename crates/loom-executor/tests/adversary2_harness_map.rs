//! Adversary pass 2 on `story:harness-module-map`. Each case names the mutant of
//! `docs/design/harness-map.md` or `docs/design/loom-design.md` it kills, which both
//! `harness_map.rs` and `adversary_harness_map.rs` let through.
//!
//! Every path is taken from `CARGO_MANIFEST_DIR` at run time, so a mutated copy of the documents can
//! be checked by pointing that variable at it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const NONE_CELL: &str = "—";

fn repo_root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset; run this test through cargo test");
    PathBuf::from(manifest).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn map_doc() -> String {
    read("docs/design/harness-map.md")
}

fn design_doc() -> String {
    read("docs/design/loom-design.md")
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

fn rows(doc: &str) -> Vec<BTreeMap<String, String>> {
    let lines = section(doc, "Map");
    let table: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.trim_start().starts_with('|'))
        .collect();
    assert!(table.len() > 2, "§ Map holds no table rows");
    let header: Vec<String> = cells(table[0])
        .into_iter()
        .map(|c| c.to_lowercase())
        .collect();
    table[2..]
        .iter()
        .map(|line| header.iter().cloned().zip(cells(line)).collect())
        .collect()
}

fn bullet_items(lines: &[&str]) -> BTreeSet<String> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|item| {
            let item = item.split(" — ").next().unwrap_or(item);
            item.trim().trim_end_matches([';', '.']).trim().to_owned()
        })
        .collect()
}

fn heading_count(doc: &str, heading: &str) -> usize {
    doc.lines()
        .filter(|line| line.trim() == format!("## {heading}"))
        .count()
}

/// The Rust module a port target names, as `::`-joined segments under the crate root: `wire/`,
/// `wire.rs` and `wire/mod.rs` are the same module. `lib.rs` and `main.rs` are the crate root, `""`.
fn module_of(target: &str) -> String {
    let rest = target
        .strip_prefix("crates/loom-executor/src/")
        .unwrap_or_else(|| panic!("`{target}` is not under crates/loom-executor/src/"));
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".rs").unwrap_or(rest);
    let rest = rest.strip_suffix("/mod").unwrap_or(rest);
    if rest == "lib" || rest == "main" || rest == "mod" {
        return String::new();
    }
    rest.replace('/', "::")
}

/// Mutants killed: a responsibility added to `loom-design.md` § Owns as `* item;`, `+ item;`, an
/// indented `  - item;` or a numbered `14. item.`, or in a second `## Owns` section further down.
/// Both existing parsers read only column-0 `- ` lines of the first § Owns, so each of these is a
/// responsibility of Loom that no row and no "New in Loom" entry has to account for, and the
/// coverage check (story acceptance 4) stays green.
#[test]
fn adversary2_every_owns_line_is_a_responsibility_the_check_reads() {
    let design = design_doc();
    assert_eq!(
        heading_count(&design, "Owns"),
        1,
        "loom-design.md has more than one `## Owns`; the map's coverage check reads only the first"
    );
    let lines = section(&design, "Owns");
    let mut read = 0;
    for line in lines.iter().filter(|line| !line.trim().is_empty()) {
        assert!(
            line.starts_with("- ") && (line.ends_with(';') || line.ends_with('.')),
            "loom-design.md § Owns line `{line}` is not a `- item;` bullet, so the map's coverage \
             check never reads it"
        );
        read += 1;
    }
    assert_eq!(read, bullet_items(&lines).len(), "§ Owns repeats an item");
}

/// Mutant killed: a second `## Map` (or `## New in Loom`) section appended to harness-map.md, with
/// a table or list nobody checks; both existing parsers stop at the first.
#[test]
fn adversary2_map_sections_appear_once() {
    let map = map_doc();
    for heading in ["Map", "New in Loom", "Seams a port reuses"] {
        assert_eq!(
            heading_count(&map, heading),
            1,
            "harness-map.md: `## {heading}` must appear exactly once"
        );
    }
}

/// Mutants killed: `harness-http` ported onto `crates/loom-executor/src/harness/wire.rs` (the module
/// `harness::wire`, which `harness-wire` already lands in as `harness/wire/`); `harness-cli` ported
/// onto `crates/loom-executor/src/harness.rs` (the parent module of every other port); any port onto
/// `crates/loom-executor/src/lib.rs` (the crate root, inside which every port sits). The existing checks
/// compare target strings, and `wire.rs` is not a string prefix of `wire/`.
#[test]
fn adversary2_port_targets_are_distinct_unnested_modules() {
    let mut modules = BTreeMap::<String, String>::new();
    for row in rows(&map_doc()) {
        if row["disposition"] != "port" {
            continue;
        }
        let krate = row["crate"].clone();
        let module = module_of(&row["loom target"]);
        assert!(
            !module.is_empty(),
            "{krate}: ported onto the crate root `{}`",
            row["loom target"]
        );
        if let Some(other) = modules.insert(module.clone(), krate.clone()) {
            panic!("{krate} and {other} are both ported onto the module `{module}`");
        }
    }
    for (outer, outer_crate) in &modules {
        for (inner, inner_crate) in &modules {
            assert!(
                outer == inner || !inner.starts_with(&format!("{outer}::")),
                "{inner_crate} (`{inner}`) is ported inside {outer_crate} (`{outer}`)"
            );
        }
    }
}

/// The story files of the planning store that depend on `story:harness-module-map`, each with the
/// scope paths it declares.
fn dependent_story_scopes() -> BTreeMap<String, Vec<String>> {
    let dir = repo_root().join(".engineering/planning/story");
    let mut scopes = BTreeMap::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.expect("story directory entry").path();
        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("story file is readable");
        let front: Vec<&str> = text
            .lines()
            .skip(1)
            .take_while(|line| *line != "---")
            .collect();
        if !front
            .iter()
            .any(|line| line.trim() == "- depends_on: story:harness-module-map")
        {
            continue;
        }
        let paths = front
            .iter()
            .skip_while(|line| **line != "scope:")
            .skip(1)
            .take_while(|line| line.starts_with(' ') || line.starts_with('-'))
            .filter_map(|line| line.trim().strip_prefix("path: "))
            .map(renamed_scope)
            .collect();
        scopes.insert(path.display().to_string(), paths);
    }
    scopes
}

/// A scope path as it reads in today's tree. Stories written before `story:crate-names` name the
/// executor's old directory `crates/loom/`; the planning store keeps them as written, so the path is
/// carried through that rename here.
fn renamed_scope(scope: &str) -> String {
    match scope.strip_prefix("crates/loom/") {
        Some(rest) => format!("crates/loom-executor/{rest}"),
        None => scope.to_owned(),
    }
}

/// Mutants killed: `harness-cli` retargeted to `crates/loom-executor/src/transcript.rs`, or `harness-wire`
/// to `crates/loom-executor/src/wire/`. Both are well-formed module paths, so the existing checks pass, yet
/// no story that acts on the map (`story:harness-loop-port` scopes `crates/loom-executor/src/harness/`,
/// `story:session-transcript-streaming` scopes `crates/loom-executor/src/session.rs`) would write there: the
/// map would send a port where no porting story lands it. The crate root `lib.rs`, which every
/// dependent story touches, does not count as a landing place.
#[test]
fn adversary2_port_targets_land_where_a_porting_story_writes() {
    let scopes = dependent_story_scopes();
    assert!(
        !scopes.is_empty(),
        "no story in the planning store depends on story:harness-module-map"
    );
    for row in rows(&map_doc()) {
        if row["disposition"] != "port" {
            continue;
        }
        let target = row["loom target"].as_str();
        let landed = scopes.values().flatten().any(|scope| {
            scope != "crates/loom-executor/src/lib.rs"
                && (target == scope || (scope.ends_with('/') && target.starts_with(scope.as_str())))
        });
        assert!(
            landed,
            "{}: port target `{target}` is in the scope of no story that depends on the map: \
             {scopes:?}",
            row["crate"]
        );
    }
}

/// Story § Scope: "Name the seams a port reuses: `ModelPort` and `ToolPort`
/// (`harness-wire/src/port.rs`), `TurnEnvironmentProvider` (`harness-loop/src/environment.rs`), the
/// approval checkpoint (`harness-loop/src/approval.rs`)". Mutant killed: the whole § Seams deleted,
/// or a seam named without its source; and a seam whose crate the map does not port, which is then
/// no seam "a port reuses".
#[test]
fn adversary2_seams_are_named_from_ported_crates() {
    let map = map_doc();
    let seams = section(&map, "Seams a port reuses").join("\n");
    for (seam, file) in [
        ("`ModelPort`", "harness-wire/src/port.rs"),
        ("`ToolPort`", "harness-wire/src/port.rs"),
        (
            "`TurnEnvironmentProvider`",
            "harness-loop/src/environment.rs",
        ),
        ("approval checkpoint", "harness-loop/src/approval.rs"),
    ] {
        let bullet = seams
            .split("\n- ")
            .find(|bullet| bullet.contains(seam))
            .unwrap_or_else(|| panic!("§ Seams names no `{seam}`"));
        assert!(
            bullet.contains(file),
            "§ Seams: `{seam}` does not cite `{file}`"
        );
        let krate = file.split('/').next().expect("crate segment");
        let row = rows(&map)
            .into_iter()
            .find(|row| row["crate"] == krate)
            .unwrap_or_else(|| panic!("no row for {krate}"));
        assert_eq!(
            row["disposition"], "port",
            "{seam} is a seam a port reuses, yet {krate} is not ported"
        );
    }
}

/// Story § Scope: "the projected catalogue, action selection and argument generation have no
/// Harness counterpart". Mutant killed: a row (say `harness-tools`, ported) claiming to serve
/// `action selection strategy`, with the entry dropped from "New in Loom" to match. Acceptance 4
/// only checks that the two sides agree, so the map would credit Harness with a counterpart the
/// story says it lacks.
#[test]
fn adversary2_new_in_loom_keeps_the_three_without_counterpart() {
    let new_in_loom = bullet_items(&section(&map_doc(), "New in Loom"));
    for item in [
        "dynamic model-visible action catalogue",
        "action selection strategy",
        "action argument generation",
    ] {
        assert!(
            new_in_loom.contains(item),
            "New in Loom lacks `{item}`, which the story says has no Harness counterpart"
        );
    }
}

/// Mutant killed: `harness-flow` (or any `none` row) turned into a `port` onto
/// `crates/loom-executor/src/harness/flow/` with the licence filled in. Every check passes, and Loom carries
/// source that serves none of its responsibilities, which is the doc's own reason (§ Why, on
/// `harness-toolchain`) for not taking a crate.
#[test]
fn adversary2_carried_rows_serve_a_responsibility() {
    for row in rows(&map_doc()) {
        if row["disposition"] != "not carried" {
            assert_ne!(
                row["serves"], "none",
                "{}: {} a crate that serves no § Owns responsibility",
                row["crate"], row["disposition"]
            );
        }
    }
}

/// Mutant killed: a `not carried` row whose owner instead is `beyond10x/loom`, a crate Loom both
/// refuses and owns. `beyond10x/loom` is one well-formed repository, so the existing checks pass.
#[test]
fn adversary2_not_carried_rows_name_another_repository() {
    for row in rows(&map_doc()) {
        if row["disposition"] == "not carried" {
            assert_ne!(
                row["owner instead"], "beyond10x/loom",
                "{}: not carried into Loom, yet owned by Loom",
                row["crate"]
            );
        }
    }
}

/// Story § Scope: each row states "what it owns today (Harness README § Layout)". Mutant killed: an
/// empty or `—` "owns today" cell; no existing check reads that column.
#[test]
fn adversary2_every_row_says_what_it_owns_today() {
    for row in rows(&map_doc()) {
        let owns = row["owns today"].as_str();
        assert!(
            !owns.is_empty() && owns != NONE_CELL,
            "{}: owns today is empty",
            row["crate"]
        );
    }
}
