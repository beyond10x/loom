---
format: aep.planning-md/3
id: review-result:adversary-w10-loom-harness-crate-port-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w10 adversary, loom story:harness-crate-port, pass 1
relations:
- reviews: story:harness-crate-port
revision: 1
---
unit: loom/harness-crate-port, working tree on top of 4e299d5 (phase 2 not yet committed), checked against Harness 798325f0
verdict: CONFIRMED
cases: executed 479→507, red 2
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 17 paths under `~/.cache/ga-wave-2026-10-04-w10/loom-harness-crate-port/scratch/adv/` (listed in part 6), plus the assigned build dir
needs-coordinator: yes. Decide whether to keep my two test files: one replaces the lost suites, the other holds the module boundaries. Also decide whether to fix the ported comments — that edits ported code, which the "paths only" rule in `harness/mod.rs` does not allow.

**1. `git --no-pager diff --stat`**
```
 Cargo.lock                     | 1911 ++++++++++++++++++++++++++++++++++++++--
 crates/loom/Cargo.toml         |   10 +-
 crates/loom/src/harness/mod.rs |   24 +-
 3 files changed, 1862 insertions(+), 83 deletions(-)
```
These three paths are the implementor's uncommitted phase 2, not mine. My only writes in the tree are two untracked files: `crates/loom/tests/adversary_harness_port_contract.rs` and `crates/loom/tests/adversary_harness_port_boundaries.rs`. I edited no ported code, nothing in `ess/` and nothing in `generated/`.

**2. Cases added**

| file | case | now |
|---|---|---|
| `adversary_harness_port_boundaries.rs:77` | every test file a ported comment names as a guard exists in Loom | red |
| `adversary_harness_port_boundaries.rs:104` | every document a ported comment cites resolves in Loom | red |
| `adversary_harness_port_boundaries.rs:218` | the dependency boundaries between Harness's crates still hold between Loom's modules; a planted violation shows the scan can fail | green |
| `adversary_harness_port_contract.rs` (25 cases) | Harness's `tests/contract.rs` ×2, `summary_request.rs` ×2 and `transport.rs`, carried with only paths rewritten | green |

The red output from running the boundaries file alone (`cargo test -p b10x-loom --locked --test adversary_harness_port_boundaries`, EXIT=101):
```
ported comments name test files Loom does not have:
responses/mod.rs:86: `crates/harness-messages/tests/transport.rs`
messages/mod.rs:113: `tests/transport.rs`
---
ported comments cite 28 documents or invariants Loom does not have:
wire/bearer.rs:46: AGENTS.md invariant 3      … (12 files; full list in red-boundaries.log)
turn_loop/agent.rs:55: `docs/design/0002-sub-agents-structured-output-hooks.md`   (6 sites)
wire/turn.rs:111: `ROADMAP.md`                 (3 sites)
test result: FAILED. 1 passed; 2 failed
```
The first run also flagged `agents/*.md` and `agents/<name>.md`. Those are paths inside a run's workspace, not repository documents, so I fixed my scan and reran; the output above is the rerun.

Mutant, on a scratch copy of the tree with its own target dir. I changed `messages/project.rs:530` from `Some(CredentialKind::Oauth)` to `Some(CredentialKind::ApiKey)`, which stops the OAuth request from getting its preamble block.
- The unit's whole suite stays green: `cargo test --workspace --locked` EXIT=0, 479 passed.
- My carried contract file fails on it: `the_request_a_subscription_token_sends_matches_its_own_pinned_fixture` and `a_subscription_token_opens_the_system_with_the_client_preamble_and_nothing_else` FAILED, 23 passed, EXIT=101.

**3. Suite run, after the cases existed**

`cargo fmt --check` EXIT=0. `cargo clippy --workspace --all-targets --locked -- -D warnings` EXIT=0. `cargo test --workspace --locked --no-fail-fast` EXIT=101: 505 passed, 2 failed, 507 executed. The only failing binary is `adversary_harness_port_boundaries` (1 passed, 2 failed). `-- --list` shows all 28 of my cases in this tree. Without `--no-fail-fast`, cargo stops at the first red binary and reports 443 executed.

**4. Findings**

- **F1, the tests that consumed the fixtures were not ported** (warning, CONFIRMED, introduced). Harness 798325f0 has 64 integration tests for the two wires; none came across:

  | Harness file | tests per wire |
  |---|---|
  | `tests/contract.rs` | messages 11, responses 7 |
  | `tests/summary_request.rs` | 2 each |
  | `tests/transport.rs` | 3 (messages only) |
  | `tests/provider_emulated.rs` | messages 21, responses 18 |

  - Loom reads only `turn-request.json`. The other 12 copied fixtures — the OAuth request, the streams, the manifests and the event inventories — are read by nothing.
  - `harness/mod.rs` says the crates were carried "with its source unchanged except for paths", which does not mention that the integration tests were left behind.
  - What reaches it: any change to the OAuth body, the headers or stream decoding passes the unit's suite, as the mutant shows.
  - Fix: keep `adversary_harness_port_contract.rs`, renamed if wanted. `provider_emulated.rs` (39 cases) needs a Python endpoint and a socket, so it needs a Rust port and should be recorded as not carried.
- **F2, two comments name a guard Loom does not have** (warning, CONFIRMED, introduced). `messages/mod.rs:113` and `responses/mod.rs:86` say a `transport.rs` test fails if the two wires stop agreeing. No such file exists in Loom. My red case `boundaries.rs:77` shows this. What reaches it: whoever edits `TRANSPORT` next, starting with `story:harness-loop-port`.
- **F3, 28 citations point at nothing in Loom** (warning, CONFIRMED, introduced). `AGENTS.md invariant N` appears at 19 sites; in Harness these are numbered invariants 3–12, and Loom's `AGENTS.md` has none. `docs/design/0002-…` appears 6 times and `ROADMAP.md` 3 times; neither exists in Loom. My red case `boundaries.rs:104` shows this. Fix: cite "Harness `AGENTS.md` invariant N at 798325f0", or carry the invariants over.
- **F4, the crate boundaries are no longer enforced** (note, NEEDS-CHANGE, introduced). In Harness each rule below was a compile error. In one Loom crate, `harness/mod.rs` makes every module `pub mod`, so any module can name any other:
  - the messages adapter must not import the responses adapter;
  - the loop depends only on the wire module;
  - the wire module has no reqwest or tokio.

  That a violation would now compile is inferred from visibility; I did not build it. My green case `boundaries.rs:218` is the guard.
- **F5, the stricter lint rules were dropped** (note, NEEDS-CHANGE, introduced). Harness denies `clippy::all` and `clippy::pedantic` across its workspace. Loom has no `[lints]` section, so the ported code is held only to clippy's default lints.
- **F6, some ported unit tests open loopback sockets** (note, CONFIRMED, introduced; the tests themselves are unchanged from Harness). They bind `127.0.0.1:0` at `http/exchange.rs:161,232,238` and `http/transport.rs:600,804,810`, and connect to `127.0.0.1:1` at `http/exchange.rs:196,216` and `http/transport.rs:649,684`. Nothing reaches an external host and no ported file uses `std::fs`.

**5. Attacked and could not break**
- **Source fidelity:** after normalising paths and running rustfmt, all 36 files are identical to Harness except the hostname line at `responses/mod.rs:105`. The 392 unit tests match Harness's 392.
- **Wire bytes:** the OAuth request, both header sets, every stream (turn, error, failed, incomplete), both inventories and both manifests match. That is 25 cases green.
- **Fixtures:** `diff -r` against Harness 798325f0 shows them byte-identical.
- **Licence:** all 36 files carry SPDX Apache-2.0 on line 1, and no proprietary marker remains. Dependency licences are all permissive: Apache/MIT, Unicode-3.0, ISC, BSD-3-Clause, Zlib, CDLA-Permissive-2.0 (webpki-roots) and MIT-0. None is copyleft.
- **Names:** the 6 Gates forbidden literals and the 9 private repository names have 0 hits. The replaced hostname had contained one of those names.
- **Footprint:**
  - reqwest uses rustls with ring and webpki-roots, HTTP/1 only; no native-tls, OpenSSL, h2 or compression.
  - tokio has rt, time and macros, plus net, io-util and sync via hyper-util.
  - serde_json features are the same as Harness's.
  - The lock grows from 36 to 217 packages.
- **Scanners:**
  - no-hand-model exits 0: none of the 140 ported type names collides with the 73 names ESS reserves.
  - drift exits 0.
  - docs-check exits 0.
- **Harness unchanged:** Harness HEAD is 798325f0 and `Cargo.lock` names no Harness source.

**6. Paths written outside the worktree.** All are under `~/.cache/ga-wave-2026-10-04-w10/loom-harness-crate-port/scratch/adv/`:
- `h/` (Harness export)
- `norm/`
- `port.diff`
- `compiled.json`
- `private.txt`
- `lock-base.txt`, `lock-new.txt`, `lock-harness.txt`
- `tree-lic.txt`
- `red-boundaries.log`, `contract.log`, `suite.log`
- `mutant/` (tree copy)
- `mutant-target/` (577M)
- `mutant-suite.log`, `mutant-contract.log`

Separately, I wrote and then deleted `lits.txt` there. I also built in the assigned dir `~/.cache/b10x-target/loom-w10-harness-crate-port`.

**7.**
```findings
- file: crates/loom/src/harness/mod.rs
  line: 5
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Harness's 64 integration tests for the two wires were not ported, so 12 of the 13 copied fixture files are read by nothing and a mutant dropping the OAuth preamble passes all 479 tests."
- file: crates/loom/src/harness/messages/mod.rs
  line: 113
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "This comment and responses/mod.rs:86 name a transport.rs test as the guard that keeps the two wires' transport settings in agreement, and no such file exists in Loom."
- file: crates/loom/src/harness/turn_loop/mod.rs
  line: 59
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "28 ported comments cite Harness's numbered AGENTS.md invariants, docs/design/0002 or ROADMAP.md, none of which exists in Loom."
- file: crates/loom/src/harness/mod.rs
  line: 21
  category: judgement
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Rules Harness enforced through crate dependencies (messages never imports responses, the loop depends only on the wire, the wire has no reqwest or tokio) are now enforced only by the adversary boundary test."
- file: crates/loom/Cargo.toml
  category: judgement
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Harness denies clippy::all and clippy::pedantic across its workspace; Loom has no lints section, so the ported code is held only to default clippy."
- file: crates/loom/src/harness/http/transport.rs
  line: 600
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Ported unit tests in http/transport.rs and http/exchange.rs bind and connect to loopback sockets, so cargo test needs loopback networking."
```
