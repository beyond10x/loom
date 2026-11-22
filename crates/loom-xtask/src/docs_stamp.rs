//! `docs-stamp`: the fast check of the generated documentation (`task docs-check`) and the
//! decision whether CI runs the slow lane that regenerates it (`task docs-slow`).

#[cfg(test)]
mod tests {
    use crate::stamp;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    };
    use syn::visit::Visit;

    /// The version line the stub `ess` of a scratch copy prints.
    const STUB_VERSION: &str = "ess 0.96.0 (stub)";
    /// What every failure of the check tells the reader to run.
    const HINT: &str = "run `task docs-generate`";
    /// The variable that carries a child's `loom-xtask` arguments, one per line.
    const CHILD: &str = "LOOM_XTASK_DOCS_STAMP_CHILD";

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A fresh directory under `target/loom-xtask/`, named for the test, the process and the time.
    fn scratch(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = crate::default_scratch(&repository())
            .join(format!("docs-stamp-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A directory that holds only a stub `ess`, a POSIX sh script printing `version`.
    fn stub_ess(dir: &Path, version: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(dir).unwrap();
        let ess = dir.join("ess");
        fs::write(&ess, format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n")).unwrap();
        fs::set_permissions(&ess, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A scratch copy of the repository's documentation inputs and generated files, with a stamp
    /// that matches them, and a `PATH` that holds only a stub `ess` printing `version`.
    struct Copy {
        root: PathBuf,
        path: PathBuf,
    }

    fn copy(name: &str, version: &str) -> Copy {
        let base = scratch(name);
        let root = base.join("repo");
        let repository = repository();
        let inputs = stamp::input_files(&repository).unwrap();
        let outputs = stamp::output_files(&repository).unwrap();
        for relative in inputs.keys().chain(outputs.keys()) {
            let to = root.join(relative);
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            fs::copy(repository.join(relative), &to).unwrap();
        }
        let stamped = stamp::Stamp::new(
            stamp::input_files(&root).unwrap(),
            STUB_VERSION,
            stamp::output_files(&root).unwrap(),
        );
        stamp::write(&root, &stamped).unwrap();
        let path = base.join("bin");
        stub_ess(&path, version);
        Copy { root, path }
    }

    /// `loom-xtask <args>` in a child process whose environment holds only `PATH`.
    fn xtask(args: &[&str], path: &Path) -> Output {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "docs_stamp::tests::child",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("PATH", path)
            .env(CHILD, args.join("\n"))
            .output()
            .unwrap()
    }

    fn check(copy: &Copy) -> Output {
        let root = copy.root.to_str().unwrap();
        xtask(&["--root", root, "docs-stamp", "--check"], &copy.path)
    }

    fn text(output: &Output) -> String {
        format!(
            "exit {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    /// The child the tests start: runs `loom-xtask` with the arguments `CHILD` names and exits
    /// with its status. Does nothing when `CHILD` is not set.
    #[test]
    #[ignore = "the child process of the docs-stamp tests; does nothing on its own"]
    fn child() {
        let Ok(args) = std::env::var(CHILD) else {
            return;
        };
        let args = std::iter::once("loom-xtask").chain(args.split('\n'));
        let code = match <crate::Cli as clap::Parser>::try_parse_from(args) {
            Err(error) => {
                eprintln!("{error}");
                2
            }
            Ok(cli) => match crate::run(cli) {
                Ok(()) => 0,
                Err(message) => {
                    eprintln!("{message}");
                    1
                }
            },
        };
        std::process::exit(code);
    }

    #[test]
    fn the_check_passes_on_an_unchanged_tree_and_builds_nothing() {
        let copy = copy("unchanged", STUB_VERSION);
        assert!(!copy.path.join("cargo").exists());
        let output = check(&copy);
        assert!(output.status.success(), "{}", text(&output));
        assert!(
            !copy.root.join("target").exists(),
            "the check made a build directory"
        );
    }

    #[test]
    fn a_changed_input_or_page_fails_naming_it() {
        let change_one_byte = |root: &Path, relative: &str| {
            let path = root.join(relative);
            let mut bytes = fs::read(&path).unwrap();
            let middle = bytes.len() / 2;
            bytes[middle] = bytes[middle].wrapping_add(1);
            fs::write(&path, bytes).unwrap();
        };
        let cases: [(&str, &str, &dyn Fn(&Path)); 5] = [
            ("lib", "crates/loom-cli/src/lib.rs", &|root| {
                change_one_byte(root, "crates/loom-cli/src/lib.rs")
            }),
            ("runlog", "ess/domains/runlog.yaml", &|root| {
                change_one_byte(root, "ess/domains/runlog.yaml")
            }),
            ("cli", "website/docs/reference/cli.md", &|root| {
                change_one_byte(root, "website/docs/reference/cli.md")
            }),
            ("added", "crates/loom-cli/src/added.rs", &|root| {
                fs::write(root.join("crates/loom-cli/src/added.rs"), "//! new\n").unwrap()
            }),
            ("removed", "ess/domains/runlog.yaml", &|root| {
                fs::remove_file(root.join("ess/domains/runlog.yaml")).unwrap()
            }),
        ];
        for (name, named, change) in cases {
            let copy = copy(name, STUB_VERSION);
            change(&copy.root);
            let output = check(&copy);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success() && stderr.contains(named) && stderr.contains(HINT),
                "{name}: the check must fail naming {named} and `{HINT}`:\n{}",
                text(&output)
            );
        }
    }

    #[test]
    fn another_ess_version_fails_the_check() {
        let copy = copy("ess-version", "ess 0.97.3");
        let output = check(&copy);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success()
                && stderr.contains("ess 0.97.3")
                && stderr.contains(STUB_VERSION)
                && stderr.contains(HINT),
            "the check must fail naming both ess versions:\n{}",
            text(&output)
        );
    }

    #[test]
    fn the_check_refuses_a_raw_admonition_title() {
        let copy = copy("admonition", STUB_VERSION);
        let page = copy.root.join("website/docs/concepts/planned.md");
        fs::create_dir_all(page.parent().unwrap()).unwrap();
        fs::write(
            &page,
            "# Planned\n:::caution Planned\nNot built yet.\n:::\n",
        )
        .unwrap();
        let output = check(&copy);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success()
                && stderr.contains("website/docs/concepts/planned.md:2")
                && stderr.contains("raw admonition titles (write `:::kind[Title]`)"),
            "the check must refuse the raw admonition title:\n{}",
            text(&output)
        );
    }

    #[test]
    fn raw_admonition_titles_are_found_and_bracketed_ones_pass() {
        for raw in [
            ":::caution Planned",
            ":::note  These thresholds",
            ":::info Decided design",
        ] {
            assert!(stamp::is_raw_admonition(raw), "{raw}");
        }
        for fine in [
            ":::caution[Planned]",
            ":::",
            ":::note",
            ":::note ",
            "::: note Title",
            ":::Note Title",
            " :::note Title",
        ] {
            assert!(!stamp::is_raw_admonition(fine), "{fine}");
        }

        let dir = scratch("admonitions");
        fs::create_dir_all(dir.join("concepts")).unwrap();
        fs::write(dir.join("index.md"), ":::caution[Bootstrap]\nok\n:::\n").unwrap();
        fs::write(
            dir.join("concepts/a.mdx"),
            "text\n:::caution Planned\n:::\n",
        )
        .unwrap();
        fs::write(dir.join("notes.txt"), ":::caution Planned\n").unwrap();
        let found = stamp::raw_admonitions(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("concepts/a.mdx:2"), "{found:?}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_slow_lane_runs_only_when_needed() {
        let base = scratch("slow-needed");
        let root = base.join("repo");
        let written = root.join(stamp::PATH);
        fs::create_dir_all(written.parent().unwrap()).unwrap();
        fs::write(&written, "{\"stamp\": 1}\n").unwrap();
        let equal = base.join("equal.json");
        fs::write(&equal, "{\"stamp\": 1}\n").unwrap();
        let other = base.join("other.json");
        fs::write(&other, "{\"stamp\": 2}\n").unwrap();
        let absent = base.join("absent.json");
        let path = base.join("bin");
        stub_ess(&path, STUB_VERSION);

        for (event, git_ref, against, expected) in [
            ("pull_request", "refs/pull/7/merge", &equal, "skip"),
            ("pull_request", "refs/pull/7/merge", &other, "run"),
            ("pull_request", "refs/pull/7/merge", &absent, "run"),
            ("push", "refs/heads/main", &equal, "run"),
            ("push", "refs/heads/feature", &equal, "skip"),
        ] {
            let output = xtask(
                &[
                    "--root",
                    root.to_str().unwrap(),
                    "docs-stamp",
                    "--slow-needed",
                    "--base",
                    against.to_str().unwrap(),
                    "--event",
                    event,
                    "--ref",
                    git_ref,
                ],
                &path,
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            let decision = stdout.lines().rev().find(|line| !line.trim().is_empty());
            assert!(
                output.status.success() && decision == Some(expected),
                "{event} {git_ref} against {}: expected `{expected}`:\n{}",
                against.display(),
                text(&output)
            );
        }

        let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &fs::read_to_string(repository().join(".github/workflows/check.yml")).unwrap(),
        )
        .unwrap();
        let steps = workflow["jobs"]["docs-slow"]["steps"]
            .as_sequence()
            .unwrap_or_else(|| panic!("check.yml has no job docs-slow with steps"));
        let decide = steps
            .iter()
            .position(|step| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run.contains("docs-stamp --slow-needed"))
            })
            .unwrap_or_else(|| panic!("job docs-slow runs no `docs-stamp --slow-needed`"));
        let id = steps[decide]["id"]
            .as_str()
            .unwrap_or_else(|| panic!("the decision step of docs-slow has no id"));
        let run = steps[decide]["run"].as_str().unwrap();
        assert!(
            run.contains("decision=") && run.contains("GITHUB_OUTPUT"),
            "the decision step does not write its decision to GITHUB_OUTPUT: {run}"
        );
        let slow: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|(_, step)| {
                step["run"]
                    .as_str()
                    .is_some_and(|run| run.contains("task docs-slow"))
            })
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            slow.len(),
            1,
            "job docs-slow must run `task docs-slow` once"
        );
        let guard = format!("steps.{id}.outputs.decision == 'run'");
        assert!(
            slow[0] > decide && steps[slow[0]]["if"].as_str() == Some(guard.as_str()),
            "`task docs-slow` must run after the decision and only on `if: {guard}`"
        );
    }

    /// Whether `attrs` hold `#[cfg(test)]` or `#[test]`.
    fn test_only(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || matches!(&attr.meta, syn::Meta::List(list)
                    if list.path.is_ident("cfg") && list.tokens.to_string() == "test")
        })
    }

    /// Every direct file read (`fs::read`, `fs::read_to_string`, `fs::read_dir`, `File::open`,
    /// or a `use` of one of them) outside `#[cfg(test)]` and `#[test]` items, by line.
    fn direct_reads(source: &str) -> Vec<usize> {
        struct Scan(Vec<usize>);
        const READS: [&str; 3] = ["read", "read_to_string", "read_dir"];
        impl<'ast> Visit<'ast> for Scan {
            fn visit_item(&mut self, item: &'ast syn::Item) {
                let attrs: &[syn::Attribute] = match item {
                    syn::Item::Mod(item) => &item.attrs,
                    syn::Item::Fn(item) => &item.attrs,
                    syn::Item::Impl(item) => &item.attrs,
                    syn::Item::Use(item) => &item.attrs,
                    syn::Item::Const(item) => &item.attrs,
                    syn::Item::Static(item) => &item.attrs,
                    _ => &[],
                };
                if !test_only(attrs) {
                    syn::visit::visit_item(self, item);
                }
            }
            fn visit_path(&mut self, path: &'ast syn::Path) {
                let names: Vec<String> =
                    path.segments.iter().map(|s| s.ident.to_string()).collect();
                if names.windows(2).any(|pair| {
                    (pair[0] == "fs" && READS.contains(&pair[1].as_str()))
                        || (pair[0] == "File" && pair[1] == "open")
                }) {
                    self.0.push(path.segments[0].ident.span().start().line);
                }
                syn::visit::visit_path(self, path);
            }
            fn visit_use_path(&mut self, used: &'ast syn::UsePath) {
                if used.ident == "fs" {
                    let mut names = Vec::new();
                    let mut trees = vec![&*used.tree];
                    while let Some(tree) = trees.pop() {
                        match tree {
                            syn::UseTree::Name(name) => names.push(&name.ident),
                            syn::UseTree::Rename(rename) => names.push(&rename.ident),
                            syn::UseTree::Group(group) => trees.extend(group.items.iter()),
                            _ => {}
                        }
                    }
                    if names
                        .iter()
                        .any(|name| READS.iter().any(|read| *name == read))
                    {
                        self.0.push(used.ident.span().start().line);
                    }
                }
                syn::visit::visit_use_path(self, used);
            }
        }
        let file = syn::parse_file(source).expect("a source file of loom-docs parses");
        let mut scan = Scan(Vec::new());
        scan.visit_file(&file);
        scan.0
    }

    #[test]
    fn every_file_loom_docs_reads_is_an_input() {
        let sample = "use std::fs;\nfn a() { let _ = fs::read_to_string(\"x\"); }\n\
                      #[cfg(test)]\nmod tests { fn b() { let _ = std::fs::read(\"y\"); } }\n";
        assert_eq!(direct_reads(sample), [2], "the scan misses a direct read");

        let src = repository().join("crates/loom-docs/src");
        let mut found = Vec::new();
        let mut files: Vec<PathBuf> = fs::read_dir(&src)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        assert!(files.len() > 1, "no source files under {}", src.display());
        for file in files {
            if file.file_name().is_some_and(|name| name == "stamp.rs") {
                continue;
            }
            for line in direct_reads(&fs::read_to_string(&file).unwrap()) {
                found.push(format!("{}:{line}", file.display()));
            }
        }
        assert!(
            found.is_empty(),
            "loom-docs reads files outside `stamp::read_input` and `stamp::read_input_dir`, so \
             the stamp may not cover them: {}",
            found.join(", ")
        );
    }
}
