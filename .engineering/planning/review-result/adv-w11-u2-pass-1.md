---
format: aep.planning-md/3
id: review-result:adv-w11-u2-pass-1
kind: review-result
status: active
title: Adversary review, loom w11 U2 workflow definition executor, pass 1
relations:
- reviews: story:dw-workflow-definition-executor
revision: 1
---
needs-revision, then fixed in the unit

Adversarial pass on story:dw-workflow-definition-executor (`crates/loom-executor/src/workflow.rs`),
cases in `crates/loom-executor/tests/adversary_w11_u2.rs`: 7 cases, 2 red on the first run, 7 green
after the fix.

- A judgement step widened the host's narrowing: the model was offered the step's actions, not the
  intersection with what the host admitted. Fixed: a step offers only the actions the host's
  narrowing also admits; a step left with none ends needing human judgement.
- Steps named `inspect` and `inspect ` both loaded. Fixed: a step name with leading or trailing
  white space is refused, naming the step.

Attacked and held: a definition naming an action outside its protocol, an empty or multi-document
definition, a frontier that blocks a step's action, deterministic steps calling no model.

```findings
- file: crates/loom-executor/src/workflow.rs
  category: governance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: a judgement step offered the model actions the host's narrowing excluded; fixed by intersecting with the host's narrowing
- file: crates/loom-executor/src/workflow.rs
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: two steps whose names differ only by surrounding white space both loaded; fixed by refusing such names
```
