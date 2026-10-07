//! A read-only host clock binding. Only a reading made here can become clock evidence.
use std::io::Write;

use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, ExecutorOutcomeProposedAction, Observation,
    ObservationData, ObservationId, commission_state,
};
use b10x_loom_commission::model::{json, primitives::Timestamp};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use chrono::{DateTime, FixedOffset, Local, SecondsFormat, Utc};
use intake_model::query::TimeObservation;
use loom_governor::{CanonGovernor, CaseStore};

use crate::case::INTENT;
use crate::effect::{Console, refuse};
use crate::executor::fresh_uuid;
use crate::run::{SliceError, json_text};
use crate::selector::Briefing;

pub const READ_TIME: &str = "system.time.read";
const SOURCE: &str = "loom/host-clock";
const PRODUCER: &str = "loom/clock-verifier";

/// A trusted host clock. One successful call supplies one instant and its local UTC offset.
pub trait Clock {
    fn read(&self) -> Result<DateTime<FixedOffset>, String>;
}

/// Read the operating system clock and its configured local timezone, without a subprocess.
pub struct HostClock;
impl Clock for HostClock {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        std::panic::catch_unwind(|| Local::now().fixed_offset())
            .map_err(|_| "host clock or local timezone conversion failed".to_owned())
    }
}

// Private construction prevents model-supplied fields or public report values being verified.
#[derive(Debug)]
struct ClockReport {
    value: TimeObservation,
    revision: i64,
    observation: ObservationId,
}

pub(crate) struct ClockEffects<'a, 'o, S> {
    governor: &'a CanonGovernor<S>,
    case: CaseId,
    intent_revision: &'a str,
    clock: &'a dyn Clock,
    briefing: Briefing,
    console: &'a Console<'o>,
}

impl<'a, 'o, S: CaseStore> ClockEffects<'a, 'o, S> {
    pub(crate) fn new(
        governor: &'a CanonGovernor<S>,
        case: CaseId,
        intent_revision: &'a str,
        clock: &'a dyn Clock,
        briefing: Briefing,
        console: &'a Console<'o>,
    ) -> Self {
        Self {
            governor,
            case,
            intent_revision,
            clock,
            briefing,
            console,
        }
    }

    fn read(&self) -> Result<ClockReport, SliceError> {
        let revision = self.governor.current_revision(&self.case)?;
        if self
            .governor
            .revisions(&self.case)?
            .get(INTENT)
            .map(String::as_str)
            != Some(self.intent_revision)
        {
            return Err(SliceError::Clock(
                "query revision changed before clock read".into(),
            ));
        }
        let instant = self.clock.read().map_err(SliceError::Clock)?;
        if instant.offset().local_minus_utc() % 60 != 0 {
            return Err(SliceError::Clock(
                "local offset cannot be represented exactly as RFC3339 minutes".into(),
            ));
        }
        let utc = instant
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true);
        let value = TimeObservation {
            case_id: self.case.0.clone(),
            intent_revision: self.intent_revision.into(),
            utc: utc.clone(),
            local: instant.to_rfc3339_opts(SecondsFormat::Secs, false),
            offset_seconds: i64::from(instant.offset().local_minus_utc()),
        };
        let observation = ObservationId(fresh_uuid("clock-observation", &self.case.0));
        self.governor.observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: SOURCE.into(),
            subject: INTENT.into(),
            observed_at: Timestamp(utc),
            payload: as_json(&value)?,
        }))?;
        Ok(ClockReport {
            value,
            revision,
            observation,
        })
    }

    fn verify(&self, report: &ClockReport) -> Result<(), SliceError> {
        if report.value.case_id != self.case.0
            || report.value.intent_revision != self.intent_revision
            || self.governor.current_revision(&self.case)? != report.revision
            || self.governor.revisions(&self.case)?.get(INTENT)
                != Some(&report.value.intent_revision)
        {
            return Err(SliceError::Clock(
                "clock observation belongs to a different case or revision".into(),
            ));
        }
        if self
            .governor
            .evidence(&self.case)?
            .iter()
            .any(|e| e.observation_ids.contains(&report.observation))
        {
            return Err(SliceError::Clock(
                "clock observation already verified".into(),
            ));
        }
        let id = EvidenceId(fresh_uuid("clock-evidence", &self.case.0));
        let facts = serde_json::json!({"format":"canon-evidence/1", "id":format!("clock-{}", id.0.0),
            "kind":"system_time", "result":"observed", "subject":INTENT,
            "subject_revision":report.value.intent_revision});
        submit_evidence(
            self.governor,
            PRODUCER,
            EvidenceData {
                evidence_id: id,
                case_id: self.case.clone(),
                kind: "system_time".into(),
                subject_revision: report.revision,
                producer: PRODUCER.into(),
                observation_ids: vec![report.observation.clone()],
                facts: json::parse(&facts.to_string())
                    .map_err(|e| SliceError::Clock(format!("clock evidence: {e:?}")))?,
                provenance: json::Value::Object(vec![(
                    "source".into(),
                    json::Value::Text(SOURCE.into()),
                )]),
            },
        )
        .map_err(|e| SliceError::Clock(e.to_string()))?;
        Ok(())
    }

    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, SliceError> {
        let step = self.console.next_step();
        self.console.write(|out| {
            writeln!(
                out,
                "step {step}: {} {}",
                crate::run::printable(&request.action),
                crate::run::printable(&json_text(&request.arguments.0))
            )?;
            Ok(())
        })?;
        if request.case_id != self.case
            || request.action != READ_TIME
            || !matches!(&request.arguments.0, json::Value::Object(fields) if fields.is_empty())
        {
            let reason = "system.time.read requires this case and exactly an empty argument object";
            self.console
                .write(|out| refuse(out, &self.briefing, &request.action, reason))?;
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: reason.into(),
            }));
        }
        let report = self.read()?;
        self.verify(&report)?;
        self.console.write(|out| {
            render(out, &report.value)?;
            writeln!(out, "  evidence: system_time observed")?;
            Ok(())
        })?;
        self.briefing.record_time(
            &ExecutorOutcomeProposedAction {
                action: request.action.clone(),
                arguments: request.arguments.clone(),
            },
            &report.value,
        );
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: as_json(&report.value)?,
            attempt: None,
        }))
    }
}

impl<S: CaseStore> EffectPort for ClockEffects<'_, '_, S> {
    fn performs(&self, action: &str) -> bool {
        action == READ_TIME
    }
    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.perform(request.data()).map_err(|failure| {
            let error = EffectError::new(failure.to_string());
            self.console.fail(failure);
            error
        })
    }
}

fn as_json(value: &TimeObservation) -> Result<json::Value, SliceError> {
    let value = serde_json::json!({"case_id":value.case_id,"intent_revision":value.intent_revision,
        "utc":value.utc,"local":value.local,"offset_seconds":value.offset_seconds});
    json::parse(&value.to_string())
        .map_err(|e| SliceError::Clock(format!("clock observation: {e:?}")))
}
fn render(out: &mut dyn Write, value: &TimeObservation) -> std::io::Result<()> {
    writeln!(out, "  effect: Local time: {}", value.local)?;
    writeln!(out, "    | UTC: {}", value.utc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_governor::MemoryCaseStore;
    use loom_protocols::ProtocolCatalog;
    struct Fixed(&'static str);
    impl Clock for Fixed {
        fn read(&self) -> Result<DateTime<FixedOffset>, String> {
            DateTime::parse_from_rfc3339(self.0).map_err(|e| e.to_string())
        }
    }
    #[test]
    fn verifier_refuses_another_case_stale_intent_and_duplicate_observation() {
        let catalog = ProtocolCatalog::bundled().unwrap();
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .unwrap();
        let case = governor
            .open(
                "system-query@1",
                [(INTENT.into(), "intent-1".into())].into(),
            )
            .unwrap();
        let other = governor
            .open(
                "system-query@1",
                [(INTENT.into(), "intent-1".into())].into(),
            )
            .unwrap();
        let mut out = Vec::new();
        let console = Console::new(&mut out);
        let clock = Fixed("2026-10-07T00:30:00+02:00");
        let effect = ClockEffects::new(
            &governor,
            case.clone(),
            "intent-1",
            &clock,
            Briefing::new("time", vec![]).for_query(),
            &console,
        );
        let mut report = effect.read().unwrap();
        report.value.case_id = other.0;
        assert!(
            effect
                .verify(&report)
                .unwrap_err()
                .to_string()
                .contains("different case")
        );
        assert!(governor.evidence(&case).unwrap().is_empty());
        report.value.case_id = case.0.clone();
        effect.verify(&report).unwrap();
        assert_eq!(governor.evidence(&case).unwrap().len(), 1);
        assert!(
            effect
                .verify(&report)
                .unwrap_err()
                .to_string()
                .contains("already verified")
        );
        governor.update_revision(&case, INTENT, "intent-2").unwrap();
        assert!(
            effect
                .verify(&report)
                .unwrap_err()
                .to_string()
                .contains("different case")
        );
        assert!(
            effect
                .read()
                .unwrap_err()
                .to_string()
                .contains("revision changed")
        );
    }
    #[test]
    fn non_minute_local_offset_refuses_without_evidence() {
        struct SecondsOffset;
        impl Clock for SecondsOffset {
            fn read(&self) -> Result<DateTime<FixedOffset>, String> {
                Ok(DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&FixedOffset::east_opt(30).unwrap()))
            }
        }
        let catalog = ProtocolCatalog::bundled().unwrap();
        let governor = CanonGovernor::new(MemoryCaseStore::default())
            .with_catalog(&catalog)
            .unwrap();
        let case = governor
            .open(
                "system-query@1",
                [(INTENT.into(), "intent-1".into())].into(),
            )
            .unwrap();
        let mut out = Vec::new();
        let console = Console::new(&mut out);
        let effect = ClockEffects::new(
            &governor,
            case.clone(),
            "intent-1",
            &SecondsOffset,
            Briefing::new("time", vec![]),
            &console,
        );
        assert!(
            effect
                .read()
                .unwrap_err()
                .to_string()
                .contains("represented exactly")
        );
        assert!(governor.evidence(&case).unwrap().is_empty());
    }
    #[test]
    fn utc_and_local_are_the_same_instant_across_day_and_dst_boundaries() {
        for text in [
            "2026-10-25T02:30:00+02:00",
            "2026-10-25T02:30:00+01:00",
            "2026-01-01T00:15:00+14:00",
            "2026-01-01T23:45:00-12:00",
        ] {
            let catalog = ProtocolCatalog::bundled().unwrap();
            let governor = CanonGovernor::new(MemoryCaseStore::default())
                .with_catalog(&catalog)
                .unwrap();
            let case = governor
                .open(
                    "system-query@1",
                    [(INTENT.into(), "intent-1".into())].into(),
                )
                .unwrap();
            let mut out = Vec::new();
            let console = Console::new(&mut out);
            let clock = Fixed(text);
            let effect = ClockEffects::new(
                &governor,
                case,
                "intent-1",
                &clock,
                Briefing::new("time", vec![]),
                &console,
            );
            let report = effect.read().unwrap();
            let local = DateTime::parse_from_rfc3339(&report.value.local).unwrap();
            let utc = DateTime::parse_from_rfc3339(&report.value.utc).unwrap();
            assert_eq!(local.timestamp(), utc.timestamp());
            assert_eq!(
                i64::from(local.offset().local_minus_utc()),
                report.value.offset_seconds
            );
        }
    }
}
