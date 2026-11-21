---
format: aep.planning-md/3
id: review-result:boundary-admits-declared-http-egress-security-2
kind: review-result
status: active
title: 'Security review, pass 2: boundary-admits-declared-http-egress (wave 2026-11-21-w6, U2, head 0c9d56e3)'
relations:
- reviews: story:boundary-admits-declared-http-egress
revision: 1
---
unit: U2 `story:boundary-admits-declared-http-egress` pass 2, worktree `loom-w1121f-u2` at head 0c9d56e3 plus my uncommitted test edit
verdict: red. One case fails: the rewritten F2 case, on its admission half.
cases: executed 333→334, red 1
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: 6 paths in `sec2/` (listed below)
needs-coordinator: yes. The implementor has to close the admission half of F2, or the planner has to accept that only the configuration read guards it.

`git diff --stat`: only `crates/loom-operator/tests/security_w20261121w6_u2.rs`, 92 lines added and 18 removed. `cargo fmt --all --check` now exits 0.

1. **F1 is closed.**
   - `admission.rs:1538-1541` adds the `modelRoute` URL to the condition-1 check.
   - My pass-1 case `the_model_route_namespace_is_not_opened_beside_a_boundary` passes unchanged.
   - New case `the_model_route_namespace_in_any_spelling_is_closed`: `test ... ok`. It covers 6 URL spellings times 3 entry spellings, each with the exact reason, member and detail. The resource gives its own `spec.routes`.
   - The model route comes only from `config.model_route`, so a resource's `spec` cannot change it.
2. **F2 is closed at the configuration read.**
   - `names_namespace` (`config.rs:433`) normalizes the URL exactly as `upstream_namespace` (`admission.rs:1495`) does, and accepts only `<svc>.<ns>.svc[.cluster.local]`. Every spelling the read accepts therefore gives admission the same namespace, and none reads as "no namespace".
   - The read refuses an IPv6 literal, a bare name, empty labels, `""` and `http://` (`config.rs:715`).
   - An absent `modelRoute` is still accepted. One with an empty `url` is refused, and the chart renders none when `url` is empty (`configmap.yaml:23`).
   - The CHANGELOG at `:44` names the refused configurations as `**Breaking:**`.
   - The admission half stays open: F2-b below.
3. **My F2 case is rewritten** and now asserts both halves.
   - The read half passes: the configuration read refuses the URL with `allowedBoundaryUpstreams[1].url: "http://gw.store:8080"`.
   - The admission half fails. The case sets that upstream on a configuration built without the read, and the entry is still admitted (`security_w20261121w6_u2.rs:221:18`): `an entry in the namespace of the allowedBoundaryUpstreams URL http://gw.store:8080 is admitted: condition 1 reads no namespace from a short service name`.
4. **The `allowing` helper (`render.rs:1748`) is not weakened.** Its 5 direct callers and the 38 `boundary_config()` uses assert what they asserted before.
   - Before, the configuration read checked nothing about upstream URLs.
   - `serde_json::from_value` uses the same `deny_unknown_fields` deserializer.
   - The new check that the helper now skips is covered by `config.rs:715` and `:741`.
5. **Per-fixture configuration.** Only `resources/with-model-route/config.yaml` exists, and only one scenario uses it, a refusal.
   - It is the shared configuration plus a `modelRoute` and one allow-list entry for `models`. That makes the list wider, so the expected refusal is decided by condition 1 alone.
   - No admitted scenario reads it.
   - The conformance reader at `conformance/admission.rs:122` checks `starts_with` component by component without normalizing `..`. Scenario paths are authored repository content, so I do not raise this as a finding.
6. **F4 and F5 are closed.**
   - The three new scenarios give the code's reason, member and detail; `admission_reasons.rs` 40/40 and the implementor's `task conform` agree.
   - `resource.yaml` and AGENTS.md now give the checks in the code's order.
   - README `:531` and `:539`, `RUN_HELP` and `status.json` are true against the code.
7. **Format:** `cargo fmt -p b10x-loom-operator` was run, and `git diff --stat` names only my file.

Suite: `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-operator --locked --no-fail-fast`, exit 101. 334 tests ran, 333 passed. The only failure is the one above.

**Finding F2-b**, `admission.rs:1505` (with `:1541`):
- Admission still treats a URL it cannot read a namespace from as "no upstream there".
- Only the configuration read stands between such a URL and an open namespace. A `Config` built without `Config::parse` (all its fields are public) gets an open door.
- The shipped binary always goes through the read, so this is INFEASIBLE in the binary.
- Possible fix: in condition 1, refuse when `upstream_namespace` returns `None`.
- Severity warning, origin introduced.

**Could not establish:** I ran no `task conform` of my own and relied on the implementor's log. A URL containing `\` before an `@` could be read differently by a WHATWG URL parser than by our reader. I did not probe it: only an operator could write such a URL.

**Written outside the worktree**, all in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w6/u2/sec2/`: the directory itself, `code.diff`, `case-run.log`, `case-run-2.log`, `fmt.log` and `suite-run.log`.

`git status --short`: ` M crates/loom-operator/tests/security_w20261121w6_u2.rs`

```findings
- file: crates/loom-operator/src/admission.rs
  line: 1505
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "admission still reads an upstream or model-route URL with no readable namespace as no upstream there, so only the configuration read keeps that namespace closed"
```
