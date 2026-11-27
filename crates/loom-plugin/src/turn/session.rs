//! Run the session executor through Commission with the single proposal-capture effect.
use super::*;
use loom::agent as authored;
use loom::agent::obligations::EndServedSessionTurnBehavior;
use loom_sdk::commission::model::responsibility as policy;
use loom_sdk::governor::{FileCaseStore, FileSuspensionStore};

pub(super) fn run_budget(b: &authored::RunBudget) -> policy::RunBudget {
    policy::RunBudget {
        max_steps: b.max_steps,
        max_tokens_in: b.max_tokens_in,
        max_tokens_out: b.max_tokens_out,
        max_duration_ms: b.max_duration_ms,
        max_cost_micro_usd: b.max_cost_micro_usd,
        max_tokens: b.max_tokens,
        deadline: b.deadline.as_ref().map(|at| Timestamp(at.0.clone())),
        per_action: b
            .per_action
            .iter()
            .map(|a| policy::ActionBudget {
                action: a.action.clone(),
                max_trials: a.max_trials,
                max_tokens_in: a.max_tokens_in,
                max_tokens_out: a.max_tokens_out,
                max_duration_ms: a.max_duration_ms,
                max_cost_micro_usd: a.max_cost_micro_usd,
            })
            .collect(),
    }
}

fn breach(step: authored::BreachStep) -> policy::BreachStep {
    match step {
        authored::BreachStep::KeepStep => policy::BreachStep::KeepStep,
        authored::BreachStep::DiscardStep => policy::BreachStep::DiscardStep,
    }
}

pub(super) fn stop_policy(s: Option<&authored::StopPolicy>) -> policy::StopPolicy {
    let Some(s) = s else {
        return policy::StopPolicy {
            approval_gate: true,
            idle_bound: None,
            on_breach: policy::BreachStep::DiscardStep,
            breach_by_limit: Vec::new(),
        };
    };
    policy::StopPolicy {
        approval_gate: s.approval_gate,
        idle_bound: s.idle_bound,
        on_breach: breach(s.on_breach),
        breach_by_limit: s
            .breach_by_limit
            .iter()
            .map(|b| policy::LimitBreachStep {
                step: breach(b.step),
                limit: match b.limit {
                    authored::BudgetLimit::MaxSteps => policy::BudgetLimit::MaxSteps,
                    authored::BudgetLimit::MaxTrials => policy::BudgetLimit::MaxTrials,
                    authored::BudgetLimit::MaxTokensIn => policy::BudgetLimit::MaxTokensIn,
                    authored::BudgetLimit::MaxTokensOut => policy::BudgetLimit::MaxTokensOut,
                    authored::BudgetLimit::MaxDurationMs => policy::BudgetLimit::MaxDurationMs,
                    authored::BudgetLimit::MaxCostMicroUsd => policy::BudgetLimit::MaxCostMicroUsd,
                    authored::BudgetLimit::MaxTokens => policy::BudgetLimit::MaxTokens,
                    authored::BudgetLimit::Deadline => policy::BudgetLimit::Deadline,
                },
            })
            .collect(),
    }
}

/// The immutable protocol plus durable case and suspension stores of one serving process.
pub struct SessionProtocol {
    governor: CanonGovernor<FileCaseStore>,
    suspensions: FileSuspensionStore,
}

impl SessionProtocol {
    /// Compile the proposal protocol once and open the host's state.
    pub fn open(state: &std::path::Path) -> Result<Self, String> {
        let store = FileCaseStore::open(state).map_err(|e| e.to_string())?;
        let governor = CanonGovernor::new(store)
            .with_catalog(&ProtocolCatalog::session_proposals()?)
            .map_err(|e| e.to_string())?;
        let suspensions = FileSuspensionStore::open(state).map_err(|e| e.to_string())?;
        Ok(Self {
            governor,
            suspensions,
        })
    }

    /// The incoming item gets one governed session. The only effect records its proposal.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        executor: &dyn AgentExecutor,
        item: &InboundItem,
        id: Uuid,
        session: &authored::SessionSpec,
        authority: &dyn AuthorityProvider,
        events: &mut dyn EventSink,
    ) -> Result<loom::plugin::RecordLine, String> {
        let ids = Ids::seeded(&id);
        let case = self
            .governor
            .open(
                loom_sdk::protocols::SESSION_PROPOSAL,
                BTreeMap::from([(ITEM.to_owned(), item.revision.clone())]),
                loom_sdk::CaseInputs::none(),
            )
            .map_err(|error| error.to_string())?;
        let commission = Commission::<commission_state::Assigned>::new(CommissionData {
            commission_id: CommissionId(id),
            agent_revision_id: AgentRevisionId(ids.next('c')),
            case_id: case.clone(),
            principal: PrincipalId(PRINCIPAL.to_owned()),
            authority_context: AuthorityContext(Value::Null),
        });
        let effects = Capture {
            evidence: crate::effects::ItemEvidence {
                governor: &self.governor,
                case: &case,
                item,
                source: "loom/session",
                producer: "loom/session-proposal",
            },
            ids: &ids,
            proposal: std::cell::RefCell::default(),
        };
        let run_id = ids.next('d');
        let mut runs = Generated::new(RunStore::new(move || RunId(run_id.clone())));
        let mut reporting = SessionEvents {
            events,
            summary: None,
        };
        let mut clock = Clock {
            ids: &ids,
            budget: 1,
            session_policy: Some(session),
            events: Some(&mut reporting),
            none: NoEvents,
        };
        let end = run_with_suspension(
            &self.governor,
            executor,
            authority,
            &effects,
            &commission,
            &mut runs,
            &self.suspensions,
            &mut clock,
        )
        .map_err(|error| error.to_string())?;
        let captured = effects.proposal.into_inner();
        let changed = captured.as_ref().is_some_and(|(_, count)| *count > 0);
        let proposal = captured.map(|(mut data, _)| {
            if let (Value::Object(fields), Some(summary)) = (&mut data, reporting.summary) {
                fields.push(("summary".into(), Value::Text(summary)));
            }
            let mut text = String::new();
            loom_sdk::commission::model::json::push_value(&mut text, &data);
            text
        });
        let completed = matches!(&end.outcome, RunOutcome::Completed(done) if done.outcome == "proposed")
            && proposal.is_some();
        let decision = loom::behaviour::Generated::new(())
            .end_served_session_turn(authored::EndServedSessionTurn {
                suspended: !completed,
                proposed_change: changed,
            })
            .map_err(|e| e.to_string())?;
        let (outcome, detail) = match decision {
            authored::EndServedSessionTurnOutcome::Proposed => {
                (RecordOutcome::Proposed, "Workspace changes proposed")
            }
            authored::EndServedSessionTurnOutcome::CompletedWithoutChange => (
                RecordOutcome::Proposed,
                "Session completed without a workspace change",
            ),
            authored::EndServedSessionTurnOutcome::Held => {
                let detail = match &end.outcome {
                    RunOutcome::AwaitingApproval(_) => "Session awaits approval",
                    RunOutcome::NeedsAuthority(_) => "Session lacks authority to propose",
                    RunOutcome::EffectRefused(_) => "The proposal was refused",
                    RunOutcome::Suspended(s) => match &s.reason {
                        SuspensionReason::Budget(_)
                        | SuspensionReason::LoopBudget(_)
                        | SuspensionReason::RunBudget(_) => "The session reached its budget",
                        SuspensionReason::ExternalAvailability(_) => {
                            "The harness could not produce an admitted proposal"
                        }
                        _ => "The governed session is suspended",
                    },
                    _ => "The governed session stopped before producing a proposal",
                };
                (RecordOutcome::Stopped, detail)
            }
        };
        Ok(loom::plugin::RecordLine {
            item: item.id.clone(),
            intent: None,
            confidence: None,
            outcome,
            reads: vec![],
            proposed_case: None,
            proposal: if completed { proposal } else { None },
            detail: Some(detail.into()),
            steps: vec![],
        })
    }
}

/// A completed session's summary belongs in the private reply, outside Canon evidence.
struct SessionEvents<'a> {
    events: &'a mut dyn EventSink,
    summary: Option<String>,
}
impl EventSink for SessionEvents<'_> {
    fn emit(&mut self, event: &policy::RuntimeEvent) {
        if let policy::RuntimeEvent::SessionClosed(closed) = event {
            self.summary = closed
                .summary
                .as_ref()
                .map(|s| s.chars().take(4096).collect());
        }
        self.events.emit(event);
    }
}

/// The only externally served session effect. The typed handler accepts a changed file with
/// text content or a deleted file with null content. The workspace is never opened for writing.
struct Capture<'a> {
    evidence: crate::effects::ItemEvidence<'a, FileCaseStore>,
    ids: &'a Ids,
    proposal: std::cell::RefCell<Option<(Value, usize)>>,
}

impl EffectPort for Capture<'_> {
    fn performs(&self, action: &str) -> bool {
        action == "session.propose"
    }
    fn invoke(
        &self,
        _: &Commission<commission_state::Assigned>,
        req: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = req.data();
        let args = &data.arguments.0;
        let refused = |why: &str| {
            Ok(EffectOutcome::Refused(policy::EffectOutcomeRefused {
                reason: why.into(),
                ends_run: true,
            }))
        };
        if self.evidence.case != &data.case_id || !self.performs(&data.action) {
            return refused("outside the session proposal");
        }
        let Some(Value::Array(files)) = args.member("files") else {
            return refused("session proposal needs a files array");
        };
        if !files.iter().all(|file| {
            matches!(file, Value::Object(fields) if fields.len() == 2)
                && matches!(file.member("path"), Some(Value::Text(path)) if !path.is_empty())
                && matches!(file.member("content"), Some(Value::Text(_) | Value::Null))
        }) {
            return refused("a proposed file needs its path and text or null content");
        }
        let mut text = String::new();
        loom_sdk::commission::model::json::push_value(&mut text, args);
        if text.len() > 64 * 1024 {
            return refused("session proposal exceeds 64 KiB");
        }
        let report = Value::Object(vec![(
            "file_count".into(),
            Value::Number(files.len().to_string()),
        )]);
        self.evidence.record(
            "session_proposed",
            report.clone(),
            || self.ids.next('a'),
            || 1,
        )?;
        self.proposal.replace(Some((args.clone(), files.len())));
        Ok(EffectOutcome::Performed(policy::EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}
