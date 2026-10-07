// generated from intake v1
// model digest 27eba34a158e1027a48b4584d4a9cfd7db24040200cceaab4f1148ba47ce865e
// contract digest 6db3749de79840686760677941a2171cf13badb3ab1d8d4190151e3870d23df9
// do not edit: regenerate with `ess synthesize --layout crate`

//! results — `intake.results`.
//!
//! Immutable UTF-8 observations retained by one run's briefing. Model-facing references select data before an ordinary action proposal reaches Commission admission; they never grant an action, carry authority, execute expressions, or stand in for verified evidence.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// Capture — `intake.results.Capture`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
    /// `Complete`.
    Complete,
    /// `Partial`.
    Partial,
}

/// Rendering — `intake.results.Rendering`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendering {
    /// `Text`.
    Text,
    /// `Json`.
    Json,
}

/// ResultReference — `intake.results.ResultReference`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultReference {
    /// `result` — `String`.
    pub result: String,
    /// `sha256` — `String`.
    pub sha256: String,
    /// `select` — `intake.results.ResultSelector`.
    pub select: ResultSelector,
    /// `rendering` — `intake.results.Rendering`.
    pub rendering: Rendering,
}

/// ResultSelector — `intake.results.ResultSelector`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSelector {
    /// `kind` — `intake.results.SelectionKind`.
    pub kind: SelectionKind,
    /// `start` — `Optional<Integer>`.
    pub start: Option<i64>,
    /// `end` — `Optional<Integer>`.
    pub end: Option<i64>,
    /// `pointer` — `Optional<String>`.
    pub pointer: Option<String>,
}

/// SelectionKind — `intake.results.SelectionKind`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// `Whole`.
    Whole,
    /// `Bytes`.
    Bytes,
    /// `Lines`.
    Lines,
    /// `JsonPointer`.
    JsonPointer,
}

/// StoredResult — `intake.results.StoredResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredResult {
    /// `result_id` — `String`.
    pub result_id: String,
    /// `sha256` — `String`.
    pub sha256: String,
    /// `origin` — `String`.
    pub origin: String,
    /// `capture` — `intake.results.Capture`.
    pub capture: Capture,
    /// `utf8_bytes` — `Integer`.
    pub utf8_bytes: i64,
}
