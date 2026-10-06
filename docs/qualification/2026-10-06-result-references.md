# Recorded-model result-reference qualification

This record covers the governed local software-change slice, not the ported AgentLoop or an
installed Claude/Codex process. No paid model request was made.

## Observable workflow

`crates/loom-intake-slice/tests/result_reference_workflow.rs` uses the public `run::run` entry point,
Commission, the local executor, a temporary Git workspace and recorded model replies. The model
selects inspection, requests a byte range, composes a replacement from two references and a literal,
then runs the existing test action. Destination bytes match the intended UTF-8/CRLF file exactly.

Measured on 2026-10-06 with `cargo test -p b10x-loom-intake-slice --locked --test
result_reference_workflow -- --nocapture`:

| Measurement | Observed |
| --- | ---: |
| Original full file | 134,439 bytes |
| Generated composition arguments | 534 bytes |
| Largest serialized model request in the fixture | 6,402 bytes |
| Recorded agent turns | 7 |
| Passing acceptance tests | 6 |

The generated composition is about 252 times smaller than this fixture's file. That comparison is
bytes, not tokenizer output, and not an aggregate before/after API bill. One optional lookup adds
one argument-generation turn. Small results can cost more when descriptor overhead dominates.

The remaining acceptance cases establish that forged digests cannot write, references cannot
bypass a changed approval requirement, literal arguments still work, captures remain accessible
within their run after transcript trimming, and process output references contain only the existing
runner's captured tail. The latter may include framing beyond its raw output budget; bounded
selection is used rather than assuming the whole artifact fits one lookup.

## Review and local invariants

The independent review prompted four fixes before approval: explicit object-root tool schemas,
bounded lookup metadata/context, preview fallback when storage fills, and byte-bounded catalogue
pagination for descriptors lost to transcript truncation. Regression tests cover these boundaries.
Result-store unit tests additionally cover exact numeric lexemes, JSON pointer escaping, duplicate
JSON keys, malformed selectors, UTF-8 boundaries, compositions, capacity and run isolation.

The ESS-first commit is `0c692ab`: `task intake-drift` failed there because the generated result
domain did not yet exist, then passed after regeneration. This is a data-only ESS domain; zero
synthesized command scenarios are not counted as behavioral proof.

## Repository gates

`cargo fetch --locked`, `task check`, `task plan` and `task website` passed. The full check includes
specification gates, generated-code drift, formatting, workspace Clippy with warnings denied,
workspace tests and documentation drift. Existing opt-in ignored tests were not enabled. Planning
validation retains an unrelated warning about the older ESS hardening review's prose-only findings.

## Limits of the claim

Provider projection was reviewed offline (Responses uses non-strict tool schemas); live endpoint
acceptance has not been exercised. Task quality, fresh input tokens, cached tokens, generated
tokens, dollar cost, retries and elapsed task time require a matched live evaluation before a
rollout claim. Storage is bounded, in memory and run-local; discarded upstream output and expired
runs cannot be recovered. This change neither rewrites historical sessions nor trains a model.
