---
format: aep.planning-md/3
id: review-result:adversary-boundary-loopback-relays-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-30-w1 u2, story:boundary-loopback-relays, at 14c68e56
relations:
- reviews: story:boundary-loopback-relays
revision: 1
---
# Adversary pass 1, wave 2026-10-30-w1 u2, story:boundary-loopback-relays, at 14c68e56

Agent `aep:adversary` (opus). Commit `ff359c31`, `crates/loom-boundary/tests/adversary_d243_u2.rs`:
12 cases, 10 green, 2 red. Both red cases were fixed in `016f1a88` (the relay drops a response
header whose value holds the token; a token path that is no regular file is not opened).

```findings
[
{"file": "crates/loom-boundary/src/lib.rs", "line": 594, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "send() returned upstream response headers unfiltered, so an upstream that reflects Authorization handed the projected token to the agent; fixed in 016f1a88"},
{"file": "crates/loom-boundary/src/lib.rs", "line": 490, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "read_token opened the path before its is_file check, so a FIFO token path hung the request; fixed in 016f1a88"}
]
```
