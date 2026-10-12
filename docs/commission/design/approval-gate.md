# The approval gate

`run_until_blocked` (`crates/loom-commission/src/runtime.rs`, step 5 of its module doc) stops a Run
`AwaitingApproval` when a step leaves the frontier's actions that need approval exactly as they
were: the executor worked around the gate, and only an approver can move the case on.

## Decision

A step performed under a grant passes the gate. When a step's request needed authority (its
revalidation answered `NeedsAuthority`), the authority provider answered `AuthorityVerdict::Allow`
and the effect port answered `EffectOutcome::Performed`, the next iteration does not read the gate
and runs the executor. The grant is the approval the gate waits for, so the Run goes on.

Every other step reads the gate as before: one that performed an action needing no authority, one
that proposed nothing, and one whose request was refused, stale, denied or not performed. The step
budget and the idle bound still end a Run that only repeats granted steps, and with the budget used
up after a granted step the Run is suspended for its budget.

Before this decision a host had to start a new Run after every granted effect to get past the
gate. A plugin turn is now one Run.

## Cases

| Step | Next iteration | Test (`crates/loom-commission-testkit/tests/runtime_effect.rs`) |
| --- | --- | --- |
| Performed an action needing no authority, beside actions that need approval | reads the gate; ends `AwaitingApproval` | `a_step_that_leaves_the_approval_gate_unchanged_ends_awaiting_approval` |
| Performed an action needing authority the provider allowed | does not read the gate; runs the executor | `a_step_performed_under_a_grant_passes_the_gate` |
| Proposed an action needing authority the provider denied | reads the gate; ends `AwaitingApproval` on the other actions, the denied one not awaited | `a_step_performed_under_a_grant_passes_the_gate` (control), `a_denied_action_is_not_awaited` |

The budget rule is held by `the_approval_gate_is_checked_before_the_step_budget` for an ungated
step and by the budget case of `a_step_performed_under_a_grant_passes_the_gate` for a granted one.
