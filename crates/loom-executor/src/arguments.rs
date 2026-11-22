//! Argument generation for the selected action only (Atlas ADR 0073, step 1;
//! `docs/contracts/loom-action-selection.md`).
//!
//! A generator is handed the argument context and the one catalogue entry the selection names,
//! never the rest of the catalogue, and returns a JSON value. That value becomes the proposed
//! action's `ProposedActionArguments`. Before arguments are generated, the request for them is
//! recorded against the selection it serves (`loom.run.RequestArguments`, outcome `requested`),
//! in the [`RequestRecord`].
//!
//! The generated arguments are checked against the argument schema the protocol the host loaded
//! declares for the action (ADR 0073, step 2;
//! `architecture-decision-record:action-argument-schema-from-protocol`), when the host hands Loom
//! the schemas of that protocol ([`ArgumentSchemas`], [`crate::Loom::with_argument_schemas`]):
//! after generation and before revalidation. The check is Canon's
//! (`b10x_canon::validate::check_arguments`), called through [`check`]; an action whose protocol
//! declares no schema takes only `{}`. Its outcome is recorded on the argument request
//! (`loom.run.CheckArguments`), and only a `Matched` request's selection is revalidated and
//! proposed. The schema comes from the host's lookup alone, never from the model or the request.
//!
//! The check terminates on any schema and any value: Canon walks a schema with a stack of its own,
//! refuses one nested past 32 levels as outside its dialect, and descends into a value only where
//! the schema describes it, so its work is linear in the value's size and its depth at most the
//! schema's. A value is read for it with `serde_json`, which refuses one nested past 128 levels.
//! A mismatch names the failing JSON pointer and keyword and Canon's one-line reason, each cut to
//! at most [`QUOTED_BYTES`] bytes, so it never carries a large value whole.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use b10x_canon::validate::{ArgumentsRefused, check_arguments};
use b10x_loom_commission::model::json::{Value, push_value};

use crate::model::behaviour::{
    ArgumentRequestStorage, ExternalCommand, Generated, SelectionRecordStorage, SelectionStorage,
    TryContext, unmet_context,
};
use crate::model::obligation::UnmetObligation;
use crate::model::run::obligations::{
    CheckArgumentsBehavior, OverruleSelectionBehavior, RecordSelectionBehavior,
    RequestArgumentsBehavior,
};
use crate::model::run::{
    AnyArgumentRequest, ArgumentRequest, ArgumentRequestData, ArgumentRequestId,
    ArgumentRequestSnapshot, ArgumentsRequested, CatalogueEntry, CatalogueId, CheckArguments,
    CheckArgumentsOutcome, OverruleSelection, OverruleSelectionOutcome, RecordSelection,
    RecordSelectionOutcome, RequestArguments, RequestArgumentsOutcome, RevalidateSelectionOutcome,
    SelectionId, SelectionNotFound, SelectionNotSelected, SelectionRecordId,
    SelectionRecordSnapshot, SelectionSnapshot, SelectionState,
};

/// What a generator is told besides the selected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentContext {
    /// The instruction the run is working on.
    pub prompt: String,
}

/// Generates the arguments of one selected action.
pub trait ArgumentGenerator {
    /// The arguments for `entry`, the catalogue entry the selection names, in `context`. An `Err`
    /// is a generator that could not answer, for the reason given; Loom answers it with
    /// `Suspended(ExternalAvailability)` carrying the reason.
    fn generate(&self, context: &ArgumentContext, entry: &CatalogueEntry) -> Result<Value, String>;
}

/// Bootstrap argument generator: always the empty object.
#[derive(Debug, Default)]
pub struct EmptyObjectArguments;

impl ArgumentGenerator for EmptyObjectArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        Ok(Value::Object(Vec::new()))
    }
}

/// How many bytes of a pointer or a reason a mismatch quotes.
pub const QUOTED_BYTES: usize = 120;

/// The argument schemas of the protocol a host runs, by action id: what the host hands
/// [`crate::Loom::with_argument_schemas`], built from the protocol entry it runs
/// (`loom_protocols::ProtocolEntry::argument_schemas`). An action with `None`, or one the lookup
/// does not list, declares no schema and takes only `{}`. A later pair for an id replaces an
/// earlier one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArgumentSchemas {
    schemas: BTreeMap<String, Option<serde_json::Value>>,
}

impl ArgumentSchemas {
    /// The schema the protocol declares for `action`; `None` when it declares none, or does not
    /// list the action.
    #[must_use]
    pub fn schema(&self, action: &str) -> Option<&serde_json::Value> {
        self.schemas.get(action).and_then(Option::as_ref)
    }
}

impl FromIterator<(String, Option<serde_json::Value>)> for ArgumentSchemas {
    fn from_iter<I: IntoIterator<Item = (String, Option<serde_json::Value>)>>(pairs: I) -> Self {
        Self {
            schemas: pairs.into_iter().collect(),
        }
    }
}

/// Why [`check`] refused one call's arguments. Every pointer and reason is cut to at most
/// [`QUOTED_BYTES`] bytes, its control characters escaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentMismatch {
    /// The arguments do not satisfy the schema: Canon's first refusal, in document order.
    Refused {
        /// The RFC 6901 JSON Pointer into the arguments; empty for the whole document.
        pointer: String,
        /// The dialect keyword that refused.
        keyword: &'static str,
        /// Canon's reason.
        reason: String,
    },
    /// The schema is outside Canon's argument-schema dialect: its first offence. No argument was
    /// checked.
    OutsideDialect {
        /// The JSON Pointer inside the schema.
        pointer: String,
        /// Canon's reason.
        reason: String,
    },
    /// An object of the arguments names a member twice. A reader keeps one of the two and an
    /// effect port may read the other, so no schema is asked.
    DuplicateMember {
        /// The JSON Pointer of the member.
        pointer: String,
    },
    /// The arguments are no JSON document `serde_json` reads (a number it cannot hold, or a value
    /// nested past its limit).
    Unreadable {
        /// `serde_json`'s reason.
        reason: String,
    },
}

impl fmt::Display for ArgumentMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                pointer,
                keyword,
                reason,
            } => write!(
                f,
                "arguments at `{pointer}` refused by `{keyword}`: {reason}"
            ),
            Self::OutsideDialect { pointer, reason } => write!(
                f,
                "the argument schema is outside Canon's argument-schema dialect at `{pointer}`: \
                 {reason}; no argument was checked"
            ),
            Self::DuplicateMember { pointer } => {
                write!(f, "arguments at `{pointer}` name a member twice")
            }
            Self::Unreadable { reason } => {
                write!(f, "the arguments do not read as JSON: {reason}")
            }
        }
    }
}

impl std::error::Error for ArgumentMismatch {}

/// Checks one call's `arguments` against `schema`, the argument schema a protocol declares for
/// the action (`None`: it declares none, so only `{}` passes), with Canon's
/// `b10x_canon::validate::check_arguments`. Pure. Canon's first refusal is the mismatch.
///
/// # Errors
///
/// [`ArgumentMismatch::Refused`] for arguments the schema refuses,
/// [`ArgumentMismatch::OutsideDialect`] for a schema outside Canon's dialect.
pub fn check(
    schema: Option<&serde_json::Value>,
    arguments: &serde_json::Value,
) -> Result<(), ArgumentMismatch> {
    match check_arguments(schema, arguments) {
        Ok(()) => Ok(()),
        Err(ArgumentsRefused::Arguments(refusals)) => Err(refusals.first().map_or_else(
            || ArgumentMismatch::Refused {
                pointer: String::new(),
                keyword: "type",
                reason: "Canon refused the arguments and named no refusal".to_owned(),
            },
            |first| ArgumentMismatch::Refused {
                pointer: quoted(&first.pointer),
                keyword: first.keyword,
                reason: quoted(&first.reason),
            },
        )),
        Err(ArgumentsRefused::Schema(offences)) => {
            let (pointer, reason) = offences.first().map_or_else(
                || (String::new(), "Canon named no offence".to_owned()),
                |(pointer, reason)| (quoted(pointer), quoted(reason)),
            );
            Err(ArgumentMismatch::OutsideDialect { pointer, reason })
        }
    }
}

/// [`check`] of arguments as Loom holds them: refused when an object names a member twice, else
/// written with Commission's `json::push_value` and read with `serde_json::from_str`, so Loom
/// rounds no number of its own, then checked.
///
/// # Errors
///
/// [`ArgumentMismatch::DuplicateMember`], [`ArgumentMismatch::Unreadable`], or what [`check`]
/// returns.
pub fn check_value(
    schema: Option<&serde_json::Value>,
    arguments: &Value,
) -> Result<(), ArgumentMismatch> {
    if let Some(pointer) = duplicate_member(arguments) {
        return Err(ArgumentMismatch::DuplicateMember {
            pointer: quoted(&pointer),
        });
    }
    let mut text = String::new();
    push_value(&mut text, arguments);
    let read: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| ArgumentMismatch::Unreadable {
            reason: quoted(&error.to_string()),
        })?;
    check(schema, &read)
}

/// The pointer of the first member an object of `value` names twice, walked with a stack of its
/// own, so a deep value cannot overflow it.
fn duplicate_member(value: &Value) -> Option<String> {
    let mut stack = vec![(String::new(), value)];
    while let Some((pointer, value)) = stack.pop() {
        let token = |key: &str| key.replace('~', "~0").replace('/', "~1");
        match value {
            Value::Object(members) => {
                let mut named = BTreeSet::new();
                for (key, _) in members {
                    if !named.insert(key.as_str()) {
                        return Some(format!("{pointer}/{}", token(key)));
                    }
                }
                for (key, member) in members.iter().rev() {
                    stack.push((format!("{pointer}/{}", token(key)), member));
                }
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate().rev() {
                    stack.push((format!("{pointer}/{index}"), item));
                }
            }
            _ => {}
        }
    }
    None
}

/// `text` with its control characters escaped, cut to at most [`QUOTED_BYTES`] bytes on a
/// character boundary and marked `…` when cut.
fn quoted(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|character| {
            let escape = character.is_control();
            let shown: Vec<char> = if escape {
                character.escape_default().collect()
            } else {
                vec![character]
            };
            shown
        })
        .collect();
    if escaped.len() <= QUOTED_BYTES {
        return escaped;
    }
    let mut end = QUOTED_BYTES - '…'.len_utf8();
    while !escaped.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &escaped[..end])
}

/// The selections Loom made, the telemetry of each (`loom.run.SelectionRecord`), the argument
/// requests that serve them and the outcome of every revalidation of them
/// ([`crate::revalidation`]), in the order they were recorded. A selection, selection record or
/// argument request stored under an identity already held replaces it.
#[derive(Debug, Default)]
pub struct RequestRecord {
    selections: Vec<SelectionSnapshot>,
    selection_records: Vec<SelectionRecordSnapshot>,
    argument_requests: Vec<ArgumentRequestSnapshot>,
    revalidations: Vec<RevalidateSelectionOutcome>,
    argument_checks: Vec<CheckArgumentsOutcome>,
    verdicts: Vec<(ArgumentRequestId, ArgumentVerdict)>,
}

impl RequestRecord {
    /// Every recorded selection.
    #[must_use]
    pub fn selections(&self) -> &[SelectionSnapshot] {
        &self.selections
    }

    /// Every recorded selection's telemetry, in the order recorded. Never evidence (Atlas ADR
    /// 0074).
    #[must_use]
    pub fn selection_records(&self) -> &[SelectionRecordSnapshot] {
        &self.selection_records
    }

    /// Every recorded argument request.
    #[must_use]
    pub fn argument_requests(&self) -> &[ArgumentRequestSnapshot] {
        &self.argument_requests
    }

    /// The outcome of every revalidation, in the order they were made.
    #[must_use]
    pub fn revalidations(&self) -> &[RevalidateSelectionOutcome] {
        &self.revalidations
    }

    /// Records the outcome of one revalidation.
    pub(crate) fn record_revalidation(&mut self, outcome: RevalidateSelectionOutcome) {
        self.revalidations.push(outcome);
    }
}

impl SelectionStorage for RequestRecord {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        self.selections
            .iter()
            .find(|held| &held.data.selection_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        match self
            .selections
            .iter_mut()
            .find(|held| held.data.selection_id == snapshot.data.selection_id)
        {
            Some(held) => *held = snapshot,
            None => self.selections.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &SelectionId) {
        self.selections
            .retain(|held| &held.data.selection_id != identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        self.selections.clone()
    }
}

/// Why [`RequestRecord::overrule`] overruled nothing. Every refusal but [`OverruleRefused::Unmet`]
/// is the host's, made before the generated behaviour runs, and leaves the record as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverruleRefused {
    /// The record holds no selection under the replacement's id, which this names.
    ReplacementNotFound(SelectionNotFound),
    /// The replacement is the selection to be overruled, whose id this names: no selection
    /// replaces itself.
    ReplacementIsTheSelection(SelectionId),
    /// The replacement, which this names, is no longer `Selected`: an `Overruled`, `Admitted` or
    /// `Refused` selection replaces nothing.
    ReplacementNotSelected(SelectionNotSelected),
    /// The replacement was made from another catalogue than the selection it would overrule, so
    /// it is not that selection's turn's: names the replacement and the catalogue it was made from.
    ReplacementFromAnotherCatalogue {
        /// The replacement's id.
        replacement_id: SelectionId,
        /// The catalogue the replacement was made from.
        catalogue_id: CatalogueId,
    },
    /// The generated behaviour refused the request as one the specification declares no outcome
    /// for.
    Unmet(UnmetObligation),
}

impl RequestRecord {
    /// `loom.run.OverruleSelection` as Loom records a confidence fallback: refused, naming the
    /// replacement, when the record holds no selection under `input.replacement_id`, when the
    /// replacement is the selection itself, when it is no longer `Selected`, or when it was made
    /// from another catalogue than a held selection it would overrule; otherwise the generated
    /// behaviour's outcome ([`OverruleSelectionBehavior`]). A refusal records nothing.
    ///
    /// ESS refuses a `when_related` guard on the replacement beside `wrong_state`
    /// (`ESS-COMMAND-004`), so the specification's behaviour, which the conformance suite holds,
    /// overrules whatever replacement it is named; these checks are the host's, made before it.
    pub fn overrule(
        &mut self,
        input: OverruleSelection,
    ) -> Result<OverruleSelectionOutcome, OverruleRefused> {
        let Some(replacement) = SelectionStorage::get(self, &input.replacement_id) else {
            return Err(OverruleRefused::ReplacementNotFound(SelectionNotFound {
                selection_id: input.replacement_id,
            }));
        };
        if input.replacement_id == input.selection_id {
            return Err(OverruleRefused::ReplacementIsTheSelection(
                input.replacement_id,
            ));
        }
        if replacement.state != SelectionState::Selected {
            return Err(OverruleRefused::ReplacementNotSelected(
                SelectionNotSelected {
                    selection_id: input.replacement_id,
                },
            ));
        }
        if let Some(overruled) = SelectionStorage::get(self, &input.selection_id)
            && overruled.data.catalogue_id != replacement.data.catalogue_id
        {
            return Err(OverruleRefused::ReplacementFromAnotherCatalogue {
                replacement_id: input.replacement_id,
                catalogue_id: replacement.data.catalogue_id,
            });
        }
        self.overrule_selection(input)
            .map_err(OverruleRefused::Unmet)
    }
}

/// The record, as the storage port of the generated `OverruleSelection`.
struct Overruling<'r>(&'r mut RequestRecord);

impl SelectionStorage for Overruling<'_> {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        SelectionStorage::get(self.0, identity)
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        SelectionStorage::put(self.0, snapshot);
    }

    fn delete(&mut self, identity: &SelectionId) {
        SelectionStorage::delete(self.0, identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        SelectionStorage::list(self.0)
    }
}

/// `loom.run.OverruleSelection`, generated, over this record: a `Selected` selection is moved to
/// `Overruled` naming the replacement in `replaced_by` (`overruled`); any other is refused
/// `wrong-state`. It does not check that the replacement is held: [`RequestRecord::overrule`] does.
impl OverruleSelectionBehavior for RequestRecord {
    fn overrule_selection(
        &mut self,
        input: OverruleSelection,
    ) -> Result<OverruleSelectionOutcome, UnmetObligation> {
        Generated::new(Overruling(self)).overrule_selection(input)
    }
}

/// The record, as the storage ports of the generated `RecordSelection`: the selections a record
/// names, and the records. Only through it is the record a store of selection records,
/// so a caller holding both storage traits never finds two `put`s on [`RequestRecord`].
struct Telemetry<'r>(&'r mut RequestRecord);

impl SelectionStorage for Telemetry<'_> {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        SelectionStorage::get(self.0, identity)
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        SelectionStorage::put(self.0, snapshot);
    }

    fn delete(&mut self, identity: &SelectionId) {
        SelectionStorage::delete(self.0, identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        SelectionStorage::list(self.0)
    }
}

impl SelectionRecordStorage for Telemetry<'_> {
    fn get(&self, identity: &SelectionRecordId) -> Option<SelectionRecordSnapshot> {
        self.0
            .selection_records
            .iter()
            .find(|held| &held.data.selection_record_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: SelectionRecordSnapshot) {
        let records = &mut self.0.selection_records;
        match records
            .iter_mut()
            .find(|held| held.data.selection_record_id == snapshot.data.selection_record_id)
        {
            Some(held) => *held = snapshot,
            None => records.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &SelectionRecordId) {
        self.0
            .selection_records
            .retain(|held| &held.data.selection_record_id != identity);
    }

    fn list(&self) -> Vec<SelectionRecordSnapshot> {
        self.0.selection_records.clone()
    }
}

/// `loom.run.RecordSelection`, generated, over this record: a record identity is recorded once
/// (`record-exists`), only for a selection the record holds (`selection-unknown`); otherwise the
/// `loom.run.SelectionRecord` is stored `Recorded` (`recorded`).
impl RecordSelectionBehavior for RequestRecord {
    fn record_selection(
        &mut self,
        input: RecordSelection,
    ) -> Result<RecordSelectionOutcome, UnmetObligation> {
        Generated::new(Telemetry(self)).record_selection(input)
    }
}

/// `loom.run.RequestArguments`: refused for a selection the record does not hold
/// (`selection-unknown`) or one no longer `Selected` (`selection-not-selected`); otherwise the
/// `loom.run.ArgumentRequest` is recorded against the selection (`requested`).
impl RequestArgumentsBehavior for RequestRecord {
    fn request_arguments(
        &mut self,
        input: RequestArguments,
    ) -> Result<RequestArgumentsOutcome, UnmetObligation> {
        let Some(selection) = SelectionStorage::get(self, &input.selection_id) else {
            return Ok(RequestArgumentsOutcome::SelectionUnknown {
                error: SelectionNotFound {
                    selection_id: input.selection_id,
                },
            });
        };
        if selection.state != SelectionState::Selected {
            return Ok(RequestArgumentsOutcome::SelectionNotSelected {
                error: SelectionNotSelected {
                    selection_id: input.selection_id,
                },
            });
        }
        let snapshot = AnyArgumentRequest::Requested(ArgumentRequest::new(ArgumentRequestData {
            argument_request_id: input.argument_request_id.clone(),
            selection_id: input.selection_id.clone(),
        }))
        .snapshot();
        match self
            .argument_requests
            .iter_mut()
            .find(|held| held.data.argument_request_id == snapshot.data.argument_request_id)
        {
            Some(held) => *held = snapshot,
            None => self.argument_requests.push(snapshot),
        }
        Ok(RequestArgumentsOutcome::Requested {
            arguments_requested: ArgumentsRequested {
                argument_request_id: input.argument_request_id,
                selection_id: input.selection_id,
            },
        })
    }
}

/// What Loom found when it checked one argument request's arguments, staged on the record for
/// `loom.run.CheckArguments` ([`RequestRecord::stage_verdict`]) to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentVerdict {
    /// The arguments satisfy the declared schema, or are `{}` for an action that declares none.
    Matched,
    /// The arguments were refused.
    Mismatched {
        /// Whether the protocol declares a schema for the action.
        declared: bool,
        /// Why, naming the action, the pointer and the keyword; never the whole value.
        reason: String,
    },
}

impl RequestRecord {
    /// The outcome of every `loom.run.CheckArguments`, in the order they were made.
    #[must_use]
    pub fn argument_checks(&self) -> &[CheckArgumentsOutcome] {
        &self.argument_checks
    }

    /// Stages `verdict` for the next `loom.run.CheckArguments` of `argument_request_id`, replacing
    /// one staged before. The check takes it; a check with none staged is unmet and records
    /// nothing.
    pub fn stage_verdict(
        &mut self,
        argument_request_id: ArgumentRequestId,
        verdict: ArgumentVerdict,
    ) {
        self.verdicts
            .retain(|(staged, _)| *staged != argument_request_id);
        self.verdicts.push((argument_request_id, verdict));
    }
}

/// The record and the verdict staged for one check, as the storage and context ports of the
/// generated `CheckArguments`.
struct Checking<'r> {
    record: &'r mut RequestRecord,
    verdict: Option<ArgumentVerdict>,
}

impl ArgumentRequestStorage for Checking<'_> {
    fn get(&self, identity: &ArgumentRequestId) -> Option<ArgumentRequestSnapshot> {
        self.record
            .argument_requests
            .iter()
            .find(|held| &held.data.argument_request_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: ArgumentRequestSnapshot) {
        let requests = &mut self.record.argument_requests;
        match requests
            .iter_mut()
            .find(|held| held.data.argument_request_id == snapshot.data.argument_request_id)
        {
            Some(held) => *held = snapshot,
            None => requests.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &ArgumentRequestId) {
        self.record
            .argument_requests
            .retain(|held| &held.data.argument_request_id != identity);
    }

    fn list(&self) -> Vec<ArgumentRequestSnapshot> {
        self.record.argument_requests.clone()
    }
}

impl TryContext for Checking<'_> {
    /// The staged mismatch's reason; unanswered for a match or with nothing staged.
    fn try_generate_string(&mut self) -> Result<String, UnmetObligation> {
        match &self.verdict {
            Some(ArgumentVerdict::Mismatched { reason, .. }) => Ok(reason.clone()),
            _ => Err(unmet_context("loom.run.CheckArguments")),
        }
    }

    /// `undeclared-not-empty` is taken for a staged mismatch of an action that declares no
    /// schema, `mismatched` for one that declares one; with nothing staged, or for any other
    /// command, the branch is unanswered and the check refused rather than decided by a default.
    fn try_external(
        &mut self,
        command: ExternalCommand<'_>,
        outcome: &'static str,
    ) -> Result<bool, UnmetObligation> {
        let ExternalCommand::LoomRunCheckArguments(_) = command else {
            return Err(unmet_context("loom.run.CheckArguments"));
        };
        let declared = match &self.verdict {
            None => return Err(unmet_context("loom.run.CheckArguments")),
            Some(ArgumentVerdict::Matched) => return Ok(false),
            Some(ArgumentVerdict::Mismatched { declared, .. }) => *declared,
        };
        match outcome {
            "undeclared-not-empty" => Ok(!declared),
            "mismatched" => Ok(declared),
            _ => Err(unmet_context("loom.run.CheckArguments")),
        }
    }
}

/// `loom.run.CheckArguments`, generated, over the requests this record holds and the verdict
/// staged for the one named ([`RequestRecord::stage_verdict`]): a mismatch moves a `Requested`
/// request to `Mismatched` (`undeclared-not-empty` when the protocol declares no schema for the
/// action, `mismatched` when it declares one), a match to `Matched`; a request the record does
/// not hold, or one no longer `Requested`, is refused `wrong-state`. Both refusals are
/// `external:`: the record holds neither the schema nor the arguments, and the verdict is Canon's
/// ([`check_value`]). With nothing staged the check is unmet and records nothing; every other
/// outcome is recorded ([`RequestRecord::argument_checks`]).
impl CheckArgumentsBehavior for RequestRecord {
    fn check_arguments(
        &mut self,
        input: CheckArguments,
    ) -> Result<CheckArgumentsOutcome, UnmetObligation> {
        let verdict = self
            .verdicts
            .iter()
            .position(|(staged, _)| *staged == input.argument_request_id)
            .map(|at| self.verdicts.remove(at).1);
        let outcome = Generated::new(Checking {
            record: self,
            verdict,
        })
        .check_arguments(input)?;
        self.argument_checks.push(outcome.clone());
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::primitives::Uuid;
    use crate::model::run::{
        AnySelection, ArgumentRequestId, ArgumentRequestState, CatalogueId, Selection,
        SelectionData, SelectionStrategy,
    };

    fn uuid(n: u32) -> Uuid {
        Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
    }

    #[test]
    fn check_passes_canons_refusal_through_with_its_pointer_and_keyword() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"limit": {"type": "integer"}}
        });

        let refused = check(Some(&schema), &serde_json::json!({"limit": "20"}));

        let Err(ArgumentMismatch::Refused {
            pointer, keyword, ..
        }) = &refused
        else {
            panic!("not a refusal: {refused:?}");
        };
        assert_eq!(
            (pointer.as_str(), *keyword),
            ("/limit", "type"),
            "{refused:?}"
        );
        assert_eq!(
            check(Some(&schema), &serde_json::json!({"limit": 20})),
            Ok(())
        );
        // Canon's order: a location's own refusals (`required` at ``) before its members'.
        let required = serde_json::json!({
            "type": "object",
            "properties": {"limit": {"type": "integer"}, "name": {"type": "string"}},
            "required": ["name"]
        });
        assert!(
            matches!(
                check(Some(&required), &serde_json::json!({"limit": "20"})),
                Err(ArgumentMismatch::Refused { ref pointer, keyword: "required", .. }) if pointer.is_empty()
            ),
            "the first pair is the whole document's"
        );
    }

    #[test]
    fn a_schema_outside_canons_dialect_is_a_mismatch_naming_the_dialect() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"target": {"$ref": "#/$defs/target"}}
        });

        // The arguments would satisfy any schema of `target`: none is checked.
        let refused = check(Some(&schema), &serde_json::json!({"target": 1}));

        let Err(mismatch @ ArgumentMismatch::OutsideDialect { pointer, .. }) = &refused else {
            panic!("not outside the dialect: {refused:?}");
        };
        assert!(pointer.starts_with("/properties/target"), "{pointer}");
        assert!(
            mismatch
                .to_string()
                .contains("outside Canon's argument-schema dialect"),
            "{mismatch}"
        );
    }

    #[test]
    fn a_mismatch_quotes_at_most_120_bytes_of_the_value() {
        let long = "x".repeat(4096);
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"note": {"type": "string", "maxLength": 1}},
            "additionalProperties": false
        });

        for (label, arguments) in [
            ("a long string", serde_json::json!({ "note": long })),
            ("a long member name", serde_json::json!({ long.clone(): 1 })),
        ] {
            let refused = check(Some(&schema), &arguments).expect_err(label);
            let ArgumentMismatch::Refused {
                pointer, reason, ..
            } = &refused
            else {
                panic!("{label}: not a refusal: {refused:?}");
            };
            assert!(pointer.len() <= QUOTED_BYTES, "{label}: {pointer}");
            assert!(reason.len() <= QUOTED_BYTES, "{label}: {reason}");
            let shown = refused.to_string();
            assert!(
                !shown.contains(&long[..=QUOTED_BYTES]),
                "{label}: more than {QUOTED_BYTES} bytes of the value quoted"
            );
            assert!(shown.len() <= 3 * QUOTED_BYTES, "{label}: {}", shown.len());
        }
    }

    #[test]
    fn a_number_past_a_double_is_checked_as_this_build_holds_it() {
        let held: serde_json::Value = serde_json::from_str("9007199254740992.5").expect("a number");
        let arbitrary_precision =
            serde_json::to_string(&held).ok().as_deref() == Some("9007199254740992.5");
        println!(
            "arbitrary_precision: {}",
            if arbitrary_precision { "on" } else { "off" }
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"n": {"type": "number", "maximum": 9_007_199_254_740_992_u64}}
        });
        let arguments: serde_json::Value =
            serde_json::from_str(r#"{"n": 9007199254740992.5}"#).expect("the arguments");

        let checked = check(Some(&schema), &arguments);

        if arbitrary_precision {
            assert!(
                matches!(
                    &checked,
                    Err(ArgumentMismatch::Refused { keyword: "maximum", pointer, .. }) if pointer == "/n"
                ),
                "{checked:?}"
            );
        } else {
            assert_eq!(checked, Ok(()));
        }
    }

    /// A member named twice is refused before Canon reads the arguments: `serde_json` keeps the
    /// last of the two, and `Value::member` the first.
    #[test]
    fn check_value_refuses_a_member_named_twice() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"limit": {"type": "integer"}}
        });
        let twice = Value::Object(vec![(
            "outer".to_owned(),
            Value::Object(vec![
                ("limit".to_owned(), Value::Text("all".to_owned())),
                ("limit".to_owned(), Value::Number("20".to_owned())),
            ]),
        )]);

        assert_eq!(
            check_value(Some(&schema), &twice),
            Err(ArgumentMismatch::DuplicateMember {
                pointer: "/outer/limit".to_owned()
            })
        );
        let once = Value::Object(vec![("limit".to_owned(), Value::Number("20".to_owned()))]);
        assert_eq!(check_value(Some(&schema), &once), Ok(()));
    }

    fn selected(id: u32) -> Selection<crate::model::run::selection_state::Selected> {
        Selection::new(SelectionData {
            selection_id: SelectionId(uuid(id)),
            catalogue_id: CatalogueId(uuid(0xc00)),
            action: "tests.run".to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision: 1,
            replaced_by: None,
        })
    }

    fn request(request: u32, selection: u32) -> RequestArguments {
        RequestArguments {
            argument_request_id: ArgumentRequestId(uuid(request)),
            selection_id: SelectionId(uuid(selection)),
        }
    }

    /// The selection and the argument request get different ids here, as `Loom::run` gives them, so
    /// a request that names its own id, or any id but its selection's, is caught.
    #[test]
    fn a_request_names_the_selection_it_serves() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Selected(selected(0x5e1)).snapshot());
        record.put(AnySelection::Selected(selected(0x5e2)).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e2));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::Requested {
                arguments_requested: ArgumentsRequested {
                    argument_request_id: ArgumentRequestId(uuid(0xa01)),
                    selection_id: SelectionId(uuid(0x5e2)),
                },
            })
        );
        let requests = record.argument_requests();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!(requests[0].state, ArgumentRequestState::Requested);
        assert_eq!(
            requests[0].data.argument_request_id,
            ArgumentRequestId(uuid(0xa01))
        );
        assert_eq!(requests[0].data.selection_id, SelectionId(uuid(0x5e2)));
    }

    /// `selection-unknown`: nothing is recorded for a selection the record does not hold.
    #[test]
    fn a_request_for_an_unknown_selection_is_refused() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Selected(selected(0x5e1)).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e9));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::SelectionUnknown {
                error: SelectionNotFound {
                    selection_id: SelectionId(uuid(0x5e9)),
                },
            })
        );
        assert!(record.argument_requests().is_empty());
    }

    /// `selection-not-selected`: nothing is recorded for a selection no longer `Selected`.
    #[test]
    fn a_request_for_a_refused_selection_is_refused() {
        let mut record = RequestRecord::default();
        record.put(AnySelection::Refused(selected(0x5e1).refuse()).snapshot());

        let outcome = record.request_arguments(request(0xa01, 0x5e1));

        assert_eq!(
            outcome,
            Ok(RequestArgumentsOutcome::SelectionNotSelected {
                error: SelectionNotSelected {
                    selection_id: SelectionId(uuid(0x5e1)),
                },
            })
        );
        assert!(record.argument_requests().is_empty());
    }
}
