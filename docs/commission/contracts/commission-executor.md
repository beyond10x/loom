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
    ProposedReads(ExecutorOutcomeProposedReads),
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

pub struct ExecutorOutcomeProposedReads {
    pub reads: Vec<ExecutorOutcomeProposedAction>,
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

An executor that knows several reads it needs proposes them together as `ProposedReads`, at least
one and at most 16 (`MAX_FAN_OUT`), each an action and its arguments as in a single proposal
(`architecture-decision-record:read-fan-out`). The runtime admits the fan-out only when the effect
port reads every listed action (`EffectPort::reads`, `false` by default; a Connector-backed port
reads exactly the actions whose binding's effect is `Read`) and the frontier the executor was handed
lists each `Admissible`, needing no authority. Otherwise the whole proposal is refused and nothing
is invoked. Each read becomes its own action request at the frontier's revision and is revalidated
before any is invoked; one that fails revalidation is recorded refused and not invoked, and the
others go on. The admitted reads are invoked concurrently (`EffectPort::invoke_reads`, which a
`Sync` port answers through `invoke_concurrently`), each once, and joined; their effects are
recorded and observed in the order proposed. A `Refused` read cancels nothing; an error from any
read fails the loop after every read has answered, and nothing is retried. A fan-out is one step.
Writes, and every action that needs authority, are proposed one at a time.

An executor that finds the case has moved to another revision since the frontier it was handed
returns `CaseMoved`, naming that frontier's revision. The runtime then loads the case again and
judges the run on the frontier current then; it never takes the revision from the executor. When
the governor still holds the case at the run's revision, the move is not borne out and the step
counts as `NoUsefulAction`. Where the frontier current then would let the run go on, the run ends
`CaseMovedOn`, naming the Run's revision and the current one: a Run stays bound to its revision,
and the caller starts a new Run at the current one.

An executor need not notice every move. When it proposes nothing (`CompletedLocalReasoning`,
`NoUsefulAction`) and the run would end on the outcome derived from the frontier it was handed, the
runtime loads the case once more first, and a case that moved is judged as after a `CaseMoved` the
governor bears out.

The executor does not mark the case complete.

Completion remains a governor/protocol determination.
