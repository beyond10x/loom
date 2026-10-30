---
format: aep.planning-md/3
id: review-result:security-connectors-cli-invoke-form-pass-1
kind: review-result
status: active
title: Security pass 1, wave 2026-10-30-w2 u3, story:connectors-cli-invoke-form, at 6bfc7f13
relations:
- reviews: story:connectors-cli-invoke-form
revision: 1
---
# Security pass 1, wave 2026-10-30-w2 u3, story:connectors-cli-invoke-form, at 6bfc7f13

```findings
[
{"file":"crates/loom-connectors/src/cli.rs","line":303,"category":"integrity","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a governed write took its target from a describe answer naming no adapter; fixed in c64e8e2d"},
{"file":"crates/loom-connectors/src/cli.rs","line":614,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"Loom relies on Connectors mapping alias to target and back to one entry; a read could land on another entry in a shared-name configuration, a write is refused by its proof instance binding"}
]
```
