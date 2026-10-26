---
format: aep.planning-md/3
id: review-result:agent-spec-constraints-adversary-q13
kind: review-result
status: active
title: Adversary pass over the named agent-spec value refusals (wave 2026-10-26-q13)
relations:
- reviews: story:agent-spec-constraints-in-ess
revision: 1
---
## Review

Adversary pass (aep:adversary, opus) over `impl/agent-spec-constraints-in-ess` at `a8dd503c`, wave
2026-10-26-q13. No blocker. Eight adversary tests added in `crates/loom-agent/tests/adv_q13_constraints.rs`,
three red (U+2212 offset sign accepted as a deadline, binding `kind` checked before undeclared
members, a port above i64::MAX answering Malformed); routed to a fix round in the unit. One operator
test (serve variant passes the new reasons through, Command variant stays Malformed) was green but is
not committed in this wave because the operator is frozen for the cluster trial; it is carried to
`story:operator-constraints-in-ess`. Twelve mutants: eleven killed by the unit's suite, M9
(`spelling(NotAnInstant)` misspelled) killed only by loom-cli's `a_value_refusal_prints_its_own_reason`.

Runs: `cargo test -p b10x-loom-agent --locked --no-fail-fast` 84→91 executed, exit 101 (the 3 red);
`-p b10x-loom-operator` 96→97 exit 0; `-p b10x-loom-sdk` 66 exit 0; conform 3 exit 0; drift and
docs check exit 0; `deploy/`, `charts/`, `crates/loom-operator` unchanged since `5642e87e`.

```findings
[
{"file":"crates/loom-sdk/src/agent.rs","line":388,"category":"boundary","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"The decoder accepts a deadline whose offset sign is U+2212 MINUS SIGN ('2026-12-31T23:59:59−05:00'): chrono 0.4.45 parse_rfc3339 allows it; Commission's reader (loom-commission/src/rfc3339.rs) refuses it, so 'as Commission reads it' (AGENTS.md:628) is false for this input. Red: adv_a_deadline_whose_offset_sign_is_a_unicode_minus_is_not_an_instant."},
{"file":"crates/loom-sdk/src/agent.rs","line":687,"category":"contract-drift","severity":"note","verdict":"needs-revision","origin":"introduced","message":"binding() text-checks `kind` before looking for undeclared members, so {kind: ' ', zzz: 1} answers BlankText at bindings[0].kind against the documented order (undeclared member first, Malformed at document). Red: adv_a_binding_answers_its_undeclared_member_before_a_blank_kind."},
{"file":"crates/loom-sdk/src/agent.rs","line":337,"category":"boundary","severity":"note","verdict":"needs-revision","origin":"introduced","message":"A port above i64::MAX (2^63, u64::MAX) answers Malformed, not PortOutOfRange; integer() uses as_i64 only. Red: adv_a_port_beyond_i64_is_out_of_range."},
{"file":"crates/loom-operator/src/admission.rs","line":5,"category":"contract-drift","severity":"note","verdict":"needs-revision","origin":"introduced","message":"The module doc lists the serve variant's reasons without the six new ones the operator now passes through (PortOutOfRange at spec.agent.expose.a2a.port etc.); the Command variant still answers Malformed at spec.agent. Operator out of scope in this wave: carried to story:operator-constraints-in-ess with the adversary's operator test."},
{"file":"crates/loom-sdk/src/agent.rs","line":436,"category":"judgement","severity":"note","verdict":"needs-revision","origin":"pre-existing","message":"The UnknownFormat detail repeats the raw format value verbatim without a length limit; only the credential walk protects it. Same message at 5642e87e line 370."}
]
```
