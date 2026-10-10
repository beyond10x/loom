#![forbid(unsafe_code)]

//! Loom: the native executor of a Commission run.
//!
//! The run model (Session, Turn, ActionCatalogue, Selection, ArgumentRequest and their ids) is
//! generated from `ess/` into `generated/rust/loom/` and re-exported here as [`model`]. It is never
//! written by hand.
//!
//! Loom implements Commission's [`AgentExecutor`] over Commission's generated `Frontier`. Its
//! selector chooses from the catalogue projected from that frontier ([`selection`]), and its
//! argument generator is handed the one catalogue entry the selection names ([`arguments`]).
//!
//! Loom keeps no admission rule of its own: whether a selected action may be proposed is
//! Commission's [`admit`]. An action that needs authority is proposed, and Commission rechecks it
//! and asks its authority provider (Atlas ADR 0082); Loom never suspends for authority.
//!
//! Given a governor ([`Loom::with_governor`]), Loom revalidates each selection against the case's
//! current frontier before proposing it ([`revalidation`]), and reports a selection made at a
//! revision the case has left as `CaseMoved`; Commission still rechecks every proposal before any
//! effect.
//!
//! [`Loom::run_loop`] runs the ported Harness loop over a commission's frontier instead
//! ([`harness::governed`]): each turn's tool list is the catalogue projected from the current
//! frontier, the model's tool call is the selection and carries its arguments, and the same
//! pipeline revalidates it before Loom proposes it. Each completed turn is recorded
//! ([`Loom::turns`]), with the catalogue it was offered ([`Loom::catalogues`]), and so is each
//! compaction of the run's session, with the usage the endpoint reported for it
//! ([`Loom::compactions`], [`compaction`]).
//!
//! A governed run is interrupted by [`Loom::interrupt`] and its session resumed by id with
//! [`Loom::resume_loop`], which continues from the approval checkpoint the run stopped at and
//! revalidates against the frontier current at resume before it proposes anything ([`recovery`]).

/// The run model, synthesized from the ESS specification.
pub use loom as model;

pub mod arguments;
pub mod compaction;
pub mod credentials;
pub mod harness;
pub mod projection;
pub mod recovery;
pub mod revalidation;
pub mod selection;
pub mod session;

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, Admission, CaseId, Commission, CompletionDetermination, ExecutorOutcome,
    ExecutorOutcomeCaseMoved, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, Frontier,
    GovernorError, ProposedActionArguments, SuspensionReason, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;

pub use arguments::{ArgumentContext, ArgumentGenerator, EmptyObjectArguments, OverruleRefused};
pub use selection::{
    ActionSelector, Confidence, FirstAdmissibleSelector, HybridSelector, InvalidThreshold,
    ReasoningModelSelector, SelectorError,
};

use arguments::RequestRecord;
use model::behaviour::SelectionStorage;
use model::run::obligations::{
    CountBoundaryRefusalBehavior, RecordSelectionBehavior, RequestArgumentsBehavior,
    RevalidateSelectionBehavior,
};
use model::run::{
    ActionCatalogue, ActionCatalogueSnapshot, AnySelection, ArgumentRequestId,
    ArgumentRequestSnapshot, CatalogueId, CompactionSnapshot, CountBoundaryRefusal,
    OverruleSelection, OverruleSelectionOutcome, RecordSelection, RequestArguments,
    RequestArgumentsOutcome, RevalidateSelection, RevalidateSelectionOutcome, Selection,
    SelectionId, SelectionRecordId, SelectionRecordSnapshot, SelectionSnapshot, SelectionStrategy,
    SessionId, TurnId, TurnSnapshot, action_catalogue_state, selection_state,
};
use recovery::Recovery;
use selection::{Pick, SelectionContext, SelectionRefusal};
use session::TurnRecord;

/// Loom as Commission's agent executor: a selector, an argument generator and the run's prompt,
/// with the record of every selection it has made, every argument request that serves one and
/// every revalidation of one.
///
/// Each call of [`AgentExecutor::run`] is one run, numbered from 0 per Loom. A run's catalogue is
/// identified by its frontier's id; its selection and its argument request get ids of their own,
/// derived from this Loom's namespace, the frontier's id and the run's number. Two runs on one
/// frontier therefore keep two selections and two requests apart, and so do two Looms: each has a
/// namespace of its own, random unless [`Loom::with_instance`] fixes it, so a Loom built to recover
/// a run never gives an id an earlier one gave. The catalogues and selections of the governed loop
/// ([`Loom::run_loop`], [`Loom::resume_loop`]) are numbered by one count of this Loom's, across all
/// of its runs, so a resumed run never repeats an id an earlier run of the same Loom gave.
///
/// `V` is how Loom holds the governor it revalidates selections against: a reference, `Box`, `Rc`
/// or `Arc` of a [`Governor`], given by [`Loom::with_governor`]. A Loom made by [`Loom::new`] holds
/// none ([`NoGovernor`]) and proposes its selections unrevalidated.
pub struct Loom<S, G, V = &'static NoGovernor> {
    selector: S,
    arguments: G,
    prompt: String,
    instance: String,
    record: Mutex<RequestRecord>,
    turns: Mutex<TurnRecord>,
    recovery: Mutex<Recovery>,
    runs: AtomicU64,
    numbered: AtomicU64,
    governor: Option<V>,
}

/// The governor of a Loom made by [`Loom::new`]: there is none. It has no value, so no call to it
/// is ever made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoGovernor {}

impl Governor for NoGovernor {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        match *self {}
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        match *self {}
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        match *self {}
    }
}

impl<S, G> Loom<S, G> {
    /// A Loom that selects with `selector`, generates arguments with `arguments` and works on
    /// `prompt`, with an empty record, no runs yet, no governor and a random namespace for its run
    /// ids.
    pub fn new(selector: S, arguments: G, prompt: impl Into<String>) -> Self {
        Self {
            selector,
            arguments,
            prompt: prompt.into(),
            instance: fresh_instance(),
            record: Mutex::default(),
            turns: Mutex::default(),
            recovery: Mutex::default(),
            runs: AtomicU64::new(0),
            numbered: AtomicU64::new(0),
            governor: None,
        }
    }

    /// This Loom, revalidating every selection against the case's current frontier before it
    /// proposes it. The frontier, its case revision and its action ids are read from `governor`
    /// once per run, after the selection; never from the model, and never from the frontier the run
    /// was handed.
    pub fn with_governor<V>(self, governor: V) -> Loom<S, G, V>
    where
        V: Deref,
        V::Target: Governor,
    {
        Loom {
            selector: self.selector,
            arguments: self.arguments,
            prompt: self.prompt,
            instance: self.instance,
            record: self.record,
            turns: self.turns,
            recovery: self.recovery,
            runs: self.runs,
            numbered: self.numbered,
            governor: Some(governor),
        }
    }
}

impl<S, G, V> Loom<S, G, V> {
    /// This Loom, deriving its run ids in the namespace `instance` instead of a random one. Two
    /// Looms in one namespace give the same id to the same run, so fix it only where ids must be
    /// reproduced, as in a test, and never share one between Looms that run side by side.
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Every catalogue [`Loom::run_loop`] offered a turn, in the order offered: one per turn, the
    /// catalogue projected from the case's current frontier for it.
    pub fn catalogues(&self) -> Vec<ActionCatalogueSnapshot> {
        self.turn_record().catalogues().to_vec()
    }
    /// Every selection this Loom has made, in the order it made them: one per run that selected,
    /// and two for a run whose confidence fallback overruled a fast selection, the fast one first,
    /// `Overruled` and naming the one that replaced it.
    pub fn selections(&self) -> Vec<SelectionSnapshot> {
        self.record().selections().to_vec()
    }

    /// The telemetry of every selection this Loom has made (`loom.run.SelectionRecord`), one per
    /// selection, in the order it made them: the strategy of the selector that made it, how many
    /// candidates it was offered, the action it chose and its confidence, on a fast selection a
    /// confidence fallback overruled the strategy of the selector that replaced it
    /// (`fell_back_to`), and how long the pick took and the tokens its selector reported for it.
    /// In a governed run ([`Loom::run_loop`]) the first selection of a turn carries that turn's
    /// latency and tokens and a later one of the same turn 0. A selection refused at the execution
    /// boundary keeps the record written when it was made. For Metaharness; never evidence (Atlas
    /// ADR 0074).
    pub fn selection_records(&self) -> Vec<SelectionRecordSnapshot> {
        self.record().selection_records().to_vec()
    }

    /// Every argument request this Loom has recorded, in the order it recorded them, each naming
    /// the selection of its own run.
    pub fn argument_requests(&self) -> Vec<ArgumentRequestSnapshot> {
        self.record().argument_requests().to_vec()
    }

    /// The outcome of every revalidation this Loom has made, one per run that reached it, in the
    /// order it made them. A refusal names the selection and why: the action the current frontier
    /// does not list, or the selection's case revision and the current one.
    pub fn revalidations(&self) -> Vec<RevalidateSelectionOutcome> {
        self.record().revalidations().to_vec()
    }

    /// Every turn [`Loom::run_loop`] recorded (`loom.run.RecordTurn`), in the order recorded: one
    /// per turn a provider completed, with the items it added.
    pub fn turns(&self) -> Vec<TurnSnapshot> {
        self.turn_record().turns().to_vec()
    }

    /// Every compaction [`Loom::run_loop`] recorded (`loom.run.RecordCompaction`), in the order
    /// recorded: one per compaction the loop made, on the session it was made in, with the usage
    /// the endpoint reported for its summary request ([`compaction`]).
    pub fn compactions(&self) -> Vec<CompactionSnapshot> {
        self.turn_record().compactions().to_vec()
    }

    /// The record, whatever a panicking holder left: each write to it is one whole snapshot.
    fn record(&self) -> MutexGuard<'_, RequestRecord> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The sessions and turns, whatever a panicking holder left: each write is one whole snapshot.
    fn turn_record(&self) -> MutexGuard<'_, TurnRecord> {
        self.turns.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The running runs and held checkpoints, whatever a panicking holder left. Taken before the
    /// turn record whenever both are held, never after it.
    fn recovery(&self) -> MutexGuard<'_, Recovery> {
        self.recovery.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The next number of this Loom's governed runs, catalogues and selections: from 0, across
    /// every run, resumed runs included, so no two get one number.
    fn next_number(&self) -> u64 {
        self.numbered.fetch_add(1, Ordering::Relaxed)
    }
}

impl<S: ActionSelector, G, V> Loom<S, G, V> {
    /// The selector's choice from `catalogue`, as the selection `selection_id`, under the strategy
    /// of the selector that made the pick ([`selection::resolve`]), as a run records it:
    /// a [`HybridSelector`] that fell back answers with the stronger selector's strategy, one
    /// that accepted the fast pick with the fast selector's, never `Hybrid`. A fast pick a
    /// fallback overruled is not returned. An action the catalogue does not list is refused and
    /// named, whatever the selector's confidence.
    pub fn select(
        &self,
        catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
        selection_id: SelectionId,
    ) -> Result<Selection<selection_state::Selected>, SelectionRefusal> {
        let context = SelectionContext {
            prompt: self.prompt.clone(),
        };
        // The overruled fast pick's id is minted as `Loom::prepare` mints it; the pick is dropped.
        let overruled_id = SelectionId(loom_id(
            &self.instance,
            "overruled-selection",
            &selection_id.0.0,
            0,
        ));
        selection::resolve(
            &self.selector,
            &context,
            catalogue,
            (selection_id, overruled_id),
        )
        .map(|resolved| resolved.selection)
    }
}

/// Whether Commission would refuse every action `frontier` lists: then there is nothing to propose.
fn admits_nothing(frontier: &Frontier<frontier_state::Issued>) -> bool {
    frontier
        .data()
        .actions
        .iter()
        .all(|listed| matches!(admit(frontier, &listed.action), Admission::Refused(_)))
}

/// Whether `frontier` lists an entry of `action` that decides `admission`, whatever the order of
/// its entries: an `Admissible` entry when admitted, an `ApprovalRequired` entry naming the
/// capability when it needs authority.
fn has_deciding_entry(
    frontier: &Frontier<frontier_state::Issued>,
    action: &str,
    admission: &Admission,
) -> bool {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.action == action)
        .any(|listed| match admission {
            Admission::Admissible(_) => listed.status == ActionStatus::Admissible,
            Admission::NeedsAuthority(needs) => {
                listed.status == ActionStatus::ApprovalRequired
                    && listed.capability.as_deref() == Some(needs.capability.as_str())
            }
            Admission::Refused(_) => false,
        })
}

/// A namespace no other Loom is given: SHA-256 over the process, the time, a count of the
/// namespaces this process made, and two randomly keyed hashes of that count.
fn fresh_instance() -> String {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let made = MADE.fetch_add(1, Ordering::Relaxed);
    let keyed = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(made);
        hasher.finish()
    };
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let digest = Sha256::digest(
        format!(
            "{}\n{nanos}\n{made}\n{:016x}{:016x}",
            std::process::id(),
            keyed(),
            keyed()
        )
        .as_bytes(),
    );
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The `kind` id of number `run` in `scope` (a frontier, a session or a catalogue), in the Loom
/// namespace `instance`: a name-based UUID (RFC 9562 version 8) over the SHA-256 of the four.
/// Within one namespace, two runs, two scopes or two kinds get different ids, up to a SHA-256
/// collision; two Looms get different ids because each has its own namespace, so a Loom built to
/// recover a run, which numbers its runs from 0 again, never repeats an earlier Loom's ids. Every
/// turn, catalogue, selection and argument-request id is one. A turn is numbered by its index in
/// its session; the others by a count of their Loom's that no run restarts, so none repeats.
fn loom_id(instance: &str, kind: &str, scope: &str, run: u64) -> model::primitives::Uuid {
    name_uuid(&format!("{instance}\n{kind}\n{scope}\n{run}"))
}

/// The `kind` id of number `run` in `scope`, in no Loom's namespace: the same in every Loom. Only
/// a compaction's id is one ([`compaction`]); it is unique within its session.
fn run_id(kind: &str, scope: &str, run: u64) -> model::primitives::Uuid {
    name_uuid(&format!("{kind}\n{scope}\n{run}"))
}

/// The name-based UUID (RFC 9562 version 8) over the SHA-256 of `name`.
fn name_uuid(name: &str) -> model::primitives::Uuid {
    let digest = Sha256::digest(name.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    model::primitives::Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn no_useful_action() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

/// The case moved after Loom's catalogue was projected at `expected_case_revision`
/// (`story:moved-case-outcome`): Commission reloads the case and judges the run on it.
fn case_moved(expected_case_revision: i64) -> ExecutorOutcome {
    ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
        expected_case_revision,
    })
}

fn outage(error: String) -> ExecutorOutcome {
    ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: SuspensionReason::ExternalAvailability(Value::Object(vec![(
            "error".to_owned(),
            Value::Text(error),
        )])),
    })
}

impl<S, G, V> AgentExecutor for Loom<S, G, V>
where
    S: ActionSelector,
    G: ArgumentGenerator,
    V: Deref,
    V::Target: Governor,
{
    /// The port has no error channel. A frontier for another case than the commission's, a
    /// frontier that admits nothing, a selector that finds nothing admissible, a selection the
    /// projected catalogue does not list, and a selection Commission refuses are `NoUsefulAction`. A selector that is unavailable, or a failing
    /// argument generator, is `Suspended` with `ExternalAvailability` carrying its message.
    ///
    /// Every selection made is recorded; for one Commission does not refuse, the argument request
    /// is recorded against it before the generator is handed the selected catalogue entry.
    ///
    /// With a governor ([`Loom::with_governor`]), the selection is then revalidated against the
    /// case's current frontier, read from the governor once, before it is proposed: a selection
    /// made at another case revision is `CaseMoved`, naming the revision of the frontier the run
    /// was handed, so Commission judges the run on the case as it is now; one of an action that
    /// frontier's catalogue does not list is `NoUsefulAction`; a governor that cannot answer is
    /// `Suspended` with `ExternalAvailability` carrying its error.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let run = self.runs.fetch_add(1, Ordering::Relaxed);
        if frontier.data().case_id != commission.data().case_id || admits_nothing(frontier) {
            return no_useful_action();
        }
        // The selector sees only the catalogue projected from this frontier. Until a run assigns
        // turn identities, one frontier is one turn and one catalogue, identified by the
        // frontier's id; the selection and the argument request are this run's own.
        let identity = || frontier.data().frontier_id.0.0.clone();
        let catalogue = projection::project(
            frontier,
            CatalogueId(model::primitives::Uuid(identity())),
            TurnId(model::primitives::Uuid(identity())),
        );
        let prepared = self.prepare(
            &self.selector,
            &self.arguments,
            (frontier, &catalogue),
            (
                SelectionId(loom_id(&self.instance, "selection", &identity(), run)),
                ArgumentRequestId(loom_id(
                    &self.instance,
                    "argument-request",
                    &identity(),
                    run,
                )),
            ),
            None,
        );
        match prepared {
            Ok(prepared) => self.finish(&commission.data().case_id, prepared, None),
            Err(outcome) => outcome,
        }
    }
}

/// The session a governed run holds and the run's number: where a selection refused at the
/// execution boundary is counted (`loom.run.CountBoundaryRefusal`). A run with no session, as
/// [`AgentExecutor::run`] is, counts nothing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Boundary<'s> {
    pub(crate) session: &'s SessionId,
    pub(crate) run: u64,
}

impl<S, G, V> Loom<S, G, V> {
    /// Counts one boundary refusal on the session `at` names, while its run still holds it: a
    /// session another run resumed, or one an interrupt moved out of `Active`, is not counted.
    pub(crate) fn count_refusal(&self, at: Option<Boundary<'_>>) {
        let Some(at) = at else {
            return;
        };
        // The recovery lock before the turn record, as everywhere both are held.
        let recovery = self.recovery();
        if recovery.holds(at.session, at.run) {
            let _ = self
                .turn_record()
                .count_boundary_refusal(CountBoundaryRefusal {
                    session_id: at.session.clone(),
                });
        }
    }

    /// Records the telemetry of `selection`, made from `pick` out of `candidates` candidates
    /// (`loom.run.RecordSelection`); `fell_back_to` is the strategy of the selector that replaced
    /// it, on a fast selection a confidence fallback overruled. Telemetry decides nothing, so a
    /// record that could not be written changes nothing about the run.
    fn record_telemetry(
        record: &mut RequestRecord,
        selection: &SelectionSnapshot,
        (pick, candidates, fell_back_to): (&Pick, usize, Option<SelectionStrategy>),
        instance: &str,
    ) {
        let selection_id = selection.data.selection_id.clone();
        let _ = record.record_selection(RecordSelection {
            selection_record_id: SelectionRecordId(loom_id(
                instance,
                "selection-record",
                &selection_id.0.0,
                0,
            )),
            selection_id,
            strategy: selection.data.strategy,
            candidate_count: i64::try_from(candidates).unwrap_or(i64::MAX),
            chosen_action: selection.data.action.clone(),
            confidence: selection.data.confidence.clone(),
            fell_back_to,
            latency_ms: pick.latency_ms,
            input_tokens: pick.input_tokens,
            output_tokens: pick.output_tokens,
        });
    }
}

/// A selection the pipeline has recorded and generated arguments for, not yet revalidated: what
/// [`Loom::prepare`] hands [`Loom::finish`].
struct Prepared {
    selection_id: SelectionId,
    action: String,
    arguments: Value,
}

impl Prepared {
    /// The proposal this selection becomes once it is revalidated.
    fn proposal(&self) -> ExecutorOutcomeProposedAction {
        ExecutorOutcomeProposedAction {
            action: self.action.clone(),
            arguments: ProposedActionArguments(self.arguments.clone()),
        }
    }
}

impl<S, G, V> Loom<S, G, V> {
    /// From a selector's choice to the arguments of the selected action, on `frontier` and the
    /// catalogue projected from it, under the ids given: the selection, Commission's admission,
    /// the argument request, then the generator. [`Loom::finish`] revalidates what it prepared;
    /// the two are one pipeline, which [`AgentExecutor::run`] runs with this Loom's own selector
    /// and generator, and [`Loom::run_loop`] with the model's tool call.
    ///
    /// Every selection made is recorded; for one Commission does not refuse, the argument request
    /// is recorded against it before the generator is handed the selected catalogue entry. What it
    /// returns instead of a prepared selection is what [`AgentExecutor::run`] documents.
    ///
    /// A fast selection a confidence fallback overruled ([`ActionSelector::resolve`]) is recorded
    /// too, before the selection that replaced it, and then overruled by it
    /// (`loom.run.OverruleSelection`): it never reaches argument generation or revalidation. Each
    /// selection carries the strategy of the selector that made it.
    ///
    /// Each selection recorded gets its telemetry as it is recorded (`loom.run.RecordSelection`,
    /// [`Loom::selection_records`]); the overruled fast one names the strategy of the selection
    /// that replaced it. A selection Commission's admission refuses is counted on the
    /// session of the run `at` names, if any ([`Loom::count_refusal`]).
    fn prepare(
        &self,
        selector: &impl ActionSelector,
        arguments: &impl ArgumentGenerator,
        (frontier, catalogue): (
            &Frontier<frontier_state::Issued>,
            &ActionCatalogue<action_catalogue_state::Projected>,
        ),
        (selection_id, argument_request_id): (SelectionId, ArgumentRequestId),
        at: Option<Boundary<'_>>,
    ) -> Result<Prepared, ExecutorOutcome> {
        let context = SelectionContext {
            prompt: self.prompt.clone(),
        };
        // A fast selection a confidence fallback overruled gets an id of its own, minted beside the
        // chosen selection's (`story:fallback-selection-recording`).
        let overruled_id = SelectionId(loom_id(
            &self.instance,
            "overruled-selection",
            &selection_id.0.0,
            0,
        ));
        let resolved =
            match selection::resolve(selector, &context, catalogue, (selection_id, overruled_id)) {
                Ok(resolved) => resolved,
                Err(SelectionRefusal::NotInCatalogue(_))
                | Err(SelectionRefusal::Selector(SelectorError::NothingAdmissible)) => {
                    return Err(no_useful_action());
                }
                Err(SelectionRefusal::Selector(SelectorError::Unavailable(error))) => {
                    return Err(outage(error));
                }
            };
        let selection = resolved.selection;
        let selection_id = selection.data().selection_id.clone();
        let selected = selection.data().action.clone();
        let candidates = catalogue.data().entries.len();
        let strategy = selection.data().strategy;
        {
            // The overruled fast selection, then the selection that replaced it, each with its
            // telemetry, then the overrule: only the replacement goes on to arguments and
            // revalidation.
            let mut record = self.record();
            let overruled =
                resolved
                    .overruled
                    .zip(resolved.overruled_pick)
                    .map(|(overruled, pick)| {
                        let snapshot = AnySelection::Selected(overruled).snapshot();
                        record.put(snapshot.clone());
                        Self::record_telemetry(
                            &mut record,
                            &snapshot,
                            (&pick, candidates, Some(strategy)),
                            &self.instance,
                        );
                        snapshot.data.selection_id
                    });
            let snapshot = AnySelection::Selected(selection).snapshot();
            record.put(snapshot.clone());
            Self::record_telemetry(
                &mut record,
                &snapshot,
                (&resolved.pick, candidates, None),
                &self.instance,
            );
            if let Some(overruled) = overruled {
                let outcome = record.overrule(OverruleSelection {
                    selection_id: overruled,
                    replacement_id: selection_id.clone(),
                });
                if !matches!(outcome, Ok(OverruleSelectionOutcome::Overruled { .. })) {
                    return Err(no_useful_action());
                }
            }
        }

        // Safety invariant: only what Commission admits, or admits once authorized, is proposed.
        // An action outside the catalogue was refused above; Commission decides the rest.
        let admission = admit(frontier, &selected);
        if matches!(admission, Admission::Refused(_))
            || !has_deciding_entry(frontier, &selected, &admission)
        {
            self.count_refusal(at);
            return Err(no_useful_action());
        }
        // The generator is handed the one entry the selection names, never the rest of the
        // catalogue (Atlas ADR 0073, step 1); the selection was checked against the catalogue.
        let Some(entry) = catalogue
            .data()
            .entries
            .iter()
            .find(|entry| entry.action == selected)
        else {
            return Err(no_useful_action());
        };
        let requested = self.record().request_arguments(RequestArguments {
            argument_request_id,
            selection_id: selection_id.clone(),
        });
        if !matches!(requested, Ok(RequestArgumentsOutcome::Requested { .. })) {
            return Err(no_useful_action());
        }

        let context = ArgumentContext {
            prompt: self.prompt.clone(),
        };
        let arguments = arguments.generate(&context, entry).map_err(outage)?;
        Ok(Prepared {
            selection_id,
            action: selected,
            arguments,
        })
    }
}

impl<S, G, V> Loom<S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// The proposal of what [`Loom::prepare`] prepared, once revalidated against the governor's
    /// current frontier of `case`; what [`Loom::revalidate`] returns instead when it is refused,
    /// a refusal counted on the session of the run `at` names.
    fn finish(
        &self,
        case: &CaseId,
        prepared: Prepared,
        at: Option<Boundary<'_>>,
    ) -> ExecutorOutcome {
        // Revalidation is the last step before the proposal (Atlas ADR 0072): it moves the
        // selection out of `Selected`, which the argument request above requires.
        if let Err(refused) = self.revalidate(case, prepared.selection_id.clone(), at) {
            return refused;
        }
        ExecutorOutcome::ProposedAction(prepared.proposal())
    }
}

impl<S, G, V> Loom<S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// `loom.run.RevalidateSelection` of `selection_id` against the current frontier of `case`,
    /// asked of the governor once: its case revision, and the action ids of the catalogue
    /// projected from it, so an action it lists but no longer admits is not in it. `Ok` when the
    /// selection is admitted, or when this Loom has no governor; otherwise what the run returns
    /// instead of a proposal. A governor that cannot answer, or answers with another case's
    /// frontier, is an outage; a selection refused `stale-revision` is `CaseMoved`, naming the
    /// revision it was selected at; any other outcome but `admitted` is `NoUsefulAction`. Every
    /// refusal is recorded on the selection and in [`Loom::revalidations`]; a `stale-revision` or
    /// `not-in-frontier` refusal is counted on the session of the run `at` names, if any
    /// ([`Loom::count_refusal`]).
    fn revalidate(
        &self,
        case: &CaseId,
        selection_id: SelectionId,
        at: Option<Boundary<'_>>,
    ) -> Result<(), ExecutorOutcome> {
        let Some(governor) = &self.governor else {
            return Ok(());
        };
        let governor: &V::Target = governor;
        let current = governor.frontier(case).map_err(|error| {
            outage(format!(
                "the governor issued no current frontier for case `{}`: {error:?}",
                case.0
            ))
        })?;
        let issued = current.data();
        if issued.case_id != *case {
            return Err(outage(format!(
                "the governor issued a frontier for case `{}` as the current frontier of case `{}`",
                issued.case_id.0, case.0
            )));
        }
        let identity = || model::primitives::Uuid(issued.frontier_id.0.0.clone());
        let catalogue = projection::project(&current, CatalogueId(identity()), TurnId(identity()));
        let revalidated = self.record().revalidate_selection(RevalidateSelection {
            selection_id,
            case_revision: issued.case_revision,
            frontier_actions: catalogue
                .data()
                .entries
                .iter()
                .map(|entry| entry.action.clone())
                .collect(),
        });
        match revalidated {
            Ok(RevalidateSelectionOutcome::Admitted { .. }) => Ok(()),
            Ok(RevalidateSelectionOutcome::StaleRevision { selection_stale }) => {
                self.count_refusal(at);
                Err(case_moved(selection_stale.catalogue_revision))
            }
            Ok(RevalidateSelectionOutcome::NotInFrontier { .. }) => {
                self.count_refusal(at);
                Err(no_useful_action())
            }
            _ => Err(no_useful_action()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_loom_commission::model::primitives::Uuid;
    use b10x_loom_commission::model::responsibility::{
        AgentRevisionId, AuthorityContext, CaseId, CommissionData, CommissionId, FrontierAction,
        FrontierData, FrontierId, PrincipalId,
    };

    /// A selector that names one action, or fails with one error.
    struct Scripted(Result<&'static str, SelectorError>);

    impl ActionSelector for Scripted {
        fn select(
            &self,
            _context: &SelectionContext,
            _candidates: &[model::run::CatalogueEntry],
        ) -> Result<selection::Choice, SelectorError> {
            self.0.clone().map(|action| selection::Choice {
                action: action.to_owned(),
                confidence: None,
            })
        }

        fn strategy(&self) -> model::run::SelectionStrategy {
            model::run::SelectionStrategy::ReasoningModel
        }
    }

    fn commission() -> Commission<commission_state::Assigned> {
        Commission::new(CommissionData {
            commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
            agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
            case_id: CaseId("CASE-1".into()),
            principal: PrincipalId("principal-a".into()),
            authority_context: AuthorityContext(Value::Null),
        })
    }

    fn frontier_for(case: &str, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
        Frontier::new(FrontierData {
            frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-000000000003".into())),
            case_id: CaseId(case.into()),
            case_revision: 1,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions,
        })
    }

    fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
        FrontierAction {
            action: action.into(),
            status,
            capability: capability.map(str::to_owned),
            reasons: Vec::new(),
        }
    }

    fn run(selector: Scripted, actions: Vec<FrontierAction>) -> ExecutorOutcome {
        Loom::new(selector, EmptyObjectArguments, "investigate")
            .run(&commission(), &frontier_for("CASE-1", actions))
    }

    /// `story:action-selector`: the selector is handed the catalogue projected from the frontier,
    /// and an action that catalogue does not list is refused before Commission is asked.
    #[test]
    fn selector_cannot_expand_frontier() {
        let outcome = run(
            Scripted(Ok("forbidden.action")),
            vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// An approval-gated selection is proposed: Commission asks for the authority (ADR 0082).
    #[test]
    fn approval_gated_selection_is_proposed_for_commission_to_authorize() {
        let outcome = run(
            Scripted(Ok("repository.merge")),
            vec![
                listed("repository.merge", ActionStatus::Admissible, None),
                listed(
                    "repository.merge",
                    ActionStatus::ApprovalRequired,
                    Some("repository.write"),
                ),
            ],
        );
        assert!(
            matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == "repository.merge"),
            "{outcome:?}"
        );
    }

    #[test]
    fn an_unavailable_selector_keeps_its_message() {
        let outcome = run(
            Scripted(Err(SelectorError::Unavailable(
                "model endpoint unreachable".to_owned(),
            ))),
            vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
        );
        let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::ExternalAvailability(detail),
        }) = &outcome
        else {
            panic!("not an outage: {outcome:?}");
        };
        assert_eq!(
            detail.member("error"),
            Some(&Value::Text("model endpoint unreachable".to_owned()))
        );
    }

    #[test]
    fn nothing_admissible_from_the_selector_is_no_useful_action() {
        let outcome = run(
            Scripted(Err(SelectorError::NothingAdmissible)),
            vec![listed(
                "repository.merge",
                ActionStatus::ApprovalRequired,
                Some("repository.write"),
            )],
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// The commission is for CASE-1; a frontier issued for CASE-2 is not its frontier, and Loom
    /// proposes nothing on it, however admissible its actions.
    #[test]
    fn a_frontier_for_another_case_is_no_useful_action() {
        let outcome = Loom::new(
            Scripted(Ok("metrics.inspect")),
            EmptyObjectArguments,
            "investigate",
        )
        .run(
            &commission(),
            &frontier_for(
                "CASE-2",
                vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
            ),
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// A run id is a version-8 UUID, the same for the same inputs, and different when the Loom's
    /// namespace, the kind, the frontier or the run number differs.
    #[test]
    fn run_ids_are_uuids_of_their_namespace_kind_frontier_and_run() {
        const FRONTIER: &str = "00000000-0000-4000-8000-000000000003";
        const LOOM: &str = "loom-a";
        let id = loom_id(LOOM, "selection", FRONTIER, 0).0;
        let groups: Vec<&str> = id.split('-').collect();
        assert_eq!(
            groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12],
            "{id}"
        );
        assert!(
            id.bytes()
                .all(|byte| byte == b'-' || matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "{id}"
        );
        assert!(groups[2].starts_with('8'), "version 8: {id}");
        assert!(
            matches!(groups[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'),
            "RFC 9562 variant: {id}"
        );
        assert_eq!(loom_id(LOOM, "selection", FRONTIER, 0).0, id);
        for other in [
            loom_id("loom-b", "selection", FRONTIER, 0),
            loom_id(LOOM, "argument-request", FRONTIER, 0),
            loom_id(LOOM, "selection", FRONTIER, 1),
            loom_id(LOOM, "selection", "00000000-0000-4000-8000-000000000004", 0),
            model::primitives::Uuid(FRONTIER.to_owned()),
        ] {
            assert_ne!(other.0, id);
        }
    }

    /// Each Loom gets a namespace no other Loom has, unless one is fixed for it.
    #[test]
    fn each_loom_has_its_own_namespace_unless_one_is_fixed() {
        let new = || {
            Loom::new(
                Scripted(Ok("metrics.inspect")),
                EmptyObjectArguments,
                "inspect",
            )
        };
        let (first, second) = (new(), new());
        assert_ne!(first.instance, second.instance);
        assert_eq!(first.instance.len(), 32, "{}", first.instance);
        assert_eq!(new().with_instance("loom-a").instance, "loom-a");
    }
}
