# Bounded context recorded qualification

Recorded on 2026-10-06 against implementation `a8cb641` with the tests in
[`bounded_context.rs`](../../crates/loom-intake-slice/tests/bounded_context.rs).
These are deterministic model replies through Commission's actual governed runtime,
the local executor, disposable Git repositories and actual fixture test processes.
No live provider, network or credentials are used. Fixtures explicitly select
`UnconfinedRunner`; this record does not qualify confinement.

The matched workflow performs 78 actions in each policy: an actual failing test, a refused
outside-workspace inspection, 72 successful inspections, a reference-based edit, a passing
test, another edit and a final inspection. The bounded policy additionally retrieves the
first test failure in both selection and argument generation and lists retained results
to construct the edit after the early event has retired. Each mode produces the same final
file bytes, failing and passing test observations, refusal and evidence count. After the
second edit, the typed state retains the earlier passing test's revision and explicitly
reports that it does not validate the current revision.

Executed command:

```console
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 cargo test -p b10x-loom-intake-slice --locked --test bounded_context -- --nocapture
```

Captured result:

```text
matched bounded context: legacy_bytes=12451429 bounded_bytes=4526606 legacy_calls=157 bounded_calls=162 bounded_max_request_bytes=45431
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The totals count the serialized `TurnRequest`, including classification, schemas, escaped
responses and all five additional retrieval turns. The JSON context report is checked
against every request observed at the model port. Every bounded request remains below
65,536 bytes. Opaque run-local identifiers can vary the exact byte totals between runs.
Provider input, cache-read, cache-write and output counters remain `null` in
this fixture because the recorded provider supplies none. Reports are checked to exclude
the intent and source contents. Request-byte reduction is measured; token reduction,
cache effectiveness, quality under live model decisions and proportional cost savings
are not established by this record. The default remains `legacy`.

The remaining tests establish:

- Inspected text that claims a passing test does not become typed test state.
- A changed approval requirement is rechecked after history retrieval and argument
  generation, before any edit reaches the workspace.
- Unicode, quoting and backslashes are included in the serialized request measurements;
  oversized escaped lookup responses are omitted whole with an explicit lookup error.
- Mandatory intent that cannot fit fails before any model call.
- The history archive's 4,096-event and 16 MiB limits each stop the next model call.
- A ninth lookup ends selection or refuses arguments without performing another lookup.
- Existing recent events form an unchanged request prefix between checkpoints.
- Result-store exhaustion on the final budgeted step returns an explicit error, preserves
  an earlier committed edit, prints the completed effects, writes the context report and
  makes no subsequent model request.

The focused Clippy gate also ran with warnings denied:

```console
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 cargo clippy -p b10x-loom-intake-slice --locked --test bounded_context -- -D warnings
```
