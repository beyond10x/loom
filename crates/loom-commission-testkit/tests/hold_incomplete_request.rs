//! `story:ess-096-upgrade`, decision 2a: a hold with some but not all of its four request members
//! (`action_request_id`, `expected_case_revision`, `action`, `arguments`) is refused
//! `incomplete-request`, naming the Suspension, whatever its reason, and this check runs before
//! `already-held`: a mixed hold for a commission that already holds a Suspension answers
//! `incomplete-request` and the held Suspension stays as it was. Over the in-memory store and over
//! `FileSuspensionStore`.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CaseId, CommissionId, DecisionHold, DecisionUnblocks, HoldSuspension,
    HoldSuspensionOutcome, ProposedActionArguments, RunId, SuspensionKey, SuspensionReason,
    SuspensionRequestIncomplete,
};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> CommissionId {
    CommissionId(uuid(0x91))
}

fn key() -> SuspensionKey {
    SuspensionKey {
        commission_id: commission(),
    }
}

/// A hold for the commission with `reason`, carrying the members whose bit is set in `mask`
/// (1 request, 2 expected case revision, 4 action, 8 arguments).
fn hold(reason: SuspensionReason, mask: u8) -> HoldSuspension {
    HoldSuspension {
        suspension: key(),
        commission_id: commission(),
        run_id: RunId(uuid(0x101)),
        run_case_revision: 1,
        action_request_id: (mask & 1 != 0).then(|| ActionRequestId(uuid(0x301))),
        case_id: CaseId("case-incomplete".to_owned()),
        expected_case_revision: (mask & 2 != 0).then_some(1),
        action: (mask & 4 != 0).then(|| "repository.merge".to_owned()),
        arguments: (mask & 8 != 0).then(|| ProposedActionArguments(Value::Object(Vec::new()))),
        reason,
    }
}

fn reasons() -> [SuspensionReason; 2] {
    [
        SuspensionReason::Authority(Value::Null),
        SuspensionReason::Decision(DecisionHold {
            request_id: "DR-1".to_owned(),
            unblocks: DecisionUnblocks::Question,
        }),
    ]
}

/// Holds a full `Authority` hold on `port`, then sends every mixed hold (14 member sets per
/// reason, both reasons) for the same commission, and returns every answer that is not
/// `IncompleteRequest` naming the Suspension, or that left the held Suspension changed.
fn mixed_holds_after_a_held_one(port: &dyn SuspensionPort) -> Vec<String> {
    let held = port.hold(hold(SuspensionReason::Authority(Value::Null), 15));
    assert!(
        matches!(held, HoldSuspensionOutcome::Held { .. }),
        "the full hold answered {held:?}"
    );
    let first = port
        .get(&commission())
        .expect("the store reads")
        .expect("the full hold is stored");
    let mut wrong = Vec::new();
    for reason in reasons() {
        for mask in 1u8..15 {
            let outcome = port.hold(hold(reason.clone(), mask));
            let expected = HoldSuspensionOutcome::IncompleteRequest {
                error: SuspensionRequestIncomplete { suspension: key() },
            };
            if outcome != expected {
                wrong.push(format!("{mask:04b} {reason:?}: answered {outcome:?}"));
            }
            let now = port.get(&commission()).expect("the store reads");
            if now.as_ref() != Some(&first) {
                wrong.push(format!(
                    "{mask:04b} {reason:?}: the held one is now {now:?}"
                ));
            }
        }
    }
    wrong
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("hold-incomplete-request")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_mixed_hold_after_a_held_one_is_incomplete_on_the_memory_store() {
    let wrong = mixed_holds_after_a_held_one(&SuspensionStore::new());
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_mixed_hold_after_a_held_one_is_incomplete_on_the_file_store() {
    let store = FileSuspensionStore::open(store_dir("after-held")).expect("the store opens");
    let wrong = mixed_holds_after_a_held_one(&store);
    assert!(wrong.is_empty(), "{wrong:#?}");
}
