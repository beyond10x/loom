//! Acceptance for `story:handoff-when-a-program-decides`: a writer's `handoff_when` check runs
//! before the writer's step and at a stuck stop; exit 0 hands the action off, any other exit lets
//! the writer run or leaves the stop `Stuck`, and a check that gives no exit status refuses the
//! run `HandoffCheckFailed`. Each evaluation writes one `handoff checked` line.
//!
//! Every test opens the one-case fixture tree `tour-leg@1` with `tour-leg-handoff-when.bindings.yaml`
//! (its check argv, and for some its writer, changed as the test says) and runs it once or twice;
//! no test runs a whole tree. The check is `stubs/handoff-when.sh`, POSIX sh. Every run but the
//! confinement test's goes through the `local` runner; that one needs bubblewrap and a delegated
//! user scope, and skips without them unless `LOOM_REQUIRE_DELEGATION` is set. No test reaches the
//! network or a model, and each ends within 60 s.

use b10x_loom_tree::runlog::HandoffChecked;

use super::*;

/// The fixture bindings file, and the check argv it names.
const BINDINGS: &str = "tour-leg-handoff-when.bindings.yaml";
const CHECK: &str = "[sh, stubs/handoff-when.sh, exit]";

/// The handoff text of `route.write` in [`BINDINGS`].
const HANDOFF: &str = "Answer the finding in findings.txt, then run the tree again.";

/// The writer whose step the check decides.
const WRITER: &str = "route.write";

/// The variable a retried writer reads its feedback from.
const RETRY_FEEDBACK: &str = "LOOM_RETRY_FEEDBACK";

/// A one-case tree opened with [`BINDINGS`], its check argv `check`, and each `(from, to)` of
/// `edits` replaced in the text first.
fn checked(label: &str, check: &str, edits: &[(&str, &str)]) -> Tree {
    let tree = Tree::new(label);
    let mut text = fs::read_to_string(tree.workspace().join(BINDINGS))
        .unwrap()
        .replace(CHECK, check);
    for (from, to) in edits {
        assert!(text.contains(from), "{from} is not in {BINDINGS}");
        text = text.replace(from, to);
    }
    fs::write(tree.workspace().join("check.bindings.yaml"), text).unwrap();
    let opened = tree.open_leg_with("check.bindings.yaml");
    assert_eq!(opened.status.code(), Some(0), "{}", stderr(&opened));
    tree
}

/// The `handoff checked` line of `line`, if it is one.
fn check_of(line: &RunLogLine) -> Option<&HandoffChecked> {
    match &line.event {
        RunLogEvent::HandoffChecked(checked) => Some(checked),
        _ => None,
    }
}

/// Whether `line` is the `action finished` line of the writer.
fn writer_finished(line: &RunLogLine) -> bool {
    matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == WRITER)
}

/// The `stopped` line's stop and decisions, when `line` is one.
fn stopped_of(line: &RunLogLine) -> Option<(&str, &[String])> {
    match &line.event {
        RunLogEvent::Stopped(stopped) => Some((stopped.stop.as_str(), stopped.next.as_slice())),
        _ => None,
    }
}

/// The blob `digest` names in the store of `tree`, as text.
fn blob(tree: &Tree, digest: Option<&String>) -> String {
    let digest = digest.expect("the line names a blob");
    fs::read_to_string(tree.store().join("blobs").join(digest)).unwrap()
}

/// `output` ended `code`, saying why when it did not.
fn ended(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}{}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn a_check_that_exits_0_hands_the_action_off() {
    let tree = checked("check-0", CHECK, &[]);
    fs::write(tree.workspace().join("check-exit"), "0").unwrap();
    let ran = tree.run();
    ended(&ran, 3);
    let waited = json(&ran);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decisions = waited["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 1, "{waited}");
    assert_eq!(decisions[0]["kind"], "Handoff", "{waited}");
    assert_eq!(decisions[0]["action"], WRITER, "{waited}");
    assert_eq!(decisions[0]["decides"], HANDOFF, "{waited}");
    let store = tree.store().display().to_string();
    assert_eq!(
        decisions[0]["command"],
        json!([
            "b10x-loom",
            "tree",
            "run",
            "--store",
            store,
            "--confinement",
            "local"
        ])
    );

    // The writer did not run; one line holds the check's exit 0, right before the stop.
    assert!(!tree.workspace().join("route.txt").exists());
    let lines = log_lines(&tree, "root");
    assert!(!lines.iter().any(writer_finished), "{lines:#?}");
    let checks: Vec<(usize, &HandoffChecked)> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| check_of(line).map(|check| (index, check)))
        .collect();
    assert_eq!(checks.len(), 1, "{lines:#?}");
    let (at, check) = checks[0];
    assert_eq!(check.action, WRITER);
    assert_eq!((check.exit, check.refusal.as_deref()), (Some(0), None));
    assert_eq!(blob(&tree, check.blob_digest.as_ref()), "");
    assert_eq!(at + 2, lines.len(), "{lines:#?}");
    assert_eq!(
        stopped_of(&lines[at + 1]),
        Some(("AwaitingPerson", [HANDOFF.to_owned()].as_slice()))
    );

    // The log is the record of the check's answer: reading the tree runs no check.
    tree.status();
    ended(&tree.verified(), 0);
    let runs = fs::read_to_string(tree.workspace().join("check-runs.txt")).unwrap();
    assert_eq!(runs.lines().count(), 1, "the check ran {runs:?}");
}

#[test]
fn a_check_that_exits_1_lets_the_writer_run() {
    let tree = checked("check-1", CHECK, &[]);
    let ran = tree.run();
    ended(&ran, 3);
    let waited = json(&ran);
    // The writer ran to two stops and the case went on to the guide's approval: not stuck, and
    // no handoff.
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    assert!(
        waited["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|decision| decision["kind"] != "Handoff"),
        "{waited}"
    );
    let route = fs::read_to_string(tree.workspace().join("route.txt")).unwrap();
    assert_eq!(route.lines().count(), 2, "{route:?}");

    // Each step of the writer was checked first, in that step: its line exit 1, right before the
    // writer's.
    let lines = log_lines(&tree, "root");
    let writers: Vec<usize> = (0..lines.len())
        .filter(|index| writer_finished(&lines[*index]))
        .collect();
    assert_eq!(writers.len(), 2, "{lines:#?}");
    for index in &writers {
        let check = check_of(&lines[index - 1]).unwrap_or_else(|| panic!("{lines:#?}"));
        assert_eq!(check.action, WRITER);
        assert_eq!((check.exit, check.refusal.as_deref()), (Some(1), None));
    }
    assert_eq!(lines.iter().filter_map(check_of).count(), 2, "{lines:#?}");
}

#[test]
fn at_a_stuck_stop_the_check_decides_the_handoff() {
    // The writer writes nothing and nothing sends it back: once it ran, the case is otherwise
    // stuck.
    let tree = checked(
        "check-stuck",
        CHECK,
        &[
            ("[sh, stubs/route-write.sh]", "[sh, stubs/route-idle.sh]"),
            ("    retry_when:\n      route_check: [invalid]\n", ""),
        ],
    );
    let first = tree.run();
    ended(&first, 4);
    assert_eq!(json(&first)["stop"], "Stuck");
    let lines = log_lines(&tree, "root");
    let tail: Vec<&RunLogLine> = lines[lines.len() - 4..].iter().collect();
    assert_eq!(check_of(tail[0]).map(|check| check.exit), Some(Some(1)));
    assert!(writer_finished(tail[1]), "{lines:#?}");
    assert_eq!(check_of(tail[2]).map(|check| check.exit), Some(Some(1)));
    assert_eq!(stopped_of(tail[3]), Some(("Stuck", [].as_slice())));

    fs::write(tree.workspace().join("check-exit"), "0").unwrap();
    let before = lines.len();
    let second = tree.run();
    ended(&second, 3);
    let waited = json(&second);
    assert_eq!(waited["stop"], "AwaitingPerson", "{waited}");
    let decisions = waited["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 1, "{waited}");
    assert_eq!(decisions[0]["kind"], "Handoff", "{waited}");
    assert_eq!(decisions[0]["decides"], HANDOFF, "{waited}");
    let since = lines_since(&tree, "root", before);
    assert_eq!(since.len(), 2, "{since:#?}");
    let check = check_of(&since[0]).unwrap_or_else(|| panic!("{since:#?}"));
    assert_eq!((check.action.as_str(), check.exit), (WRITER, Some(0)));
    assert_eq!(
        stopped_of(&since[1]),
        Some(("AwaitingPerson", [HANDOFF.to_owned()].as_slice()))
    );
}

#[test]
fn a_failed_check_stops_the_run_never_skips() {
    for (label, check) in [
        (
            "check-timeout",
            "[sh, stubs/handoff-when.sh, sleep]\n    timeout_seconds: 1",
        ),
        ("check-missing", "[stubs/no-such-check.sh]"),
        ("check-signal", "[sh, stubs/handoff-when.sh, signal]"),
    ] {
        let tree = checked(label, check, &[]);
        let ran = tree.run();
        ended(&ran, 1);
        let said = stderr(&ran);
        assert!(said.contains("HandoffCheckFailed"), "{label}: {said}");
        let refusal: Value = serde_json::from_str(said.lines().last().unwrap()).unwrap();
        assert_eq!(refusal["reason"], "HandoffCheckFailed", "{label}: {said}");
        assert_eq!(refusal["action"], WRITER, "{label}: {said}");
        assert_eq!(refusal["case"], "root", "{label}: {said}");

        // The line holds the refusal, it is the last, and the writer never ran.
        let lines = log_lines(&tree, "root");
        let check = check_of(lines.last().unwrap()).unwrap_or_else(|| panic!("{lines:#?}"));
        assert_eq!(check.action, WRITER, "{label}");
        assert_eq!(check.exit, None, "{label}: {check:?}");
        assert!(check.refusal.is_some(), "{label}: {check:?}");
        assert!(!lines.iter().any(writer_finished), "{label}: {lines:#?}");
        assert!(!tree.workspace().join("route.txt").exists(), "{label}");
    }
}

/// The names of the variables a blob of `KEY=value` lines holds.
fn variables(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_owned()))
        .collect()
}

#[test]
fn the_check_gets_path_and_loom_variables_only() {
    let tree = checked(
        "check-env",
        "[sh, stubs/handoff-when.sh, env]",
        &[(
            "[sh, stubs/route-write.sh]",
            "[sh, stubs/handoff-when.sh, writer-env]",
        )],
    );
    let ran = tree.run();
    ended(&ran, 3);
    let lines = log_lines(&tree, "root");
    let mut retried = 0;
    for (index, line) in lines.iter().enumerate() {
        if !writer_finished(line) {
            continue;
        }
        let RunLogEvent::ActionFinished(writer) = &line.event else {
            unreachable!()
        };
        let check = check_of(&lines[index - 1]).unwrap_or_else(|| panic!("{lines:#?}"));
        assert_eq!(check.exit, Some(1), "{check:?}");
        let given = variables(&blob(&tree, check.blob_digest.as_ref()));
        assert!(given.contains(&"PATH".to_owned()), "{given:?}");
        assert!(given.contains(&"LOOM_CASE".to_owned()), "{given:?}");
        for name in &given {
            assert!(
                name == "PATH" || name.starts_with("LOOM_"),
                "the check was given {name}: {given:?}"
            );
        }
        assert!(!given.contains(&RETRY_FEEDBACK.to_owned()), "{given:?}");
        // The writer of this step was given its retry feedback: the check was not.
        if variables(&blob(&tree, writer.blob_digest.as_ref())).contains(&RETRY_FEEDBACK.to_owned())
        {
            retried += 1;
        }
    }
    assert!(retried > 0, "no writer step was a retry: {lines:#?}");
}

/// Whether bubblewrap is here and a delegated scope of the user's manager starts; without them
/// the test skips, or fails when `LOOM_REQUIRE_DELEGATION` is set.
fn delegation() -> bool {
    let missing = |what: String| {
        assert!(
            std::env::var_os("LOOM_REQUIRE_DELEGATION").is_none(),
            "LOOM_REQUIRE_DELEGATION is set and a prerequisite is missing: {what}"
        );
        eprintln!("skipped: missing prerequisite: {what}");
        false
    };
    if !Path::new("/usr/bin/bwrap").is_file() {
        return missing("executable /usr/bin/bwrap".to_owned());
    }
    match Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--quiet",
            "-p",
            "Delegate=yes",
            "--",
            "true",
        ])
        .output()
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => missing(format!(
            "a user systemd manager that starts a delegated scope ({}: {})",
            output.status,
            stderr(&output).trim()
        )),
        Err(error) => missing(format!("systemd-run ({error})")),
    }
}

#[test]
fn the_check_cannot_write_the_workspace() {
    if !delegation() {
        return;
    }
    let tree = checked("check-write", "[sh, stubs/handoff-when.sh, write]", &[]);
    fs::create_dir_all(tree.workspace().join("target")).unwrap();
    // Under the default confinement, `substrate`: the route's check, then the writer's step.
    let ran = tree
        .command()
        .args(["tree", "run", "--max-steps", "2", "--at", AT, "--store"])
        .arg(tree.store())
        .output()
        .unwrap();
    ended(&ran, 5);
    let lines = log_lines(&tree, "root");
    let check = lines
        .iter()
        .find_map(check_of)
        .unwrap_or_else(|| panic!("{lines:#?}"));
    // Its own exit, not the 1 of "no handoff": the write failed and the check went on.
    assert_eq!((check.exit, check.refusal.as_deref()), (Some(3), None));
    let output = blob(&tree, check.blob_digest.as_ref());
    assert_eq!(
        output.matches("Read-only file system").count(),
        2,
        "both writes are refused read-only: {output}"
    );
    assert!(!tree.workspace().join("check-wrote.txt").exists());
    assert!(!tree.workspace().join("target/check-wrote.txt").exists());
}
