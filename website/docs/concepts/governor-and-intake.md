---
title: The governor and intake
sidebar_position: 6
description: How a case gets its frontier from Canon, and how an intent becomes a governed case that a local slice works until it is blocked.
status: shipped
lede: The governor evaluates a case's protocol with Canon and decides; it never acts. Intake turns an intent into a governed case and works it with a local slice whose evidence comes from verified host observations.
source: Atlas ADRs 0074, 0089 and 0090; crates/loom-governor, crates/loom-intake-references, crates/loom-intake-router, crates/loom-intake-slice; ess/domains/evaluation.yaml, ess/intake/domains/routing.yaml
---

## The governor

`CanonGovernor` (`b10x-loom-governor`) implements Commission's governor, evidence and observation
ports over [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)).
The compatibility API opens a case on a protocol of the
[engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)) registry, named `<name>@<major>`
(`software-change@1`), with the caller's revision of every artifact the protocol declares.

- **It evaluates afresh every time.** Each frontier and completion compiles the protocol and
  evaluates the case's evidence with Canon. It reads no clock, makes no network or model call, and
  supplies no authority decision.
- **It never invents authority.** An action that requires a capability is at best *approval
  required*; Commission asks its authority provider.
- **It decides and never acts.** It reports the frontier and whether one outcome is legitimate. It
  executes nothing.
- **Evidence applies to a revision.** A test result for the previous revision does not satisfy a
  claim about the current one: after an edit, the tests run again before the merge becomes
  possible.

The case store is a port (`CaseStore`); `MemoryCaseStore` keeps cases in memory.

Since 0.2.0, a host can register a reviewed Canon model through
`CanonGovernor::with_protocol(name, &model)`. Canon validates it; duplicate registrations,
built-in replacements and actions requiring more capabilities than Commission can represent are
refused. Protocol adoption is the host's decision, never a model proposal accepted as authority.
`with_protocol_yaml(name, yaml)` parses through Loom's pinned Canon so the host does not need a
matching Canon dependency merely to register a definition.

Since 0.4.0, Loom provides `ProtocolCatalog`, which combines engineering definitions, Loom's own
`system-query@1`, and explicitly installed custom definitions. `with_catalog` admits and seals the
same names and content digests that routing and case initialization use. No later definition can
replace them. Existing `classify`, `case::open`, and software-slice callers retain their engineering
catalog behavior; new catalog-aware entrypoints serve custom protocols.

`with_evaluation_time` supplies a trusted callback for freshness evaluation. Failure to obtain
time makes the governor unavailable. No authority or explicit decision enters through that port.
Evidence producers authenticate their observations before calling `submit_evidence`, including
any product requirement for independent reviewer execution contexts.

Durable hosts implement `FallibleCaseStore`. Reads and writes return errors; an insert error
stops opening the case rather than retrying another identifier. Updates are atomic, so a failed
write preserves previous state. `try_observations` reports read failures. Existing infallible
`CaseStore` implementations retain their API through a compatibility adapter. Loom supplies no
database backend: hosts restore their cases and the same admitted protocol definitions.

### One evaluation, for a caller that keeps its own case

A supervisor that keeps its own case record does not need Commission's types or a case store to get
a decision. `loom_governor::evaluate(&catalog, &request)` takes a protocol named from the host's
`ProtocolCatalog`, a `canon-case/1` case snapshot, the `canon-evidence/1` records and an optional
trusted time, and returns Canon's decision as the frontier and completion report it: each action's
status, the capabilities it requires and Canon's reasons, every claim and obligation, the one
legitimate outcome when the case is complete, and Canon's whole `canon-decision/1` document. It
uses the same evaluation as `CanonGovernor`, holds nothing, and reads no clock. The request and the
decision are declared in `ess/domains/evaluation.yaml`.

`b10x-loom evaluate` is the same call for a program in any language: the request as JSON on standard
input (or `--input <PATH>`), the decision as JSON on standard output.

```console
b10x-loom evaluate --input request.json | jq -c '{outcome, actions}'
```

```text
{"outcome":"answered","actions":[{"action":"system.time.read","status":"Admissible","requires":[],"reasons":[]}]}
```

An input it cannot use is refused with exit status 3, naming the input: the protocol (the catalog
has no such name), the snapshot (including a termination the records do not make legitimate), an
evidence record by its position, or the time. A record is refused when it is not a readable
`canon-evidence/1` record or repeats an earlier record's id. A readable record that does not apply
to the case, such as one of a kind the protocol does not declare, is set aside as `CanonGovernor`
sets it aside, and the decision is made from the rest; the decision does not list it. The two
refusals are where `evaluate` differs from the governor: `CanonGovernor` sets an unreadable record
aside, and of two records with one id keeps the first and sets the later aside, and still decides;
`evaluate` refuses the request, naming the record (`duplicate-identifier` and the later position
for a repeated id). An unreadable record:

```text
b10x-loom: evaluation refused: evidence record 0: evidence `clock-1` is not a canon-evidence/1 document: missing field `kind`
{"input":"Evidence","evidence_index":0,"code":"malformed-input","message":"evidence `clock-1` is not a canon-evidence/1 document: missing field `kind`"}
```

The protocol comes only from the catalog the host installed (`protocols add`), never from the
request, so host review of protocols still holds. Authenticating the evidence stays the host's job.

## Intake

Intake takes what someone asked for, as given, and runs it.

| Crate | Does | Does not |
|---|---|---|
| `b10x-loom-intake-references` | Extracts tracker keys, chat permalinks, merge and pull requests and URLs from the intent, in order, without a model | Fetch what they point at |
| `b10x-loom-intake-router` | Offers a model every protocol of the selected catalog and takes one forced `pick_protocol` call | Turn a pick into authority. A pick outside the registry or below the threshold is refused |
| `b10x-loom-intake-slice` | Opens the case through the governor, runs the Commission runtime with Loom proposing, performs software changes or read-only clock queries with host-supplied bindings, and submits verified observations as evidence | Merge, push or deploy; run a loop of its own; trust a model's account of what happened |

The slice's selector and argument generator (`ModelSelector`, `ModelArguments`) each ask a model
for one forced tool call through [llm](https://beyond10x.github.io/llm/)
([GitHub](https://github.com/beyond10x/llm)). The model sees the intent, its references and a
bounded transcript of what the executor reported. Whatever else it says is dropped.

Evidence comes from trusted verifiers: the exit status of an executed test command, or a clock
reading produced by the host clock binding (Atlas ADR 0074). A transcript line that a model-written file imitates can mislead the
next choice; it cannot become evidence.

:::note[Test confinement]
Tests use [Substrate](https://beyond10x.github.io/substrate/)
([GitHub](https://github.com/beyond10x/substrate)) with no network, read-only source and toolchain,
and workspace writes only to `target/`. Dependencies are prefetched explicitly. A missing
confinement guarantee stops the run; only `--confinement none` opts out. Every executed test
observation names its actual confinement. These changes shipped in 0.2.0.
The slice's own git calls run none of
the work tree's hooks, no `core.fsmonitor` command and no signing program, a case does not open
on a work tree whose own git configuration names a program, and a call is refused once the test
command changed that configuration or the git directory.
:::

The `intake.routing` domain specifies these nouns; its [reference](/docs/reference/ess/intake-routing)
is generated from `ess/intake/`. [Run an intent](../guides/run-an-intent.md) shows the slice from
the command line.
