# Agent serve qualification — 2026-10-25

This records the standing of `story:agent-serve` (`b10x-loom agent serve`, `agent resume`) at
its implementation, before a tagged release. It does not claim a tagged release or published
documentation.

## What was verified headlessly

Every acceptance test of the story runs offline on the fake `connectors` of `loom-connectors`
and recorded model answers, and passes in the unit's gate:

| Claim | Test |
|---|---|
| A message outside `channel.allow` is no item | `crates/loom-agent/tests/serve.rs`, `a_message_outside_the_allow_list_is_no_item` |
| A send outside the allow list runs no command | `crates/loom-agent/tests/serve.rs`, `a_send_outside_the_allow_list_runs_no_command` |
| A send without a proposal is not admissible | `crates/loom-agent/tests/serve.rs`, `a_send_without_a_proposal_is_not_admissible` |
| A send is held until approved, then performed once with the grant as its proof | `crates/loom-agent/tests/serve.rs`, `a_send_is_held_until_approved`; `crates/loom-cli/tests/agent_serve.rs`, `agent_serve_once_holds_the_send_and_resume_performs_it` |
| The sixth send in the window is refused | `crates/loom-agent/tests/serve.rs`, `a_sixth_send_in_the_window_is_refused` |
| SIGTERM ends the host after the item | `crates/loom-cli/tests/agent_serve.rs`, `sigterm_ends_the_host_after_the_item` |
| `SpecAuthority` grants exactly `policy.capabilities` | `crates/loom-agent/tests/serve.rs`, `the_spec_authority_grants_exactly_the_policy_capabilities` |

## Not verified here

- The conformance suite drives `loom.agent.AdmitSend` on its declared inputs alone; that a served
  host feeds it the item's channel, the recorded proposal, the grant and the counter is held by
  the tests above, not by conformance.
- No real Connectors run: every write in the tests goes to the fake `connectors`, so what a real
  `operations invoke` answers for a channel's post operation is unverified until the manual run.

## Manual run

No manual run against a live channel was made: the implementation session had no channel, no
Connectors connection and no model credential, and the story's tests use none. The record of one
is pending. To make it:

1. Write a `loom.agent/1` spec with a `connectors` channel binding whose `write` is the channel's
   post operation, `writes: {mode: approval, max_per_hour: 5}`, and `runtime.connectors` naming a
   `connectors` with a connection to that channel; `b10x-loom agent check --spec <FILE>` says
   `checked`.
2. `b10x-loom agent serve --spec <FILE> --state <DIR> --once` after a question is posted in an
   allowed channel: one `proposed` line naming the commission; nothing is posted.
3. `b10x-loom approve <COMMISSION> reply.send --state <DIR>`, then `b10x-loom agent resume
   <COMMISSION> --spec <FILE> --state <DIR>`: one post in the channel's thread, the line
   `sent for commission …: applied`, and one line in `<DIR>/sends.jsonl`.
4. Record the spec (credentials elided), the three outputs and the posted message here, dated.
