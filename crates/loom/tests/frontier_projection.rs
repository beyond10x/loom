//! Acceptance for `story:frontier-projection`: Loom derives the model-visible action catalogue from
//! the current frontier only (Atlas ADR 0072). Admissible and approval-gated actions are projected,
//! blocked ones are not, and each catalogue carries the case revision of the frontier it was
//! projected from.
//!
//! The frontiers the scripted fake governor serves are transcribed from ELS
//! `docs/examples/software-change.md` (case CHG-1842), as in `agent_executor.rs`: "Initial",
//! "After `tests.run` on R2" and "After authority", typed as Commission's generated `Frontier`
//! items. ELS `software.change/1` has no machine fixture yet. Loom knows nothing of why merge waits
//! on `tests.pass`; the frontier says so, and the test reads the claim from the frontier.

use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, Frontier, FrontierAction, FrontierClaim, Truth, frontier_state,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{
    ActionCatalogueData, ActionCatalogueState, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    TurnId,
};
use b10x_loom::projection::project;

/// The case of the ELS example.
const CASE: &str = "CHG-1842";

/// The action that waits on `tests.pass`.
const MERGE: &str = "repository.merge";

/// The claim merge waits on.
const TESTS_PASS: &str = "tests.pass";

/// The capability the frontier names for the approval-gated merge.
const MERGE_CAPABILITY: &str = "repository.write";

/// One projection per scripted frontier.
const TURNS: usize = 3;

fn action(
    name: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
    }
}

fn tests_pass(value: Truth) -> Vec<FrontierClaim> {
    vec![FrontierClaim {
        claim: TESTS_PASS.to_owned(),
        value,
    }]
}

/// The three actions the example keeps admissible throughout, then `merge`.
fn actions_with(merge: FrontierAction) -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None, &[]),
        action("repository.edit", ActionStatus::Admissible, None, &[]),
        action("tests.run", ActionStatus::Admissible, None, &[]),
        merge,
    ]
}

/// "Initial", revision 1: `tests.pass` is unknown and merge is blocked.
fn initial() -> Answer {
    Answer::at(1).with_items(
        tests_pass(Truth::Unknown),
        Vec::new(),
        actions_with(action(
            MERGE,
            ActionStatus::Blocked,
            None,
            &["tests.pass is Unknown, required True"],
        )),
    )
}

/// "After `tests.run` on R2", revision 2: `tests.pass` is true and merge needs approval.
fn after_tests_on_r2() -> Answer {
    Answer::at(2).with_items(
        tests_pass(Truth::True),
        Vec::new(),
        actions_with(action(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some(MERGE_CAPABILITY),
            &[],
        )),
    )
}

/// "After authority", revision 3: merge is admissible.
fn after_authority() -> Answer {
    Answer::at(3).with_items(
        tests_pass(Truth::True),
        Vec::new(),
        actions_with(action(MERGE, ActionStatus::Admissible, None, &[])),
    )
}

fn uuid(n: usize) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

/// The entries the example's frontier at `revision` projects to, in the frontier's order.
fn expected_entries(revision: i64) -> Vec<CatalogueEntry> {
    let mut entries = vec![
        entry("repository.inspect", CatalogueEntryStatus::Admissible),
        entry("repository.edit", CatalogueEntryStatus::Admissible),
        entry("tests.run", CatalogueEntryStatus::Admissible),
    ];
    match revision {
        1 => {}
        2 => entries.push(entry(MERGE, CatalogueEntryStatus::ApprovalRequired)),
        3 => entries.push(entry(MERGE, CatalogueEntryStatus::Admissible)),
        other => panic!("the script has no revision {other}"),
    }
    entries
}

/// The value of `tests.pass` the frontier states.
fn tests_pass_on(frontier: &Frontier<frontier_state::Issued>) -> Option<Truth> {
    frontier
        .data()
        .claims
        .iter()
        .find(|claim| claim.claim == TESTS_PASS)
        .map(|claim| claim.value)
}

fn lists(catalogue: &ActionCatalogueData, action: &str) -> bool {
    catalogue.entries.iter().any(|entry| entry.action == action)
}

/// The fake governor serves the software-change frontier for CHG-1842 at revisions 1, 2 and 3; one
/// catalogue is projected per turn from the frontier current at that turn.
#[test]
fn projection_follows_frontier() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [initial(), after_tests_on_r2(), after_authority()],
    );

    let mut projected = Vec::new();
    for turn in 0..TURNS {
        let frontier = governor
            .frontier(&case)
            .unwrap_or_else(|error| panic!("frontier for {CASE} failed: {error:?}"));
        let catalogue_id = CatalogueId(uuid(0xc00 + turn));
        let turn_id = TurnId(uuid(0x700 + turn));
        let catalogue = project(&frontier, catalogue_id.clone(), turn_id.clone());

        assert_eq!(catalogue.state(), ActionCatalogueState::Projected);
        let data = catalogue.into_data();
        assert_eq!(data.catalogue_id, catalogue_id, "turn {turn}: catalogue id");
        assert_eq!(
            data.turn_id, turn_id,
            "turn {turn}: a catalogue carries the turn it is projected for"
        );
        assert_eq!(
            data.frontier,
            frontier.data().frontier_id.0.0,
            "turn {turn}: a catalogue names the frontier it was projected from"
        );
        projected.push((frontier, data));
    }

    // 4. Each catalogue's case revision is its frontier's, and the governor did move.
    for (frontier, catalogue) in &projected {
        assert_eq!(
            catalogue.case_revision,
            frontier.data().case_revision,
            "a catalogue's case_revision is the case revision of its frontier: {catalogue:?}"
        );
    }
    let revisions: Vec<i64> = projected
        .iter()
        .map(|(_, catalogue)| catalogue.case_revision)
        .collect();
    assert_eq!(
        revisions,
        vec![1, 2, 3],
        "the fake governor served revisions 1, 2, 3"
    );

    // 1. Where the frontier does not state tests.pass TRUE, merge is not projected.
    let untested: Vec<&ActionCatalogueData> = projected
        .iter()
        .filter(|(frontier, _)| tests_pass_on(frontier) != Some(Truth::True))
        .map(|(_, catalogue)| catalogue)
        .collect();
    assert!(
        !untested.is_empty(),
        "the script serves a frontier where tests.pass is not TRUE"
    );
    for catalogue in &untested {
        assert!(
            !lists(catalogue, MERGE),
            "{MERGE} projected at revision {} where tests.pass is not TRUE: {catalogue:?}",
            catalogue.case_revision
        );
    }

    // 2. The first projection after the governor moves to tests.pass TRUE contains merge.
    let (_, first_tested) = projected
        .iter()
        .find(|(frontier, _)| tests_pass_on(frontier) == Some(Truth::True))
        .expect("the script serves a frontier where tests.pass is TRUE");
    assert!(
        lists(first_tested, MERGE),
        "{MERGE} not projected at revision {} where tests.pass is TRUE: {first_tested:?}",
        first_tested.case_revision
    );

    // 3. A blocked action is never projected; an approval-gated one is, with its status.
    for (frontier, catalogue) in &projected {
        for listed in &frontier.data().actions {
            let entries: Vec<&CatalogueEntry> = catalogue
                .entries
                .iter()
                .filter(|entry| entry.action == listed.action)
                .collect();
            match listed.status {
                ActionStatus::Blocked => assert!(
                    entries.is_empty(),
                    "blocked {} projected at revision {}: {entries:?}",
                    listed.action,
                    catalogue.case_revision
                ),
                ActionStatus::ApprovalRequired => assert_eq!(
                    entries,
                    vec![&entry(
                        &listed.action,
                        CatalogueEntryStatus::ApprovalRequired
                    )],
                    "approval-gated {} at revision {}",
                    listed.action,
                    catalogue.case_revision
                ),
                ActionStatus::Admissible => assert_eq!(
                    entries,
                    vec![&entry(&listed.action, CatalogueEntryStatus::Admissible)],
                    "admissible {} at revision {}",
                    listed.action,
                    catalogue.case_revision
                ),
            }
        }
    }

    // The whole catalogue: one entry per projected frontier action, in the frontier's order, and
    // nothing the frontier does not list.
    for (_, catalogue) in &projected {
        assert_eq!(
            catalogue.entries,
            expected_entries(catalogue.case_revision),
            "catalogue at revision {}",
            catalogue.case_revision
        );
    }
}
