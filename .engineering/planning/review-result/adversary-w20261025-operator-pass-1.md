---
format: aep.planning-md/3
id: review-result:adversary-w20261025-operator-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:loom-operator
tags:
- non-interactive
relations:
- reviews: story:loom-operator
revision: 1
---
```
unit: story:loom-operator
verdict: red
cases: executed 45→54, red 7
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-operator/u1-adversary/ (11 logs)
needs-coordinator: no
```

These findings cover head 90952cc6. I changed no tracked file, so `git diff --stat` is empty. The one file I added is `crates/loom-operator/tests/adv_w20261025_operator.rs`: 9 cases, 7 red and 2 green, each run alone first (log: `u1-adversary/alone.log`).

| Id | file:line | Finding | Fix | Verdict | Severity | Origin |
|---|---|---|---|---|---|---|
| F1 | `crates/loom-operator/src/admission.rs:205` | A routes `auth.secret` holding a plain value instead of an `env:`/`file:` reference (`not-a-reference-000-fake`) passes admission and is written into the ConfigMap. Design § 7 forbids this, and llm's routes parser itself treats it as "a literal secret". | Parse `spec.routes` with llm's strict `Routes` parser during admission and refuse on its error. That error never echoes the value. | CONFIRMED | blocker | introduced |
| F2 | `admission.rs:110` | A routes text containing more than 4096 `{`/`[` bytes, even inside a quoted string, is never walked: `credentials::bound` returns None for non-agent text. An `xoxb-000-fake` secret beside 4097 braces is admitted. | Refuse a routes text the walk did not cover, or walk it without the agent decoder's bound. | CONFIRMED (must be crafted on purpose) | warning | introduced |
| F3 | `admission.rs:218` | The word split over `connectorsConfig` misses TOML escapes. `token = "xoxb-000-fake"` is admitted, and the TOML parser decodes it to the token. | Decode the TOML (with the `toml` crate), then walk the decoded strings. | CONFIRMED (must be crafted on purpose) | warning | introduced |
| F4 | `reconcile.rs:313` | The record is keyed by name only, never by uid. (a) Refused at gen 1, then deleted and re-created fixed within the 300 s requeue: the outcome is `unchanged`, it stays Refused with the old message, gets no children, and stays that way until the controller restarts. (b) Gen 3 replaced by a new gen 1: `stale` on every reconcile, no children, no status ever. | Keep a uid per name and call `DeleteLoomAgent` when the uid changes. | CONFIRMED | blocker | introduced |
| F5 | `reconcile.rs:240` | Children are force-applied (`force=true`) without checking who owns them. A `service_account` naming the controller's own ServiceAccount (`loom-agents-operator`) or `default` takes that account over: it gets a `controller: true` owner reference, so deleting the resource also deletes the account. The delete path (`delete_owned`) does check ownership; the apply path does not. | `get_opt` before applying; refuse or fail when the existing object has no controller reference with this uid. | NEEDS-CHANGE | warning | introduced |
| F6 | `reconcile.rs:323` | Old children are deleted only in the reconcile that records the refusal. If one delete fails there (a 500), the retry finds the resource already Refused and deletes nothing, so gen 1's Deployment keeps running under a Refused status. | Delete owned children on every reconcile while the state is Refused. | CONFIRMED (needs a failed API delete) | warning | introduced |

What reaches each finding:
- **F1–F3:** anyone who writes a LoomAgent.
- **F4:** in kube-runtime 4.2.0, `controller/mod.rs:749-750` triggers only on `applied_objects()`, so deleting the resource itself never triggers a reconcile. A refused resource owns no Deployment whose deletion would trigger one either. Only its 300 s requeue forgets it.
- **F5:** a spec's `deploy.kubernetes.service_account`, or a resource named `<release>-operator`.
- **F6:** a transient API error.

**Suite runs** (all with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only`). The "before" count is a run made after my file existed, with my test target left out:
- `cargo test -p b10x-loom-operator --locked --no-fail-fast` exited 101:
  - `test result: FAILED. 2 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s` (adv)
  - lib 17, chart 6, cli 6, reconcile 8, render 8: all `ok`
- `cargo test … --lib --bins --test reconcile --test render --test cli --test chart` exited 0, 45 passed.
- `cargo test -p b10x-loom-resource --locked --no-fail-fast` exited 0 (9+1+2+16+5 `ok`).

**What I attacked and could not break:**
- **Gate steps:** `ess specify validate --strict-requires` (0.69.0) exits 0. `task spec`, `ess-gate`, `drift`, `crd-drift`, `chart-check`, `conform` and `docs-check` all exit 0. `ess/SKIPPED.md` lists no scenario. I did not count `loom.resource` scenarios individually.
- **ConfigMap round trip:** `agent.yaml` decodes to the same `AgentSpec` as the example, `classify_threshold: 0.5` included (green case).
- **Status:** every status published as Running or Refused stays within the CRD's enums and required members (green case).
- **CLI:** `crd` is byte-identical to `deploy/crd`. `render` on a directory exits 1; on a non-LoomAgent it exits 2.
- **Chart:** its CRD copy is byte-identical to `deploy/crd`. No comments in `values.yaml`, and no `/home/` or organization literal in the chart.
- **Mutants:** I checked these by reading only, without running them. My charter forbids mutating the tree under attack, and I did not build a scratch copy of the workspace.
  - Dropping the routes walk is caught by `reconcile.rs:393` and `render.rs:401`.
  - Dropping `blockOwnerDeletion` is caught by `render.rs:183` and the golden file.
  - Swapping Available/Unavailable is caught by `reconcile.rs:322` and `:335`.

```findings
[
  {"file": "crates/loom-operator/src/admission.rs", "line": 205, "category": "acceptance", "severity": "blocker", "verdict": "CONFIRMED", "origin": "introduced", "message": "F1: a routes auth.secret that is a value, not an env:/file: reference, is admitted and written into the ConfigMap"},
  {"file": "crates/loom-operator/src/admission.rs", "line": 110, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "F2: a routes text with more than 4096 brace bytes is never walked, so a token beside them is admitted"},
  {"file": "crates/loom-operator/src/admission.rs", "line": 218, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "F3: a connectorsConfig token spelt with a TOML unicode escape passes the word walk"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 313, "category": "concurrency", "severity": "blocker", "verdict": "CONFIRMED", "origin": "introduced", "message": "F4: a resource re-created under its name is unchanged or stale against the old record and never gets checked or gets children"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 240, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "F5: children are force-applied over objects another owner holds, taking over the controller's own or the default ServiceAccount"},
  {"file": "crates/loom-operator/src/reconcile.rs", "line": 323, "category": "concurrency", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "F6: a refusal whose child delete failed once never deletes that child on the retry"}
]
```
