// generated from intake v1
// model digest cf1fbbfc1206e350609f5f4f3522b47696f03a8049cc32a47b5ed52b150d1d84
// contract digest 7dd547604a8ca8784e13b80a1979b25542e31ca93b347da47fcc3787eeb2950c
// do not edit: regenerate with `ess synthesize --layout crate`

//! query — `intake.query`.
//!
//! Loom owns the system-query@1 protocol and its system.time.read host binding. The action takes an empty object, samples one instant from the host clock and derives UTC and local time with its numeric offset. No shell or network is used. A typed observation is bound to the case and intent revision; only the trusted verifier submits system_time observed evidence on intent. Model text, earlier cases and fabricated observations do not establish completion. The answer is rendered directly from that observation, without another model call. Canon's answered outcome is successful completion, exit zero. Clock and conversion failures never submit success evidence. CLI routing precedes resource initialization. System queries need no workspace, Git, Substrate or test runner and ignore supplied workspace paths without accessing them. Software changes retain workspace, confinement and merge authority requirements. Installed custom clock protocols may reuse the same host binding when they declare only the intent artifact and the read-only system.time.read action with the system_time evidence contract. Other definitions remain available to embedders and run in this CLI only through commands the operator binds to their actions (intake.bindings); without one they have an explicit missing-executor result. Both context policies and measurements apply. Legacy remains default. Only local date/time queries are supported initially; an unrelated system question must not complete using a clock observation.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// TimeObservation — `intake.query.TimeObservation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeObservation {
    /// `case_id` — `String`.
    pub case_id: String,
    /// `intent_revision` — `String`.
    pub intent_revision: String,
    /// `utc` — `String`.
    pub utc: String,
    /// `local` — `String`.
    pub local: String,
    /// `offset_seconds` — `Integer`.
    pub offset_seconds: i64,
}
