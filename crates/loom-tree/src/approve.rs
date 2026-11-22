//! `tree approve`: a principal's decision on one capability for one case of the tree, recorded at
//! the case's current revision.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use b10x_loom_bindings::{ChildInputs, NotPrepared, Prepared, RunPlace, prepare};
use b10x_loom_intake_slice::bindings::admit_bound_program_runner;
use b10x_loom_intake_slice::confinement::{ConfinementError, TestRunner};

use b10x_loom_commission::model::primitives::Timestamp as CommissionTimestamp;
use b10x_loom_commission::model::responsibility::{ActionStatus, CaseId, EffectOutcome};
use b10x_loom_commission::ports::governor::Governor as _;
use loom::bindings::{BindingsDocument, Executor, ResultSource};
use loom::runlog::{ActionFinished, AuthorityRecorded, RunLogEvent};
use loom::tree::{ApprovalDecision, CaseApproval, TreeManifest, TreeRefusalReason};
use loom_governor::{CanonGovernor, FallibleCaseStore, FileCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::approvals::record_approval;
use crate::log::Logs;
use crate::person::{PERFORMED, PersonEffects, ProgramEnd, ProgramEnded};
use crate::run::{case_bindings, one_step};
use crate::status::members;
use crate::store::{existing_store, read_manifest};
use crate::{Refused, StoreLock, TreeTime, unreadable};

/// The most bytes of a principal or a capability `tree approve` records.
pub const MAX_NAME_BYTES: usize = 200;

/// What `tree approve` is given.
pub struct ApproveRequest<'a> {
    /// The store directory `tree open` made.
    pub store: &'a Path,
    /// The case of the tree the decision is for.
    pub case: &'a str,
    /// The capability decided; `None` only with a person action whose protocol action needs no
    /// capability, which then records no authority.
    pub capability: Option<&'a str>,
    /// The principal who decided.
    pub principal: &'a str,
    /// The decision.
    pub decision: ApprovalDecision,
    /// The `person` action the principal then takes, with `--action`.
    pub action: Option<&'a str>,
    /// The result the person action ends with, with `--result`: a value every `from: effect` entry
    /// of the action's binding lists in its `results`; `performed` when `None`.
    pub result: Option<&'a str>,
    /// The person's reasons for a named result, kept as a blob of the store.
    pub reasons: Option<Reasons<'a>>,
    /// Where the command reads its instants.
    pub time: TreeTime,
    /// The tree's catalog.
    pub catalog: &'a ProtocolCatalog,
    /// The runner the program of a person action whose binding names an `argv` starts through,
    /// as a program step's does; `None` refuses such an action `ConfinementRefused` (`NoRunner`)
    /// before anything is written. An action with no program starts nothing and needs none.
    pub runner: Option<Arc<dyn TestRunner>>,
}

impl std::fmt::Debug for ApproveRequest<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("ApproveRequest")
            .field("store", &self.store)
            .field("case", &self.case)
            .field("capability", &self.capability)
            .field("principal", &self.principal)
            .field("decision", &self.decision)
            .field("action", &self.action)
            .field("result", &self.result)
            .field("reasons", &self.reasons)
            .field("time", &self.time)
            .field("runner", &self.runner.as_ref().map(|runner| runner.label()))
            .finish_non_exhaustive()
    }
}

/// Whether `action` of `case` in the tree `store` holds is bound to a person whose binding names
/// an `argv`, a program its approval runs: read from the case's bindings, taking no lock of the
/// store and writing nothing. A host calls it before [`approve`] to know whether the approval
/// needs a runner.
///
/// # Errors
/// As the store and the tree's bindings files are read: `NotATree`, `CaseNotInTree`,
/// `UnknownProtocol`, `BindingsRefused`, `StoreUnreadable`.
pub fn person_program(
    store: &Path,
    case: &str,
    action: &str,
    catalog: &ProtocolCatalog,
) -> Result<bool, Refused> {
    let store = existing_store(store)?;
    let manifest = read_manifest(&store)?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let bindings = case_bindings(&governor, &manifest, catalog, case)?;
    Ok(bindings.actions.iter().any(|bound| {
        bound.action == action
            && bound.executor.kind == Executor::Person
            && bound.executor.argv.is_some()
    }))
}

/// Where a person's reasons come from.
#[derive(Debug, Clone, Copy)]
pub enum Reasons<'a> {
    /// The text itself (`--reasons`).
    Text(&'a str),
    /// An absolute path of a regular file inside the tree's workspace (`--reasons-file`), opened
    /// without following a symbolic link ([`b10x_loom_bindings::read_reasons_file`]).
    File(&'a Path),
}

/// Records the decision `request` names (see the module documentation) and returns it; `None`
/// for a person action taken with no capability, which records no authority.
///
/// # Errors
/// `StoreInUse` when another writer holds the store; `NotATree`; `CaseNotInTree` when the case is
/// not a case of the tree; `NotAPersonAction` for a principal or capability that is empty, longer
/// than [`MAX_NAME_BYTES`] or holds a control character, for no capability without a person
/// action to allow, and for a person action whose protocol action needs a capability none was
/// given for; for a result without a person action to allow or that an entry of the action does
/// not list, reasons without a result, and reasons that are empty, over 65536 bytes or 60 lines,
/// or a file not read as [`b10x_loom_bindings::read_reasons_file`] reads it; `LogRefused` when
/// the case's run log does not verify; `StoreUnreadable`. Nothing is written on a refusal, but
/// for `PersonProgramFailed` after a start.
///
/// A person action whose binding names an `argv` (`story:person-action-runs-a-program`) runs that
/// program once, after the decision is recorded and inside the effect Commission admitted:
/// `ConfinementRefused` when [`ApproveRequest::runner`] is none or not admitted for bound programs
/// or refuses the workspace, and `PersonProgramFailed` when the step is refused before a start
/// (`RunsSpent`, `ArtifactUnreadable`, a program that cannot be started), all before anything is
/// written. Started, the program reads `{}` on standard input, the environment a program step's
/// reads and the principal only as the argv entries that are exactly `{principal}`. Only an exit
/// of 0 records, with the placed artifacts' revisions, in one store write. Any other end (an exit
/// other than 0, killed at its timeout, refused by its runner, `ChangedByNonWriter`, a placed
/// artifact that cannot be digested after the run) records nothing, keeps the approval and the
/// run's observation, writes the moves of what could be digested and the case's `action
/// finished` line, and refuses `PersonProgramFailed`, naming the end and the blob of the
/// program's output by its digest, never what it printed.
pub fn approve(request: &ApproveRequest<'_>) -> Result<Option<CaseApproval>, Refused> {
    let allows_action = request.action.is_some() && request.decision == ApprovalDecision::Allow;
    if request.capability.is_none() && !allows_action {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "a decision names a capability; only a person action that needs none is taken without one",
        ));
    }
    if request.result.is_some() && !allows_action {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "a result is named only for a person action taken",
        ));
    }
    if request.reasons.is_some() && request.result.is_none() {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "reasons are given only with a named result",
        ));
    }
    for (what, name) in [
        ("principal", Some(request.principal)),
        ("capability", request.capability),
        ("result", request.result),
    ] {
        let Some(name) = name else { continue };
        if name.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(Refused::new(
                TreeRefusalReason::NotAPersonAction,
                format!(
                    "the {what} is empty, longer than {MAX_NAME_BYTES} bytes or holds a control character"
                ),
            ));
        }
    }
    let store = existing_store(request.store)?;
    read_manifest(&store)?;
    let _held = StoreLock::acquire(&store)?;
    let manifest = read_manifest(&store)?;
    let at = request.time.now()?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(request.catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    if !members(&governor, &manifest.root)?
        .iter()
        .any(|member| member.case == request.case)
    {
        return Err(Refused::new(
            TreeRefusalReason::CaseNotInTree,
            format!("the tree rooted at {} holds no such case", manifest.root),
        )
        .case(crate::time::cut(request.case)));
    }
    let case = CaseId(request.case.to_owned());
    let revision = governor
        .current_revision(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    let logs = Logs::open(&store, request.time.clone())?;
    logs.reconcile(request.case)?;
    // The program of an allowed person action is prepared here, before anything is written: a
    // refusal of its runner, its run count or its placed artifacts writes nothing.
    let mut program = None;
    if let Some(action) = request.action {
        let bindings = person_action(&governor, &manifest, request, action)?;
        if request.decision == ApprovalDecision::Allow {
            program = prepare_program(&governor, &store, &manifest, request, action, &bindings)?;
        }
    }
    // The reasons are read and bounded before anything is written.
    let reasons = match request.reasons {
        None => None,
        Some(Reasons::Text(text)) => {
            b10x_loom_bindings::reasons_fit(text)
                .map_err(|detail| reasons_refused(request, detail))?;
            Some(text.to_owned())
        }
        Some(Reasons::File(path)) => Some(
            b10x_loom_bindings::read_reasons_file(Path::new(&manifest.workspace), path)
                .map_err(|detail| reasons_refused(request, detail))?,
        ),
    };
    let approval = match request.capability {
        Some(capability) => {
            let approval = CaseApproval {
                case: request.case.to_owned(),
                capability: capability.to_owned(),
                decision: request.decision,
                principal: request.principal.to_owned(),
                case_revision: revision,
                at,
            };
            record_approval(&store, &approval)?;
            logs.append(
                request.case,
                RunLogEvent::AuthorityRecorded(AuthorityRecorded {
                    capability: approval.capability.clone(),
                    decision: format!("{:?}", approval.decision),
                    principal: approval.principal.clone(),
                }),
            )?;
            Some(approval)
        }
        None => None,
    };
    if let Some(action) = request.action
        && request.decision == ApprovalDecision::Allow
    {
        let reasons_blob = reasons
            .map(|text| {
                b10x_loom_bindings::keep_reasons(&store, &text).map_err(|error| {
                    unreadable(
                        format!("case {}", request.case),
                        format!("the reasons: {error}"),
                    )
                })
            })
            .transpose()?;
        take_person_action(
            &governor,
            &store,
            &manifest,
            request,
            action,
            &logs,
            PersonStep {
                reasons_blob,
                program,
            },
        )?;
    }
    Ok(approval)
}

/// A person's program prepared to start, and the admitted runner it starts through.
type PersonProgram = (Prepared, Arc<dyn TestRunner>);

/// What a person's step is taken with besides the request: the digest of the person's reasons
/// kept as a blob, and the program its binding names, prepared.
struct PersonStep {
    reasons_blob: Option<String>,
    program: Option<PersonProgram>,
}

/// The program of `action` prepared to start, with the admitted runner it starts through, when
/// its person binding names an `argv`; `None` when it names none. Nothing is written.
///
/// # Errors
/// `ConfinementRefused` when the request's runner is not admitted for bound programs or refuses
/// the workspace; `PersonProgramFailed` when the step is refused before a start (`RunsSpent`,
/// `ArtifactUnreadable`, a program that cannot be started); `StoreUnreadable`.
fn prepare_program<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    store: &Path,
    manifest: &TreeManifest,
    request: &ApproveRequest<'_>,
    action: &str,
    bindings: &BindingsDocument,
) -> Result<Option<PersonProgram>, Refused> {
    let Some(bound) = bindings.actions.iter().find(|bound| {
        bound.action == action
            && bound.executor.kind == Executor::Person
            && bound.executor.argv.is_some()
    }) else {
        return Ok(None);
    };
    let confinement = |error: ConfinementError| {
        Refused::new(TreeRefusalReason::ConfinementRefused, error.to_string())
            .case(request.case.to_owned())
            .action(crate::time::cut(action).to_owned())
    };
    let runner = admit_bound_program_runner(request.runner.as_ref()).map_err(confinement)?;
    let workspace = PathBuf::from(&manifest.workspace);
    runner.preflight_child(&workspace).map_err(confinement)?;
    let case = CaseId(request.case.to_owned());
    let place = RunPlace {
        governor,
        case: &case,
        document: bindings,
        workspace: &workspace,
        store,
        runner: runner.as_ref(),
        cancel: None,
    };
    // A person's program reads `{}` on standard input and no retry feedback.
    match prepare(&place, bound, Some(request.principal), |_, _| {
        Ok(ChildInputs::empty())
    }) {
        Ok(prepared) => Ok(Some((prepared, runner))),
        Err(NotPrepared::Refused(outcome)) => {
            let detail = match outcome {
                EffectOutcome::Refused(refused) => refused.reason,
                other => format!("{other:?}"),
            };
            Err(Refused::new(TreeRefusalReason::PersonProgramFailed, detail)
                .case(request.case.to_owned())
                .action(crate::time::cut(action).to_owned()))
        }
        Err(NotPrepared::Failed(error)) => {
            Err(unreadable(format!("case {}", request.case), error.message))
        }
    }
}

/// The line the case's log gains for a person's program that did not end well: its exit, or why
/// it has none, its blob, and no evidence.
fn finished(action: &str, ended: &ProgramEnded) -> ActionFinished {
    let run = &ended.run;
    let (exit, refusal) = if run.timed_out {
        (None, Some("timed out".to_owned()))
    } else if run.exit.is_some() {
        (run.exit, None)
    } else {
        (
            None,
            Some(
                run.runner_refusal
                    .clone()
                    .unwrap_or_else(|| "ended with no exit".to_owned()),
            ),
        )
    };
    ActionFinished {
        action: action.to_owned(),
        exit,
        refusal,
        blob_digest: run.blob_sha256.clone(),
        evidence: Vec::new(),
        revisions: Vec::new(),
    }
}

/// The refusal `PersonProgramFailed` of `action` whose program ended `ended`: it names the end
/// and the blob of the program's output by its digest, never what the program printed.
fn program_failed(request: &ApproveRequest<'_>, action: &str, ended: &ProgramEnded) -> Refused {
    let program = &ended.program;
    let how = match &ended.end {
        ProgramEnd::Exit(Some(code)) => format!("ended exit {code}"),
        ProgramEnd::Exit(None) => "ended with no exit".to_owned(),
        ProgramEnd::TimedOut => "timed out".to_owned(),
        ProgramEnd::RunnerRefused(text) => format!("was refused by its runner: {text}"),
        ProgramEnd::ChangedByNonWriter => format!(
            "ended exit 0 and changed a placed artifact, and {action} does not write (ChangedByNonWriter); the artifact's revision moved"
        ),
        ProgramEnd::Unreadable(why) => format!(
            "ran, and a placed artifact could not be digested after it (ArtifactUnreadable); no revision moved: {why}"
        ),
    };
    let output = ended.run.blob_sha256.as_deref().map_or_else(
        || "it left no output".to_owned(),
        |digest| format!("its output is the blob {digest}"),
    );
    Refused::new(
        TreeRefusalReason::PersonProgramFailed,
        format!("the program {program} of {action} {how}; nothing was recorded, and {output}"),
    )
    .case(request.case.to_owned())
    .action(crate::time::cut(action).to_owned())
}

/// The refusal of `request`'s reasons, saying `detail`.
fn reasons_refused(request: &ApproveRequest<'_>, detail: String) -> Refused {
    let refused = Refused::new(TreeRefusalReason::NotAPersonAction, detail).case(request.case);
    match request.action {
        Some(action) => refused.action(crate::time::cut(action).to_owned()),
        None => refused,
    }
}

/// The bindings document of `request`'s case, and why `action` cannot be taken as its principal
/// now, if it cannot: it must be bound to `person` in the case, its principal the one named,
/// admissible, and given a capability when its protocol action needs one.
fn person_action<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    manifest: &TreeManifest,
    request: &ApproveRequest<'_>,
    action: &str,
) -> Result<BindingsDocument, Refused> {
    let case = CaseId(request.case.to_owned());
    let bindings = case_bindings(governor, manifest, request.catalog, request.case)?;
    let refused = |detail: String| {
        Refused::new(TreeRefusalReason::NotAPersonAction, detail)
            .case(request.case.to_owned())
            .action(crate::time::cut(action).to_owned())
    };
    let Some(bound) = bindings
        .actions
        .iter()
        .find(|bound| bound.action == action && bound.executor.kind == Executor::Person)
    else {
        return Err(refused("the case binds no such action to a person".into()));
    };
    if bound.executor.principal.as_deref() != Some(request.principal) {
        return Err(refused(format!(
            "the action is bound to the person {}, not to the principal named",
            bound.executor.principal.as_deref().unwrap_or("nobody")
        )));
    }
    let frontier = governor
        .frontier(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    let Some(listed) = frontier
        .data()
        .actions
        .iter()
        .find(|listed| listed.action == action && listed.status != ActionStatus::Blocked)
    else {
        return Err(refused(
            "the action is not admissible in the case now".into(),
        ));
    };
    // Only an action that needs no capability is taken without one: authority is never skipped.
    if let (Some(needed), None) = (&listed.capability, request.capability) {
        return Err(refused(format!(
            "the action needs the capability {needed}; name it with --capability"
        )));
    }
    // A named result is one every `from: effect` entry lists: checked over all of them before the
    // first record, so there is never a half write.
    if let Some(result) = request.result
        && let Some(entry) = bound.evidence.iter().find(|entry| {
            entry.from == ResultSource::Effect
                && !entry.results.iter().any(|rule| rule.value == result)
        })
    {
        let listed: Vec<&str> = entry
            .results
            .iter()
            .map(|rule| rule.value.as_str())
            .collect();
        return Err(refused(format!(
            "the {} entry lists no result {result}; it lists {}",
            entry.kind,
            listed.join(", ")
        )));
    }
    Ok(bindings)
}

/// Takes the `person` action `action` of `request`'s case as its principal, through Commission's
/// loop: the approval just recorded is the authority it is revalidated against.
fn take_person_action<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    store: &Path,
    manifest: &TreeManifest,
    request: &ApproveRequest<'_>,
    action: &str,
    logs: &Logs,
    step: PersonStep,
) -> Result<(), Refused> {
    let PersonStep {
        reasons_blob,
        program,
    } = step;
    let bindings = person_action(governor, manifest, request, action)?;
    let (prepared, runner) = match program {
        Some((prepared, runner)) => (Some(prepared), Some(runner)),
        None => (None, None),
    };
    let case = CaseId(request.case.to_owned());
    let at = request.time.now()?;
    let revision = governor
        .current_revision(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    // The log's length too: a person action taken with no approval is taken again at one revision
    // with records of their own.
    let seed = format!(
        "{}-{revision}-{}",
        crate::approvals::read_approvals(store, request.case)?.len(),
        crate::run::read_lines(store, request.case)?.len()
    );
    let effects = PersonEffects {
        governor,
        case: case.clone(),
        bindings: &bindings,
        workspace: PathBuf::from(&manifest.workspace),
        principal: request.principal.to_owned(),
        at: CommissionTimestamp(at.0.clone()),
        seed: seed.clone(),
        result: request.result.unwrap_or(PERFORMED).to_owned(),
        reasons_blob,
        store: store.to_owned(),
        program: RefCell::new(prepared),
        runner,
        ended: RefCell::new(None),
    };
    let stepped = one_step(governor, store, &case, action, &effects, &request.time);
    // A program that did not end well refused the step: its run is logged, and the refusal names
    // how it ended.
    if let Some(ended) = effects.ended.into_inner() {
        logs.append(
            request.case,
            RunLogEvent::ActionFinished(finished(action, &ended)),
        )?;
        return Err(program_failed(request, action, &ended));
    }
    logs.append(request.case, RunLogEvent::ActionFinished(stepped?))?;
    Ok(())
}
