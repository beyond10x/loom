// generated from intake v1
// model digest d8c32bcb041f8a56eb453f084d0015a73ed14ab9d01ec38dae4f55e65396160a
// contract digest d0764514a3e03d525650b901f3321835fba82b74a89ce72eb11c673bf3b3eb4e
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
