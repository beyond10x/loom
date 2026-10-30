---
format: aep.planning-md/3
id: review-result:adversary-connectors-cli-invoke-form-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-30-w2 u3, story:connectors-cli-invoke-form, at 6bfc7f13
relations:
- reviews: story:connectors-cli-invoke-form
revision: 1
---
# Adversary pass 1, wave 2026-10-30-w2 u3, story:connectors-cli-invoke-form, at 6bfc7f13

```findings
[
{"file":"crates/loom-connectors/src/cli.rs","line":303,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the adapter echo was checked only when present while the invoke target came from the same answer; fixed in c64e8e2d"},
{"file":"crates/loom-connectors/src/cli.rs","line":614,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"under Connectors v0.60.0 a legacy catalog alias sharing derived names with a [connectors] instance is described as that target while invoke selects the other entry; source-read only, not observed"}
]
```
