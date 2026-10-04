//! Acceptance for `story:run-outcomes`: the rule that derives a run's outcome from the governor's
//! determination, the executor's outcome and any authority verdict, and the Run's `Suspended`
//! state, which the suspend and resume commands enter and leave on the same run.
//!
//! Every derivation row is scripted through the fakes: the governor's answer through
//! [`FakeGovernor`], the executor's outcome through [`ScriptedExecutor`] and the authority verdict
//! through [`StaticAuthorityProvider`], asked only when the frontier says the proposed action
//! needs authority.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time:
//! a build directory shared between worktrees reuses binaries across them.

use b10x_commission::admission::admit;
use b10x_commission::model::behaviour::{Generated, RunStorage};
use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::obligations::{
    ResumeRunBehavior, RunStatesQuery, StartRunBehavior, SuspendRunBehavior,
};
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeNeedsHumanJudgment,
    ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, FrontierAction, FrontierObligation,
    HumanDecisionRequest, PrincipalId, ProposedActionArguments, ResumeRun, ResumeRunOutcome, RunId,
    RunOutcome, RunOutcomeCompleted, RunOutcomeNeedsAuthority, RunOutcomeNeedsExternalEvidence,
    RunOutcomeNeedsHumanJudgment, RunOutcomeSuspended, RunState, RunStateConflict, StartRun,
    StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionReason, Unit, commission_state,
};
use b10x_commission::outcome::{CapabilityVerdict, Derived, RunStore, derive};
use b10x_commission::ports::authority::{AuthorityCheck, check_authority};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use std::path::PathBuf;
use std::process::Command;

const NS: &str = "commission.responsibility.";

const CASE: &str = "case-run-outcomes";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn obligation(name: &str, open: bool) -> FrontierObligation {
    FrontierObligation {
        obligation: name.to_owned(),
        open,
    }
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

fn human_request(text: &str) -> HumanDecisionRequest {
    HumanDecisionRequest(
        json::parse(&format!(r#"{{"question":"{text}"}}"#))
            .unwrap_or_else(|error| panic!("human request fixture is not JSON: {error:?}")),
    )
}

/// One row of the derivation table.
struct Row {
    name: &'static str,
    /// What the governor answers for the case: its revision, determination and frontier.
    governor: Answer,
    /// What the executor returns for that frontier.
    executor: ExecutorOutcome,
    /// What the authority provider answers, should it be asked.
    authority: StaticAuthorityProvider,
    /// The capability the provider must be asked about, or `None` when it must not be asked.
    asked: Option<&'static str>,
    expected: Derived,
}

/// Runs one row through the fakes and the derivation.
///
/// The governor is asked for the frontier and the determination, the executor runs on that
/// frontier, and the provider is asked only when the executor proposes an action the frontier
/// marks as needing authority.
fn derived(row: &Row) -> Derived {
    let commission = commission();
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [row.governor.clone()]);
    let frontier = governor
        .frontier(&case)
        .unwrap_or_else(|error| panic!("{}: frontier call failed: {error:?}", row.name));
    let determination = governor
        .completion(&case)
        .unwrap_or_else(|error| panic!("{}: completion call failed: {error:?}", row.name));
    assert_eq!(
        governor.calls(),
        [
            GovernorCall::Frontier(case.clone()),
            GovernorCall::Completion(case)
        ],
        "{}: governor calls",
        row.name
    );

    let executor = ScriptedExecutor::new([row.executor.clone()]);
    let outcome = executor.run(&commission, &frontier);
    assert_eq!(executor.calls().len(), 1, "{}: executor calls", row.name);

    let verdict = match &outcome {
        ExecutorOutcome::ProposedAction(proposed) => match admit(&frontier, &proposed.action) {
            Admission::NeedsAuthority(needs) => {
                match check_authority(&row.authority, &commission, &needs.capability) {
                    AuthorityCheck::Decided(verdict) => Some((needs.capability, verdict)),
                    AuthorityCheck::Refused(error) => {
                        panic!("{}: the provider failed: {error}", row.name)
                    }
                }
            }
            _ => None,
        },
        _ => None,
    };
    let asked: Vec<String> = row
        .authority
        .asked()
        .into_iter()
        .map(|query| query.capability)
        .collect();
    assert_eq!(
        asked,
        row.asked.map(str::to_owned).into_iter().collect::<Vec<_>>(),
        "{}: authority questions",
        row.name
    );

    derive(
        &determination,
        &frontier,
        &outcome,
        verdict
            .as_ref()
            .map(|(capability, verdict)| CapabilityVerdict {
                capability,
                verdict,
            }),
    )
}

/// Expectations 1 to 6: one row each, and a second row for expectation 1.
fn derivation_rows() -> Vec<Row> {
    let admissible = || vec![action("inspect", ActionStatus::Admissible, None)];
    vec![
        Row {
            name: "1. completed: the governor reports the case complete with outcome X",
            governor: Answer::at(4).complete("X"),
            executor: ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Ended(RunOutcome::Completed(RunOutcomeCompleted {
                outcome: "X".to_owned(),
            })),
        },
        Row {
            name: "1. continue: CompletedLocalReasoning on a case the governor reports open",
            governor: Answer::at(4).with_items(Vec::new(), Vec::new(), admissible()),
            executor: ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Continue,
        },
        Row {
            name: "2. suspended: the executor returns Suspended with a reason",
            governor: Answer::at(4).with_items(Vec::new(), Vec::new(), admissible()),
            executor: ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: SuspensionReason::Evidence(vec!["build log".to_owned()]),
            }),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Ended(RunOutcome::Suspended(RunOutcomeSuspended {
                reason: SuspensionReason::Evidence(vec!["build log".to_owned()]),
            })),
        },
        Row {
            name: "3. needs authority: an ApprovalRequired action, the provider asks for Q",
            governor: Answer::at(4).with_items(
                Vec::new(),
                Vec::new(),
                vec![action(
                    "deploy",
                    ActionStatus::ApprovalRequired,
                    Some("prod.deploy"),
                )],
            ),
            executor: proposal("deploy"),
            authority: StaticAuthorityProvider::new()
                .answer(
                    "prod.deploy",
                    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                        request: "Q".to_owned(),
                    }),
                )
                .answer(
                    "other.capability",
                    AuthorityVerdict::Deny(AuthorityVerdictDeny {
                        reason: "not this one".to_owned(),
                    }),
                ),
            asked: Some("prod.deploy"),
            expected: Derived::Ended(RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
                request: "Q".to_owned(),
            })),
        },
        Row {
            name: "4. needs human judgment: the executor returns NeedsHumanJudgment with H",
            governor: Answer::at(4).with_items(Vec::new(), Vec::new(), admissible()),
            executor: ExecutorOutcome::NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment {
                request: human_request("H"),
            }),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Ended(RunOutcome::NeedsHumanJudgment(
                RunOutcomeNeedsHumanJudgment {
                    request: human_request("H"),
                },
            )),
        },
        Row {
            name: "5. needs external evidence: no admitted action, open obligations O",
            governor: Answer::at(4).with_items(
                Vec::new(),
                vec![
                    obligation("tests pass", true),
                    obligation("reviewed", false),
                    obligation("security sign-off", true),
                ],
                vec![action("deploy", ActionStatus::Blocked, None)],
            ),
            executor: ExecutorOutcome::NoUsefulAction(Unit(true)),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Ended(RunOutcome::NeedsExternalEvidence(
                RunOutcomeNeedsExternalEvidence {
                    requirements: vec!["tests pass".to_owned(), "security sign-off".to_owned()],
                },
            )),
        },
        Row {
            name: "6. no admissible action: no admitted action and no open obligation",
            governor: Answer::at(4).with_items(
                Vec::new(),
                vec![obligation("reviewed", false)],
                Vec::new(),
            ),
            executor: ExecutorOutcome::NoUsefulAction(Unit(true)),
            authority: StaticAuthorityProvider::new(),
            asked: None,
            expected: Derived::Ended(RunOutcome::NoAdmissibleAction(Unit(true))),
        },
    ]
}

/// Expectation 7: suspend and then resume, both through their commands, leave the same run
/// running: the same run id and the same case revision.
fn resume_continues_the_same_run() {
    const REVISION: i64 = 11;
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));

    let started = runs
        .start_run(StartRun {
            commission_id: CommissionId(uuid(1)),
            case_revision: REVISION,
        })
        .unwrap_or_else(|unmet| panic!("start: {unmet}"));
    let StartRunOutcome::Started { run_started } = started;
    let run_id = run_started.run_id.clone();
    assert_eq!(run_started.case_revision, REVISION, "RunStarted revision");

    // A running run cannot be resumed.
    assert_eq!(
        runs.resume_run(ResumeRun {
            run_id: run_id.clone(),
        }),
        Ok(ResumeRunOutcome::WrongState {
            error: RunStateConflict {
                state: RunState::Running,
            },
        }),
        "resume of a running run"
    );

    let reason = SuspensionReason::Human(human_request("approve the merge"));
    let suspended = runs
        .suspend_run(SuspendRun {
            run_id: run_id.clone(),
            reason: reason.clone(),
        })
        .unwrap_or_else(|unmet| panic!("suspend: {unmet}"));
    match suspended {
        SuspendRunOutcome::Suspended { run_suspended } => {
            assert_eq!(run_suspended.run_id, run_id, "RunSuspended run id");
            assert_eq!(run_suspended.reason, reason, "RunSuspended reason");
        }
        other => panic!("suspend of a running run: {other:?}"),
    }
    let held = RunStorage::get(&runs.ports, &run_id)
        .unwrap_or_else(|| panic!("the suspended run is not stored"));
    assert_eq!(held.state, RunState::Suspended, "state after suspend");

    // A suspended run cannot be suspended again.
    assert_eq!(
        runs.suspend_run(SuspendRun {
            run_id: run_id.clone(),
            reason,
        }),
        Ok(SuspendRunOutcome::WrongState {
            error: RunStateConflict {
                state: RunState::Suspended,
            },
        }),
        "suspend of a suspended run"
    );

    let resumed = runs
        .resume_run(ResumeRun {
            run_id: run_id.clone(),
        })
        .unwrap_or_else(|unmet| panic!("resume: {unmet}"));
    match resumed {
        ResumeRunOutcome::Resumed { run_resumed } => {
            assert_eq!(run_resumed.run_id, run_id, "RunResumed run id");
        }
        other => panic!("resume of a suspended run: {other:?}"),
    }

    let rows = runs
        .run_states()
        .unwrap_or_else(|unmet| panic!("RunStates: {unmet}"));
    assert_eq!(rows.len(), 1, "resume created another run: {rows:?}");
    let row = &rows[0];
    assert_eq!(row.run_id, run_id, "run id after resume");
    assert_eq!(row.commission_id, CommissionId(uuid(1)), "commission");
    assert_eq!(row.case_revision, REVISION, "case revision after resume");
    assert_eq!(row.state, RunState::Running, "state after resume");
}

/// Expectation 8: `RunOutcome` is declared in the specification as a union of the six variants
/// and reaches this crate generated, through `b10x-commission`'s re-export.
fn run_outcome_is_the_generated_type() {
    let path = root().join("ess/domains/responsibility.yaml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let declared = format!("- name: {NS}RunOutcome\n    kind: union\n");
    assert!(
        source.contains(&declared),
        "{} does not declare {NS}RunOutcome as a union",
        path.display()
    );

    let out = Command::new("ess")
        .args(["specify", "compile", "--path"])
        .arg(root().join("ess"))
        .args(["--format", "json"])
        .output()
        .unwrap_or_else(|error| panic!("run `ess specify compile`: {error}"));
    assert!(
        out.status.success(),
        "`ess specify compile` failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let model = json::parse(&String::from_utf8_lossy(&out.stdout))
        .unwrap_or_else(|error| panic!("compiled model is not JSON: {error:?}"));
    let body = model
        .member("types")
        .and_then(|types| types.member(&format!("{NS}RunOutcome")))
        .and_then(|declaration| declaration.member("body"))
        .unwrap_or_else(|| panic!("the compiled model declares no {NS}RunOutcome"));
    assert_eq!(
        body.member("kind"),
        Some(&Value::Text("union".to_owned())),
        "RunOutcome kind"
    );
    let mut variants: Vec<&str> = match body.member("variants") {
        Some(Value::Object(members)) => members.iter().map(|(name, _)| name.as_str()).collect(),
        other => panic!("RunOutcome variants: {other:?}"),
    };
    variants.sort_unstable();
    assert_eq!(
        variants,
        [
            "Completed",
            "NeedsAuthority",
            "NeedsExternalEvidence",
            "NeedsHumanJudgment",
            "NoAdmissibleAction",
            "Suspended",
        ],
        "RunOutcome variants"
    );

    let name = std::any::type_name::<RunOutcome>();
    assert!(
        name.starts_with("commission::") && !name.starts_with("b10x_commission::"),
        "{name} is not the generated type"
    );
}

#[test]
fn run_outcome_derivation() {
    for row in derivation_rows() {
        assert_eq!(derived(&row), row.expected, "{}", row.name);
    }
    resume_continues_the_same_run();
    run_outcome_is_the_generated_type();
}

/// A verdict counts only for the capability it was obtained for: the one the frontier names for
/// the proposed action. A verdict for another capability is treated like no verdict, so the
/// frontier decides; this frontier admits nothing and holds no open obligation.
#[test]
fn authority_verdict_binds_to_its_capability() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(4).with_items(
            Vec::new(),
            Vec::new(),
            vec![action(
                "deploy",
                ActionStatus::ApprovalRequired,
                Some("prod.deploy"),
            )],
        )],
    );
    let frontier = governor
        .frontier(&case)
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    let determination = governor
        .completion(&case)
        .unwrap_or_else(|error| panic!("completion call failed: {error:?}"));
    let outcome = proposal("deploy");

    let allow = AuthorityVerdict::Allow(Unit(true));
    let approval = AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: "Q".to_owned(),
    });
    let derive_with = |capability: &str, verdict: &AuthorityVerdict| {
        derive(
            &determination,
            &frontier,
            &outcome,
            Some(CapabilityVerdict {
                capability,
                verdict,
            }),
        )
    };
    let nothing_admissible = Derived::Ended(RunOutcome::NoAdmissibleAction(Unit(true)));

    assert_eq!(
        derive_with("prod.deploy", &allow),
        Derived::Continue,
        "an allow for the frontier's capability"
    );
    assert_eq!(
        derive_with("prod.deploy", &approval),
        Derived::Ended(RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: "Q".to_owned(),
        })),
        "approval required for the frontier's capability"
    );
    assert_eq!(
        derive_with("logs.read", &allow),
        nothing_admissible,
        "an allow for another capability must not continue"
    );
    assert_eq!(
        derive_with("logs.read", &approval),
        nothing_admissible,
        "approval required for another capability must not ask for it"
    );
    assert_eq!(
        derive(&determination, &frontier, &outcome, None),
        nothing_admissible,
        "no verdict"
    );
}
