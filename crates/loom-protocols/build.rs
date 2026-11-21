//! Embeds the vendored engineering definitions, `protocols/engineering/` at the repository root,
//! in the crate. It writes two files to `$OUT_DIR` that `src/lib.rs` includes:
//!
//! - `engineering_files.rs`: one `(path, include_bytes!(file))` entry for every regular file below
//!   `protocols/engineering/` other than `lock.json`, the path relative to that directory with
//!   `/` between its parts, sorted by path as bytes. Every file is listed, whatever its name, so a
//!   file the lock does not name reaches the lock's check rather than being skipped;
//! - `engineering_lock.rs`: `include_bytes!` of `lock.json`, or no bytes when there is none, which
//!   the lock's decoder refuses as malformed.
//!
//! It reads nothing outside `protocols/engineering/`: a symbolic link or anything else that is
//! neither a regular file nor a directory fails the build.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let repository = Path::new(&manifest).join("../..");
    let repository = repository
        .canonicalize()
        .unwrap_or_else(|error| panic!("{}: {error}", repository.display()));
    let root = repository.join("protocols/engineering");
    let canonical = root
        .canonicalize()
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()));
    assert!(
        canonical == root,
        "{}: the vendored engineering protocols are a directory of the repository, not a link to {}",
        root.display(),
        canonical.display()
    );
    // Cargo scans a directory named here for any change below it, so an added, changed or removed
    // file re-runs this script.
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));

    let mut listed = String::from("&[\n");
    let mut lock = String::from("b\"\"");
    for (relative, path) in &files {
        let path = path
            .to_str()
            .unwrap_or_else(|| panic!("{}: path is not UTF-8", path.display()));
        if relative == "lock.json" {
            lock = format!("include_bytes!({path:?})");
        } else {
            listed.push_str(&format!("    ({relative:?}, include_bytes!({path:?})),\n"));
        }
    }
    listed.push_str("]\n");

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    for (name, text) in [
        ("engineering_files.rs", listed),
        ("engineering_lock.rs", format!("{lock}\n")),
    ] {
        let target = out.join(name);
        std::fs::write(&target, text)
            .unwrap_or_else(|error| panic!("{}: {error}", target.display()));
    }
}

/// Every regular file below `directory`, as (path relative to `root` joined by `/`, path).
fn walk(root: &Path, directory: &Path, found: &mut Vec<(String, PathBuf)>) {
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("{}: {error}", directory.display()));
        let path = entry.path();
        let kind = entry
            .file_type()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if kind.is_dir() {
            walk(root, &path, found);
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("below the root")
                .components()
                .map(|part| {
                    part.as_os_str()
                        .to_str()
                        .unwrap_or_else(|| panic!("{}: path is not UTF-8", path.display()))
                        .to_owned()
                })
                .collect::<Vec<_>>()
                .join("/");
            found.push((relative, path));
        } else {
            panic!(
                "{}: a vendored engineering protocol is a regular file, not a link or a special file",
                path.display()
            );
        }
    }
}
