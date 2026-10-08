//! Adversary pass 2 on `story:harness-crate-port`.
//!
//! In Harness each ported module was its own crate, and its manifest was the list of what it could
//! name: a module that reached for a crate its manifest did not list did not compile. In one Loom
//! crate every module sees every dependency and every sibling, so a source scan is all that holds
//! those rules. This file holds them by **allowlist**, from the five manifests at Harness
//! `798325f0`, and resolves the path forms a name scan misses (`super::super::`, brace groups, a
//! multi-line `use` tree, an alias, a leading `::`).
//!
//! It also checks what the pass-1 fix wrote into the ported comments: the citation rewrite and the
//! `rustfmt` comment rewrap.
//!
//! The ported source is read at run time through `CARGO_MANIFEST_DIR`.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo test"))
}

fn harness_dir() -> PathBuf {
    crate_dir().join("src/harness")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap_or_else(|e| panic!("{}: {e}", next.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// What each ported module may name, from the `[dependencies]` of its Harness crate at `798325f0`
/// (`crates/harness-*/Cargo.toml`): the sibling modules (Harness crates) and the extern crates.
/// Dev-dependencies are not granted; nothing in the ported `src/` uses one today.
const ALLOWED: [(&str, &[&str], &[&str]); 5] = [
    (
        "wire",
        &[],
        &["httpdate", "loom", "serde", "serde_json", "thiserror", "zeroize"],
    ),
    ("http", &["wire"], &["reqwest", "serde_json", "tokio"]),
    ("responses", &["http", "wire"], &["serde", "serde_json"]),
    ("messages", &["http", "wire"], &["serde", "serde_json"]),
    (
        "turn_loop",
        &["wire"],
        &["jsonschema", "serde", "serde_json", "sha2", "thiserror"],
    ),
];

/// Every crate `crates/loom-executor/Cargo.toml` makes nameable inside the crate, read from the manifest
/// so that a dependency added later is refused to every ported module that was not granted it.
fn loom_extern_crates() -> BTreeSet<String> {
    let manifest = fs::read_to_string(crate_dir().join("Cargo.toml")).expect("Cargo.toml");
    let mut names = BTreeSet::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]" || line == "[dev-dependencies]";
            continue;
        }
        if in_deps && let Some((key, _)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() && !key.starts_with('#') {
                names.insert(key.replace('-', "_"));
            }
        }
    }
    assert!(
        names.contains("reqwest") && names.contains("serde_json"),
        "the manifest scan found {names:?}"
    );
    names
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    PathSep,
    Open,
    Close,
    Comma,
    Semi,
    Star,
    Other,
}

/// Rust source as tokens with their line, comments and literals removed. Enough of a lexer to
/// find paths: it does not need to know types, only where code is.
fn lex(src: &str) -> Vec<(Tok, usize)> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let at = |i: usize| chars.get(i).copied().unwrap_or('\0');
    let ident_start = |c: char| c == '_' || c.is_alphabetic();
    let ident_char = |c: char| c == '_' || c.is_alphanumeric();
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && at(i + 1) == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && at(i + 1) == '*' {
            let mut depth = 0;
            loop {
                if i >= chars.len() {
                    break;
                }
                if chars[i] == '/' && at(i + 1) == '*' {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                if at(i) == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 1;
        } else if c == '\'' {
            // A char literal ('a', '\n', '\u{1F600}') or a lifetime ('a, 'static).
            if at(i + 1) == '\\' {
                // Past the quote, the backslash and the escaped character, which may be a quote.
                i += 3;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if at(i + 2) == '\'' {
                i += 3;
            } else {
                i += 1;
            }
        } else if ident_start(c) {
            let start = i;
            while i < chars.len() && ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // Raw strings (r"…", r#"…"#, br#"…"#) and raw identifiers (r#type).
            if (word == "r" || word == "br") && (at(i) == '"' || at(i) == '#') {
                let mut hashes = 0;
                while at(i + hashes) == '#' {
                    hashes += 1;
                }
                if at(i + hashes) == '"' {
                    i += hashes + 1;
                    loop {
                        if i >= chars.len() {
                            break;
                        }
                        if chars[i] == '"' && (0..hashes).all(|h| at(i + 1 + h) == '#') {
                            i += 1 + hashes;
                            break;
                        }
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                if word == "r" && hashes == 1 && ident_start(at(i + 1)) {
                    i += 1;
                    let start = i;
                    while i < chars.len() && ident_char(chars[i]) {
                        i += 1;
                    }
                    out.push((Tok::Ident(chars[start..i].iter().collect()), line));
                    continue;
                }
            }
            out.push((Tok::Ident(word), line));
        } else if c.is_ascii_digit() {
            while i < chars.len() && ident_char(chars[i]) {
                i += 1;
            }
        } else if c == ':' && at(i + 1) == ':' {
            out.push((Tok::PathSep, line));
            i += 2;
        } else {
            let tok = match c {
                '{' => Tok::Open,
                '}' => Tok::Close,
                ',' => Tok::Comma,
                ';' => Tok::Semi,
                '*' => Tok::Star,
                _ => Tok::Other,
            };
            out.push((tok, line));
            i += 1;
        }
    }
    out
}

/// One path named in code, as written, with the module it was written in.
#[derive(Debug, Clone)]
struct Named {
    line: usize,
    module: Vec<String>,
    absolute: bool,
    segments: Vec<String>,
    alias: Option<String>,
}

fn ident(tokens: &[(Tok, usize)], i: usize) -> Option<&str> {
    match tokens.get(i) {
        Some((Tok::Ident(s), _)) => Some(s.as_str()),
        _ => None,
    }
}

fn is(tokens: &[(Tok, usize)], i: usize, tok: &Tok) -> bool {
    tokens.get(i).is_some_and(|(t, _)| t == tok)
}

/// Parses a `use` tree starting at `i`, appending every leaf path (`a::b::{c, d::*}` gives `a::b::c`
/// and `a::b::d`). Returns the index after the tree.
fn use_tree(
    tokens: &[(Tok, usize)],
    mut i: usize,
    prefix: &[String],
    out: &mut Vec<(Vec<String>, Option<String>, usize)>,
) -> usize {
    let mut path = prefix.to_vec();
    loop {
        if is(tokens, i, &Tok::Open) {
            i += 1;
            loop {
                if is(tokens, i, &Tok::Close) {
                    return i + 1;
                }
                i = use_tree(tokens, i, &path, out);
                if is(tokens, i, &Tok::Comma) {
                    i += 1;
                }
                if i >= tokens.len() {
                    return i;
                }
            }
        }
        if is(tokens, i, &Tok::Star) {
            out.push((path, None, tokens[i].1));
            return i + 1;
        }
        let Some(word) = ident(tokens, i) else {
            return i;
        };
        let line = tokens[i].1;
        if word == "self" && !path.is_empty() && !is(tokens, i + 1, &Tok::PathSep) {
            i += 1;
        } else {
            path.push(word.to_owned());
            i += 1;
        }
        if is(tokens, i, &Tok::PathSep) {
            i += 1;
            continue;
        }
        let mut alias = None;
        if ident(tokens, i) == Some("as") {
            alias = ident(tokens, i + 1).map(str::to_owned);
            i += 2;
        }
        out.push((path, alias, line));
        return i;
    }
}

/// The module path of a ported file, below the crate root: `messages/project.rs` is
/// `harness::messages::project`, `wire/mod.rs` is `harness::wire`.
fn file_module(rel: &Path) -> Vec<String> {
    let mut module = vec!["harness".to_owned()];
    let parts: Vec<String> = rel
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    for (n, part) in parts.iter().enumerate() {
        if n + 1 == parts.len() {
            let stem = part.trim_end_matches(".rs");
            if stem != "mod" {
                module.push(stem.to_owned());
            }
        } else {
            module.push(part.clone());
        }
    }
    module
}

/// Every path `src` names, with the (inline-)module it is named in.
fn named_paths(src: &str, file_module: &[String]) -> Vec<Named> {
    let tokens = lex(src);
    let mut out = Vec::new();
    let mut mods: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let module_at = |mods: &[(String, usize)]| {
        let mut m = file_module.to_vec();
        m.extend(mods.iter().map(|(n, _)| n.clone()));
        m
    };
    while i < tokens.len() {
        match &tokens[i].0 {
            Tok::Open => {
                depth += 1;
                i += 1;
            }
            Tok::Close => {
                if mods.last().is_some_and(|(_, d)| *d == depth) {
                    mods.pop();
                }
                depth = depth.saturating_sub(1);
                i += 1;
            }
            Tok::Ident(w) if w == "mod" => {
                if let Some(name) = ident(&tokens, i + 1)
                    && is(&tokens, i + 2, &Tok::Open)
                {
                    mods.push((name.to_owned(), depth + 1));
                }
                i += 2;
            }
            Tok::Ident(w) if w == "use" => {
                let absolute = is(&tokens, i + 1, &Tok::PathSep);
                let start = if absolute { i + 2 } else { i + 1 };
                let mut leaves = Vec::new();
                let next = use_tree(&tokens, start, &[], &mut leaves);
                for (segments, alias, line) in leaves {
                    out.push(Named {
                        line,
                        module: module_at(&mods),
                        absolute,
                        segments,
                        alias,
                    });
                }
                i = next.max(i + 1);
            }
            Tok::Ident(w) => {
                let after_sep = i > 0 && is(&tokens, i - 1, &Tok::PathSep);
                let leading = i > 0
                    && is(&tokens, i - 1, &Tok::PathSep)
                    && !(i > 1 && matches!(tokens[i - 2].0, Tok::Ident(_)));
                if (!after_sep || leading) && is(&tokens, i + 1, &Tok::PathSep) {
                    let line = tokens[i].1;
                    let mut segments = vec![w.clone()];
                    let mut j = i + 1;
                    while is(&tokens, j, &Tok::PathSep) {
                        match ident(&tokens, j + 1) {
                            Some(s) => {
                                segments.push(s.to_owned());
                                j += 2;
                            }
                            None => break,
                        }
                    }
                    out.push(Named {
                        line,
                        module: module_at(&mods),
                        absolute: leading,
                        segments,
                        alias: None,
                    });
                    i = j;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// What a named path reaches, from inside `own` (a ported module name): another ported module, the
/// harness root, the rest of Loom, or an extern crate. `None` is a path inside `own`, into `std`,
/// or one that starts at a local item.
fn reach(
    named: &Named,
    own: &str,
    externs: &BTreeSet<String>,
    aliases: &BTreeMap<String, Vec<String>>,
) -> Option<String> {
    let first = named.segments.first()?.as_str();
    let resolved: Vec<String> = if named.absolute {
        return externs.contains(first).then(|| format!("crate `{first}`"));
    } else if first == "crate" {
        named.segments[1..].to_vec()
    } else if first == "super" || first == "self" {
        let mut base = named.module.clone();
        let mut rest = named.segments.as_slice();
        while let Some(head) = rest.first() {
            match head.as_str() {
                "super" => {
                    base.pop();
                }
                "self" => {}
                _ => break,
            }
            rest = &rest[1..];
        }
        base.extend(rest.iter().cloned());
        base
    } else if let Some(target) = aliases.get(first) {
        let mut base = target.clone();
        base.extend(named.segments[1..].iter().cloned());
        base
    } else if externs.contains(first) {
        return Some(format!("crate `{first}`"));
    } else {
        return None;
    };
    match resolved.as_slice() {
        [] => Some("the crate root".to_owned()),
        [h] if h == "harness" => Some("the harness root (every sibling)".to_owned()),
        [h, m, ..] if h == "harness" => (m != own).then(|| format!("module `{m}`")),
        [other, ..] => Some(format!("Loom's `crate::{other}`")),
    }
}

/// Everything `src`, at `rel` below `src/harness`, names that its Harness crate could not have.
fn crossings(rel: &Path, src: &str, externs: &BTreeSet<String>) -> Vec<String> {
    let own = rel
        .iter()
        .next()
        .expect("a module dir")
        .to_string_lossy()
        .into_owned();
    let (_, modules, crates) = ALLOWED
        .iter()
        .find(|(m, _, _)| *m == own)
        .unwrap_or_else(|| panic!("{own} is not a ported module"));
    let module = file_module(rel);
    let named = named_paths(src, &module);
    // `use x::y as z;` and `use x::y;` bind a local name to a crate path.
    let mut aliases = BTreeMap::new();
    for n in &named {
        let Some(last) = n.segments.last() else {
            continue;
        };
        let local = n.alias.clone().unwrap_or_else(|| last.clone());
        let target = match n.segments.first().map(String::as_str) {
            Some("crate") => n.segments[1..].to_vec(),
            Some("super" | "self") => {
                let mut base = n.module.clone();
                for s in &n.segments {
                    match s.as_str() {
                        "super" => {
                            base.pop();
                        }
                        "self" => {}
                        other => base.push(other.to_owned()),
                    }
                }
                base
            }
            _ => continue,
        };
        aliases.insert(local, target);
    }
    let mut found = BTreeSet::new();
    for n in &named {
        let Some(what) = reach(n, &own, externs, &aliases) else {
            continue;
        };
        let allowed = match what.strip_prefix("module `") {
            Some(m) => modules.contains(&m.trim_end_matches('`')),
            None => match what.strip_prefix("crate `") {
                Some(c) => crates.contains(&c.trim_end_matches('`')),
                None => false,
            },
        };
        if !allowed {
            found.insert(format!(
                "{}:{}: {} names {what} ({})",
                rel.display(),
                n.line,
                own,
                n.segments.join("::")
            ));
        }
    }
    found.into_iter().collect()
}

/// The scan finds each form a crate boundary would have refused, and passes what it allowed. Run
/// on planted sources, so it shows the check can fail.
#[test]
fn the_allowlist_scan_finds_every_form_a_crate_boundary_refused() {
    let externs = loom_extern_crates();
    let refused = [
        (
            "messages/project.rs",
            "use super::super::responses::WIRE;\n",
        ),
        (
            "messages/mod.rs",
            "const OTHER: &str = super::responses::WIRE;\n",
        ),
        (
            "messages/project.rs",
            "use crate::harness::{http::Headers, responses};\n",
        ),
        (
            "messages/project.rs",
            "use crate::harness::{\n    http::Headers,\n    responses::WIRE,\n};\n",
        ),
        (
            "messages/project.rs",
            "use super::super as h;\nconst W: &str = h::responses::WIRE;\n",
        ),
        ("messages/mod.rs", "use super::*;\n"),
        ("wire/turn.rs", "fn f() { ::tokio::spawn(async {}); }\n"),
        ("wire/turn.rs", "use crate::selection::Selector;\n"),
        (
            "wire/turn.rs",
            "fn f() -> b10x_loom_commission::X { todo!() }\n",
        ),
        ("http/sse.rs", "#[derive(serde::Serialize)]\nstruct S;\n"),
        ("http/sse.rs", "#[derive(thiserror::Error)]\nstruct S;\n"),
        (
            "turn_loop/mod.rs",
            "let c = reqwest::blocking::Client::new();\n",
        ),
        ("turn_loop/mod.rs", "use crate::harness::http::Headers;\n"),
        (
            "turn_loop/tests.rs",
            "mod inner { use super::super::super::messages::X; }\n",
        ),
        ("responses/project.rs", "use httpdate::fmt_http_date;\n"),
    ];
    for (rel, src) in refused {
        assert!(
            !crossings(Path::new(rel), src, &externs).is_empty(),
            "the scan let `{}` in {rel} through",
            src.trim()
        );
    }
    let allowed = [
        (
            "messages/project.rs",
            "use crate::harness::wire::{Item, WireError};\n",
        ),
        (
            "messages/mod.rs",
            "use crate::harness::http::{Framing, Headers};\n",
        ),
        (
            "messages/project.rs",
            "use super::WIRE;\nuse serde_json::Value;\n",
        ),
        (
            "turn_loop/tests.rs",
            "use super::*;\nuse super::super::wire::Item;\n",
        ),
        (
            "http/sse.rs",
            "// reqwest::Client and super::super::messages in a comment\n",
        ),
        (
            "wire/turn.rs",
            "const S: &str = \"tokio::spawn and crate::harness::http\";\n",
        ),
        (
            "wire/turn.rs",
            "const R: &str = r#\"reqwest::get(\"x\")\"#;\n",
        ),
        ("wire/turn.rs", "fn f<'a>(x: &'a str) -> char { 'x' }\n"),
    ];
    for (rel, src) in allowed {
        assert_eq!(
            crossings(Path::new(rel), src, &externs),
            Vec::<String>::new(),
            "the scan refused `{}` in {rel}",
            src.trim()
        );
    }
}

/// Each ported module names only the siblings and crates its Harness crate's manifest listed.
#[test]
fn each_ported_module_names_only_what_its_harness_manifest_allowed() {
    let externs = loom_extern_crates();
    let root = harness_dir();
    let mut found = Vec::new();
    let mut scanned = 0;
    for (module, _, _) in ALLOWED {
        for path in rust_files(&root.join(module)) {
            let rel = path.strip_prefix(&root).expect("below src/harness");
            let src = fs::read_to_string(&path).expect("readable source");
            found.extend(crossings(rel, &src, &externs));
            scanned += 1;
        }
    }
    assert_eq!(scanned, 36, "the five ported modules hold 36 files");
    assert!(
        found.is_empty(),
        "a ported module names what its Harness crate could not depend on:\n{}",
        found.join("\n")
    );
}

/// A comment paragraph: consecutive comment lines of one kind (`//`, `///`, `//!`) with no blank
/// comment line between them, as `(first line, joined text)`.
fn comment_paragraphs(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current: Option<(usize, &str, String)> = None;
    for (n, raw) in src.lines().enumerate() {
        let line = raw.trim_start();
        let kind = if line.starts_with("//!") {
            Some("//!")
        } else if line.starts_with("///") {
            Some("///")
        } else if line.starts_with("//") {
            Some("//")
        } else {
            None
        };
        let text = kind.map(|k| line[k.len()..].trim());
        match (kind, text, current.as_mut()) {
            (Some(k), Some(t), Some((_, ck, buf))) if *ck == k && !t.is_empty() => {
                buf.push(' ');
                buf.push_str(t);
            }
            (Some(k), Some(t), _) if !t.is_empty() => {
                if let Some((start, _, buf)) = current.take() {
                    out.push((start, buf));
                }
                current = Some((n + 1, k, t.to_owned()));
            }
            _ => {
                if let Some((start, _, buf)) = current.take() {
                    out.push((start, buf));
                }
            }
        }
    }
    if let Some((start, _, buf)) = current {
        out.push((start, buf));
    }
    out
}

/// The pass-1 fix rewrote the citations the pass-1 guard could see (`AGENTS.md invariant N`, a
/// backticked `docs/design/…` or `ROADMAP.md` path) to name Harness and `798325f0`. The short form
/// of the same citation, `design 0002 § N` and its milestones, was left as it was. Loom has no
/// design 0002 (`docs/design/` holds `harness-map.md` and `loom-design.md`), so each one points at
/// nothing here, or at whatever Loom numbers 0002 next.
#[test]
fn every_harness_document_a_ported_comment_cites_is_named_as_harnesss() {
    let root = harness_dir();
    let mut dangling = Vec::new();
    for path in rust_files(&root) {
        let rel = path
            .strip_prefix(&root)
            .expect("below")
            .display()
            .to_string();
        let src = fs::read_to_string(&path).expect("readable");
        for (line, text) in comment_paragraphs(&src) {
            let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
            let cites = flat.contains("design 0002")
                || flat.contains("ROADMAP")
                || flat
                    .split("milestone M")
                    .skip(1)
                    .any(|r| r.starts_with(|c: char| c.is_ascii_digit()))
                || flat
                    .split("invariant ")
                    .skip(1)
                    .any(|r| r.starts_with(|c: char| c.is_ascii_digit()));
            if cites && !flat.contains("Harness") {
                let shown: String = flat.chars().take(90).collect();
                dangling.push(format!("{rel}:{line}: {shown}"));
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "{} ported comment paragraphs cite a Harness document without naming Harness:\n{}",
        dangling.len(),
        dangling.join("\n")
    );
}

/// `rustfmt +nightly` with `wrap_comments` split Harness's section dividers
/// (`// --- name ------`) where the heading pushed them past the comment width, leaving a heading
/// with no rule and a rule with no heading. Harness at `798325f0` has no comment line that is
/// dashes alone.
#[test]
fn no_ported_comment_line_is_the_tail_of_a_split_divider() {
    let root = harness_dir();
    let mut split = Vec::new();
    for path in rust_files(&root) {
        let rel = path
            .strip_prefix(&root)
            .expect("below")
            .display()
            .to_string();
        let src = fs::read_to_string(&path).expect("readable");
        for (n, raw) in src.lines().enumerate() {
            let line = raw.trim();
            if let Some(text) = line.strip_prefix("//") {
                let text = text.trim_start_matches(['/', '!']).trim();
                if text.len() >= 3 && text.chars().all(|c| c == '-') {
                    split.push(format!("{rel}:{}", n + 1));
                }
            }
        }
    }
    assert!(
        split.is_empty(),
        "comment lines that are only the dashes of a divider whose heading is on the line above:\n{}",
        split.join("\n")
    );
}
