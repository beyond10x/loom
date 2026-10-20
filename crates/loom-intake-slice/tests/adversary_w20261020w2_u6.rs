//! Adversary cases for `story:child-writable-workspace` (wave 2026-10-20-w2, U6).
//!
//! The delegated cases run against the real backend in a delegated cgroup v2 scope, the way
//! `tests/confined_child.rs` does: the test binary starts itself again under
//! `systemd-run --user --scope -p Delegate=yes` and runs the case there. Where bubblewrap, a user
//! systemd manager or the Substrate driver is absent, the case prints
//! `skipped: missing prerequisite: …` and returns. Every path a case writes lies in its own
//! temporary directory, and nothing connects anywhere.

use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_intake_slice::confinement::{
    ChildCommand, ConfinementRefusal, SubstrateRunner, TestRunner as _, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;

const DELEGATED: &str = "LOOM_ADV_W20261020W2_U6_DELEGATED";
const REPORT: &str = "LOOM_ADV_W20261020W2_U6_REPORT";
const BUBBLEWRAP: &str = "/usr/bin/bwrap";

fn bubblewrap_present() -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(BUBBLEWRAP).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Run `case` against the real backend in a delegated scope, or skip naming what is missing.
fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
    if std::env::var_os(DELEGATED).is_some() {
        let report = PathBuf::from(std::env::var_os(REPORT).expect("the report path is given"));
        std::fs::write(&report, "started").unwrap();
        let cgroup = match prepare_delegated_cgroup() {
            Ok(cgroup) => cgroup,
            Err(error) => {
                std::fs::write(&report, format!("a delegated cgroup v2 scope ({error})")).unwrap();
                return;
            }
        };
        let runner = SubstrateRunner::new(Some(cgroup));
        let fixture = tempfile::tempdir().unwrap();
        let workspace = fixture.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        if let Err(error) = runner.preflight(&workspace) {
            std::fs::write(&report, format!("the Substrate driver ({error})")).unwrap();
            return;
        }
        std::fs::write(&report, "ran").unwrap();
        case(&runner, &workspace);
        return;
    }
    if !bubblewrap_present() {
        eprintln!("skipped: missing prerequisite: executable {BUBBLEWRAP}");
        return;
    }
    let state = tempfile::tempdir().unwrap();
    let report = state.path().join("report");
    let started = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "-p", "Delegate=yes", "--"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(DELEGATED, "1")
        .env(REPORT, &report)
        .output();
    let output = match started {
        Ok(output) => output,
        Err(error) => {
            eprintln!("skipped: missing prerequisite: systemd-run ({error})");
            return;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match std::fs::read_to_string(&report).ok().as_deref() {
        None => eprintln!(
            "skipped: missing prerequisite: a user systemd manager that starts a delegated scope (systemd-run {}: {})",
            output.status,
            stderr.trim()
        ),
        Some("started") => panic!(
            "the delegated case stopped before it decided its prerequisites: {}\n{stdout}{stderr}",
            output.status
        ),
        Some("ran") => {
            assert!(
                output.status.success(),
                "the delegated case failed: {}\n{stdout}{stderr}",
                output.status
            );
            assert!(
                stdout.contains("1 passed"),
                "the delegated binary ran the case: {stdout}{stderr}"
            );
            eprintln!("ran delegated: {name}");
        }
        Some(missing) => eprintln!("skipped: missing prerequisite: {missing}"),
    }
}

/// For every directory descriptor the confined process holds, try to create `escape-<n>` in that
/// directory and `escape-up-<n>` in its parent, and to read `beside` in its parent, saying which
/// landed; then list the descriptors. A confined process holding no descriptor of a host
/// directory writes and reads nothing that way.
const THROUGH_DESCRIPTORS: &str = r#"for d in /proc/self/fd/*; do
  [ -d "$d" ] || continue
  n=${d##*/}
  (printf escaped > "$d/escape-$n") 2>/dev/null && echo "fd $n wrote in $(readlink "$d")"
  (printf escaped > "$d/../escape-up-$n") 2>/dev/null && echo "fd $n wrote above $(readlink "$d")"
  b=$(cat "$d/../beside" 2>/dev/null) && echo "fd $n read beside: $b"
done
ls -l /proc/self/fd
true"#;

/// What the host keeps in `beside`, next to the workspace, where no confined process reads.
const BESIDE: &str = "host-only-marker-7f3a";

/// The names starting with `escape` directly in `directory`.
fn escapes(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("escape"))
        .collect();
    names.sort();
    names
}

/// What a run reached outside the paths it may write: `escape` names in the workspace root and
/// beside the workspace, and whether its output holds what the host keeps beside it.
fn reached(workspace: &Path, output: &str) -> (Vec<String>, Vec<String>, bool) {
    (
        escapes(workspace),
        escapes(workspace.parent().unwrap()),
        output.contains(BESIDE),
    )
}

/// The story's acceptance: a child "fails to write one outside" the path the host named. A child
/// that uses what it was handed, not only the paths it sees, must still write nothing in the
/// read-only rest of the workspace and nothing beside the workspace.
#[test]
fn adv_a_child_writes_nothing_outside_its_named_path_through_a_descriptor_it_holds() {
    delegated(
        "adv_a_child_writes_nothing_outside_its_named_path_through_a_descriptor_it_holds",
        |runner, workspace| {
            std::fs::create_dir(workspace.join("scratch")).unwrap();
            std::fs::write(workspace.join("README"), "kept\n").unwrap();
            std::fs::write(workspace.parent().unwrap().join("beside"), BESIDE).unwrap();
            let child = ChildCommand::new("sh", ["-c", THROUGH_DESCRIPTORS], workspace)
                .with_writable("scratch");
            let result = runner.run_child(&child).expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.applied.writable_scopes, ["scratch", "target"]);
            assert_eq!(
                reached(workspace, &result.output),
                (Vec::new(), Vec::new(), false),
                "(written in the read-only workspace root, written beside the workspace, read \
                 beside it):\n{}",
                result.output
            );
        },
    );
}

/// The story's acceptance: `TestRunner::run` keeps `target` as its only writable path, for a test
/// that uses what it was handed as much as for one that writes where it sees.
#[test]
fn adv_a_test_writes_nothing_outside_target_through_a_descriptor_it_holds() {
    delegated(
        "adv_a_test_writes_nothing_outside_target_through_a_descriptor_it_holds",
        |runner, workspace| {
            std::fs::create_dir(workspace.join("src")).unwrap();
            std::fs::write(workspace.parent().unwrap().join("beside"), BESIDE).unwrap();
            let test = TestCommand::new("sh", ["-c", THROUGH_DESCRIPTORS]);
            let result = runner
                .run(&test, workspace)
                .expect("the test runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.applied.writable_scopes, ["target"]);
            assert_eq!(
                reached(workspace, &result.output),
                (Vec::new(), Vec::new(), false),
                "(written in the read-only workspace root, written beside the workspace, read \
                 beside it):\n{}",
                result.output
            );
        },
    );
}

/// A nested named path `a/b`: the child writes nothing beside it, in its parent, through `..`,
/// through a symbolic link or hard link it makes, and moves nothing out of it, nor the path
/// itself.
#[test]
fn adv_a_child_writes_nothing_beside_or_above_a_nested_named_path() {
    delegated(
        "adv_a_child_writes_nothing_beside_or_above_a_nested_named_path",
        |runner, workspace| {
            std::fs::create_dir_all(workspace.join("a/b")).unwrap();
            std::fs::write(workspace.join("a/file"), "kept\n").unwrap();
            std::fs::write(workspace.join("a/b/moved"), "inside\n").unwrap();
            let script = r#"try() { if (eval "$2") >/dev/null 2>&1; then echo "$1:done"; else echo "$1:refused"; fi; }
try sibling 'printf x > a/sibling'
try parent-file 'printf x > a/file'
try dotdot 'printf x > a/b/../dotdot'
try mkdir 'mkdir a/b2'
try hardlink 'ln a/file a/b/hardlink'
try symlink-write 'ln -s ../file a/b/symlink && printf x > a/b/symlink'
try rename-out 'mv a/b/moved a/moved'
try rename-path 'mv a/b a/c'
try own 'printf x > a/b/own'"#;
            let child = ChildCommand::new("sh", ["-c", script], workspace).with_writable("a/b");
            let result = runner.run_child(&child).expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(
                result.output,
                "sibling:refused\nparent-file:refused\ndotdot:refused\nmkdir:refused\n\
                 hardlink:refused\nsymlink-write:refused\nrename-out:refused\n\
                 rename-path:refused\nown:done\n"
            );
            assert_eq!(
                std::fs::read_to_string(workspace.join("a/file")).unwrap(),
                "kept\n"
            );
            assert!(workspace.join("a/b/moved").is_file());
            for absent in [
                "a/sibling",
                "a/dotdot",
                "a/b2",
                "a/moved",
                "a/c",
                "a/b/hardlink",
            ] {
                assert!(!workspace.join(absent).exists(), "{absent}");
            }
            assert_eq!(result.applied.writable_scopes, ["a/b", "target"]);
        },
    );
}

/// Two children in one workspace through one runner's shared driver, side by side, each naming
/// another writable path: each writes its own and not the other's, and each result lists its own.
#[test]
fn adv_two_children_naming_different_paths_in_one_workspace_write_only_their_own() {
    delegated(
        "adv_two_children_naming_different_paths_in_one_workspace_write_only_their_own",
        |runner, workspace| {
            for directory in ["a", "b"] {
                std::fs::create_dir(workspace.join(directory)).unwrap();
            }
            let script = |own: &str, other: &str| {
                format!(
                    "printf started > {own}/started; n=0; \
                     while [ ! -e {other}/started ] && [ $n -lt 200 ]; do sleep 0.05; n=$((n+1)); done; \
                     if (printf x > {other}/from-{own}) 2>/dev/null; then echo {other}:written; else echo {other}:refused; fi; \
                     if (printf x > {own}/own) 2>/dev/null; then echo {own}:written; else echo {own}:refused; fi"
                )
            };
            let first = ChildCommand::new("sh", ["-c".to_owned(), script("a", "b")], workspace)
                .with_writable("a");
            let first_runner = runner.clone();
            let handle = std::thread::spawn(move || first_runner.run_child(&first));
            let second = runner
                .run_child(
                    &ChildCommand::new("sh", ["-c".to_owned(), script("b", "a")], workspace)
                        .with_writable("b"),
                )
                .expect("the second child runs confined");
            let first = handle
                .join()
                .unwrap()
                .expect("the first child runs confined");
            assert_eq!(first.output, "b:refused\na:written\n");
            assert_eq!(second.output, "a:refused\nb:written\n");
            assert_eq!(first.applied.writable_scopes, ["a", "target"]);
            assert_eq!(second.applied.writable_scopes, ["b", "target"]);
            assert!(!workspace.join("b/from-a").exists());
            assert!(!workspace.join("a/from-b").exists());
        },
    );
}

/// A named directory refused for a socket it holds leaves the workspace as it was, as one refused
/// for a symbolic link, an absent path or a file does (`confinement.rs`: "Before `target` is
/// created, so a refused path leaves the workspace as it was").
#[test]
fn adv_a_named_directory_refused_for_a_socket_leaves_the_workspace_as_it_was() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    std::fs::create_dir_all(workspace.join("sockets")).unwrap();
    let _held = UnixListener::bind(workspace.join("sockets/held")).unwrap();
    // A directory that is no cgroup: a launch that got past the refusal would be refused for it.
    let runner = SubstrateRunner::new(Some(fixture.path().to_owned()));
    let child = ChildCommand::new("true", [] as [&str; 0], &workspace).with_writable("sockets");
    let error = runner.run_child(&child).unwrap_err();
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");
    assert!(error.detail.contains("socket"), "{error}");
    assert!(
        !workspace.join("target").exists(),
        "the refused launch created `target` in the workspace"
    );
}
