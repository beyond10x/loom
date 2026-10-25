---
format: aep.planning-md/3
id: review-result:adversary-w20261025-resource-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:loom-agent-resource-domain
tags:
- non-interactive
relations:
- reviews: story:loom-agent-resource-domain
revision: 1
---
```
unit: story:loom-agent-resource-domain, worktree loom-agent-domain-u1 at f0acace9 plus 2 untracked test files
verdict: red
cases: executed 151→154, red 3
origin: introduced 4 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 directory (assigned scratch)
needs-coordinator: F1's fix sits in loom-sdk, outside this unit's scope: fix the decoder or change the docs
```

`crates/loom-sdk/src/models.rs:194` — **F1**: without `model.roles`, the CRD admits the document and the decoder refuses it ("field `roles` is missing"). 1 disagreement in 135 single-member omissions. Fix: read an absent `roles` as empty, or keep `roles` required and correct crd.rs:25 and AGENTS.md § Resource. Introduced.
`crates/loom-resource/src/crd.rs:511` — **F2**: the CRD admits a binding carrying the other variant's member and the decoder refuses it, 10 of 10 cases. Declared members are not pruned, so they reach the pod. Fix: give each `oneOf` entry `not: {anyOf: [{required: [f]}…]}` over the other variants' fields, and let `violations` accept that inside a junctor. Introduced.
`crates/loom-resource/src/crd.rs:71` — **F3**: ESS 0.69.0 writes `Map<String,String>` as `{"type":"object","additionalProperties":{…}}` (I ran it on a one-struct spec). The projection turns that into `{"type":"object"}` with no error, so the API server prunes every entry. No Map is in the current spec. Fix: carry an `additionalProperties` that is a schema, or refuse it as `CrdError::Keyword`. Introduced.
`deploy/crd/loomagents.loom.beyond10x.dev.yaml:529` — **J1**: the design (§ 5.1, and the § 8 e2e test waiting for `status.phase: Running`) names `status.phase`, `status.deployment` and `status.conditions`. The CRD has `status.state` and neither of the other two. Nothing in the diff records the rename. Fix: amend the design or record it in AGENTS.md § Resource. Introduced.
`crates/loom-resource/src/crd.rs:15` — **J2**: "the pod check refuses an undeclared member" does not hold for members the API server prunes, such as a misspelt `bearer_env`. Whether a client refuses them first depends on its field-validation mode; I don't know what ArgoCD sends. Not run against an API server. Fix: correct the comment. Introduced.
`crates/loom-sdk/src/agent.rs:287` — **J3**: the decoder accepts any non-blank text as `deadline`. The CRD declares `format: date-time`, so the API server is expected to refuse `2026-12-31` (not run against one). Fix: parse RFC 3339 in `timestamp()`. Pre-existing.
`ess/domains/agent.yaml:435` — **J5**: `format`, `port` (1–65535), non-blank text and the `bearer_env` name are enforced only by the decoder. The CRD admits violations and the pod refuses them (read, not run). Fix: declare them in ESS. Pre-existing.

1. **Diff.** `git --no-pager diff --stat` is empty: nothing tracked changed. `git status --short` lists only two new test files, `crates/loom-sdk/tests/adversary_w20261025_resource_crd.rs` (327 lines) and `crates/loom-resource/tests/adversary_w20261025_crd_projection.rs` (80 lines). No `Cargo.toml` or `Cargo.lock` changed. I put the decoder cases in loom-sdk, which already has the decoder, `serde_json` and `serde_yaml_ng`, so adding a dev-dependency was not needed.

2. **Cases, each run alone first and red** (verbatim summaries):
   - `the_crd_and_the_decoder_agree_on_every_member_the_operator_example_can_omit`: "disagree on 1 of 135 omissions: without /model/roles: the decoder refuses it (`model`: field `roles` is missing), the CRD schema admits it". The test validates against the committed CRD with its own small validator: it prunes undeclared members, ignores `format` and `pattern`, and panics on any keyword it does not model. Each case first checks that the operator example passes both the decoder and the schema.
   - `the_crd_refuses_a_binding_that_carries_a_member_of_the_other_variant`: "admits 10 of 10 cross-variant bindings the decoder refuses". Examples are `bindings[0]` (connectors) with `base_url`, and `bindings[1]` (http) with `adapter`.
   - `a_map_field_is_carried_or_refused_never_projected_to_an_object_that_prunes_it`: "`labels` (Map<String, String>) projects to {"type":"object"} … the projection reported nothing".

3. **Suite run, after the cases existed.** `cargo test --locked --no-fail-fast -p b10x-loom-resource -p b10x-loom-sdk -p b10x-loom-conformance -p loom-xtask` gave 151 passed, 3 failed, EXIT=101; only my 3 cases failed. The same run with those 3 deselected by `--skip` gave 151 passed, EXIT=0. `conform` passed 3 of 3. `ess/SKIPPED.md` names no `loom.resource` scenario, so all 26 synthesized `loom.resource` scenarios ran and passed, including the 3 `state/*/refuses/RecordCheck`. Clippy `-D warnings` and `cargo fmt --check` are clean on the new files.

4. **Attacked and not broken:**
   - Record: stale, equal and newer generations from all four states; a stale check after a respecify; `RecordCheck` outside Pending; delete then re-apply (created again, Pending, no `check`); two records sharing nothing. Four probes, all green, deleted.
   - Conformance target: the outcome mapping is 1:1 with the generated behaviour.
   - `crd-drift`: a changed header, an extra newline and CRLF each exit 1 and name the file. With no `ess` on PATH it prints "run ess: No such file…"; that message shape predates this unit.
   - Structural rules: none broken in the committed CRD.
   - `Decimal` projected as `number`: a quoted decimal is refused by the API server; crd.rs:21 documents this, so not raised.

5. **Written outside the worktree:** only the assigned scratch directory (`design.md`, `mapspec/`, `suite-after.log`, `suite-before.log`, `synth.out`, `synth.txt`, `validate.txt`).

My worktree lease is released.

```findings
- file: crates/loom-sdk/src/models.rs
  line: 194
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the projection never requires a list on the premise that the decoder reads an absent list as empty, but the decoder refuses a model without roles, so the CRD admits a document the pod refuses
- file: crates/loom-resource/src/crd.rs
  line: 511
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the flattened bindings union admits members of the other variant, which the API server neither prunes nor refuses and the decoder refuses as unknown members
- file: crates/loom-resource/src/crd.rs
  line: 71
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: a Map field, which ESS 0.69.0 writes as additionalProperties with a schema, silently projects to an object the API server prunes empty instead of being refused; no Map exists in the spec today
- file: deploy/crd/loomagents.loom.beyond10x.dev.yaml
  line: 529
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the design names status.phase, status.deployment and status.conditions, the CRD publishes status.state without the other two, and the rename is recorded nowhere
- file: crates/loom-resource/src/crd.rs
  line: 15
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the comment says the pod check refuses an undeclared member, but a member the API server prunes never reaches the pod, so whether a typo is refused depends on the client's field validation
- file: crates/loom-sdk/src/agent.rs
  line: 287
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: the decoder accepts any text as a deadline while the CRD declares date-time, so a document the decoder reads is expected to be refused by the API server
- file: ess/domains/agent.yaml
  line: 435
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: the format value, port range, non-blank text and bearer_env name are enforced only by the decoder and not declared in ESS, so the CRD admits documents the pod refuses
```
