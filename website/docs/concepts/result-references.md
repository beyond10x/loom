---
title: Reusing tool results
sidebar_position: 9
description: Bounded previews and exact result references in the local software-change slice.
lede: Loom retains inspected content outside the rolling model briefing and resolves references before an action reaches admission.
source: crates/loom-intake-slice/src/results.rs; crates/loom-intake-slice/src/selector.rs; crates/loom-intake-slice/tests/result_reference_workflow.rs; ess/intake/domains/results.yaml
---

:::caution[Measured offline only]
This capability shipped in `0.3.0`. Recorded-model checks establish content preservation and
bounded request bytes; they do not establish live task quality or billed-token savings.
:::

An inspection keeps each file's immutable UTF-8 contents for the run and shows the model a preview
of at most 1,024 bytes, with a result ID, SHA-256, size and completeness. The argument generator can
ask for a range or JSON value when it needs more. It can also compose edit contents from stored
selections and new literal text, so it need not generate unchanged file content again.

References select a whole result, a half-open byte range, inclusive lines or one RFC 6901 JSON
pointer. Text ranges retain original newlines and require UTF-8 boundaries. JSON selection can
return a decoded string or the original value lexeme, retaining number precision. Invalid ranges,
unknown references, wrong digests and ambiguous JSON fail explicitly.

Expansion happens before Loom proposes the action. Commission checks the resulting ordinary
arguments and the existing executor performs them under its usual workspace checks. Result text
grants no authority and is never verification evidence. A later briefing keeps edit sizes and
digests instead of repeating expanded file bodies.

The store holds at most 16 MiB per result, 64 MiB per briefing and 1,024 results. It outlives the
rolling transcript within the run, but expires with the briefing and is not a persistent session
archive. Argument generation allows eight lookups of at most 8 KiB each, and up to 16 MiB of expanded
edit content. Lookup calls have overhead and must be counted in an efficiency comparison.
The data all lookups return together is capped at 64 KiB after JSON escaping; a lookup that fails
or would pass that total is answered as a lookup error and still counts, and a ninth lookup or edit
contents that do not resolve refuse the step, which the model is told. If an inspection's
descriptors fall outside the transcript, `{"$list_results": 0}` starts a paginated catalogue of
retained results. Catalogue pages share the lookup budget. When storage fills, new results retain
a bounded preview but explicitly have no reference; existing references remain usable.

Test output is explicitly partial: the existing runner already retains a tail. Loom preserves that
captured representation and its exit/timeout metadata; it cannot retrieve output discarded upstream.
This integration affects the governed local slice, not the separately ported Harness loop or an
installed Claude Code or Codex process.
