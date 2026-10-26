---
format: aep.planning-md/3
id: review-result:adversary-trial-fixes-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:loom-agent-trial-fixes
tags:
- non-interactive
relations:
- reviews: story:loom-agent-trial-fixes
revision: 1
---
```
unit: loom-u-trial-fixes
verdict: red
cases: executed 0→6 (two new test files), red 2
origin: introduced 3, pre-existing 0, undecided 1
write-outside-worktree: none
needs-coordinator: should a serve-variant spec without `runtime.connectors` be refused at admission?
```

Pass 1 on `1e000699..9d8c420c` (ESS `fdb86b1f`, implementation `9d8c420c`). No implementation file
was touched; the pass added `crates/loom-operator/tests/adv_trial_fixes.rs` (4 cases) and
`crates/loom-cli/tests/adv_stage.rs` (2 cases).

| case | now | red output |
|---|---|---|
| `adv_serve_variant_without_runtime_connectors_is_refused_at_admission` | red | admitted and rendered a serve pod whose `agent serve` refuses to start |
| `adv_a_key_removed_from_the_secret_is_not_left_staged_by_a_rerun` | red | `left: ["old.pem", "policy.json"] right: ["policy.json"]` |
| `adv_staged_files_under_a_setgid_world_writable_root_pass_the_gates_reader` | green | |
| `adv_serve_variant_with_secret_files_alone_stages_first_and_sets_fs_group` | green | |
| `adv_command_variant_with_secret_files_and_no_workspace_sets_fs_group` | green | |
| `adv_secret_files_at_the_workspace_path_is_refused` | green | |

Findings:

| # | where | severity | finding | answer |
|---|---|---|---|---|
| F1 | `crates/loom-sdk/src/agent.rs:731` | warning | an `AgentServe` resource whose `runtime` has no `connectors` is admitted, then `agent serve` refuses at start: a crash loop where the decoder refused it before | fixed: admission refuses it `Malformed` at `spec.agent.runtime.connectors` for `AgentServe` only (a spec with no `runtime` at all stays admitted, as before) |
| F2 | `crates/loom-cli/src/stage.rs:180` | note | a rerun leaves a staged file whose key left the Secret | fixed: `stage` removes a file in `--to` that no current key names and refuses a directory there |
| F3 | the agent-check scenario | warning | the scenario says `serveable` for a spec `agent serve` refuses | answered: `CheckAgentSpec` does not read `runtime` (as for `loops` and `writes.mode`); the scenario's comment now says so and names the two refusals |
| F4 | `crates/loom-agent/src/serve.rs:132` | note | the serve refusal without connectors had no test | fixed: `a_spec_without_runtime_connectors_is_refused_before_anything_runs` |
| F5 | `crates/loom-operator/src/render.rs:322` | note | staged files no longer follow Secret rotation while the pod runs | documented in AGENTS.md, README, design § 9a and the CHANGELOG bullet: the copy is taken when the pod starts |
| F6 | `crates/loom-cli/src/stage.rs:183` | note, infeasible | a Secret swap during staging could mix versions | accepted: the window is the init container's milliseconds, no test |

Attacked and not broken (reasoned against kubelet behaviour, not run on a cluster): the agent sees
the bind-mounted `files` directory as the file's parent, uid 10001 mode 0700, files 0600, so the
gates `protected_read` check passes; planted links are replaced, not followed; sibling-prefix
escapes fail; FIFO and directory keys are skipped; umask is overridden by the explicit chmod.
