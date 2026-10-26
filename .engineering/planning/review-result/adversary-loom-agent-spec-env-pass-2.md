---
format: aep.planning-md/3
id: review-result:adversary-loom-agent-spec-env-pass-2
kind: review-result
status: active
title: Adversary pass 2, wave 2026-10-26-cmdv u3, story:loom-agent-spec-env, at 49b77d96
relations:
- reviews: story:loom-agent-spec-env
revision: 1
---
# Adversary pass 2, wave 2026-10-26-cmdv u3, story:loom-agent-spec-env, at 49b77d96

Agent `aep:adversary` (opus), the same agent as pass 1, on the correction commit. Header and
findings block verbatim; prose condensed.

```
unit: story:loom-agent-spec-env (u3) pass 2, tree loom-cmdv-u3 at 49b77d96 plus one untracked test file
verdict: red
cases: executed 142→145, red 1
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (suite, clippy and task logs)
needs-coordinator: yes. One red case: the expanded walk does not know the service environment the kubelet expands from. It is note severity, so the coordinator decides whether it holds the unit.
```

Cases in `crates/loom-operator/tests/adv_cmdv_u3_expansion.rs`:
`adv_a_url_joined_with_the_kubernetes_service_link_is_refused` (red),
`adv_a_reference_the_kubelet_does_not_expand_is_admitted` (green),
`adv_tokens_joined_over_several_references_are_refused` (green).

Suite: `cargo test -p b10x-loom-operator --locked --no-fail-fast` exit 101, 144 passed, 1 failed (the
red case). Golden renders unchanged by the `controller_env` refactor; clippy, `task drift`,
`task crd-drift`, `task docs-check` exit 0. The kubelet's expansion order (envFrom, earlier env,
service environment) is from a reading of its source, not a runtime observation. Attacked and held:
`$$`, an unclosed `$(`, nested references, a later name, a self-reference, no re-expansion of
substituted text, the expansion budget, the linear duplicate check, refusal texts.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 391, "category": "correctness", "severity": "note", "verdict": "needs-revision", "origin": "introduced", "message": "the expanded walk knows only controller_env and spec.env, while the kubelet also expands the always-present service environment, so $(KUBERNETES_PORT) supplies a scheme that turns an admitted value into a password URL, contrary to the 'whole environment' wording in AGENTS.md:933 and README.md:356; red: adv_a_url_joined_with_the_kubernetes_service_link_is_refused"}
]
```

## Routing

Sent back to the implementor as round 2: an unknown `$(NAME)` is walked as a possible URL scheme,
and the doc lines name what the walk knows. The coordinator reads the round-2 diff; no third pass.
