# Provider overload during a clock query

On 2026-10-07, the installed development build at `957a8a7` intermittently stopped after routing
`need to know the current time` to `system-query@1`, reporting
`Refused: the provider refused this request`. Repeating the user's bounded command produced two
completed runs followed by the same failure. The failed selection request was 4,127 serialized
bytes; a later reproduction failed at argument generation with 5,273 bytes. Neither reached the
clock effect.

Temporary instrumentation identified the top-level Responses error code `server_is_overloaded`.
The llm 0.1.7 adapter treated unknown provider codes as refusal. This was an upstream capacity
failure, not a context ceiling failure or a model refusal to read the clock.

## Change and ownership

[llm PR #24](https://github.com/beyond10x/llm/pull/24), pinned at
`c0e97d620c27a8b413facffb119793d6881599f2`, classifies this exact code as transient unavailability
before output. Its shared decoder prevents retry after visible or silent output items, including
for recorded streams. Unknown codes and genuine refusals remain final. Provider error prose is
never copied into the public diagnostic.

Loom's caller orchestration uses llm's error classification and retry policy for at most three
identical requests on the same binding. Each attempt passes through the existing measurement and
request-ceiling boundary. Any offered stream event prevents retry. Cancellation interrupts backoff;
exhaustion retains the final error class, dispatch and usage evidence and names the attempt count.
Effects remain governed by Commission and outside model retries.

The ESS-first commit is `8037406`. Its changed digest made `task intake-drift` fail before
regeneration. The recorded clock recovery test then failed before caller retries were implemented.
The implementation is `81d4b42`.

## Observed verification

Five live executions of the exact user intent with `--context-policy=bounded --workspace=/tmp/foo`
completed, each with one clock effect and three model calls. Every request fit 64 KiB. These runs
encountered no overload, so they qualify the normal live path, not live overload recovery.

Recorded workflow tests inject a transient overload separately at classification, selection and
argument generation. They verify byte-identical retries, one clock read, all attempts measured,
and preservation of failed-attempt usage. Exhaustion and genuine refusal read no clock. Unit tests
exercise cancellation, one/two-second backoff, the thirty-second server-delay cap, and sink output
and rejection. The llm adapter's loopback tests replay the exact provider error through the real
HTTP and streaming decoder boundary.

Independent review found and corrected a pure-decoder retry eligibility gap in llm. Final review
found no blocking issue in the provider decoder or Loom's caller orchestration.

The development pin is deliberate: this fix is not yet in a released llm tag. No remote main branch
is merged by this qualification.

Repository validation on the implementation commit passed: `task check`, `task plan`, and
`task website`. All 1,252 distinct test names reported by the gate were present in that checkout's
`cargo test --workspace --locked -- --list` inventory (including existing ignored tests). Planning
retains one pre-existing prose-only review warning. The signed source gate passed as well.
