---
format: aep.planning-md/3
id: story:decider-port-and-grounding-check
kind: story
status: draft
title: A decider proposes through a port, and a recommendation counts only when every ground resolves
summary: Decider port, rule adapter, citation resolver port, the refusals of decision-grounds.sh, prior-decision reuse
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:autonomy-grant-table
revision: 2
---
## Outcome

`b10x-loom-decision` declares the decider port and the grounding check.

- `Decider` is given a `DeciderInput`: the request as opened (question, options with their
  consequences, subject artifact at its revision, class, holder), the recorded recommendation when
  there is one (a rule's verdict under `rule_and_decider*`), and the sources the host offers it
  (each a `GroundSource` kind, a `Citation` and the cited text). It answers a `DeciderReply`:
  `Propose` (option, strength, default on silence, grounds, reasons), `Take` (accept the recorded
  recommendation, with reasons) or `Escalate` (reasons and an optional `best` option), or fails
  `Unavailable` or `InvalidReply`. A failure is never a proposal: it is handled as an `Escalate`
  with no `best` (`~/beyond10x/sdc/docs/autonomy-levels.md:257`, "an invalid reply";
  `:456-457`). Each decider names its principal (`decider:<kind>` or `rule:<name>`) and its
  proposer kind (`llm` or `rule`).
- `RuleDecider` adapts a host function from `DeciderInput` to `DeciderReply` as a decider with
  proposer kind `rule`. Loom ships no rule.
- `CitationResolver` is the port that answers, per citation, resolved (with the digest of the
  cited lines for a `path_line`) or refused with a reason. `PathLineResolver` resolves `path_line`
  citations against the case directory, then a root the host names, as
  `~/beyond10x/sdc/tools/decision-grounds.sh:22-30` does: the lines must be in range, a given
  digest must equal the SHA-256 of the lines, and the computed digest is written into the ground.
  It refuses every other form; `story:grounds-resolve-against-held-cases` resolves those.
- `check_grounds(request, reply, resolver, store)` gives the `Recommend` input with
  `check_verdict` and `check_report`. It refuses, listing every reason, as
  `decision-grounds.sh:88-96` does: the subject revision is not current ("supersede the
  request"); no ground; no option proposed (the proposer escalated); an option that is not one of
  the request's; strength `none`; a default on silence that is not strong; a ground that supports
  a letter that is not an option; no ground supporting the chosen option; any citation refused.
  A verified report reads `N grounds, all resolve`; a refused one reads `no recommendation: `
  followed by the reasons joined by `; ` (`decision-grounds.sh:100-101`).
- A `Take` reply proposes nothing of its own: `check_grounds` checks the recorded recommendation's
  grounds again and the subject revision, with the same reasons, and gives a `Recommend` input
  with the recorded option and strength when they still hold. A `Take` on a request with no
  recorded recommendation is refused "no recommendation to take".
- Before the reply is checked, a Decided request in the store with the same `question_key`,
  subject artifact and subject revision whose grounds all still resolve is reused: a strong
  recommendation proposed by `prior_decision` whose first ground cites the prior request
  (`decision-grounds.sh:68-79`). A prior whose grounds no longer resolve is named in the report
  and not reused.

The check takes the proposed strength as given and does not judge whether the cited text supports
the option (`~/beyond10x/sdc/docs/decisions.md:142-143`). Its code goes in its own module of
`crates/loom-decision`; `lib.rs` gains one `mod` line.

## Why

Item 1 of the request: "a recommendation is accepted only when every ground resolves
(`sdc/tools/decision-grounds.sh` is the reference behaviour), otherwise none, with the reason"
(`decisions.md:301-303`). The port follows `ActionSelector`
(`crates/loom-executor/src/selection.rs:95`): the decider is given only what it may choose from
and an answer outside it is refused, never repaired. Placement: a port trait and plain types may
live outside Commission; the citations that need a Canon case resolve in a Canon-linked crate
(`AGENTS.md:436-437`), which is why the resolver is a port here.

## Waits on

`story:autonomy-grant-table`: both change `crates/loom-decision/src/lib.rs`, and the
`DeciderInput` carries the class and holder that story resolves. `story:decision-request-entity`
through it.

## ESS first

`ess/domains/decision.yaml` gains `DeciderInput`, `OfferedSource`, `DeciderReply` (`Propose`,
`Take`, `Escalate`), `DeciderFailure` (`Unavailable`, `InvalidReply`), `CitationVerdict` and
`GroundingRefusal` (one variant per reason above). On that commit `task drift` fails, and
`crates/loom-decision/tests/grounding_check.rs` does not compile.

## Acceptance

Ported from the hand-made requests of `decisions.md:145-153`, as local fixtures:

- A reply with five `path_line` grounds in range is verified with the report
  `5 grounds, all resolve`, and each ground carries the SHA-256 digest of its lines.
- A ground whose given digest differs from the lines, a line past the end of the file, and a path
  that leaves the case directory and the root are each refused naming the citation.
- A reply proposing option `D` of a request with options `A` to `C`, one with no ground for its
  option, and one with strength `none` are each refused, and the report lists every reason, not
  only the first.
- A reply with strength `moderate` and `default_on_silence: true` is refused "a default on silence
  needs a strong recommendation".
- A reply proposing `A` with one ground supporting `A` and one supporting `E` (not an option) is
  refused "a ground supports E, not an option".
- A `Take` reply on a request whose recorded rule recommendation `A` has grounds that resolve
  gives a verified `Recommend` input with option `A`; after one cited file changes, the same
  `Take` is refused naming that citation; a `Take` on a request with no recommendation is refused
  "no recommendation to take".
- A reply whose subject revision is not current is refused with "the subject moved, supersede the
  request".
- An `Escalate` reply gives a refused `Recommend` input with "no option proposed (the proposer
  escalated)".
- A decider that fails `InvalidReply` gives the same as an `Escalate` with no `best`, and no
  recommendation is recorded.
- With a Decided prior on the same question key and subject revision whose grounds resolve, the
  check returns a strong `prior_decision` recommendation citing the prior first, and the
  decider's reply is not used; after one cited file changes, the prior is named in the report and
  not reused.
- A `RuleDecider` over a fixture function answers with proposer kind `rule` and its `rule:<name>`
  principal.
- `crates/loom-decision/Cargo.toml` still names no Canon, executor, `llm` or Mandate crate.
- `task check` exits 0.

## Out of scope

Resolving `record_id` and `artifact_revision` citations (`story:grounds-resolve-against-held-cases`).
An LLM decider (`story:llm-decider`). Judging strength against the criteria of
`~/beyond10x/sdc/ess/domains/decision.yaml:112`. Choosing which sources a host offers.
