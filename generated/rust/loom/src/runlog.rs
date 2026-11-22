// generated from loom v1
// contract digest ebc9d11ceaee6c0cdfda5635d3aa5e2bbf27b6d2cf06c94d23b72ed7fa7e8eeb
// do not edit: regenerate with `ess synthesize --layout crate`

//! Run log — `loom.runlog`.
//!
//! The run log of one case: one line per event, each carrying its sequence number, the SHA-256 of the line before it, the instant and the event; and the refusals a reader gives a log that was cut, has a line missing or out of order, or has a changed byte.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionFinished — `loom.runlog.ActionFinished`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFinished {
    /// `action` — `String`.
    pub action: String,
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `refusal` — `Optional<String>`.
    pub refusal: Option<String>,
    /// `blob_digest` — `Optional<String>`.
    pub blob_digest: Option<String>,
    /// `evidence` — `List<String>`.
    pub evidence: Vec<String>,
    /// `revisions` — `List<loom.runlog.RevisionMove>`.
    pub revisions: Vec<RevisionMove>,
}

/// ActionStarted — `loom.runlog.ActionStarted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionStarted {
    /// `action` — `String`.
    pub action: String,
}

/// AuthorityRecorded — `loom.runlog.AuthorityRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityRecorded {
    /// `capability` — `String`.
    pub capability: String,
    /// `decision` — `String`.
    pub decision: String,
    /// `principal` — `String`.
    pub principal: String,
}

/// CaseOpened — `loom.runlog.CaseOpened`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseOpened {
    /// `case` — `String`.
    pub case: String,
    /// `protocol` — `String`.
    pub protocol: String,
}

/// ChildClosed — `loom.runlog.ChildClosed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildClosed {
    /// `child` — `String`.
    pub child: String,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
}

/// ChildOpened — `loom.runlog.ChildOpened`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildOpened {
    /// `child` — `String`.
    pub child: String,
    /// `protocol` — `String`.
    pub protocol: String,
}

/// Discarded — `loom.runlog.Discarded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discarded {
    /// `action` — `String`.
    pub action: String,
    /// `reason` — `String`.
    pub reason: String,
}

/// Evaluated — `loom.runlog.Evaluated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluated {
    /// `case_revision` — `Integer`.
    pub case_revision: i64,
    /// `frontier` — `String`.
    pub frontier: String,
    /// `decision_digest` — `String`.
    pub decision_digest: String,
}

/// RevisionMove — `loom.runlog.RevisionMove`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionMove {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `from` — `Optional<String>`.
    pub from: Option<String>,
    /// `to` — `String`.
    pub to: String,
}

/// RunLogEvent — `loom.runlog.RunLogEvent`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogEvent {
    /// Tagged `ActionFinished` — `loom.runlog.ActionFinished`.
    ActionFinished(ActionFinished),
    /// Tagged `ActionStarted` — `loom.runlog.ActionStarted`.
    ActionStarted(ActionStarted),
    /// Tagged `AuthorityRecorded` — `loom.runlog.AuthorityRecorded`.
    AuthorityRecorded(AuthorityRecorded),
    /// Tagged `CaseOpened` — `loom.runlog.CaseOpened`.
    CaseOpened(CaseOpened),
    /// Tagged `ChildClosed` — `loom.runlog.ChildClosed`.
    ChildClosed(ChildClosed),
    /// Tagged `ChildOpened` — `loom.runlog.ChildOpened`.
    ChildOpened(ChildOpened),
    /// Tagged `Discarded` — `loom.runlog.Discarded`.
    Discarded(Discarded),
    /// Tagged `Evaluated` — `loom.runlog.Evaluated`.
    Evaluated(Evaluated),
    /// Tagged `Stopped` — `loom.runlog.Stopped`.
    Stopped(Stopped),
    /// Tagged `Terminated` — `loom.runlog.Terminated`.
    Terminated(Terminated),
}

/// RunLogLine — `loom.runlog.RunLogLine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLogLine {
    /// `seq` — `Integer`.
    pub seq: i64,
    /// `prev` — `Optional<String>`.
    pub prev: Option<String>,
    /// `at` — `Timestamp`.
    pub at: crate::primitives::Timestamp,
    /// `event` — `loom.runlog.RunLogEvent`.
    pub event: RunLogEvent,
}

/// RunLogRefusal — `loom.runlog.RunLogRefusal`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunLogRefusal {
    /// `CutLine`.
    CutLine,
    /// `BrokenChain`.
    BrokenChain,
    /// `SequenceGap`.
    SequenceGap,
}

/// RunLogRefused — `loom.runlog.RunLogRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLogRefused {
    /// `refusal` — `loom.runlog.RunLogRefusal`.
    pub refusal: RunLogRefusal,
    /// `line` — `Integer`.
    pub line: i64,
    /// `detail` — `String`.
    pub detail: String,
}

/// Stopped — `loom.runlog.Stopped`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    /// `stop` — `String`.
    pub stop: String,
    /// `next` — `List<String>`.
    pub next: Vec<String>,
}

/// Terminated — `loom.runlog.Terminated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminated {
    /// `reason` — `String`.
    pub reason: String,
}
