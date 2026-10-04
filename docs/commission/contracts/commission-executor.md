# Contract Sketch — Commission Executor

Commission must not depend on Loom specifically.

A generic executor contract should be able to support:

- Loom;
- external coding harnesses;
- workflow executors;
- human executors;
- test fakes.

The port as built (`crates/commission/src/ports/executor.rs`):

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
```

The executor does not mark the case complete.

Completion remains a governor/protocol determination.
