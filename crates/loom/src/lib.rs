#![forbid(unsafe_code)]

//! Bootstrap Loom harness contracts.

use b10x_canon::{ActionCandidate, ActionId, ActionStatus, Frontier};
use b10x_commission::{AgentExecutor, Commission, ExecutorOutcome};

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub action: ActionId,
    pub confidence: Option<f32>,
}

pub trait ActionSelector {
    fn select(&self, frontier: &Frontier, prompt: &str) -> Result<Selection, String>;
}

pub trait ArgumentGenerator {
    fn generate(&self, action: &ActionCandidate, prompt: &str) -> Result<String, String>;
}

/// Deterministic bootstrap selector used only for tests/examples.
#[derive(Debug, Default)]
pub struct FirstAdmissibleSelector;

impl ActionSelector for FirstAdmissibleSelector {
    fn select(&self, frontier: &Frontier, _prompt: &str) -> Result<Selection, String> {
        let candidate = frontier
            .actions
            .iter()
            .find(|candidate| matches!(&candidate.status, ActionStatus::Admissible))
            .ok_or_else(|| "no admissible action".to_owned())?;

        Ok(Selection {
            action: candidate.id.clone(),
            confidence: Some(1.0),
        })
    }
}

#[derive(Debug, Default)]
pub struct EmptyObjectArguments;

impl ArgumentGenerator for EmptyObjectArguments {
    fn generate(&self, _action: &ActionCandidate, _prompt: &str) -> Result<String, String> {
        Ok("{}".to_owned())
    }
}

pub struct Loom<S, G> {
    selector: S,
    arguments: G,
    prompt: String,
}

impl<S, G> Loom<S, G> {
    pub fn new(selector: S, arguments: G, prompt: impl Into<String>) -> Self {
        Self {
            selector,
            arguments,
            prompt: prompt.into(),
        }
    }
}

impl<S, G> AgentExecutor for Loom<S, G>
where
    S: ActionSelector,
    G: ArgumentGenerator,
{
    fn run(
        &self,
        _commission: &Commission,
        frontier: &Frontier,
    ) -> Result<ExecutorOutcome, String> {
        let selection = self.selector.select(frontier, &self.prompt)?;

        // Safety invariant: selector may only choose from the current frontier.
        if !frontier.contains_action(&selection.action) {
            return Err(format!(
                "selector chose action outside the current frontier: {}",
                selection.action.0
            ));
        }

        let action = frontier
            .actions
            .iter()
            .find(|candidate| candidate.id == selection.action)
            .ok_or_else(|| "selected action disappeared from frontier".to_owned())?;

        // Approval-gated actions are not executed by Loom directly.
        if let ActionStatus::ApprovalRequired { capability } = &action.status {
            return Ok(ExecutorOutcome::Suspended {
                reason: format!("authority required: {capability}"),
            });
        }

        let arguments_json = self.arguments.generate(action, &self.prompt)?;

        Ok(ExecutorOutcome::ProposedAction {
            action: selection.action,
            arguments_json,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_canon::{CaseId, ClaimValue};
    use b10x_commission::{AgentId, CommissionId};

    struct BadSelector;

    impl ActionSelector for BadSelector {
        fn select(&self, _frontier: &Frontier, _prompt: &str) -> Result<Selection, String> {
            Ok(Selection {
                action: ActionId("forbidden.action".into()),
                confidence: Some(0.99),
            })
        }
    }

    fn commission() -> Commission {
        Commission {
            id: CommissionId("COM-1".into()),
            agent: AgentId("agent-1".into()),
            case: CaseId("CASE-1".into()),
        }
    }

    #[test]
    fn selector_cannot_expand_frontier() {
        let frontier = Frontier {
            case: CaseId("CASE-1".into()),
            revision: 1,
            claims: Vec::<ClaimValue>::new(),
            obligations: vec![],
            actions: vec![ActionCandidate {
                id: ActionId("metrics.inspect".into()),
                status: ActionStatus::Admissible,
            }],
            blocked_conclusions: vec![],
        };

        let loom = Loom::new(BadSelector, EmptyObjectArguments, "investigate");
        let err = loom.run(&commission(), &frontier).unwrap_err();

        assert!(err.contains("outside the current frontier"));
    }
}
