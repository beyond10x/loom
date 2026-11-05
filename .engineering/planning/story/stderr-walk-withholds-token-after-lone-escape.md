---
format: aep.planning-md/3
id: story:stderr-walk-withholds-token-after-lone-escape
kind: story
status: draft
title: The stderr walk withholds a token after a lone escape byte
tags:
- from-finding
- security
revision: 1
---
## Problem

A lone ESC before a token is read as a two-character escape sequence that eats the first letter of the
token's prefix, so `ESC ghp_<secret>` is kept as `hp_<secret>` in the transcript, the observer, the error
and the failure record.

## Acceptance

- The case `security_a_token_after_a_lone_escape_reaches_neither_the_transcript_nor_the_record` in
  `crates/loom-session-executor/tests/security_w20261105w2_u4.rs` is green: a token after a lone ESC
  reaches none of the four.

## Origin

Pre-existing finding of the wave 2026-11-05-w2 U4 security review (`inert`, unchanged since 0.57.0).
