//! The compaction contract of a governed run ([`Loom::run_loop`]).
//!
//! The ported loop compacts (`AgentLoop::compact_run` in [`crate::harness::turn_loop`]). Before
//! every request it measures the conversation. With a declared context window, past 80 % of it
//! ([`COMPACTION_TRIGGER_PERCENT`](crate::harness::turn_loop::COMPACTION_TRIGGER_PERCENT)), by the
//! provider's last reported input or an estimate of the conversation, whichever is larger, it
//! elides old tool results and, where that is not enough, spends one summary request folding the
//! earlier conversation into one item, aiming at 50 % of the window
//! ([`COMPACTION_TARGET_PERCENT`](crate::harness::turn_loop::COMPACTION_TARGET_PERCENT)). A
//! summary no shorter than the items it would replace is not kept: those items are elided instead,
//! behind one item beginning with
//! [`ELISION_MARKER`](crate::harness::turn_loop::ELISION_MARKER), so a compaction never leaves the
//! conversation larger than it found it. Without a declared window it keeps the byte rule
//! ([`MAX_CONVERSATION_BYTES`](crate::harness::turn_loop::MAX_CONVERSATION_BYTES),
//! [`COMPACTED_TARGET_BYTES`](crate::harness::turn_loop::COMPACTED_TARGET_BYTES)) and only elides.
//! What a governed run adds:
//!
//! - **Recorded on the session.** Each compaction the loop reports ([`LoopEvent::Compacted`]) is
//!   recorded on the run's session (`loom.run.RecordCompaction`, [`Loom::compactions`]) with the
//!   usage the endpoint reported for its summary request ([`reported_usage`]), never an estimate. A
//!   compaction that made no summary request, or whose provider reported nothing, records no usage.
//!   The session file ([`crate::session::SessionFile`]) does not carry compactions: its format and
//!   version are unchanged.
//! - **No stale catalogue.** Compaction rewrites conversation items only. The tool list of the
//!   request after it is projected from the frontier the governor issues for that request, as for
//!   every turn ([`crate::harness::governed`]).
//! - **Nothing the model wrote is promoted.** Compaction never rewrites instruction text: the
//!   request after it carries the run's standing instruction unchanged. The summary is content the
//!   model wrote and stays conversation content: one item, beginning with
//!   [`SUMMARY_MARKER`](crate::harness::turn_loop::SUMMARY_MARKER), in place of the items it folds.
//!   It is not recorded as a turn of the session.
#![forbid(unsafe_code)]

use crate::harness::turn_loop::{LoopEvent, LoopSink};
use crate::harness::wire::Usage;
use crate::model::run::obligations::RecordCompactionBehavior;
use crate::model::run::{CompactionId, RecordCompaction, ReportedUsage, SessionId};
use crate::{Loom, run_id};

/// The usage a provider reported, as the run model records it: every figure as reported, and a
/// cache-write figure the provider did not report still absent. A count past `i64::MAX` is
/// recorded as `i64::MAX`.
#[must_use]
pub fn reported_usage(usage: &Usage) -> ReportedUsage {
    let count = |tokens: u64| i64::try_from(tokens).unwrap_or(i64::MAX);
    ReportedUsage {
        model: usage.model.clone(),
        input_tokens: count(usage.input_tokens),
        output_tokens: count(usage.output_tokens),
        cached_input_tokens: count(usage.cached_input_tokens),
        cache_creation_input_tokens: usage.cache_creation_input_tokens.map(count),
    }
}

/// The [`LoopSink`] a governed run hands the loop: the caller's sink, with each compaction the
/// loop reports recorded on the run's session before the event is passed on. Only the run's own
/// compactions count; a delegate's would arrive nested, and a governed run runs none.
pub(crate) struct CompactionRecorder<'s, 'r, S, G, V> {
    pub(crate) sink: &'s mut dyn LoopSink,
    pub(crate) loom: &'r Loom<S, G, V>,
    pub(crate) session: &'r SessionId,
}

impl<S, G, V> LoopSink for CompactionRecorder<'_, '_, S, G, V> {
    fn emit(&mut self, event: LoopEvent) {
        if let LoopEvent::Compacted { usage, .. } = &event {
            self.loom.record_compaction(self.session, usage.as_ref());
        }
        self.sink.emit(event);
    }
}

impl<S, G, V> Loom<S, G, V> {
    /// Records one compaction on `session` (`loom.run.RecordCompaction`), priced at `usage`. Its
    /// id is the session's and its number in the session, from 1.
    fn record_compaction(&self, session: &SessionId, usage: Option<&Usage>) {
        let mut record = self.turn_record();
        let held = record.compactions_of(session);
        let number = u64::try_from(held).unwrap_or(u64::MAX).saturating_add(1);
        // The session was opened `Active` for this run and nothing files it while the loop runs,
        // so the compaction is `recorded`.
        let _ = record.record_compaction(RecordCompaction {
            compaction_id: CompactionId(run_id("compaction", &session.0.0, number)),
            session_id: session.clone(),
            usage: usage.map(reported_usage),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_usage_keeps_every_figure_and_an_absent_cache_write_absent() {
        let usage = Usage {
            model: "m".to_owned(),
            input_tokens: 11,
            output_tokens: 7,
            cached_input_tokens: 4,
            cache_creation_input_tokens: None,
        };
        assert_eq!(
            reported_usage(&usage),
            ReportedUsage {
                model: "m".to_owned(),
                input_tokens: 11,
                output_tokens: 7,
                cached_input_tokens: 4,
                cache_creation_input_tokens: None,
            }
        );
        let written = Usage {
            cache_creation_input_tokens: Some(0),
            ..usage
        };
        assert_eq!(
            reported_usage(&written).cache_creation_input_tokens,
            Some(0),
            "reported as zero is not unreported"
        );
    }
}
