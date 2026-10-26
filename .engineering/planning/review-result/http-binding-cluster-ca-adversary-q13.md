---
format: aep.planning-md/3
id: review-result:http-binding-cluster-ca-adversary-q13
kind: review-result
status: active
title: Adversary pass over the cluster CA file (wave 2026-10-26-q13)
relations:
- reviews: story:http-binding-cluster-ca
revision: 1
---
## Review

Adversary pass (aep:adversary, opus) over `impl/http-binding-cluster-ca` at `1db54113`, wave
2026-10-26-q13. No defect in the implementation. Nine adversary tests added and kept
(`crates/loom-agent/tests/adv_q13_cluster_ca.rs`, commit `00235d4c`), all green; they kill mutants
M1 and M2 that the unit's suite missed. Mutants M4 and M5 (the CA not handed to the host's agent or
the A2A listener's agent) survive `agent_serve`; M3 (built-in roots disabled) cannot be shown on
loopback and rests on reading the code (reqwest 0.12.28 with `rustls-tls-webpki-roots` only).
Checked and not broken: other clients, file shapes, the 1 MiB bound, empty variable, ESS order,
leaks, `a2a::Agent.trust` (publish = false), generated and website drift, operator/CRD/chart
unchanged.

`cargo test -p b10x-loom-agent --locked --no-fail-fast`: 87 → 96 executed, 0 failed.

```findings
[
{"file":"crates/loom-cli/src/main.rs","line":170,"category":"mutant","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"Mutant M4 deletes `.with_http_trust(trust.clone())` and `cargo test -p b10x-loom-cli --locked --test agent_serve` stays green (10 passed): the host's agent getting the CA is untested; no agent serve test starts with a valid CA file."},
{"file":"crates/loom-cli/src/a2a.rs","line":114,"category":"mutant","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"Mutant M5 deletes `.with_http_trust(agent.trust)` and `--test agent_serve` stays green (10 passed): the A2A listener's agent getting the CA is untested."},
{"file":"crates/loom-agent/src/serve.rs","line":233,"category":"mutant","severity":"note","verdict":"needs-revision","origin":"introduced","message":"Mutant M1 makes with_http_trust a no-op and http_trust.rs stays green; killed by adversary test adv_a_served_turn_reads_an_https_binding_through_the_plugin_trust (kept, commit 00235d4c)."},
{"file":"crates/loom-agent/src/http.rs","line":146,"category":"mutant","severity":"note","verdict":"needs-revision","origin":"introduced","message":"Mutant M2 disables the regular-file check and the unit suite stays green; killed by adversary tests for a character device, a FIFO and symlinks (kept, commit 00235d4c)."},
{"file":"README.md","line":285,"category":"contract-drift","severity":"note","verdict":"needs-revision","origin":"introduced","message":"README.md:285, the CLI help (crates/loom-cli/src/lib.rs:417, website cli page) and CHANGELOG.md:17 say the host refuses a file that 'holds no certificate'; code, ESS and AGENTS.md also refuse a file where any one block is rejected (all or nothing)."}
]
```
