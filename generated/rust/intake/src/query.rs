// generated from intake v1
// model digest 0dcbb44891d966a08164d0845cc89345f20e9da1a2bff9d8412d32181f207f99
// contract digest 03a4eba225331c590679bbbce8fd69d4e4f51fa6589d185caf24c7551cb770fd
// do not edit: regenerate with `ess synthesize --layout crate`

//! query — `intake.query`.
//!
//! Loom owns the system-query@1 protocol and its system.time.read host binding. The action takes an empty object, samples one instant from the host clock and derives UTC and local time with its numeric offset. No shell or network is used. A typed observation is bound to the case and intent revision; only the trusted verifier submits system_time observed evidence on intent. Model text, earlier cases and fabricated observations do not establish completion. The answer is rendered directly from that observation, without another model call. Canon's answered outcome is successful completion, exit zero. Clock and conversion failures never submit success evidence. CLI routing precedes resource initialization. System queries need no workspace, Git, Substrate or test runner and ignore supplied workspace paths without accessing them. Software changes retain workspace, confinement and merge authority requirements. Installed custom clock protocols may reuse the same host binding when they declare only the intent artifact and the read-only system.time.read action with the system_time evidence contract. Other definitions remain available to embedders but have an explicit missing-executor result in this CLI. Both context policies and measurements apply. Legacy remains default. Only local date/time queries are supported initially; an unrelated system question must not complete using a clock observation.
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
