//! An artifact revision update to the revision the case already holds changes nothing (story
//! `canon-governor`, adversary pass 2, finding 4).

use std::collections::BTreeMap;

use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, MemoryCaseStore};

const SOFTWARE_CHANGE: &str = "software-change@1";

/// `update_revision` with the revision the artifact already holds returns the current case
/// revision and leaves the revisions, the case revision and the frontier, id included, as they
/// were; it does so both on a freshly opened case and after a real update.
#[test]
fn an_update_to_the_revision_already_held_changes_nothing() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");

    for (step, held, case_revision) in [("opened", "R2", 1), ("after a real update", "R3", 2)] {
        if held == "R3" {
            assert_eq!(
                governor.update_revision(&case, "implementation", "R3"),
                Ok(2),
                "a new revision raises the case revision"
            );
        }
        let revisions = governor.revisions(&case).expect("held");
        let frontier = governor.frontier(&case).expect("frontier").into_data();
        assert_eq!(revisions["implementation"], held, "{step}");

        assert_eq!(
            governor.update_revision(&case, "implementation", held),
            Ok(case_revision),
            "{step}: the update returns the current case revision"
        );
        assert_eq!(
            governor.current_revision(&case).expect("held"),
            case_revision,
            "{step}: the case revision is unchanged"
        );
        assert_eq!(
            governor.revisions(&case).expect("held"),
            revisions,
            "{step}: the revisions are unchanged"
        );
        assert_eq!(
            governor.frontier(&case).expect("frontier").into_data(),
            frontier,
            "{step}: the frontier is unchanged"
        );
    }
}

fn software_change_revisions() -> BTreeMap<String, String> {
    [
        ("intent", "i1"),
        ("system_specification", "s1"),
        ("plan", "p1"),
        ("implementation", "R2"),
        ("release", "v0"),
        ("deployment", "d0"),
    ]
    .into_iter()
    .map(|(artifact, revision)| (artifact.to_owned(), revision.to_owned()))
    .collect()
}
