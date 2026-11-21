---
format: aep.planning-md/3
id: story:decider-port-and-grounding-check
kind: story
status: proposed
title: A decider proposes through a port, and a recommendation counts only when every ground resolves
summary: Decider port answering Result<DeciderReply, DeciderFailure>, rule adapter, citation resolver port, the refusals of decision-grounds.sh, a prior's reply checked with its record resolved from the store
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:autonomy-grant-table
scope:
- confidence: inferred
  path: crates/loom-decision/Cargo.toml
- confidence: inferred
  path: crates/loom-decision/src/decider.rs
- confidence: inferred
  path: crates/loom-decision/src/grounding.rs
- confidence: cited
  path: crates/loom-decision/src/lib.rs
- confidence: cited
  path: crates/loom-decision/tests/grounding_check.rs
- confidence: cited
  path: ess/domains/decision.yaml
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:19Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom-decision` declares the decider port and the grounding check, in two modules,
`decider` and `grounding`.

- `Decider` is a synchronous trait, as `ActionSelector` is. It is given a `DeciderInput`: the
  request as opened (question, options with their consequences, subject artifact at its revision,
  class, holder), the recorded recommendation when there is one (a rule's verdict under
  `rule_and_decider*`), and the sources the host offers it (each a `GroundSource` kind, a
  `Citation` and the cited text). It answers `Result<DeciderReply, DeciderFailure>`: a
  `DeciderReply` is `Propose` (option, strength, default on silence, grounds, reasons), `Take`
  (accept the recorded recommendation, with reasons) or `Escalate` (reasons and an optional `best`
  option); a `DeciderFailure` is `Unavailable` or `InvalidReply`, each with what went wrong. A
  failure is never a proposal: it is handled as an `Escalate` with no `best`
  (`~/beyond10x/sdc/docs/autonomy-levels.md:257`, "an invalid reply"; `:456-457`). Each decider
  names its principal (`decider:<kind>` or `rule:<name>`) and its proposer kind (`llm` or `rule`).
- `RuleDecider` adapts a host function from `DeciderInput` to `DeciderReply` as a decider with
  proposer kind `rule`. Loom ships no rule.
- `CitationResolver` is the port that answers, per citation, resolved (with the digest of the
  cited lines for a `path_line`) or refused with a reason. `PathLineResolver::new(case_dir, root)`
  resolves `path_line` citations against the case directory, then the root, as
  `~/beyond10x/sdc/tools/decision-grounds.sh:22-30` does: the lines must be in range, a given
  digest must equal the SHA-256 of the lines, and the computed digest is written into the ground.
  One rule is Loom's own, not the script's (`:24` joins the path to each base unchecked): the path
  is normalised lexically against the base it is joined to, and a path that is absolute or whose
  `..` leaves that base is refused naming the citation, even where it would land inside the other
  base. `PathLineResolver` refuses every other form; `story:grounds-resolve-against-held-cases`
  resolves those, and its `HeldCaseGrounds` delegates `path_line` to a `PathLineResolver`.
- `check_grounds(request, answer, resolver, store)`, `answer` being the decider's
  `Result<DeciderReply, DeciderFailure>`, gives the `Recommend` input with `check_verdict` and
  `check_report`. The subject check asks the resolver for the synthetic `artifact_revision`
  citation of the request's subject artifact at its subject revision, as `decision-grounds.sh:86`
  does. For a `Propose` it refuses, listing every reason, as `decision-grounds.sh:88-96` does: the
  subject revision is not current ("supersede the request"); no ground; an option that is not one
  of the request's; strength `none`; a default on silence that is not strong; a ground that
  supports a letter that is not an option; no ground supporting the chosen option; any citation
  refused. Strength is tested by membership (`none`, `strong`), never by order: Loom's `Strength`
  carries no order: the Loom copy drops `ordered: true`, which `ess/31` does not have
  (`story:decision-request-entity`).
- An `Escalate` short-circuits: the report is `no recommendation: no option proposed (the
  proposer escalated)` and nothing else, where the script would add "no ground cited", "strength
  none" and "no ground supports null" (`decision-grounds.sh:89-95`). A failure gives
  `no recommendation: decider failed: <Unavailable or InvalidReply>: <what went wrong>`. Both give
  a refused `Recommend` input with no option, strength `none`, no default on silence and no
  grounds; the `withheld` outcome clears those anyway (`~/beyond10x/sdc/ess/domains/decision.yaml:496-515` at `00e3557`).
- A verified report reads `N grounds, all resolve`; a refused one reads `no recommendation: `
  followed by the reasons joined by `; ` (`decision-grounds.sh:100-101`).
- A `Take` reply proposes nothing of its own: `check_grounds` checks the recorded recommendation's
  grounds again and the subject revision, with the same reasons, and gives a `Recommend` input
  with the recorded option and strength when they still hold. A `Take` on a request with no
  recorded recommendation is refused "no recommendation to take"; a `ModelDecider` never hands one
  over (`story:llm-decider` makes it an `InvalidReply`), a `RuleDecider` can.
- Reusing a prior decision happens before any decider runs: `story:decision-routing-by-holder`
  builds a `Propose` from the prior (its decided option, strength `strong`, proposer
  `prior_decision`, a first ground citing the prior request, then the prior's grounds) and passes
  it to `check_grounds` like any reply. The first ground's citation, a `record_id` in store
  `sdc_decisions`, is resolved by `check_grounds` against `store` (a Decided request with that id,
  found by listing the store), never through the resolver, so it holds under `PathLineResolver`
  too. A verified report of such a reply reads `N grounds, all resolve; reuses <prior id>`, N
  counting the prior's own ground (`decision-grounds.sh:100`).

The check takes the proposed strength as given and does not judge whether the cited text supports
the option (`~/beyond10x/sdc/docs/decisions.md:142-143`). Its code goes in the two modules of
`crates/loom-decision`; `lib.rs` gains their `mod` lines. The digest uses `sha2`, which
`story:autonomy-grant-table` adds to the crate's manifest.

## Why

Item 1 of the request: "a recommendation is accepted only when every ground resolves
(`sdc/tools/decision-grounds.sh` is the reference behaviour), otherwise none, with the reason"
(`decisions.md:301-303`). The port follows `ActionSelector`
(`crates/loom-executor/src/selection.rs:106`): the decider is given only what it may choose from
and an answer outside it is refused, never repaired. Placement: a port trait and plain types may
live outside Commission; the citations that need a Canon case resolve in a Canon-linked crate
(`AGENTS.md:433-434`), which is why the resolver is a port here. The SDC `Recommend` input takes
`recommended_option` as `Optional<OptionLetter>` (`decision.yaml:465`), and a refused check of an
`Escalate` or a failure has no option; Loom's copy declares it optional too (below).

## Waits on

`story:autonomy-grant-table`: both change `crates/loom-decision/src/lib.rs`, and the
`DeciderInput` carries the class and holder that story resolves. `story:decision-request-entity`
through it.

## ESS first

`ess/domains/decision.yaml` gains `DeciderInput`, `OfferedSource`, `DeciderReply` (`Propose`,
`Take`, `Escalate`), `DeciderFailure` (`Unavailable`, `InvalidReply`), `CitationVerdict` and
`GroundingRefusal` (one variant per reason above), and `Recommend`'s `recommended_option` is
`Optional<OptionLetter>` in Loom's copy, as in the contract. On that commit `task drift` fails, and
`crates/loom-decision/tests/grounding_check.rs` does not compile.

## Acceptance

Ported from the hand-made requests of `decisions.md:145-153`, as local fixtures, over a fixture
resolver that delegates `path_line` to `PathLineResolver` and answers the subject's
`artifact_revision` citation from a table:

- A reply with five `path_line` grounds in range is verified with the report
  `5 grounds, all resolve`, and each ground carries the SHA-256 digest of its lines.
- A ground whose given digest differs from the lines, a line past the end of the file, a path
  `../outside.md` that leaves the case directory, and an absolute path are each refused naming the
  citation.
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
- A reply on a request whose subject revision the fixture table no longer holds is refused with
  "the subject moved, supersede the request".
- An `Escalate` reply gives a refused `Recommend` input with no option and the report
  `no recommendation: no option proposed (the proposer escalated)`, with no other reason.
- An answer `Err(InvalidReply)` gives a refused `Recommend` input with no option and a report
  beginning `no recommendation: decider failed: InvalidReply`.
- A reply built from a Decided prior in the in-memory store, whose grounds resolve, is verified
  with the report `3 grounds, all resolve; reuses DR-C-roadmap.approve-1` (two path grounds and the
  prior's), through a resolver that refuses every `record_id`; after one of the prior's cited
  files changes, it is refused naming that citation.
- A `RuleDecider` over a fixture function answers with proposer kind `rule` and its `rule:<name>`
  principal.
- `crates/loom-decision/Cargo.toml` still names no Canon, executor, `llm` or Mandate crate.
- `task check` exits 0.

## Out of scope

Resolving `record_id` citations of other stores and `artifact_revision` citations against a held
case (`story:grounds-resolve-against-held-cases`). Choosing and building a prior's reply
(`story:decision-routing-by-holder`). An LLM decider (`story:llm-decider`). Judging strength
against the criteria of `~/beyond10x/sdc/ess/domains/decision.yaml:112`. Choosing which sources a
host offers.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (crate `b10x-loom-decision`), the grounding check and decider port in new modules — cited (Outcome: "`b10x-loom-decision` declares the decider port and the grounding check"; "Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line")
- **Files:** `crates/loom-decision/src/lib.rs` (one `mod` line per module) — cited
- **Files:** `crates/loom-decision/tests/grounding_check.rs` (the red test) — cited (ESS first)
- **Files:** `ess/domains/decision.yaml` (gains `DeciderInput`, `OfferedSource`, `DeciderReply`, `DeciderFailure`, `CitationVerdict`, `GroundingRefusal`) — cited (ESS first)
- **Files:** the new module file(s), e.g. `crates/loom-decision/src/decider.rs` and `grounding.rs` — inferred, the story gives no names
- **Files:** `crates/loom-decision/Cargo.toml` — inferred, gains `sha2` for the SHA-256 digest. The loom-governor, loom-executor and loom-a2a crates pin `sha2 = "0.11.0"` (`crates/loom-governor/Cargo.toml:21`)
- **Symbols:** `Decider`, `DeciderInput`, `DeciderReply` (`Propose`/`Take`/`Escalate`), `DeciderFailure` (`Unavailable`/`InvalidReply`), `RuleDecider`, `CitationResolver`, `PathLineResolver`, `check_grounds`, `GroundingRefusal`, `CitationVerdict`, `OfferedSource` — cited, none exist in the tree (`git grep` finds nothing)
- **Symbols it builds on:** `DecisionStore`, `Recommend` input, `Holder`, `DecisionClassId`, `DecisionRequest` — inferred. They arrive with `story:decision-request-entity` and `story:autonomy-grant-table`, neither delivered
- **Reference behaviour:** `~/beyond10x/sdc/tools/decision-grounds.sh:22-30, 68-80, 88-96, 100-101` — cited, read, and those line ranges are right
- **Also likely:** `crates/loom-conformance/src/decision.rs` — inferred, not touched. The entity story owns it, and this story adds no command or scenario
- **Documents:** none. `ess/domains/decision.yaml` is the only non-code file, and its generated reference page is excluded as a house-rule file.
- **Confidence:** medium. The surface and the three named files are cited. The module file names and the `Cargo.toml` edit are inferred. Every type the check consumes (`DecisionStore`, `Recommend`) is declared by an undelivered story, so signatures could shift.
- **Would collide with**, from `aep plan artifact graph --format json`:
  - `ess/domains/decision.yaml`:
    - `story:decision-request-entity` creates it. Ordered: `depends_on` from `story:autonomy-grant-table` to the entity story to this story.
    - `story:decision-routing-by-holder` adds `DecisionStop` and `RouteOutcome`. Ordered: `depends_on` this story, direct.
    - `story:escalate-when-conditions` and `story:non-independent-review-becomes-decision-request` also edit it. Ordered: both reach this story through routing.
    - `story:decided-record-written-then-recorded` adds `CompletionOutcome`. Ordered: `depends_on` `story:grounds-resolve-against-held-cases`, which depends on this story.
    - `story:silence-and-expiry-at-a-host-tick` adds `TickReport`. Ordered: it depends on the decided-record story, so through grounds-resolve.
  - `crates/loom-decision/src/lib.rs` (a `mod` line each): the same set as above, ordered the same way.
    - The entity story creates the crate.
    - `story:autonomy-grant-table` adds a `mod`. Ordered: this story's direct `depends_on`.
    - `story:decision-request-file-store` does not touch it (its body names `crates/loom-governor` only) and is unordered against this story, which is harmless.
  - `crates/loom-decision/Cargo.toml`: `story:decision-request-entity` creates it. Ordered. No other sibling edits it.
  - Consumers that do not share the file: `story:llm-decider` (`crates/loom-executor`) and `story:grounds-resolve-against-held-cases` (`crates/loom-governor`) implement the port. Both `depends_on` this story, so they conflict with its signatures but not its files.
  - No sibling collides without an ordering `depends_on` path.
- **Safety fact:** the crate stays free of Canon, executor, `llm` and Mandate crates. The manifest test the entity story adds enforces it, and this story adds only `sha2` and the generated `loom` crate — the entity story's Acceptance ("`Cargo.toml` names no Commission, Canon, executor, `llm` or Mandate crate"), unproven.
