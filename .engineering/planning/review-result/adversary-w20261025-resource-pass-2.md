---
format: aep.planning-md/3
id: review-result:adversary-w20261025-resource-pass-2
kind: review-result
status: active
title: Adversary pass 2 on story:loom-agent-resource-domain (600694ee)
tags:
- non-interactive
relations:
- reviews: story:loom-agent-resource-domain
revision: 1
---
**Pass 2 adversary report: story:loom-agent-resource-domain at 600694ee**

```
unit: story:loom-agent-resource-domain, pass 2 (head 600694ee)
verdict: red, 2 cases
cases executed before→after: 161 → 163, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: yes, scratch only (listed below)
needs-coordinator: no
```

**Tree:** `git diff --stat` is empty. `git status --short` shows one file, `?? crates/loom-resource/tests/adversary_w20261025_crd_projection_2.rs`. That file passes rustfmt (`--check` clean) and clippy (`-D warnings`, clean).

**Case 1.** Run alone, it fails at `adversary_w20261025_crd_projection_2.rs:107`.
- Input: ESS 0.69.0's own output for union `U`, with variants `labels: Map<String,String>` newtype, `raw: Json` newtype and `s: S`.
- Calibration (an `s`-only union) projects Ok.
```
the API server prunes these and the projection reported nothing:
`u` with kind `labels` carries `team`, ... the flattened union {"oneOf":[{"not":{"anyOf":[{"required":["name"]}]},"properties":{"kind":{"enum":["labels"]}}},{"properties":{"kind":{"enum":["s"]}},"required":["name"]}],"properties":{"kind":{"enum":["labels","s"],"type":"string"},"name":{"type":"string"}},"required":["kind"],"type":"object"} declares no such member and neither preserves unknown fields nor carries the value's schema
`u` with kind `raw` carries `anything`, ... the flattened union {"oneOf":[{"not":{"anyOf":[{"required":["name"]}]},"properties":{"kind":{"enum":["raw"]}}},{"properties":{"kind":{"enum":["s"]}},"required":["name"]}],"properties":{"kind":{"enum":["raw","s"],"type":"string"},"name":{"type":"string"}},"required":["kind"],"type":"object"} declares no such member ...
```

**Case 2.** Run alone, it fails at `:126`. The input is a `Map<String, Json>` field (ESS output) checked with `jsonschema`.
```
the projected `values` {"additionalProperties":{"type":"object","x-kubernetes-preserve-unknown-fields":true},"type":"object"} refuses JSON values the specification's `Json` admits
  left: ["/values/flag: true is not of type \"object\"", "/values/list: [1] is not of type \"object\"", "/values/number: 1 is not of type \"object\"", "/values/text: \"x\" is not of type \"object\""]
```

**Suite:** `cargo test --locked --no-fail-fast -p b10x-loom-resource -p b10x-loom-sdk -p b10x-loom-conformance -p loom-xtask`

| Run | Passed | Failed | Ignored | Exit |
|---|---|---|---|---|
| after (my cases in) | 161 | 2 (the 2 above) | 6 | 101 |
| before (`--skip` my 2) | 161 | 0 | 6 | 0 |

**Findings**

| # | Where | Verdict | Origin | What and fix |
|---|---|---|---|---|
| P2-1 | `crates/loom-resource/src/crd.rs:590-594` | INFEASIBLE, low | introduced | `union()` keeps only the `properties` and `required` of a variant's projected value. It drops a Map value's `additionalProperties` schema and a Json value's preserve-unknown-fields without a word, so the API server prunes the variant's content. This breaks the claims at crd.rs:11-13 ("none is passed over unexamined") and :27 ("a map keeps its entries"). No loom union has a Map or Json variant today (bindings are structs). Fix: refuse such a variant as `CrdError::Union`. |
| P2-2 | `crates/loom-resource/src/crd.rs:391-392` | INFEASIBLE, low | introduced | Json always becomes `{type: object, preserve}`. That is right for `spec.routes`, a document. But with map values now carried (F3), a `Map<String, Json>` refuses scalar and list values. Fix: project a map's Json value as `{x-kubernetes-preserve-unknown-fields: true}` with no `type`, and let the check at :400 accept that form. |
| N1 | CARRIED `format` | INFEASIBLE, info | introduced | ESS `Duration` writes `format: duration` (ESS 0.69.0 probe), which means ISO 8601. The API server checks `duration` with kube-openapi's strfmt parser: Go durations first, then a loose `\d+\s*unit` regex over ns/us/ms/s/m/h/d/w. By a local kube-openapi strfmt copy (2022), "P1M" would read as 1 minute. This is inferred, not run. No Duration field is reachable today. |

**Attacked and could not break**
- **F1:** a list in another domain stays required; the variant `required` comes from the already-reduced value; status still requires its members. The rule is keyed by the definition's domain, so inlining one definition twice under different owners gives the same result.
- **F2:** I tested 2-variant and 3-variant unions:
  - a tag-only document;
  - a document mixing both variants;
  - a document carrying another variant's member.
  All are refused as expected.
  - Only `enum`, `required` and `not.anyOf.required` are emitted inside junctors, never `type` or `default`.
  - A shared field with different schemas gives `UnionConflict` (tests/crd.rs:1202).
  - `violations` does not check `x-kubernetes-*` inside junctors, but the projection emits none there.
- **F3:** these all project correctly and validate as expected:
  - `Map<String, U>` with a 2-variant union: cross-variant members are refused;
  - `Map<String, Enum>`: an unknown value is refused;
  - a `List<String>` outside `loom.agent` stays required;
  - `Optional<T>` is simply not required.

  The throwaway probe for these was green and has been deleted (`tests/adversary_probe_pass2.rs`). Unknown `x-ess-*`, `uniqueItems` and a `oneOf` outside a union are already refused by unit tests.

**Lease:** `loom-agent-domain-u1-adversary` was released (`worktree hook session-end`, exit 0). No git write, no home-directory literal or company name in the test file.

**Written outside the worktree**, all under the assigned scratch directory `u1-adversary/`:
- `unionspec/`
- `mapspec2/`
- `suite2-after.log`
- `suite2-before.log`

Severities in the block below are the store's (`low` recorded as `warning`, `info` as `note`).

```findings
- file: crates/loom-resource/src/crd.rs
  line: 590
  category: correctness
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: union() keeps only properties/required of a variant's projected value; a Map variant's additionalProperties schema and a Json variant's x-kubernetes-preserve-unknown-fields are dropped without refusal, so the API server prunes the variant's content (red - crates/loom-resource/tests/adversary_w20261025_crd_projection_2.rs:107); refuse such a variant as CrdError::Union
- file: crates/loom-resource/src/crd.rs
  line: 391
  category: correctness
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: Json projects to {type object, preserve} everywhere; as a carried map value (Map<String, Json>) it refuses number, text, list and bool values ESS Json admits (red - crates/loom-resource/tests/adversary_w20261025_crd_projection_2.rs:126); project a map's Json value as untyped x-kubernetes-preserve-unknown-fields
- file: crates/loom-resource/src/crd.rs
  line: 14
  category: contract
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: format is carried as written; ESS Duration writes format duration (ISO 8601) while the API server parses duration with kube-openapi strfmt (Go durations plus a loose unit regex), so values can be misread (P1M as 1 minute); inferred, not run; no Duration field is reachable today
```
