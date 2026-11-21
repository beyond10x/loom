//! Security review, wave 2026-11-21-w3, unit U2 (`story:decision-suspension-reason`): `approve` on
//! a commission held for a decision writes no grant, whether the hold is `Pending` or `Resuming`,
//! and whether or not request members were smuggled into it (invariants 5 and 6).

use std::path::PathBuf;

use b10x_loom_commission::model::behaviour::SuspensionStorage;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CaseId, CommissionId, DecisionHold, DecisionUnblocks, ProposedActionArguments,
    RunId, SuspensionData, SuspensionKey, SuspensionReason, SuspensionSnapshot, SuspensionState,
};
use b10x_loom_commission::ports::suspension::SuspensionStore;
use b10x_loom_intake_slice::grant::{ApproveRefusal, GrantStore, approve, now};
use b10x_loom_intake_slice::handoff::MergeHandoffStore;

const COMMISSION: &str = "00000000-0000-4000-8000-0000000000d1";
const MERGE: &str = "repository.merge";

fn hold(state: SuspensionState, smuggled: bool) -> SuspensionSnapshot {
    let commission_id = CommissionId(Uuid(COMMISSION.to_owned()));
    SuspensionSnapshot {
        state,
        data: SuspensionData {
            suspension: SuspensionKey {
                commission_id: commission_id.clone(),
            },
            commission_id,
            run_id: RunId(Uuid("00000000-0000-4000-8000-0000000000d2".to_owned())),
            run_case_revision: 1,
            action_request_id: smuggled
                .then(|| ActionRequestId(Uuid("00000000-0000-4000-8000-0000000000d3".to_owned()))),
            case_id: CaseId("case-sec-approve".to_owned()),
            expected_case_revision: smuggled.then_some(1),
            action: smuggled.then(|| MERGE.to_owned()),
            arguments: smuggled.then(|| ProposedActionArguments(Value::Object(Vec::new()))),
            reason: SuspensionReason::Decision(DecisionHold {
                request_id: "DR-1".to_owned(),
                unblocks: DecisionUnblocks::Approval,
            }),
        },
    }
}

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w20261121w3-u2-approve")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn security_approve_writes_no_grant_for_a_decision_hold() {
    for (name, state, smuggled) in [
        ("pending", SuspensionState::Pending, false),
        ("resuming", SuspensionState::Resuming, false),
        ("pending-smuggled", SuspensionState::Pending, true),
        ("resuming-smuggled", SuspensionState::Resuming, true),
    ] {
        let mut suspensions = SuspensionStore::new();
        let held = hold(state, smuggled);
        SuspensionStorage::put(&mut suspensions, held.clone());
        let root = dir(name);
        let grants = GrantStore::open(root.join("grants")).expect("the grant store opens");
        let handoffs = MergeHandoffStore::open(root.join("merges")).expect("the hand-offs open");
        let head_of = |_: &CaseId| Ok("0123abcd".to_owned());

        let approved = approve(
            &suspensions,
            &grants,
            &handoffs,
            &head_of,
            COMMISSION,
            MERGE,
            now(),
            None,
        );

        assert!(
            matches!(
                approved,
                Err(ApproveRefusal::OtherCapability { held: None })
                    | Err(ApproveRefusal::InDoubt { .. })
            ),
            "{name}: approve answered {approved:?}"
        );
        let written: Vec<String> = std::fs::read_dir(grants.dir())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != ".lock")
            .collect();
        assert!(
            written.is_empty(),
            "{name}: the grant store wrote {written:?}"
        );
        assert_eq!(
            SuspensionStorage::list(&suspensions),
            vec![held],
            "{name}: the Suspension changed"
        );
    }
}
