# Contract Sketch — Commission Executor

Commission must not depend on Loom specifically.

A generic executor contract should be able to support:

- Loom;
- external coding harnesses;
- workflow executors;
- human executors;
- test fakes.

The port as built (`crates/loom-commission/src/ports/executor.rs`):

```rust
pub trait AgentExecutor {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome;
}
```

`run` is synchronous and has no error channel: an executor that fails returns
`Suspended(ExecutorOutcomeSuspended { reason: SuspensionReason::ExternalAvailability(..) })`.

The outcome it returns, generated from `ess/` into `generated/rust/commission/src/responsibility.rs`:

```rust
pub enum ExecutorOutcome {
    ProposedAction(ExecutorOutcomeProposedAction),
    NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment),
    Suspended(ExecutorOutcomeSuspended),
    NoUsefulAction(Unit),
    CompletedLocalReasoning(Unit),
    CaseMoved(ExecutorOutcomeCaseMoved),
}

pub struct ExecutorOutcomeProposedAction {
    pub action: String,
    pub arguments: ProposedActionArguments,
}

pub struct ExecutorOutcomeNeedsHumanJudgment {
    pub request: HumanDecisionRequest,
}

pub struct ExecutorOutcomeSuspended {
    pub reason: SuspensionReason,
}

pub struct ExecutorOutcomeCaseMoved {
    pub expected_case_revision: i64,
}
```

An executor that finds the case has moved to another revision since the frontier it was handed
returns `CaseMoved`, naming that frontier's revision. The runtime then loads the case again and
judges the run on the frontier current then; it never takes the revision from the executor. When
the governor still holds the case at the run's revision, the move is not borne out and the step
counts as `NoUsefulAction`.

An executor need not notice every move. When it proposes nothing (`CompletedLocalReasoning`,
`NoUsefulAction`) and the run would end on the outcome derived from the frontier it was handed, the
runtime loads the case once more first, and a case that moved is judged as after a `CaseMoved` the
governor bears out.

The executor does not mark the case complete.

Completion remains a governor/protocol determination.
