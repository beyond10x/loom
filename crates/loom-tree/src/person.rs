//! The effect of a `person` action, taken only by `tree approve --action` as its principal: the
//! records the binding's evidence entries declare, each produced by that principal about the
//! subject's revision at that moment. `tree run` never takes one.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use b10x_loom_bindings::PlacedRevisions;
use b10x_loom_commission::model::json::Value as CommissionJson;
use b10x_loom_commission::model::primitives::Timestamp;
use b10x_loom_commission::model::responsibility::{
    CaseId, Commission, EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused, EvidenceData,
    EvidenceId, GovernorError, Observation, ObservationData, ObservationId, commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{
    AttributedEvidence, EvidencePort, ObservationPort, submit_evidence,
};
use loom::bindings::{BindingsDocument, Executor, ResultSource};
use loom_governor::{CanonGovernor, FallibleCaseStore};

use crate::context::derived_uuid;

/// The source of the observation a person action's effect records.
pub const PERSON_SOURCE: &str = "loom/person";

/// The result a person action's evidence entry reads: its effect was performed.
const PERFORMED: &str = "performed";

/// The person effect port of one case, for one principal.
pub(crate) struct PersonEffects<'a, S> {
    pub(crate) governor: &'a CanonGovernor<S>,
    pub(crate) case: CaseId,
    pub(crate) bindings: &'a BindingsDocument,
    pub(crate) workspace: PathBuf,
    pub(crate) principal: String,
    pub(crate) at: Timestamp,
    pub(crate) seed: String,
}

fn refused(reason: String) -> EffectOutcome {
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason,
        ends_run: true,
    })
}

fn failed(error: impl std::fmt::Debug) -> EffectError {
    EffectError::new(format!("the governor: {error:?}"))
}

impl<S: FallibleCaseStore> PersonEffects<'_, S> {
    fn perform(&self, action: &str) -> Result<EffectOutcome, EffectError> {
        let Some(bound) = self
            .bindings
            .actions
            .iter()
            .find(|bound| bound.action == action && bound.executor.kind == Executor::Person)
        else {
            return Ok(refused(format!("no person is bound to {action}")));
        };
        if bound.executor.principal.as_deref() != Some(self.principal.as_str()) {
            return Ok(refused(format!(
                "{action} is bound to the person {}, not to {}",
                bound.executor.principal.as_deref().unwrap_or("nobody"),
                self.principal
            )));
        }
        // The subject's revision at this moment: what the workspace holds now.
        PlacedRevisions::new(self.governor, self.bindings, &self.workspace)
            .refresh(&self.case)
            .map_err(|error| EffectError::new(error.to_string()))?;
        let revisions = self.governor.revisions(&self.case).map_err(failed)?;
        let case_revision = self.governor.current_revision_of(&self.case)?;
        let observation = ObservationId(derived_uuid(&[
            "person-observation",
            &self.seed,
            &self.case.0,
            action,
        ]));
        let payload = serde_json::json!({
            "case": self.case.0,
            "action": action,
            "principal": self.principal,
        });
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: PERSON_SOURCE.to_owned(),
                subject: action.to_owned(),
                observed_at: self.at.clone(),
                payload: commission(&payload),
            }))
            .map_err(failed)?;
        let held = Held::default();
        let mut recorded = Vec::new();
        for (index, entry) in bound.evidence.iter().enumerate() {
            if entry.from != ResultSource::Effect {
                continue;
            }
            let result = entry
                .results
                .iter()
                .find(|rule| rule.value == PERFORMED)
                .and_then(|rule| rule.result.clone());
            let subject_revision = revisions
                .get(&entry.subject)
                .cloned()
                .unwrap_or_else(|| "none".to_owned());
            let id = EvidenceId(derived_uuid(&[
                "person-evidence",
                &self.seed,
                &self.case.0,
                action,
                &index.to_string(),
            ]));
            let mut facts = serde_json::json!({
                "format": "canon-evidence/1",
                "id": format!("person-{}", id.0.0),
                "kind": entry.kind,
                "subject": entry.subject,
                "subject_revision": subject_revision,
                "observed_at": self.at.0,
            });
            if let Some(result) = &result {
                facts["result"] = serde_json::Value::String(result.clone());
            }
            submit_evidence(
                &held,
                &self.principal,
                EvidenceData {
                    evidence_id: id.clone(),
                    case_id: self.case.clone(),
                    kind: entry.kind.clone(),
                    subject_revision: case_revision,
                    producer: self.principal.clone(),
                    observation_ids: vec![observation.clone()],
                    facts: commission(&facts),
                    provenance: commission(&serde_json::json!({"source": PERSON_SOURCE})),
                },
            )
            .map_err(|error| EffectError::new(format!("the {} record: {error}", entry.kind)))?;
            recorded.push(serde_json::json!({
                "evidence_id": id.0.0,
                "kind": entry.kind,
                "subject": entry.subject,
                "subject_revision": subject_revision,
                "result": result,
                "producer": self.principal,
            }));
        }
        self.governor
            .record_step(&self.case, &BTreeMap::new(), held.0.into_inner())
            .map_err(failed)?;
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: commission(&serde_json::json!({
                "person": self.principal,
                "evidence": recorded,
            })),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

trait CurrentRevision {
    fn current_revision_of(&self, case: &CaseId) -> Result<i64, EffectError>;
}

impl<S: FallibleCaseStore> CurrentRevision for CanonGovernor<S> {
    fn current_revision_of(&self, case: &CaseId) -> Result<i64, EffectError> {
        b10x_loom_commission::ports::governor::Governor::current_revision(self, case)
            .map_err(failed)
    }
}

impl<S: FallibleCaseStore> EffectPort for PersonEffects<'_, S> {
    fn performs(&self, action: &str) -> bool {
        self.bindings
            .actions
            .iter()
            .any(|bound| bound.action == action && bound.executor.kind == Executor::Person)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let request = request.data();
        if request.case_id != self.case {
            return Ok(refused(format!(
                "the request is for case {}, and this port takes case {}",
                request.case_id.0, self.case.0
            )));
        }
        self.perform(&request.action)
    }
}

/// The records of a step, held for its one write.
#[derive(Default)]
struct Held(RefCell<Vec<AttributedEvidence>>);

impl EvidencePort for Held {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        self.0.borrow_mut().push(evidence);
        Ok(())
    }
}

/// `value` as Commission's JSON.
pub(crate) fn commission(value: &serde_json::Value) -> CommissionJson {
    match value {
        serde_json::Value::Null => CommissionJson::Null,
        serde_json::Value::Bool(value) => CommissionJson::Bool(*value),
        serde_json::Value::Number(number) => CommissionJson::Number(number.to_string()),
        serde_json::Value::String(text) => CommissionJson::Text(text.clone()),
        serde_json::Value::Array(items) => {
            CommissionJson::Array(items.iter().map(commission).collect())
        }
        serde_json::Value::Object(members) => CommissionJson::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), commission(member)))
                .collect(),
        ),
    }
}
