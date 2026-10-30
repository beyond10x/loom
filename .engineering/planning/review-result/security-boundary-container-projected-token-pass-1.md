---
format: aep.planning-md/3
id: review-result:security-boundary-container-projected-token-pass-1
kind: review-result
status: active
title: Security review, wave 2026-10-30-w1 u1, story:boundary-container-projected-token, at 7086fd37
relations:
- reviews: story:boundary-container-projected-token
revision: 1
---
# Security review, wave 2026-10-30-w1 u1, story:boundary-container-projected-token, at 7086fd37

Agent `aep:security-reviewer` (opus). Commit `80825c77`, `crates/loom-operator/tests/security_d243_u1.rs`:
2 cases, 1 red (an API-server audience was admitted), fixed in `85c92a0f` by binding each allowed
upstream to its relay kind and audience in the controller's configuration. Verified: only the
boundary mounts the token volume; no process, IPC or host namespace is shared; no Secret volume,
envFrom or valueFrom; automountServiceAccountToken false on pod and account; the boundary's argv
cannot be steered past declared values; RBAC unchanged; the two existing goldens unchanged. The
kubelet's projected-token file mode with mixed container users (0640 root:10001 with fsGroup, 0644
root:root without) is modelled from the kubelet code, not observed on a node.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 1269, "category": "integrity", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the projected token's audience was author-chosen and bound to no allowed upstream, so an API-server audience was admitted; fixed in 85c92a0f"},
{"file": "crates/loom-operator/src/render.rs", "line": 1095, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the upstream rule opened the Service port, not the pod's target port; 85c92a0f adds an optional pod port per allowed upstream"}
]
```
