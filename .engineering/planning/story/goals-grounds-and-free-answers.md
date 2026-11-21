---
format: aep.planning-md/3
id: story:goals-grounds-and-free-answers
kind: story
status: draft
title: A ground can cite one goals entry, checked by rule D93 at the request's opened_at through a host port, and a question request takes a free answer, behind the contract members story 1 copies at 00e3557
summary: 'Story 1''s copy of the contract already declares the goals ground and the free answer. This story adds Loom''s declarations: GoalsRefusalCause, the GoalsEntry refusal carrying every cause, the revision_moved note kept in grounding_report, GoalsEntryState, the host port GoalsEntries, the answer on DeciderReply::Propose, and the answer on the ServedTree command Decide. It adds the behaviour too. HeldCaseGrounds::with_goals_entries checks a goals ground by rule D93, and the slice and the tree pass the port. Decode, inbox printing and --answer, the tree and daemon Decide, the question record and the tick carry the free answer'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:question-and-outcome-decisions-hold-runs
- depends_on: story:grounds-resolve-against-held-cases
- depends_on: story:llm-decider
- depends_on: story:decision-request-file-store
- depends_on: story:decision-inbox-commands
- depends_on: story:silence-and-expiry-at-a-host-tick
- depends_on: story:decisions-resume-the-held-run
- depends_on: story:daemon-continues-a-decided-request
- depends_on: story:decider-port-and-grounding-check
- depends_on: story:decision-request-entity
- depends_on: story:non-independent-review-becomes-decision-request
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/decisions_free_answer.rs
- confidence: cited
  path: crates/loom-decision
- confidence: inferred
  path: crates/loom-decision/src/grounding.rs
- confidence: cited
  path: crates/loom-decision/tests/free_answer.rs
- confidence: cited
  path: crates/loom-decision/tests/goals_grounds.rs
- confidence: cited
  path: crates/loom-executor
- confidence: inferred
  path: crates/loom-executor/src/model_decider.rs
- confidence: cited
  path: crates/loom-executor/tests/llm_decider_answer.rs
- confidence: cited
  path: crates/loom-governor
- confidence: inferred
  path: crates/loom-governor/src/held_case_grounds.rs
- confidence: cited
  path: crates/loom-governor/tests/held_case_goals.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: crates/loom-sdk/src/decisions.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: inferred
  path: website/docs/guides/decisions.md
revision: 8
---
## Outcome

Loom carries what SDC's decision contract gained after the epic's first read. The contract is
final at commit `00e3557` (`~/beyond10x/sdc/ess/domains/decision.yaml` at `00e3557`, read with
`git -C ~/beyond10x/sdc show 00e3557:ess/domains/decision.yaml`). `story:decision-request-entity`
already copies that contract whole into Loom's `ess/domains/decision.yaml`, guards included, and
its crate answers every guard. So the contract's members below are story 1's declarations. This
story adds two things:

- Loom's own declarations (§ ESS first);
- the behaviour behind the contract's members, through the surfaces stories 1 to 14, 16, 21 and
  22 built.

The two features are a ground that cites one entry of the goals document (rule D93), and a free
answer in place of an option on a question request.

**Goals grounds, the rule:**

- The contract's members (story 1's copy): `GroundSource` `goals` (`decision.yaml:116`), and
  `ArtifactRevisionCitation`'s optional `entry` and `digest` (`:146`, `:147`). A goals ground is an
  `artifact_revision` citation of artifact `goals` with an entry id and the entry's digest. It is
  displayed `goals@<revision>#<entry id>` (`:118`, `:157`).
- Rule D93 (`~/beyond10x/sdc/docs/decisions.md:141` at `00e3557`, and the message of commit
  `00e3557`, "Plan decision D93") says the ground rests on the entry, not on the document. It
  holds when all four hold (`:141-143`):
  - the entry exists in the current goals layers;
  - its digest is the cited one;
  - it is inside its validity window at the instant;
  - no applying lower-layer entry overrides it.
- Otherwise the ground is refused naming each cause (`docs/decisions.md:144-145`), so the refusal
  carries every cause that applies, in this order:
  - `no_entry`: the entry is not in the current layers;
  - `digest_changed`: its digest differs from the cited one;
  - `not_yet_valid`: the instant is before its validity window;
  - `expired`: the instant is at or after the window's end;
  - `overridden`: an applying lower-layer entry overrides it;
  - `invalid_goals`: the goals document does not validate.

  `no_entry` and `invalid_goals` each come alone, because with either no entry can be read. The
  other four combine: `digest_changed`, `overridden`, and at most one of `not_yet_valid` and
  `expired`.
- A cited revision that is no longer the current one is not a refusal
  (`docs/decisions.md:145-147`). The check gives the note `revision_moved`, with the cited and the
  current revision. The current revision is the held case's revision of artifact `goals`, read as
  an `artifact_revision` citation is resolved (`story:grounds-resolve-against-held-cases`), just
  as SDC passes `--current` from `case.json`. A held case that declares no `goals` artifact has no
  current revision, so the revision is not compared and no note is given.
- A goals citation with no `digest` is given the entry's current digest by the check, as the
  contract says (`:147`), so a later check sees a change. The digest is written into the ground
  the `Recommend` input carries.

**The instant.** The check answers at the request's `opened_at` (`decision.yaml:241`). That is the
contract's default: the instant is `--at`, else the request's `opened_at`
(`docs/decisions.md:147-148`). Loom has no `--at` for the check. `check_grounds` already takes the
request (`story:decider-port-and-grounding-check`), so it passes `request.opened_at` on. The
re-checks below pass the Decided request's `opened_at`. No clock is read, and the held-case
resolver still reads none (`story:grounds-resolve-against-held-cases`, § Acceptance).

**How the port reaches the check:**

- `GoalsEntries` is a host port in `crates/loom-decision`. Given an entry id and an instant, it
  answers `GoalsEntryState`:
  - whether the goals document validates;
  - and, when the entry is present, its digest, its `valid_from`, its optional `valid_until`,
    and the id of the applying lower-layer entry that overrides it at that instant, if any.

  A pure function of the crate, `goals_verdict(citation, state, current revision, instant)`, gives
  the `CitationVerdict`: the causes, or verified with the note.
- `CitationResolver::resolve` (`story:decider-port-and-grounding-check`) gains the instant, and
  `check_grounds` fills it with `request.opened_at`. `PathLineResolver` ignores it, as
  `HeldCaseGrounds` does for every citation that is not a goals entry.
- `HeldCaseGrounds::with_goals_entries(port)` adds the port, as `with_authority_records` adds the
  `AuthorityRecords` port (`story:grounds-resolve-against-held-cases`). For a citation of artifact
  `goals` with an `entry`, the resolver:
  - asks the port at the instant;
  - reads the held case's current `goals` revision;
  - calls `goals_verdict`.
- Without the port, a goals ground is refused `invalid_goals` with the reason `no goals port`.
- The slice takes an optional `Arc<dyn GoalsEntries>` from its host in a new field `goals` of
  `RunOptions` (`crates/loom-intake-slice/src/run.rs:208`). It passes the port to
  `with_goals_entries` where it builds its `HeldCaseGrounds` with the decision flags
  (`story:approvals-supplied-to-canon-evaluation`).
- The tree takes the same optional port from its host. `crates/loom-tree` passes it to
  `with_goals_entries` in the `HeldCaseGrounds` build that `tree run` and the daemon share
  (`story:tree-waits-open-decision-requests`, `story:daemon-continues-a-decided-request`).
- On the command line, `run`, `resume`, `tree run` and `tree serve` pass no port, because Loom
  ships no implementation of it. So through the binary a goals ground is refused
  `invalid_goals`. Session `sdc` accepts this until a file-backed provider lands
  (`.engineering/drafts/e3-sdc-answers-11.md`, (c); `story:goals-entries-from-a-resolved-document`).
  The `decisions` commands run no grounding check.

**Where `revision_moved` is kept.** It is kept in the contract's `grounding_report`
(`decision.yaml:250`), which `Recommend` sets from `check_report` on a verified check (`:526`).
The verified report reads `N grounds, all resolve` (`story:decider-port-and-grounding-check`),
followed by one `; revision_moved goals@<cited>#<entry id> (current <revision>)` per noted ground.
The contract has the member, so Loom declares none for it. On a refused check,
`no_recommendation_reason` lists the causes, and the note is not a reason.

**When a goals ground is checked again:**

- Not at `Decide`, because the contract's `Decide` guards read no ground (`decision.yaml:556-603`).
- Not at `RecordProduced` either.
- Where Loom checks grounds again, a goals ground is checked again with the same rule:
  - `check_grounds` on a decider's `Take` reply and on the reuse of a prior decision
    (`story:decider-port-and-grounding-check`);
  - `HeldCaseGrounds` where the slice answers from a Decided request's record
    (`story:approval-decisions-resume-held-runs`);
  - where the slice's `AuthorityRecords` filters a record
    (`story:approvals-supplied-to-canon-evaluation`);
  - where `tree run` continues past a wait (`story:tree-waits-open-decision-requests`).
- At those places, a moved digest (`digest_changed`) or any other cause means the ground no
  longer resolves:
  - the slice and the tree supersede the request and route the stop anew;
  - the record is withheld from Canon;
  - the `Take` is withheld;
  - the prior is not reused.
- A moved revision alone (`revision_moved`) still resolves.

**Free answers.** The contract's members and guards are story 1's. This story's tests hold the
free-answer guards (`crates/loom-decision/tests/free_answer.rs`).

- `Ground.supports` is optional (`:166`). When it is absent, the ground supports the recommended
  free answer. The grounding check is verified when at least one ground supports the recommended
  option or free answer (`:204`); `check_grounds` gains that reading.
- `recommended_answer` (`:245`, `Recommend` input `:466`): a verified recommendation names exactly
  one of an option and an answer (`OptionOrAnswer`, `:335`, guard `:482-485`). Only a question
  request is recommended an answer (`AnswerNotAllowed`, `:340`, guard `:490-495`).
- `Decide.answer`, with `option` optional (`:549-550`): the same two guards (`:560-563`,
  `:568-573`), and `decided_answer` (`:255`, set at `:609`).
- `TakeRecommendation` sets `decided_answer` from `recommended_answer` (`:678`), so silence and
  `take` take a recommended answer as they take an option.
- The second invariant (`:276`): no `recommended_answer` beside a `no_recommendation_reason`.
- A free answer exists only on a question request (`when_subject: unblocks.kind != question`,
  `:493`, `:571`). Session `sdc` earlier named the classes scope-changing answer and finding
  (`.engineering/drafts/e3-sdc-answers-6.md`). The final contract keys the answer on the question
  kind, Loom follows it, and session `sdc` accepted that (`.engineering/drafts/e3-sdc-answers-11.md`,
  (d)). This supersedes the epic's earlier wording. Approvals and outcomes stay option-only.

**The surfaces this extends:**

- **Decode** (`story:decision-request-file-store`, `story:llm-decider`):
  - A stored request with `recommended_answer`, `decided_answer` and goals grounds decodes, and
    one without them decodes as before.
  - `DeciderReply::Propose` (`story:decider-port-and-grounding-check`) carries an optional answer
    in place of the option.
  - The LLM decider's forced tool call offers `answer` only for a question request. A reply with
    an answer for any other request is refused `InvalidReply`, never repaired.
- **Inbox printing** (`story:decision-inbox-commands`):
  - `decisions list` and `show` print `recommended_answer`, `decided_answer` and
    `grounding_report`, and print each goals ground as `goals@<revision>#<entry id>` with its
    digest.
  - `decisions decide ID --answer TEXT --as P` decides a question request with a free answer, in
    place of `OPTION`. `OPTION` becomes optional in the parser, so the domain answers both cases:
    giving both, or neither, exits 10 naming `OptionOrAnswer`.
  - `--answer` on an approval or outcome exits 10 naming `AnswerNotAllowed`.
- **Trees** (`story:tree-waits-open-decision-requests`, `story:daemon-continues-a-decided-request`):
  - `decisions decide --answer` with `--tree` applies the answer, as with `--store`.
  - On a served tree the `ServedTree` command `Decide` (`ess/domains/tree.yaml`) gains an optional
    `answer`, with its option optional, and the daemon applies it.
- **The question's record** (`story:question-and-outcome-decisions-hold-runs`,
  `story:decisions-resume-the-held-run`):
  - A question decided with an answer prints the answer beside the `decisions record` command,
    so the host writes it to its file.
  - The host then runs `decisions record`, which resumes the run as story 21 says.
- **The record handed to Canon** is unchanged:
  - A free answer exists only on question requests. Their record is an `artifact_revision`, not
    a Canon record, so `GrantRecordWriter` and `ExplicitDecisionWriter` are untouched.
  - A goals ground on an approval or outcome reaches Canon with
    `story:decided-records-carry-grounds-to-canon`, which carries the grounds a request holds as
    they are.
- **The tick** (`story:silence-and-expiry-at-a-host-tick`): a strong recommended answer with a
  default on silence is taken by the tick, and the request is Decided by `silence` with
  `decided_answer`.

**Files:**

- `ess/domains/decision.yaml` (Loom's declarations only), `ess/domains/tree.yaml` (`Decide`'s
  answer), and the regenerated `generated/rust/loom`.
- `crates/loom-decision`: `GoalsEntries`, `GoalsEntryState`, `goals_verdict`, the instant on
  `CitationResolver`, `check_grounds`' reading of `supports`, and the answer on `DeciderReply`.
- `crates/loom-governor`: `HeldCaseGrounds::with_goals_entries` and the dispatch of an `entry`
  citation.
- `crates/loom-intake-slice`: the `goals` field of `RunOptions` and its use in the slice's
  `HeldCaseGrounds` build.
- `crates/loom-tree`:
  - the optional port in the `HeldCaseGrounds` build that `tree run` and the daemon share;
  - the daemon's `Decide` with an answer;
  - the `decide` and `take` types, which change because `Decide.option` is now optional,
    `Decide.answer` is new and `Ground.supports` is optional.
- `crates/loom-executor`: the LLM decider's tool schema and decode.
- `crates/loom-cli/src/decisions.rs`: `--answer`, the optional `OPTION`, and the printing.
- Tests:
  - `crates/loom-decision/tests/goals_grounds.rs`;
  - `crates/loom-governor/tests/held_case_goals.rs`;
  - `crates/loom-decision/tests/free_answer.rs`;
  - `crates/loom-executor/tests/llm_decider_answer.rs`;
  - `crates/loom-cli/tests/decisions_free_answer.rs`.

## Why

Session `sdc` reported the contract final at `00e3557` with these additions
(`.engineering/drafts/e3-sdc-answers-9.md`). It said earlier that they "fit a later story of your
epic" and that stories 1 to 14 need not wait on it (`.engineering/drafts/e3-sdc-answers-6.md`).
The operator's two trial-2 answers were "neither option as asked" (`decision.yaml:546`, the
`Decide` summary). The requester's goals document is a ground a decider cites
(`decision.yaml:118`). SDC will drive Loom through the binary, so the port's provider is a later
story of its own (`.engineering/drafts/e3-sdc-answers-11.md`, (c)).

## Waits on

- `story:decision-request-entity`: the contract's members and guards, which this story gives
  behaviour to and does not declare again.
- `story:decider-port-and-grounding-check`: `check_grounds`, `CitationResolver`, `CitationVerdict`
  and `DeciderReply`, which this story extends.
- `story:question-and-outcome-decisions-hold-runs`: the question request, `decisions record` and
  the writers. Stories 1 to 13 come through it.
- `story:grounds-resolve-against-held-cases`: `HeldCaseGrounds`, which gains `with_goals_entries`.
- `story:llm-decider`: the tool schema and the reply decode.
- `story:decision-request-file-store`: the stored request's decode.
- `story:decision-inbox-commands`: `list`, `show` and `decide`, which gain the answer.
- `story:silence-and-expiry-at-a-host-tick`: the tick, which takes a recommended answer.
- `story:decisions-resume-the-held-run`: both change `decide` in `crates/loom-cli/src/decisions.rs`,
  and the question's resume after `record` is that story's.
- `story:daemon-continues-a-decided-request`: the `ServedTree` `Decide` and the daemon code that
  this story's type changes force an edit in. Story 16 (`story:tree-waits-open-decision-requests`,
  `decisions … --tree` and the tree's `HeldCaseGrounds` build) comes first through it. So this
  story lands after both, and their `decisions.rs` and `crates/loom-tree` edits are in place.
- `story:non-independent-review-becomes-decision-request`: it changes the same files
  (`ess/domains/decision.yaml`, the decider-port module, `ModelDecider` and `crates/loom-tree`), and
  it has no typed scope, so `aep plan artifact waves` cannot keep the two apart by path. A direct
  edge orders this story after it. Neither reads the other's members, so the order is arbitrary,
  and this story is the one of lower priority.

## ESS first

`story:decision-request-entity`'s copy of the contract is not changed: it already holds every
contract member and guard named above. Loom's own declarations in `ess/domains/decision.yaml`:

- `GoalsRefusalCause` (`no_entry`, `digest_changed`, `not_yet_valid`, `expired`, `overridden`,
  `invalid_goals`);
- the `GroundingRefusal` variant `GoalsEntry`: the citation and a list of `GoalsRefusalCause`;
- `CitationNote` `revision_moved` (cited revision, current revision), optional on
  `CitationVerdict`;
- `GoalsEntryState`: `document_valid`, and an optional entry holding `digest`, `valid_from`,
  optional `valid_until` and optional `overridden_by`;
- the host port `GoalsEntries` (entry id, instant → `GoalsEntryState`). It is declared here by the
  epic's rule, because the digest it answers becomes part of the `Recommend` input;
- the optional answer of `DeciderReply::Propose`.

`ess/domains/tree.yaml`: the `ServedTree` command `Decide` gains an optional `answer`, and its
option becomes optional. On that commit `task drift` fails for these declarations, and the five
test files above do not compile.

## Acceptance

In `crates/loom-decision/tests/goals_grounds.rs`, with no model call. `goals_verdict` and
`check_grounds` run over a fixture resolver that answers goals citations from a fixture
`GoalsEntryState`: entry `product/p1`, digest `sha256:aa`, valid from `2026-11-01T00:00:00Z` to
`2026-12-01T00:00:00Z`, document valid. The current revision is `g3`, and the request is opened
at `2026-11-21T12:00:00Z`.

- A ground citing `goals@g3#product/p1` with digest `sha256:aa` is verified with no note.
- The same ground with current revision `g4` is verified, and the `Recommend` input's
  `check_report` reads `1 grounds, all resolve; revision_moved goals@g3#product/p1 (current g4)`.
- A state with no entry is refused with the causes exactly `[no_entry]`.
- A state whose digest is `sha256:bb` is refused `[digest_changed]`.
- The request opened at `2026-10-31T23:59:59Z` is refused `[not_yet_valid]`.
- The request opened at `2026-12-01T00:00:00Z` is refused `[expired]`.
- A state naming `overridden_by: request/r1` is refused `[overridden]`.
- A state with digest `sha256:bb`, `overridden_by: request/r1`, and the request opened at
  `2026-12-01T00:00:01Z` is refused with the causes exactly
  `[digest_changed, expired, overridden]`.
- A state whose document does not validate is refused `[invalid_goals]`.
- A goals citation with no digest is verified, and the ground in the `Recommend` input carries the
  digest `sha256:aa`.
- A recommendation whose one ground is refused `digest_changed` is withheld:
  `no_recommendation_reason` names `goals@g3#product/p1` and `digest_changed`.

In `crates/loom-governor/tests/held_case_goals.rs`, over a held case declaring artifact `goals`
at revision `g3`, a counting `EvaluationTime`, and a recording fixture `GoalsEntries` port:

- `HeldCaseGrounds::new(…).with_goals_entries(port)` resolves `goals@g3#product/p1` through the
  port. The port records one call, for `product/p1` at the request's `opened_at`, and the
  `EvaluationTime` count is unchanged by the resolution.
- After the test raises the case's `goals` artifact to `g4` through
  `CanonGovernor::update_revision`, the same citation is verified with the note `revision_moved`
  naming `g3` and `g4`.
- `HeldCaseGrounds::new(…)` without the port refuses the citation `invalid_goals` naming
  `no goals port`.
- A held case that declares no `goals` artifact verifies the citation with no note.

In `crates/loom-decision/tests/free_answer.rs`, over the in-memory store:

- `Recommend` on a question request, with `recommended_answer` "ship both", strength `strong` and
  one ground with no `supports`, leaves the request with that `recommended_answer`.
- `Recommend` with both `recommended_option` and `recommended_answer` is refused `OptionOrAnswer`,
  and the request file is byte-identical.
- `Recommend` with `recommended_answer` on an approval request is refused `AnswerNotAllowed`.
- `Decide` on a question request with `answer` "neither" and no option leaves it Decided with
  `decided_answer` "neither" and no `decided_option`.
- `Decide` with neither option nor answer is refused `OptionOrAnswer`.
- `Decide` with an answer on an outcome request is refused `AnswerNotAllowed`.
- `TakeRecommendation` by `silence` on a question request with a strong recommended answer and a
  default on silence leaves `decided_answer` equal to the recommended answer.
- Random sequences of the commands over a fixed request never leave `recommended_answer` beside a
  `no_recommendation_reason`.
- A stored request written before this story, with no answer members, decodes, and its other
  members are unchanged.

In `crates/loom-executor/tests/llm_decider_answer.rs`, over recorded model responses:

- A recorded reply proposing the answer "neither" for a question request gives
  `DeciderReply::Propose` with that answer and no option.
- The same reply for an approval request is `DeciderFailure::InvalidReply`.

In `crates/loom-cli/tests/decisions_free_answer.rs`, over a fixture store directory whose question
request `DR-q` carries `recommended_answer` "ship both" and whose `DR-g` carries a goals ground
and the `grounding_report` from the `revision_moved` bullet above:

- `decisions list --principal timo --store DIR` prints `recommended_answer` "ship both" on
  `DR-q`'s entry.
- `decisions show DR-q --store DIR` prints `recommended_answer` "ship both".
- `decisions decide DR-q --answer "neither" --as timo --store DIR` exits 0, and
  `decisions show DR-q` prints `decided_answer` "neither".
- That `decide` prints the answer "neither" beside the `decisions record` command.
- `decisions decide DR-q --as timo --store DIR`, with neither `OPTION` nor `--answer`, exits 10
  naming `OptionOrAnswer`, and the request file is byte-identical.
- `decisions decide DR-q A --answer "x" --as timo --store DIR` exits 10 naming `OptionOrAnswer`.
- `decisions decide DR-o --answer "x" --as timo --store DIR` on an outcome request exits 10
  naming `AnswerNotAllowed`.
- `decisions show DR-g --store DIR` prints `goals@g3#product/p1`, its digest `sha256:aa`, and
  the `grounding_report` holding `revision_moved goals@g3#product/p1 (current g4)`.
- `decisions tick --at E --store DIR`, over a question request with a strong recommended answer
  and a default on silence due at `E`, decides it by `silence`, and `decisions show` prints the
  `decided_answer`.
- On the three-level fixture tree of `story:tree-run-command` with a question request,
  `decisions decide DR-q --answer "neither" --as timo --tree STORE` on a store no daemon serves
  leaves `decided_answer` "neither".
- With `b10x-loom tree serve STORE` running, the same command exits 0 through the daemon's
  `Decide`, and the request is Decided with `decided_answer` "neither".
- `task check` exits 0.

## Out of scope

- Reading the goals document and computing an entry's digest. That is the host's, through
  `GoalsEntries`; the file-backed provider is `story:goals-entries-from-a-resolved-document`.
- A free answer on an approval or an outcome: the contract refuses it.
- Writing a question's answer into its file (the host writes it, then runs `decisions record`).
- Carrying grounds to Canon (`story:decided-records-carry-grounds-to-canon`).
- An instant other than the request's `opened_at` for the check (SDC's `--at`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (guards, goals check, `GoalsEntries` port) with `ess/domains/decision.yaml` — cited, story § Files
- **Files:** `ess/domains/decision.yaml` — cited; not at HEAD, created by `story:decision-request-entity`
- **Files:** `generated/rust/loom` (regenerated) — cited; exists at HEAD
- **Files:** `crates/loom-decision` — cited; not at HEAD, created by `story:decision-request-entity`
- **Files:** `crates/loom-decision/src/` module for the goals check, e.g. `grounding.rs` — inferred; `story:decider-port-and-grounding-check` plans the check in new modules, no name given
- **Files:** `crates/loom-governor` — cited, the held-case resolver's `entry` dispatch; exists at HEAD
- **Files:** `crates/loom-governor/src/held_case_grounds.rs` — inferred; not at HEAD, planned by `story:grounds-resolve-against-held-cases` (name inferred there too)
- **Files:** `crates/loom-executor` — cited, LLM decider tool schema and decode; exists at HEAD
- **Files:** `crates/loom-executor/src/model_decider.rs` — inferred; not at HEAD, planned by `story:llm-decider`
- **Files:** `crates/loom-cli/src/decisions.rs` — cited; not at HEAD, created by `story:decision-inbox-commands`
- **Files (tests):** `crates/loom-decision/tests/goals_grounds.rs`, `crates/loom-decision/tests/free_answer.rs`, `crates/loom-executor/tests/llm_decider_answer.rs`, `crates/loom-cli/tests/decisions_free_answer.rs` — cited; all new, none at HEAD
- **Symbols:** `GoalsEntries`, `GoalsRefusalCause`, `GroundingRefusal::GoalsEntry`, `CitationVerdict` note `revision_moved`, `OptionOrAnswer`, `AnswerNotAllowed`, `DeciderReply::Propose` — cited; none in the tree
- **Also likely:** `crates/loom-sdk/src/decisions.rs` (`loom_sdk::decisions` carries the `decide` input) — inferred; not at HEAD, created by `story:decision-inbox-commands`
- **Also likely:** `crates/loom-cli/src/lib.rs` (the `--answer` flag, if `DecisionsCommand` lives there) — inferred; exists at HEAD
- **Documents:** none named; `website/docs/guides/decisions.md` (planned by `story:decision-inbox-commands`) may need `--answer` — inferred
- **Confidence:** medium — every path the story names is cited and checked, but four of the five code paths do not exist at HEAD, and the story conflicts with `story:decision-request-entity` on what is new in the yaml (see defects)
- **Would collide with:**
  - `ess/domains/decision.yaml`: `story:decision-request-entity`, `decider-port-and-grounding-check`, `decision-routing-by-holder`, `silence-and-expiry-at-a-host-tick` — ordered by `depends_on` (all before it). `story:non-independent-review-becomes-decision-request` (adds the review stop) — no `depends_on` path.
  - `crates/loom-cli/src/decisions.rs`: `story:decision-inbox-commands`, `silence-and-expiry-at-a-host-tick`, `question-and-outcome-decisions-hold-runs`, `decisions-resume-the-held-run` — ordered (before it). `story:tree-waits-open-decision-requests` and `story:daemon-continues-a-decided-request` — no `depends_on` path in either direction. Both depend on `story:decisions-resume-the-held-run`, as this story does, so nothing orders them against it.
  - `crates/loom-executor` `model_decider.rs`: `story:llm-decider` — ordered (before it). `story:non-independent-review-becomes-decision-request` extends `ModelDecider` — no path.
  - `crates/loom-governor` `held_case_grounds.rs`: `story:grounds-resolve-against-held-cases` — ordered (before it).
  - `crates/loom-decision/src/lib.rs` and `generated/rust/loom`: every story that edits the loom ESS or adds a module. Those in the closure are ordered; `story:non-independent-review-becomes-decision-request` is not.
- **Safety fact:** every new member is `Optional` in the contract (`decision.yaml:245`, `:255`, `:146-147`, `:166`, `:466`, `:549-550` at 00e3557), so a stored request written before this story still decodes — step 2 (`file:line`), unproven
