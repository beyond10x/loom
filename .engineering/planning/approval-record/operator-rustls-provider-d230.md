---
format: aep.planning-md/3
id: approval-record:operator-rustls-provider-d230
kind: approval-record
status: approved
title: Operator installs one rustls provider; Loom 0.50.2 (D230)
summary: b10x-loom-operator run installs ring before the kube client; release Loom 0.50.2
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T23:04:37Z", actor: "human:timo", revision: 2}
---
## Decision

Session `b10x-chef-orchestrator`, 2026-10-29, decision D230 under the operator's grant (class C:
a pull request, a merge and a release through Loom's own process). The item goes alone, without a
wave, because a consumer is blocked on it today: the takt trial's controller pod in its local
kind cluster runs image `loom-operator:0.50.1`, which exits 101 at start with rustls'
"Could not automatically determine the process-level CryptoProvider" panic.

1. `b10x-loom-operator run` installs rustls' `ring` provider as the process default before it
   builds the kube client.
2. A process test, `crates/loom-operator/tests/startup.rs`, starts `run` against a local listener
   and fails on the panic or on an exit before the connection.
3. Loom releases 0.50.2.

## Checked

- `cargo tree -p b10x-loom-operator -e features -i rustls`: tag 0.45.0 enables `ring` only; tag
  0.46.0 and later enable `ring` (kube `rustls-tls`, reqwest) and `aws-lc-rs` (`tokio-rustls`
  `aws_lc_rs` from `b10x-substrate-host`, reached through `b10x-loom-sdk` and
  `b10x-loom-intake-slice`). With both, rustls picks no default.
- The new test fails on 0.50.1's code with the same panic text as the pod log, and passes with
  the change.
- The ESS pin stays at 0.77.0, the newest ESS release.
