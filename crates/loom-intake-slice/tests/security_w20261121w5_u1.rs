//! Security review, wave 2026-11-21-w5, unit U1 (`story:ess-096-upgrade`), pass 1, the named
//! case through the host: a held Suspension of a reason other than `Decision` with no request
//! member, held through `FileSuspensionStore` as `correction-0` now allows, satisfies no check
//! that reads "held for authority". `approve` writes no grant for it, whatever capability it is
//! asked for, `Pending` or `Resuming`; `GrantAuthority` allows nothing on it, even given a grant
//! for the commission's capability; the same `approve` on a plain `Authority` hold with its
//! request writes the grant, so the refusal is the missing request and nothing else.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId as HeldRequestId, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, CaseId, CommissionData, CommissionId as HeldCommissionId, HoldSuspension,
    HoldSuspensionOutcome, PrincipalId, ProposedActionArguments, RunId, SuspensionKey,
    SuspensionReason, TakeSuspension, TakeSuspensionOutcome,
};
use b10x_loom_commission::ports::authority::AuthorityProvider;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::grant::{
    ActionRequestId, ApproveRefusal, CommissionId, GrantAuthority, GrantData, GrantStore, approve,
    now,
};
use b10x_loom_intake_slice::handoff::MergeHandoffStore;
use loom_governor::FileSuspensionStore;

const COMMISSION: &str = "00000000-0000-4000-8000-0000000005c1";
const REQUEST: &str = "00000000-0000-4000-8000-0000000005c3";
const CASE: &str = "case-sec-w5-u1-approve";
const MERGE: &str = "repository.merge";

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w20261121w5-u1")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn key() -> SuspensionKey {
    SuspensionKey {
        commission_id: HeldCommissionId(Uuid(COMMISSION.to_owned())),
    }
}

fn authority(capability: &str) -> SuspensionReason {
    SuspensionReason::Authority(Value::Object(vec![(
        "capability".to_owned(),
        Value::Text(capability.to_owned()),
    )]))
}

/// A hold of `reason` with all four request members, or with none.
fn hold(reason: SuspensionReason, with_request: bool) -> HoldSuspension {
    HoldSuspension {
        suspension: key(),
        commission_id: HeldCommissionId(Uuid(COMMISSION.to_owned())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-0000000005c2".to_owned())),
        run_case_revision: 1,
        action_request_id: with_request.then(|| HeldRequestId(Uuid(REQUEST.to_owned()))),
        case_id: CaseId(CASE.to_owned()),
        expected_case_revision: with_request.then_some(1),
        action: with_request.then(|| MERGE.to_owned()),
        arguments: with_request.then(|| ProposedActionArguments(Value::Object(Vec::new()))),
        reason,
    }
}

fn grants_written(grants: &GrantStore) -> Vec<String> {
    std::fs::read_dir(grants.dir())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("grant-"))
        .collect()
}

fn approve_on(
    suspensions: &dyn SuspensionPort,
    root: &std::path::Path,
    capability: &str,
) -> (Result<GrantData, ApproveRefusal>, Vec<String>) {
    let grants = GrantStore::open(root.join("grants")).expect("the grant store opens");
    let handoffs = MergeHandoffStore::open(root.join("merges")).expect("the hand-offs open");
    let head_of = |_: &CaseId| Ok("0123abcd".to_owned());
    let approved = approve(
        suspensions,
        &grants,
        &handoffs,
        &head_of,
        COMMISSION,
        capability,
        now(),
        None,
    );
    (approved, grants_written(&grants))
}

/// A grant for the commission as `approve` writes one for a plain hold of `REQUEST`.
fn grant() -> GrantData {
    GrantData {
        commission_id: CommissionId(intake_model::primitives::Uuid(COMMISSION.to_owned())),
        action: MERGE.to_owned(),
        capability: MERGE.to_owned(),
        granted: true,
        action_request_id: ActionRequestId(intake_model::primitives::Uuid(REQUEST.to_owned())),
        granted_at: now(),
        mandate_approval: None,
    }
}

fn commission() -> CommissionData {
    CommissionData {
        commission_id: HeldCommissionId(Uuid(COMMISSION.to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("agent".to_owned())),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    }
}

fn asked() -> ActionRequestData {
    ActionRequestData {
        action_request_id: HeldRequestId(Uuid(REQUEST.to_owned())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-0000000005c2".to_owned())),
        case_id: CaseId(CASE.to_owned()),
        expected_case_revision: 1,
        action: MERGE.to_owned(),
        arguments: ProposedActionArguments(Value::Object(Vec::new())),
    }
}

#[test]
fn security_w5_u1_approve_writes_no_grant_for_a_hold_without_a_request() {
    let reasons = [
        ("authority-merge", authority(MERGE)),
        ("authority-edit", authority("implementation.edit")),
        ("budget", SuspensionReason::Budget(Value::Null)),
        ("time", SuspensionReason::Time(Value::Null)),
    ];
    for (name, reason) in reasons {
        let root = dir(&format!("approve-{name}"));
        let suspensions =
            FileSuspensionStore::open(root.join("suspensions")).expect("the store opens");
        let held = suspensions.hold(hold(reason, false));
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "{name}: {held:?}"
        );
        let before = suspensions
            .get(&key().commission_id)
            .expect("the store reads");
        for capability in [MERGE, "implementation.edit"] {
            let (approved, written) = approve_on(&suspensions, &root, capability);
            assert_eq!(
                approved,
                Err(ApproveRefusal::OtherCapability { held: None }),
                "{name} / {capability}: approve answered {approved:?}"
            );
            assert!(
                written.is_empty(),
                "{name} / {capability}: wrote {written:?}"
            );
        }
        // Pending, a grant for the commission's capability and the request a plain hold would
        // name allows nothing.
        let provider = GrantAuthority::new(Some(grant()), &suspensions);
        let verdict = provider.decide(&commission(), &asked(), MERGE);
        assert!(
            matches!(verdict, Ok(AuthorityVerdict::ApprovalRequired(_))),
            "{name}: Pending, GrantAuthority answered {verdict:?}"
        );
        assert_eq!(
            suspensions
                .get(&key().commission_id)
                .expect("the store reads"),
            before,
            "{name}"
        );

        // Resuming, after a take naming neither request: still no grant, still nothing allowed.
        let taken = suspensions.take(TakeSuspension {
            suspension: key(),
            action_request_id: None,
            decision_request_id: None,
        });
        assert!(
            matches!(taken, TakeSuspensionOutcome::Taken { .. }),
            "{name}: {taken:?}"
        );
        let (approved, written) = approve_on(&suspensions, &root, MERGE);
        assert_eq!(
            approved,
            Err(ApproveRefusal::OtherCapability { held: None }),
            "{name}: Resuming, approve answered {approved:?}"
        );
        assert!(written.is_empty(), "{name}: Resuming, wrote {written:?}");
        let verdict = provider.decide(&commission(), &asked(), MERGE);
        assert!(
            matches!(verdict, Ok(AuthorityVerdict::ApprovalRequired(_))),
            "{name}: Resuming, GrantAuthority answered {verdict:?}"
        );
    }
}

/// The comparison: the same `approve` on a plain `Authority` hold with its request writes the
/// grant, and the provider then allows exactly that request.
#[test]
fn security_w5_u1_approve_writes_the_grant_for_a_plain_hold() {
    let root = dir("approve-plain");
    let suspensions = FileSuspensionStore::open(root.join("suspensions")).expect("the store opens");
    let held = suspensions.hold(hold(authority(MERGE), true));
    assert!(
        matches!(held, HoldSuspensionOutcome::Held { .. }),
        "{held:?}"
    );
    let (approved, written) = approve_on(&suspensions, &root, MERGE);
    let approved = approved.expect("approve writes the grant for a plain hold");
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(approved.action_request_id.0.0, REQUEST);
    let provider = GrantAuthority::new(Some(approved), &suspensions);
    let verdict = provider.decide(&commission(), &asked(), MERGE);
    assert!(
        matches!(verdict, Ok(AuthorityVerdict::Allow(_))),
        "GrantAuthority answered {verdict:?}"
    );
}
