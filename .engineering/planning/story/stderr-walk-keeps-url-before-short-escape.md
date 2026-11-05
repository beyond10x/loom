---
format: aep.planning-md/3
id: story:stderr-walk-keeps-url-before-short-escape
kind: story
status: draft
title: The stderr walk keeps a URL line followed by a short escape sequence
tags:
- from-finding
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 1
---
## Problem

The stderr walk's escape-aware form keeps each escape sequence's final byte as text. A two-byte
`ESC ?` or a CSI ending in `@` therefore leaves a `?` or `@` after a URL, and the line can be
withheld although its shown form holds no credential's shape. This only ever over-withholds; it
never leaks.

## Acceptance

- A case in `crates/loom-session-executor/tests/` with a URL followed by `ESC ?`, and one followed by
  a CSI ending in `@`, each kept as shown in the transcript and the tail. Every case that withholds
  today still withholds.

## Origin

Gap reported by the U2 implementor of wave 2026-11-05-w3 after fix round 1. No real log is known to
emit either right after a URL.
