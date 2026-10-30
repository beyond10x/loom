---
format: aep.planning-md/3
id: review-result:adversary-boundary-container-projected-token-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-30-w1 u1, story:boundary-container-projected-token, at 7086fd37
relations:
- reviews: story:boundary-container-projected-token
revision: 1
---
# Adversary pass 1, wave 2026-10-30-w1 u1, story:boundary-container-projected-token, at 7086fd37

Agent `aep:adversary` (opus). Commit `ca7b8277`, `crates/loom-operator/tests/adversary_d243_u1.rs`:
5 cases, 4 red, all fixed in `85c92a0f` under the coordinator's decisions: `allowedBoundaryUpstreams`
entries carry the relay kind, url, audience and pod port; namespace egress entries beside a
boundary are refused `BoundaryEgressNamespace`; an AgentServe boundary without a model and with
empty routes is refused `NoModelRoute`; admission runs the relay's own declaration checks.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 1258, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "allowedBoundaryUpstreams was one kind-blind list, so the Connectors upstream was admitted as the model relay; fixed in 85c92a0f"},
{"file": "crates/loom-operator/src/render.rs", "line": 1043, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "namespace egress entries rendered beside a boundary opened an upstream namespace on other ports; fixed in 85c92a0f"},
{"file": "crates/loom-operator/src/render.rs", "line": 1076, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a boundary without model kept default routes to a model route its NetworkPolicy closed; fixed in 85c92a0f"},
{"file": "crates/loom-operator/src/admission.rs", "line": 1122, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "admission bounded no operation id length while the relay refuses ids over 256 bytes; fixed in 85c92a0f"},
{"file": "crates/loom-operator/src/render.rs", "line": 1095, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the NetworkPolicy port was the Service port while Kubernetes matches the pod port; 85c92a0f adds an optional pod port to each allowed upstream entry"},
{"file": "crates/loom-operator/src/render.rs", "line": 59, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a credential-free resource may mount read-write the workspace claim a Secret-bearing resource mounts; filed as story:credential-free-workspace-isolation"}
]
```
