//! Acceptance for `story:bound-program-signal-exit-is-refused`: under Substrate a bound program a
//! signal ended is refused `ProgramSignalled` naming its signal, never read as an exit of its
//! result table; under `local` the same program is refused by its runner, as before.
//!
//! The fixture is `fixtures/tree-delegated/` with `killed.bindings.yaml`, whose `cell.look` runs
//! `stubs/killed.sh` (POSIX sh): it writes `target/started` and ends itself with SIGKILL. The
//! Substrate case starts `b10x-loom tree run` under its default confinement through the harness of
//! `delegated_cgroup.rs`, and skips naming what is missing unless `LOOM_REQUIRE_DELEGATION` is
//! set; the `local` case runs the library in process. No test reaches the network or a model.

use super::*;

const DELEGATED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tree-delegated");

/// A tree of one `cell@1` case whose `cell.look` runs `stubs/killed.sh`, opened, its workspace
/// holding `target/`.
fn killed_cell(label: &str) -> Tree {
    let tree = Tree::new(label);
    InstallStore::new(tree.root.join("home/loom/protocols"))
        .install_file("cell@1", &Path::new(DELEGATED).join("cell.yaml"), false)
        .unwrap();
    for file in ["killed.bindings.yaml", "cell.txt", "stubs/killed.sh"] {
        fs::copy(Path::new(DELEGATED).join(file), tree.workspace().join(file)).unwrap();
    }
    fs::create_dir_all(tree.workspace().join("target")).unwrap();
    open_with(&tree, "cell@1", "killed.bindings.yaml");
    tree
}

/// The refusal of the one `ActionFinished` line of `cell.look` in the root's log, which names no
/// exit.
fn the_refusal_of_cell_look(tree: &Tree) -> String {
    let finished: Vec<_> = log_lines(tree, "root")
        .into_iter()
        .filter_map(|line| match line.event {
            RunLogEvent::ActionFinished(finished) if finished.action == "cell.look" => {
                Some(finished)
            }
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1, "{finished:#?}");
    assert_eq!(finished[0].exit, None, "{finished:#?}");
    finished[0]
        .refusal
        .clone()
        .unwrap_or_else(|| panic!("no refusal: {finished:#?}"))
}

/// `output` ended 4, its stop `Stuck`, the program started, no `cell_look` recorded and the root
/// not terminated.
fn stuck_with_nothing_recorded(tree: &Tree, output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}{}",
        stdout(output),
        stderr(output)
    );
    assert_eq!(json(output)["stop"], "Stuck", "{}", stdout(output));
    assert!(
        tree.workspace().join("target/started").exists(),
        "the program never started"
    );
    let looks = records_of(tree, "root", "cell_look");
    assert!(looks.is_empty(), "{looks:?}");
    let status = tree.status();
    assert!(
        entry(&status, "root")["outcome"].is_null(),
        "the root terminated: {status}"
    );
}

#[test]
fn under_substrate_a_killed_program_is_refused_naming_signal_9() {
    if !super::delegated_cgroup::delegation_present() {
        return;
    }
    let tree = killed_cell("signal-exit-substrate");
    let output = super::delegated_cgroup::tree_run(&tree, None, None)
        .output()
        .unwrap();
    stuck_with_nothing_recorded(&tree, &output);
    let refusal = the_refusal_of_cell_look(&tree);
    assert!(
        refusal.starts_with("ProgramSignalled: action cell.look: "),
        "{refusal}"
    );
    assert!(
        refusal.contains("exit 137,") && refusal.contains("signal 9 ended"),
        "{refusal}"
    );
}

#[test]
fn under_local_a_killed_program_is_as_before() {
    let tree = killed_cell("signal-exit-local");
    let output = tree.run();
    stuck_with_nothing_recorded(&tree, &output);
    let refusal = the_refusal_of_cell_look(&tree);
    assert!(refusal.contains("was refused by its runner"), "{refusal}");
    assert!(!refusal.contains("ProgramSignalled"), "{refusal}");
}
