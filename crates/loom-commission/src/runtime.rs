//! The runtime loop: [`run_until_blocked`] drives one commission over its ports until its run ends,
//! and hands every admitted action request to the [`EffectPort`] (Atlas ADR 0082).
//!
//! One call is one loop, and one loop starts one Run of the commission (`StartRun`) at the case
//! revision its first iteration loaded. A Run holds the case at that revision, or at a revision its
//! own effect moved the case to (`ess/commission/domains/responsibility.yaml`, `Run.case_revision`).
//! Each iteration makes these calls, in this order:
//!
//! 0. the step budget ([`LoopContext::step_budget`], or [`UNBUDGETED_STEP_LIMIT`] when it sets
//!    none): when the steps taken reach it, the Run is suspended through `SuspendRun` with
//!    `SuspensionReason::Budget({"max_steps": <budget>})` and the run ends suspended with that
//!    reason, or `Waiting` while a request is pending (8b). A step is an executor call whose
//!    iteration did not end the run ([`LoopEnd::steps`]), or an invocation of a pending request
//!    again (8b). When the last step left an approval gate pending (5) and no request is pending,
//!    the iteration first reads the case and
//!    the frontier (1-3) and checks the gate, so a last step that leaves the gate unchanged ends
//!    `AwaitingApproval`, not suspended for its budget. When the last step's effect was
//!    `Performed` (8) and left no gate pending, as a step performed under a grant never does (5),
//!    the iteration first loads the case (1-2), so a case that step completed ends the run
//!    completed; a case still open is suspended for its budget. When the last step's
//!    proposal was stale (8), the iteration first loads the case (1-2), so a move it finds ends the
//!    run as item 2 says; only a case still at the revision held is suspended for its budget;
//! 1. [`Governor::current_revision`]: the case is loaded. After an iteration whose effect the port
//!    reported `Performed`, the loaded revision is the one the loop holds the case at from then on;
//!    after a `Refused` or `Pending` one, as after no effect or a fan-out with a stale read (8a), a
//!    move is somebody else's;
//! 2. [`Governor::completion`]: a case the governor holds complete ends the run completed, carrying
//!    the governor's outcome, and the iteration makes no further call. Otherwise, when the loaded
//!    revision is not the one the loop holds the case at, the runtime has found a move itself (as
//!    after a stale proposal, 8): the frontier is read ([`Governor::frontier`]). A frontier for
//!    another revision than the loaded one ends the run with no admissible action; one that lists
//!    actions, none of which the effect port performs, with `NoPerformableAction`, naming them and
//!    that frontier's open claims as in 4, as a move the governor bears out does (9); the run ends
//!    `CaseMovedOn` when that frontier, as the executor would be handed it (6), admits an action,
//!    naming the Run's revision and the loaded one (`story:moved-run-named-outcome`); otherwise
//!    with no admissible action. The executor is not run;
//! 3. [`Governor::frontier`]: the frontier is obtained ([`LoopEnd::last_frontier`]). When it is for
//!    another revision than the one held, the run ends with no admissible action, so every request
//!    carries the held revision;
//! 4. a frontier that lists actions, none of which the effect port performs
//!    ([`EffectPort::performs`]), ends the run with `NoPerformableAction`, and the executor is not
//!    run. The outcome names every action the frontier lists, each once, in frontier order, and
//!    the frontier's open claims, its claims whose value is not `True`, in frontier order, as
//!    [`judge_performable`] answers `unperformable` (`story:unperformable-names-actions`); the
//!    frontier relates no action to a claim, so the claims are the frontier's. A frontier that
//!    lists no action at all goes on (`performable`): the outcome is derived from it;
//! 5. the approval gate, read on the frontier the executor is handed (6): when the previous
//!    iteration was a step on a handed frontier listing actions that need approval, and this
//!    handed frontier lists the same actions, each with the same status and reasons, the run ends
//!    `AwaitingApproval`, carrying the actions that need approval in frontier order, and the
//!    executor is not run. An action an authority verdict denied in this Run is not awaited; a
//!    frontier whose only actions needing approval were denied is no gate. A step performed under
//!    a grant passes the gate: when the previous step's request needed authority (8), the provider
//!    allowed it and the effect port reported it `Performed`, the gate is not read and the
//!    executor runs. Every other step leaves the gate to be read: one that performed an action
//!    needing no authority, one that proposed nothing, and one whose request was refused, stale,
//!    denied or not performed. The step budget (0) and the idle bound still end a run that only
//!    repeats granted steps; at the budget, the case a granted step leaves is loaded first (0), so
//!    a case it completed ends the run completed. While a request is pending (8b) the gate is not
//!    read: the step that admitted it is the one the gate is read after, once the port answered
//!    it otherwise, as though it had answered so at once. A stop policy with the gate off (§ Stop
//!    policy) never arms it;
//! 6. [`AgentExecutor::run_with_usage`] on that frontier, less every action the effect port does not perform
//!    and that needs no authority (`story:effect-invocation`), action by action: an action with an
//!    entry listed `ApprovalRequired`, or an entry naming a capability, stays with all its entries,
//!    performed or not, so a run still stops at its gate, and a kept action is admitted as the
//!    governor's frontier admits it. The gate (5) and the run outcome (8-10) read this handed
//!    frontier, or after a move (9, 10) the reloaded one as it would be handed, so an action the
//!    executor was never offered neither keeps a run going nor takes it off its gate; the request
//!    (8) and [`LoopEnd::last_frontier`] read the frontier as the governor issued it. The step is
//!    reported to the [`ObservationPort`] as one observation: its id and time come from the
//!    [`LoopContext`], its source is `executor`, its subject is `<case>@<frontier revision>`, and
//!    its payload names the outcome (`outcome`) and carries a proposal's `action` and `arguments`,
//!    a fan-out's `reads` (each an `action` and its `arguments`, in the order proposed), a human
//!    request's `request`, or a reported move's `expected_case_revision`, and, when the call
//!    reported what it spent ([`ExecutorStep`]'s `usage`, `story:executor-reports-usage`), that
//!    `usage` (input and output tokens, wall time, and the cost when it reported one), and the
//!    step's `transcript_digest` and `summary` when it stated them (an agent session's, never part
//!    of the proposal's arguments). It is never evidence. A reported usage is charged to the run
//!    before the step is observed, and once it is observed the run budget's spending limits are
//!    checked (Run and action budgets, below): a breach ends the run there, so the outcome is not
//!    acted on (7-10);
//! 7. on `Suspended`, the Run is suspended through `SuspendRun` with the executor's reason and the
//!    run ends suspended, carrying it. The Run is suspended with that reason even when delivering
//!    the step's observation failed; the loop then returns that failure as a [`LoopError`] that
//!    carries the reason;
//! 8. on `ProposedAction`, the proposal becomes an action request bound to the frontier's revision
//!    ([`request`], its id from the context) and is revalidated ([`revalidate`]). A request needing
//!    authority is asked about ([`check_authority`]); once the provider allows it, the case is
//!    loaded again ([`Governor::current_revision`]), since somebody else may have moved it while
//!    the provider decided: at another revision than the request's, the request is stale,
//!    recorded in [`LoopEnd::requests`] with that answer in place of `NeedsAuthority`, and never
//!    handed to the port. Admitted, or needing authority the provider allows on a case still at
//!    the request's revision: when the port does not perform
//!    its action ([`EffectPort::performs`]), the run ends with `NoPerformableAction`, naming that
//!    action alone and the open claims of the frontier the request was made on, as in 4, the
//!    request recorded with its revalidation in [`LoopEnd::requests`] but not admitted, and the
//!    port is never handed it. When the host's stop was not asked before this executor call (its
//!    handed frontier admitting nothing without a grant), it is asked now, and a stop it names
//!    suspends the Run for `HostStop` with nothing invoked (§ Run and action budgets). Otherwise
//!    the request is recorded as admitted and handed to [`EffectPort::invoke`] once, as an
//!    [`AdmittedRequest`]. Its
//!    [`EffectOutcome`] is recorded ([`LoopEnd::effects`]) and delivered as one observation: id and
//!    time from the context, source [`EFFECT_SOURCE`], subject `<case>@<request revision>`, payload
//!    `outcome` (`Performed`, `Refused` or `Pending`), `action_request`, `action`, `arguments`, and
//!    `report` with, when the effect was invoked through a Connector, the one `attempt` a write
//!    produced or the `audit` record of a read, or `reason`, or a pending answer's `report`. It is
//!    never evidence.
//!    The loop never retries an invocation: a request is invoked again only after the port
//!    answered it `Pending` (8b). A `Refused` the port marks final
//!    (`EffectOutcomeRefused.ends_run`, `story:refused-effect-ends-run`) then ends the run, once the
//!    run budget's spending limits were checked: [`judge_refusal`] answers `final`, and the run
//!    ends `EffectRefused`, naming the request and the port's reason, with no further load of the
//!    case. An ordinary refusal changes nothing (`ordinary`). Stale: the iteration is a step, and
//!    the next iteration loads the case, which has moved: it ends the run completed if the case is complete, else
//!    `CaseMovedOn` or with no admissible action (2). Otherwise, and after an authority answer other than an allow, the run
//!    outcome is derived ([`derive`]) from the frontier the executor was handed, and the case is
//!    not loaded again;
//!
//!    8a. on `ProposedReads` (`architecture-decision-record:read-fan-out`), the fan-out is admitted
//!    only when it lists at least one and at most [`MAX_FAN_OUT`] reads, no two with the same
//!    action and arguments (each read is invoked once), and each names an action the effect port
//!    performs and reads ([`EffectPort::reads`]) that the handed frontier lists, every entry of it
//!    `Admissible` and naming no capability. Otherwise the whole proposal is
//!    refused: no request is made, no authority is asked, nothing is invoked, and the run outcome
//!    is derived as for a refused proposal. An admitted fan-out makes one request per read, in the
//!    order proposed, each with its own id from the context and bound to the frontier's revision,
//!    and revalidates every one before any is invoked. A read whose revalidation does not answer
//!    `Admitted` (a stale one, or one that needs authority by then) is recorded with its answer
//!    and not invoked; the others go on. The admitted reads are recorded as admitted and handed
//!    together to [`EffectPort::invoke_reads`], which invokes each once, concurrently, and joins
//!    them; a port that answers `None` fails the loop as an effect failure. Their effects are
//!    recorded and observed as in 8, in the order proposed, not the order they finished. A
//!    `Refused` read cancels nothing; an `Err` from any read fails the loop once every read has
//!    answered and each that answered was recorded and observed, and nothing is retried. Without
//!    one, and with the run budget not passed, a read the port refused as final ends the run
//!    `EffectRefused` as in 8, naming the first such read in the order proposed, at once when no
//!    read was stale. Beside a stale read the refusal is kept and the move is judged first, as
//!    below: a case the governor holds complete ends the run completed, a move ends it as after a
//!    stale proposal (2), and when the next iteration's load finds no move, the kept refusal ends
//!    the run `EffectRefused` before any budget stop and before the executor runs. When a
//!    read was stale, somebody else moved the case: once the admitted reads were recorded, the
//!    iteration is as a stale proposal, so the next one loads the case and judges the move (2), and
//!    no read of the fan-out makes the loaded revision the Run's (1). When no read was admitted and
//!    none was stale, the iteration is as a refused proposal. A fan-out is one step, for the step
//!    budget and the idle bound alike; it is progress when one of its admitted reads is new, and it
//!    never passes the approval gate (5). The reads the port answered `Pending` are invoked again
//!    together (8b);
//!
//!    8b. a request the port answered `Pending` (`EffectOutcomePending`, `story:pending-effect`),
//!    after a proposal (8), a fan-out (8a) or a resume, is pending: the work it started has not
//!    finished. The next iteration loads the case and reads the frontier as any other (1-4), and,
//!    instead of the gate (5) and the executor (6), invokes the same request again, under its own
//!    id and at its own revision, with no wait or timer before it; the reads of a fan-out pending
//!    together are invoked again together, the others of it not again. A case the governor holds
//!    complete ends the run first, and a move the load finds ends it as in 2, the request not
//!    invoked again. Each pending request is revalidated as before its first invocation and
//!    recorded with that answer ([`LoopEnd::requests`]): one that still needs authority is asked
//!    about again ([`check_authority`]), and once the provider allows it the case is loaded again,
//!    a case at another revision making it stale, as in 8. A pending read is never invoked on an
//!    authority: it is invoked again only while the rule that admitted its fan-out (8a) still holds
//!    for it on the frontier the iteration read, the port reading it and every entry of it
//!    `Admissible` and naming no capability; one it no longer holds for is recorded as needing the
//!    capability an entry names (otherwise as not admitted) and, as one the frontier no longer
//!    admits, is not invoked again. A stale one is not invoked again, and the iteration is as a
//!    stale proposal:
//!    a step, after which the next iteration judges the move. A request the provider answers
//!    `ApprovalRequired` for when asked again ends the run `NeedsAuthority`, naming the provider's
//!    request, as at a first `ApprovalRequired` (8): with a suspension port the Run is suspended
//!    and the commission's Suspension holds the request, at its revision, for a resume to perform;
//!    it is never dropped. One the frontier no longer admits, or the provider denies (remembered as
//!    in 8) or fails to decide for, is not invoked again and is no longer pending; when none is
//!    admitted and none is stale, the iteration goes on to the gate (5) and the executor (6) as any
//!    other. The admitted ones, unless their actions' trial
//!    limits stop the run first, are recorded as admitted and handed to the port again, a read
//!    through [`EffectPort::invoke_reads`], the one request otherwise through
//!    [`EffectPort::invoke`], and every answer is recorded and observed as in 8, the run budget's
//!    spending limits checked, and a final refusal judged, as in 8a. An invocation again is a
//!    step. A request answered `Pending` again stays pending; once none is, the step that admitted
//!    them is judged against the idle bound as though the port had answered so at once (progress
//!    when it was), and a request performed under a grant passes the gate (5). A step budget or a
//!    run budget that stops the run while a request is pending ends it `Waiting`
//!    ([`judge_budget_stop`] answers `waiting`), naming the pending requests in the order made and
//!    the budget's reason, the Run suspended for that reason as at any budget stop.
//!    A `Pending` the port marks as ending the run (`EffectOutcomePending.ends_run`,
//!    `story:pending-answer-ends-run`), after a proposal (8), a fan-out (8a), an invocation again
//!    or a resumed request, is not invoked again: once the answer was recorded, charged and
//!    observed, the run budget's spending limits checked and a final refusal judged, the Run is
//!    suspended through `SuspendRun` with `SuspensionReason::Time`, carrying that answer's
//!    `report`, and the run ends `Waiting` ([`judge_budget_stop`]), naming every request answered
//!    `Pending` by that step in the order made, with or without `ends_run`; the reason is the first
//!    one's that asked to end. A breach the stop policy kept (`KeepStep`) is the
//!    reason instead. Beside a stale request the wait is kept, as a final refusal is, and the move
//!    is judged first; when the next iteration's load finds no move, the kept wait ends the run,
//!    after a kept refusal. Commission sets no wait or timer: the host resumes the work;
//! 9. on `CaseMoved`, by which the executor reports that the case moved while it worked, naming
//!    the revision of the frontier it was handed (`story:moved-case-outcome`), the governor decides
//!    whether it moved: the case is loaded once more ([`Governor::current_revision`]). At the
//!    revision the loop holds it at, the move is not borne out, and the step is taken as
//!    `NoUsefulAction`: the run goes on as on that outcome, but the case is not loaded again for
//!    it (10). At another, the run ends judged on the case as the governor holds it now, and the
//!    executor is not run again ([`Governor::completion`], then [`Governor::frontier`]): a case the
//!    governor holds complete ends the run completed, with no frontier read; a frontier for another
//!    revision than the one just loaded ends it with no admissible action; one that lists actions,
//!    none of which the effect port performs, with `NoPerformableAction`, naming them and that
//!    frontier's open claims as in 4. Otherwise the outcome
//!    is derived from that frontier as the executor would be handed it (6), and where it would let
//!    the run go on, the run ends `CaseMovedOn` (`story:moved-run-named-outcome`), naming the Run's
//!    revision (`bound_case_revision`) and the one just loaded (`current_case_revision`): a Run
//!    stays bound to its revision, and the caller starts a new Run at the current one. The
//!    reported revision is observed (6), never trusted;
//! 10. on `CompletedLocalReasoning` or `NoUsefulAction`, the run outcome is derived from the
//!     frontier the executor was handed. Where that would end the run, the case is first loaded
//!     once more ([`Governor::current_revision`]): an executor can propose nothing without having
//!     read a move that happened while it worked. At the revision the loop holds it at, the run
//!     ends on the derived outcome; at another, it ends as on a `CaseMoved` the governor bears out
//!     (9). A governor that fails on that read fails the loop as on item 9's. Where the derivation
//!     lets the run go on, the case is not loaded here: the next iteration loads it (1). On any
//!     other outcome, the run outcome is derived from the frontier the executor was handed.
//!
//! Without a step budget the loop is bounded where the frontier does not change: two iterations in
//! a row that are idle end the run `IdleBound`, naming how many (`story:idle-bound-outcome`).
//! [`judge_idle`] decides it from the idle iterations in a row, this one included, and the bound:
//! `bound` once they reach it, and `below`, which changes nothing, before. The bound is the stop
//! policy's (below), two when it sets none. A run whose frontier admits nothing still ends with
//! no admissible action, on the outcome derived from that frontier (8-10), never on the idle
//! bound. An iteration is idle when it admits no request, or admits only requests with the action
//! and arguments of ones already admitted in this Run, at whatever revision (a refused fan-out
//! admits none): an effect that moved the case does not make the same proposal new. A newly
//! admitted request starts the count again. Idle covers `NoUsefulAction`,
//! `CompletedLocalReasoning`, a refused proposal, an authority answer other than an allow and a
//! repeated admission alike, whatever the effect port answered for it short of a final refusal,
//! which ends the run (8), and a `CaseMoved` the governor does not bear out counts as
//! `NoUsefulAction`; a stale proposal is not counted, since the next iteration ends the run, and a
//! `CaseMoved` the governor bears out ends it at once (9). An iteration whose request the port
//! answered `Pending`, and each that invokes it again and is answered `Pending`, is no idle
//! iteration and leaves the count where it was (8b). The idle bound ends a run whose
//! derivation would let it go on, so the case is not loaded again before it (10), except after an
//! iteration whose effect the port reported `Performed` (8, 8a): a completed case comes first, so,
//! as at the step budget (0), the case is loaded ([`Governor::current_revision`], then
//! [`Governor::completion`]) before the bound ends the run, and a case the governor holds complete
//! ends it completed, carrying the governor's outcome, not `IdleBound`. A loop that keeps making
//! progress is ended by [`UNBUDGETED_STEP_LIMIT`]. With a step budget, the budget
//! alone bounds the loop, unless the stop policy sets an idle bound.
//!
//! # Stop policy
//!
//! [`LoopContext::stop_policy`] is the host's choice, per run, of how the loop stops where the
//! frontier does not change (`ess/commission/domains/responsibility.yaml`, `StopPolicy`,
//! `story:stop-policy-gate-and-idle-bound`); the model never supplies it. The default keeps the
//! stops above: the gate on and no idle bound.
//!
//! * `approval_gate: false` turns the approval gate (5) off: no step arms it, so a run never ends
//!   `AwaitingApproval`, and a step on an unchanged frontier listing actions that need approval
//!   goes to the executor and is judged as any other, idle when it admits no new request. With a
//!   step budget, the budget used up suspends the Run for it at once (0), as when no gate is
//!   pending. Authority is untouched: a proposal of an action that needs approval is still asked
//!   about (8), one the provider answers `ApprovalRequired` for ends the run `NeedsAuthority` (and,
//!   under [`run_with_suspension`], holds a Suspension), and one it does not allow is never handed
//!   to the effect port.
//! * `idle_bound: Some(n)` is the bound the idle iterations are judged against, with or without a
//!   step budget, whichever is reached first ending the run; a value below one is read as one,
//!   so it never turns the bound off. `None` keeps two, judged only for a run with no step budget.
//!   A run budget's `max_steps` counts as a step budget here, as at (0).
//! * `on_breach` says what becomes of an executor step whose reported usage passed a spending
//!   limit, or whose call ended at or after the run's deadline
//!   (`story:run-budget-tokens-and-deadline`, below). `DiscardStep`, the default, ends the
//!   run before the step's outcome is acted on. `KeepStep` acts on it as with no breach: a proposal
//!   is revalidated, asked about and, when admitted, performed, as any other, and nothing the
//!   breach did not do is granted. A step whose outcome ends the run ends it with that outcome;
//!   otherwise the Run is suspended for the breach before the next step, at the point and in the
//!   way the used-up step budget suspends it (0), the kept breach the reason even when the step's
//!   own effect then passed another limit.
//! * `breach_by_limit` chooses per limit (`story:breach-choice-per-limit`): a limit it lists takes
//!   its own `step`, and a limit it does not list takes `on_breach`, so the empty list, the
//!   default, is `on_breach` for every limit. Once an executor call was observed, every run limit
//!   it breached is collected, the spending limits in the order `spent_past` checks them, then
//!   the deadline, which is read whether or not a spending breach was kept. The step is discarded
//!   when any of them discards it, the Run suspended for the first that does; when every one
//!   keeps it, it is acted on and the Run suspended before the next step for the first breached
//!   limit. A limit listed twice is a [`LoopError`] ([`LoopFailure::BreachLimitListedTwice`])
//!   with nothing started, and [`resume_after_grant`] refuses it before it reads the Suspension.
//!   `MaxSteps` and `MaxTrials` stop the run before a step, never as an executor step's breach:
//!   listing them is allowed and changes nothing.
//!
//! # Run and action budgets
//!
//! A [`LoopContext::run_budget`] bounds the run and each action it lists
//! (`ess/commission/domains/responsibility.yaml`, `RunBudget`). Every effect answer the loop
//! records (8, 8a, 8b, and a resumed request) is charged to the run and to its request's action
//! ([`LoopEnd::charges`]): one trial, and the usage a `Performed` effect reported (input and output
//! tokens, wall time, cost; a negative figure charges nothing, and a usage with no cost leaves the
//! cost unknown); a `Pending` answer is one trial and no usage. A reported usage is delivered in
//! the effect's observation as `usage`. A breach suspends the Run through `SuspendRun` with
//! `SuspensionReason::RunBudget`, naming the action (none for the run's own limits), the limit,
//! its value and the charge, and the run ends suspended with that reason, or `Waiting` while a
//! request is pending (8b):
//!
//! * `max_steps` acts as the step budget (0), the lower of the two holding; when it is the one
//!   reached, the reason is `RunBudget`, else `Budget`;
//! * an action's `max_trials` is checked before a request is handed to the effect port, and before
//!   a pending one is handed to it again: a request that would pass it is recorded with its
//!   revalidation and not admitted, and a fan-out that would take any action past it invokes none
//!   of its reads. A resumed request is not checked;
//! * the spending limits (`max_tokens_in`, `max_tokens_out`, `max_duration_ms`,
//!   `max_cost_micro_usd`, and the run's `max_tokens`, its input and output tokens together), the
//!   action's in the order the budget lists them and then the run's in that order, are checked
//!   once an effect, or a whole fan-out, was recorded and observed: a charge above a limit, or an
//!   unknown cost under a cost limit, ends the run there, before the case is loaded again, so no
//!   further request is made;
//! * the run's `deadline`, an RFC 3339 date-time (`story:run-budget-tokens-and-deadline`), is read
//!   once before the Run starts, and one that is not RFC 3339 is a [`LoopError`]
//!   ([`LoopFailure::InvalidDeadline`]) with nothing started; [`resume_after_grant`] refuses it
//!   before it reads the Suspension. The trusted clock ([`LoopContext::now`]) is read against it
//!   before each executor call and before a pending request is invoked again (8b): at or after it,
//!   the Run is suspended for `RunBudget`, limit `Deadline`, `max` the deadline and `charged` the
//!   reading, each in whole seconds since the Unix epoch, and the run ends suspended, or `Waiting`
//!   while a request is pending. It is read again once an executor call was observed, after the
//!   spending limits: a call that ended at or after the deadline is a breach the stop policy
//!   decides (`breach_by_limit`, else `on_breach`), so by default nothing it proposed is
//!   performed. It is not read there only when a spending limit already discarded the step,
//!   which is then the reason. A reading that is not RFC 3339 counts as past
//!   the deadline, its `charged` absent. A resume reads it twice, as it reads the case revision: before the
//!   Suspension is taken, when a deadline passed takes nothing and leaves the Suspension
//!   `Pending`, and right before the held request is invoked, when one passed releases the
//!   Suspension and performs nothing; either way the resumed Run ends suspended for `RunBudget`
//!   as above.
//!
//! A host stops a run for a reason only it can read, its active time less its own pauses or the
//! free space of a disk (`story:host-stop-before-step`), through [`LoopContext::host_stop`]. It is
//! asked where a step would follow the admissibility and authority checks
//! (`story:host-stop-when-a-step-follows`), right after the deadline and only when the deadline did
//! not stop the run: before an executor call whose handed frontier admits an action without a
//! grant ([`admit`] answers it `Admissible`), before a pending request is invoked again (8b), and
//! on a resume before the Suspension is taken and right before the held request is invoked. A
//! frontier that admits no action without a grant (each action it lists needs a grant or is
//! blocked, or it lists none) goes to the executor unasked, so the run ends as the authority or
//! admissibility check says (`NeedsAuthority`, `NoAdmissibleAction`), never for the host's stop;
//! a request that executor call's proposal is admitted to, the provider's allow included, is asked
//! about once the case revision was read again and right before the effect is invoked (8), after
//! its action's trial limit. A host asked before the call is not asked again before that effect.
//! A stop it names ends the run as a passed deadline there does, the Run suspended for
//! `SuspensionReason::HostStop`, carrying the host's `name` and `detail` unchanged: suspended, or
//! `Waiting` while a request is pending; a stop right before an effect invokes nothing; a stop
//! before the take leaves the Suspension `Pending`, and one right before the invoke releases it and
//! performs nothing. What stops a run without a step (the step budget and a kept breach, 0;
//! `NoPerformableAction`, 4; the approval gate, 5) comes first, and the host is not asked then. It
//! is never asked after an effect was invoked, and no effect is invoked past a stop it names. A
//! fan-out of reads (8a) is admitted only on a handed frontier that admits each read without a
//! grant, so the host was asked before its executor call. `None`, the default, never stops the
//! run, so a context that does not implement it runs as before.
//!
//! An executor call that reports what it spent ([`ExecutorStep`]'s `usage`, through
//! [`AgentExecutor::run_with_usage`], `story:executor-reports-usage`: an executor that is not a
//! model loop, such as an agent session) is charged to the run alone ([`LoopEnd::charges`]'s
//! `run`): its input and output tokens, wall time and cost, as a `Performed` effect's usage is (a
//! negative figure charges nothing, a usage with no cost leaves the run's cost unknown), with no
//! trial and to no action. Once the step was observed (6), the spending limits are checked as
//! after an effect, the run's being the only ones the charge moved: a breach suspends the Run for
//! `RunBudget` and ends the run suspended with that reason before its outcome is acted on (7-10):
//! no request is made from a proposal, and an executor's own `Suspended` is not the Run's, unless
//! the stop policy keeps the step for every limit it breached (`KeepStep`, through
//! `breach_by_limit` or `on_breach`, above). Nothing
//! is pending while an executor runs (8b), so such a run never ends `Waiting`. An executor that
//! reports no usage, as a model loop metered at its model port does, is charged nothing for its
//! call, as before.
//!
//! A failure that leaves no run outcome is a [`LoopError`]. Once the Run is started, the error
//! names it. When the governor, its observation port or the effect port fails after the Run
//! started, the loop first suspends the Run (`SuspendRun`, `ExternalAvailability`) unless it
//! already suspended it for the executor, so no Run is left `Running` that no return value
//! identifies; when that suspension fails too, the error carries both failures. The error reports
//! the reason the loop suspended the Run for ([`LoopError::suspension_reason`]): the reason is
//! reported, not stored on the Run. A run store that answers `storage-failed` is reported as
//! [`LoopFailure::RunStorage`]: from `StartRun`, the loop ends with no Run named before any
//! executor, authority or effect call; from `SuspendRun`, as the suspension's failure.
//!
//! # Suspending on an approval, and resuming after a restart
//!
//! [`run_with_suspension`] is the same loop given a suspension port ([`SuspensionPort`],
//! `story:approval-suspend-resume-slice`). It differs in two places. Before anything else it reads
//! the port: a commission that holds a Suspension, `Pending` or `Resuming`, starts no Run
//! ([`LoopFailure::CommissionSuspended`], naming its state and request; a port that cannot be read
//! is [`LoopFailure::SuspensionStore`]), and nothing is performed. And when a step's request needs
//! authority and the provider answers `ApprovalRequired`, so the run ends `NeedsAuthority` (8), or
//! answers so when a pending request is asked about again (8b), the Run is first suspended through `SuspendRun` with `SuspensionReason::Authority`, whose payload
//! names the `capability` and the provider's `request`, and the commission's Suspension — the Run,
//! the pending request and that reason — is held `Pending` through the port (`HoldSuspension`).
//! The caller sees the same [`LoopEnd`] as without a port. A `SuspendRun` that fails ends the loop
//! as on any other suspension, and no Suspension is held; a Suspension the port cannot hold ends it
//! with [`LoopFailure::RunStorage`] (`storage-failed`) or [`LoopFailure::SuspensionNotHeld`], the
//! Run suspended, its reason in [`LoopError::suspension_reason`]. A hold the host settled
//! (`settled`: it already did what an approval of the request would lead to,
//! `story:handoff-holds-no-suspension`) holds nothing and is no failure: the Run is put back
//! `Running` through `ResumeRun`, as a loop without a port leaves it, so no Run stays `Suspended`
//! with no Suspension behind it and a resume finds nothing to resume
//! ([`ResumeRefusal::NothingSuspended`]); the caller sees the same [`LoopEnd`]. A `ResumeRun` that
//! fails then ends the loop with [`LoopFailure::RunStorage`] or [`LoopFailure::NotResumed`], the
//! Run suspended. [`run_until_blocked`] passes no port, reads none and never suspends a Run on an
//! approval, as in 0.16.0.
//!
//! [`resume_after_grant`] continues the same Run from the stored Suspension alone, in a process
//! that need not be the one that suspended it. In this order: the Suspension is read
//! ([`ResumeRefusal::NothingSuspended`] when none is stored; [`ResumeRefusal::InDoubt`], naming its
//! request, when it is `Resuming`); it must be the commission's, on its case; the case is loaded,
//! and a revision other than the request's ends it with [`ResumeRefusal::CaseMoved`]; the request
//! is revalidated as the loop revalidates one (8), and a request that still needs authority is
//! resumed only on the provider's allow ([`ResumeRefusal::NotGranted`]), after which the case is
//! loaded again, and a revision other than the request's, the case moved while the provider
//! decided, ends it with [`ResumeRefusal::CaseMoved`]; the effect port must
//! perform its action. Then the Run is put back into a run store that does not hold it,
//! `Suspended` at the case revision it started at, and resumed through `ResumeRun`. A run budget's
//! deadline passed by then (read from [`LoopContext::now`], as above) takes nothing: the Run is
//! suspended again for `RunBudget`, limit `Deadline`, the Suspension stays `Pending`, and the
//! resume ends as a passed deadline ends a run; a host's stop asked then
//! ([`LoopContext::host_stop`], as above) does the same, the Run suspended for `HostStop`. The
//! Suspension
//! is then taken through the port for the request read and revalidated (`TakeSuspension`,
//! `Pending` → `Resuming`): of two resumes one takes it, and the other is refused,
//! [`ResumeRefusal::NothingSuspended`] when the first already released it,
//! [`ResumeRefusal::InDoubt`] when it is still `Resuming`, and [`ResumeRefusal::OtherRequest`],
//! naming both requests, when the commission holds a Suspension for another request by then (the
//! first resumed it in full and the commission was suspended again), which is kept as it is; a take
//! the port cannot record ends the resume with [`LoopFailure::RunStorage`]. On any answer but
//! `taken` the Run is suspended again in the run store with its stored reason, no Suspension is
//! held, and nothing is performed. Then the case is loaded once more
//! ([`Governor::current_revision`]), since somebody else may have moved it while the resume wrote
//! the Run and took the Suspension; before that load, a deadline passed since the take, or else a
//! host's stop, releases the Suspension, invokes nothing and ends the resumed Run as before the
//! take. At the request's
//! revision, the request, under its own id and
//! at its own revision, is handed to [`EffectPort::invoke`] once, recorded and observed as the loop
//! does (8), and the Suspension is released for that request (`ReleaseSuspension`), removed; a
//! refusal the port marks final then ends the resumed Run `EffectRefused`, as in 8, and a
//! `Pending` answer leaves the request pending, invoked again from the loop's first iteration on
//! as in 8b, revalidated and asked about again, never from the released Suspension, unless the
//! port marked it as ending the run, which then ends `Waiting` as in 8b. A provider that
//! allows the request only while its Suspension is `Pending` (the intake slice's grant authority)
//! answers `ApprovalRequired` at that ask, so the resumed Run ends `NeedsAuthority` and holds a new
//! Suspension for the same request, which an approval and a resume perform again. At
//! another revision the request can never be performed: it is recorded `Stale` in
//! [`LoopEnd::requests`], the Suspension is released for it all the same, nothing is invoked, and
//! the loop goes on as after a stale proposal (8), so its first iteration judges the move (2). A
//! release the port refuses ends the loop with [`LoopFailure::SuspensionNotReleased`], naming the
//! request. Either is the resumed Run's
//! first step ([`LoopEnd::steps`]); the loop holds no frontier an executor was handed for it, so
//! the next iteration reads no approval gate (5), as after a step performed under a grant. The loop
//! goes on from that iteration, holding the case at the revision the request was made at, or at the
//! one a `Performed` effect moved it to (1). A refusal, and any failure before the take, performs
//! nothing and leaves the Suspension `Pending`. A process that stops after the take and before the
//! release — a crash, an effect port or a governor that fails — leaves it `Resuming`: the request
//! may have been performed, so it is never performed again from the Suspension, which stays stored
//! as its durable trace; every later resume is refused [`ResumeRefusal::InDoubt`], and settling the
//! request is the host's.
#![forbid(unsafe_code)]

use std::fmt;

use crate::action_request::{request, revalidate};
use crate::admission::admit;
use crate::model::behaviour::RunStorage;
use crate::model::json::Value;
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Timestamp;
use crate::model::responsibility::obligations::{
    ResumeRunBehavior, StartRunBehavior, SuspendRunBehavior,
};
use crate::model::responsibility::{
    ActionCharge, BreachStep, BudgetLimit, ChargedUsage, RunBudget, RunBudgetBreach, RunCharges,
};
use crate::model::responsibility::{
    ActionNeedsAuthority, ActionNotAdmitted, ActionRequest, ActionRequestData, ActionRequestId,
    ActionRequestStale, ActionStatus, Admission, AnyRun, AuthorityVerdict, CaseId, Commission,
    CompletionDetermination, EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorStep, ExecutorUsage, Frontier, FrontierAction, FrontierData, GovernorError,
    HoldSuspension, HoldSuspensionOutcome, HostStop, JudgeBudgetStop, JudgeBudgetStopOutcome,
    JudgeEffectRefusal, JudgeEffectRefusalOutcome, JudgeIdleIteration, JudgeIdleIterationOutcome,
    JudgePerformableActions, JudgePerformableActionsOutcome, Observation, ObservationData,
    ObservationId, ReleaseSuspension, ReleaseSuspensionOutcome, ResumeRun, ResumeRunOutcome,
    RevalidateActionRequestOutcome, Run, RunData, RunId, RunOutcome, RunOutcomeAwaitingApproval,
    RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeEffectRefused, RunOutcomeIdleBound,
    RunOutcomeNeedsAuthority, RunOutcomeNoPerformableAction, RunOutcomeSuspended,
    RunOutcomeWaiting, RunStarted, RunStorageFailed, StartRun, StartRunOutcome, StopPolicy,
    SuspendRun, SuspendRunOutcome, SuspensionKey, SuspensionReason, SuspensionState,
    SuspensionStateConflict, TakeSuspension, TakeSuspensionOutcome, Truth, Unit, commission_state,
    frontier_state, observation_state,
};
use crate::outcome::{
    CapabilityVerdict, Derived, derive, judge_budget_stop, judge_idle, judge_performable,
    judge_refusal,
};
use crate::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use crate::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use crate::ports::evidence::ObservationPort;
use crate::ports::executor::AgentExecutor;
use crate::ports::governor::Governor;
use crate::ports::suspension::{SuspensionPort, SuspensionStoreError};
use crate::rfc3339;

/// The source of the observation of an executor step.
pub const EXECUTOR_SOURCE: &str = "executor";

/// The source of the observation of an effect.
pub const EFFECT_SOURCE: &str = "effect";

/// What the loop needs that neither the model nor the executor may supply: new ids, trusted time
/// and the budgets and stop policy the host chose.
pub trait LoopContext {
    /// A new id for an action request.
    fn action_request_id(&mut self) -> ActionRequestId;
    /// A new id for an observation.
    fn observation_id(&mut self) -> ObservationId;
    /// The current time, from a trusted clock.
    fn now(&mut self) -> Timestamp;
    /// The most steps the loop takes before it suspends the Run for `Budget`. `None` sets no
    /// budget: the idle bound then ends a loop that makes no progress, and [`UNBUDGETED_STEP_LIMIT`]
    /// ends any other, so the loop always ends.
    fn step_budget(&self) -> Option<usize>;
    /// The run's budget and its actions' budgets, as the module documents
    /// (`ess/commission/domains/responsibility.yaml`, `RunBudget`). `None`, the default, sets
    /// none: only the step budget bounds the run.
    fn run_budget(&self) -> Option<RunBudget> {
        None
    }
    /// How the run stops where its frontier does not change, as the module documents
    /// (`ess/commission/domains/responsibility.yaml`, `StopPolicy`): whether the approval gate (5)
    /// ends it `AwaitingApproval`, and how many idle iterations in a row end it `IdleBound`. The
    /// default keeps the gate and sets no idle bound: two idle iterations end a run with no step
    /// budget, and a budgeted run is bounded by its budget alone. It also says what becomes of an
    /// executor step whose reported usage passed a spending limit, or whose call ended past the
    /// deadline, per limit (`breach_by_limit`) and for every limit it does not list
    /// (`on_breach`); the default lists none and discards: `DiscardStep` ends the run before that
    /// step's outcome is acted on.
    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: true,
            idle_bound: None,
            on_breach: BreachStep::DiscardStep,
            breach_by_limit: Vec::new(),
        }
    }
    /// The host's own stop before a step, for a reason only the host can read (its active time
    /// less its own pauses, the free space of a disk), as the module documents
    /// (`ess/commission/domains/responsibility.yaml`, `HostStop`). It is asked only where a step
    /// would follow the admissibility and authority checks: before an executor call whose
    /// frontier admits an action without a grant, otherwise right before an admitted request's
    /// effect is invoked. `None`, the default, never stops the run.
    fn host_stop(&mut self) -> Option<HostStop> {
        None
    }
}

/// The steps a loop with no step budget takes at most before it suspends the Run for `Budget`.
pub const UNBUDGETED_STEP_LIMIT: usize = 64;

/// The idle iterations in a row that end a run with no step budget when its stop policy sets no
/// idle bound (`StopPolicy.idle_bound`).
const DEFAULT_IDLE_BOUND: i64 = 2;

/// The most reads one `ExecutorOutcome::ProposedReads` may list; a longer fan-out is refused whole
/// (`architecture-decision-record:read-fan-out`).
pub const MAX_FAN_OUT: usize = 16;

/// One action request the loop made and how its revalidation answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revalidated {
    /// The request, bound to the frontier it was chosen from.
    pub request: ActionRequestData,
    /// Its revalidation outcome; `Stale` for a request the provider allowed on a case that moved
    /// while it decided.
    pub outcome: RevalidateActionRequestOutcome,
}

/// How a loop ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopEnd {
    /// The Run the loop started.
    pub run_id: RunId,
    /// How the run ended.
    pub outcome: RunOutcome,
    /// Every request the loop made, in order, with its revalidation outcome. A request the effect
    /// port answered `Pending` is listed again with each revalidation before it is invoked again.
    pub requests: Vec<Revalidated>,
    /// The requests admitted, in order, each once for every time it was handed to the effect
    /// port: once, unless the port answered `Pending` and it was invoked again.
    pub admitted: Vec<ActionRequestData>,
    /// What the effect port answered for each admitted request, in the same order.
    pub effects: Vec<EffectOutcome>,
    /// The steps the run took, which the step budget bounds: every executor call whose iteration
    /// did not end the run, every invocation of a pending request again (item 8b) whose iteration
    /// did not end it, and a resumed Run's resumed request. A run whose effect port never answers
    /// `Pending` counts its executor calls alone, as before.
    pub steps: usize,
    /// The frontier the last iteration that got so far read before its executor step; `None` when
    /// no iteration read one.
    pub last_frontier: Option<FrontierData>,
    /// What the run, and each action a request was handed to the effect port for, was charged:
    /// the requests handed over and the usage their `Performed` effects reported, and, in the
    /// run's alone, the usage its executor calls reported ([`ExecutorStep`]).
    pub charges: RunCharges,
}

/// One failure that stopped a loop, or the suspension after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopFailure {
    /// The governor, or its observation port, could not answer.
    Governor(GovernorError),
    /// A run command refused with an unmet obligation.
    Obligation(UnmetObligation),
    /// `SuspendRun` did not suspend the loop's run, for a reason other than storage.
    NotSuspended(Box<SuspendRunOutcome>),
    /// The effect port could not answer for an admitted request.
    Effect(EffectError),
    /// The run store could not hold the new run (`StartRun`) or record its suspension
    /// (`SuspendRun`): their `storage-failed` outcome. Also the suspension port's `storage-failed`
    /// when it cannot hold a Suspension (`HoldSuspension`) or record a resume's take
    /// (`TakeSuspension`).
    RunStorage(RunStorageFailed),
    /// [`run_with_suspension`] started no Run: the commission holds a Suspension, in this state and
    /// for this request. Nothing was performed.
    CommissionSuspended(SuspensionStateConflict),
    /// The suspension port could not be read before a Run started. Nothing was performed.
    SuspensionStore(SuspensionStoreError),
    /// `HoldSuspension` did not store the Run's Suspension, for a reason other than storage or a
    /// hold the host settled (`settled`, which is no failure).
    SuspensionNotHeld(Box<HoldSuspensionOutcome>),
    /// `ResumeRun` did not put back `Running` the Run the loop suspended for an approval whose
    /// hold the host settled, for a reason other than storage. The Run stays `Suspended`, with no
    /// Suspension behind it.
    NotResumed(Box<ResumeRunOutcome>),
    /// `ReleaseSuspension` did not remove the Suspension a resume took, after the request it names
    /// was performed, or after the resume found the case moved off the request's revision and
    /// performed nothing. [`UnreleasedSuspension::outcome`] says what the port left stored.
    SuspensionNotReleased(Box<UnreleasedSuspension>),
    /// The run budget's `deadline` is not an RFC 3339 date-time; it holds the text. Nothing was
    /// started: the loop refuses it before the Run starts, and a resume before it takes anything.
    InvalidDeadline(String),
    /// The stop policy's `breach_by_limit` lists this limit more than once
    /// (`story:breach-choice-per-limit`). Nothing was started: the loop refuses it before the Run
    /// starts, and a resume before it takes anything.
    BreachLimitListedTwice(BudgetLimit),
}

/// A release of the Suspension a resume took that did not remove it: after its request was
/// performed (the effect port answered for it and the governor observed the answer), or after the
/// resume found the case moved off the request's revision before invoking it, and invoked nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreleasedSuspension {
    /// The request the resume took the Suspension for: performed, or stale and not performed.
    pub action_request_id: ActionRequestId,
    /// What `ReleaseSuspension` answered, and so what is stored for the commission:
    ///
    /// * `storage-failed`: the port changed nothing, and the Suspension stays `Resuming` for this
    ///   request, in doubt: every later resume is refused [`ResumeRefusal::InDoubt`], and settling
    ///   it is the host's;
    /// * `not-found`: no Suspension is stored for the commission (something other than this resume
    ///   removed it), so nothing durable records that the request was performed;
    /// * `other-request`: a Suspension for another request is stored, as it was; nothing stored
    ///   names this request;
    /// * `not-taken`: a `Pending` Suspension for this request is stored, as it was, and a later
    ///   resume may take it and perform the request again.
    pub outcome: ReleaseSuspensionOutcome,
}

impl fmt::Display for LoopFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Obligation(unmet) => write!(f, "run command failed: {unmet}"),
            Self::NotSuspended(outcome) => write!(f, "run not suspended: {outcome:?}"),
            Self::Effect(error) => error.fmt(f),
            Self::RunStorage(failed) => write!(f, "run store failed: {}", failed.reason),
            Self::CommissionSuspended(held) => write!(
                f,
                "the commission holds a {:?} Suspension for request {}",
                held.state, held.action_request_id.0.0
            ),
            Self::SuspensionStore(error) => error.fmt(f),
            Self::SuspensionNotHeld(outcome) => write!(f, "suspension not held: {outcome:?}"),
            Self::NotResumed(outcome) => {
                write!(f, "run not resumed after a settled hold: {outcome:?}")
            }
            Self::SuspensionNotReleased(unreleased) => write!(
                f,
                "suspension not released after request {} was performed or found stale: {:?}",
                unreleased.action_request_id.0.0, unreleased.outcome
            ),
            Self::InvalidDeadline(text) => {
                write!(f, "the run budget's deadline is not RFC 3339: {text:?}")
            }
            Self::BreachLimitListedTwice(limit) => write!(
                f,
                "the stop policy's breach_by_limit lists the limit {limit:?} more than once"
            ),
        }
    }
}

impl From<GovernorError> for LoopFailure {
    fn from(error: GovernorError) -> Self {
        Self::Governor(error)
    }
}

impl From<UnmetObligation> for LoopFailure {
    fn from(unmet: UnmetObligation) -> Self {
        Self::Obligation(unmet)
    }
}

/// Why a loop stopped without a run outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopError {
    /// The Run the loop started, or `None` when it failed before starting one.
    pub run_id: Option<RunId>,
    /// What stopped the loop.
    pub failure: LoopFailure,
    /// When the loop could not suspend the Run after a governor failure, why.
    pub suspension: Option<Box<LoopFailure>>,
    /// The reason the loop suspended the Run for, or tried to: the executor's, `Budget`, or
    /// `ExternalAvailability` after a governor or effect failure. `None` when it did not try.
    pub suspension_reason: Option<Box<SuspensionReason>>,
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.run_id {
            Some(run_id) => write!(f, "run {}: {}", run_id.0.0, self.failure)?,
            None => write!(f, "no run started: {}", self.failure)?,
        }
        if let Some(reason) = &self.suspension_reason {
            write!(f, "; suspended for {reason:?}")?;
        }
        match &self.suspension {
            Some(suspension) => write!(f, "; suspending the run failed too: {suspension}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for LoopError {}

/// Why [`resume_after_grant`] resumed nothing. Nothing was performed, and the Suspension, when one
/// was stored, is stored as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeRefusal {
    /// The suspension port holds no Suspension for the commission, or another resume took and
    /// released it first (`TakeSuspension` answered `not-found`).
    NothingSuspended,
    /// The commission's Suspension is `Resuming`: a resume took it and did not release it, so its
    /// request may have been performed or not. It is never performed again from the Suspension,
    /// which stays stored; settling the request is the host's.
    InDoubt {
        /// The request whose performance is in doubt.
        action_request_id: ActionRequestId,
    },
    /// The commission's Suspension is held for another request than the one this resume read and
    /// revalidated (`TakeSuspension` answered `other-request`): it was resumed and replaced since.
    /// That Suspension is kept as it is, and neither request is performed.
    OtherRequest {
        /// The request this resume read from the Suspension and revalidated.
        action_request_id: ActionRequestId,
        /// The request the Suspension now stored is held for.
        held_action_request_id: ActionRequestId,
    },
    /// The stored Suspension, or the run the run store holds under its run id, names another
    /// commission, case or run revision than the one given.
    OtherCommission,
    /// The case is no longer at the revision the request was suspended at: when the resume first
    /// loads it, or once the provider allowed the request.
    CaseMoved {
        /// The revision the request was suspended at.
        expected_case_revision: i64,
        /// The case's current revision.
        current_case_revision: i64,
    },
    /// The current frontier refuses the request's action: its revalidation answered this.
    NotAdmitted(RevalidateActionRequestOutcome),
    /// The request still needs authority, and the provider did not allow it.
    NotGranted(AuthorityCheck),
    /// The effect port does not perform the request's action.
    NotPerformed,
    /// The run store does not hold the Run suspended: `ResumeRun` answered this.
    RunNotSuspended(ResumeRunOutcome),
}

/// Why [`resume_after_grant`] returned no run outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// Nothing was resumed; why.
    Refused(ResumeRefusal),
    /// The suspension port could not be read. Nothing was resumed.
    Store(SuspensionStoreError),
    /// The governor could not answer before the Run resumed. Nothing was resumed.
    Governor(GovernorError),
    /// The run budget's deadline is not RFC 3339 (nothing was read or resumed), `ResumeRun`
    /// failed, the Suspension could not be taken, or the resumed loop stopped without a run
    /// outcome.
    Loop(LoopError),
}

impl fmt::Display for ResumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => write!(f, "resume refused: {refusal:?}"),
            Self::Store(error) => error.fmt(f),
            Self::Governor(error) => write!(f, "governor failed: {error:?}"),
            Self::Loop(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ResumeError {}

/// A failure before the Run started.
fn unstarted(failure: impl Into<LoopFailure>) -> LoopError {
    LoopError {
        run_id: None,
        failure: failure.into(),
        suspension: None,
        suspension_reason: None,
    }
}

/// What stopped the iterations without a run outcome, and the suspension the loop already made.
struct Stopped {
    failure: LoopFailure,
    /// The reason the loop suspended the Run for, or tried to, and the suspension's failure. The
    /// reason is boxed, as [`LoopError`] holds it, so a stop stays small.
    suspended: Option<(Box<SuspensionReason>, Option<LoopFailure>)>,
}

impl From<LoopFailure> for Stopped {
    fn from(failure: LoopFailure) -> Self {
        Self {
            failure,
            suspended: None,
        }
    }
}

impl From<GovernorError> for Stopped {
    fn from(error: GovernorError) -> Self {
        LoopFailure::from(error).into()
    }
}

/// Suspends the loop's Run for `reason`.
fn suspend<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
    reason: SuspensionReason,
) -> Result<(), LoopFailure> {
    match runs.suspend_run(SuspendRun {
        run_id: run_id.clone(),
        reason,
    })? {
        SuspendRunOutcome::Suspended { .. } => Ok(()),
        SuspendRunOutcome::StorageFailed { error } => Err(LoopFailure::RunStorage(error)),
        other => Err(LoopFailure::NotSuspended(Box::new(other))),
    }
}

/// Runs `commission` until its run ends, in the order the module documents.
pub fn run_until_blocked<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: StartRunBehavior + SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: None,
    };
    start(&ports, commission, runs, context)
}

/// Runs `commission` until its run ends, as [`run_until_blocked`] does, suspending the Run into
/// `suspensions` when it ends on an approval the provider withholds, as the module documents.
// The ports of `run_until_blocked`, and the suspension port.
#[allow(clippy::too_many_arguments)]
pub fn run_with_suspension<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    suspensions: &dyn SuspensionPort,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: StartRunBehavior + SuspendRunBehavior + ResumeRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    // A commission holding a Suspension, `Pending` or `Resuming`, starts no new Run: its request
    // is resumed, or settled by the host, from that Suspension alone.
    match suspensions.get(&commission.data().commission_id) {
        Err(error) => return Err(unstarted(LoopFailure::SuspensionStore(error))),
        Ok(Some(held)) => {
            return Err(unstarted(LoopFailure::CommissionSuspended(
                SuspensionStateConflict {
                    state: held.state,
                    action_request_id: held.data.action_request_id,
                },
            )));
        }
        Ok(None) => {}
    }
    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: Some(Suspensions::new(suspensions)),
    };
    start(&ports, commission, runs, context)
}

/// Starts a Run of `commission` and drives it until it ends.
fn start<G, E, A, F, R, C>(
    ports: &Ports<'_, G, E, A, F, R>,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
) -> Result<LoopEnd, LoopError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: StartRunBehavior + SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    // A deadline that names no instant, and a stop policy that lists a limit twice, are refused
    // before anything is asked or started.
    deadline_of(context.run_budget().as_ref()).map_err(unstarted)?;
    check_stop_policy(&context.stop_policy()).map_err(unstarted)?;
    let revision = ports
        .governor
        .current_revision(&commission.data().case_id)
        .map_err(unstarted)?;
    let run_started = match runs
        .start_run(StartRun {
            commission_id: commission.data().commission_id.clone(),
            case_revision: revision,
        })
        .map_err(unstarted)?
    {
        StartRunOutcome::Started { run_started } => run_started,
        StartRunOutcome::StorageFailed { error } => {
            return Err(unstarted(LoopFailure::RunStorage(error)));
        }
    };
    let result = iterate(
        ports,
        commission,
        runs,
        context,
        &run_started,
        run_started.case_revision,
        None,
    );
    finish(runs, run_started.run_id, result)
}

/// The loop's answer for the Run `run_id`: its end, or the error naming it, after suspending the
/// Run for `ExternalAvailability` when a governor or effect failure stopped it and nothing
/// suspended it yet.
fn finish<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: RunId,
    result: Result<LoopEnd, Stopped>,
) -> Result<LoopEnd, LoopError> {
    let Stopped { failure, suspended } = match result {
        Ok(end) => return Ok(end),
        Err(stopped) => stopped,
    };
    let unavailable = match (&failure, &suspended) {
        (_, Some(_)) => None,
        (LoopFailure::Governor(error), None) => Some(format!("governor failed: {error:?}")),
        (LoopFailure::Effect(error), None) => Some(error.to_string()),
        (_, None) => None,
    };
    let suspended = match unavailable {
        Some(text) => {
            let reason = SuspensionReason::ExternalAvailability(Value::Text(text));
            let suspension = suspend(runs, &run_id, reason.clone()).err();
            Some((Box::new(reason), suspension))
        }
        None => suspended,
    };
    let (suspension_reason, suspension) = match suspended {
        Some((reason, suspension)) => (Some(reason), suspension.map(Box::new)),
        None => (None, None),
    };
    Err(LoopError {
        run_id: Some(run_id),
        failure,
        suspension,
        suspension_reason,
    })
}

/// The ports one loop calls out through.
struct Ports<'p, G: ?Sized, E: ?Sized, A: ?Sized, F: ?Sized, R: ?Sized> {
    governor: &'p G,
    executor: &'p E,
    authority: &'p A,
    effects: &'p F,
    /// Where a Run ended on a withheld approval is suspended to; `None` suspends none.
    suspensions: Option<Suspensions<'p, R>>,
}

/// The suspension port of a loop given one, and how the loop puts back `Running` a Run it
/// suspended for an approval when the host settled the hold ([`unsuspend`]).
struct Suspensions<'p, R: ?Sized> {
    port: &'p dyn SuspensionPort,
    unsuspend: fn(&mut R, &RunId) -> Result<(), LoopFailure>,
}

impl<R: ?Sized> Clone for Suspensions<'_, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<R: ?Sized> Copy for Suspensions<'_, R> {}

impl<'p, R: ResumeRunBehavior + ?Sized> Suspensions<'p, R> {
    fn new(port: &'p dyn SuspensionPort) -> Self {
        Self {
            port,
            unsuspend: unsuspend::<R>,
        }
    }
}

/// Puts the Run `run_id`, which the loop suspended for an approval whose hold the host settled,
/// back `Running` (`ResumeRun`): no Suspension stands behind it, so it is left as a loop without a
/// suspension port leaves a Run that ends on an approval.
fn unsuspend<R: ResumeRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
) -> Result<(), LoopFailure> {
    match runs.resume_run(ResumeRun {
        run_id: run_id.clone(),
    })? {
        ResumeRunOutcome::Resumed { .. } => Ok(()),
        ResumeRunOutcome::StorageFailed { error } => Err(LoopFailure::RunStorage(error)),
        other => Err(LoopFailure::NotResumed(Box::new(other))),
    }
}

/// A suspended request [`resume_after_grant`] revalidated and admitted, which the resumed loop
/// hands to the effect port before its first iteration.
struct Resumed {
    request: ActionRequestData,
    answer: RevalidateActionRequestOutcome,
    /// The Suspension the resume took, released once the effect port answered for the request.
    taken: SuspensionKey,
}

/// The requests the effect port last answered `Pending`, which the next iteration invokes again
/// (`story:pending-effect`).
struct Waiting {
    /// The pending requests, in the order they were made: one request, or reads of one fan-out.
    requests: Vec<ActionRequestData>,
    /// Whether they are reads of a fan-out (8a), invoked again together through
    /// [`EffectPort::invoke_reads`]; otherwise the one request is invoked through
    /// [`EffectPort::invoke`].
    reads: bool,
    /// Whether the step that admitted them made progress, a request whose action and arguments
    /// no earlier admitted request had: once none of them is pending, the idle bound judges that
    /// step by it.
    progressed: bool,
}

impl Waiting {
    /// The ids of the pending requests, in order.
    fn ids(&self) -> Vec<ActionRequestId> {
        self.requests
            .iter()
            .map(|pending| pending.action_request_id.clone())
            .collect()
    }
}

/// What one loop has made so far.
struct Track {
    requests: Vec<Revalidated>,
    admitted: Vec<ActionRequestData>,
    effects: Vec<EffectOutcome>,
    steps: usize,
    last_frontier: Option<FrontierData>,
    charges: RunCharges,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            requests: Vec::new(),
            admitted: Vec::new(),
            effects: Vec::new(),
            steps: 0,
            last_frontier: None,
            charges: RunCharges {
                run: uncharged(),
                actions: Vec::new(),
            },
        }
    }
}

impl Track {
    fn end(self, run_id: &RunId, outcome: RunOutcome) -> LoopEnd {
        LoopEnd {
            run_id: run_id.clone(),
            outcome,
            requests: self.requests,
            admitted: self.admitted,
            effects: self.effects,
            steps: self.steps,
            last_frontier: self.last_frontier,
            charges: self.charges,
        }
    }

    /// Records `effect`, which the port answered for `request`, and charges it ([`charge`]).
    fn record(&mut self, request: &ActionRequestData, effect: &EffectOutcome) {
        self.effects.push(effect.clone());
        charge(&mut self.charges, &request.action, effect);
    }
}

/// The iterations of the loop of the Run `run`, holding its case at `held`: the Run's own revision
/// for a Run just started, whose case was loaded at it; for a resumed Run, the revision its
/// suspended request was made at, which [`resume_after_grant`] loaded. A resumed Run first hands
/// its request to the effect port, as the module documents.
fn iterate<G, E, A, F, R, C>(
    ports: &Ports<'_, G, E, A, F, R>,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    context: &mut C,
    run: &RunStarted,
    held: i64,
    resumed: Option<Resumed>,
) -> Result<LoopEnd, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: SuspendRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions,
    } = *ports;
    let case = &commission.data().case_id;
    let run_id = &run.run_id;
    let run_budget = context.run_budget();
    // The run budget's step limit bounds the loop as the step budget does; the lower one holds.
    let run_steps = run_budget
        .as_ref()
        .and_then(|limits| limits.max_steps)
        .map(|max| usize::try_from(max).unwrap_or(0));
    let budget = match (context.step_budget(), run_steps) {
        (Some(steps), Some(run_steps)) => Some(steps.min(run_steps)),
        (steps, run_steps) => steps.or(run_steps),
    };
    let policy = context.stop_policy();
    // The instant past which no step is taken; [`start`] and [`resume_after_grant`] refused a
    // deadline that names none before the Run started or resumed.
    let deadline = deadline_of(run_budget.as_ref())?;
    // How many idle iterations in a row end the run (`JudgeIdleIteration`, `bound`): the host's,
    // read as one below one, with or without a step budget; when it sets none, two, and only for a
    // run with no step budget.
    let idle_bound = match policy.idle_bound {
        Some(bound) => Some(bound.max(1)),
        None => budget.is_none().then_some(DEFAULT_IDLE_BOUND),
    };
    let mut track = Track::default();
    // The revision the loop holds the case at: the Run's, or one its own effect moved the case to.
    let mut held = held;
    // Whether the previous iteration invoked an effect.
    let mut effected = false;
    // The idle iterations in a row up to the previous one: none after one that made progress.
    let mut idle: i64 = 0;
    // The actions of the frontier the previous step's executor was handed, when it listed actions
    // awaiting approval and the step was not performed under a grant.
    let mut gate: Option<Vec<FrontierAction>> = None;
    // The actions an authority verdict denied in this Run.
    let mut denied: Vec<String> = Vec::new();
    let mut first = true;
    // Whether the previous iteration's proposal was stale: the case has moved, and this iteration
    // loads it before any budget stop (0).
    let mut after_stale = false;
    // How the run ends on a read the port refused as final in a fan-out beside a stale read, kept
    // until the next iteration has judged the move (8a).
    let mut kept_refusal: Option<RunOutcome> = None;
    // Why the run waits on a pending answer that ends it (`ends_run`) beside a stale request, kept
    // as `kept_refusal` is until the next iteration has judged the move (8a, 8b).
    let mut kept_wait: Option<SuspensionReason> = None;
    // The requests the effect port last answered `Pending`, which the next iteration invokes
    // again instead of running the executor (8b).
    let mut waiting: Option<Waiting> = None;
    // The spending limit an executor step's usage passed under a stop policy that keeps the step
    // (`BreachStep::KeepStep`): its outcome was acted on, and the Run is suspended for it before
    // the next step, as when the step budget is used up.
    let mut kept_breach: Option<RunBudgetBreach> = None;

    let limit = budget.unwrap_or(UNBUDGETED_STEP_LIMIT);
    // Why the Run is suspended when its steps reach `limit`: the run budget's step limit when it
    // is the one reached, else the step budget.
    let exhausted_for = if run_steps == Some(limit) {
        SuspensionReason::RunBudget(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxSteps,
            max: i64::try_from(limit).unwrap_or(i64::MAX),
            charged: Some(i64::try_from(limit).unwrap_or(i64::MAX)),
        })
    } else {
        step_budget_reason(limit)
    };

    // A resumed Run's suspended request is performed once, as an admitted request is (8), when
    // the case is still at the request's revision: the resume read it before it wrote the Run and
    // took the Suspension, and somebody else may have moved it since, so it is loaded once more
    // right before the invoke. The loop holds no frontier an executor was handed for that step, so
    // no gate is pending after it (5). Once the effect port answered and the governor observed its
    // answer, the request is no longer in doubt and the Suspension the resume took is released. An
    // effect port or a governor that fails first leaves it `Resuming`. On a case that moved, the
    // request bound to its revision can never be performed: it is recorded stale, the Suspension
    // is released, nothing is invoked, and the next iteration judges the move, as after a stale
    // proposal (8). A request the port answered `Pending` is invoked again as in 8b, unless the
    // answer ends the run (`ends_run`), which then waits on it. A deadline the trusted clock reads
    // as reached right before the invoke, or else the host's own stop, releases the Suspension the
    // resume took, and the Run is suspended for it with nothing invoked, as before an executor
    // call.
    if let Some(Resumed {
        request: data,
        answer,
        taken,
    }) = resumed
    {
        first = false;
        if let Some(reason) = stopped_before_step(deadline, context) {
            release_taken(suspensions, taken, &data)?;
            return budget_ended(runs, run_id, reason, None, track);
        }
        let current = governor.current_revision(&data.case_id)?;
        if current == data.expected_case_revision {
            track.requests.push(Revalidated {
                request: data.clone(),
                outcome: answer,
            });
            track.admitted.push(data.clone());
            let effect = effects
                .invoke(commission, &AdmittedRequest::new(data.clone()))
                .map_err(|error| Stopped::from(LoopFailure::Effect(error)))?;
            track.record(&data, &effect);
            governor.observe(effect_observation(context, &data, &effect))?;
            release_taken(suspensions, taken, &data)?;
            effected = matches!(effect, EffectOutcome::Performed(_));
            if matches!(effect, EffectOutcome::Pending(_)) {
                // The resumed step is judged against no idle bound, so neither is its request
                // once the port answers otherwise.
                waiting = Some(Waiting {
                    requests: vec![data.clone()],
                    reads: false,
                    progressed: true,
                });
            }
            track.steps += 1;
            if let Some(breach) = run_budget
                .as_ref()
                .and_then(|limits| spent_past(limits, &track))
            {
                let reason = SuspensionReason::RunBudget(breach);
                return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
            }
            // A refusal the port marks final ends the resumed Run, its Suspension released (8).
            if let Some(ended) = refusal_end(&data, &effect)? {
                return Ok(track.end(run_id, ended));
            }
            // A pending answer that ends the run ends the resumed Run waiting on its request, its
            // Suspension released (8b).
            if let Some(reason) = wait_reason(&effect) {
                return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
            }
        } else {
            track.requests.push(Revalidated {
                request: data.clone(),
                outcome: RevalidateActionRequestOutcome::Stale {
                    error: ActionRequestStale {
                        expected_case_revision: data.expected_case_revision,
                        current_case_revision: current,
                    },
                },
            });
            release_taken(suspensions, taken, &data)?;
            after_stale = true;
            track.steps += 1;
        }
    }

    loop {
        // With the budget used up, a pending approval gate is still read and checked first, after
        // a stale proposal the move is judged first, and after a performed effect the case is
        // loaded first, so a case the last step completed ends the run completed. While a request
        // is pending no gate is read (8b), and the run ends `Waiting`. A spending limit the
        // previous executor step passed under `KeepStep` stops the run as the used-up budget
        // does, and is the reason given.
        let stop = match &kept_breach {
            Some(breach) => Some(SuspensionReason::RunBudget(breach.clone())),
            None => (track.steps >= limit).then(|| exhausted_for.clone()),
        };
        if let Some(reason) = &stop
            && (gate.is_none() || waiting.is_some())
            && !after_stale
            && !effected
        {
            return budget_ended(runs, run_id, reason.clone(), waiting.as_ref(), track);
        }

        let loaded = if first {
            run.case_revision
        } else {
            governor.current_revision(case)?
        };
        first = false;

        let determination = governor.completion(case)?;
        if let CompletionDetermination::Complete(complete) = &determination {
            let outcome = RunOutcome::Completed(RunOutcomeCompleted {
                outcome: complete.outcome.clone(),
            });
            return Ok(track.end(run_id, outcome));
        }
        if effected {
            held = loaded;
            effected = false;
        } else if loaded != held {
            // The runtime found the move itself, as after a stale proposal (2).
            let ended = found_moved(
                governor,
                effects,
                case,
                run.case_revision,
                loaded,
                &determination,
            )?;
            return Ok(track.end(run_id, ended));
        }
        // The move a stale read showed was judged and ended nothing: the final refusal kept beside
        // it ends the run, before any budget stop and before the executor runs (8a).
        if let Some(ended) = kept_refusal.take() {
            return Ok(track.end(run_id, ended));
        }
        // So does a pending answer that ends the run kept beside it: the run waits on every
        // request still pending (8b).
        if let Some(reason) = kept_wait.take() {
            return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
        }
        // After a performed effect that left no gate pending, a case still open is suspended for
        // its budget (0), or ends `Waiting` while a request is pending.
        if let Some(reason) = &stop
            && (gate.is_none() || waiting.is_some())
            && !after_stale
        {
            return budget_ended(runs, run_id, reason.clone(), waiting.as_ref(), track);
        }

        let frontier = governor.frontier(case)?;
        if frontier.data().case_revision != held {
            return Ok(track.end(run_id, RunOutcome::NoAdmissibleAction(Unit(true))));
        }
        track.last_frontier = Some(frontier.data().clone());
        if let Some(ended) =
            unperformable_end(listed_actions(frontier.data()), frontier.data(), effects)?
        {
            return Ok(track.end(run_id, ended));
        }
        // The frontier the executor is handed; the gate and the outcome read it too.
        let handed = offered(&frontier, effects);

        // The requests the port answered `Pending` are invoked again before any executor runs,
        // each revalidated as before its first invocation (8b). No gate is read for them.
        if let Some(pending) = waiting.take() {
            if let Some(reason) = &stop {
                return budget_ended(runs, run_id, reason.clone(), Some(&pending), track);
            }
            // A deadline the trusted clock reads as reached, or else the host's own stop, stops
            // the run before the requests are invoked again; they are still pending.
            if let Some(reason) = stopped_before_step(deadline, context) {
                return budget_ended(runs, run_id, reason, Some(&pending), track);
            }
            let again = invoke_again(
                governor,
                authority,
                effects,
                commission,
                context,
                &handed,
                &pending,
                run_budget.as_ref(),
                &mut track,
                &mut denied,
            )?;
            // The provider withholds the pending request now: the run ends needing authority as
            // at a first `ApprovalRequired` (8), and with a suspension port the Run is suspended
            // and the commission's Suspension holds the request, for a resume to perform.
            if let Some(Withheld {
                request,
                capability,
                approval,
            }) = again.withheld
            {
                if let Some(suspending) = suspensions {
                    let reason = SuspensionReason::Authority(Value::Object(vec![
                        ("capability".to_owned(), Value::Text(capability)),
                        ("request".to_owned(), Value::Text(approval.clone())),
                    ]));
                    suspend_for_approval(runs, suspending, run, &request, reason)?;
                }
                let outcome =
                    RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority { request: approval });
                return Ok(track.end(run_id, outcome));
            }
            if let Some(breach) = again.over_budget {
                // Nothing was invoked: the requests admitted are still pending.
                let still = Waiting {
                    requests: again.admitted,
                    ..pending
                };
                let reason = SuspensionReason::RunBudget(breach);
                return budget_ended(runs, run_id, reason, Some(&still), track);
            }
            if !again.admitted.is_empty() {
                waiting = (!again.pending.is_empty()).then_some(Waiting {
                    requests: again.pending,
                    reads: pending.reads,
                    progressed: pending.progressed,
                });
                if let Some(breach) = run_budget
                    .as_ref()
                    .and_then(|limits| spent_past(limits, &track))
                {
                    let reason = SuspensionReason::RunBudget(breach);
                    return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
                }
                // A refusal the port marks final ends the run as in 8 and 8a: at once, unless a
                // stale revalidation showed a move, which the next iteration judges first.
                if let Some(ended) = again.ended {
                    if !again.stale {
                        return Ok(track.end(run_id, ended));
                    }
                    kept_refusal = Some(ended);
                }
                // A pending answer that ends the run ends it waiting on every request still
                // pending, as a refusal marked final ends it: at once, unless a stale revalidation
                // showed a move, which the next iteration judges first.
                if let Some(reason) = again.wait {
                    if !again.stale {
                        return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
                    }
                    kept_wait = Some(reason);
                }
                effected = again.performed && !again.stale;
                // Once none is pending, the step that admitted them is judged against the idle
                // bound as it would have been had the port answered so at once, and a step
                // performed under a grant passes the approval gate (5).
                if waiting.is_none() {
                    if let Some(bound) = idle_bound
                        && !again.stale
                        && !pending.progressed
                        && let Some(ended) =
                            idle_judged(governor, case, idle.saturating_add(1), bound, effected)?
                    {
                        return Ok(track.end(run_id, ended));
                    }
                    if !again.stale {
                        idle = if pending.progressed {
                            0
                        } else {
                            idle.saturating_add(1)
                        };
                    }
                    if effected && again.allowed {
                        gate = None;
                    }
                }
                after_stale = again.stale;
                track.steps += 1;
                continue;
            }
            if again.stale {
                // Somebody else moved the case: the next iteration judges the move (8).
                after_stale = true;
                track.steps += 1;
                continue;
            }
            // None is admitted any more, so none is pending: the iteration goes on (5, 6).
        }

        if let Some(before) = &gate
            && unchanged(before, &handed.data().actions)
        {
            let actions = awaited(&handed, &denied);
            if !actions.is_empty() {
                let outcome = RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval { actions });
                return Ok(track.end(run_id, outcome));
            }
        }
        if let Some(reason) = stop {
            return budget_ended(runs, run_id, reason, None, track);
        }
        // A deadline the trusted clock reads as reached stops the run before the executor runs.
        // The host's own stop does too, asked only when a step can follow this call without a
        // grant: the handed frontier admits an action without one. Otherwise it is asked right
        // before an effect this call's proposal is admitted to (8), and a frontier only a grant
        // moves ends the run as the authority check says (`story:host-stop-when-a-step-follows`).
        let host_asked = admits_without_grant(&handed);
        let before = if host_asked {
            stopped_before_step(deadline, context)
        } else {
            deadline_reached(deadline, context).map(SuspensionReason::RunBudget)
        };
        if let Some(reason) = before {
            return budget_ended(runs, run_id, reason, None, track);
        }

        let step = executor.run_with_usage(commission, &handed);
        if let Some(usage) = &step.usage {
            charge_step(&mut track.charges, usage);
        }
        let observed = governor.observe(observation(context, &frontier, &step));
        let ExecutorStep { outcome, usage, .. } = step;
        // Once the step is observed, every run limit the call breached is collected: the spending
        // limits its own usage passed, in the order `spent_past` checks them, then the deadline,
        // read again, which a call that ended at or after it breached. Each takes the stop
        // policy's choice for it ([`breach_step`], `story:breach-choice-per-limit`). When any
        // discards the step (`DiscardStep`, the default), the run ends suspended for the first
        // that does, before the outcome is acted on: no request is made from it, and an executor's
        // own suspension is not the Run's. When every one keeps it (`KeepStep`), the outcome is
        // acted on as with no breach, and the run, unless that outcome ends it, is suspended for
        // the first breached limit before the next step.
        if observed.is_ok() {
            let mut breached = match (&run_budget, &usage) {
                (Some(limits), Some(_)) => spent_past_each(limits, &track),
                _ => Vec::new(),
            };
            // The spending limits come first, so a spending limit that discards the step is the
            // reason whatever the clock reads; the deadline is read whenever none does, kept
            // spending breaches or not.
            if let Some(discarded) = first_discarded(&policy, &breached) {
                let reason = SuspensionReason::RunBudget(discarded);
                return budget_ended(runs, run_id, reason, None, track);
            }
            breached.extend(deadline_reached(deadline, context));
            if let Some(discarded) = first_discarded(&policy, &breached) {
                let reason = SuspensionReason::RunBudget(discarded);
                return budget_ended(runs, run_id, reason, None, track);
            }
            kept_breach = breached.into_iter().next();
        }
        if let ExecutorOutcome::Suspended(suspended) = &outcome {
            let reason = suspended.reason.clone();
            let suspension = suspend(runs, run_id, reason.clone()).err();
            return match (observed, suspension) {
                (Ok(()), None) => Ok(track.end(
                    run_id,
                    RunOutcome::Suspended(RunOutcomeSuspended { reason }),
                )),
                (Ok(()), Some(failure)) => Err(Stopped {
                    failure,
                    suspended: Some((Box::new(reason), None)),
                }),
                (Err(error), suspension) => Err(Stopped {
                    failure: LoopFailure::Governor(error),
                    suspended: Some((Box::new(reason), suspension)),
                }),
            };
        }
        observed?;
        // An outcome that proposes nothing ends the run only once the case is loaded again (10).
        let proposes_nothing = matches!(
            outcome,
            ExecutorOutcome::CompletedLocalReasoning(_) | ExecutorOutcome::NoUsefulAction(_)
        );
        let outcome = match outcome {
            ExecutorOutcome::CaseMoved(_) => {
                match moved_outcome(governor, effects, case, held, run.case_revision, &outcome)? {
                    Some(ended) => return Ok(track.end(run_id, ended)),
                    // The governor holds the case where the Run does: the move is not borne out.
                    None => ExecutorOutcome::NoUsefulAction(Unit(true)),
                }
            }
            other => other,
        };

        let mut progressed = false;
        let mut decided = false;
        let mut stale = false;
        let mut verdict: Option<(String, AuthorityVerdict)> = None;
        // Whether this step's request needed authority, the provider allowed it and the effect
        // port performed it: such a step passes the approval gate (5).
        let mut granted = false;
        // This step's request, when its revalidation answered that it needs authority.
        let mut pending: Option<ActionRequestData> = None;
        if let ExecutorOutcome::ProposedAction(proposed) = &outcome {
            let made = request(
                context.action_request_id(),
                run_id.clone(),
                &frontier,
                proposed.clone(),
            );
            let answer = revalidate(governor, &made)?;
            let data = made.into_data();
            track.requests.push(Revalidated {
                request: data.clone(),
                outcome: answer.clone(),
            });
            // A request for the action and arguments of one already admitted in this Run is no
            // progress, at whatever revision: an effect that moves the case does not make the same
            // proposal new.
            let new = !track.admitted.iter().any(|earlier: &ActionRequestData| {
                earlier.action == data.action && earlier.arguments == data.arguments
            });
            let mut admitted = false;
            match answer {
                RevalidateActionRequestOutcome::Admitted => admitted = true,
                // Not counted toward the idle bound: the next iteration's load ends the run.
                RevalidateActionRequestOutcome::Stale { .. } => stale = true,
                RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                    pending = Some(data.clone());
                    if let AuthorityCheck::Decided(answer) =
                        check_authority(authority, commission, &data, &error.capability)
                    {
                        match &answer {
                            AuthorityVerdict::Allow(Unit(true)) => admitted = true,
                            AuthorityVerdict::Deny(_) => denied.push(error.action.clone()),
                            _ => {}
                        }
                        verdict = Some((error.capability, answer));
                    }
                    // The provider allowed the request at its revision. A case somebody else
                    // moved while it decided is at another one by now: the request is stale,
                    // recorded so, and not invoked, and the next iteration judges the move (8).
                    if admitted {
                        let current = governor.current_revision(&data.case_id)?;
                        if current != data.expected_case_revision {
                            admitted = false;
                            stale = true;
                            if let Some(recorded) = track.requests.last_mut() {
                                recorded.outcome = RevalidateActionRequestOutcome::Stale {
                                    error: ActionRequestStale {
                                        expected_case_revision: data.expected_case_revision,
                                        current_case_revision: current,
                                    },
                                };
                            }
                        }
                    }
                }
                RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
            }
            if admitted {
                // Never hand the port an action it says it does not perform: the run ends naming
                // it, with the open claims of the frontier the request was made on.
                if let Some(ended) =
                    unperformable_end([data.action.as_str()], frontier.data(), effects)?
                {
                    return Ok(track.end(run_id, ended));
                }
                // An action at its trial limit is not handed to the port again.
                if let Some(breach) = run_budget
                    .as_ref()
                    .and_then(|limits| trials_past(limits, &track, &[data.action.as_str()]))
                {
                    let reason = SuspensionReason::RunBudget(breach);
                    return budget_ended(runs, run_id, reason, None, track);
                }
                // The host not asked before this executor call, its frontier admitting nothing
                // without a grant, is asked right before the effect: a stop suspends the Run for
                // it, nothing invoked (`story:host-stop-when-a-step-follows`).
                if !host_asked && let Some(stop) = context.host_stop() {
                    let reason = SuspensionReason::HostStop(stop);
                    return budget_ended(runs, run_id, reason, None, track);
                }
                track.admitted.push(data.clone());
                progressed = new;
                decided = true;
                let effect = effects
                    .invoke(commission, &AdmittedRequest::new(data.clone()))
                    .map_err(|error| Stopped::from(LoopFailure::Effect(error)))?;
                track.record(&data, &effect);
                governor.observe(effect_observation(context, &data, &effect))?;
                // A pending answer leaves the request to be invoked again at the next
                // iteration (8b).
                if matches!(effect, EffectOutcome::Pending(_)) {
                    waiting = Some(Waiting {
                        requests: vec![data.clone()],
                        reads: false,
                        progressed: new,
                    });
                }
                // Usage past a spending limit stops the run once the effect is recorded and
                // observed. A step kept past a breach is suspended for that breach once its
                // outcome was acted on, at the next iteration.
                if kept_breach.is_none()
                    && let Some(breach) = run_budget
                        .as_ref()
                        .and_then(|limits| spent_past(limits, &track))
                {
                    let reason = SuspensionReason::RunBudget(breach);
                    return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
                }
                // A refusal the port marks final ends the run once it is recorded and observed;
                // the case is not loaded again (8).
                if let Some(ended) = refusal_end(&data, &effect)? {
                    return Ok(track.end(run_id, ended));
                }
                // A pending answer that ends the run is not invoked again: the run ends waiting
                // on it, for the time the port's report names, or for the spending limit a kept
                // step passed, which ends the run first (8b).
                if let Some(reason) = wait_reason(&effect) {
                    let reason = kept_breach
                        .clone()
                        .map_or(reason, SuspensionReason::RunBudget);
                    return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
                }
                // Only an effect that took place may have moved the case; after a refusal, a move
                // is somebody else's.
                effected = matches!(effect, EffectOutcome::Performed(_));
                granted = effected && matches!(verdict, Some((_, AuthorityVerdict::Allow(_))));
            }
        }
        // A fan-out of reads (8a): refused whole unless every read is admissible on the handed
        // frontier and one the port reads; otherwise each read is revalidated, and the admitted
        // ones are invoked together. It is one step.
        if let ExecutorOutcome::ProposedReads(proposed) = &outcome
            && fans_out(&proposed.reads, &handed, effects)
        {
            let mut fanned = fan_out(
                governor,
                effects,
                commission,
                context,
                run_id,
                &frontier,
                &proposed.reads,
                run_budget.as_ref(),
                &mut track,
            )?;
            // The reads the port answered `Pending` are invoked again together at the next
            // iteration (8b).
            if !fanned.pending.is_empty() {
                waiting = Some(Waiting {
                    requests: std::mem::take(&mut fanned.pending),
                    reads: true,
                    progressed: fanned.progressed,
                });
            }
            if let Some(breach) = fanned.over_budget.or_else(|| {
                run_budget
                    .as_ref()
                    .filter(|_| kept_breach.is_none())
                    .and_then(|limits| spent_past(limits, &track))
            }) {
                let reason = SuspensionReason::RunBudget(breach);
                return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
            }
            // A read the port refused as final ends the run once every read was recorded, unless a
            // stale read showed the case moved: then the refusal is kept, the next iteration
            // judges the move first, as after a stale proposal, and the kept refusal ends the run
            // when that judgement ends nothing (8a).
            if let Some(ended) = fanned.ended {
                if !fanned.stale {
                    return Ok(track.end(run_id, ended));
                }
                kept_refusal = Some(ended);
            }
            // A read answered pending with `ends_run` ends the run once every read was recorded,
            // waiting on every read answered pending, in the order proposed, for the time the
            // first such read's report names, or for the spending limit a kept step passed; beside
            // a stale read it is kept as a final refusal is (8a, 8b).
            if let Some(reason) = fanned.wait {
                let reason = kept_breach
                    .clone()
                    .map_or(reason, SuspensionReason::RunBudget);
                if !fanned.stale {
                    return budget_ended(runs, run_id, reason, waiting.as_ref(), track);
                }
                kept_wait = Some(reason);
            }
            if fanned.admitted {
                decided = true;
                progressed = fanned.progressed;
                // A stale read shows that somebody else moved the case: no read of this fan-out
                // makes the revision the next iteration loads the Run's own (1, 8a).
                effected = fanned.performed && !fanned.stale;
            }
            stale = fanned.stale;
        }

        if !decided && !stale {
            let counted = verdict
                .as_ref()
                .map(|(capability, verdict)| CapabilityVerdict {
                    capability,
                    verdict,
                });
            if let Derived::Ended(derived) = derive(&determination, &handed, &outcome, counted) {
                // The case may have moved while the executor worked without its reading it (10).
                if proposes_nothing
                    && let Some(ended) =
                        moved_outcome(governor, effects, case, held, run.case_revision, &outcome)?
                {
                    return Ok(track.end(run_id, ended));
                }
                // With a suspension port, a run that ends on an approval the provider withholds is
                // suspended, and its Suspension stored, before the caller hears of it; a hold the
                // host settled stores none and puts the Run back `Running`.
                if let (
                    Some(suspending),
                    RunOutcome::NeedsAuthority(_),
                    Some(request),
                    Some((capability, AuthorityVerdict::ApprovalRequired(required))),
                ) = (suspensions, &derived, &pending, &verdict)
                {
                    let reason = SuspensionReason::Authority(Value::Object(vec![
                        ("capability".to_owned(), Value::Text(capability.clone())),
                        ("request".to_owned(), Value::Text(required.request.clone())),
                    ]));
                    suspend_for_approval(runs, suspending, run, request, reason)?;
                }
                return Ok(track.end(run_id, derived));
            }
        }

        // Where an idle bound applies (without a step budget, or one the stop policy sets), an
        // iteration that made no progress is judged against it; a stale one is not counted, since
        // the next iteration judges the move. A case this iteration's performed effect completed
        // comes first: the case is loaded before the bound ends the run, as at the step budget
        // (0), and a complete one ends it completed. An iteration whose request the port answered
        // `Pending` is not judged and leaves the count as it was: it is judged once the port
        // answers otherwise (8b).
        if let Some(bound) = idle_bound
            && !stale
            && !progressed
            && waiting.is_none()
            && let Some(ended) =
                idle_judged(governor, case, idle.saturating_add(1), bound, effected)?
        {
            return Ok(track.end(run_id, ended));
        }
        after_stale = stale;
        if !stale && waiting.is_none() {
            idle = if progressed {
                0
            } else {
                idle.saturating_add(1)
            };
        }
        track.steps += 1;
        // A stop policy with the gate off never arms it: the next iteration runs the executor on
        // an unchanged frontier too, and the step is judged as any other (5).
        gate = if !policy.approval_gate || granted || awaited(&handed, &denied).is_empty() {
            None
        } else {
            Some(handed.data().actions.clone())
        };
    }
}

/// Releases `taken`, the Suspension a resume took for `request` (`ReleaseSuspension`): once the
/// request was performed, or once it was found stale and never will be. A release the port does
/// not answer `released` is [`LoopFailure::SuspensionNotReleased`].
fn release_taken<R: ?Sized>(
    suspensions: Option<Suspensions<'_, R>>,
    taken: SuspensionKey,
    request: &ActionRequestData,
) -> Result<(), LoopFailure> {
    let Some(Suspensions { port, .. }) = suspensions else {
        return Ok(());
    };
    match port.release(ReleaseSuspension {
        suspension: taken,
        action_request_id: request.action_request_id.clone(),
    }) {
        ReleaseSuspensionOutcome::Released { .. } => Ok(()),
        outcome => Err(LoopFailure::SuspensionNotReleased(Box::new(
            UnreleasedSuspension {
                action_request_id: request.action_request_id.clone(),
                outcome,
            },
        ))),
    }
}

/// The reason a Run is suspended for at a step budget of `limit` steps.
fn step_budget_reason(limit: usize) -> SuspensionReason {
    SuspensionReason::Budget(Value::Object(vec![(
        "max_steps".to_owned(),
        Value::Number(limit.to_string()),
    )]))
}

/// Suspends the Run for `reason`, a budget it reached, and ends the loop as [`judge_budget_stop`]
/// answers (`JudgeBudgetStop`, `story:pending-effect`): `Waiting`, naming the requests `waiting`
/// holds and `reason`, when one is pending; otherwise suspended with `reason`, as before. A
/// judgement the generated behaviour cannot make is [`LoopFailure::Obligation`], the Run
/// suspended.
fn budget_ended<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    run_id: &RunId,
    reason: SuspensionReason,
    waiting: Option<&Waiting>,
    track: Track,
) -> Result<LoopEnd, Stopped> {
    let stopped = |failure: LoopFailure, reason: &SuspensionReason| Stopped {
        failure,
        suspended: Some((Box::new(reason.clone()), None)),
    };
    if let Err(failure) = suspend(runs, run_id, reason.clone()) {
        return Err(stopped(failure, &reason));
    }
    let judged = judge_budget_stop(JudgeBudgetStop {
        pending: waiting.map(Waiting::ids).unwrap_or_default(),
        reason: reason.clone(),
    });
    let outcome = match judged {
        Ok(JudgeBudgetStopOutcome::Waiting { error }) => RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: error.action_request_ids,
            reason: error.reason,
        }),
        Ok(JudgeBudgetStopOutcome::Suspended) => {
            RunOutcome::Suspended(RunOutcomeSuspended { reason })
        }
        Err(unmet) => return Err(stopped(LoopFailure::Obligation(unmet), &reason)),
    };
    Ok(track.end(run_id, outcome))
}

/// Suspends the Run `run` for `reason` and holds the commission's Suspension, holding the Run and
/// `request`, through the port (`HoldSuspension`). The Suspension is not held when the Run could
/// not be suspended. A hold the host settled (`settled`) stores nothing: the Run is put back
/// `Running` ([`unsuspend`]) and the run ends as it would without a port.
fn suspend_for_approval<R: SuspendRunBehavior + ?Sized>(
    runs: &mut R,
    suspending: Suspensions<'_, R>,
    run: &RunStarted,
    request: &ActionRequestData,
    reason: SuspensionReason,
) -> Result<(), Stopped> {
    if let Err(failure) = suspend(runs, &run.run_id, reason.clone()) {
        return Err(Stopped {
            failure,
            suspended: Some((Box::new(reason), None)),
        });
    }
    let failure = match suspending.port.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: run.commission_id.clone(),
        },
        commission_id: run.commission_id.clone(),
        run_id: run.run_id.clone(),
        run_case_revision: run.case_revision,
        action_request_id: request.action_request_id.clone(),
        case_id: request.case_id.clone(),
        expected_case_revision: request.expected_case_revision,
        action: request.action.clone(),
        arguments: request.arguments.clone(),
        reason: reason.clone(),
    }) {
        HoldSuspensionOutcome::Held { .. } => return Ok(()),
        // Nothing is held, so nothing may leave the Run suspended: a resume finds no Suspension.
        HoldSuspensionOutcome::Settled { .. } => match (suspending.unsuspend)(runs, &run.run_id) {
            Ok(()) => return Ok(()),
            Err(failure) => failure,
        },
        HoldSuspensionOutcome::StorageFailed { error } => {
            LoopFailure::RunStorage(RunStorageFailed {
                reason: error.reason,
            })
        }
        other => LoopFailure::SuspensionNotHeld(Box::new(other)),
    };
    Err(Stopped {
        failure,
        suspended: Some((Box::new(reason), None)),
    })
}

/// Continues the Run `commission`'s stored Suspension holds, after the grant, as the module
/// documents: the Suspension is taken, the suspended request is performed once, the Suspension is
/// released, and the loop goes on until the Run ends. A run budget's deadline passed before the
/// take, or between the take and the invocation, performs nothing and ends the resumed Run
/// suspended for `RunBudget` (limit `Deadline`): the Suspension stays `Pending` in the first case
/// and is released in the second. A host's stop ([`LoopContext::host_stop`]), asked at both places
/// right after the deadline, does the same, the Run suspended for `HostStop`.
///
/// `runs` need not hold the Run: a store of a new process gets it back, `Suspended`, from the
/// Suspension.
///
/// # Errors
///
/// [`ResumeError::Refused`] when there is nothing to resume, the request is in doubt or may not be
/// performed now, or another resume took the Suspension first or the commission holds one for
/// another request by the time it is taken; [`ResumeError::Store`] or
/// [`ResumeError::Governor`] when a port fails before the Run resumes: nothing is performed, and
/// the Suspension stays stored as it was. [`ResumeError::Loop`] when the run budget's deadline is
/// not RFC 3339 ([`LoopFailure::InvalidDeadline`], before the Suspension is read), `ResumeRun`
/// fails, the Suspension cannot be taken (nothing is performed), or the resumed loop stops without
/// a run outcome.
// The ports of `run_until_blocked`, and the suspension port.
#[allow(clippy::too_many_arguments)]
pub fn resume_after_grant<G, E, A, F, R, C>(
    governor: &G,
    executor: &E,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    runs: &mut R,
    suspensions: &dyn SuspensionPort,
    context: &mut C,
) -> Result<LoopEnd, ResumeError>
where
    G: Governor + ObservationPort + ?Sized,
    E: AgentExecutor + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    R: RunStorage + SuspendRunBehavior + ResumeRunBehavior + ?Sized,
    C: LoopContext + ?Sized,
{
    let refused = |refusal| Err(ResumeError::Refused(refusal));
    // A deadline that names no instant, and a stop policy that lists a limit twice, are refused
    // before the Suspension is read or taken.
    let deadline = deadline_of(context.run_budget().as_ref())
        .map_err(|failure| ResumeError::Loop(unstarted(failure)))?;
    check_stop_policy(&context.stop_policy())
        .map_err(|failure| ResumeError::Loop(unstarted(failure)))?;
    let held = commission.data();
    let Some(stored) = suspensions
        .get(&held.commission_id)
        .map_err(ResumeError::Store)?
    else {
        return refused(ResumeRefusal::NothingSuspended);
    };
    if stored.state == SuspensionState::Resuming {
        return refused(ResumeRefusal::InDoubt {
            action_request_id: stored.data.action_request_id,
        });
    }
    let stored = stored.data;
    if stored.commission_id != held.commission_id
        || stored.suspension.commission_id != held.commission_id
        || stored.case_id != held.case_id
    {
        return refused(ResumeRefusal::OtherCommission);
    }
    let current = governor
        .current_revision(&held.case_id)
        .map_err(ResumeError::Governor)?;
    if current != stored.expected_case_revision {
        return refused(ResumeRefusal::CaseMoved {
            expected_case_revision: stored.expected_case_revision,
            current_case_revision: current,
        });
    }
    let request = ActionRequestData {
        action_request_id: stored.action_request_id.clone(),
        run_id: stored.run_id.clone(),
        case_id: stored.case_id.clone(),
        expected_case_revision: stored.expected_case_revision,
        action: stored.action.clone(),
        arguments: stored.arguments.clone(),
    };
    let answer = revalidate(governor, &ActionRequest::new(request.clone()))
        .map_err(ResumeError::Governor)?;
    match &answer {
        RevalidateActionRequestOutcome::Admitted => {}
        RevalidateActionRequestOutcome::Stale { error } => {
            return refused(ResumeRefusal::CaseMoved {
                expected_case_revision: error.expected_case_revision,
                current_case_revision: error.current_case_revision,
            });
        }
        RevalidateActionRequestOutcome::NotAdmitted { .. } => {
            return refused(ResumeRefusal::NotAdmitted(answer));
        }
        RevalidateActionRequestOutcome::NeedsAuthority { error } => {
            let check = check_authority(authority, commission, &request, &error.capability);
            if !check.allows() {
                return refused(ResumeRefusal::NotGranted(check));
            }
            // The provider allowed the request at its revision. A case somebody else moved while
            // it decided is at another one by now: nothing is taken or performed.
            let current = governor
                .current_revision(&held.case_id)
                .map_err(ResumeError::Governor)?;
            if current != request.expected_case_revision {
                return refused(ResumeRefusal::CaseMoved {
                    expected_case_revision: request.expected_case_revision,
                    current_case_revision: current,
                });
            }
        }
    }
    if !effects.performs(&request.action) {
        return refused(ResumeRefusal::NotPerformed);
    }

    let run_id = stored.run_id.clone();
    match RunStorage::get(runs, &run_id) {
        None => {
            let run = Run::new(RunData {
                run_id: run_id.clone(),
                commission_id: stored.commission_id.clone(),
                case_revision: stored.run_case_revision,
            });
            RunStorage::put(runs, AnyRun::Suspended(run.suspend()).snapshot());
        }
        Some(held_run)
            if held_run.data.commission_id != stored.commission_id
                || held_run.data.case_revision != stored.run_case_revision =>
        {
            return refused(ResumeRefusal::OtherCommission);
        }
        Some(_) => {}
    }
    let failed = |failure: LoopFailure| LoopError {
        run_id: Some(run_id.clone()),
        failure,
        suspension: None,
        suspension_reason: None,
    };
    match runs
        .resume_run(ResumeRun {
            run_id: run_id.clone(),
        })
        .map_err(|unmet| ResumeError::Loop(failed(LoopFailure::Obligation(unmet))))?
    {
        ResumeRunOutcome::Resumed { .. } => {}
        ResumeRunOutcome::StorageFailed { error } => {
            return Err(ResumeError::Loop(failed(LoopFailure::RunStorage(error))));
        }
        other => return refused(ResumeRefusal::RunNotSuspended(other)),
    }
    // A deadline the trusted clock reads as reached, or else the host's own stop, takes nothing:
    // the Suspension stays `Pending`, and the Run is suspended for it, as before an executor call.
    if let Some(reason) = stopped_before_step(deadline, context) {
        let ended = budget_ended(runs, &run_id, reason, None, Track::default());
        return finish(runs, run_id, ended).map_err(ResumeError::Loop);
    }
    // Taken before anything is performed, for the request read and revalidated above: of two
    // resumes one takes it, a Suspension a resume took and never released is never taken again,
    // and one held since for another request is not taken. Whatever the take answers but `taken`,
    // the Run is suspended again with its stored reason and nothing is performed.
    let key = stored.suspension.clone();
    let failure = match suspensions.take(TakeSuspension {
        suspension: key.clone(),
        action_request_id: stored.action_request_id.clone(),
    }) {
        TakeSuspensionOutcome::Taken { .. } => None,
        TakeSuspensionOutcome::OtherRequest { error } => Some(Err(ResumeRefusal::OtherRequest {
            action_request_id: error.action_request_id,
            held_action_request_id: error.held_action_request_id,
        })),
        TakeSuspensionOutcome::NotFound { .. } => Some(Err(ResumeRefusal::NothingSuspended)),
        TakeSuspensionOutcome::WrongState { error } => Some(Err(ResumeRefusal::InDoubt {
            action_request_id: error.action_request_id,
        })),
        TakeSuspensionOutcome::StorageFailed { error } => {
            Some(Ok(LoopFailure::RunStorage(RunStorageFailed {
                reason: error.reason,
            })))
        }
    };
    if let Some(failure) = failure {
        let suspension = suspend(runs, &run_id, stored.reason.clone()).err();
        return Err(match failure {
            Err(refusal) => ResumeError::Refused(refusal),
            Ok(failure) => ResumeError::Loop(LoopError {
                run_id: Some(run_id),
                failure,
                suspension: suspension.map(Box::new),
                suspension_reason: Some(Box::new(stored.reason)),
            }),
        });
    }

    let ports = Ports {
        governor,
        executor,
        authority,
        effects,
        suspensions: Some(Suspensions::new(suspensions)),
    };
    let run = RunStarted {
        run_id: run_id.clone(),
        commission_id: stored.commission_id,
        case_revision: stored.run_case_revision,
    };
    let result = iterate(
        &ports,
        commission,
        runs,
        context,
        &run,
        stored.expected_case_revision,
        Some(Resumed {
            request,
            answer,
            taken: key,
        }),
    );
    finish(runs, run_id, result).map_err(ResumeError::Loop)
}

/// How a run ends when the effect port performs none of the actions it needs
/// (`JudgePerformableActions`, `story:unperformable-names-actions`): `NoPerformableAction` when
/// [`judge_performable`] answers `unperformable`, naming the needed actions the port does not
/// perform, each once, in the order given, and the claims of `frontier` whose value is not `True`,
/// in frontier order; `None` when it answers `performable`, and the run goes on. `needed` are every
/// action `frontier` lists, in frontier order, before the executor runs (items 4 and 9), or the
/// action of an admitted request (item 8). A judgement the generated behaviour cannot make is
/// [`LoopFailure::Obligation`].
fn unperformable_end<'a, F: EffectPort + ?Sized>(
    needed: impl IntoIterator<Item = &'a str>,
    frontier: &FrontierData,
    effects: &F,
) -> Result<Option<RunOutcome>, LoopFailure> {
    let mut seen: Vec<&str> = Vec::new();
    let mut performed: i64 = 0;
    let mut unperformed = Vec::new();
    for action in needed {
        if seen.contains(&action) {
            continue;
        }
        seen.push(action);
        if effects.performs(action) {
            performed = performed.saturating_add(1);
        } else {
            unperformed.push(action.to_owned());
        }
    }
    let open_claims = frontier
        .claims
        .iter()
        .filter(|listed| listed.value != Truth::True)
        .map(|listed| listed.claim.clone())
        .collect();
    Ok(
        match judge_performable(JudgePerformableActions {
            performed,
            unperformed,
            open_claims,
        })? {
            JudgePerformableActionsOutcome::Unperformable { error } => Some(
                RunOutcome::NoPerformableAction(RunOutcomeNoPerformableAction {
                    actions: error.actions,
                    open_claims: error.open_claims,
                }),
            ),
            JudgePerformableActionsOutcome::Performable => None,
        },
    )
}

/// Every action `frontier` lists, in frontier order, an action listed more than once each time.
fn listed_actions(frontier: &FrontierData) -> impl Iterator<Item = &str> {
    frontier.actions.iter().map(|listed| listed.action.as_str())
}

/// Whether a fan-out of `reads` is admitted (item 8a): it lists at least one and at most
/// [`MAX_FAN_OUT`] reads, no two with the same action and arguments, and each names an action the
/// effect port performs and reads ([`EffectPort::reads`]) that the `handed` frontier lists, every
/// entry of it `Admissible` and naming no capability.
fn fans_out<F: EffectPort + ?Sized>(
    reads: &[ExecutorOutcomeProposedAction],
    handed: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> bool {
    // Two reads with the same action and arguments would invoke one read twice in one step.
    let distinct = reads.iter().enumerate().all(|(at, read)| {
        reads[..at]
            .iter()
            .all(|earlier| earlier.action != read.action || earlier.arguments != read.arguments)
    });
    (1..=MAX_FAN_OUT).contains(&reads.len())
        && distinct
        && reads
            .iter()
            .all(|read| fan_out_read(&read.action, handed, effects))
}

/// Whether `action` may be read in a fan-out (items 8a, 8b): the effect port performs and reads it
/// ([`EffectPort::reads`]), and the `handed` frontier lists it, every entry of it `Admissible`
/// and naming no capability.
fn fan_out_read<F: EffectPort + ?Sized>(
    action: &str,
    handed: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> bool {
    let mut listed = handed
        .data()
        .actions
        .iter()
        .filter(|entry| entry.action == action)
        .peekable();
    effects.performs(action)
        && effects.reads(action)
        && listed.peek().is_some()
        && listed.all(|entry| {
            entry.status == ActionStatus::Admissible
                && entry.capability.as_deref().is_none_or(str::is_empty)
        })
}

/// How a pending read that [`fan_out_read`] no longer allows on `handed` is recorded, though its
/// revalidation admitted it (item 8b): as needing the capability an entry of it names, since a
/// read of a fan-out is never invoked on an authority (8a); otherwise as not admitted, with the
/// reasons its entries give.
fn refused_read(
    action: &str,
    handed: &Frontier<frontier_state::Issued>,
) -> RevalidateActionRequestOutcome {
    let entries = || {
        handed
            .data()
            .actions
            .iter()
            .filter(move |entry| entry.action == action)
    };
    match entries().find_map(|entry| entry.capability.clone().filter(|named| !named.is_empty())) {
        Some(capability) => RevalidateActionRequestOutcome::NeedsAuthority {
            error: ActionNeedsAuthority {
                action: action.to_owned(),
                capability,
            },
        },
        None => RevalidateActionRequestOutcome::NotAdmitted {
            error: ActionNotAdmitted {
                action: action.to_owned(),
                reasons: entries().flat_map(|entry| entry.reasons.clone()).collect(),
            },
        },
    }
}

/// What one admitted fan-out did.
struct FannedOut {
    /// Whether any read was admitted, and so invoked.
    admitted: bool,
    /// Whether any admitted read has the action and arguments of no request admitted before it.
    progressed: bool,
    /// Whether any effect was `Performed`.
    performed: bool,
    /// Whether some revalidation answered stale: the case moved under somebody else.
    stale: bool,
    /// The trial limit the fan-out would have passed, so none of its reads was invoked.
    over_budget: Option<RunBudgetBreach>,
    /// How the run ends when the port refused a read as final: the first such read, in the
    /// order proposed ([`refusal_end`]). It ends the run only when no read was `stale`.
    ended: Option<RunOutcome>,
    /// The reads the port answered `Pending`, in the order proposed, which the next iteration
    /// invokes again (8b).
    pending: Vec<ActionRequestData>,
    /// Why the run waits when the port answered a read `Pending` with `ends_run`: the first such,
    /// in the order proposed ([`wait_reason`]). No read is invoked again then.
    wait: Option<SuspensionReason>,
}

/// Performs the fan-out `reads`, which [`fans_out`] admitted, as item 8a documents: each read
/// becomes its own request at `frontier`'s revision and is revalidated, all before any is invoked;
/// the admitted ones are handed to [`EffectPort::invoke_reads`] together, and their effects are
/// recorded and observed in the order proposed. An `Err` from a read fails the loop once every
/// read has answered and each that answered was recorded and observed; the first `Err` in proposal
/// order is the failure. Nothing is retried, and no authority is asked.
// The ports, the loop's state and the proposal.
#[allow(clippy::too_many_arguments)]
fn fan_out<G, F, C>(
    governor: &G,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    context: &mut C,
    run_id: &RunId,
    frontier: &Frontier<frontier_state::Issued>,
    reads: &[ExecutorOutcomeProposedAction],
    budget: Option<&RunBudget>,
    track: &mut Track,
) -> Result<FannedOut, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    F: EffectPort + ?Sized,
    C: LoopContext + ?Sized,
{
    let mut admitted: Vec<ActionRequestData> = Vec::new();
    let mut stale = false;
    for read in reads {
        let made = request(
            context.action_request_id(),
            run_id.clone(),
            frontier,
            read.clone(),
        );
        let answer = revalidate(governor, &made)?;
        let data = made.into_data();
        track.requests.push(Revalidated {
            request: data.clone(),
            outcome: answer.clone(),
        });
        match answer {
            RevalidateActionRequestOutcome::Admitted => admitted.push(data),
            RevalidateActionRequestOutcome::Stale { .. } => stale = true,
            // A read that needs authority is never performed in a fan-out.
            RevalidateActionRequestOutcome::NeedsAuthority { .. }
            | RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
        }
    }
    if admitted.is_empty() {
        return Ok(FannedOut {
            admitted: false,
            progressed: false,
            performed: false,
            stale,
            over_budget: None,
            ended: None,
            pending: Vec::new(),
            wait: None,
        });
    }
    // A fan-out that would take an action past its trial limit invokes none of its reads.
    let actions: Vec<&str> = admitted.iter().map(|data| data.action.as_str()).collect();
    if let Some(breach) = budget.and_then(|limits| trials_past(limits, track, &actions)) {
        return Ok(FannedOut {
            admitted: false,
            progressed: false,
            performed: false,
            stale,
            over_budget: Some(breach),
            ended: None,
            pending: Vec::new(),
            wait: None,
        });
    }
    let progressed = admitted.iter().any(|data| {
        !track
            .admitted
            .iter()
            .any(|earlier| earlier.action == data.action && earlier.arguments == data.arguments)
    });
    track.admitted.extend(admitted.iter().cloned());
    let answers = invoke_together(effects, commission, &admitted)?;
    let answered = record_answers(governor, context, track, &admitted, answers)?;
    Ok(FannedOut {
        admitted: true,
        progressed,
        performed: answered.performed,
        stale,
        over_budget: None,
        ended: answered.ended,
        pending: answered.pending,
        wait: answered.wait,
    })
}

/// Hands `reads`, admitted reads, to [`EffectPort::invoke_reads`] together and answers what each
/// returned, in order. A port that answers `None`, or another number of answers, fails the loop
/// as an effect failure.
fn invoke_together<F: EffectPort + ?Sized>(
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    reads: &[ActionRequestData],
) -> Result<Vec<Result<EffectOutcome, EffectError>>, Stopped> {
    let requests: Vec<AdmittedRequest> = reads.iter().cloned().map(AdmittedRequest::new).collect();
    match effects.invoke_reads(commission, &requests) {
        Some(answers) if answers.len() == requests.len() => Ok(answers),
        Some(answers) => Err(LoopFailure::Effect(EffectError::new(format!(
            "the effect port answered {} of {} reads",
            answers.len(),
            requests.len()
        )))
        .into()),
        None => Err(LoopFailure::Effect(EffectError::new(
            "the effect port reads the actions proposed but invokes no fan-out",
        ))
        .into()),
    }
}

/// What the effect port answered for requests handed to it together, once every answer was
/// recorded and observed.
struct Answered {
    /// Whether any effect was `Performed`.
    performed: bool,
    /// The requests answered `Pending`, in order.
    pending: Vec<ActionRequestData>,
    /// How the run ends when the port refused one as final: the first such, in order
    /// ([`refusal_end`]).
    ended: Option<RunOutcome>,
    /// Why the run waits when the port answered one `Pending` with `ends_run`: the first such, in
    /// order ([`wait_reason`]).
    wait: Option<SuspensionReason>,
}

/// Records and observes, in the order of `handed`, what the effect port answered for each
/// (`answers`, in the same order). An `Err` fails the loop once every answer that came was
/// recorded and observed; the first `Err` in order is the failure.
fn record_answers<G, C>(
    governor: &G,
    context: &mut C,
    track: &mut Track,
    handed: &[ActionRequestData],
    answers: Vec<Result<EffectOutcome, EffectError>>,
) -> Result<Answered, Stopped>
where
    G: ObservationPort + ?Sized,
    C: LoopContext + ?Sized,
{
    let mut answered = Answered {
        performed: false,
        pending: Vec::new(),
        ended: None,
        wait: None,
    };
    let mut failed: Option<EffectError> = None;
    for (data, answer) in handed.iter().zip(answers) {
        match answer {
            Ok(effect) => {
                answered.performed |= matches!(effect, EffectOutcome::Performed(_));
                track.record(data, &effect);
                governor.observe(effect_observation(context, data, &effect))?;
                if matches!(effect, EffectOutcome::Pending(_)) {
                    answered.pending.push(data.clone());
                }
                if answered.ended.is_none() {
                    answered.ended = refusal_end(data, &effect)?;
                }
                if answered.wait.is_none() {
                    answered.wait = wait_reason(&effect);
                }
            }
            Err(error) => {
                failed.get_or_insert(error);
            }
        }
    }
    match failed {
        Some(error) => Err(LoopFailure::Effect(error).into()),
        None => Ok(answered),
    }
}

/// What invoking the pending requests again did (item 8b).
struct InvokedAgain {
    /// The requests revalidation admitted again, in order: handed to the port, unless
    /// `over_budget` names the trial limit that kept them back.
    admitted: Vec<ActionRequestData>,
    /// Whether some revalidation answered stale, or the case moved while the provider decided:
    /// somebody else moved the case.
    stale: bool,
    /// Whether the one request needed authority and the provider allowed it.
    allowed: bool,
    /// The trial limit handing `admitted` to the port would pass, so nothing was invoked.
    over_budget: Option<RunBudgetBreach>,
    /// Whether any effect was `Performed`.
    performed: bool,
    /// The requests the port answered `Pending` again, in order.
    pending: Vec<ActionRequestData>,
    /// How the run ends when the port refused one as final ([`refusal_end`]).
    ended: Option<RunOutcome>,
    /// Why the run waits when the port answered one `Pending` with `ends_run`: the first such,
    /// in order ([`wait_reason`]).
    wait: Option<SuspensionReason>,
    /// The one request the provider answered `ApprovalRequired` for when asked again: the run
    /// ends needing authority, as at a first `ApprovalRequired` (8).
    withheld: Option<Withheld>,
}

/// A pending request whose authority the provider withheld when it was asked again (item 8b).
struct Withheld {
    /// The pending request, as the run made it.
    request: ActionRequestData,
    /// The capability the frontier names for its action.
    capability: String,
    /// What the provider's `ApprovalRequired` asks to be approved.
    approval: String,
}

/// Invokes `waiting`'s requests again, as item 8b documents. Each is revalidated first, as before
/// its first invocation, under its own id and at its own revision: a request that still needs
/// authority is asked about again, and once the provider allows it the case is loaded again, a
/// case at another revision making it stale (8). One revalidation does not admit is recorded with
/// its answer and is not invoked. A provider's deny is remembered as in 8. A read of a fan-out is
/// invoked again only while the fan-out rule of item 8a still holds for it on `handed`, the
/// frontier the iteration read ([`fan_out_read`]): one it no longer holds for is recorded as
/// [`refused_read`] answers and not invoked, so a read is never invoked on an authority. The
/// admitted ones are checked against their actions' trial limits and handed to the port
/// together: reads through [`EffectPort::invoke_reads`], the one request otherwise through
/// [`EffectPort::invoke`]. Every answer is recorded and observed in order.
// The ports, the loop's state, the frontier the iteration read and the pending requests.
#[allow(clippy::too_many_arguments)]
fn invoke_again<G, A, F, C>(
    governor: &G,
    authority: &A,
    effects: &F,
    commission: &Commission<commission_state::Assigned>,
    context: &mut C,
    handed: &Frontier<frontier_state::Issued>,
    waiting: &Waiting,
    budget: Option<&RunBudget>,
    track: &mut Track,
    denied: &mut Vec<String>,
) -> Result<InvokedAgain, Stopped>
where
    G: Governor + ObservationPort + ?Sized,
    A: AuthorityProvider + ?Sized,
    F: EffectPort + ?Sized,
    C: LoopContext + ?Sized,
{
    let mut again = InvokedAgain {
        admitted: Vec::new(),
        stale: false,
        allowed: false,
        over_budget: None,
        performed: false,
        pending: Vec::new(),
        ended: None,
        wait: None,
        withheld: None,
    };
    for data in &waiting.requests {
        let mut answer = revalidate(governor, &ActionRequest::new(data.clone()))?;
        let mut admitted = false;
        match &answer {
            // A read is admitted again only as a read of a fan-out is (8a).
            RevalidateActionRequestOutcome::Admitted
                if waiting.reads && !fan_out_read(&data.action, handed, effects) =>
            {
                answer = refused_read(&data.action, handed);
            }
            RevalidateActionRequestOutcome::Admitted => admitted = true,
            RevalidateActionRequestOutcome::Stale { .. } => again.stale = true,
            RevalidateActionRequestOutcome::NeedsAuthority { error } if !waiting.reads => {
                if let AuthorityCheck::Decided(verdict) =
                    check_authority(authority, commission, data, &error.capability)
                {
                    match verdict {
                        AuthorityVerdict::Allow(Unit(true)) => {
                            let current = governor.current_revision(&data.case_id)?;
                            if current == data.expected_case_revision {
                                admitted = true;
                                again.allowed = true;
                            } else {
                                again.stale = true;
                                answer = RevalidateActionRequestOutcome::Stale {
                                    error: ActionRequestStale {
                                        expected_case_revision: data.expected_case_revision,
                                        current_case_revision: current,
                                    },
                                };
                            }
                        }
                        AuthorityVerdict::Deny(_) => denied.push(error.action.clone()),
                        AuthorityVerdict::ApprovalRequired(required) => {
                            again.withheld = Some(Withheld {
                                request: data.clone(),
                                capability: error.capability.clone(),
                                approval: required.request,
                            });
                        }
                        AuthorityVerdict::Allow(_) => {}
                    }
                }
            }
            RevalidateActionRequestOutcome::NeedsAuthority { .. }
            | RevalidateActionRequestOutcome::NotAdmitted { .. } => {}
        }
        track.requests.push(Revalidated {
            request: data.clone(),
            outcome: answer,
        });
        if admitted {
            again.admitted.push(data.clone());
        }
    }
    if again.admitted.is_empty() {
        return Ok(again);
    }
    let actions: Vec<&str> = again
        .admitted
        .iter()
        .map(|data| data.action.as_str())
        .collect();
    if let Some(breach) = budget.and_then(|limits| trials_past(limits, track, &actions)) {
        again.over_budget = Some(breach);
        return Ok(again);
    }
    track.admitted.extend(again.admitted.iter().cloned());
    let answers = if waiting.reads {
        invoke_together(effects, commission, &again.admitted)?
    } else {
        again
            .admitted
            .iter()
            .map(|data| effects.invoke(commission, &AdmittedRequest::new(data.clone())))
            .collect()
    };
    let answered = record_answers(governor, context, track, &again.admitted, answers)?;
    again.performed = answered.performed;
    again.pending = answered.pending;
    again.ended = answered.ended;
    again.wait = answered.wait;
    Ok(again)
}

/// Why the run waits when the effect port answered `effect` `Pending` with `ends_run`
/// (`story:pending-answer-ends-run`): `Time`, carrying the port's report; `None` for any other
/// answer, a `Pending` one without `ends_run` included, and the request is invoked again (8b).
/// Called once the answer was recorded and observed.
fn wait_reason(effect: &EffectOutcome) -> Option<SuspensionReason> {
    match effect {
        EffectOutcome::Pending(pending) if pending.ends_run => {
            Some(SuspensionReason::Time(pending.report.clone()))
        }
        _ => None,
    }
}

/// How the run ends after the effect port answered `effect` for `request`: with `EffectRefused`,
/// naming the request and the port's reason, when `effect` is a refusal [`judge_refusal`] answers
/// `final` for, as the port marked it `ends_run`; `None` for a performed effect and an ordinary
/// refusal, and the run goes on. Called once the answer was recorded and observed. A judgement the
/// generated behaviour cannot make is [`LoopFailure::Obligation`].
fn refusal_end(
    request: &ActionRequestData,
    effect: &EffectOutcome,
) -> Result<Option<RunOutcome>, LoopFailure> {
    let EffectOutcome::Refused(refused) = effect else {
        return Ok(None);
    };
    Ok(
        match judge_refusal(JudgeEffectRefusal {
            action_request_id: request.action_request_id.clone(),
            reason: refused.reason.clone(),
            ends_run: refused.ends_run,
        })? {
            JudgeEffectRefusalOutcome::Final { error } => {
                Some(RunOutcome::EffectRefused(RunOutcomeEffectRefused {
                    action_request_id: error.action_request_id,
                    reason: error.reason,
                }))
            }
            JudgeEffectRefusalOutcome::Ordinary => None,
        },
    )
}

/// How a run its idle bound applies to ends after `idle_iterations` idle iterations in a row, this
/// one included, against `bound` (`JudgeIdleIteration`, `story:idle-bound-outcome`,
/// `story:stop-policy-gate-and-idle-bound`): `IdleBound`, naming them, when [`judge_idle`] answers
/// `bound`; `None` when it answers `below`, and the run goes on.
fn idle_end(idle_iterations: i64, bound: i64) -> Result<Option<RunOutcome>, LoopFailure> {
    let judged = judge_idle(JudgeIdleIteration {
        idle_iterations,
        bound,
    })?;
    Ok(match judged {
        JudgeIdleIterationOutcome::Bound { error } => {
            Some(RunOutcome::IdleBound(RunOutcomeIdleBound {
                idle_iterations: error.idle_iterations,
            }))
        }
        JudgeIdleIterationOutcome::Below => None,
    })
}

/// How a run its idle bound applies to ends after an iteration that made no progress,
/// `idle_iterations` in a row with it, against `bound` ([`idle_end`]), or `None` when it goes on. When the bound ends the run and the
/// iteration's own effect was performed (`effected`), the case is loaded first, as at the step
/// budget (0), and a case the governor holds complete ends the run completed, carrying the
/// governor's outcome.
fn idle_judged<G: Governor + ?Sized>(
    governor: &G,
    case: &CaseId,
    idle_iterations: i64,
    bound: i64,
    effected: bool,
) -> Result<Option<RunOutcome>, Stopped> {
    let Some(ended) = idle_end(idle_iterations, bound)? else {
        return Ok(None);
    };
    if effected {
        governor.current_revision(case)?;
        if let CompletionDetermination::Complete(complete) = governor.completion(case)? {
            return Ok(Some(RunOutcome::Completed(RunOutcomeCompleted {
                outcome: complete.outcome,
            })));
        }
    }
    Ok(Some(ended))
}

/// How a run ends after its executor reported `CaseMoved` (item 9), or proposed nothing on a
/// frontier that would end the run (item 10), or `None` when the governor does not bear a move
/// out: it holds `case` at `held`, the revision the loop holds it at. Otherwise the case is judged
/// by [`moved_judged`] at the revision just loaded.
fn moved_outcome<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    held: i64,
    bound: i64,
    reported: &ExecutorOutcome,
) -> Result<Option<RunOutcome>, LoopFailure>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    let loaded = governor.current_revision(case)?;
    if loaded == held {
        return Ok(None);
    }
    let determination = governor.completion(case)?;
    moved_judged(
        governor,
        effects,
        case,
        bound,
        loaded,
        &determination,
        reported,
    )
    .map(Some)
}

/// How a run ends whose case the runtime found moved to `loaded` when it loaded it (item 2), the
/// governor not holding it complete (`determination`): with no admissible action when the frontier
/// current then is for another revision than `loaded`; `NoPerformableAction` when it lists
/// actions, none of which the effect port performs, naming them and its open claims as a move the
/// governor bears out does (item 9, `story:unperformable-names-actions`); `CaseMovedOn` when, as
/// the executor would be handed it, it admits an action; otherwise with no admissible action. The
/// Run is bound to `bound`, its own revision.
fn found_moved<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    bound: i64,
    loaded: i64,
    determination: &CompletionDetermination,
) -> Result<RunOutcome, LoopFailure>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    let frontier = governor.frontier(case)?;
    if frontier.data().case_revision != loaded {
        return Ok(RunOutcome::NoAdmissibleAction(Unit(true)));
    }
    if let Some(ended) =
        unperformable_end(listed_actions(frontier.data()), frontier.data(), effects)?
    {
        return Ok(ended);
    }
    let admits = matches!(
        derive(
            determination,
            &offered(&frontier, effects),
            &ExecutorOutcome::NoUsefulAction(Unit(true)),
            None,
        ),
        Derived::Continue
    );
    Ok(if admits {
        case_moved_on(bound, loaded)
    } else {
        RunOutcome::NoAdmissibleAction(Unit(true))
    })
}

/// `CaseMovedOn` from the Run's revision `bound` to `loaded`.
fn case_moved_on(bound: i64, loaded: i64) -> RunOutcome {
    RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
        bound_case_revision: bound,
        current_case_revision: loaded,
    })
}

/// How a run whose case moved to `loaded` ends, judged on the case as the governor holds it now
/// (`determination`, then the frontier current then, as the executor would be handed it). A case
/// the governor holds complete ends the run completed, with no frontier read; a frontier for
/// another revision than `loaded` ends it with no admissible action; one that lists actions, none
/// of which the effect port performs, with `NoPerformableAction`. Otherwise the outcome is derived
/// from that frontier, and where it would let the run go on, the run ends `CaseMovedOn`: the Run is
/// bound to `bound`, its own revision, and the caller starts a new Run at `loaded`.
fn moved_judged<G, F>(
    governor: &G,
    effects: &F,
    case: &CaseId,
    bound: i64,
    loaded: i64,
    determination: &CompletionDetermination,
    reported: &ExecutorOutcome,
) -> Result<RunOutcome, LoopFailure>
where
    G: Governor + ?Sized,
    F: EffectPort + ?Sized,
{
    if let CompletionDetermination::Complete(complete) = determination {
        return Ok(RunOutcome::Completed(RunOutcomeCompleted {
            outcome: complete.outcome.clone(),
        }));
    }
    let frontier = governor.frontier(case)?;
    if frontier.data().case_revision != loaded {
        return Ok(RunOutcome::NoAdmissibleAction(Unit(true)));
    }
    if let Some(ended) =
        unperformable_end(listed_actions(frontier.data()), frontier.data(), effects)?
    {
        return Ok(ended);
    }
    let handed = offered(&frontier, effects);
    Ok(match derive(determination, &handed, reported, None) {
        Derived::Ended(outcome) => outcome,
        Derived::Continue => case_moved_on(bound, loaded),
    })
}

/// The frontier an executor is handed: `frontier` less every action the effect port does not
/// perform and that needs no authority. An action behind an authority gate stays, performed or not
/// (`decision-blocker:gated-unbound-action-visibility`, B). The filter works by action, not by
/// entry: an action the frontier lists more than once is kept or dropped with every entry it has,
/// and it needs authority when any of its entries is `ApprovalRequired` or names a capability, as
/// the local slice's `repository.merge` does while it is still `Blocked`. So each kept action is
/// admitted exactly as the governor's frontier admits it. Same id, case and revision.
fn offered<F: EffectPort + ?Sized>(
    frontier: &Frontier<frontier_state::Issued>,
    effects: &F,
) -> Frontier<frontier_state::Issued> {
    let listed = &frontier.data().actions;
    let gated = |action: &str| {
        listed.iter().any(|entry| {
            entry.action == action
                && (entry.status == ActionStatus::ApprovalRequired
                    || entry
                        .capability
                        .as_deref()
                        .is_some_and(|capability| !capability.is_empty()))
        })
    };
    let mut data = frontier.data().clone();
    data.actions
        .retain(|entry| effects.performs(&entry.action) || gated(&entry.action));
    Frontier::new(data)
}

/// The actions `frontier` lists as needing approval that no verdict in this Run denied, in its
/// order.
fn awaited(frontier: &Frontier<frontier_state::Issued>, denied: &[String]) -> Vec<String> {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.status == ActionStatus::ApprovalRequired)
        .filter(|listed| !denied.contains(&listed.action))
        .map(|listed| listed.action.clone())
        .collect()
}

/// Whether `after` lists the same actions as `before`, each with the same status and reasons.
fn unchanged(before: &[FrontierAction], after: &[FrontierAction]) -> bool {
    before.len() == after.len()
        && before.iter().zip(after).all(|(was, is)| {
            was.action == is.action && was.status == is.status && was.reasons == is.reasons
        })
}

/// The observation of one executor step, as the module documents: `outcome`; `usage`, what the
/// call reported it spent, when it reported any; and the step's `transcript_digest` and `summary`,
/// each when the step stated it.
fn observation<C: LoopContext + ?Sized>(
    context: &mut C,
    frontier: &Frontier<frontier_state::Issued>,
    step: &ExecutorStep,
) -> Observation<observation_state::Reported> {
    let frontier = frontier.data();
    let outcome = &step.outcome;
    let text = |text: &str| Value::Text(text.to_owned());
    let mut payload = match outcome {
        ExecutorOutcome::ProposedAction(proposed) => vec![
            ("outcome".to_owned(), text("ProposedAction")),
            ("action".to_owned(), text(&proposed.action)),
            ("arguments".to_owned(), proposed.arguments.0.clone()),
        ],
        ExecutorOutcome::ProposedReads(proposed) => vec![
            ("outcome".to_owned(), text("ProposedReads")),
            (
                "reads".to_owned(),
                Value::Array(
                    proposed
                        .reads
                        .iter()
                        .map(|read| {
                            Value::Object(vec![
                                ("action".to_owned(), text(&read.action)),
                                ("arguments".to_owned(), read.arguments.0.clone()),
                            ])
                        })
                        .collect(),
                ),
            ),
        ],
        ExecutorOutcome::NeedsHumanJudgment(needs) => vec![
            ("outcome".to_owned(), text("NeedsHumanJudgment")),
            ("request".to_owned(), needs.request.0.clone()),
        ],
        ExecutorOutcome::Suspended(_) => vec![("outcome".to_owned(), text("Suspended"))],
        ExecutorOutcome::NoUsefulAction(_) => vec![("outcome".to_owned(), text("NoUsefulAction"))],
        ExecutorOutcome::CompletedLocalReasoning(_) => {
            vec![("outcome".to_owned(), text("CompletedLocalReasoning"))]
        }
        ExecutorOutcome::CaseMoved(moved) => vec![
            ("outcome".to_owned(), text("CaseMoved")),
            (
                "expected_case_revision".to_owned(),
                Value::Number(moved.expected_case_revision.to_string()),
            ),
        ],
    };
    if let Some(usage) = &step.usage {
        payload.push((
            "usage".to_owned(),
            spent_value(
                usage.input_tokens,
                usage.output_tokens,
                usage.duration_ms,
                usage.cost_micro_usd,
            ),
        ));
    }
    for (name, stated) in [
        ("transcript_digest", &step.transcript_digest),
        ("summary", &step.summary),
    ] {
        if let Some(stated) = stated {
            payload.push((name.to_owned(), text(stated)));
        }
    }
    Observation::new(ObservationData {
        observation_id: context.observation_id(),
        source: EXECUTOR_SOURCE.to_owned(),
        subject: format!("{}@{}", frontier.case_id.0, frontier.case_revision),
        observed_at: context.now(),
        payload: Value::Object(payload),
    })
}

/// The observation of one effect, as the module documents.
fn effect_observation<C: LoopContext + ?Sized>(
    context: &mut C,
    request: &ActionRequestData,
    effect: &EffectOutcome,
) -> Observation<observation_state::Reported> {
    let text = |text: &str| Value::Text(text.to_owned());
    let mut payload = vec![
        (
            "outcome".to_owned(),
            text(match effect {
                EffectOutcome::Performed(_) => "Performed",
                EffectOutcome::Refused(_) => "Refused",
                EffectOutcome::Pending(_) => "Pending",
            }),
        ),
        (
            "action_request".to_owned(),
            text(&request.action_request_id.0.0),
        ),
        ("action".to_owned(), text(&request.action)),
        ("arguments".to_owned(), request.arguments.0.clone()),
    ];
    match effect {
        EffectOutcome::Performed(performed) => {
            payload.push(("report".to_owned(), performed.report.clone()));
            if let Some(attempt) = &performed.attempt {
                payload.push(("attempt".to_owned(), text(&attempt.0)));
            }
            if let Some(audit) = &performed.audit {
                payload.push(("audit".to_owned(), text(&audit.0)));
            }
            if let Some(usage) = &performed.usage {
                payload.push((
                    "usage".to_owned(),
                    spent_value(
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.duration_ms,
                        usage.cost_micro_usd,
                    ),
                ));
            }
        }
        EffectOutcome::Refused(refused) => {
            payload.push(("reason".to_owned(), text(&refused.reason)));
        }
        EffectOutcome::Pending(pending) => {
            payload.push(("report".to_owned(), pending.report.clone()));
        }
    }
    Observation::new(ObservationData {
        observation_id: context.observation_id(),
        source: EFFECT_SOURCE.to_owned(),
        subject: format!("{}@{}", request.case_id.0, request.expected_case_revision),
        observed_at: context.now(),
        payload: Value::Object(payload),
    })
}

/// A reported usage as an observation carries it: the input and output tokens, the wall time and,
/// when one was reported, the cost.
fn spent_value(
    input_tokens: i64,
    output_tokens: i64,
    duration_ms: i64,
    cost_micro_usd: Option<i64>,
) -> Value {
    let figure = |figure: i64| Value::Number(figure.to_string());
    let mut spent = vec![
        ("input_tokens".to_owned(), figure(input_tokens)),
        ("output_tokens".to_owned(), figure(output_tokens)),
        ("duration_ms".to_owned(), figure(duration_ms)),
    ];
    if let Some(cost) = cost_micro_usd {
        spent.push(("cost_micro_usd".to_owned(), figure(cost)));
    }
    Value::Object(spent)
}

/// Nothing charged: no trial, no usage, and a known cost of zero.
fn uncharged() -> ChargedUsage {
    ChargedUsage {
        trials: 0,
        input_tokens: 0,
        output_tokens: 0,
        duration_ms: 0,
        cost_micro_usd: Some(0),
    }
}

/// Charges `effect`, the port's answer for a request for `action`, to the run and to `action`:
/// one trial, and the usage a `Performed` effect reported. A negative figure charges nothing, and
/// a usage that reports no cost leaves the cost unknown from then on.
fn charge(charges: &mut RunCharges, action: &str, effect: &EffectOutcome) {
    let usage = match effect {
        EffectOutcome::Performed(performed) => performed.usage.as_ref(),
        EffectOutcome::Refused(_) | EffectOutcome::Pending(_) => None,
    };
    let index = if let Some(index) = charges.actions.iter().position(|c| c.action == action) {
        index
    } else {
        charges.actions.push(ActionCharge {
            action: action.to_owned(),
            charged: uncharged(),
        });
        charges.actions.len() - 1
    };
    for charged in [&mut charges.run, &mut charges.actions[index].charged] {
        charged.trials = charged.trials.saturating_add(1);
        if let Some(usage) = usage {
            add_usage(
                charged,
                usage.input_tokens,
                usage.output_tokens,
                usage.duration_ms,
                usage.cost_micro_usd,
            );
        }
    }
}

/// Charges `usage`, what one executor call reported it spent, to the run alone
/// (`story:executor-reports-usage`): no trial and no action, the figures as [`charge`] adds an
/// effect's.
fn charge_step(charges: &mut RunCharges, usage: &ExecutorUsage) {
    add_usage(
        &mut charges.run,
        usage.input_tokens,
        usage.output_tokens,
        usage.duration_ms,
        usage.cost_micro_usd,
    );
}

/// Adds one reported usage to `charged`. A negative figure charges nothing, and a usage that
/// reports no cost leaves the cost unknown from then on.
fn add_usage(
    charged: &mut ChargedUsage,
    input_tokens: i64,
    output_tokens: i64,
    duration_ms: i64,
    cost_micro_usd: Option<i64>,
) {
    charged.input_tokens = charged.input_tokens.saturating_add(input_tokens.max(0));
    charged.output_tokens = charged.output_tokens.saturating_add(output_tokens.max(0));
    charged.duration_ms = charged.duration_ms.saturating_add(duration_ms.max(0));
    charged.cost_micro_usd = match (charged.cost_micro_usd, cost_micro_usd) {
        (Some(sum), Some(cost)) => Some(sum.saturating_add(cost.max(0))),
        _ => None,
    };
}

/// Every spending limit of `limits` that `charged` passed, for `action` (`None` for the run's), in
/// the order given: a charge above its limit, or an unknown cost under a cost limit.
fn spending(
    action: Option<&str>,
    limits: [(BudgetLimit, Option<i64>, Option<i64>); 5],
) -> impl Iterator<Item = RunBudgetBreach> + '_ {
    limits.into_iter().filter_map(move |(limit, max, charged)| {
        let max = max?;
        match charged {
            Some(charged) if charged <= max => None,
            charged => Some(RunBudgetBreach {
                action: action.map(str::to_owned),
                limit,
                max,
                charged,
            }),
        }
    })
}

/// The spending limits of `charged` under the limits given: the four an action's budget may set,
/// then the run's alone, its input and output tokens together (`tokens`).
fn spent(
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    duration_ms: Option<i64>,
    cost: Option<i64>,
    tokens: Option<i64>,
    charged: &ChargedUsage,
) -> [(BudgetLimit, Option<i64>, Option<i64>); 5] {
    [
        (
            BudgetLimit::MaxTokensIn,
            tokens_in,
            Some(charged.input_tokens),
        ),
        (
            BudgetLimit::MaxTokensOut,
            tokens_out,
            Some(charged.output_tokens),
        ),
        (
            BudgetLimit::MaxDurationMs,
            duration_ms,
            Some(charged.duration_ms),
        ),
        (BudgetLimit::MaxCostMicroUsd, cost, charged.cost_micro_usd),
        (
            BudgetLimit::MaxTokens,
            tokens,
            Some(charged.input_tokens.saturating_add(charged.output_tokens)),
        ),
    ]
}

/// The first spending limit of `budget` the charges of `track` passed: an action's, in the order
/// the budget lists them, then the run's.
fn spent_past(budget: &RunBudget, track: &Track) -> Option<RunBudgetBreach> {
    spent_past_each(budget, track).into_iter().next()
}

/// Every spending limit of `budget` the charges of `track` passed, in the order [`spent_past`]
/// checks them: each action's, in the order the budget lists the actions and then their limits,
/// then the run's.
fn spent_past_each(budget: &RunBudget, track: &Track) -> Vec<RunBudgetBreach> {
    let charges = &track.charges;
    let mut breached = Vec::new();
    for limits in &budget.per_action {
        let Some(charge) = charges
            .actions
            .iter()
            .find(|charge| charge.action == limits.action)
        else {
            continue;
        };
        breached.extend(spending(
            Some(&limits.action),
            spent(
                limits.max_tokens_in,
                limits.max_tokens_out,
                limits.max_duration_ms,
                limits.max_cost_micro_usd,
                None,
                &charge.charged,
            ),
        ));
    }
    breached.extend(spending(
        None,
        spent(
            budget.max_tokens_in,
            budget.max_tokens_out,
            budget.max_duration_ms,
            budget.max_cost_micro_usd,
            budget.max_tokens,
            &charges.run,
        ),
    ));
    breached
}

/// What the stop policy makes of an executor step that breached `limit`
/// (`story:breach-choice-per-limit`): the step `breach_by_limit` lists for it, else `on_breach`.
fn breach_step(policy: &StopPolicy, limit: BudgetLimit) -> BreachStep {
    policy
        .breach_by_limit
        .iter()
        .find(|entry| entry.limit == limit)
        .map_or(policy.on_breach, |entry| entry.step)
}

/// The first of `breached`, in order, whose limit's choice under `policy` discards the step.
fn first_discarded(policy: &StopPolicy, breached: &[RunBudgetBreach]) -> Option<RunBudgetBreach> {
    breached
        .iter()
        .find(|breach| matches!(breach_step(policy, breach.limit), BreachStep::DiscardStep))
        .cloned()
}

/// Refuses a stop policy whose `breach_by_limit` lists a limit more than once
/// (`story:breach-choice-per-limit`). Listing `MaxSteps` or `MaxTrials` is allowed and changes
/// nothing: they stop the run before a step, never as an executor step's breach.
///
/// # Errors
///
/// [`LoopFailure::BreachLimitListedTwice`], naming the first limit listed again.
fn check_stop_policy(policy: &StopPolicy) -> Result<(), LoopFailure> {
    let listed = &policy.breach_by_limit;
    for (index, entry) in listed.iter().enumerate() {
        if listed[..index]
            .iter()
            .any(|earlier| earlier.limit == entry.limit)
        {
            return Err(LoopFailure::BreachLimitListedTwice(entry.limit));
        }
    }
    Ok(())
}

/// The instant `budget`'s deadline names, in whole seconds since the Unix epoch and the
/// nanoseconds past that second; `None` when it sets no deadline.
///
/// # Errors
///
/// [`LoopFailure::InvalidDeadline`] when the deadline is not an RFC 3339 date-time.
fn deadline_of(budget: Option<&RunBudget>) -> Result<Option<(i64, u32)>, LoopFailure> {
    let Some(Timestamp(text)) = budget.and_then(|limits| limits.deadline.as_ref()) else {
        return Ok(None);
    };
    match rfc3339::instant(text) {
        Some(instant) => Ok(Some(instant)),
        None => Err(LoopFailure::InvalidDeadline(text.clone())),
    }
}

/// The breach of `deadline` when the trusted clock reads it as reached: at or after it, or a
/// reading that is no RFC 3339 date-time, which counts as past it and charges nothing known. The
/// breach names the deadline and the reading in whole seconds since the Unix epoch.
fn deadline_reached<C: LoopContext + ?Sized>(
    deadline: Option<(i64, u32)>,
    context: &mut C,
) -> Option<RunBudgetBreach> {
    let deadline = deadline?;
    let reading = rfc3339::instant(&context.now().0);
    if reading.is_some_and(|now| now < deadline) {
        return None;
    }
    Some(RunBudgetBreach {
        action: None,
        limit: BudgetLimit::Deadline,
        max: deadline.0,
        charged: reading.map(|(seconds, _)| seconds),
    })
}

/// Why the run stops before a step, when it does: the deadline the trusted clock reads as reached
/// (`RunBudget`), else the host's own stop (`HostStop`), which is asked only when the deadline did
/// not stop the run.
fn stopped_before_step<C: LoopContext + ?Sized>(
    deadline: Option<(i64, u32)>,
    context: &mut C,
) -> Option<SuspensionReason> {
    if let Some(breach) = deadline_reached(deadline, context) {
        return Some(SuspensionReason::RunBudget(breach));
    }
    context.host_stop().map(SuspensionReason::HostStop)
}

/// Whether `frontier` admits a listed action without a grant ([`admit`] answers it
/// `Admissible`), so a step can follow an executor call on it without the provider's allow. The
/// host's own stop is asked before the call only then (`story:host-stop-when-a-step-follows`).
fn admits_without_grant(frontier: &Frontier<frontier_state::Issued>) -> bool {
    listed_actions(frontier.data())
        .any(|action| matches!(admit(frontier, action), Admission::Admissible(_)))
}

/// The trial limit of `budget` that handing requests for `actions` to the effect port would pass,
/// naming the trials already charged to its action.
fn trials_past(budget: &RunBudget, track: &Track, actions: &[&str]) -> Option<RunBudgetBreach> {
    budget.per_action.iter().find_map(|limits| {
        let max = limits.max_trials?;
        let more = actions
            .iter()
            .filter(|action| **action == limits.action)
            .count();
        if more == 0 {
            return None;
        }
        let charged = track
            .charges
            .actions
            .iter()
            .find(|charge| charge.action == limits.action)
            .map_or(0, |charge| charge.charged.trials);
        (charged.saturating_add(i64::try_from(more).unwrap_or(i64::MAX)) > max).then(|| {
            RunBudgetBreach {
                action: Some(limits.action.clone()),
                limit: BudgetLimit::MaxTrials,
                max,
                charged: Some(charged),
            }
        })
    })
}
