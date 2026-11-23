//! The effect of a `person` action, taken only by `tree approve --action` as its principal: the
//! records the binding's evidence entries declare, each produced by that principal about the
//! subject's revision at that moment. `tree run` never takes one.
//!
//! A person binding that names an `argv` (`story:person-action-runs-a-program`) runs that
//! program inside this effect, prepared by `tree approve` before anything was written: started
//! once through the admitted runner with the principal in place of each `{principal}` entry, its
//! run observed. Only an exit of 0 records, with the placed artifacts' moves, in one store write;
//! any other end writes the moves alone and refuses the step.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use b10x_loom_bindings::{
    PERSON_SOURCE, PlacedRevisions, Prepared, RunPlace, Started, revisions_of, start,
};
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
use b10x_loom_intake_slice::confinement::TestRunner;
use loom::bindings::{BindingsDocument, Executor, ProgramRun, ResultSource};
use loom_governor::{CanonGovernor, FallibleCaseStore};

use crate::context::derived_uuid;

/// The result a person action's evidence entry reads when the person names none: its effect was
/// performed.
pub(crate) const PERFORMED: &str = "performed";

/// The person effect port of one case, for one principal.
pub(crate) struct PersonEffects<'a, S> {
    pub(crate) governor: &'a CanonGovernor<S>,
    pub(crate) case: CaseId,
    pub(crate) bindings: &'a BindingsDocument,
    pub(crate) workspace: PathBuf,
    pub(crate) principal: String,
    pub(crate) at: Timestamp,
    pub(crate) seed: String,
    /// The value of the entries' result tables the person names: [`PERFORMED`] when none.
    pub(crate) result: String,
    /// The digest of the person's reasons kept as a blob of the store, if any.
    pub(crate) reasons_blob: Option<String>,
    /// The store directory, which keeps the program's blob.
    pub(crate) store: PathBuf,
    /// The program of an action whose binding names an `argv`, prepared before anything was
    /// written; taken by the one step that starts it.
    pub(crate) program: RefCell<Option<Prepared>>,
    /// The admitted runner the program starts through, when there is a program.
    pub(crate) runner: Option<Arc<dyn TestRunner>>,
    /// How the program ended, once it ran: filled before the port returns.
    pub(crate) ended: RefCell<Option<ProgramEnded>>,
}

/// How the program of a person's approval ended when it did not end well
/// (`story:person-action-runs-a-program`): what `tree approve` names in its refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProgramEnd {
    /// An exit other than 0, or none read.
    Exit(Option<i64>),
    /// Killed at its timeout.
    TimedOut,
    /// The runner refused the start, its text.
    RunnerRefused(String),
    /// The program of an action that does not write changed a placed artifact.
    ChangedByNonWriter,
    /// A placed artifact could not be digested after the run.
    Unreadable(String),
}

/// The program a person's approval started and that did not end well: its run as observed, its
/// name and how it ended.
#[derive(Debug, Clone)]
pub(crate) struct ProgramEnded {
    pub(crate) run: ProgramRun,
    pub(crate) program: String,
    pub(crate) end: ProgramEnd,
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
        // A named result every entry lists, checked before anything is written: no half write.
        if self.result != PERFORMED
            && let Some(entry) = bound.evidence.iter().find(|entry| {
                entry.from == ResultSource::Effect
                    && !entry.results.iter().any(|rule| rule.value == self.result)
            })
        {
            return Ok(refused(format!(
                "the {} entry of {action} lists no result {}",
                entry.kind, self.result
            )));
        }
        let (targets, revisions, case_revision, run) = if bound.executor.argv.is_some() {
            let Some(prepared) = self.program.borrow_mut().take() else {
                return Ok(refused(format!(
                    "the program of {action} was not prepared before the approval was written"
                )));
            };
            match self.run_program(action, prepared)? {
                Ran::Ended(outcome) => return Ok(outcome),
                Ran::Wrote {
                    targets,
                    revisions,
                    case_revision,
                    run,
                } => (targets, revisions, case_revision, Some(run)),
            }
        } else {
            // The subject's revision at this moment: what the workspace holds now.
            PlacedRevisions::new(self.governor, self.bindings, &self.workspace)
                .refresh(&self.case)
                .map_err(|error| EffectError::new(error.to_string()))?;
            let revisions = self.governor.revisions(&self.case).map_err(failed)?;
            let case_revision = self.governor.current_revision_of(&self.case)?;
            (BTreeMap::new(), revisions, case_revision, None)
        };
        let observation = ObservationId(derived_uuid(&[
            "person-observation",
            &self.seed,
            &self.case.0,
            action,
        ]));
        let mut payload = serde_json::json!({
            "case": self.case.0,
            "action": action,
            "principal": self.principal,
        });
        // The reasons are named by digest only: their text is in the blob, never in a record.
        if let Some(digest) = &self.reasons_blob {
            payload["blob_sha256"] = serde_json::Value::String(digest.clone());
        }
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: PERSON_SOURCE.to_owned(),
                subject: action.to_owned(),
                observed_at: self.at.clone(),
                payload: commission(&payload),
            }))
            .map_err(failed)?;
        // The person's observation alone: a record's instant is the approval's (`--at`), and the
        // program run's observation holds the wall clock's, which a record may not cite.
        let observation_ids = vec![observation.clone()];
        let held = Held::default();
        let mut recorded = Vec::new();
        for (index, entry) in bound.evidence.iter().enumerate() {
            if entry.from != ResultSource::Effect {
                continue;
            }
            let result = entry
                .results
                .iter()
                .find(|rule| rule.value == self.result)
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
            // The revision the case holds for each upstream artifact the entry declares, as the
            // program and open paths record it; no member when none is declared.
            if !entry.upstream.is_empty() {
                facts["upstream_revisions"] = entry
                    .upstream
                    .iter()
                    .map(|artifact| {
                        let revision = revisions
                            .get(artifact)
                            .cloned()
                            .unwrap_or_else(|| "none".to_owned());
                        (artifact.clone(), serde_json::Value::String(revision))
                    })
                    .collect::<serde_json::Map<_, _>>()
                    .into();
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
                    observation_ids: observation_ids.clone(),
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
        // The program's moves and the records in one store write: a crash leaves neither or both.
        self.governor
            .record_step(&self.case, &targets, held.0.into_inner())
            .map_err(failed)?;
        let mut report = serde_json::json!({
            "person": self.principal,
            "evidence": recorded,
        });
        if let Some(run) = run {
            report["run"] = run;
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: commission(&report),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }

    /// Starts the prepared program of `action` once and keeps its run. A program that did not end
    /// well writes its placed artifacts' moves alone (none when one could not be digested), fills
    /// [`PersonEffects::ended`] and refuses the step; one that ended with exit 0 gives the moves
    /// and revisions the records are written with.
    fn run_program(&self, action: &str, prepared: Prepared) -> Result<Ran, EffectError> {
        let Some(runner) = self.runner.as_deref() else {
            return Ok(Ran::Ended(refused(format!(
                "the program of {action} has no runner"
            ))));
        };
        let place = RunPlace {
            governor: self.governor,
            case: &self.case,
            document: self.bindings,
            workspace: &self.workspace,
            store: &self.store,
            runner,
            cancel: None,
        };
        let started = start(&place, prepared)?;
        let changed = started.changed();
        let run_json = started.run_json();
        let Started {
            run,
            answer,
            blob_failure,
            after,
            writes,
            program,
            ..
        } = started;
        let after = match after {
            Ok(after) => after,
            Err(unreadable) => {
                return Ok(self.ended(run, program, ProgramEnd::Unreadable(unreadable)));
            }
        };
        let targets = revisions_of(&after);
        let end = match &answer {
            Err(refusal) => Some(ProgramEnd::RunnerRefused(refusal.to_string())),
            Ok(execution) if execution.timed_out => Some(ProgramEnd::TimedOut),
            Ok(execution) if execution.exit_code != Some(0) => {
                Some(ProgramEnd::Exit(execution.exit_code.map(i64::from)))
            }
            Ok(_) if changed && !writes => Some(ProgramEnd::ChangedByNonWriter),
            Ok(_) => None,
        };
        if let Some(failure) = blob_failure {
            self.write(&targets)?;
            return Err(EffectError::new(failure));
        }
        if let Some(end) = end {
            self.write(&targets)?;
            return Ok(self.ended(run, program, end));
        }
        // The revisions the case holds once this step's moves are written.
        let mut revisions = self.governor.revisions(&self.case).map_err(failed)?;
        let moving = targets
            .iter()
            .filter(|(artifact, revision)| revisions.get(*artifact) != Some(*revision))
            .count();
        let case_revision = self.governor.current_revision_of(&self.case)?
            + i64::try_from(moving).unwrap_or(i64::MAX);
        revisions.extend(targets.clone());
        Ok(Ran::Wrote {
            targets,
            revisions,
            case_revision,
            run: run_json,
        })
    }

    /// Writes the moves `targets` alone, in one store write.
    fn write(&self, targets: &BTreeMap<String, String>) -> Result<(), EffectError> {
        self.governor
            .record_step(&self.case, targets, Vec::new())
            .map(drop)
            .map_err(|error| {
                EffectError::new(format!(
                    "the step of case {} was not written, nothing of it was kept: {error}",
                    self.case.0
                ))
            })
    }

    /// The step refused, its program having ended `end`: kept in [`PersonEffects::ended`].
    fn ended(&self, run: ProgramRun, program: String, end: ProgramEnd) -> Ran {
        let reason = format!("the program {program} ended: {end:?}");
        *self.ended.borrow_mut() = Some(ProgramEnded { run, program, end });
        Ran::Ended(refused(reason))
    }
}

/// What the start of a person's program gave the step.
enum Ran {
    /// The step ends with this refusal; nothing more is written.
    Ended(EffectOutcome),
    /// The program ended with exit 0: the moves the records are written with, the revisions the
    /// case holds once they are, the case's revision then, and the run as the report names it.
    Wrote {
        targets: BTreeMap<String, String>,
        revisions: BTreeMap<String, String>,
        case_revision: i64,
        run: serde_json::Value,
    },
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
