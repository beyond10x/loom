//! Adversarial cases for `story:mandate-approval-on-resume`, Mandate's side: who may approve, a
//! log line duplicated or moved, an approval presented for another commission, and which packages
//! reach Mandate through the edge `b10x-loom-cli -> b10x-loom-mandate` this unit opened. Ids are
//! synthetic.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use b10x_loom_commission::model::primitives::Uuid as Id;
use b10x_loom_commission::model::responsibility::{
    AgentRevisionId, CaseId, CommissionData, CommissionId as HeldCommissionId, PrincipalId,
};
use intake_model::primitives::{Timestamp, Uuid};
use intake_model::routing::{
    ActionRequestId, CommissionId, MandateApprovalEntry, MandateApprovalId, MandateApprovalRecorded,
};
use loom_mandate::approval::{HeldRequest, HostedMandate, Refused};
use serde_json::{Value, json};

const CAPABILITY: &str = "repository.merge";
const CASE: &str = "case-1";
const COMMISSION: &str = "00000000-0000-4000-8000-000000000081";
const OTHER_COMMISSION: &str = "00000000-0000-4000-8000-000000000091";
const REQUEST: &str = "00000000-0000-4000-8000-0000000000a1";
const APPROVAL: &str = "00000000-0000-4000-8000-0000000000e1";
const SUBJECT: &str = "22222222-2222-4222-8222-222222222222";
const ACTOR: &str = "55555555-5555-4555-8555-555555555555";
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
            "correlation": "correlation-adversary"
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

fn commission_named(hosted: &HostedMandate, id: &str) -> CommissionData {
    CommissionData {
        commission_id: HeldCommissionId(Id(id.to_owned())),
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

fn clear_for(
    hosted: &HostedMandate,
    commission: &str,
    log: &[MandateApprovalEntry],
    decision: &str,
) -> Result<MandateApprovalEntry, Refused> {
    hosted
        .clear(
            &commission_named(hosted, commission),
            CAPABILITY,
            &request(),
            &approval_id(),
            log,
            &Timestamp(BEFORE_EXPIRY.to_owned()),
            decision,
        )
        .map(MandateApprovalEntry::ApprovedDecisionRecorded)
}

/// "An approval is a second party's": the host file refuses an approver who is the verified
/// subject. The verified context may also name an actor, the principal the subject acts through
/// (the agent). An approver who is that actor approves the actor's own request, so the host file
/// must refuse it as it refuses the subject.
#[test]
fn adversary_a_host_file_whose_approver_is_the_acting_actor_is_refused() {
    let mut document = host_document();
    document["verified"]["actor"] = json!(ACTOR);
    // The same document with another approver is accepted, so the refusal below is the
    // approver's and not the actor's.
    assert!(
        HostedMandate::parse(&document.to_string()).is_ok(),
        "a host file whose verified context names an actor is a valid host file"
    );
    document["approver"] = json!(ACTOR);
    let parsed = HostedMandate::parse(&document.to_string());
    assert!(
        parsed.is_err(),
        "a host file whose approver is the verified actor ({ACTOR}) is accepted: the acting \
         principal approves its own request"
    );
}

/// The same host file end to end: the approval its approver (the verified actor) records clears
/// the actor's own request through Mandate's `check_with_approval`. It must not.
#[test]
fn adversary_an_approval_by_the_acting_actor_does_not_clear_its_own_request() {
    let mut document = host_document();
    document["verified"]["actor"] = json!(ACTOR);
    document["approver"] = json!(ACTOR);
    let Ok(hosted) = HostedMandate::parse(&document.to_string()) else {
        // Refused at the host file, which is the outcome asked for.
        return;
    };
    let approval = recorded(&hosted);
    assert_eq!(approval.approver, approval.actor.clone().unwrap());
    let log = vec![MandateApprovalEntry::ApprovalRecorded(approval)];
    let cleared = clear_for(
        &hosted,
        COMMISSION,
        &log,
        "00000000-0000-4000-8000-0000000000d4",
    );
    assert!(
        cleared.is_err(),
        "Mandate cleared the actor's own request on the actor's own approval: {cleared:?}"
    );
}

/// A duplicated `ApprovalRecorded` line, written after the consumption, does not put a consumed
/// approval back in force; neither does the consumption moved before the approval it consumes.
#[test]
fn adversary_a_duplicated_or_reordered_log_line_does_not_replay_a_consumed_approval() {
    let hosted = hosted();
    let approval = MandateApprovalEntry::ApprovalRecorded(recorded(&hosted));
    let consumption = clear_for(
        &hosted,
        COMMISSION,
        std::slice::from_ref(&approval),
        "00000000-0000-4000-8000-0000000000d1",
    )
    .unwrap_or_else(|why| panic!("the first presentation clears: {why}"));
    let logs: [(&str, Vec<MandateApprovalEntry>); 3] = [
        (
            "the approval line duplicated after its consumption",
            vec![approval.clone(), consumption.clone(), approval.clone()],
        ),
        (
            "the consumption line moved before its approval",
            vec![consumption.clone(), approval.clone()],
        ),
        (
            "the approval line twice before its consumption",
            vec![approval.clone(), approval.clone(), consumption.clone()],
        ),
    ];
    for (name, log) in logs {
        let again = clear_for(
            &hosted,
            COMMISSION,
            &log,
            "00000000-0000-4000-8000-0000000000d2",
        );
        assert!(
            again.is_err(),
            "{name}: a consumed approval cleared again: {again:?}"
        );
    }
}

/// An approval recorded for one commission's request does not clear another commission's request
/// on the same case, for the same subject and capability: the grant of one commission cannot
/// carry another's approval.
#[test]
fn adversary_an_approval_recorded_for_another_commission_on_the_same_case_is_refused() {
    let hosted = hosted();
    let log = vec![MandateApprovalEntry::ApprovalRecorded(recorded(&hosted))];
    let refused = clear_for(
        &hosted,
        OTHER_COMMISSION,
        &log,
        "00000000-0000-4000-8000-0000000000d3",
    );
    let refused = refused.expect_err("another commission's approval cleared this one");
    assert!(refused.0.contains("not this one"), "{refused}");
}

/// The packages of the workspace's `Cargo.lock`, each with its direct dependencies' names.
fn locked_packages() -> BTreeMap<String, BTreeSet<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut packages = BTreeMap::new();
    for block in text.split("[[package]]").skip(1) {
        let name = block
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("a package has a name: {block}"))
            .to_owned();
        let mut dependencies = BTreeSet::new();
        if let Some((_, list)) = block.split_once("dependencies = [") {
            let list = list.split(']').next().unwrap_or_default();
            for entry in list.split(',') {
                let entry = entry.trim().trim_matches('"');
                if let Some(dependency) = entry.split_whitespace().next() {
                    dependencies.insert(dependency.to_owned());
                }
            }
        }
        packages
            .entry(name)
            .or_insert_with(BTreeSet::new)
            .extend(dependencies);
    }
    packages
}

/// `security_w10_u2` now lets `b10x-loom-cli` depend on `b10x-loom-mandate`, and checks direct
/// edges only: a package depending on `b10x-loom-cli` reaches Mandate without a direct edge and
/// passes it. Transitively, the packages that reach a Mandate crate are the provider, the command
/// line and the tools that link the command line to render its help (`loom-docs`), and no
/// library of the runtime: not the SDK, Commission, the governor, the slice or a plugin.
#[test]
fn adversary_no_runtime_library_reaches_mandate_through_the_command_line() {
    let packages = locked_packages();
    let is_mandate = |name: &str| name.starts_with("mandate-") || name == "b10x-loom-mandate";
    let mut reaches: BTreeSet<String> = BTreeSet::new();
    loop {
        let before = reaches.len();
        for (package, dependencies) in &packages {
            if dependencies
                .iter()
                .any(|dependency| is_mandate(dependency) || reaches.contains(dependency))
            {
                reaches.insert(package.clone());
            }
        }
        if reaches.len() == before {
            break;
        }
    }
    let unexpected: Vec<&String> = reaches
        .iter()
        .filter(|package| {
            !is_mandate(package)
                && !matches!(package.as_str(), "b10x-loom-cli" | "loom-docs")
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "these packages reach Mandate transitively: {unexpected:?} (all: {reaches:?})"
    );
}
