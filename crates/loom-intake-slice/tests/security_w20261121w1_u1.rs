//! Security review of wave 2026-11-21-w1, unit U1 (story:substrate-0-21-pin), pass 1.
//!
//! Integrity conformance on Substrate 0.21.0, on every launch path a child takes: run to its end
//! (`run_child`), given standard input (`run_child` with `with_stdin`, Substrate's pipe session)
//! and live (`spawn_child`). Each records the adapter `linux` and exactly the five facts the story
//! names, in Substrate's own `FACT_ORDER`, and keeps Substrate's record of what it applied: a
//! named, non-empty cgroup equal to the projected one, and no `requires` echo, which Loom never
//! sends. The story's five-facts acceptance otherwise rests on
//! `tests/confinement.rs::real_confinement_rust_system_tools_and_cleanup`, which is ignored.
//!
//! The cases run through the delegated harness in `tests/support`, as `tests/confined_child.rs`
//! does, and skip naming what is missing where bubblewrap or a delegated scope is absent.

mod support;

use std::path::Path;

use b10x_loom_intake_slice::confinement::{AppliedConfinement, ChildCommand, SubstrateRunner};
use substrate_host::confinement::FACT_ORDER;

/// The facts the story's acceptance names, in the order it names them.
const STORY_FACTS: [&str; 5] = [
    "whole_tree_kill",
    "ipc_socket_allowlist",
    "ipc_signals_contained",
    "stdin_pipe",
    "output_live",
];

/// A fact's wire spelling, as Substrate serializes it.
fn spelling(fact: &impl serde::Serialize) -> String {
    serde_json::to_value(fact)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

/// `applied` names the adapter and exactly the story's facts, each at a strictly later place in
/// Substrate's `FACT_ORDER` than the one before; `record` names a non-empty cgroup, the one
/// projected, and echoes no requirement.
fn conforms(path: &str, applied: &AppliedConfinement, record: &substrate_wire::AppliedConfinement) {
    assert_eq!(applied.adapter.as_deref(), Some("linux"), "{path}");
    let facts = applied
        .facts
        .clone()
        .expect("a Substrate launch records its facts");
    assert_eq!(facts, STORY_FACTS, "{path}: the recorded facts");
    let order: Vec<String> = FACT_ORDER.iter().map(spelling).collect();
    let places: Vec<usize> = facts
        .iter()
        .map(|fact| {
            order
                .iter()
                .position(|named| named == fact)
                .unwrap_or_else(|| panic!("{path}: `{fact}` is no fact in FACT_ORDER"))
        })
        .collect();
    assert!(
        places.windows(2).all(|pair| pair[0] < pair[1]),
        "{path}: the facts are not in FACT_ORDER: {places:?}"
    );
    assert!(
        record
            .cgroup
            .as_deref()
            .is_some_and(|cgroup| !cgroup.is_empty()),
        "{path}: the record names a cgroup: {:?}",
        record.cgroup
    );
    assert_eq!(
        record.cgroup.as_deref(),
        Some(applied.cgroup.as_str()),
        "{path}"
    );
    assert_eq!(record.requires, None, "{path}: no requirement is echoed");
}

fn delegated(name: &str, case: impl FnOnce(&SubstrateRunner, &Path)) {
    support::delegated(name, &[], case);
}

#[test]
fn security_w20261121w1_u1_a_child_run_to_its_end_records_the_five_facts() {
    delegated(
        "security_w20261121w1_u1_a_child_run_to_its_end_records_the_five_facts",
        |runner, workspace| {
            let result = runner
                .run_child(&ChildCommand::new("true", [] as [&str; 0], workspace))
                .expect("the child runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            let record = result.driver_record.as_ref().expect("the driver's record");
            conforms("run_child", &result.applied, record);
        },
    );
}

#[test]
fn security_w20261121w1_u1_a_child_given_stdin_records_the_five_facts() {
    delegated(
        "security_w20261121w1_u1_a_child_given_stdin_records_the_five_facts",
        |runner, workspace| {
            let result = runner
                .run_child(
                    &ChildCommand::new("cat", [] as [&str; 0], workspace)
                        .with_stdin(b"in\n".to_vec()),
                )
                .expect("the child given standard input runs confined");
            assert_eq!(result.exit_code, Some(0), "{}", result.output);
            assert_eq!(result.child_output.stdout, b"in\n");
            let record = result.driver_record.as_ref().expect("the driver's record");
            conforms("run_child with stdin", &result.applied, record);
        },
    );
}

#[test]
fn security_w20261121w1_u1_a_live_child_records_the_five_facts() {
    delegated(
        "security_w20261121w1_u1_a_live_child_records_the_five_facts",
        |runner, workspace| {
            let mut child = runner
                .spawn_child(&ChildCommand::new("cat", [] as [&str; 0], workspace))
                .expect("cat starts live, confined");
            child.close_stdin().unwrap();
            let result = child.wait().expect("the live child's result");
            assert_eq!(result.exit.exit_code, Some(0));
            let record = result.driver_record.as_ref().expect("the driver's record");
            conforms("spawn_child", &result.applied, record);
        },
    );
}
