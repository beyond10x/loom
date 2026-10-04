# Contract — Frontier

`Frontier` is the bridge between governance and execution.

It is not merely a tool list.

The frontier is the generated `commission.responsibility.Frontier`, declared in
`ess/domains/responsibility.yaml`; this file follows that specification. A frontier is issued for
one case revision and answers:

- which claims are currently true, false or unknown;
- which obligations remain open;
- which actions are admissible;
- which actions require authority, and the capability to ask for;
- which actions are blocked, and why.

Shape:

```yaml
frontier_id: 6b0f3c1e-8d2a-4e57-9c41-2f7a5d3e8b90
case_id: CHG-1842
case_revision: 17

claims:
  - claim: tests.pass
    value: Unknown            # True | False | Unknown

obligations:
  - obligation: verify.tests
    open: true

actions:
  - action: repository.inspect
    status: Admissible
    capability: null
    reasons: []

  - action: repository.push
    status: ApprovalRequired
    capability: repository.write
    reasons: []

  - action: repository.merge
    status: Blocked
    capability: null
    reasons:
      - tests.pass is Unknown, required True
```

## Invariant

An executor may not invoke an action absent from the current frontier/admissible set.

Commission's admission check (`crates/commission/src/admission.rs`) sorts a proposed action into
the generated `Admission` union:

- `Admissible` when the frontier lists it `Admissible`;
- `NeedsAuthority`, naming the capability, when it is listed `ApprovalRequired` with a capability;
- `Refused`, naming the action and carrying its reasons, when it is listed `Blocked`, listed
  `ApprovalRequired` with no capability, or not listed at all (then with no reasons).

A capability that is empty or whitespace only names nothing to ask for, and counts as no
capability.

When the frontier lists one action more than once, the least-authority entry decides, whatever
their order:

1. any `Blocked` entry: `Refused`, carrying the reasons of every `Blocked` entry;
2. otherwise any `ApprovalRequired` entry with no capability: `Refused`, carrying the reasons of
   every such entry;
3. otherwise `ApprovalRequired` entries that name different capabilities: `Refused` with one
   reason only, `conflicting capabilities for an ApprovalRequired action: ` followed by each
   capability as a JSON string, sorted and separated by `, ` — for example `"a, b", "c"` — so
   different sets never read the same; no entry's own reasons are carried;
4. otherwise any `ApprovalRequired` entry: `NeedsAuthority`, naming its capability;
5. otherwise `Admissible`.

Where reasons come from several entries, each entry's reason list keeps its own order, the lists
are taken in lexicographic order, and identical lists are taken once.

## Differences from the earlier sketch

The earlier sketch of this contract disagreed with the specification. Each difference was settled
in favour of the specification (story:frontier-admission):

| Earlier sketch | Specification, now this contract |
|---|---|
| items keyed by `id` | keyed by `claim`, `obligation`, `action` |
| obligation `status: open` and `priority` | `open: Boolean`; no priority |
| structured block reasons (`claim`, `required`, `actual`) | `reasons: List<String>` |
| no approval-required action status | `ApprovalRequired`, with an optional `capability` |
| a claim may be "unknown or contradicted" | `Truth` is `True`, `False` or `Unknown` |
| `format: frontier/1` and `case: {id, revision}` | `case_id` and `case_revision` fields |
| no frontier identity | `frontier_id`, the identity of the frontier issued for one case revision |
| per-action `input_schema` | not part of the frontier |
| `conclusions` with a blocked status | not part of the frontier |
