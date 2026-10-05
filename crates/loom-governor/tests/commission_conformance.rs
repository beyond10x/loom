//! Commission's governor conformance kit (`b10x_loom_commission_testkit::kits::governor`, story
//! `adapter-conformance-suites`) run against the Canon-backed governor on a `software-change@1`
//! case.

use std::collections::BTreeMap;
use std::sync::Mutex;

use b10x_loom_commission::model::responsibility::{CaseId, EvidenceData, ObservationData};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission_testkit::kits::governor::{Check, GovernorFixture, run};
use loom_governor::{CanonGovernor, MemoryCaseStore};

/// Holds one case per governor, opened on `software-change@1`; a move records a new
/// implementation revision.
#[derive(Default)]
struct Fixture {
    held: Mutex<Option<CaseId>>,
}

impl Fixture {
    fn held(&self) -> CaseId {
        self.held
            .lock()
            .expect("fixture lock")
            .clone()
            .expect("the kit holds a case before it asks about it")
    }
}

impl GovernorFixture for Fixture {
    type Governor = CanonGovernor<MemoryCaseStore>;

    fn hold(&self, case: &CaseId) -> Self::Governor {
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let revisions: BTreeMap<String, String> = [
            ("intent", "i1"),
            ("system_specification", "s1"),
            ("plan", "p1"),
            ("implementation", "R1"),
            ("release", "v0"),
            ("deployment", "d0"),
        ]
        .into_iter()
        .map(|(artifact, revision)| (artifact.to_owned(), revision.to_owned()))
        .collect();
        governor
            .open_case(case.clone(), "software-change@1", revisions)
            .expect("the held case opens");
        *self.held.lock().expect("fixture lock") = Some(case.clone());
        governor
    }

    fn advance(&self, governor: &Self::Governor, case: &CaseId) {
        let next = governor.current_revision(case).expect("held case") + 1;
        governor
            .update_revision(case, "implementation", &format!("R{next}"))
            .expect("a new implementation revision");
    }

    fn observations(&self, governor: &Self::Governor) -> Vec<ObservationData> {
        governor.observations()
    }

    fn evidence(&self, governor: &Self::Governor) -> Vec<EvidenceData> {
        governor.evidence(&self.held()).expect("held case")
    }
}

#[test]
fn the_canon_governor_passes_commissions_governor_conformance_kit() {
    let report = run(&Fixture::default());
    assert_eq!(report.ran(), Check::ALL, "every check of the kit ran");
    report.assert_passed();
}
