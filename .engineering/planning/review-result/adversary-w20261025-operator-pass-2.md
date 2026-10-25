---
format: aep.planning-md/3
id: review-result:adversary-w20261025-operator-pass-2
kind: review-result
status: active
title: Adversary pass 2 on story:loom-operator
tags:
- non-interactive
relations:
- reviews: story:loom-operator
revision: 1
---
```
unit: story:loom-operator, head 5ef99bf8 on impl/loom-operator (worktree ~/.local/state/worktree/trees/b10x/loom/loom-operator-u1, plus one untracked test file)
verdict: NEEDS-CHANGE (red: 5 failing cases; one blocker)
cases: executed 62→71, red 5
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-operator/u1-adversary-2/ (logs, mutant diffs, and a scratch export ws/ with its own 2.8G target/)
needs-coordinator: none
```

**1. Diff.** `git --no-pager diff --stat` is empty: no tracked file changed. The one untracked file is `crates/loom-operator/tests/adv2_w20261025_operator.rs`, a test file. I changed no implementation file, nothing under `ess/` or `.engineering/`, and ran no git write verb.

**2. Cases** (each run alone first; the verbatim output is in `u1-adversary-2/alone.log`)

| Case | Asserts | Now |
|---|---|---|
| `adv2_a_token_in_a_toml_comment_never_reaches_the_config_map` | no request body carries `xoxb-000-fake` from `# token = "…"` | red: `left: [(PATCH, "/api/v1/namespaces/agents/configmaps/platform-operator-spec")]` |
| `adv2_a_token_after_a_word_in_a_toml_string_never_reaches_the_config_map` | same, for `authorization = "token ghp_000fake"` | red: same ConfigMap PATCH |
| `adv2_a_refused_generation_deletes_the_service_account_an_earlier_generation_named` | the ServiceAccount generation 1 named is gone once generation 2 is Refused (two variants: generation 2 renames the account, or generation 2 does not decode) | red: no DELETE for `serviceaccounts/operator-account` |
| `adv2_a_child_not_owned_refusal_clears_once_the_child_is_removed` | once the leftover ConfigMap is removed, the same generation runs, as it does after a controller restart | red: `left: []`, the record still holds `Refused … ChildNotOwned` (a restarted context publishes `Running`) |
| `adv2_an_egress_entry_naming_no_host_opens_no_port_the_allow_list_did_not` | no egress rule without a peer, except on ports of allowed hosts or the model route | red: `left: [6443]` |
| `adv2_an_unobserved_rollout_does_not_degrade_and_an_observed_one_does` | the `observedGeneration < generation` guard holds | green; only this case catches mutant m1 |
| `adv2_a_predecessors_child_gone_or_replaced_meanwhile_does_not_fail_the_reconcile` | a DELETE answered 404 or 409 still ends in requeue after 30 s | green; only this case catches m3 and m3b |
| `adv2_a_resource_replaced_while_degraded_and_again_runs_as_the_newest` | uid A while Degraded, then B, then C: predecessors deleted, then applied with the newest uid | green |
| `adv2_every_states_status_is_admitted_by_the_crds_status_schema` | all 4 states × 3 checks × 3 reads against the CRD `status` schema | green |

Two deviations from the ordering rule:
- **Mid-string token case:** my first version used `Bearer xoxb-…` and was green, because `Bearer ` is itself one of the walk's token prefixes (`crates/loom-agent/src/lib.rs:67`). I respelt it before the suite ran and reran it alone: red.
- **Raced-delete case:** I added it after the first suite run, ran it alone (green), then ran the suite again.

**3. Suite runs** (`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only`)
- **Final run.** `cargo test -p b10x-loom-operator --locked --no-fail-fast` exited 101:
  - adv2: `test result: FAILED. 4 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s`
  - lib `ok. 22 passed`, adv `ok. 9 passed`, chart `ok. 6 passed`, cli `ok. 6 passed`, reconcile `ok. 11 passed`, render `ok. 8 passed`
- **The "before" count (62)** is a run made after my file existed, with adv2 left out (`--lib --bins --test adv_w20261025_operator --test chart --test cli --test reconcile --test render`): exited 0, 22+0+9+6+6+11+8.
- **Other checks:**
  - `cargo test -p b10x-loom-resource --locked --no-fail-fast` exited 0 (9+1+2+16+5).
  - `task conform` (ESS 0.69.0) exited 0: `test result: ok. 3 passed`.
  - `cargo clippy -p b10x-loom-operator --all-targets --locked -- -D warnings` exited 0.

**4. Findings**

| Id | file:line | Finding | What reaches it | Fix (named, not applied) | Verdict | Severity | Origin |
|---|---|---|---|---|---|---|---|
| F1 | `crates/loom-operator/src/admission.rs:290` | The correction for pass-1 F3 walks only the decoded TOML, but the ConfigMap gets the raw text. A token in a TOML comment, or one that is not the first word of a string, reaches `connectors.toml`. The `90952cc6` word walk caught both: this is a regression. | anyone writing `spec.connectorsConfig`; commenting out an old token line is the ordinary path | keep the word walk over the raw text beside the decoded walk. Probe p1 in scratch: both cases go green, the rest of the suite stays green. Or write the re-serialized TOML to the ConfigMap. | NEEDS-CHANGE | blocker | introduced |
| F2 | `admission.rs:326` (renders at `render.rs:403`) | An egress entry with neither `host` nor `namespace` decodes, skips the `allowedEgressHosts` check, and renders a rule with no peer: every destination on its port. With the default `allowedEgressHosts: []` it still opens any port. | anyone writing a LoomAgent | refuse an egress entry naming neither host nor namespace, naming the member | NEEDS-CHANGE | warning | introduced |
| F3 | `crates/loom-operator/src/reconcile.rs:571` | A `ChildNotOwned` refusal depends on the cluster, not the spec, but it sticks. After the named object is removed, the resource stays Refused at that generation until a spec edit or a controller restart; a restarted controller runs it. | a leftover or orphaned child (for example after `--cascade=orphan`), then the operator follows the message | on a Refused resource whose refusal came from the survey, survey again; when nothing foreign remains, forget the record and apply it again, as `hold` does on a uid change | NEEDS-CHANGE | warning | introduced |
| F4 | `reconcile.rs:595` | The Refused arm deletes the ServiceAccount the *refused* generation names. An earlier generation's account is kept when the refused generation renames it or does not decode. This contradicts AGENTS.md § Operator: "on every reconcile each child it controls is deleted". | an account rename plus a refusal, or any Malformed generation after a custom `service_account` | delete by owner uid, for example a ServiceAccount list by `app.kubernetes.io/instance`, or remember the last applied account name | CONFIRMED | note | introduced |
| F5 | `reconcile.rs:448` | Mutant m1 (`<` flipped to `>`) passes every pre-existing case; only my rollout case catches it. | every rollout: just after an apply, `observedGeneration` is below `generation` | keep `adv2_an_unobserved_rollout…` in the suite | CONFIRMED | note | introduced |
| F6 | `reconcile.rs:320` | Mutants m3 (409 dropped) and m3b (404 dropped) pass every pre-existing case; the fake never answers DELETE with anything but 200. | a predecessor's children being garbage-collected while they are deleted (404) | keep `adv2_a_predecessors_child_gone…` in the suite | CONFIRMED | note | introduced |

All six are introduced: the base `92eb26d6` has no `crates/loom-operator`. Mutant diffs and logs are in scratch as `m*.diff` and `m*.log`.

**5. Attacked and not broken**
- **The strict routes parser:** it refuses a `base_url` with userinfo, query or fragment, and its messages never repeat a value (`malformed()` gives line and column only). Header values cannot be declared.
- **Bounds:** the 4096 opener bound is held at its edge by the unit test. The ±64-byte edge of the 2 MiB bound is unreachable from a cluster. I did not test this: it rests on the etcd default request limit of 1.5 MiB, and I don't know this cluster's setting.
- **uid change:** while Degraded, while Running, and twice in a row: green.
- **survey:** a child controlled by a LoomAgent of another name is `ChildNotOwned` (mutant m2 is caught).
- **Mutants caught:** m4 routes walk dropped, m5 `blockOwnerDeletion` dropped, m6 Available/Unavailable swapped, m7 uid change never forgets, m8 egress allow-list dropped.
- **The relaxed assertion** at `tests/reconcile.rs:306`: nothing is lost. GET reads are pinned to exactly the five children, and any other method still fails.
- **Scenarios:** `loom.resource` has 26 at base and 28 at head. The two added are `observe-running`/`still-running` and `observe-degraded`/`still-degraded`; none was dropped. `ess/SKIPPED.md` lists none.
- **NetworkPolicy:** both policy types; DNS UDP and TCP 53 to `kube-system`; ingress only from the declared namespaces. Apart from F2 it matches § 7.
- **Status:** every state is admitted by the CRD `status` schema, Pending and Degraded included.

**6. Paths written outside the worktree:** `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-operator/u1-adversary-2/`. It holds the logs, two mutant scripts (`mutants.zsh`, `mutants-2.zsh`), `pristine/`, `base/` (the base `ess/`), and `ws/` (a `git archive 5ef99bf8` export) with its own `target/` of 2.8G. My session lease is released.

```findings
[
  {"file": "crates/loom-operator/src/admission.rs", "line": 290, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "F1: the decoded-TOML walk misses a token in a comment or after a word in a string, yet the raw text goes to the ConfigMap; 90952cc6's word walk caught both"},
  {"file": "crates/loom-operator/src/admission.rs", "line": 326, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "F2: an egress entry naming neither host nor namespace bypasses allowedEgressHosts and renders egress to every destination on its port"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 571, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "F3: a ChildNotOwned refusal never clears at the same generation after the child is removed, while a restarted controller runs the resource"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 595, "category": "acceptance", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "F4: a Refused resource keeps the ServiceAccount an earlier generation named when the refused generation renames it or does not decode"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 448, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "F5: flipping the observedGeneration guard leaves every pre-existing case green"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 320, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "F6: dropping the 404 or the 409 tolerance of delete_uid leaves every pre-existing case green"}
]
```
