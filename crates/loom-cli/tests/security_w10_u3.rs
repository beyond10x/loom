//! Security review of `story:mandate-approval-on-resume`: integrity conformance checks of the
//! approval's single consumption across resumers and crashes, of the approval's binding to its
//! commission and request, and of where Mandate is linked.
//!
//! Each case drives the code the unit ships: the command line's `Mandate` over the state
//! directory's `MandateApprovalLog`, behind `GrantAuthority::with_mandate`, against a commission
//! Suspension held in the governor's `FileSuspensionStore`, exactly as `resume --mandate` wires
//! them. Every "resumer" opens its own stores, as a separate process does. Ids are synthetic.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use b10x_loom_cli::mandate::Mandate;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::Uuid as HeldUuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId as HeldRequestId, AgentRevisionId, AuthorityVerdict, CaseId, CommissionData,
    CommissionId as HeldCommissionId, HoldSuspension, HoldSuspensionOutcome, PrincipalId,
    ProposedActionArguments, RunId, SuspensionKey, SuspensionReason,
};
use b10x_loom_commission::ports::authority::AuthorityProvider;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_intake_slice::grant::{
    ActionRequestId, CommissionId, GrantAuthority, GrantData, HeldApproval, MandateApprovalId,
    MandateApprovals,
};
use b10x_loom_intake_slice::mandate_log::{FILE as MANDATE_LOG, MandateApprovalLog};
use intake_model::primitives::{Timestamp, Uuid};
use loom_governor::FileSuspensionStore;
use loom_mandate::approval::HostedMandate;
use serde_json::{Value, json as serde_json_value};

const CAPABILITY: &str = "repository.merge";
const CASE: &str = "case-1";
const COMMISSION_A: &str = "00000000-0000-4000-8000-0000000000a0";
const REQUEST_A: &str = "00000000-0000-4000-8000-0000000000a1";
const COMMISSION_B: &str = "00000000-0000-4000-8000-0000000000b0";
const REQUEST_B: &str = "00000000-0000-4000-8000-0000000000b1";
const SUBJECT: &str = "22222222-2222-4222-8222-222222222222";
const APPROVER: &str = "99999999-9999-4999-8999-999999999999";
const APPROVED_AT: &str = "2026-10-14T09:00:00Z";
const RESUMED_AT: &str = "2026-10-14T09:10:00Z";

fn host_document(seconds: i64) -> Value {
    serde_json_value!({
        "verified": {
            "subject": SUBJECT,
            "organization": "11111111-1111-4111-8111-111111111111",
            "audience": "loom",
            "credential": "77777777-7777-4777-8777-777777777777",
            "correlation": "correlation-security-w10-u3"
        },
        "approver": APPROVER,
        "approver_policy": "40404040-4040-4040-8040-404040404040",
        "relation": "maintainer",
        "capabilities": [CAPABILITY, "tests.run"],
        "approval_required": [CAPABILITY],
        "approval_seconds": seconds
    })
}

fn hosted(seconds: i64) -> HostedMandate {
    HostedMandate::parse(&host_document(seconds).to_string()).unwrap_or_else(|why| panic!("{why}"))
}

/// One state directory holding commission Suspensions, Mandate approvals and the grants naming
/// them, as `run --state` and `approve --mandate` leave it.
struct State {
    _dir: tempfile::TempDir,
    path: PathBuf,
    seconds: i64,
}

impl State {
    fn new(prefix: &str, seconds: i64) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("security-w10-u3-{prefix}-"))
            .tempdir()
            .unwrap();
        let path = dir.path().to_path_buf();
        Self {
            _dir: dir,
            path,
            seconds,
        }
    }

    /// Holds `commission`'s Suspension for `request`, a `repository.merge` on the case.
    fn hold(&self, commission: &str, request: &str) {
        let suspensions = FileSuspensionStore::open(&self.path).unwrap();
        let held = suspensions.hold(HoldSuspension {
            suspension: SuspensionKey {
                commission_id: HeldCommissionId(HeldUuid(commission.to_owned())),
            },
            commission_id: HeldCommissionId(HeldUuid(commission.to_owned())),
            run_id: RunId(HeldUuid("00000000-0000-4000-8000-0000000000c3".to_owned())),
            run_case_revision: 1,
            action_request_id: HeldRequestId(HeldUuid(request.to_owned())),
            case_id: CaseId(CASE.to_owned()),
            expected_case_revision: 3,
            action: CAPABILITY.to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
            reason: SuspensionReason::Authority(json::Value::Object(vec![(
                "capability".to_owned(),
                json::Value::Text(CAPABILITY.to_owned()),
            )])),
        });
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "{held:?}"
        );
    }

    /// What `approve --mandate` records for `commission`'s held `request`: the approval's id.
    fn approve(&self, commission: &str, request: &str) -> MandateApprovalId {
        let log = MandateApprovalLog::open(&self.path).unwrap();
        let mandate = Mandate::new(hosted(self.seconds), &log, || {
            Timestamp(APPROVED_AT.to_owned())
        });
        mandate
            .record(&HeldApproval {
                commission_id: &CommissionId(Uuid(commission.to_owned())),
                action_request_id: &ActionRequestId(Uuid(request.to_owned())),
                case_id: &CaseId(CASE.to_owned()),
                capability: CAPABILITY,
                at: &Timestamp(APPROVED_AT.to_owned()),
            })
            .unwrap_or_else(|why| panic!("approve --mandate records an approval: {why}"))
    }

    /// One `resume --mandate` deciding `grant` for `commission` at `at`, from its own process's
    /// stores, as `resume_prepared` builds the provider and the commission.
    fn decide_at(&self, commission: &str, grant: &GrantData, at: &str) -> Result<(), String> {
        let suspensions = FileSuspensionStore::open(&self.path).unwrap();
        let log = MandateApprovalLog::open(&self.path).unwrap();
        let at = at.to_owned();
        let mandate = Mandate::new(hosted(self.seconds), &log, move || Timestamp(at.clone()));
        let authority =
            GrantAuthority::new(Some(grant.clone()), &suspensions).with_mandate(&mandate);
        let data = CommissionData {
            commission_id: HeldCommissionId(HeldUuid(commission.to_owned())),
            agent_revision_id: AgentRevisionId(HeldUuid(
                "00000000-0000-4000-8000-0000000000c2".to_owned(),
            )),
            case_id: CaseId(CASE.to_owned()),
            principal: PrincipalId("operator".to_owned()),
            authority_context: mandate.authority_context(),
        };
        match authority.decide(&data, CAPABILITY) {
            Ok(AuthorityVerdict::Allow(_)) => Ok(()),
            other => Err(format!("{other:?}")),
        }
    }

    fn decide(&self, commission: &str, grant: &GrantData) -> Result<(), String> {
        self.decide_at(commission, grant, RESUMED_AT)
    }

    fn log_path(&self) -> PathBuf {
        self.path.join(MANDATE_LOG)
    }

    fn log_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.log_path())
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn kinds(&self) -> Vec<String> {
        self.log_lines()
            .iter()
            .map(|line| {
                serde_json::from_str::<Value>(line).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }
}

/// The grant `approve --mandate` writes for `commission`'s held `request`, naming `approval`.
fn grant(commission: &str, request: &str, approval: &MandateApprovalId) -> GrantData {
    GrantData {
        commission_id: CommissionId(Uuid(commission.to_owned())),
        action: CAPABILITY.to_owned(),
        capability: CAPABILITY.to_owned(),
        granted: true,
        action_request_id: ActionRequestId(Uuid(request.to_owned())),
        granted_at: Timestamp(APPROVED_AT.to_owned()),
        mandate_approval: Some(approval.clone()),
    }
}

/// Of eight resumers presenting one approval at once, each in its own stores as a separate process
/// is, exactly one is allowed, and the log keeps exactly one consumption.
#[test]
fn security_one_approval_is_allowed_once_across_concurrent_resumers() {
    let state = Arc::new(State::new("concurrent", 3600));
    state.hold(COMMISSION_A, REQUEST_A);
    let approval = state.approve(COMMISSION_A, REQUEST_A);
    let granted = grant(COMMISSION_A, REQUEST_A, &approval);

    let resumers = 8;
    let barrier = Arc::new(Barrier::new(resumers));
    let handles: Vec<_> = (0..resumers)
        .map(|_| {
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            let granted = granted.clone();
            std::thread::spawn(move || {
                barrier.wait();
                state.decide(COMMISSION_A, &granted)
            })
        })
        .collect();
    let answers: Vec<Result<(), String>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let allowed = answers.iter().filter(|answer| answer.is_ok()).count();
    assert_eq!(
        allowed, 1,
        "one approval allowed {allowed} of {resumers} concurrent resumes: {answers:?}"
    );
    assert_eq!(
        state.kinds(),
        vec!["ApprovalRecorded", "ApprovedDecisionRecorded"],
        "exactly one consumption is kept"
    );
}

/// The consumption is on disk by the time the provider answers allow, so a crash at any point
/// after the allow (before the take, before the effect) leaves the approval consumed: a fresh
/// process presenting it again is refused `wrong-state`, while the Suspension is still Pending.
#[test]
fn security_the_consumption_is_durable_before_the_allow_and_a_crash_never_replays_it() {
    let state = State::new("crash", 3600);
    state.hold(COMMISSION_A, REQUEST_A);
    let approval = state.approve(COMMISSION_A, REQUEST_A);
    let granted = grant(COMMISSION_A, REQUEST_A, &approval);

    state
        .decide(COMMISSION_A, &granted)
        .unwrap_or_else(|why| panic!("the first resume is allowed: {why}"));
    // The process "crashes" here: nothing is taken, nothing performed.
    assert_eq!(
        state.kinds(),
        vec!["ApprovalRecorded", "ApprovedDecisionRecorded"],
        "the consumption was kept before the allow was answered"
    );
    let again = state
        .decide(COMMISSION_A, &granted)
        .expect_err("a resume after the crash presents a consumed approval and is refused");
    assert!(again.contains("wrong-state"), "{again}");
}

/// A log whose lines were duplicated or reordered (a copy, a restore from two sources, a hand
/// edit) does not put a consumed approval back in force: the recorded approval appended again
/// after its consumption, the consumption written before the approval, the approval appended again
/// with a later expiry, each is refused.
#[test]
fn security_a_duplicated_or_reordered_log_line_does_not_replay_a_consumed_approval() {
    let state = State::new("replay", 3600);
    state.hold(COMMISSION_A, REQUEST_A);
    let approval = state.approve(COMMISSION_A, REQUEST_A);
    let granted = grant(COMMISSION_A, REQUEST_A, &approval);
    state.decide(COMMISSION_A, &granted).unwrap();
    let lines = state.log_lines();
    assert_eq!(lines.len(), 2, "{lines:?}");
    let (recorded, consumed) = (lines[0].clone(), lines[1].clone());
    let mut extended: Value = serde_json::from_str(&recorded).unwrap();
    extended["expires_at"] = serde_json_value!("2026-10-15T09:00:00Z");
    let extended = extended.to_string();

    let variants: [(&str, Vec<&str>); 4] = [
        (
            "the approval duplicated after its consumption",
            vec![&recorded, &consumed, &recorded],
        ),
        (
            "the consumption before the approval",
            vec![&consumed, &recorded],
        ),
        (
            "the approval appended again with a later expiry",
            vec![&recorded, &consumed, &extended],
        ),
        (
            "the approval duplicated before its consumption",
            vec![&recorded, &recorded, &consumed],
        ),
    ];
    for (name, log) in variants {
        let mut text = log.join("\n");
        text.push('\n');
        std::fs::write(state.log_path(), text).unwrap();
        let answer = state.decide(COMMISSION_A, &granted);
        assert!(
            answer.is_err(),
            "{name}: a consumed approval was allowed again"
        );
    }
}

/// A torn or unreadable log refuses rather than forgets: a consumption line cut short by a torn
/// write, a trailing fragment, or an empty line makes the whole log unreadable, and nothing is
/// allowed over the part of it that still reads.
#[test]
fn security_a_torn_or_unreadable_log_allows_nothing() {
    let state = State::new("torn", 3600);
    state.hold(COMMISSION_A, REQUEST_A);
    let approval = state.approve(COMMISSION_A, REQUEST_A);
    let granted = grant(COMMISSION_A, REQUEST_A, &approval);
    state.decide(COMMISSION_A, &granted).unwrap();
    let lines = state.log_lines();
    let (recorded, consumed) = (lines[0].clone(), lines[1].clone());

    let torn = &consumed[..consumed.len() / 2];
    let variants: [(&str, String); 3] = [
        ("the consumption torn", format!("{recorded}\n{torn}")),
        (
            "a trailing fragment",
            format!("{recorded}\n{consumed}\n{{\"kind\":"),
        ),
        ("an empty line", format!("{recorded}\n\n{consumed}\n")),
    ];
    for (name, text) in variants {
        std::fs::write(state.log_path(), &text).unwrap();
        let answer = state.decide(COMMISSION_A, &granted);
        assert!(answer.is_err(), "{name}: allowed over a torn log");
        assert_eq!(
            std::fs::read_to_string(state.log_path()).unwrap(),
            text,
            "{name}: the log was rewritten"
        );
    }
}

/// An approval expires `approval_seconds` after it was recorded, through the command line's
/// clock: one second before the expiry it is allowed, at the expiry it is refused and nothing is
/// consumed.
#[test]
fn security_an_approval_is_allowed_only_before_its_expiry() {
    let state = State::new("expiry", 60);
    state.hold(COMMISSION_A, REQUEST_A);
    let approval = state.approve(COMMISSION_A, REQUEST_A);
    let granted = grant(COMMISSION_A, REQUEST_A, &approval);
    let at_expiry = state.decide_at(COMMISSION_A, &granted, "2026-10-14T09:01:00Z");
    assert!(at_expiry.is_err(), "allowed at its expiry");
    assert_eq!(state.kinds(), vec!["ApprovalRecorded"], "nothing consumed");
    state
        .decide_at(COMMISSION_A, &granted, "2026-10-14T09:00:59Z")
        .unwrap_or_else(|why| panic!("allowed one second before its expiry: {why}"));
}

/// An approval is bound to the commission and the request it was recorded for, however its id is
/// written. Commission A's approval, named by commission B's grant (same case, same subject, same
/// capability: everything Mandate itself compares), is refused, and it must be refused when the
/// grant writes the same UUID in upper case too: Mandate reads the id as a UUID, so Loom's check
/// of the commission and the request must find the record Mandate will use.
#[test]
fn security_an_approval_is_bound_to_its_commission_however_its_id_is_written() {
    let state = State::new("binding", 3600);
    state.hold(COMMISSION_A, REQUEST_A);
    state.hold(COMMISSION_B, REQUEST_B);
    let approval = state.approve(COMMISSION_A, REQUEST_A);

    let foreign = grant(COMMISSION_B, REQUEST_B, &approval);
    let refused = state
        .decide(COMMISSION_B, &foreign)
        .expect_err("commission A's approval does not allow commission B's request");
    assert!(refused.contains("not this one"), "{refused}");

    let upper = MandateApprovalId(Uuid(approval.0.0.to_uppercase()));
    assert_ne!(upper, approval, "the id has letters to change case");
    let foreign = grant(COMMISSION_B, REQUEST_B, &upper);
    let answer = state.decide(COMMISSION_B, &foreign);
    assert!(
        answer.is_err(),
        "commission A's approval {} (written {}) allowed commission B's request {REQUEST_B}; \
         the log now holds {:?}",
        approval.0.0,
        upper.0.0,
        state.kinds()
    );
}

/// The packages of the workspace's `Cargo.lock`, each with its direct dependencies' names.
fn locked_packages(lock: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(lock)
        .unwrap_or_else(|error| panic!("{}: {error}", lock.display()));
    let mut packages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for block in text.split("[[package]]").skip(1) {
        let name = block
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("a package has a name: {block}"))
            .to_owned();
        let mut dependencies = BTreeSet::new();
        if let Some((_, list)) = block.split_once("dependencies = [") {
            for entry in list.split(']').next().unwrap_or_default().split(',') {
                if let Some(dependency) = entry.trim().trim_matches('"').split_whitespace().next() {
                    dependencies.insert(dependency.to_owned());
                }
            }
        }
        packages.entry(name).or_default().extend(dependencies);
    }
    packages
}

/// Mandate reaches, directly or through another package, only the command line and the
/// documentation generator that links the command line for its help text. `security_w10_u2`
/// checks direct edges only, so a package depending on `b10x-loom-cli` links Mandate unseen by
/// it; this checks the closure, and names the packages that must never reach it.
#[test]
fn security_mandate_reaches_no_package_beyond_the_command_line() {
    let lock = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let packages = locked_packages(&lock);
    let is_mandate = |name: &str| {
        name == "mandate" || name.starts_with("mandate-") || name == "b10x-loom-mandate"
    };
    let mut reaching: BTreeSet<String> = BTreeSet::new();
    loop {
        let before = reaching.len();
        for (package, dependencies) in &packages {
            if is_mandate(package) {
                continue;
            }
            if dependencies
                .iter()
                .any(|dependency| is_mandate(dependency) || reaching.contains(dependency))
            {
                reaching.insert(package.clone());
            }
        }
        if reaching.len() == before {
            break;
        }
    }
    let allowed: BTreeSet<String> = ["b10x-loom-cli", "loom-docs"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let beyond: Vec<&String> = reaching.difference(&allowed).collect();
    assert!(beyond.is_empty(), "Mandate reaches {beyond:?}");
    for never in [
        "b10x-loom-sdk",
        "b10x-loom-commission",
        "b10x-loom-governor",
        "b10x-loom-intake-slice",
        "b10x-loom-plugin",
    ] {
        assert!(!reaching.contains(never), "Mandate reaches {never}");
    }
}
