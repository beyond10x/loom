//! `story:host-stop-before-step`: a Suspension held for a host's stop reads back as it was held,
//! the host's name and its detail unchanged, whatever JSON the detail holds.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CaseId, CommissionId, HoldSuspension, HoldSuspensionOutcome, HostStop,
    ProposedActionArguments, RunId, SuspensionKey, SuspensionReason,
};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use loom_governor::FileSuspensionStore;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("suspension-host-stop")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hold(n: u64, reason: SuspensionReason) -> HoldSuspension {
    let commission_id = CommissionId(uuid(0x100 + n));
    HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(0x200 + n)),
        run_case_revision: 1,
        action_request_id: ActionRequestId(uuid(0x300 + n)),
        case_id: CaseId("case-host-stop".to_owned()),
        expected_case_revision: 1,
        action: "evaluate".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
        reason,
    }
}

#[test]
fn a_host_stop_round_trips_through_the_file_suspension_store() {
    let store = FileSuspensionStore::open(store_dir("round-trip")).expect("the store opens");
    let stops = [
        HostStop {
            name: "DiskLow".to_owned(),
            detail: Value::Object(vec![
                ("path".to_owned(), Value::Text("target".to_owned())),
                ("free_bytes".to_owned(), Value::Number("1048576".to_owned())),
                (
                    "threshold_bytes".to_owned(),
                    Value::Number("21474836480".to_owned()),
                ),
            ]),
        },
        HostStop {
            name: "ActiveTime".to_owned(),
            detail: Value::Array(vec![
                Value::Number("5400000".to_owned()),
                Value::Text("paused \"on the forge\"\n\u{e9}".to_owned()),
                Value::Bool(false),
                Value::Null,
                Value::Object(Vec::new()),
            ]),
        },
        HostStop {
            name: String::new(),
            detail: Value::Null,
        },
        HostStop {
            name: "a \"quoted\" name\twith a tab".to_owned(),
            detail: Value::Text("detail".to_owned()),
        },
    ];
    for (n, stop) in (1..).zip(stops) {
        let reason = SuspensionReason::HostStop(stop.clone());
        let input = hold(n, reason.clone());
        let commission = input.commission_id.clone();
        assert!(
            matches!(store.hold(input), HoldSuspensionOutcome::Held { .. }),
            "the Suspension for {stop:?} is held"
        );
        let held = store
            .get(&commission)
            .expect("the store reads")
            .expect("the Suspension is stored");
        assert_eq!(held.data.reason, reason, "{stop:?} reads back as held");
    }
}

/// A store reopened on the same directory, as after a restart, reads the host's stop back.
#[test]
fn a_host_stop_reads_back_after_the_store_is_reopened() {
    let dir = store_dir("reopened");
    let reason = SuspensionReason::HostStop(HostStop {
        name: "DiskLow".to_owned(),
        detail: Value::Object(vec![(
            "free_bytes".to_owned(),
            Value::Number("0".to_owned()),
        )]),
    });
    let input = hold(9, reason.clone());
    let commission = input.commission_id.clone();
    {
        let store = FileSuspensionStore::open(&dir).expect("the store opens");
        assert!(matches!(
            store.hold(input),
            HoldSuspensionOutcome::Held { .. }
        ));
    }
    let reopened = FileSuspensionStore::open(&dir).expect("the store opens again");
    let held = reopened
        .get(&commission)
        .expect("the store reads")
        .expect("the Suspension is stored");
    assert_eq!(held.data.reason, reason);
}
