---
format: aep.planning-md/3
id: story:stderr-walk-withholds-token-after-joined-escape
kind: story
status: draft
title: The stderr walk withholds a token after an escape joined to a word
tags:
- from-finding
- security
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 1
---
## Problem

An escape byte that follows a letter or digit and precedes a token prefix, as in
`debug: value=x\x1bghp_<secret>`, is still kept. The walk checks three forms of a line:

- the shown form, which reads the escape sequence and drops the prefix's first letter;
- the bare form;
- the dropped form, which removes control bytes and so joins `x` to the token, so the prefix no
  longer starts a word.

None of the three holds a known prefix at the start of a word, so the token reaches the transcript,
the observer, the error and the failure record.

## Acceptance

- A case in `crates/loom-session-executor/tests/` that runs the scripted child with stderr
  `starting\ndebug: value=x\u{1b}ghp_<secret>\n<refused line>\n` and asserts that the secret reaches
  neither the transcript, the observed lines, the error nor the record tail. It is written red
  first; the U2 implementor of wave 2026-11-05-w3 confirmed it is missed today.
- A suggested fourth form replaces each control character with U+FFFD and consumes nothing. Any fix
  must keep the walk linear and keep every line that the existing cases keep.

## Origin

Gap reported by the U2 implementor of wave 2026-11-05-w3, outside that unit's scope.
