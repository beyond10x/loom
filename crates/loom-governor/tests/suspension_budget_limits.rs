//! A Suspension held for a run budget's breach reads back as it was held, whichever limit the
//! breach names: the two `story:run-budget-tokens-and-deadline` adds, `MaxTokens` and
//! `Deadline`, as the six before them, and a `Deadline` breach whose reading charged nothing.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, BudgetLimit, CaseId, CommissionId, HoldSuspension, HoldSuspensionOutcome,
    ProposedActionArguments, RunBudgetBreach, RunId, SuspensionKey, SuspensionReason,
};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use loom_governor::FileSuspensionStore;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("suspension-budget-limits")
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
        case_id: CaseId("case-limits".to_owned()),
        expected_case_revision: 1,
        action: "evaluate".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
        reason,
    }
}

#[test]
fn every_budget_limit_reads_back_as_held() {
    let store = FileSuspensionStore::open(store_dir("every-limit")).expect("the store opens");
    let breaches = [
        (BudgetLimit::MaxSteps, Some(4)),
        (BudgetLimit::MaxTrials, Some(2)),
        (BudgetLimit::MaxTokensIn, Some(1_200)),
        (BudgetLimit::MaxTokensOut, Some(80)),
        (BudgetLimit::MaxDurationMs, Some(9_000)),
        (BudgetLimit::MaxCostMicroUsd, None),
        (BudgetLimit::MaxTokens, Some(110)),
        (BudgetLimit::Deadline, Some(1_792_576_800)),
        (BudgetLimit::Deadline, None),
    ];
    for (n, (limit, charged)) in (1..).zip(breaches) {
        let reason = SuspensionReason::RunBudget(RunBudgetBreach {
            action: (limit == BudgetLimit::MaxTrials).then(|| "evaluate".to_owned()),
            limit,
            max: 1_792_575_000,
            charged,
        });
        let input = hold(n, reason.clone());
        let commission = input.commission_id.clone();
        assert!(
            matches!(store.hold(input), HoldSuspensionOutcome::Held { .. }),
            "the Suspension for {limit:?} is held"
        );
        let held = store
            .get(&commission)
            .expect("the store reads")
            .expect("the Suspension is stored");
        assert_eq!(held.data.reason, reason, "{limit:?} reads back as held");
    }
}
