//! Acceptance for `story:tree-run-takes-a-delegated-cgroup`: `tree run` under its default
//! confinement, `substrate`, runs its confined programs under a delegated cgroup root of its own,
//! never one another run uses, and a refusal names `tree run`'s own remedy.
//!
//! The fixture is `fixtures/tree-delegated/`: the protocol `cell@1`, one program action whose
//! stub (`stubs/look.sh`, POSIX sh) keeps the cgroup it ran in, says it started and waits for the
//! test's `target/release`; `scope.sh`, which holds a delegated scope of `systemd-run --user
//! --scope -p Delegate=yes` for a test, and `enter.sh`, which moves a `tree run` into a child of
//! it. A test that needs bubblewrap and a user systemd manager that starts a delegated scope
//! prints `skipped: missing prerequisite: …` without them, unless `LOOM_REQUIRE_DELEGATION` is
//! set: then it fails. No test reaches the network or a model, and each ends within 60 s.

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Stdio};

use super::*;

const DELEGATED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-delegated");

const BINARY: &str = env!("CARGO_BIN_EXE_b10x-loom");

/// Set, a missing prerequisite fails the test instead of skipping it.
const REQUIRE: &str = "LOOM_REQUIRE_DELEGATION";

/// The flags of a delegated scope of the user's systemd manager.
const SCOPE: [&str; 6] = ["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"];

/// The hint `run`'s refusal gives, which names a flag `tree run` did not have.
const RUN_HINT: &str = "use systemd-run --user -p Delegate=yes --scope or --cgroup-root";

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
                "a test process that may move a process into a scope of the user's manager, as under systemd-run --user --scope ({})",
                stderr(&probe).trim()
            ));
            return None;
        }
        Some(scope)
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        drop(self.keeper.stdin.take());
        let _ = self.keeper.wait();
    }
}

/// A child the test started, killed when the test ends before it did.
struct Running(Option<Child>);

impl Running {
    fn spawn(mut command: Command) -> Self {
        Self(Some(
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn child(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }

    /// Waits until `path` exists; fails when the child ends first or after 30 s.
    fn wait_for(&mut self, path: &Path, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.exists() {
            if self.child().try_wait().unwrap().is_some() {
                let ended = self.finish();
                panic!(
                    "{what} ended before {} existed: {:?}\n{}{}",
                    path.display(),
                    ended.status,
                    stdout(&ended),
                    stderr(&ended)
                );
            }
            assert!(
                Instant::now() < deadline,
                "{what}: {} does not exist after 30 s",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn finish(&mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// A tree of one `cell@1` case, opened, its workspace holding `target/`.
fn cell(label: &str) -> Tree {
    let tree = Tree::new(label);
    InstallStore::new(tree.root.join("home/loom/protocols"))
        .install_file("cell@1", &Path::new(DELEGATED).join("cell.yaml"), false)
        .unwrap();
    for file in ["cell.bindings.yaml", "cell.txt", "stubs/look.sh"] {
        fs::copy(Path::new(DELEGATED).join(file), tree.workspace().join(file)).unwrap();
    }
    fs::create_dir_all(tree.workspace().join("target")).unwrap();
    open_with(&tree, "cell@1", "cell.bindings.yaml");
    tree
}

/// Lets the tree's `look.sh` end.
fn release(tree: &Tree) {
    fs::write(tree.workspace().join("target/release"), "").unwrap();
}

fn started(tree: &Tree) -> PathBuf {
    tree.workspace().join("target/started")
}

/// `tree run` of `tree`, under `--cgroup-root root` when given, moved into the cgroup `cgroup`
/// first when given.
fn tree_run(tree: &Tree, cgroup: Option<&Path>, root: Option<&Path>) -> Command {
    let home = tree.root.join("home");
    let mut command = match cgroup {
        Some(cgroup) => {
            let mut command = Command::new("sh");
            command
                .arg(Path::new(DELEGATED).join("enter.sh"))
                .arg(cgroup)
                .arg(BINARY);
            command
        }
        None => Command::new(BINARY),
    };
    command
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &home)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .env_remove("RUST_LOG")
        .args(["tree", "run", "--at", AT, "--store"])
        .arg(tree.store());
    if let Some(root) = root {
        command.arg("--cgroup-root").arg(root);
    }
    command
}

/// The cgroup root the tree's confined launch ran under: the parent of its `substrate-` cgroup.
fn launch_root(tree: &Tree) -> PathBuf {
    let kept = fs::read_to_string(tree.workspace().join("target/cgroup.txt")).unwrap();
    let path = kept
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or_else(|| panic!("no cgroup v2 line: {kept}"));
    let path = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    path.ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("substrate-"))
        })
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("the launch ran in no substrate cgroup: {}", path.display()))
        .to_owned()
}

/// `output` ended 0 with the root terminated.
fn terminated(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}{}",
        stdout(output),
        stderr(output)
    );
    assert_eq!(json(output)["stop"], "Terminal", "{}", stdout(output));
}

/// `output` refused `ConfinementRefused` with exit 1 before any step, saying each of `named`.
fn refused_before_any_step(tree: &Tree, output: &Output, named: &[&str]) {
    let said = stderr(output);
    assert_eq!(output.status.code(), Some(1), "{}{said}", stdout(output));
    assert!(said.contains("ConfinementRefused"), "{said}");
    for name in named {
        assert!(said.contains(name), "{name}: {said}");
    }
    assert!(!started(tree).exists(), "a step started: {said}");
}

#[test]
fn two_tree_runs_never_share_a_cgroup_root() {
    if !delegation_present() {
        return;
    }
    let first = cell("own-root-first");
    let second = cell("own-root-second");
    // The second run opens its driver while the first one's launch runs: on a shared root that
    // open would end it.
    let mut one = Running::spawn(tree_run(&first, None, None));
    one.wait_for(&started(&first), "the first tree run");
    let mut two = Running::spawn(tree_run(&second, None, None));
    two.wait_for(&started(&second), "the second tree run");

    let roots = [launch_root(&first), launch_root(&second)];
    assert_ne!(roots[0], roots[1], "both runs launched under one root");
    let binary = fs::canonicalize(BINARY).unwrap();
    for root in &roots {
        assert!(
            root.extension()
                .is_some_and(|extension| extension == "scope"),
            "{} is no scope of its own",
            root.display()
        );
        // The run that launched there is in a child of it: the leaf it moved itself into.
        let leaves: Vec<PathBuf> = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("loom-controller-"))
            })
            .collect();
        assert_eq!(leaves.len(), 1, "{}: {leaves:?}", root.display());
        let pids = fs::read_to_string(leaves[0].join("cgroup.procs")).unwrap();
        let pid = pids.lines().next().unwrap_or_else(|| {
            panic!("{} holds no process", leaves[0].display());
        });
        assert_eq!(
            fs::read_link(format!("/proc/{pid}/exe")).unwrap(),
            binary,
            "{}",
            leaves[0].display()
        );
    }

    release(&first);
    release(&second);
    terminated(&one.finish());
    terminated(&two.finish());
}

#[test]
fn tree_run_uses_the_cgroup_root_it_is_given() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let tree = cell("given-root");
    release(&tree);
    let ran = tree_run(&tree, Some(&scope.root.join("run")), Some(&scope.root))
        .output()
        .unwrap();
    terminated(&ran);
    assert_eq!(launch_root(&tree), scope.root);
}

#[test]
fn a_second_tree_run_on_the_same_root_is_refused() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let first = cell("held-first");
    let second = cell("held-second");
    let mut one = Running::spawn(tree_run(
        &first,
        Some(&scope.root.join("run-1")),
        Some(&scope.root),
    ));
    one.wait_for(&started(&first), "the first tree run");

    let before = every_file(&second.store());
    let refused = tree_run(&second, Some(&scope.root.join("run-2")), Some(&scope.root))
        .output()
        .unwrap();
    refused_before_any_step(
        &second,
        &refused,
        &[&scope.root.display().to_string(), "held by another process"],
    );
    assert_eq!(every_file(&second.store()), before, "the refused run wrote");

    release(&first);
    terminated(&one.finish());
}

#[test]
fn the_root_hold_is_released_on_exit_and_on_kill() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let ended = cell("hold-ended");
    release(&ended);
    terminated(
        &tree_run(&ended, Some(&scope.root.join("run-1")), Some(&scope.root))
            .output()
            .unwrap(),
    );
    let after_exit = cell("hold-after-exit");
    release(&after_exit);
    terminated(
        &tree_run(
            &after_exit,
            Some(&scope.root.join("run-2")),
            Some(&scope.root),
        )
        .output()
        .unwrap(),
    );

    let killed = cell("hold-killed");
    let mut run = Running::spawn(tree_run(
        &killed,
        Some(&scope.root.join("run-3")),
        Some(&scope.root),
    ));
    run.wait_for(&started(&killed), "the tree run to be killed");
    run.child().kill().unwrap();
    let status = run.finish().status;
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(9),
        "{status:?}"
    );
    let after_kill = cell("hold-after-kill");
    release(&after_kill);
    terminated(
        &tree_run(
            &after_kill,
            Some(&scope.root.join("run-4")),
            Some(&scope.root),
        )
        .output()
        .unwrap(),
    );
}

#[test]
fn a_cgroup_root_that_does_not_qualify_is_refused() {
    let Some(scope) = Scope::start() else {
        return;
    };
    let refused = |label: &str, cgroup: Option<&Path>, root: &Path, rule: &str| {
        let tree = cell(label);
        let before = every_file(&tree.store());
        let output = tree_run(&tree, cgroup, Some(root)).output().unwrap();
        refused_before_any_step(&tree, &output, &[&root.display().to_string(), rule]);
        assert_eq!(every_file(&tree.store()), before, "{label}: the run wrote");
    };

    // A process in the root itself, the run in a child of it.
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
        assert!(
            sleeper.child().try_wait().unwrap().is_none(),
            "the sleeper ended"
        );
        assert!(Instant::now() < deadline, "the sleeper never entered");
        std::thread::sleep(Duration::from_millis(20));
    }
    refused("not-empty", Some(&busy.join("run")), &busy, "is not empty");
    drop(sleeper);

    // A root whose cgroup.procs this process may not write.
    let locked = scope.root.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(
        locked.join("cgroup.procs"),
        std::os::unix::fs::PermissionsExt::from_mode(0o444),
    )
    .unwrap();
    refused(
        "not-writable",
        Some(&locked.join("run")),
        &locked,
        "is not writable",
    );

    // Writable and empty, but its parent hands it no controller: not delegated.
    let bare = scope.root.join("plain/bare");
    fs::create_dir_all(&bare).unwrap();
    refused(
        "not-delegated",
        Some(&bare.join("run")),
        &bare,
        "is not delegated",
    );

    // Empty, writable and delegated, the run outside it.
    let free = scope.root.join("free");
    fs::create_dir(&free).unwrap();
    refused("not-in-a-child", None, &free, "is not in a child");
}

/// `tree run` of a fresh `cell@1` tree with a `PATH` that holds no `systemd-run`, and the store's
/// files before it.
fn refused_without_systemd_run(label: &str) -> (Tree, Output, Vec<(PathBuf, Vec<u8>)>) {
    let tree = cell(label);
    let empty = tree.root.join("no-programs");
    fs::create_dir(&empty).unwrap();
    let before = every_file(&tree.store());
    let output = tree_run(&tree, None, None)
        .env("PATH", &empty)
        .output()
        .unwrap();
    (tree, output, before)
}

#[test]
fn the_tree_refusal_names_tree_runs_own_remedy() {
    let (tree, output, before) = refused_without_systemd_run("remedy");
    refused_before_any_step(
        &tree,
        &output,
        &[
            "CgroupUndelegated",
            "tree run --cgroup-root",
            "systemd-run",
            "No such file or directory",
        ],
    );
    let said = stderr(&output);
    assert!(!said.contains(RUN_HINT), "{said}");
    assert_eq!(every_file(&tree.store()), before, "the refused run wrote");
}

#[test]
fn tree_run_help_and_refusal_name_the_confinement_precondition() {
    let help = Command::new(BINARY)
        .args(["tree", "run", "--help"])
        .output()
        .unwrap();
    assert_eq!(help.status.code(), Some(0), "{}", stderr(&help));
    let (_tree, refused, _) = refused_without_systemd_run("precondition");
    for (what, text) in [("help", stdout(&help)), ("refusal", stderr(&refused))] {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in [
            "Linux, bubblewrap and a delegated cgroup v2",
            "--cgroup-root",
            "--confinement local",
        ] {
            assert!(text.contains(phrase), "{what} lacks {phrase:?}: {text}");
        }
    }
}
