---
format: aep.planning-md/3
id: review-result:adversary-w2-20261008-loom-llm-credentials-bearer-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w2 adversary, loom story:llm-credentials-bearer, pass 1
relations:
- reviews: story:llm-credentials-bearer
revision: 1
---
## Adversary pass 1, story:llm-credentials-bearer (wave 2026-10-08-w2)

Attacked `impl/llm-credentials-bearer` at 531d1e3 (unit commits 9fd740b..531d1e3).

- verdict: NEEDS-CHANGE, 5 red of 6 new cases, all introduced
- cases: executed 8→14 in `wire_credential` + `adversary_w2_20261008_credentials`
- fixed in 989b6bf; the cases are committed unchanged in efcb823 except the bridge case, renamed to assert the documented behaviour

| case | finding | after 989b6bf |
|---|---|---|
| adv_a_secret_with_a_trailing_newline_is_refused_as_a_credential_naming_the_reference | a resolved secret with a newline failed as a 4-attempt Transport error naming no reference | green: refused Unauthorized, not retried, names the reference |
| adv_a_secret_carrying_a_header_break_is_refused_by_the_source | a CR/LF secret was returned as a bearer | green |
| adv_an_empty_resolved_secret_is_refused_naming_the_reference | empty secret refused without the reference | green |
| adv_the_bridge_blocks_the_callers_runtime_and_refuses_naming_the_reference | the bridge doc claimed it never blocks the caller's runtime; it does | doc corrected; `RESOLVE_BOUND` (30 s) refuses naming the reference |
| adv_a_configuration_decode_error_never_echoes_a_value | a decode error repeated a pasted value | green: names the field only |
| ApiKey header shape | `x-api-key` only | green before and after |

Held: ESS `loom.run.WireCredential` has no secret field; `ResolvedBearer` and `Bearer` Debug output is redacted.
Residual (pre-existing): the header `String` in `responses/mod.rs` and `messages/mod.rs` is not zeroized.
Package `b10x-loom-executor`: 687 passed, 0 failed, 1 ignored after the fix.
