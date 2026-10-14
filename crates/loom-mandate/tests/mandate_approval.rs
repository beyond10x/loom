//! Acceptance for `story:mandate-approval-on-resume`, Mandate's side: `HostedMandate` records the
//! approval of the request a commission's Suspension holds, and `clear` asks Mandate's
//! `check_with_approval` with it over the approval event log, answering the
//! `ApprovedDecisionRecorded` event that consumes it. An approval naming another subject, action or
//! resource than the request is refused, as are an expired and a consumed one; nothing is consumed
//! then. Ids are synthetic.

use b10x_loom_commission::model::primitives::Uuid as Id;
use b10x_loom_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, CommissionData, CommissionId as HeldCommissionId,
    PrincipalId,
};
use intake_model::json as intake_json;
use intake_model::primitives::{Timestamp, Uuid};
use intake_model::routing::{
    ActionRequestId, CommissionId, MandateApprovalEntry, MandateApprovalId, MandateApprovalRecorded,
};
use loom_mandate::approval::{HeldRequest, HostedMandate, fold};
use loom_mandate::{CASE_RESOURCE_TYPE, case_resource_id, verified_context};
use mandate_authz::agent::AgentAuthorityRead;
use mandate_authz::delegation::ApprovalState;
use mandate_types::ApprovalId;
use serde_json::{Value, json};

const CAPABILITY: &str = "repository.merge";
const CASE: &str = "case-1";
const COMMISSION: &str = "00000000-0000-4000-8000-000000000081";
const REQUEST: &str = "00000000-0000-4000-8000-0000000000a1";
const APPROVAL: &str = "00000000-0000-4000-8000-0000000000e1";
const SUBJECT: &str = "22222222-2222-4222-8222-222222222222";
const ORGANIZATION: &str = "11111111-1111-4111-8111-111111111111";
const APPROVER: &str = "99999999-9999-4999-8999-999999999999";
const RECORDED_AT: &str = "2026-10-14T09:00:00Z";
const EXPIRES_AT: &str = "2026-10-14T10:00:00Z";
const BEFORE_EXPIRY: &str = "2026-10-14T09:30:00Z";

fn host_document() -> Value {
    json!({
        "verified": {
            "subject": SUBJECT,
            "organization": ORGANIZATION,
            "audience": "loom",
            "credential": "77777777-7777-4777-8777-777777777777",
            "correlation": "correlation-approval"
        },
        "approver": APPROVER,
        "approver_policy": "40404040-4040-4040-8040-404040404040",
        "relation": "maintainer",
        "capabilities": [CAPABILITY, "tests.run"],
        "approval_required": [CAPABILITY],
        "approval_seconds": 3600
    })
}

fn hosted() -> HostedMandate {
    HostedMandate::parse(&host_document().to_string()).unwrap_or_else(|why| panic!("{why}"))
}

fn commission(hosted: &HostedMandate) -> CommissionData {
    CommissionData {
        commission_id: HeldCommissionId(Id(COMMISSION.to_owned())),
        agent_revision_id: AgentRevisionId(Id("00000000-0000-4000-8000-000000000082".to_owned())),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("operator".to_owned()),
        authority_context: hosted.authority_context(),
    }
}

fn request() -> ActionRequestId {
    ActionRequestId(Uuid(REQUEST.to_owned()))
}

fn approval_id() -> MandateApprovalId {
    MandateApprovalId(Uuid(APPROVAL.to_owned()))
}

/// The approval `approve --mandate` records for the held request.
fn recorded(hosted: &HostedMandate) -> MandateApprovalRecorded {
    let case = CaseId(CASE.to_owned());
    let commission_id = CommissionId(Uuid(COMMISSION.to_owned()));
    let request = request();
    hosted
        .approval(
            &HeldRequest {
                commission_id: &commission_id,
                action_request_id: &request,
                case_id: &case,
                capability: CAPABILITY,
            },
            &approval_id(),
            &Timestamp(RECORDED_AT.to_owned()),
            &Timestamp(EXPIRES_AT.to_owned()),
        )
        .unwrap_or_else(|why| panic!("{why}"))
}

fn clear(
    hosted: &HostedMandate,
    log: &[MandateApprovalEntry],
    at: &str,
) -> Result<MandateApprovalEntry, loom_mandate::approval::Refused> {
    match hosted.clear(
        &commission(hosted),
        CAPABILITY,
        &request(),
        &approval_id(),
        log,
        &Timestamp(at.to_owned()),
        "00000000-0000-4000-8000-0000000000d1",
    ) {
        Ok(decided) => Ok(MandateApprovalEntry::ApprovedDecisionRecorded(decided)),
        Err(refused) => Err(refused),
    }
}

/// The approval records the host file's approver approving the capability on the case's derived
/// resource, for the verified subject, and the commission's authority context is that subject's.
#[test]
fn the_recorded_approval_names_the_held_request_on_the_case() {
    let hosted = hosted();
    let approval = recorded(&hosted);
    assert_eq!(approval.subject, Uuid(SUBJECT.to_owned()));
    assert_eq!(approval.organization, Uuid(ORGANIZATION.to_owned()));
    assert_eq!(approval.approver, Uuid(APPROVER.to_owned()));
    assert_eq!(approval.actor, None);
    assert_eq!(approval.action, CAPABILITY);
    assert_eq!(approval.resource_type, CASE_RESOURCE_TYPE);
    assert_eq!(
        Some(approval.resource_id.0.clone()),
        case_resource_id(&CaseId(CASE.to_owned())).map(|id| id.to_string())
    );
    assert_eq!(
        approval.commission_id,
        CommissionId(Uuid(COMMISSION.to_owned()))
    );
    assert_eq!(approval.action_request_id, request());
    let context = verified_context(&commission(&hosted).authority_context)
        .expect("the authority context holds the host's verified context");
    assert_eq!(context.subject.to_string(), SUBJECT);
}

/// A recorded, unexpired approval clears the held request once: Mandate answers the event that
/// consumes it, folding the event leaves the approval consumed, and the same approval is then
/// refused `wrong-state`.
#[test]
fn a_recorded_approval_clears_once_and_its_event_consumes_it() {
    let hosted = hosted();
    let log = vec![MandateApprovalEntry::ApprovalRecorded(recorded(&hosted))];
    let event = match clear(&hosted, &log, BEFORE_EXPIRY) {
        Ok(event) => event,
        Err(refused) => panic!("Mandate clears the approval: {refused}"),
    };
    let MandateApprovalEntry::ApprovedDecisionRecorded(decided) = &event else {
        panic!("an ApprovedDecisionRecorded: {event:?}");
    };
    assert_eq!(decided.approval, approval_id());
    assert_eq!(decided.action_request_id, request());
    let mut text = String::new();
    intake_json::push_value(&mut text, &decided.decision);
    let decision: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(decision["allowed"], json!(true), "{decision}");

    let consumed = vec![log[0].clone(), event];
    let records = fold(&consumed).unwrap_or_else(|why| panic!("{why}"));
    let approval = records
        .approval(&ApprovalId::new(
            mandate_types::Uuid::parse(APPROVAL).unwrap(),
        ))
        .unwrap()
        .expect("the approval is recorded");
    assert_eq!(approval.state, ApprovalState::Consumed);
    let again = clear(&hosted, &consumed, BEFORE_EXPIRY)
        .expect_err("a consumed approval does not clear again");
    assert!(again.0.contains("wrong-state"), "{again}");
}

/// An approval naming another subject, action or resource than the held request is refused, one
/// case each, and so is an expired one; nothing is answered to append.
#[test]
fn an_approval_naming_another_subject_action_or_resource_is_refused() {
    let hosted = hosted();
    let approval = recorded(&hosted);
    let cases: [(&str, MandateApprovalRecorded); 3] = [
        (
            "another subject",
            MandateApprovalRecorded {
                subject: Uuid("33333333-3333-4333-8333-333333333333".to_owned()),
                ..approval.clone()
            },
        ),
        (
            "another action",
            MandateApprovalRecorded {
                action: "tests.run".to_owned(),
                ..approval.clone()
            },
        ),
        (
            "another resource",
            MandateApprovalRecorded {
                resource_id: Uuid(
                    case_resource_id(&CaseId("case-2".to_owned()))
                        .unwrap()
                        .to_string(),
                ),
                ..approval.clone()
            },
        ),
    ];
    for (name, foreign) in cases {
        let log = vec![MandateApprovalEntry::ApprovalRecorded(foreign)];
        let refused = clear(&hosted, &log, BEFORE_EXPIRY).expect_err(name);
        assert!(refused.0.contains("Mandate refused"), "{name}: {refused}");
        assert!(refused.0.contains("(denied)"), "{name}: {refused}");
    }

    let log = vec![MandateApprovalEntry::ApprovalRecorded(approval)];
    let expired =
        clear(&hosted, &log, EXPIRES_AT).expect_err("an approval at its expiry does not clear");
    assert!(expired.0.contains("(denied)"), "{expired}");
}

/// An approval the log recorded for another request of the commission does not clear this one,
/// and neither does an approval the log does not hold.
#[test]
fn an_approval_recorded_for_another_request_or_not_recorded_is_refused() {
    let hosted = hosted();
    let other = MandateApprovalRecorded {
        action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-0000000000a2".to_owned())),
        ..recorded(&hosted)
    };
    let log = vec![MandateApprovalEntry::ApprovalRecorded(other)];
    let refused = clear(&hosted, &log, BEFORE_EXPIRY).expect_err("another request's approval");
    assert!(refused.0.contains("not this one"), "{refused}");
    let unrecorded = clear(&hosted, &[], BEFORE_EXPIRY).expect_err("an unrecorded approval");
    assert!(unrecorded.0.contains("(denied)"), "{unrecorded}");
}

/// A host file is refused for an undeclared member, a verified context that is no Mandate's, an
/// approval-requiring capability it does not name, a negative lifetime, and an approver who is
/// the subject.
#[test]
fn a_host_file_that_cannot_configure_mandate_is_refused() {
    let refuses = |change: &dyn Fn(&mut Value), naming: &str| {
        let mut document = host_document();
        change(&mut document);
        let refused =
            HostedMandate::parse(&document.to_string()).expect_err("the host file is refused");
        assert!(refused.0.contains(naming), "{naming}: {refused}");
    };
    refuses(&|d| d["note"] = json!("x"), "note");
    refuses(&|d| d["verified"] = json!({"subject": SUBJECT}), "verified");
    refuses(&|d| d["approval_required"] = json!(["deploy"]), "deploy");
    refuses(&|d| d["approval_seconds"] = json!(-1), "negative");
    refuses(&|d| d["approver"] = json!(SUBJECT), "second party");
    refuses(&|d| d["capabilities"] = json!([]), "no capability");
    let authority: AuthorityContext = hosted().authority_context();
    assert!(verified_context(&authority).is_some());
}
