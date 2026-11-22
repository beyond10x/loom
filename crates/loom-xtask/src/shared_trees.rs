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
    /// Loom sources, a `Cargo.lock`, and a fake `target/debug/b10x-loom` with its dep-info, which
    /// lists the two Loom sources as cargo writes it. No cargo run, no start.
    fn repository_at(root: &Path) {
        for (n, file) in FILES.iter().enumerate() {
            put(root, file, format!("file {n}\n").as_bytes());
        }
        put(root, "target/debug/b10x-loom", b"\x7fELF one build");
        let dep_info = format!(
            "{target}/debug/b10x-loom: {root}/crates/loom-tree/src/run.rs {root}/crates/loom-cli/src/main.rs\n",
            target = root.join("target").display(),
            root = root.display()
        );
        put(root, "target/debug/b10x-loom.d", dep_info.as_bytes());
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

        // The executable's bytes and modification time do not.
        put(&one, "target/debug/b10x-loom", b"\x7fELF another build");
        fs::File::options()
            .write(true)
            .open(one.join("target/debug/b10x-loom"))
            .unwrap()
            .set_modified(SystemTime::now() + Duration::from_secs(3600))
            .unwrap();
        assert_eq!(key_of(&one, DEFAULT_SCOPE), key, "the executable");

        // The home's key: the protocols' bytes and the tree key, and nothing else.
        let installed = |byte: u8| -> Vec<(&'static str, Vec<u8>)> {
            key::INSTALLED
                .iter()
                .map(|(name, _)| (*name, vec![byte, b'\n']))
                .collect()
        };
        let home = key::home_key(&installed(b'a'), &key);
        assert!(key::is_setup_key(&home), "{home}");
        assert_eq!(home, key::home_key(&installed(b'a'), &key_of(&two, DEFAULT_SCOPE)));
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
        let setup = this.split_once("#[cfg(test)]").map_or(this.as_str(), |(code, _)| code);
        for (name, text) in [
            ("installed_home", home_maker),
            ("tree_run/key.rs", keys.as_str()),
            ("shared_trees.rs", setup),
        ] {
            for needle in [concat!("current", "_exe"), concat!("CARGO_BIN", "_EXE")] {
                assert!(!text.contains(needle), "{name} reads an executable: {needle}");
            }
        }
        let _ = fs::remove_dir_all(&scratch);
    }
}
