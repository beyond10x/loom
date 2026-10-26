---
format: aep.planning-md/3
id: review-result:adversary-w20261026-slicec-pass-2
kind: review-result
status: active
title: Adversary pass 2 on story:loom-agent-command-variant
tags:
- non-interactive
relations:
- reviews: story:loom-agent-command-variant
revision: 1
---
```
unit: loom-u-command-variant
verdict: red
cases: executed 88→91, red 3
origin: introduced 3, pre-existing 0, undecided 0
write-outside-worktree: ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv2/ (3 files) | none in the tree beyond the new test file
needs-coordinator: no
```

The pass-1 fix holds only partly. I found two more ways to get a credential through `spec.command` admission and into the Deployment's `args`, and wrote three failing cases for them. This covers HEAD 988c6ac9 plus one untracked test file.

**1. Diff stat**

`git --no-pager diff --stat` is empty: no tracked file changed. `git status --short` shows one untracked file, and it is a test file:
```
?? crates/loom-operator/tests/adv2_w20261026_slicec.rs
```

**2. Cases added**

All three are in `crates/loom-operator/tests/adv2_w20261026_slicec.rs`. Each was red the first time it ran, run alone with `cargo test --locked -p b10x-loom-operator --test adv2_w20261026_slicec`, which exited 101. Each case also asserts that the refusal does not repeat the fake value.

| case | asserts | red output (verbatim) |
|---|---|---|
| `adv2_slicec_an_authorization_colon_bearer_argument_in_spec_command_is_refused` | args `[run, -H, 'Authorization:Bearer 000fake…']` is refused `SecretLiteral at spec.command.args[2]` | `:114` `a bearer token in spec.command reached the Deployment: ADMITTED and rendered (8532 bytes of children)` |
| `adv2_slicec_an_authorization_colon_bearer_header_in_a_shell_script_is_refused` | entrypoint `[sh, -c]`, a script `curl -fsS -H Authorization:Bearer 000fake… …` is refused at `args[0]` | `:129` `… ADMITTED and rendered (8596 bytes of children)` |
| `adv2_slicec_a_here_string_token_in_a_shell_script_is_refused` | the script `gh auth login --with-token &lt;&lt;&lt;ghp_000fake… &amp;&amp; exec takt run` is refused at `args[0]` | `:145` `… ADMITTED and rendered (8574 bytes of children)` |

**3. Suite run** (after the cases existed)

`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo test --locked --no-fail-fast -p b10x-loom-operator` exited 101.

| binary | result |
|---|---|
| lib | 25 ok |
| main | 0 |
| adv2_w20261025 | 9 ok |
| adv2_w20261026_slicec | 0 passed, **3 FAILED** |
| adv_w20261025 | 9 ok |
| adv_w20261026_slicec | 3 ok |
| chart | 6 ok |
| cli | 6 ok |
| reconcile | 16 ok |
| render | 14 ok |

The "before" count of 88 is this same run with my binary left out.

**4. Findings**

- **`crates/loom-operator/src/admission.rs:340-350` (`words`)**: NEEDS-CHANGE, introduced.
  - **Measured:** cases 1 and 2. `:` splits `Authorization:Bearer` into the pieces `Authorization` and `Bearer`. The `Bearer ` join is only tried on whole words (`:341`), never on the pieces, so the token is never walked as `Bearer &lt;token&gt;`.
  - **What reaches it:** `curl -H Authorization:Bearer …` written with no space after the colon, inside a `sh -c` script or as a single argument.
  - **Fix:** split on `:` and `;` before the join, so the join also sees the pieces.
- **`crates/loom-operator/src/admission.rs:329`**: NEEDS-CHANGE, introduced.
  - **Measured:** case 3. `&lt;` is not a separator, and the library only matches a prefix at the start of a value (`loom-agent/src/lib.rs:209`). A here-string glued to its word therefore hides `ghp_`. The same holds for `(`, `|`, `$` and backtick.
  - **What reaches it:** the bash here-string `gh auth login --with-token &lt;&lt;&lt;ghp_…` in the entrypoint's `sh -c` script.
  - **Fix:** add shell metacharacters (`&lt;&gt;|&amp;()$` and backtick) to the separators.
- **`crates/loom-operator/src/admission.rs:30-31`**: INFEASIBLE, introduced. This is the residue, a note.
  - **Measured:** a probe, since removed. One argument holding 4097 `{` is refused `TooLarge at spec.command: it holds more than 4096 …`. Step 8 of the module doc and the AGENTS.md list give only `SecretLiteral` for `spec.command`, and AGENTS.md lists `TooLarge` only for routes and connectors.
  - **What reaches it:** nothing found.
  - **Fix:** one doc line.

**5. Attacked and could not break**

These ran as probes. They behaved as claimed and were then removed from the file.
- **Status lifecycle:** Command then Degraded gives `Checked` `SpecAdmitted` and `variant: Command`. `respecified` clears `variant` and gives Pending. A later AgentServe check gives `SpecServeable`.
- **Command with `expose.a2a`:** the pod gets probes and a Service, as AGENTS.md documents ("probes only with expose.a2a"). That is a design choice, not a finding.
- **Pass-1 cases:** green on 988c6ac9. That covers `user:ghp_`, `x-api-key:sk-` and a huge Command `spec.agent` refused `Malformed`.
- **Read only, not executed:** variant switching (the Deployment is `Recreate`, applied server-side by one field manager) and the serve golden file (unchanged).

**6. Written outside the worktree**

- `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv2/cases-alone.log`
- `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv2/probes.log`
- `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv2/suite-operator.log`
- `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-s2/adv2/cases-only.rs.bak`

I acquired the worktree session lease `adv2-slicec-loom-cmdvar-20261026` and released it at the end.

```findings
- file: crates/loom-operator/src/admission.rs
  line: 341
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the Bearer join applies only to whole words, so an Authorization:Bearer &lt;token&gt; argument or script in spec.command is split at ':' but never walked as 'Bearer &lt;token&gt;' and is admitted into the Deployment's args
- file: crates/loom-operator/src/admission.rs
  line: 329
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: shell punctuation such as '&lt;' is no word separator, so a here-string token like &lt;&lt;&lt;ghp_… in a spec.command sh -c script is never walked from its prefix and is admitted
- file: crates/loom-operator/src/admission.rs
  line: 30
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: a spec.command beyond 4096 flow openers is refused TooLarge, a reason the step-8 doc and AGENTS.md do not list for spec.command
```
