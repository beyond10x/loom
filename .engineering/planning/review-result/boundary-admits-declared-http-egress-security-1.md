---
format: aep.planning-md/3
id: review-result:boundary-admits-declared-http-egress-security-1
kind: review-result
status: active
title: 'Security review, pass 1: boundary-admits-declared-http-egress (wave 2026-11-21-w6, U2, head 984ce66e)'
relations:
- reviews: story:boundary-admits-declared-http-egress
revision: 1
---
unit: U2 `story:boundary-admits-declared-http-egress`, worktree `loom-w1121f-u2` at head 984ce66e (base 9b4170fb)
verdict: red
cases: executed 319→330, red 2
origin: introduced 2, pre-existing 1, undecided 0
wrote-outside-worktree: 5 paths (listed below)
needs-coordinator: yes. F1 conflicts with the story: its condition 1 covers only `allowedBoundaryUpstreams`, so whether the `modelRoute` namespace is closed too is a story decision. The implementor cannot settle it alone.

The two red cases are the ones I predicted, and all 9 I predicted green are green. No other test in the package went red. Both red cases stay as written.

`git diff --stat`: empty. My only file is the new, untracked test file, so nothing under review was touched.

**Cases**, in `crates/loom-operator/tests/security_w20261121w6_u2.rs`.

Run alone with `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-operator --locked --test security_w20261121w6_u2`: exit 101, `test result: FAILED. 9 passed; 2 failed`.
- F1, `the_model_route_namespace_is_not_opened_beside_a_boundary`, failed at `security_w20261121w6_u2.rs:233:25`:
  `an entry opening the modelRoute's namespace (agents) on its port is admitted beside a boundary: [...]`
  The rendered policy holds a peer for namespace `agents`, `podSelector {app: model-route}`, TCP port 8080.
- F2, `an_upstream_named_by_a_short_service_name_still_closes_its_namespace`, failed at `:209:18`:
  `an entry in the namespace of the allowedBoundaryUpstreams URL http://gw.store:8080 is admitted: condition 1 reads no namespace from a short service name`

Green as predicted:
- `an_upstream_namespace_in_any_spelling_is_never_admitted`
- `an_upstream_url_in_any_cluster_service_spelling_closes_its_namespace`
- `an_entry_without_a_selector_is_refused_at_its_selector`
- `each_entry_needs_a_binding_in_its_own_namespace`
- `a_binding_out_of_shape_ties_no_entry`
- `the_allow_list_matches_one_entry_exactly`
- `an_admitted_entry_renders_one_peer_no_wider_and_changes_nothing_else`
- `the_base_adversary_case_was_refused_before_the_egress_check`
- `the_configuration_refuses_an_http_egress_entry_out_of_shape`

**Suite**: `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-operator --locked --no-fail-fast`, exit 101. 330 tests ran, 328 passed. The only failures are the 2 above, in `error: 1 target failed`.

**Invariants**: the run left every answer as before. Number 2 is now confirmed by the run: it holds for every URL that `cluster_service` accepts, and fails for the configuration's `modelRoute` (F1) and for a short-name upstream (F2). Numbers 1, 3, 4, 5, 6 and 8 hold; 3, 4, 5 and 8 now also have a passing case.

**Findings**, covering 984ce66e:
- F1, `admission.rs:1534`: condition 1 never checks the namespace of the configuration's `modelRoute`, which is a signing proxy that holds a credential. With a matching binding and allow-list entry, the agent container gets a direct path to that proxy, around the boundary.
  - It is reached only if the operator lists that entry, against the warning on the operator page.
  - warning, CONFIRMED, introduced.
- F2, `admission.rs:1504`: an upstream URL with a short-name or IP host reads as no namespace at all, so condition 1 leaves that namespace open.
  - INFEASIBLE, because no resource can declare such an upstream (`:1409`), and the operator would also have to list the namespace.
  - warning, introduced.
- F3, `adversary_d243_u1.rs:294` at the base: the test passed without testing anything, because its resource was refused `NoModelRoute` at step 4 before the egress check. The run confirms it. This unit fixed the test.
  - note, CONFIRMED, pre-existing.
- F4, `ess/scenarios/resource-admission/`: no scenario covers the condition-2 refusal (reported at `selector`) or the refusal of a namespace that is not a valid name.
  - note, CONFIRMED, introduced.
- F5, `AGENTS.md:1318`: the text puts the namespace-name check before the upstream-namespace check, the reverse of the code. The reason and member are the same; only the detail differs.
  - note, CONFIRMED, introduced.

**Checked and found nothing wrong**: unchanged from the first report: the chart, the CHANGELOG `**Breaking:**` bullet, the manifests and lock, `UNMAPPED:` markers, repository boundaries, § Out of scope, and the prose.

**Disk at the end**: `df -h /` 82G free, `du -sh target` 11G.

**Written outside the worktree**, all in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w6/u2/sec1/`: the directory itself, `src.diff`, `other.diff`, `case-run.log` and `suite-run.log`. The first report named two more diff files there; they were never written.

`git status --short`: `?? crates/loom-operator/tests/security_w20261121w6_u2.rs`

```findings
- file: crates/loom-operator/src/admission.rs
  line: 1534
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "condition 1 ignores the configuration's modelRoute, so an allowed entry can open the credential-holding signing proxy's namespace past the boundary"
- file: crates/loom-operator/src/admission.rs
  line: 1504
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "an allowedBoundaryUpstreams URL with a short-name or IP host reads as no namespace, so condition 1 leaves that namespace open"
- file: crates/loom-operator/tests/adversary_d243_u1.rs
  line: 294
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: "at base the upstream-namespace adversary case was refused NoModelRoute at step 4 and returned on any refusal, so it tested nothing"
- file: ess/scenarios/resource-admission/
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "no scenario covers the condition-2 refusal at selector or the refusal of a namespace that is not a valid name"
- file: AGENTS.md
  line: 1318
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the prose puts the namespace-name check before the upstream-namespace check, the reverse of the code's order"
```
