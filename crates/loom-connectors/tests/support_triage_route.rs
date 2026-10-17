//! Acceptance for `story:connectors-governed-write` on `support-triage@1`: the governor admits
//! `ticket.route` only after `classification.review` approved the classification and the
//! requester is known; the host performs the admitted route through
//! `ConnectorsCli::invoke_write`, and the host evidence adapter
//! (`b10x_loom_connectors::evidence::WriteEvidenceAdapter`) turns an applied write into a
//! `routing_record` naming the step, through `submit_evidence`. A write Connectors classifies
//! `refused`, `not_attempted` or `unknown` records none.
//!
//! The case runs on the Canon-backed governor (`loom_governor::CanonGovernor`) over the bundled
//! `support-triage@1`, unchanged. The evidence before the route is the
//! `fixtures/support-triage/tri-48213.fixture.yaml` records of engineering-protocols, submitted
//! through `submit_evidence`. The program is the fake `connectors` of `connectors_cli.rs`, its
//! logs under `CARGO_TARGET_TMPDIR/support_triage_route`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, ExecutorOutcome, ExecutorOutcomeProposedAction,
    FrontierData, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Truth, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, MutationClassification, OperationId,
    WriteApproval, WriteOperation, WriteResult,
};
use b10x_loom_connectors::cli::{ConnectorsCli, json};
use b10x_loom_connectors::evidence::WriteEvidenceAdapter;
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};

const PROTOCOL: &str = "support-triage@1";
const ROUTE: &str = "ticket.route";
const ROUTING_RECORD: &str = "routing_record";
const PRODUCER: &str = "service:helpdesk";
const ADAPTER: &str = "desk";
const CONNECTION: &str = "conn-desk";
const NOW: &str = "2026-10-05T09:26:00Z";

type Store = CanonGovernor<MemoryCaseStore>;

// ---------------------------------------------------------------------------------------------
// The fake `connectors`.

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str, invoke: &str, exit: i32) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("support_triage_route")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home", "approvals"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let status = std::process::Command::new("/usr/bin/install")
            .args(["-m", "755"])
            .arg(fixtures().join("fake-connectors"))
            .arg(root.join("connectors"))
            .status()
            .unwrap();
        assert!(status.success());
        let fixture = Self { root };
        for (verb, file) in [("describe", "describe-route.json"), ("invoke", invoke)] {
            fs::copy(
                fixtures().join(file),
                fixture.state().join(format!("{verb}.{ROUTE}.json")),
            )
            .unwrap();
        }
        fs::write(fixture.state().join("invoke.exit"), exit.to_string()).unwrap();
        fixture
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cli(&self) -> ConnectorsCli {
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: None,
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds: None,
            },
            Vec::new(),
        )
        .with_environment([
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ])
    }

    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    fn invokes(&self) -> usize {
        fs::read_to_string(self.state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.contains("\tinvoke\t"))
            .count()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

// ---------------------------------------------------------------------------------------------
// The host: its effect port routes through `ConnectorsCli::invoke_write`.

/// The host's effect port for `ticket.route`: one `invoke_write` per admitted request, with the
/// proof the person's confirmation wrote. It keeps the request and what Connectors answered for
/// the evidence adapter.
struct Desk<'f> {
    cli: ConnectorsCli,
    fixture: &'f Fixture,
    written: Mutex<Vec<(AdmittedRequest, WriteResult)>>,
}

impl EffectPort for Desk<'_> {
    fn performs(&self, action: &str) -> bool {
        action == ROUTE
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let mut arguments = String::new();
        commission_json::push_value(&mut arguments, &request.data().arguments.0);
        let input =
            json::parse(&arguments).map_err(|error| EffectError::new(format!("{error:?}")))?;
        let written = self
            .cli
            .invoke_write(
                request,
                &WriteOperation {
                    adapter: AdapterAlias(ADAPTER.to_owned()),
                    connection: ConnectionId(CONNECTION.to_owned()),
                    operation: OperationId(ROUTE.to_owned()),
                },
                &input,
                &WriteApproval {
                    proof_file: self.fixture.proof(),
                },
            )
            .map_err(|error| EffectError::new(format!("{error:?}")))?;
        let classification = written.classification;
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((request.clone(), written));
        match classification {
            MutationClassification::Applied => {
                Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                    report: commission_json::Value::Null,
                    attempt: None,
                    audit: None,
                    usage: None,
                }))
            }
            MutationClassification::Refused | MutationClassification::NotAttempted => {
                Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                    reason: "connectors changed nothing".to_owned(),
                }))
            }
            MutationClassification::Unknown => {
                Err(EffectError::new("the route's outcome is unknown"))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The case.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

#[derive(Default)]
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> Uuid {
        self.0 += 1;
        uuid(0x1000 + self.0)
    }
}

#[derive(Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn open() -> (Store, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(
            PROTOCOL,
            BTreeMap::from([
                ("ticket".to_owned(), "c3".to_owned()),
                ("requester".to_owned(), "u1".to_owned()),
            ]),
            CaseInputs::none(),
        )
        .expect("a support-triage@1 case opens");
    (governor, case)
}

/// Submits one `canon-evidence/1` record through `submit_evidence`, citing an observation the
/// governor received at `at`.
fn submit(
    governor: &Store,
    case: &CaseId,
    ids: &mut Ids,
    at: &str,
    (kind, result, subject, revision): (&str, Option<&str>, &str, &str),
) {
    let observation = ObservationId(ids.next());
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "host/fixture".to_owned(),
            subject: subject.to_owned(),
            observed_at: Timestamp(at.to_owned()),
            payload: commission_json::Value::Null,
        }))
        .expect("the governor keeps the observation");
    let id = ids.next();
    let mut facts = vec![
        ("format", "canon-evidence/1".to_owned()),
        ("id", format!("{kind}-{}", id.0)),
        ("kind", kind.to_owned()),
        ("subject", subject.to_owned()),
        ("subject_revision", revision.to_owned()),
        ("observed_at", at.to_owned()),
    ];
    if let Some(result) = result {
        facts.push(("result", result.to_owned()));
    }
    let evidence = EvidenceData {
        evidence_id: EvidenceId(id),
        case_id: case.clone(),
        kind: kind.to_owned(),
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![observation],
        facts: commission_json::Value::Object(
            facts
                .into_iter()
                .map(|(name, value)| (name.to_owned(), commission_json::Value::Text(value)))
                .collect(),
        ),
        provenance: commission_json::Value::Null,
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

fn frontier(governor: &Store, case: &CaseId) -> FrontierData {
    governor.frontier(case).expect("frontier").into_data()
}

fn status(frontier: &FrontierData, action: &str) -> ActionStatus {
    frontier
        .actions
        .iter()
        .find(|entry| entry.action == action)
        .unwrap_or_else(|| panic!("the frontier lists {action}"))
        .status
}

fn claim(frontier: &FrontierData, name: &str) -> Truth {
    frontier
        .claims
        .iter()
        .find(|entry| entry.claim == name)
        .unwrap_or_else(|| panic!("the frontier lists {name}"))
        .value
}

/// The ticket read and a classification proposed (`tri-48213`, `ticket-read` and
/// `classification-proposed`).
fn classified(governor: &Store, case: &CaseId, ids: &mut Ids) {
    submit(
        governor,
        case,
        ids,
        "2026-10-05T09:20:00Z",
        ("ticket_snapshot", Some("within_target"), "ticket", "c3"),
    );
    submit(
        governor,
        case,
        ids,
        "2026-10-05T09:22:00Z",
        ("classification", None, "ticket", "c3"),
    );
}

fn approved(governor: &Store, case: &CaseId, ids: &mut Ids) {
    submit(
        governor,
        case,
        ids,
        "2026-10-05T09:24:00Z",
        ("classification_review", Some("approved"), "ticket", "c3"),
    );
}

fn identified(governor: &Store, case: &CaseId, ids: &mut Ids) {
    submit(
        governor,
        case,
        ids,
        "2026-10-05T09:21:00Z",
        ("requester_profile", Some("identified"), "requester", "u1"),
    );
}

/// One run of the commission in which the executor proposes `ticket.route` and the authority
/// provider grants its capability; the host's effect port performs what the runtime admits.
fn run_route(governor: &Store, case: &CaseId, desk: &Desk<'_>) -> Vec<String> {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-desk".to_owned()),
        authority_context: AuthorityContext(commission_json::Value::Null),
    });
    let authority =
        StaticAuthorityProvider::new().answer(ROUTE, AuthorityVerdict::Allow(Unit(true)));
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(
                commission_json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap(),
            ),
        }),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    match run_until_blocked(
        governor,
        &executor,
        &authority,
        desk,
        &commission,
        &mut runs,
        &mut Context::default(),
    ) {
        Ok(end) => end
            .admitted
            .iter()
            .map(|request| request.action.clone())
            .collect(),
        // A route whose outcome is unknown fails the effect port: the run stops there.
        Err(_) => desk
            .written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(request, _)| request.data().action.clone())
            .collect(),
    }
}

/// Records what the host's effect port kept through the evidence adapter; answers the evidence
/// ids it recorded.
fn record(governor: &Store, desk: &Desk<'_>, ids: &mut Ids) -> Vec<EvidenceId> {
    let ticket = governor
        .revisions(&desk_case(desk))
        .expect("revisions")
        .get("ticket")
        .cloned()
        .expect("the ticket's revision");
    let adapter = WriteEvidenceAdapter::support_triage_route();
    desk.written
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter_map(|(request, written)| {
            adapter
                .record(
                    governor,
                    PRODUCER,
                    request,
                    written,
                    ObservationId(ids.next()),
                    Timestamp(NOW.to_owned()),
                    EvidenceId(ids.next()),
                    &ticket,
                )
                .expect("the adapter records the route")
        })
        .collect()
}

fn desk_case(desk: &Desk<'_>) -> CaseId {
    desk.written
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .first()
        .map(|(request, _)| request.data().case_id.clone())
        .expect("a route was admitted")
}

fn routing_records(governor: &Store, case: &CaseId) -> Vec<EvidenceData> {
    governor
        .evidence(case)
        .expect("evidence")
        .into_iter()
        .filter(|evidence| evidence.kind == ROUTING_RECORD)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

/// `ticket.route` is blocked until the review approved the classification and the requester is
/// known; then admitted once, performed through `invoke_write`, and the applied write recorded as
/// a `routing_record` naming `ticket.route` and its admitted request, after which the ticket is
/// routed and triaged.
#[test]
fn an_applied_route_records_a_routing_record_naming_the_step() {
    let fixture = Fixture::new("applied", "invoke-route-applied.json", 0);
    let (governor, case) = open();
    let mut ids = Ids::default();
    let desk = Desk {
        cli: fixture.cli(),
        fixture: &fixture,
        written: Mutex::new(Vec::new()),
    };

    classified(&governor, &case, &mut ids);
    approved(&governor, &case, &mut ids);
    assert_eq!(
        status(&frontier(&governor, &case), ROUTE),
        ActionStatus::Blocked,
        "approved but the requester not known: {ROUTE} is blocked"
    );
    assert!(
        run_route(&governor, &case, &desk).is_empty(),
        "nothing is admitted while the requester is not known"
    );
    assert_eq!(fixture.invokes(), 0, "nothing reaches connectors");

    identified(&governor, &case, &mut ids);
    assert_eq!(
        status(&frontier(&governor, &case), ROUTE),
        ActionStatus::ApprovalRequired,
        "approved and the requester known: {ROUTE} needs only its authority"
    );
    assert_eq!(run_route(&governor, &case, &desk), [ROUTE]);
    assert_eq!(fixture.invokes(), 1, "the route is invoked once");

    let recorded = record(&governor, &desk, &mut ids);
    assert_eq!(recorded.len(), 1, "one routing_record");
    let records = routing_records(&governor, &case);
    assert_eq!(records.len(), 1, "{records:?}");
    let evidence = &records[0];
    assert_eq!(evidence.evidence_id, recorded[0]);
    assert_eq!(evidence.producer, PRODUCER);
    let written = desk.written.lock().unwrap_or_else(PoisonError::into_inner);
    let request = &written[0].0;
    let provenance = &evidence.provenance;
    assert_eq!(
        provenance.member("action"),
        Some(&commission_json::Value::Text(ROUTE.to_owned())),
        "{provenance:?}"
    );
    assert_eq!(
        provenance.member("action_request_id"),
        Some(&commission_json::Value::Text(
            request.data().action_request_id.0.0.clone()
        )),
        "{provenance:?}"
    );
    assert_eq!(
        provenance.member("request_id"),
        Some(&commission_json::Value::Text(
            "00000000-0000-4000-8000-0000000000a1".to_owned()
        )),
        "{provenance:?}"
    );
    assert_eq!(
        evidence.facts.member("subject"),
        Some(&commission_json::Value::Text("ticket".to_owned()))
    );
    let after = frontier(&governor, &case);
    assert_eq!(claim(&after, "ticket.routed"), Truth::True);
    assert_eq!(claim(&after, "ticket.triaged"), Truth::True);
}

/// The review not yet approved, `ticket.route` is blocked and nothing is admitted, though the
/// requester is known.
#[test]
fn the_route_is_not_admitted_before_the_review_approved() {
    let fixture = Fixture::new("unreviewed", "invoke-route-applied.json", 0);
    let (governor, case) = open();
    let mut ids = Ids::default();
    let desk = Desk {
        cli: fixture.cli(),
        fixture: &fixture,
        written: Mutex::new(Vec::new()),
    };

    classified(&governor, &case, &mut ids);
    identified(&governor, &case, &mut ids);
    assert_eq!(
        status(&frontier(&governor, &case), ROUTE),
        ActionStatus::Blocked
    );
    assert!(run_route(&governor, &case, &desk).is_empty());
    assert_eq!(fixture.invokes(), 0);
    assert!(routing_records(&governor, &case).is_empty());
}

/// A route Connectors classifies `refused`, `not_attempted` or `unknown` records no evidence, and
/// the ticket stays unrouted.
#[test]
fn a_route_not_applied_records_no_evidence() {
    for (test, file) in [
        ("refused", "invoke-route-refused.json"),
        ("not_attempted", "invoke-route-not-attempted.json"),
        ("unknown", "invoke-route-unknown.json"),
    ] {
        let fixture = Fixture::new(test, file, 1);
        let (governor, case) = open();
        let mut ids = Ids::default();
        let desk = Desk {
            cli: fixture.cli(),
            fixture: &fixture,
            written: Mutex::new(Vec::new()),
        };
        classified(&governor, &case, &mut ids);
        approved(&governor, &case, &mut ids);
        identified(&governor, &case, &mut ids);

        assert_eq!(run_route(&governor, &case, &desk), [ROUTE], "{test}");
        assert_eq!(fixture.invokes(), 1, "{test}: invoked once, never again");
        assert!(record(&governor, &desk, &mut ids).is_empty(), "{test}");
        assert!(routing_records(&governor, &case).is_empty(), "{test}");
        assert_eq!(
            claim(&frontier(&governor, &case), "ticket.routed"),
            Truth::Unknown,
            "{test}"
        );
    }
}
