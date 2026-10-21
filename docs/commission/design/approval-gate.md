# The approval gate

`run_until_blocked` (`crates/loom-commission/src/runtime.rs`, step 5 of its module doc) stops a Run
`AwaitingApproval` when a step leaves the frontier's actions that need approval exactly as they
were: the executor worked around the gate, and only an approver can move the case on.

## Decision

A step performed under a grant passes the gate. When a step's request needed authority (its
revalidation answered `NeedsAuthority`), the authority provider answered `AuthorityVerdict::Allow`
and the effect port answered `EffectOutcome::Performed`, the next iteration does not read the gate
and runs the executor. A granted and performed step passes the whole gate for the next iteration,
whichever actions the gate lists, and the gate forms again after the next step that is not
granted.

Every other step reads the gate as before: one that performed an action needing no authority, one
that proposed nothing, and one whose request was refused, stale, denied or not performed. After
the provider allows a request, the case's revision is read again: on a move, which somebody else
made while the provider decided, the request is stale, it is not performed and the step is no
grant; the next iteration judges the move. A move after that read, while the effect itself runs,
is not seen; an effect port that must refuse a write on a moved case binds it to the request's
`expected_case_revision`. The step
budget and the idle bound still end a Run that only repeats granted steps. With the budget used up
after a granted step, the next iteration first loads the case: a case that step completed ends the
Run `Completed`, and a case still open is suspended for its budget.

Before this decision a host had to start a new Run after every granted effect to get past the
gate. A plugin turn is now one Run.

## Stop policy

The gate is the host's to turn off (`story:stop-policy-gate-and-idle-bound`). A host whose
`LoopContext::stop_policy` answers `StopPolicy { approval_gate: false, .. }` gets no gate: no step
arms it, the run never ends `AwaitingApproval`, and a step on an unchanged frontier that lists
actions needing approval goes to the executor and is judged as any other step. One that admits no
new request is an idle iteration, counted toward the idle bound, and the policy's `idle_bound`
sets how many in a row end the run `IdleBound`, with or without a step budget (a value below 1 is
read as 1). Without an idle bound and with a step budget, such a run goes on until its budget.

Turning the gate off grants nothing. A proposal of an action that needs approval is still asked
about: the provider's `ApprovalRequired` ends the run `NeedsAuthority` (under `run_with_suspension`
it holds a Suspension), a denial is an idle iteration, and the effect port is never handed a request
the provider did not allow. A context that does not implement `stop_policy` keeps the gate and the
stops described above; the intake slice, plugin turns and `b10x-loom` use that default.

## Cases

| Step | Next iteration | Test (`crates/loom-commission-testkit/tests/`) |
| --- | --- | --- |
| Performed an action needing no authority, beside actions that need approval | reads the gate; ends `AwaitingApproval` | `a_step_that_leaves_the_approval_gate_unchanged_ends_awaiting_approval` |
| Performed an action needing authority the provider allowed | does not read the gate; runs the executor | `a_step_performed_under_a_grant_passes_the_gate` |
| Proposed an action needing authority the provider denied | reads the gate; ends `AwaitingApproval` on the other actions, the denied one not awaited | `a_step_performed_under_a_grant_passes_the_gate` (control), `a_denied_action_is_not_awaited` |
| Performed, on the last budgeted step, an action needing authority the provider allowed, which completes the case | loads the case; ends `Completed` | `a_granted_step_that_completes_the_case_at_the_budget_ends_completed` (`adversary_granted_gate.rs`) |
| Proposed an action needing authority the provider allowed, on a case that moved while it decided | performs nothing; loads the case and judges the move | `adversary_the_run_loop_does_not_perform_a_request_whose_case_moved_while_the_provider_decided` (`adversary_w20261020_u2.rs`) |
| Proposed nothing, beside actions that need approval, with the gate off | runs the executor; an idle iteration, ending `IdleBound` at the policy's bound | `an_unchanged_frontier_without_the_gate_is_an_idle_iteration` (`stop_policy.rs`) |
| Proposed an action needing approval the provider withheld, with the gate off | ends `NeedsAuthority`, or holds a Suspension; nothing is invoked | `an_ungranted_approval_required_action_is_still_refused_without_the_gate` (`stop_policy.rs`) |

The first three tests are in `runtime_effect.rs`. The budget rule is held by
`the_approval_gate_is_checked_before_the_step_budget` for an ungated step, by the budget case of
`a_step_performed_under_a_grant_passes_the_gate` for a granted one that leaves the case open, and
by `a_granted_step_that_completes_the_case_at_the_budget_ends_completed` for a granted one that
completes it.
