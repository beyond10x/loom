//! Adversary pass 2 on `story:agent-executor-port`: the dependency guard where pass 1 did not
//! look — `cargo tree` failing, the listing's `(*)` and `(proc-macro)` suffixes — and the
//! `Taskfile.yml` wiring that `story:commission-ess-conformance` edits two waves later.
//!
//! The guard runs as pass 1 runs it: the compiled `executor_port` test binary beside this one,
//! with `CARGO_MANIFEST_DIR` pointed into a fixture workspace, so it runs its own `cargo tree`
//! there. Fixture crates are local path crates; nothing is fetched.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The newest `executor_port-<hash>` test binary beside this one, refused when older than its
/// source or when it does not list `executor_port_contract`.
fn guard_binary() -> PathBuf {
    let me = std::env::current_exe().unwrap_or_else(|error| panic!("current_exe: {error}"));
    let deps = me
        .parent()
        .unwrap_or_else(|| panic!("{} has no parent", me.display()));
    let newest = std::fs::read_dir(deps)
        .unwrap_or_else(|error| panic!("read {}: {error}", deps.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_none()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("executor_port-"))
        })
        .filter_map(|path| Some((path.metadata().ok()?.modified().ok()?, path)))
        .max()
        .unwrap_or_else(|| {
            panic!(
                "no executor_port test binary in {}: build it with \
                 `cargo test -p b10x-commission-testkit --no-run`",
                deps.display()
            )
        });
    let source = root().join("crates/commission-testkit/tests/executor_port.rs");
    let source_modified = source
        .metadata()
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| panic!("stat {}: {error}", source.display()));
    assert!(
        newest.0 >= source_modified,
        "{} is older than {}: rebuild it",
        newest.1.display(),
        source.display()
    );
    let listed = Command::new(&newest.1)
        .arg("--list")
        .output()
        .unwrap_or_else(|error| panic!("run {} --list: {error}", newest.1.display()));
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains("executor_port_contract: test"),
        "{} does not list executor_port_contract",
        newest.1.display()
    );
    newest.1
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// A fixture workspace laid out like this repository, without a lock file: `crates/commission`
/// is `b10x-commission` with `manifest_tail` appended, each `(name, tail)` in `stubs` is a path
/// crate under `stubs/`, and the real deny list sits at the root.
fn fixture(case: &str, manifest_tail: &str, stubs: &[(&str, &str)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary2-executor-guard")
        .join(case);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .unwrap_or_else(|error| panic!("clear {}: {error}", dir.display()));
    }
    let mut members = vec!["\"crates/commission\"".to_owned()];
    for (name, tail) in stubs {
        members.push(format!("\"stubs/{name}\""));
        write(
            &dir.join("stubs").join(name).join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{tail}"
            ),
        );
        write(&dir.join("stubs").join(name).join("src/lib.rs"), "");
    }
    write(
        &dir.join("Cargo.toml"),
        &format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    );
    write(
        &dir.join("crates/commission/Cargo.toml"),
        &format!(
            "[package]\nname = \"b10x-commission\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             {manifest_tail}"
        ),
    );
    write(&dir.join("crates/commission/src/lib.rs"), "");
    std::fs::create_dir_all(dir.join("crates/commission-testkit"))
        .unwrap_or_else(|error| panic!("create testkit dir: {error}"));
    std::fs::copy(
        root().join("model-provider-deny.txt"),
        dir.join("model-provider-deny.txt"),
    )
    .unwrap_or_else(|error| panic!("copy deny list: {error}"));
    dir
}

fn lock(dir: &Path) {
    let out = Command::new(cargo())
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(dir)
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap_or_else(|error| panic!("run cargo generate-lockfile: {error}"));
    assert!(
        out.status.success(),
        "fixture {} does not resolve:\n{}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Runs the real guard against `dir`: `(passed, its output)`.
fn run_guard(dir: &Path) -> (bool, String) {
    let out = Command::new(guard_binary())
        .args(["executor_port_contract", "--exact", "--test-threads=1"])
        .env("CARGO_MANIFEST_DIR", dir.join("crates/commission-testkit"))
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap_or_else(|error| panic!("run the guard: {error}"));
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// The guard must not pass a tree it could not list. A manifest dependency the lock file does not
/// carry makes `cargo tree --locked` exit non-zero with an empty listing; the guard has to fail and
/// say `cargo tree` failed, not report "no refused crate" over nothing.
#[test]
fn adversary2_guard_fails_loudly_when_the_lock_file_misses_an_entry() {
    let dir = fixture("stale-lock", "", &[("glue", "")]);
    lock(&dir);
    // Add a dependency after locking: the lock file now misses its entry.
    let manifest = dir.join("crates/commission/Cargo.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap_or_default();
    text.push_str("[dependencies]\nglue = { path = \"../../stubs/glue\" }\n");
    write(&manifest, &text);

    let (passed, output) = run_guard(&dir);
    assert!(
        !passed,
        "the guard passed a tree whose lock file misses an entry:\n{output}"
    );
    assert!(
        output.contains("`cargo tree` failed"),
        "the guard failed, but not on the `cargo tree` failure:\n{output}"
    );
}

/// With no lock file at all, `cargo tree --locked` cannot run; the guard fails, loudly.
#[test]
fn adversary2_guard_fails_loudly_without_a_lock_file() {
    let dir = fixture("no-lock", "", &[]);
    let (passed, output) = run_guard(&dir);
    assert!(
        !passed,
        "the guard passed a tree with no lock file:\n{output}"
    );
    assert!(
        output.contains("`cargo tree` failed"),
        "the guard failed, but not on the `cargo tree` failure:\n{output}"
    );
}

/// A denied crate that is a proc-macro, reached on two paths: `cargo tree --prefix none` prints it
/// with `(proc-macro)` and, the second time, `(*)`. The guard still names it.
#[test]
fn adversary2_guard_refuses_a_deduplicated_proc_macro_provider() {
    const PROVIDER: &str = "async-openai";
    let path_dep = |from: &str| format!("{PROVIDER} = {{ path = \"{from}{PROVIDER}\" }}\n");
    let dir = fixture(
        "proc-macro",
        "[dependencies]\nglue-a = { path = \"../../stubs/glue-a\" }\n\
         glue-b = { path = \"../../stubs/glue-b\" }\n",
        &[
            ("leaf", ""),
            (
                PROVIDER,
                "\n[lib]\nproc-macro = true\n\n[dependencies]\nleaf = { path = \"../leaf\" }\n",
            ),
            ("glue-a", &format!("\n[dependencies]\n{}", path_dep("../"))),
            ("glue-b", &format!("\n[dependencies]\n{}", path_dep("../"))),
        ],
    );
    lock(&dir);
    let listing = Command::new(cargo())
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(&dir)
        .args([
            "tree",
            "--offline",
            "-p",
            "b10x-commission",
            "-e",
            "normal",
            "--all-features",
            "--target",
            "all",
            "--prefix",
            "none",
        ])
        .output()
        .unwrap_or_else(|error| panic!("run cargo tree: {error}"));
    let listing = String::from_utf8_lossy(&listing.stdout).into_owned();
    assert!(
        listing.lines().any(|line| line.starts_with(PROVIDER)
            && line.contains("(proc-macro)")
            && line.ends_with("(*)")),
        "fixture: no deduplicated proc-macro line for {PROVIDER}:\n{listing}"
    );

    let (passed, output) = run_guard(&dir);
    assert!(
        !passed && output.contains("depends on refused crates") && output.contains(PROVIDER),
        "the guard did not refuse the proc-macro {PROVIDER}:\n{output}"
    );
}

/// The lines of one top-level task's block in `Taskfile.yml`, up to the next task.
fn task_block(taskfile: &str, task: &str) -> Vec<String> {
    let header = format!("  {task}:");
    let mut lines = taskfile.lines().skip_while(|line| *line != header);
    assert!(lines.next().is_some(), "Taskfile.yml has no task `{task}`");
    lines
        .take_while(|line| line.is_empty() || line.starts_with("    "))
        .map(|line| line.trim().to_owned())
        .collect()
}

/// `task check` runs `deps-guard` as its own step, and `deps-guard` runs exactly the story's
/// command. `Taskfile.yml` is shared with `story:commission-ess-conformance`; nothing else holds
/// this wiring in place.
#[test]
fn adversary2_taskfile_check_runs_deps_guard_with_the_story_command() {
    let path = root().join("Taskfile.yml");
    let taskfile = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let check = task_block(&taskfile, "check");
    assert!(
        check.iter().any(|line| line == "- task: deps-guard"),
        "`task check` does not list deps-guard:\n{check:#?}"
    );
    let guard = task_block(&taskfile, "deps-guard");
    let commands: Vec<&String> = guard
        .iter()
        .skip_while(|line| *line != "cmds:")
        .skip(1)
        .take_while(|line| line.starts_with("- "))
        .collect();
    assert_eq!(
        commands,
        vec!["- cargo test --locked -p b10x-commission-testkit --test executor_port"],
        "`task deps-guard` does not run the story's command"
    );
}
