//! The three views of `loom.decision`, answered by their generated queries over what a
//! [`DecisionStore`] lists.

use loom::behaviour::{DecisionRequestStorage, Generated};
use loom::decision::obligations::{
    ArtifactDecisionHistoryQuery, DecisionRequestsQuery, OpenDecisionsForPrincipalQuery,
};
use loom::decision::{
    ArtifactDecisionHistory, ArtifactId, DecisionRequestId, DecisionRequestSnapshot,
    DecisionRequests, OpenDecisionsForPrincipal, Principal,
};

use crate::store::{DecisionStore, StoreFailure};

/// The requests a store listed, as the generated queries read them.
struct Listed(Vec<DecisionRequestSnapshot>);

impl DecisionRequestStorage for Listed {
    fn get(&self, identity: &DecisionRequestId) -> Option<DecisionRequestSnapshot> {
        self.0
            .iter()
            .find(|held| &held.data.request_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: DecisionRequestSnapshot) {
        self.0.push(snapshot);
    }

    fn delete(&mut self, identity: &DecisionRequestId) {
        self.0.retain(|held| &held.data.request_id != identity);
    }

    fn list(&self) -> Vec<DecisionRequestSnapshot> {
        self.0.clone()
    }
}

fn listed<S: DecisionStore + ?Sized>(store: &S) -> Result<Generated<Listed>, StoreFailure> {
    Ok(Generated::new(Listed(store.list()?)))
}

fn unmet(view: &str, error: &loom::obligation::UnmetObligation) -> StoreFailure {
    StoreFailure {
        at: format!("the view {view}"),
        why: error.to_string(),
    }
}

/// `loom.decision.OpenDecisionsForPrincipal`: the Open and Escalated requests addressed to
/// `principal`, in the order the store lists them.
///
/// # Errors
///
/// [`StoreFailure`] when the store cannot list.
pub fn open_decisions_for_principal<S: DecisionStore + ?Sized>(
    store: &S,
    principal: &Principal,
) -> Result<Vec<OpenDecisionsForPrincipal>, StoreFailure> {
    listed(store)?
        .open_decisions_for_principal(principal.clone())
        .map_err(|error| unmet("loom.decision.OpenDecisionsForPrincipal", &error))
}

/// `loom.decision.ArtifactDecisionHistory`: every request about `artifact`, in the order the
/// store lists them.
///
/// # Errors
///
/// [`StoreFailure`] when the store cannot list.
pub fn artifact_decision_history<S: DecisionStore + ?Sized>(
    store: &S,
    artifact: &ArtifactId,
) -> Result<Vec<ArtifactDecisionHistory>, StoreFailure> {
    listed(store)?
        .artifact_decision_history(artifact.clone())
        .map_err(|error| unmet("loom.decision.ArtifactDecisionHistory", &error))
}

/// `loom.decision.DecisionRequests`: every request, in the order the store lists them.
///
/// # Errors
///
/// [`StoreFailure`] when the store cannot list.
pub fn decision_requests<S: DecisionStore + ?Sized>(
    store: &S,
) -> Result<Vec<DecisionRequests>, StoreFailure> {
    listed(store)?
        .decision_requests()
        .map_err(|error| unmet("loom.decision.DecisionRequests", &error))
}
