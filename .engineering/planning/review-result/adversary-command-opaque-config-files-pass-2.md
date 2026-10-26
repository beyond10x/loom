---
format: aep.planning-md/3
id: review-result:adversary-command-opaque-config-files-pass-2
kind: review-result
status: active
title: Adversary pass 2, wave 2026-10-26-cmdv u4, story:command-opaque-config-files, at 8d91f345
relations:
- reviews: story:command-opaque-config-files
revision: 1
---
# Adversary pass 2, wave 2026-10-26-cmdv u4, story:command-opaque-config-files, at 8d91f345

Agent `aep:adversary` (opus), the same agent as pass 1, on the correction commit. Header and
findings block verbatim; prose condensed.

```
unit: u4 pass 2, tree loom-cmdv-u4 at 8d91f345 plus one untracked test file
verdict: red
cases: executed 275→279 (operator 159→163, b10x-loom-agent 116→116), red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (diff, suite, clippy and task logs)
needs-coordinator: yes. Three red cases need routing: a duplicate key hides an escaped token inside a value, and the scan-error stop (D-u4.11) hides a later document from a reader that splits the stream at `---`.
```

Cases in `crates/loom-operator/tests/adv_cmdv_u4_yaml_stream.rs`:
`adv_an_escaped_token_inside_a_value_beside_a_duplicate_key_is_refused` (red),
`adv_an_escaped_token_in_a_document_after_a_duplicate_key_is_refused` (red),
`adv_an_escaped_token_after_a_document_with_a_scan_error_is_refused` (red),
`adv_a_token_before_nesting_past_the_recursion_limit_is_refused` (green).

Suites: `b10x-loom-operator` exit 101 (160 passed, 3 failed, the new cases); `b10x-loom-agent` exit
0 (116 passed); clippy, `task drift`, `task crd-drift`, `task docs-check` exit 0. That Kubernetes's
YAML reader splits a stream at `---` lines is from a reading of its code, not a run. Held: a
credential found before a limit is `Found`, not masked; `check_text` and `first` unchanged for
`spec.agent`; no refusal repeats a value; the `..` prefix refusal; tags walked.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 865, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "yaml_words stops at the first document serde_yaml_ng::Value refuses (a duplicate key), so an escaped token inside a value of that or any later document is never walked as words, though a typed reader decodes it, contrary to AGENTS.md:954; red: adv_an_escaped_token_inside_a_value_beside_a_duplicate_key_is_refused, adv_an_escaped_token_in_a_document_after_a_duplicate_key_is_refused"},
{"file": "crates/loom-agent/src/credentials.rs", "line": 133, "category": "judgement", "severity": "note", "verdict": "needs-revision", "origin": "introduced", "message": "D-u4.11 answers Clean at a scan error, so a document after it, which a reader splitting at `---` lines decodes, is never walked; red: adv_an_escaped_token_after_a_document_with_a_scan_error_is_refused"}
]
```

## Routing

Round 2 to the implementor: the stream walker collects the words itself (no `Value` decode), and
after a scan error the content is also split at `---` lines and each piece walked under the same
budget (overriding D-u4.11). The coordinator reads the round-2 diff; no third pass.
