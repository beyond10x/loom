//! What `set_aside` reads back through D4 a (`story:writer-not-called-leaves-the-due-set`, from the
//! second adversary pass of wave 7): a refusal there still ends at its own `evaluated` line, and
//! two moving not-called writers each start once, the set-aside ones being idle.

use super::*;

/// `work.a`: a writer that may produce only `looked`, which `work.look` records first, so it is
/// useful only while due (its retry record), never through open evidence.
const A_DUE_ONLY: &str = "  work.a:
    effect: write
    may_produce:
      - evidence: looked
";

const A_DUE_ONLY_BINDING: &str = r#"  - action: work.a
    executor: {kind: program, argv: [sh, stubs/a.sh]}
    retry_when: {looked: [failed]}
    exits: {error: [78]}
    evidence:
      - {kind: looked, subject: work, producer: aer, from: exit, results: {"0": ok, "*": failed}}
"#;

/// The read-back half of the fix: a refusal in the interval whose `evaluated` line is the
/// not-called writer's own still ends at that line (`latest` in `set_aside`), so the refused
/// writer, due through its retry record alone, is started again once after it. The unit's
/// `a_refusal_beside_a_moving_not_called_writer_keeps_it_set_aside` cannot see this: its `work.a`
/// may produce `done`, so it is useful at the new revision through open evidence whether or not
/// it is due, and dropping `latest` leaves that test green.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_refusal_read_back_ends_at_its_own_evaluated_line() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let protocol = PROTOCOL.replace("{tidy}", A_DUE_ONLY);
    let bindings = BINDINGS
        .replace("{exits}", NOT_CALLED)
        .replace("{tidy}", A_DUE_ONLY_BINDING);
    let tree = Tree::open_with(
        "nc-read-back-readback",
        &protocol,
        &bindings,
        &moving,
        &[("a.sh", "exit 78\n")],
    );
    let run = tree.run();
    assert_eq!(
        steps(&tree.lines()),
        texts(&[
            "finished work.look exit 1",
            "finished work.a refused",
            "finished work.do exit 79 moved notes",
            "finished work.a refused",
            "checked work.ask exit 0",
        ]),
        "{run:?}"
    );
    handed_off(&run);
}

/// `work.b`: a second not-called writer that moves `notes`, ordered before `work.do`.
const B_ACTION: &str = "  work.b:
    effect: write
    may_produce:
      - evidence: done
";

const B_BINDING: &str = r#"  - action: work.b
    executor: {kind: program, argv: [sh, stubs/b.sh]}
    retry_when: {looked: [failed]}
    exits: {not_called: [79]}
    evidence:
      - {kind: done, subject: work, producer: beer, from: exit, results: {"0": ok, "*": failed}}
"#;

/// Two moving not-called writers: each `evaluated` line their moves give is their own writing,
/// so the read-back keeps both set aside and each starts once before the handoff.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn two_moving_not_called_writers_each_start_once() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let protocol = PROTOCOL.replace("{tidy}", B_ACTION);
    let bindings = BINDINGS
        .replace("{exits}", NOT_CALLED)
        .replace("{tidy}", B_BINDING);
    let tree = Tree::open_with(
        "nc-read-back-two",
        &protocol,
        &bindings,
        &moving,
        &[("b.sh", "echo b >> notes.txt\nexit 79\n")],
    );
    let run = tree.run();
    assert_eq!(
        steps(&tree.lines()),
        texts(&[
            "finished work.look exit 1",
            "finished work.b exit 79 moved notes",
            "finished work.do exit 79 moved notes",
            "checked work.ask exit 0",
        ]),
        "{run:?}"
    );
    handed_off(&run);
}
