//! `loom-xtask shared-trees`: the trees the `tree_run` tests share, built once per key and kept
//! across runs (`story:tree-run-binary-fits-its-start-bound`).
//!
//! The key ([`tree_key`]) is SHA-256 over the inputs and Loom's sources, never an executable: the
//! files of the fixture directory, the builders' sources (`tests/tree_run.rs` and
//! `tests/tree_run/*.rs`), Loom's sources ([`built_sources`]: every file of the repository that a
//! rustc dep-info file of the build directory lists, outside `target/` and outside a `tests`
//! directory, so each source of every crate built there, `build.rs` files and what `include_str!`
//! and `include_bytes!` read), `Cargo.lock`, `rust-toolchain.toml` and `.cargo/config.toml` when
//! present, and the scope the tests were built in (`LOOM_TEST_SCOPE`, else `-p b10x-loom-cli`),
//! since under resolver 2 the scope decides which features are unified. Cargo writes no dep-info of
//! `b10x-loom` itself (`<target>/debug/b10x-loom.d`) when it builds the binary for a test, so the
//! key reads the dep-info of every crate: a superset of the binary's sources.
//!
//! [`make`] removes every attempt directory and installed home of another key, then, when a tree or
//! the home of the key is missing, runs `cargo test --locked <scope> --test tree_run -- --ignored
//! --exact the_shared_trees_are_made` with the key. The test step calls it after its build, outside
//! its timed window; nextest runs it as the setup script of `binary_id(b10x-loom-cli::tree_run)`
//! (`.config/nextest.toml`), where it exports the key to the tests through `NEXTEST_ENV` and, with
//! the trees there, builds nothing. Run without `LOOM_TEST_SCOPE` after a build of another scope,
//! its `cargo test` may build again and replace the `b10x-loom` the tests start; the key then
//! names that scope and the trees are built for it. The test step always sets the variable.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::{Digest as _, Sha256};

#[path = "../../loom-cli/tests/tree_run/key.rs"]
pub mod key;

/// The variable naming the scope the tests were built in, as cargo's arguments.
pub const SCOPE_VARIABLE: &str = "LOOM_TEST_SCOPE";

/// The scope when [`SCOPE_VARIABLE`] is not set.
pub const DEFAULT_SCOPE: &str = "-p b10x-loom-cli";

/// The fixture directory the builders read, under the repository root.
const FIXTURE: &str = "crates/loom-cli/tests/fixtures/tree-two-level";

/// The builders' sources: the test file and its modules' directory.
const BUILDERS: (&str, &str) = (
    "crates/loom-cli/tests/tree_run.rs",
    "crates/loom-cli/tests/tree_run",
);

/// The files of the repository, beside the sources, that decide what a build gives.
const BUILD_FILES: [&str; 3] = ["Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml"];

/// The test binary and the ignored test that build the trees.
const BUILDER_TEST: (&str, &str) = ("tree_run", "the_shared_trees_are_made");

/// The variables the setup's `cargo test` must not inherit: its starts are the setup's, not a
/// test's, and it builds no tree as part of another's build.
const NOT_INHERITED: [&str; 2] = ["LOOM_TREE_RUN_STARTS", "LOOM_TREE_RUN_FIXTURE"];

/// The file under `<target>/nextest/` each run of [`make`] writes what it did to, which the test
/// step reads after its nextest run to print the setup's time.
pub const REPORT: &str = "shared-trees.txt";

/// What one run of [`make`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    pub key: String,
    /// The trees (and `home`) that were missing and were built; empty when none was.
    pub built: Vec<String>,
    /// The directories and lock files of another key removed before.
    pub removed: usize,
    pub seconds: f64,
}

impl Made {
    /// The line the setup prints and writes to [`REPORT`].
    pub fn line(&self) -> String {
        format!(
            "shared trees: key {}, built {}, removed {} entr{} of another key, {:.1} s",
            self.key,
            if self.built.is_empty() {
                "nothing".to_owned()
            } else {
                self.built.join(", ")
            },
            self.removed,
            if self.removed == 1 { "y" } else { "ies" },
            self.seconds
        )
    }
}

/// The scope from the value of [`SCOPE_VARIABLE`].
pub fn scope_of(value: Option<&std::ffi::OsStr>) -> String {
    value
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_SCOPE)
        .to_owned()
}

/// The key of the shared trees of the repository at `root`, built into `target` in `scope`: 16
/// hex digits of SHA-256 over the inputs named in the module's documentation, each file by its
/// path relative to `root` (absolute when outside it), its length and its bytes. Fails when the
/// dep-info of `b10x-loom` is missing (nothing was built) or names no file.
pub fn tree_key(root: &Path, target: &Path, scope: &str) -> Result<String, String> {
    let canonical = |path: &Path| {
        path.canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))
    };
    let (root, target) = (&canonical(root)?, &canonical(target)?);
    let mut hasher = Sha256::new();
    let mut add = |section: &str, path: &Path| -> Result<(), String> {
        let name = path.strip_prefix(root).unwrap_or(path);
        hasher.update(section.as_bytes());
        hasher.update([0]);
        hasher.update(name.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        match fs::read(path) {
            Ok(bytes) => {
                hasher.update((bytes.len() as u64).to_be_bytes());
                hasher.update(&bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hasher.update(b"absent");
            }
            Err(error) => return Err(format!("read {}: {error}", path.display())),
        }
        Ok(())
    };

    let mut fixture = Vec::new();
    files_under(&root.join(FIXTURE), &mut fixture)?;
    fixture.sort();
    for file in &fixture {
        add("fixture", file)?;
    }

    let mut builders = vec![root.join(BUILDERS.0)];
    let mut modules = Vec::new();
    files_under(&root.join(BUILDERS.1), &mut modules)?;
    modules.retain(|path| path.extension().is_some_and(|extension| extension == "rs"));
    modules.sort();
    builders.extend(modules);
    for file in &builders {
        add("builder", file)?;
    }

    for file in &built_sources(root, target)? {
        add("source", file)?;
    }

    for file in BUILD_FILES {
        add("build", &root.join(file))?;
    }
    hasher.update(b"scope");
    hasher.update([0]);
    hasher.update(
        scope
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .as_bytes(),
    );

    Ok(hasher.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Loom's sources as the build directory `target` knows them: every file the rustc dep-info files
/// of `<target>/debug/deps/` and `<target>/debug/build/*/` list (a relative path read from
/// `root`) that lies under `root`, not under `target` and not in a directory named `tests`, sorted,
/// each once. Fails when no dep-info file names such a file (nothing was built).
pub fn built_sources(root: &Path, target: &Path) -> Result<Vec<PathBuf>, String> {
    let debug = target.join("debug");
    let mut dep_infos = Vec::new();
    let mut directories = vec![debug.join("deps")];
    if let Ok(builds) = fs::read_dir(debug.join("build")) {
        directories.extend(builds.filter_map(Result::ok).map(|entry| entry.path()));
    }
    for directory in directories {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        dep_infos.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "d")),
        );
    }
    let mut sources = Vec::new();
    for dep_info in &dep_infos {
        let text = fs::read_to_string(dep_info)
            .map_err(|e| format!("read {}: {e}", dep_info.display()))?;
        // rustc names a workspace member's files relative to the workspace root, where cargo
        // starts it, and every other file by its absolute path.
        let named = dep_info_files(&text).into_iter().map(|file| {
            if file.is_relative() {
                root.join(file)
            } else {
                file
            }
        });
        sources.extend(named.filter(|file| {
            file.strip_prefix(root).is_ok_and(|relative| {
                !relative
                    .components()
                    .any(|part| part.as_os_str() == "tests")
            }) && !file.starts_with(target)
        }));
    }
    sources.sort();
    sources.dedup();
    if sources.is_empty() {
        return Err(format!(
            "no dep-info file under {} names a source of {}: build the tests of b10x-loom-cli \
             first",
            debug.display(),
            root.display()
        ));
    }
    Ok(sources)
}

/// The files a dep-info file of cargo or rustc lists after its target on its first line, a space
/// within a path written `\ `.
pub fn dep_info_files(text: &str) -> Vec<PathBuf> {
    let Some(line) = text.lines().find(|line| !line.trim().is_empty()) else {
        return Vec::new();
    };
    let Some(at) = line.find(": ") else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut current = String::new();
    let mut chars = line[at + 2..].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&' ') => {
                current.push(' ');
                chars.next();
            }
            ' ' => {
                if !current.is_empty() {
                    files.push(PathBuf::from(std::mem::take(&mut current)));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        files.push(PathBuf::from(current));
    }
    files
}

/// Every file under `dir`, at any depth.
fn files_under(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let kind = entry
            .file_type()
            .map_err(|e| format!("read {}: {e}", entry.path().display()))?;
        if kind.is_dir() {
            files_under(&entry.path(), found)?;
        } else {
            found.push(entry.path());
        }
    }
    Ok(())
}

/// The entries of `base` that belong to the shared trees or the installed home under a setup's
/// key other than `key` and `home`: attempt directories, lock files, homes and homes being made.
/// A test's copy (`tree-run-ended-copy-…`) and a tree under a process's own key are not one.
pub fn of_another_key(base: &Path, key: &str, home: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(base) else {
        return Vec::new();
    };
    let mut stale = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("tree-run-") else {
            continue;
        };
        let owned = key::SHARED
            .iter()
            .map(|(tree, _)| (*tree, key))
            .chain([("home", home)]);
        for (tree, current) in owned {
            let Some(tail) = rest
                .strip_prefix(tree)
                .and_then(|tail| tail.strip_prefix('-'))
            else {
                continue;
            };
            let named = tail.get(..16).unwrap_or("");
            let after = &tail[named.len()..];
            let shaped = after.is_empty() || after.starts_with('-') || after == ".lock";
            if key::is_setup_key(named) && shaped && named != current {
                stale.push(entry.path());
            }
        }
    }
    stale.sort();
    stale
}

/// Whether the trees and the home of `key` are all there, whole, in `base`; the names of those
/// that are not.
fn missing(base: &Path, key: &str, home: &str) -> Vec<String> {
    let mut missing: Vec<String> = key::SHARED
        .iter()
        .filter(|(tree, sentinel)| key::sealed_attempt(base, tree, key, sentinel).is_none())
        .map(|(tree, _)| (*tree).to_owned())
        .collect();
    if !base.join(format!("tree-run-home-{home}")).is_dir() {
        missing.push("home".to_owned());
    }
    missing
}

/// The setup over the repository at `root` in `scope`: computes the key, removes what belongs to
/// another key, builds what is missing of this key's trees and home, writes [`REPORT`], and
/// exports the key to the file `NEXTEST_ENV` names when nextest runs it.
pub fn make(root: &Path, scope: &str) -> Result<Made, String> {
    let started = Instant::now();
    let target = crate::test_step::target_dir(root);
    let base = target.join("tmp");
    let key = tree_key(root, &target, scope)?;
    let home = key::home_key(&key::installed_bytes(&root.join(FIXTURE))?, &key);
    fs::create_dir_all(&base).map_err(|e| format!("create {}: {e}", base.display()))?;
    let stale = of_another_key(&base, &key, &home);
    for path in &stale {
        let removed = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        removed.map_err(|e| format!("remove {}: {e}", path.display()))?;
    }
    let built = missing(&base, &key, &home);
    if !built.is_empty() {
        let mut args: Vec<String> = vec!["test".to_owned(), "--locked".to_owned()];
        args.extend(scope.split_whitespace().map(str::to_owned));
        args.extend(
            [
                "--test",
                BUILDER_TEST.0,
                "--",
                "--ignored",
                "--exact",
                BUILDER_TEST.1,
            ]
            .map(str::to_owned),
        );
        let mut command = crate::test_step::cargo_command(root);
        for name in NOT_INHERITED {
            command.env_remove(name);
        }
        let status = command
            .env(key::KEY_VARIABLE, &key)
            .args(&args)
            .status()
            .map_err(|e| format!("run cargo {}: {e}", args.join(" ")))?;
        if !status.success() {
            return Err(format!(
                "shared trees: cargo {} exited {status}",
                args.join(" ")
            ));
        }
        let still = missing(&base, &key, &home);
        if !still.is_empty() {
            return Err(format!(
                "shared trees: cargo {} passed and left {} unbuilt under key {key}",
                args.join(" "),
                still.join(", ")
            ));
        }
    }
    let made = Made {
        key,
        built,
        removed: stale.len(),
        seconds: started.elapsed().as_secs_f64(),
    };
    let report = target.join("nextest").join(REPORT);
    if let Some(directory) = report.parent() {
        let _ = fs::create_dir_all(directory);
    }
    fs::write(&report, format!("{}\n", made.line()))
        .map_err(|e| format!("write {}: {e}", report.display()))?;
    if let Some(file) = std::env::var_os("NEXTEST_ENV") {
        let mut opened = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&file)
            .map_err(|e| format!("open NEXTEST_ENV {}: {e}", Path::new(&file).display()))?;
        writeln!(opened, "{}={}", key::KEY_VARIABLE, made.key)
            .map_err(|e| format!("write NEXTEST_ENV: {e}"))?;
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// The repository root of this crate.
    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Writes `text` to `root/relative`, making its directory.
    fn put(root: &Path, relative: &str, text: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The files of the fixture of [`the_key_ignores_the_executables`], relative to its root.
    const FILES: [&str; 7] = [
        "crates/loom-cli/tests/fixtures/tree-two-level/tour.yaml",
        "crates/loom-cli/tests/fixtures/tree-two-level/stubs/route.sh",
        "crates/loom-cli/tests/tree_run.rs",
        "crates/loom-cli/tests/tree_run/stop.rs",
        "crates/loom-tree/src/run.rs",
        "crates/loom-cli/src/main.rs",
        "Cargo.lock",
    ];

    /// A scratch repository at `root`: a fixture directory of two files, two builder sources, two
    /// Loom sources, a `Cargo.lock`, and a fake `target/debug/deps/b10x_loom-<hash>` with its
    /// dep-info, which lists the two Loom sources as rustc writes it, beside one that lists a test
    /// source and a build output. No cargo run, no start.
    fn repository_at(root: &Path) {
        for (n, file) in FILES.iter().enumerate() {
            put(root, file, format!("file {n}\n").as_bytes());
        }
        put(root, "target/debug/b10x-loom", b"\x7fELF one build");
        let root = root.canonicalize().unwrap();
        let deps = root.join("target/debug/deps");
        // rustc writes a workspace member's file relative to the workspace root, any other file
        // by its absolute path.
        let dep_info = format!(
            "{deps}/b10x_loom-0123456789abcdef.d: crates/loom-tree/src/run.rs \
             {root}/crates/loom-cli/src/main.rs\n\ncrates/loom-tree/src/run.rs:\n\
             {root}/crates/loom-cli/src/main.rs:\n",
            deps = deps.display(),
            root = root.display()
        );
        put(
            &root,
            "target/debug/deps/b10x_loom-0123456789abcdef.d",
            dep_info.as_bytes(),
        );
        put(
            &root,
            "target/debug/deps/b10x_loom-0123456789abcdef",
            b"\x7fELF one build",
        );
        let other = format!(
            "{deps}/tree_run-fedcba9876543210.d: crates/loom-cli/tests/tree_run.rs \
             {root}/target/debug/build/x/out/made.rs /no-such-registry/src/lib.rs\n",
            deps = deps.display(),
            root = root.display()
        );
        put(
            &root,
            "target/debug/deps/tree_run-fedcba9876543210.d",
            other.as_bytes(),
        );
        put(&root, "target/debug/build/x/out/made.rs", b"made once");
    }

    /// [`tree_key`] of the scratch repository at `root` in `scope`.
    fn key_of(root: &Path, scope: &str) -> String {
        tree_key(root, &root.join("target"), scope).unwrap()
    }

    /// Flips the first byte of `root/relative`.
    fn flip(root: &Path, relative: &str) {
        let path = root.join(relative);
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fs::write(&path, bytes).unwrap();
    }

    #[test]
    fn the_key_ignores_the_executables() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch = crate::test_step::target_dir(&repository())
            .join("tmp")
            .join(format!("shared-trees-key-{}-{nanos}", std::process::id()));
        let (one, two) = (scratch.join("one"), scratch.join("elsewhere/two"));
        repository_at(&one);
        repository_at(&two);

        let key = key_of(&one, DEFAULT_SCOPE);
        assert!(key::is_setup_key(&key), "{key}");
        assert_eq!(key, key_of(&two, DEFAULT_SCOPE), "two copies at two paths");

        // One byte of a fixture file, a builder source, a Loom source the dep-info lists, or
        // `Cargo.lock` changes it, and so does the scope.
        for file in FILES {
            flip(&one, file);
            assert_ne!(key_of(&one, DEFAULT_SCOPE), key, "{file}");
            flip(&one, file);
            assert_eq!(key_of(&one, DEFAULT_SCOPE), key, "{file} restored");
        }
        assert_ne!(key_of(&one, "--workspace"), key, "the scope");

        // The executable's bytes and modification time do not, nor does a build output.
        for executable in [
            "target/debug/b10x-loom",
            "target/debug/deps/b10x_loom-0123456789abcdef",
        ] {
            put(&one, executable, b"\x7fELF another build");
            fs::File::options()
                .write(true)
                .open(one.join(executable))
                .unwrap()
                .set_modified(SystemTime::now() + Duration::from_secs(3600))
                .unwrap();
        }
        put(&one, "target/debug/build/x/out/made.rs", b"made again");
        assert_eq!(key_of(&one, DEFAULT_SCOPE), key, "the executable");
        assert_eq!(
            built_sources(
                &one.canonicalize().unwrap(),
                &one.join("target").canonicalize().unwrap()
            )
            .unwrap()
            .len(),
            2,
            "the two Loom sources, no test source, no build output"
        );

        // The home's key: the protocols' bytes and the tree key, and nothing else.
        let installed = |byte: u8| -> Vec<(&'static str, Vec<u8>)> {
            key::INSTALLED
                .iter()
                .map(|(name, _)| (*name, vec![byte, b'\n']))
                .collect()
        };
        let home = key::home_key(&installed(b'a'), &key);
        assert!(key::is_setup_key(&home), "{home}");
        assert_eq!(
            home,
            key::home_key(&installed(b'a'), &key_of(&two, DEFAULT_SCOPE))
        );
        assert_ne!(home, key::home_key(&installed(b'b'), &key), "a protocol");
        assert_ne!(
            home,
            key::home_key(&installed(b'a'), &key_of(&one, "--workspace")),
            "the tree key"
        );

        // No key reads an executable: not the test binary, not `b10x-loom`.
        let root = repository();
        let tree_run = fs::read_to_string(root.join("crates/loom-cli/tests/tree_run.rs")).unwrap();
        let home_maker = tree_run
            .split_once("fn installed_home(")
            .and_then(|(_, rest)| rest.split_once("\n}\n"))
            .map(|(body, _)| body)
            .expect("fn installed_home in tree_run.rs");
        let keys = fs::read_to_string(root.join("crates/loom-cli/tests/tree_run/key.rs")).unwrap();
        let this = fs::read_to_string(root.join("crates/loom-xtask/src/shared_trees.rs")).unwrap();
        let setup = this
            .split_once("#[cfg(test)]")
            .map_or(this.as_str(), |(code, _)| code);
        for (name, text) in [
            ("installed_home", home_maker),
            ("tree_run/key.rs", keys.as_str()),
            ("shared_trees.rs", setup),
        ] {
            for needle in [concat!("current", "_exe"), concat!("CARGO_BIN", "_EXE")] {
                assert!(
                    !text.contains(needle),
                    "{name} reads an executable: {needle}"
                );
            }
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}
