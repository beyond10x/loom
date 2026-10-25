---
format: aep.planning-md/3
id: review-result:adversary-agent-http-binding-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:agent-http-binding, unit commit a96c9230 on base 62ea2a89
relations:
- reviews: story:agent-http-binding
revision: 1
---
Adversary pass 1 on `story:agent-http-binding` (wave 2026-10-25-agents, unit tree `loom-w2-agent-http-binding`, branch `impl/agent-http-binding`, unit commit `a96c9230` on base `62ea2a89`), `aep:adversary` (opus), 2026-10-25. The report, in the adversary's own words except where shortened.

```
unit: story:agent-http-binding, tree loom-w2-agent-http-binding at a96c9230 plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 286→293, red 5
origin: introduced 7 / pre-existing 0 / undecided 0
needs-coordinator: none
```

**Cases** in `crates/loom-agent/tests/adv_http_binding.rs`, each run alone before the suite:

| case | asserts | now | red output (verbatim) |
|---|---|---|---|
| `adv_an_empty_bearer_variable_is_refused_before_any_request` | a set but empty `bearer_env` gives `BearerMissing` and no connection | red | `Ok(HttpAnswer { status: 200, ... }); the endpoint received: Some("GET /api/v1/query HTTP/1.1\r\nauthorization: Bearer \r\n...` |
| `adv_a_cut_answer_stays_within_max_bytes_on_a_character_boundary` | a cut body is at most `max_bytes` | red | `the cut answer is 103 bytes, past max_bytes 101: "�éé"` |
| `adv_a_non_utf8_answer_stays_within_max_bytes` | 100 bytes of 0xFF with `max_bytes` 100 stays at 100 | red | `the answer is 300 bytes, past max_bytes 100` |
| `adv_a_backslash_dot_dot_path_is_refused_even_when_listed` | D2 also holds for `\..\` | red | `path_allowed admits "/api\\..\\secret"; ... the endpoint received Some("GET /secret HTTP/1.1")` |
| `adv_a_redirect_location_is_held_to_max_bytes` | the redirect refusal stays near `max_bytes` | red | `the refusal carries 8289 bytes of the endpoint's location past max_bytes 100` |
| `adv_a_listener_that_never_answers_is_unavailable_within_the_timeout` | a silent listener gives `Unavailable` within 1 s | green | |
| `adv_an_endless_chunked_body_is_cut_at_max_bytes` | a chunked body with no length is cut at 100 and marked | green | |

Suite after the cases: `cargo test --locked --no-fail-fast -p b10x-loom-agent -p b10x-loom-cli -p b10x-loom-plugin -p b10x-loom-conformance` → exit 101, 288 passed, 5 failed.

**Judgement.** `bearer_missing` only checks `var_os(..).is_none()` (`http.rs:186`), so an empty variable sends `Bearer ` with no token. A body cut mid-character is decoded lossily to 1-2 bytes over `max_bytes` (`http.rs:299`); a non-UTF-8 body grows up to 3x (`http.rs:302`). `path_allowed` accepts a raw `\` and the URL parser treats `\..\` as `..` (`http.rs:95`); `agent check` did not validate `allow_paths`. The endpoint's `Location` goes uncut into the refusal and the transcript (`http.rs:289`). ESS conformance takes `path_allowed` as an input, so a `path_allowed()=true` mutant passes every scenario (`crates/loom-conformance/src/agent.rs:170`, worked out from the code, not run). `reqwest` is declared with `default-features = false`; https works only through feature unification from `b10x-llm-http` with webpki roots, and there is no cluster-CA setting.

**Held up:** paths (trailing slash, `//`, case, unicode, prefix-of-allowed, `?`/`#`, absolute-URI paths) refused by the exact match and the leading-`/` rule; a body of exactly `max_bytes` comes back whole; an endless chunked body is cut; gzip not compiled in; no redirect is followed so the bearer never reaches a redirect target; refusal text names the variable, never its value; the timeout holds; the response body stays inside the escaped data block.

**Fix round (opus implementor, same tree, uncommitted on a96c9230 at the time of this record):** ESS first (the bearer is missing when unset, empty or blank; the path rule; the 256-byte location cap; a new `agent check` scenario refusing a listed `/api/v1\..\admin`; 23 scenarios valid). D8 one path rule `loom_sdk::agent::plain_http_path` for the decoder and `path_allowed`, refusing `\` and `.`/`..` segments; D9 a redirect refusal carries at most 256 bytes of the location and never more than `max_bytes`; D10 non-UTF-8 bytes are dropped, not replaced; D11 the blank-bearer case is a unit test inside `http.rs`, no test sets a variable; D12 "holds no text" vs "is unset". Gate after the round: 299 passed, 0 failed across the four crates, sdk 63; clippy clean on five crates; fmt clean; `task drift` and `task docs-check` current. All seven findings: `review_outcome` fixed (the conformance-input and cluster-CA notes by lines in the qualification doc; a cluster-CA setting is a later story).

```findings
[
{"file":"crates/loom-agent/src/http.rs","line":186,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"An empty bearer_env variable is not BearerMissing; the request goes out with 'authorization: Bearer ' and no token."},
{"file":"crates/loom-agent/src/http.rs","line":299,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"A body cut mid-character is decoded lossily to max_bytes+1..2 bytes with U+FFFD, against HttpAnswer's 'at most max_bytes'."},
{"file":"crates/loom-agent/src/http.rs","line":302,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"A non-UTF-8 body read whole grows up to 3x max_bytes after lossy decoding; no endpoint shown to serve one."},
{"file":"crates/loom-agent/src/http.rs","line":95,"category":"acceptance","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"path_allowed admits a listed '/api\\..\\secret', which the URL parser sends as GET /secret, so D2's 'refused even when listed' is false."},
{"file":"crates/loom-agent/src/http.rs","line":289,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A redirect's Location reaches the refusal and the transcript uncut, bypassing max_bytes (8289 bytes at max_bytes 100)."},
{"file":"crates/loom-conformance/src/agent.rs","line":170,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"ESS conformance takes path_allowed as an input, so a path_allowed()=true mutant passes every scenario; only the Rust test catches it."},
{"file":"crates/loom-agent/Cargo.toml","line":21,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"https works only through feature unification from b10x-llm-http with webpki roots, with no cluster-CA setting for an in-cluster TLS endpoint."}
]
```
