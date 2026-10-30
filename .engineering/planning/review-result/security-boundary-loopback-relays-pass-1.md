---
format: aep.planning-md/3
id: review-result:security-boundary-loopback-relays-pass-1
kind: review-result
status: active
title: Security review, wave 2026-10-30-w1 u2, story:boundary-loopback-relays, at 14c68e56
relations:
- reviews: story:boundary-loopback-relays
revision: 1
---
# Security review, wave 2026-10-30-w1 u2, story:boundary-loopback-relays, at 14c68e56

Agent `aep:security-reviewer` (opus). Commit `ae20360e`, `crates/loom-boundary/tests/security_d243_u2.rs`:
5 cases, all green (an upstream redirect is not followed and carries no token, agent trailers never
reach the upstream, a connection header cannot strip the token or host, each relay sends only its
own token, TRACK is refused). Verified: loopback bind only, the agent cannot choose the upstream
host, the token is never in a response, log or error, Connectors and the boundary read the same
operation, argv holds no secret, no TLS added to the binary.

```findings
[
{"file": "crates/loom-boundary/src/upstream.rs", "line": 131, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "Upstream::parse admits loopback and unspecified hosts; the controller admits only <service>.<namespace>.svc upstreams its configuration lists for the relay kind, so no rendered pod reaches that state"}
]
```
