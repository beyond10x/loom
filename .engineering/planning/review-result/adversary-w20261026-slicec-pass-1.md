---
format: aep.planning-md/3
id: review-result:adversary-w20261026-slicec-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:loom-agent-command-variant
tags:
- non-interactive
relations:
- reviews: story:loom-agent-command-variant
revision: 1
---
```
unit: loom-u-command-variant, story:loom-agent-command-variant, beyond10x/loom HEAD 6a522d88 plus one untracked test file
verdict: red (NEEDS-CHANGE: 2 warnings, INFEASIBLE: 1 note)
cases: executed 85→88, red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path (log under assigned scratch)
needs-coordinator: no
```

The unit is red: 3 failing cases. Two let a credential through admission and into the Deployment; the third is a refusal reason the docs don't list.

**1. Diff stat**

`git diff --stat` is empty (no tracked file changed). `git status --short` shows one untracked file, a test file:
```
?? crates/loom-operator/tests/adv_w20261026_slicec.rs   | 144 +++
```

**2. Cases added** (`crates/loom-operator/tests/adv_w20261026_slicec.rs`). Each was red when run alone, before the suite:

| case | asserts | now |
|---|---|---|
| `adv_slicec_a_user_colon_token_argument_in_spec_command_is_refused` | args `[run, -u, 'x-access-token:ghp_000fake…']` is refused `SecretLiteral at spec.command.args[2]` | red: `ADMITTED and rendered (8530 bytes of children)` (:108) |
| `adv_slicec_a_header_colon_key_argument_in_spec_command_is_refused` | args `[run, -H, 'x-api-key:sk-ant-000fake…']` is refused the same way | red: `ADMITTED and rendered (8528 bytes of children)` (:120) |
| `adv_slicec_a_command_spec_agent_beyond_the_decoder_flow_bound_is_refused_as_the_decoder_does` | a `spec.agent` with 4097 `{` is refused `Malformed` on both variants | serve gives `Malformed`; Command gives `refused: TooLarge at spec.agent: it holds more than 4096 '[' or '{' bytes, more than the credential walk reads` (:140) |

**3. Suite run** (after the cases existed)

`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo test --locked --no-fail-fast -p b10x-loom-operator` exited 101.

| binary | result |
|---|---|
| lib | 25 ok |
| main | 0 |
| adv2_w20261025 | 9 ok |
| adv_w20261025 | 9 ok |
| adv_w20261026_slicec | 0 passed, 3 FAILED |
| chart | 6 ok |
| cli | 6 ok |
| reconcile | 16 ok |
| render | 14 ok |
| doc | 0 |

The "before" count of 85 is this same run with my binary left out.

**4. Findings**

- **`admission.rs:327` (`command_credential`), with `words` at :327-ish callers using the separator set** — NEEDS-CHANGE, introduced. `words()` does not split on `:`, so a single argument like `user:ghp_…` or `x-api-key:sk-…` is never checked against the token prefixes. Neither is it caught by the URL rule, because it has no `://`. The token is admitted and lands in the resource and the Deployment's `args`. What reaches it: `curl -u user:token` or `-H key:value` with no space after the colon, both ordinary forms in a `spec.command` for the takt delivery. Fix: add `:` (and arguably `;`) to the separators in `words()`. That also tightens the walk over `connectorsConfig`.
- **`admission.rs:407`** — INFEASIBLE, introduced. The Command branch calls `credential()`, whose `first_credential` returns `TooLarge`. The module doc (:7-9) and AGENTS.md list only `SecretLiteral`/`Malformed` for this step. What reaches it: nothing found, since a real spec would need more than 4096 `[`/`{`. Fix: fix the doc line, or map the error to `Malformed` as `check_text` does.

**5. Attacked and could not break.** These were run as probes, all green, then removed from the file:
- **Mount paths:**
  - Refused as expected: relative paths, `/zzw/../config`, `//zzw`, a trailing `/`, `/`, `/var/lib`, `/var/lib/agent/x`, `/tmp`, `/etc`, `/config`, `/etc/agent`, `/./s`, NUL, tab, 1025 bytes, and a workspace and Secret path that are equal or nested.
  - Admitted as expected: `/tmpfoo`, `/configx`, `/TMP`, `/var/lib/agentx`.
  - No refusal detail repeated the path.
- **Claim names:** `'takt-workspace '` (trailing space) is refused `ClaimNotAllowed`.
- **Credential shapes already caught:** `=`-joined, `;takt`, `Bearer` inside `sh -c`, a URL with user information, a tab-prefixed `sk-`. No refusal repeated the value.
- **`/config/agent.yaml`:** byte-identical to serve's `agent.yaml` for the same `spec.agent`.

Read but not executed: switching variant between generations (server-side apply drops `initContainers`, `args` and `env`; `respecified` clears the variant), the serve golden file (unchanged in `git diff ea73042c..HEAD`), and conformance.

**6. Written outside the worktree**
- `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv1/suite-operator.log`

```findings
- file: crates/loom-operator/src/admission.rs
  line: 327
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: spec.command words are not split on ':', so a user:ghp_ or header x-api-key:sk- argument is admitted and rendered into the Deployment instead of refused SecretLiteral
- file: crates/loom-operator/src/admission.rs
  line: 407
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a Command spec.agent beyond 4096 flow openers is refused TooLarge, a reason the module doc and AGENTS.md do not list for that step, while serve refuses the same document Malformed
```
