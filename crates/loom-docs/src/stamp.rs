//! The stamp of the generated documentation, `website/docs/reference/.generated.json`.
//!
//! `loom-docs generate` writes it after the pages it generates: the SHA-256 of every file the
//! generator reads ([`INPUTS`]), the version line of the `ess` that ran, and the SHA-256 of every
//! file it writes ([`OUTPUTS`]). `loom-xtask docs-stamp --check` (`task docs-check`) compares the
//! working tree with it, so a tree whose change leaves the inputs alone never builds `loom-docs`.
//!
//! This one file is compiled by two crates: `loom-docs` declares it `mod stamp;`, and
//! `loom-xtask` includes it by `#[path]`, with no crate edge between them, so the check never
//! builds `loom-docs` or `b10x-loom-cli`. It uses `std` and `sha2` only, its errors are
//! [`std::io::Error`] with the path in the message, and it holds no test and nothing that starts
//! `cargo` or `ess`.
#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

/// The stamp, relative to the repository root.
pub const PATH: &str = "website/docs/reference/.generated.json";

/// The format of the stamp, its first field.
const SCHEMA: &str = "loom-docs-stamp/1";

/// What the generator reads, relative to the repository root. A directory covers every file
/// under it. `cargo metadata --no-deps` reads the manifests and the lock; `ess` reads `ess/`,
/// which holds `ess/intake/`; the CLI pages walk the clap definitions of the two command lines.
pub const INPUTS: [&str; 7] = [
    "Cargo.lock",
    "Cargo.toml",
    "crates/loom-cli",
    "crates/loom-docs",
    "crates/loom-operator",
    "ess",
    "website/docs/reference/run-events.md",
];

/// Every `Cargo.toml` under this directory is an input too: the manifests of the workspace's
/// members, which `cargo metadata` reads for the crate list.
pub const MANIFESTS: &str = "crates";

/// Everything `loom-docs generate` writes, relative to the repository root: a directory it
/// replaces whole, or one file.
pub const OUTPUTS: [&str; 7] = [
    "docs/contracts/run-events-v1.json",
    "docs/contracts/run-log-v1.json",
    "website/data/ess",
    "website/docs/reference/cli.md",
    "website/docs/reference/crates.md",
    "website/docs/reference/ess",
    "website/docs/reference/operator-cli.md",
];

/// `sha256:<hex>` of `bytes`.
pub fn hash(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Whether `relative` is `entry` or lies under it.
fn under(relative: &str, entry: &str) -> bool {
    relative == entry
        || relative
            .strip_prefix(entry)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// A `/`-separated path relative to the root, with no `.` or `..` component.
fn plain(relative: &str) -> bool {
    !relative.is_empty()
        && !relative.starts_with('/')
        && relative
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Whether `relative` is a file [`INPUTS`] or [`MANIFESTS`] covers.
pub fn is_input(relative: &str) -> bool {
    plain(relative)
        && (INPUTS.iter().any(|entry| under(relative, entry))
            || (under(relative, MANIFESTS) && relative.ends_with("/Cargo.toml")))
}

/// Whether `relative` is a file [`OUTPUTS`] covers.
pub fn is_output(relative: &str) -> bool {
    plain(relative) && OUTPUTS.iter().any(|entry| under(relative, entry))
}

fn refused(relative: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "loom-docs reads {relative}, which is neither an input of {PATH} (`stamp::INPUTS`) \
             nor a file it generates (`stamp::OUTPUTS`); add it to `INPUTS`"
        ),
    )
}

fn with_path(error: io::Error, action: &str, path: &Path) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{action} {}: {error}", path.display()),
    )
}

/// The text of `<root>/<relative>`, which must be an input or an output; the one way
/// `loom-docs` reads a file of the repository.
pub fn read_input(root: &Path, relative: &str) -> io::Result<String> {
    if !is_input(relative) && !is_output(relative) {
        return Err(refused(relative));
    }
    let path = root.join(relative);
    fs::read_to_string(&path).map_err(|e| with_path(e, "reading", &path))
}

/// The names of the entries of the directory `<root>/<relative>`, sorted; the directory must be
/// an input or an output.
pub fn read_input_dir(root: &Path, relative: &str) -> io::Result<Vec<String>> {
    if !is_input(relative) && !is_output(relative) {
        return Err(refused(relative));
    }
    names(&root.join(relative))
}

fn names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| with_path(e, "reading", dir))? {
        let entry = entry.map_err(|e| with_path(e, "reading", dir))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
}

/// The directory of `ess generate`'s output under the build directory `build`: what `ess`
/// writes from `ess/`, an input, with the version line the stamp records.
fn ess_output(build: &Path, path: &Path) -> io::Result<()> {
    let owned = build.join("loom-docs");
    if path.starts_with(&owned) && path != owned {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "loom-docs reads {} as output of ess, which lies outside {}",
                path.display(),
                owned.display()
            ),
        ))
    }
}

/// The text of a file `ess` wrote under `<build>/loom-docs/`.
pub fn read_ess_output(build: &Path, path: &Path) -> io::Result<String> {
    ess_output(build, path)?;
    fs::read_to_string(path).map_err(|e| with_path(e, "reading", path))
}

/// The sorted entry names of a directory `ess` wrote under `<build>/loom-docs/`.
pub fn read_ess_output_dir(build: &Path, dir: &Path) -> io::Result<Vec<String>> {
    ess_output(build, dir)?;
    names(dir)
}

/// Adds every file at or under `<root>/<relative>` to `out` with its hash, keyed by its
/// `/`-separated path relative to `root`; adds nothing when it does not exist.
fn walk(
    root: &Path,
    relative: &str,
    keep: &dyn Fn(&str) -> bool,
    out: &mut BTreeMap<String, String>,
) -> io::Result<()> {
    let path = root.join(relative);
    let meta = match fs::metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(with_path(error, "reading", &path)),
    };
    if meta.is_dir() {
        for name in names(&path)? {
            walk(root, &format!("{relative}/{name}"), keep, out)?;
        }
    } else if keep(relative) {
        let bytes = fs::read(&path).map_err(|e| with_path(e, "reading", &path))?;
        out.insert(relative.to_owned(), hash(&bytes));
    }
    Ok(())
}

/// Every input file under `root` with its hash, by sorted path.
pub fn input_files(root: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in INPUTS {
        walk(root, entry, &|_| true, &mut files)?;
    }
    walk(
        root,
        MANIFESTS,
        &|relative| relative.ends_with("/Cargo.toml"),
        &mut files,
    )?;
    Ok(files)
}

/// Every output file under `root` with its hash, by sorted path.
pub fn output_files(root: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in OUTPUTS {
        walk(root, entry, &|_| true, &mut files)?;
    }
    Ok(files)
}

/// The digest of a sorted list of input paths and their hashes: one line `<path> <hash>` each.
/// A changed, an added and a removed file each change it.
pub fn input_hash_of(files: &BTreeMap<String, String>) -> String {
    let mut text = String::new();
    for (path, digest) in files {
        let _ = writeln!(text, "{path} {digest}");
    }
    hash(text.as_bytes())
}

/// The digest of the generator's inputs under `root`.
pub fn input_hash(root: &Path) -> io::Result<String> {
    Ok(input_hash_of(&input_files(root)?))
}

/// The version line of an `ess --version` output: its last non-empty line, which is the
/// delegate's when a dispatcher hands the call to a pinned release.
pub fn version_line(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

/// How a file differs between the stamp and the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Both have it, with other bytes.
    Changed,
    /// The tree has it; the stamp does not.
    Added,
    /// The stamp has it; the tree does not.
    Removed,
}

/// The first path, in path order, whose hash differs between `stamped` and `found`.
pub fn first_difference(
    stamped: &BTreeMap<String, String>,
    found: &BTreeMap<String, String>,
) -> Option<(String, Change)> {
    let mut paths: Vec<&String> = stamped.keys().chain(found.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .find_map(|path| match (stamped.get(path), found.get(path)) {
            (Some(a), Some(b)) if a == b => None,
            (Some(_), Some(_)) => Some((path.clone(), Change::Changed)),
            (None, _) => Some((path.clone(), Change::Added)),
            (_, None) => Some((path.clone(), Change::Removed)),
        })
}

/// What the stamp records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// The version line of the `ess` that generated the pages.
    pub ess: String,
    /// [`input_hash_of`] of `input_files`.
    pub inputs: String,
    /// Every input file with its hash.
    pub input_files: BTreeMap<String, String>,
    /// Every output file with its hash.
    pub outputs: BTreeMap<String, String>,
}

impl Stamp {
    /// The stamp of these inputs, this `ess` and these outputs.
    pub fn new(
        input_files: BTreeMap<String, String>,
        ess: &str,
        outputs: BTreeMap<String, String>,
    ) -> Stamp {
        Stamp {
            ess: ess.to_owned(),
            inputs: input_hash_of(&input_files),
            input_files,
            outputs,
        }
    }

    /// The stamp as the JSON document committed at [`PATH`].
    pub fn render(&self) -> String {
        let mut out = String::from("{\n");
        let _ = writeln!(out, "  \"schema\": {},", json_string(SCHEMA));
        let _ = writeln!(out, "  \"ess\": {},", json_string(&self.ess));
        let _ = writeln!(out, "  \"inputs\": {},", json_string(&self.inputs));
        map(&mut out, "input_files", &self.input_files, ",");
        map(&mut out, "outputs", &self.outputs, "");
        out.push_str("}\n");
        out
    }

    /// Reads a document [`Stamp::render`] wrote; anything else is refused.
    pub fn parse(text: &str) -> io::Result<Stamp> {
        let bad = |what: &str| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{PATH} is not a stamp loom-docs wrote ({what}); run `task docs-generate`"),
            )
        };
        let mut lines = text.lines();
        let mut next = || lines.next().ok_or_else(|| bad("it ends early"));
        if next()? != "{" {
            return Err(bad("no opening brace"));
        }
        let field = |line: &str, name: &str| -> io::Result<String> {
            let value = line
                .strip_prefix(&format!("  {}: ", json_string(name)))
                .and_then(|rest| rest.strip_suffix(','))
                .ok_or_else(|| bad(&format!("no field {name}")))?;
            parse_string(value).ok_or_else(|| bad(&format!("field {name} is not a string")))
        };
        if field(next()?, "schema")? != SCHEMA {
            return Err(bad("another schema"));
        }
        let ess = field(next()?, "ess")?;
        let inputs = field(next()?, "inputs")?;
        let mut read_map = |name: &str, last: bool| -> io::Result<BTreeMap<String, String>> {
            if next()? != format!("  {}: {{", json_string(name)) {
                return Err(bad(&format!("no map {name}")));
            }
            let mut entries = BTreeMap::new();
            loop {
                let line = next()?;
                if line == if last { "  }" } else { "  }," } {
                    return Ok(entries);
                }
                let entry = line
                    .strip_prefix("    ")
                    .ok_or_else(|| bad(&format!("a malformed entry of {name}")))?;
                let entry = entry.strip_suffix(',').unwrap_or(entry);
                let (key, value) = take_string(entry)
                    .and_then(|(key, rest)| Some((key, rest.strip_prefix(": ")?)))
                    .and_then(|(key, rest)| Some((key, parse_string(rest)?)))
                    .ok_or_else(|| bad(&format!("a malformed entry of {name}")))?;
                if entries.insert(key, value).is_some() {
                    return Err(bad(&format!("a repeated entry of {name}")));
                }
            }
        };
        let input_files = read_map("input_files", false)?;
        let outputs = read_map("outputs", true)?;
        if next()? != "}" || lines.next().is_some() {
            return Err(bad("text after the closing brace"));
        }
        Ok(Stamp {
            ess,
            inputs,
            input_files,
            outputs,
        })
    }
}

fn map(out: &mut String, name: &str, entries: &BTreeMap<String, String>, after: &str) {
    let _ = writeln!(out, "  {}: {{", json_string(name));
    let last = entries.len().saturating_sub(1);
    for (n, (key, value)) in entries.iter().enumerate() {
        let comma = if n == last { "" } else { "," };
        let _ = writeln!(
            out,
            "    {}: {}{comma}",
            json_string(key),
            json_string(value)
        );
    }
    let _ = writeln!(out, "  }}{after}");
}

/// `value` as a JSON string.
fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The value of the JSON string [`json_string`] wrote at the start of `text`, and the text after
/// it; `None` for anything else.
fn take_string(text: &str) -> Option<(String, &str)> {
    let inner = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = inner.char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Some((out, &inner[at + 1..])),
            '\\' => match chars.next()?.1 {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let code: String = chars.by_ref().take(4).map(|(_, c)| c).collect();
                    out.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
                }
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}

/// The value of a JSON string [`json_string`] wrote, and nothing after it.
fn parse_string(text: &str) -> Option<String> {
    take_string(text).and_then(|(value, rest)| rest.is_empty().then_some(value))
}

/// The stamp committed under `root`.
pub fn read(root: &Path) -> io::Result<Stamp> {
    let path = root.join(PATH);
    let text = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                error.kind(),
                format!("{PATH} is missing; run `task docs-generate`"),
            )
        } else {
            with_path(error, "reading", &path)
        }
    })?;
    Stamp::parse(&text)
}

/// Writes `stamp` to [`PATH`] under `root`.
pub fn write(root: &Path, stamp: &Stamp) -> io::Result<()> {
    let path = root.join(PATH);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| with_path(e, "creating", dir))?;
    }
    fs::write(&path, stamp.render()).map_err(|e| with_path(e, "writing", &path))
}

/// Removes the stamp under `root`, if there is one.
pub fn remove(root: &Path) -> io::Result<()> {
    let path = root.join(PATH);
    match fs::remove_file(&path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(with_path(error, "removing", &path))
        }
        _ => Ok(()),
    }
}

/// Whether `line` matches `^:::[a-z]+ +\S`: an admonition whose title is not in brackets, which
/// Docusaurus prints as raw text.
pub fn is_raw_admonition(line: &str) -> bool {
    let Some(rest) = line.strip_prefix(":::") else {
        return false;
    };
    let kind = rest.bytes().take_while(u8::is_ascii_lowercase).count();
    let rest = &rest[kind..];
    let spaces = rest.bytes().take_while(|b| *b == b' ').count();
    kind > 0
        && spaces > 0
        && rest[spaces..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace())
}

/// `file:line` of every raw admonition title in the Markdown and MDX pages under `dir`. The
/// pages are written by hand and are not inputs, so the stamp does not cover them; the check
/// runs this lint on every call.
pub fn raw_admonitions(dir: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    for name in names(dir)? {
        let path: PathBuf = dir.join(name);
        if path.is_dir() {
            found.extend(raw_admonitions(&path)?);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "md" || ext == "mdx")
        {
            let text = fs::read_to_string(&path).map_err(|e| with_path(e, "reading", &path))?;
            for (n, line) in text.lines().enumerate() {
                if is_raw_admonition(line) {
                    found.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        }
    }
    Ok(found)
}
