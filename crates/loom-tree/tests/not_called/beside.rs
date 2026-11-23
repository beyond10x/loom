//! A moving not-called writer beside other lines of its interval
//! (`story:writer-not-called-leaves-the-due-set`, D4 a, from the adversary pass of wave 7): a
//! refusal beside it keeps it set aside, a person's edit in its interval ends the set-aside, and a
//! writer of evidence on the artifact it moves alternates with it until the step budget, the limit
//! `AGENTS.md` names.

use super::*;

/// `work.a`: a writer of `done` whose id orders before `work.do`.
const A_ACTION: &str = "  work.a:
    effect: write
    may_produce:
      - evidence: done
";

/// `work.a`'s binding: its retry record opens on `looked: failed` as `work.do`'s does, and its
/// program exits 78, listed in `exits.error`, so each start is refused `ProgramError`.
const A_BINDING: &str = r#"  - action: work.a
    executor: {kind: program, argv: [sh, stubs/a.sh]}
    retry_when: {looked: [failed]}
    exits: {error: [78]}
    evidence:
      - {kind: done, subject: work, producer: aer, from: exit, results: {"0": ok, "*": failed}}
"#;

impl Tree {
    /// A one-case tree from the given protocol and bindings text and `do.sh`, with `extra` stubs.
    fn open_with(
        label: &str,
        protocol: &str,
        bindings: &str,
        program: &str,
        extra: &[(&str, &str)],
    ) -> Self {
        let dir = super::super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(workspace.join("stubs")).unwrap();
        std::fs::write(protocols.join("nudge.yaml"), protocol).unwrap();
        std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
        std::fs::write(workspace.join("notes.txt"), "notes\n").unwrap();
        for (file, text) in STUBS.iter().chain(extra) {
            std::fs::write(workspace.join("stubs").join(file), text).unwrap();
        }
        std::fs::write(workspace.join("stubs/do.sh"), program).unwrap();
        let bindings_file = dir.join("nudge.bindings.yaml");
        std::fs::write(&bindings_file, bindings).unwrap();
        let directory = read_protocols_directory(&protocols).unwrap();
        let store = dir.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace: workspace.clone(),
            protocol: "nudge@1".into(),
            bindings: bindings_file,
            revisions: Vec::new(),
            case: "root".into(),
            time: TreeTime::At(AT.into()),
            catalog: &directory.catalog,
            protocols: directory.kept,
        })
        .unwrap();
        Self {
            store,
            workspace,
            catalog: directory.catalog,
        }
    }
}

impl Tree {
    /// One `tree run` of at most `steps` steps.
    fn run_steps(&self, steps: i64) -> TreeRun {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        let settings = RunSettings {
            max_steps: Some(steps),
            max_wall_seconds: Some(50),
            time: TreeTime::At(AT.into()),
            runner: std::sync::Arc::new(runner),
            confinement: None,
            cgroup_root: None,
        };
        run_tree(&self.store, &self.catalog, &settings, None, None).expect("the tree runs")
    }
}

/// Guard for the `accounted` condition of `set_aside`, which no test of the unit reaches: a run
/// whose step budget ends right after a moving not-called run leaves that line open (no
/// `evaluated` line after it); a person then edits `notes.txt`, so the next run's `evaluated`
/// line is two moves above the opening one while the interval names one. That line is not the
/// writer's own writing, so `work.do` is due again and starts once. Dropping `accounted` from
/// the `own` test of `set_aside` keeps it set aside and turns this red.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_edit_in_the_interval_of_a_moving_not_called_run_ends_the_set_aside() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let tree = Tree::open("nc-beside-accounted", &moving, NOT_CALLED, false);
    let first = tree.run_steps(2);
    let lines = tree.lines();
    assert_eq!(
        steps(&lines),
        texts(&[
            "finished work.look exit 1",
            "finished work.do exit 79 moved notes"
        ]),
        "{first:?}"
    );
    let last_evaluated = lines
        .iter()
        .rposition(|line| matches!(line.event, RunLogEvent::Evaluated(_)))
        .unwrap();
    let last_finished = lines
        .iter()
        .rposition(|line| matches!(line.event, RunLogEvent::ActionFinished(_)))
        .unwrap();
    assert!(last_evaluated < last_finished, "{first:?}");
    let opened = *evaluated(&lines).last().unwrap();
    let before = lines.len();
    std::fs::write(tree.workspace.join("notes.txt"), "a person's edit\n").unwrap();
    let second = tree.run();
    let lines = tree.lines();
    assert_eq!(evaluated(&lines[before..]).first(), Some(&(opened + 2)));
    assert_eq!(
        steps(&lines[before..]),
        texts(&[
            "finished work.do exit 79 moved notes",
            "checked work.ask exit 0"
        ]),
        "{second:?}"
    );
    handed_off(&second);
}

fn starts_of(lines: &[RunLogLine], action: &str) -> usize {
    lines
        .iter()
        .filter(|line| {
            matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == action)
        })
        .count()
}

/// A refused writer (`work.a`, `exits.error`) whose retry record is open beside a not-called
/// writer that moves `notes`: the refusal names no move and records nothing, so the `evaluated`
/// line after `work.do`'s move is still `work.do`'s own writing (D4 a). `work.do` starts once and
/// the run hands off. A rule that counted the refusal as another cause would make `work.do` due
/// after each of its own moves, and the run would spend its steps (Runbook's loop, back).
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_refusal_beside_a_moving_not_called_writer_keeps_it_set_aside() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let protocol = PROTOCOL.replace("{tidy}", A_ACTION);
    let bindings = BINDINGS
        .replace("{exits}", NOT_CALLED)
        .replace("{tidy}", A_BINDING);
    let tree = Tree::open_with(
        "nc-beside-refusal",
        &protocol,
        &bindings,
        &moving,
        &[("a.sh", "exit 78\n")],
    );
    let run = tree.run();
    let lines = tree.lines();
    assert_eq!(
        starts_of(&lines, "work.do"),
        1,
        "{:?}\n{run:?}",
        steps(&lines)
    );
    handed_off(&run);
}

/// A not-called writer that moves `notes` beside `work.tidy`, a writer of `tidied` on `notes`:
/// each move of `work.do` makes `work.tidy` useful again, `work.tidy`'s record ends the set-aside,
/// and the two alternate until the step budget. That is the limit D4 a keeps and `AGENTS.md`
/// names: any change other than the writer's own moves ends the set-aside, so such a pair is
/// bounded by `max_steps` (in a suite cycle, its 60 s wall bound), never by one start per
/// evaluation.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_moving_not_called_writer_and_a_writer_of_its_output_alternate_until_the_step_budget() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let tree = Tree::open("nc-beside-alternate", &moving, NOT_CALLED, true);
    let run = tree.run();
    let lines = tree.lines();
    let mut expected = vec!["finished work.look exit 1"];
    for _ in 0..4 {
        expected.extend([
            "finished work.do exit 79 moved notes",
            "finished work.tidy exit 0",
        ]);
    }
    expected.push("finished work.do exit 79 moved notes");
    assert_eq!(steps(&lines), texts(&expected), "{run:?}");
    assert_eq!(run.stop, TreeStop::BudgetSpent, "{run:?}");
}
