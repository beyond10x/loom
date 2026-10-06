# Result references in the governed local slice

## Owner and purpose

Loom owns this capability. `loom-intake-slice::Briefing` captures results from performed actions;
`results::ResultStore` keeps immutable UTF-8 content and generated `intake.results` metadata.
Context Lens remains an offline analysis consumer. The ported `AgentLoop` has a separate execution
path and is not changed by this implementation.

The first integration targets the current `b10x-loom run` software-change slice. It reduces bytes
sent to models and bytes they must generate while preserving exact selected content. It is not a
claim of lower measured API cost or unchanged task quality; those require controlled live tasks.

## Capture and lifetime

Each successfully captured inspected file receives a unique result ID and SHA-256 of its exact
UTF-8 bytes. Equal content from separate captures has separate provenance. One briefing owns the
store; cloning that briefing intentionally shares it, while a fresh briefing gets a distinct
scope. IDs are opaque references, not authority. A store only resolves objects it owns.

Results remain available after the rolling 64-entry transcript drops their previews. They expire
with the briefing. The first version holds data in bounded memory, not a durable cross-process
archive: 16 MiB per result, 64 MiB total, 1,024 captures. Capacity failure produces an explicit
unavailable result with a bounded non-referenceable preview, never eviction of an existing
reference or re-execution of a producing action. Retained artifacts can be discovered through
`{"$list_results": 0}` and successive `next_offset` pages, even when a multi-file result or older
transcript entry was truncated. Each page returns at most eight descriptors without payloads.

The initial preview contains at most 1,024 UTF-8 bytes. Inspected files show their beginning; test
results show the end of the already-captured tail. Offsets, total size, truncation and completeness
are explicit. Existing test runners discard earlier output and may decode lossily, so these
artifacts are always partial. They do not recover original stdout/stderr or prove tests passed.
Exit status, timeout and implementation revision remain visible; the verifier remains unchanged.
A test run's briefing entry quotes the executor's report beside the captured output: how the
command ended, whether the work tree had uncommitted changes, and the confinement applied.

## Selection and rendering

An example envelope, with placeholders for the actual capture identity and digest:

```json
{
  "result": "opaque-result-id",
  "sha256": "source-sha256",
  "select": {"kind": "lines", "start": 4, "end": 12},
  "rendering": "text"
}
```

- `whole` selects all stored text.
- `bytes` uses zero-based, half-open offsets and requires UTF-8 boundaries.
- `lines` uses one-based inclusive bounds. LF terminates lines, retaining both LF and a preceding
  CR. The final unterminated segment is a line; a final LF adds no phantom empty line.
- `json_pointer` uses one RFC 6901 pointer and explicit array indices. There are no wildcards,
  expressions, implicit coercions, shell execution, or network lookups.

Whole, byte and line selections render text only. JSON-pointer `text` rendering requires a string
and returns its decoded UTF-8 contents. `json` rendering returns the original selected JSON value
lexeme, retaining numeric spelling and precision. Malformed JSON and duplicate object keys are
refused. Missing properties and JSON null remain different outcomes.

The source digest is mandatory. Unknown or wrong-scope IDs, digest mismatch, invalid selectors and
oversized selections fail explicitly. Matching a digest alone never imports another run's result.

## Generation, admission and effects

The model-facing `action_arguments` schema accepts ordinary arguments and a local lookup envelope
`{"$read_result": <reference>}`. A lookup selects at most 8 KiB and adds a quoted data response to
the current argument-generation exchange. At most eight lookups are allowed before a final action
answer; errors are explicit and consume the same budget. They are not frontier actions and cannot
read anything not already captured by this briefing. Catalogue pages share this lookup budget.
Encoded read references are capped at 4 KiB. The data the lookups of one argument generation
return together (selected text and catalogue pages) is capped at 64 KiB, counted after JSON
escaping, so eight full lookups of text that needs no escaping fit. Each response adds a fixed
framing outside that total. A malformed lookup, an invalid catalogue offset, or a lookup that
would pass the total is answered as a lookup error, never an aborted generation. A ninth lookup
refuses the step, as an unresolvable edit does (below). The model is told these limits. Lookup
responses do not echo model-supplied references.

For `repository.edit`, each `contents` value can remain a literal string or use:

```json
{"segments": [{"literal": "prefix\n"}, {"ref": {"result": "opaque-result-id", "sha256": "source-sha256", "select": {"kind": "whole"}, "rendering": "text"}}, {"literal": "\nsuffix"}]}
```

Up to 256 ordered segments form a string, with at most 16 MiB of expanded content across an edit.
No path or permission is inferred from the source. Resolution occurs in `ModelArguments` before
Loom returns the `ProposedAction`. Commission therefore admits actual arguments; the existing
executor still enforces workspace paths, ignored files, transaction behavior and ordinary schemas.
A missing or invalid reference never reaches an effect. Edit contents that do not resolve refuse
the step, as arguments the executor refuses do: nothing is proposed, the step counts against the
budget, and the next briefing records `refused:` with the reason. The run goes on. An unreachable
model alone suspends it. A later briefing records edit-content sizes and digests rather than
reintroducing the expanded bodies into model context.

## Validation and remaining experiments

The ESS data contract is generated from `ess/intake/domains/results.yaml`. Its data-only domain
does not synthesize behavioral conformance cases. Rust unit and integrated recorded-model tests
exercise selectors, scope, limits and inspect-to-edit behavior, including ordinary literal edits.
The workflow compares final file bytes and request/argument bytes, not tokenizer counts or prices.
The [recorded-model qualification](../qualification/2026-10-06-result-references.md) records the
measured fixture, refusal cases, independent review and limits of those claims.

Live evaluation must compare matched tasks with meaningful acceptance tests and reviewed outcomes,
counting retrieval calls, cache reads/writes, fresh input, generated output, retries and task time.
Preview size and context retention policy are separate treatments. Nothing here automatically
changes Claude Code or Codex sessions, reduces their historical usage, or trains a model. The
reference/composition contract is a concrete future target for normalized-action training.
