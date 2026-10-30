---
format: aep.planning-md/3
id: review-result:adversary-session-executor-keyless-loopback-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-30-w2 u1, story:session-executor-keyless-loopback, at 0a51ddbf
relations:
- reviews: story:session-executor-keyless-loopback
revision: 1
---
# Adversary pass 1, wave 2026-10-30-w2 u1, story:session-executor-keyless-loopback, at 0a51ddbf

```findings
[
{"file":"crates/loom-session-executor/src/lib.rs","line":2985,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"endpoint_keyless.rs had no loopback URL with a password and no user, so dropping the password check stayed green; covered by adversary_a_loopback_url_with_a_password_and_no_user_is_refused"},
{"file":"crates/loom-session-executor/src/lib.rs","line":2984,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"every non-http URL in endpoint_keyless.rs was refused without the scheme check; ws://127.0.0.1 now covers it"}
]
```
