//! Acceptance for `story:tree-library-exports-what-an-engine-needs`: an engine that depends on
//! `b10x-loom-tree` alone opens, runs, approves, closes and reads a tree, under Substrate on a
//! delegated cgroup root it holds, in a delegated scope of its own or unconfined, and cancels a
//! run from another thread; the root is held once, by the runner and every clone of it, and no
//! ancestor is ever discovered.
//!
//! Each test starts `embedder` (`src/main.rs`) as child processes. The delegated root comes from
//! the CLI's fixture `crates/loom-cli/tests/fixtures/tree-delegated/`: `scope.sh` holds a scope of
//! `systemd-run --user --scope -p Delegate=yes` and prints its directory, and `enter.sh` moves an
//! `embedder` into a leaf of it. A test that needs bubblewrap and a user systemd manager that
//! starts a delegated scope prints `skipped: missing prerequisite: …` without them, unless
//! `LOOM_REQUIRE_DELEGATION` is set: then it fails. The stubs are POSIX sh. Scratch is under
//! `CARGO_TARGET_TMPDIR`; no test reaches the network or a model, and each ends within 60 s.

use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_tree::UNCONFINED_FLAG_REASON;

const EMBEDDER: &str = env!("CARGO_BIN_EXE_embedder");

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixture");

const DELEGATED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../loom-cli/tests/fixtures/tree-delegated"
);

/// Set, a missing prerequisite fails the test instead of skipping it.
const REQUIRE: &str = "LOOM_REQUIRE_DELEGATION";

/// The flags of a delegated scope of the user's systemd manager.
const SCOPE: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];

// ---------------------------------------------------------------------------------------------
// The package

/// The lines of the `[name]` table of the manifest `text`, comments and blank lines left out;
/// `None` when the table is absent.
fn table<'a>(text: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let header = format!("[{name}]");
    let mut lines = text.lines().skip_while(|line| line.trim() != header);
    lines.next()?;
    Some(
        lines
            .take_while(|line| !line.trim_start().starts_with('['))
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect(),
    )
}

#[test]
fn the_embedder_fixture_depends_on_b10x_loom_tree_only() {
    let manifest = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let dependencies = table(&manifest, "dependencies").expect("a [dependencies] table");
    assert_eq!(
        dependencies,
        [r#"b10x-loom-tree = { path = "../.." }"#],
        "{manifest}"
    );
    for empty in ["dev-dependencies", "build-dependencies"] {
        assert!(
            table(&manifest, empty).is_none_or(|lines| lines.is_empty()),
            "[{empty}] holds something: {manifest}"
        );
    }
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim_start().starts_with("[target.")),
        "a [target.*] table: {manifest}"
    );

    // The lock's entry of this package names b10x-loom-tree alone.
    let lock = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../Cargo.lock"
    ))
    .unwrap();
    let entry: Vec<&str> = lock
        .split("[[package]]")
        .filter(|entry| {
            entry
                .lines()
                .any(|line| line.trim() == r#"name = "b10x-loom-tree-embedder-fixture""#)
        })
        .collect();
    assert_eq!(entry.len(), 1, "one lock entry of the fixture");
    let named: Vec<&str> = entry[0]
        .lines()
        .skip_while(|line| !line.starts_with("dependencies = ["))
        .skip(1)
        .take_while(|line| line.trim() != "]")
        .map(|line| line.trim().trim_end_matches(',').trim_matches('"'))
        .collect();
    assert_eq!(named, ["b10x-loom-tree"], "{}", entry[0]);
}

// ---------------------------------------------------------------------------------------------
// The processes

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("embedder-{label}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A tree's store, not made yet, and its workspace, holding `cell.txt` and `probe.sh`, the stub
/// named `stub` of the fixture.
struct Tree {
    store: PathBuf,
    workspace: PathBuf,
}

impl Tree {
    fn new(label: &str, stub: &str) -> Self {
        let dir = scratch(label);
        let workspace = dir.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("cell.txt"), "one cell\n").unwrap();
        fs::copy(Path::new(FIXTURE).join(stub), workspace.join("probe.sh")).unwrap();
        Self {
            store: dir.join("store"),
            workspace,
        }
    }

    /// The blobs of the programs the run started: those its log's `ActionFinished` lines name
    /// (the store keeps its protocols as blobs too).
    fn blobs(&self) -> Vec<String> {
        let log = self.store.join(b10x_loom_tree::log_file_name("root"));
        let Ok(log) = fs::read_to_string(log) else {
            return Vec::new();
        };
        log.lines()
            .filter(|line| line.contains(r#""kind":"ActionFinished""#))
            .filter_map(|line| line.split(r#""blob_digest":""#).nth(1))
            .map(|rest| rest.split('"').next().unwrap())
            .map(|digest| fs::read_to_string(self.store.join("blobs").join(digest)).unwrap())
            .collect()
    }

    /// The cgroup the one blob of the run names, under `/sys/fs/cgroup`.
    fn stub_cgroup(&self) -> PathBuf {
        let blobs = self.blobs();
        assert_eq!(blobs.len(), 1, "one blob: {blobs:?}");
        let path = blobs[0]
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .unwrap_or_else(|| panic!("no cgroup v2 line: {}", blobs[0]));
        Path::new("/sys/fs/cgroup").join(path.trim().trim_start_matches('/'))
    }
}

/// `embedder` with `mode` on `tree`, moved into the cgroup `cgroup` first when given.
fn embedder(cgroup: Option<&Path>, mode: &[&str], tree: Option<&Tree>) -> Command {
    let mut command = match cgroup {
        Some(cgroup) => {
            let mut command = Command::new("sh");
            command
                .arg(Path::new(DELEGATED).join("enter.sh"))
                .arg(cgroup)
                .arg(EMBEDDER);
            command
        }
        None => Command::new(EMBEDDER),
    };
    command.args(mode);
    if let Some(tree) = tree {
        command
            .arg("--store")
            .arg(&tree.store)
            .arg("--workspace")
            .arg(&tree.workspace);
    }
    command.env_remove("RUST_LOG");
    command
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The value of the first `key value` line of `output` for `key`.
fn said<'a>(output: &'a str, key: &str) -> Option<&'a str> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(' '))
}

/// `output` ended 0, the root closed through `kept`.
fn closed(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}{}",
        stdout(output),
        stderr(output)
    );
    let text = stdout(output);
    assert_eq!(said(&text, "closed"), Some("kept"), "{text}");
    assert_eq!(said(&text, "case root outcome"), Some("kept"), "{text}");
}

/// `output` ended non-zero, refused `ConfinementRefused` saying each of `named`.
fn refused(output: &Output, named: &[&str]) {
    let text = stderr(output);
    assert_ne!(output.status.code(), Some(0), "{}{text}", stdout(output));
    assert!(text.contains("refused ConfinementRefused"), "{text}");
    for name in named {
        assert!(text.contains(name), "{name}: {text}");
    }
}

/// Skips the test naming `what`, or fails it when [`REQUIRE`] is set.
fn missing(what: &str) -> bool {
    assert!(
        std::env::var_os(REQUIRE).is_none(),
        "{REQUIRE} is set and a prerequisite is missing: {what}"
    );
    eprintln!("skipped: missing prerequisite: {what}");
    false
}

/// Whether bubblewrap is here and a delegated scope starts.
fn delegation_present() -> bool {
    if !Path::new("/usr/bin/bwrap").is_file() {
        return missing("executable /usr/bin/bwrap");
    }
    match Command::new("systemd-run").args(SCOPE).arg("true").output() {
        Ok(output) if output.status.success() => true,
        Ok(output) => missing(&format!(
            "a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr(&output).trim()
        )),
        Err(error) => missing(&format!("systemd-run ({error})")),
    }
}

/// A delegated scope a test holds: `scope.sh` waits in `keeper/` until it is dropped.
struct Scope {
    root: PathBuf,
    keeper: Child,
}

impl Scope {
    /// A fresh delegated scope, cpu, memory and pids handed to its children, into which this
    /// test may move a process; `None` when a prerequisite is missing.
    fn start() -> Option<Self> {
        if !delegation_present() {
            return None;
        }
        let mut keeper = Command::new("systemd-run")
            .args(SCOPE)
            .arg("sh")
            .arg(Path::new(DELEGATED).join("scope.sh"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(keeper.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let root = PathBuf::from(line.trim());
        assert!(
            root.starts_with("/sys/fs/cgroup") && root.join("keeper").is_dir(),
            "scope.sh printed no scope: {line:?}"
        );
        let scope = Self { root, keeper };
        let probe = Command::new("sh")
            .arg(Path::new(DELEGATED).join("enter.sh"))
            .arg(scope.root.join("probe"))
            .arg("true")
            .output()
            .unwrap();
        let _ = fs::remove_dir(scope.root.join("probe"));
        if !probe.status.success() {
            missing(&format!(
                "a test process that may move a process into a scope of the user's manager ({})",
                stderr(&probe).trim()
            ));
            return None;
        }
        Some(scope)
    }

    /// Every `substrate-` cgroup at any depth under the scope.
    fn launches(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("substrate-")
                {
                    found.push(path.clone());
                }
                walk(&path, found);
            }
        }
        let mut found = Vec::new();
        walk(&self.root, &mut found);
        found
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        drop(self.keeper.stdin.take());
        let _ = self.keeper.wait();
    }
}

/// An `embedder` the test started and talks to, killed when the test ends before it did.
struct Running {
    child: Option<Child>,
    lines: BufReader<ChildStdout>,
}

impl Running {
    fn spawn(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap());
        Self {
            child: Some(child),
            lines,
        }
    }

    /// Reads standard output until the line `wanted`; fails at its end or after 30 s.
    fn expect(&mut self, wanted: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut seen = String::new();
        loop {
            assert!(
                Instant::now() < deadline,
                "no {wanted:?} after 30 s: {seen}"
            );
            let mut line = String::new();
            let read = self.lines.read_line(&mut line).unwrap();
            assert!(read > 0, "the embedder ended before {wanted:?}: {seen}");
            if line.trim_end() == wanted {
                return;
            }
            seen.push_str(&line);
        }
    }

    fn write_line(&mut self) {
        let child = self.child.as_mut().unwrap();
        writeln!(child.stdin.as_mut().unwrap()).unwrap();
    }

    fn alive(&mut self) -> bool {
        self.child.as_mut().unwrap().try_wait().unwrap().is_none()
    }

    /// Closes its standard input and waits for its end.
    fn finish(&mut self) -> std::process::ExitStatus {
        let mut child = self.child.take().unwrap();
        drop(child.stdin.take());
        child.wait().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A held root

#[test]
fn an_embedded_tree_runs_under_the_root_it_holds() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let tree = Tree::new("held", "probe.sh");
    let root = scope.root.display().to_string();
    let ran = embedder(
        Some(&scope.root.join("run")),
        &["--cgroup-root", &root],
        Some(&tree),
    )
    .output()
    .unwrap();
    closed(&ran);
    let cgroup = tree.stub_cgroup();
    assert!(
        cgroup.starts_with(&scope.root) && cgroup != scope.root,
        "the stub ran in {}, not under {root}",
        cgroup.display()
    );
}

/// The embedder two levels below the root it holds (`root/a/leaf`): the program runs under the
/// root, never under `a`, the empty delegated ancestor nearest the process that a runner
/// discovering its root takes.
#[test]
fn an_embedded_tree_two_levels_below_its_root_runs_under_the_root() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let tree = Tree::new("held-deep", "probe.sh");
    let root = scope.root.display().to_string();
    let between = scope.root.join("a");
    let ran = embedder(
        Some(&between.join("leaf")),
        &["--cgroup-root", &root],
        Some(&tree),
    )
    .output()
    .unwrap();
    closed(&ran);
    let cgroup = tree.stub_cgroup();
    assert!(
        cgroup.starts_with(&scope.root) && cgroup != scope.root && !cgroup.starts_with(&between),
        "the stub ran in {}, not under {root} beside {}",
        cgroup.display(),
        between.display()
    );
}

#[test]
fn a_held_root_is_refused_to_a_second_embedder() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let root = scope.root.display().to_string();
    let mut first = Running::spawn(embedder(
        Some(&scope.root.join("run-1")),
        &["--hold-clone", "--cgroup-root", &root],
        None,
    ));
    first.expect("clone-held");

    let second = Tree::new("second", "probe.sh");
    let output = embedder(
        Some(&scope.root.join("run-2")),
        &["--cgroup-root", &root],
        Some(&second),
    )
    .output()
    .unwrap();
    refused(&output, &[&root, "held by another process"]);
    assert!(
        second.blobs().is_empty(),
        "the refused embedder ran a program"
    );

    assert!(first.finish().success());
    let third = Tree::new("third", "probe.sh");
    closed(
        &embedder(
            Some(&scope.root.join("run-3")),
            &["--cgroup-root", &root],
            Some(&third),
        )
        .output()
        .unwrap(),
    );
}

#[test]
fn a_runner_clone_keeps_the_hold() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let root = scope.root.display().to_string();
    let mut first = Running::spawn(embedder(
        Some(&scope.root.join("run-1")),
        &["--hold-clone", "--cgroup-root", &root],
        None,
    ));
    // The TreeRunner is dropped; a clone of its runner lives.
    first.expect("clone-held");
    let second = Tree::new("clone-second", "probe.sh");
    let output = embedder(
        Some(&scope.root.join("run-2")),
        &["--cgroup-root", &root],
        Some(&second),
    )
    .output()
    .unwrap();
    refused(&output, &[&root, "held by another process"]);

    // The clone dropped, the root is free while the first process still lives.
    first.write_line();
    first.expect("clone-dropped");
    let third = Tree::new("clone-third", "probe.sh");
    closed(
        &embedder(
            Some(&scope.root.join("run-3")),
            &["--cgroup-root", &root],
            Some(&third),
        )
        .output()
        .unwrap(),
    );
    assert!(
        first.alive(),
        "the first embedder ended before the third ran"
    );
    assert!(first.finish().success());
}

#[test]
fn a_root_is_held_once_per_process() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let root = scope.root.display().to_string();
    let output = embedder(
        Some(&scope.root.join("run")),
        &["--hold-twice", "--cgroup-root", &root],
        None,
    )
    .output()
    .unwrap();
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}{}", stderr(&output));
    let second = said(&text, "second-refused")
        .unwrap_or_else(|| panic!("a second runner on the root was not refused: {text}"));
    assert!(second.starts_with("ConfinementRefused"), "{second}");
    assert!(second.contains(&root), "{second}");
}

#[test]
fn an_unqualified_root_is_refused_and_no_ancestor_is_used() {
    let Some(scope) = Scope::start() else {
        return;
    };
    // A process in the root, the embedder in a child of it: not empty. The scope above it is an
    // empty delegated ancestor a discovering runner would take.
    let busy = scope.root.join("busy");
    let mut sleeper = Running::spawn({
        let mut command = Command::new("sh");
        command
            .arg(Path::new(DELEGATED).join("enter.sh"))
            .arg(&busy)
            .args(["sleep", "50"]);
        command
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while fs::read_to_string(busy.join("cgroup.procs"))
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        assert!(sleeper.alive(), "the sleeper ended");
        assert!(Instant::now() < deadline, "the sleeper never entered");
        std::thread::sleep(Duration::from_millis(20));
    }
    let not_empty = Tree::new("not-empty", "probe.sh");
    let output = embedder(
        Some(&busy.join("run")),
        &["--cgroup-root", &busy.display().to_string()],
        Some(&not_empty),
    )
    .output()
    .unwrap();
    refused(&output, &[&busy.display().to_string(), "is not empty"]);
    assert!(not_empty.blobs().is_empty(), "the stub ran");
    drop(sleeper);

    // Writable and empty, but its parent hands it no controller: not delegated.
    let bare = scope.root.join("plain/bare");
    fs::create_dir_all(&bare).unwrap();
    let not_delegated = Tree::new("not-delegated", "probe.sh");
    let output = embedder(
        Some(&bare.join("run")),
        &["--cgroup-root", &bare.display().to_string()],
        Some(&not_delegated),
    )
    .output()
    .unwrap();
    refused(&output, &[&bare.display().to_string(), "is not delegated"]);
    assert!(not_delegated.blobs().is_empty(), "the stub ran");

    assert_eq!(
        scope.launches(),
        Vec::<PathBuf>::new(),
        "a launch ran under the scope"
    );
}

#[test]
fn in_own_scope_moves_every_thread_into_a_leaf() {
    if !delegation_present() {
        return;
    }
    // A delegated scope of the user's manager with the embedder as its one process, as an
    // operator starts one; no enter.sh.
    let tree = Tree::new("own-scope", "probe.sh");
    let mut command = Command::new("systemd-run");
    command.args(SCOPE).arg(EMBEDDER).arg("--own-scope");
    command
        .arg("--store")
        .arg(&tree.store)
        .arg("--workspace")
        .arg(&tree.workspace);
    let ran = command.output().unwrap();
    closed(&ran);

    let text = stdout(&ran);
    let tasks: Vec<PathBuf> = text
        .lines()
        .filter_map(|line| line.strip_prefix("task-cgroup "))
        .map(|rest| {
            let path = rest.split_once(' ').unwrap().1;
            Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'))
        })
        .collect();
    assert!(tasks.len() >= 2, "two threads or more: {text}");
    let leaf = &tasks[0];
    assert!(
        tasks.iter().all(|task| task == leaf),
        "every thread in one leaf: {tasks:?}"
    );
    assert!(
        leaf.file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("loom-controller-")),
        "{}",
        leaf.display()
    );
    let scope = leaf.parent().unwrap();
    assert!(
        scope
            .extension()
            .is_some_and(|extension| extension == "scope"),
        "{} is no scope",
        scope.display()
    );
    let cgroup = tree.stub_cgroup();
    assert!(
        cgroup.starts_with(scope) && !cgroup.starts_with(leaf),
        "the stub ran in {}, not under {}",
        cgroup.display(),
        scope.display()
    );
}

// ---------------------------------------------------------------------------------------------
// Unconfined

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn local_supplies_the_unconfined_reason() {
    let tree = Tree::new("local", "probe.sh");
    let ran = embedder(None, &["--local"], Some(&tree)).output().unwrap();
    closed(&ran);
    let text = stdout(&ran);
    assert_eq!(said(&text, "local-reason"), Some(UNCONFINED_FLAG_REASON));
    let blobs = tree.blobs();
    assert_eq!(blobs.len(), 1, "{blobs:?}");
    assert!(blobs[0].contains("0::"), "{}", blobs[0]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_embedder_cancels_a_running_tree() {
    let tree = Tree::new("cancel", "sleep.sh");
    let started = Instant::now();
    let ran = embedder(None, &["--local", "--cancel-after-ms", "1000"], Some(&tree))
        .output()
        .unwrap();
    let took = started.elapsed();
    let text = stdout(&ran);
    assert_eq!(ran.status.code(), Some(0), "{text}{}", stderr(&ran));
    let stops: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("stop "))
        .collect();
    assert_eq!(stops, ["AwaitingAuthority", "Cancelled"], "{text}");
    let returned: u64 = said(&text, "returned-after-cancel-ms")
        .unwrap_or_else(|| panic!("{text}"))
        .parse()
        .unwrap();
    assert!(returned <= 10_000, "returned {returned} ms after the flag");
    assert!(took < Duration::from_secs(25), "the embedder took {took:?}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_default_principal_is_the_approve_decisions_principal() {
    let tree = Tree::new("principal", "probe.sh");
    let ran = embedder(None, &["--local"], Some(&tree)).output().unwrap();
    closed(&ran);
    let text = stdout(&ran);
    assert_eq!(
        said(&text, "decision-principal"),
        Some("operator"),
        "{text}"
    );
    assert_eq!(said(&text, "default-principal"), Some("operator"), "{text}");
    assert_eq!(said(&text, "approved"), Some("cell.look"), "{text}");
    // The approval recorded the default principal, and the run went on past it.
    let log = fs::read_to_string(tree.store.join(b10x_loom_tree::log_file_name("root"))).unwrap();
    assert!(
        log.lines()
            .any(|line| line.contains(r#""kind":"AuthorityRecorded""#)
                && line.contains(r#""principal":"operator""#)),
        "{log}"
    );
    let stops: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("stop "))
        .collect();
    assert_eq!(stops, ["AwaitingAuthority", "AwaitingPerson"], "{text}");
}

// ---------------------------------------------------------------------------------------------
// The guide

/// The guide's "Run a case tree" example is this fixture's code: every line of its Rust block is a
/// line of `src/main.rs`.
#[test]
fn the_guides_tree_example_is_the_fixtures_code() {
    let guide = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../website/docs/guides/embed-the-runtime.md"
    ))
    .unwrap();
    let section: String = guide
        .lines()
        .skip_while(|line| line.trim() != "## Run a case tree")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n");
    let block: Vec<&str> = section
        .lines()
        .skip_while(|line| line.trim() != "```rust")
        .skip(1)
        .take_while(|line| line.trim() != "```")
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "// …")
        .collect();
    assert!(
        block.len() > 10,
        "the guide has a Rust block under `## Run a case tree`"
    );
    let source = include_str!("../src/main.rs");
    let lines: Vec<&str> = source.lines().map(str::trim).collect();
    for line in block {
        assert!(lines.contains(&line), "not a line of src/main.rs: {line}");
    }
}
