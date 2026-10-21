---
format: aep.planning-md/3
id: story:metaharness-session-executor
kind: story
status: active
title: Loom hosts the metaharness session executor for harness, Claude Code and Codex
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:delete-harness-port
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-sdk/Cargo.toml
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: crates/loom-session-executor
- confidence: cited
  path: ess/SKIPPED.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: inferred
  path: ess/domains/session.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: website/docs/reference
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 20
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T00:00:06Z", actor: "human:timo", revision: 19, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-10-21T00:00:06Z", actor: "human:timo", revision: 20, decided_on: {"recorded":{"review_outcome":4}}}
---
## Outcome

Loom hosts the `AgentExecutor` that runs an agent session through metaharness (harness, Claude Code
or Codex) in a scratch copy of the workspace and returns the session's change as a
`ProposedAction`, which Commission revalidates and an effect applies (ADR 0098 points 2 and 3). The
workspace itself is untouched until the effect applies. takt builds the executor first as a
standalone crate with no takt-only dependency (ADR 0099 point 3); this story moves that crate into
Loom and exports it from loom-sdk. Loom depends on metaharness; metaharness never depends on
loom-sdk. No crate in Loom depends on metaharness today (`Cargo.lock`, 0 matches, 2026-10-20).

## Why

The session executor is agent work, which ADR 0098 sends through metaharness, and Loom is where an
application finds it. takt's Codex executor extends the same crate.

## Acceptance

- The move: the first commit copies takt `crates/session-executor` at takt `main` `7f2843bb` into
  `crates/loom-session-executor/`; `diff -r` of that commit's `src/` against takt's shows no change
  but the package name and its path dependencies. The decisions (section "Decisions on the session
  domain") are later commits.
- The session domain: `ess/domains/session.yaml` (from the draft below, its eight markers replaced by
  the decisions, no `UNMAPPED:` left) is listed in `ess/system.yaml` and `ess/ess-inputs.yaml`;
  `task spec`, `task ess-gate`, `task drift` and `task conform` pass with every session scenario run
  against the executor (none in `ess/SKIPPED.md`). The transcript digest and the summary are optional
  fields on `ExecutorStep`, declared in `ess/commission/domains/responsibility.yaml` first.
- `crates/loom-session-executor/tests/proposes.rs::a_session_change_returns_as_a_proposed_action`:
  over a recorded metaharness session on a fixture workspace, the executor returns a
  `ProposedAction` whose arguments are `{files}` only (a deletion with `content: null`), mints a new
  `SessionId` per call, names the real `commission_id` and `frontier_id`, and leaves the fixture
  workspace's bytes unchanged.
- `…::a_proposal_on_a_moved_case_is_refused_at_revalidation`: run through `run_until_blocked`, a
  proposal whose case moved while the session ran is refused and the effect is not invoked.
- `…::a_concurrent_workspace_write_refuses_the_stale_proposal`: a workspace file written while the
  session runs moves the case revision and the proposal (bound to `expected_case_revision`) is
  refused; if the write cannot be shown to move the revision, the executor diffs against the
  frontier's snapshot and this test shows the concurrent write is never proposed back.
- `…::the_effect_applies_the_admitted_change`: with a test effect, the admitted proposal's files
  reach the workspace only through the effect.
- `…::the_session_trace_is_never_evidence`: the step's `ExecutorStep` carries the transcript digest
  and the summary, and no evidence record is submitted for them.
- `…::a_session_usage_is_charged_to_the_run`: the executor implements `run_with_usage`; a session's
  tokens and cost reach the Run's charges and budget, and a breach suspends the Run with `RunBudget`.
- `…::a_repeated_proposal_keeps_its_identity`: two sessions proposing the same files with different
  summaries are the same request for the idle check.
- `crates/loom-sdk/tests/host_ports.rs` reaches the executor through `loom_sdk`, and the stability
  page lists it.
- Loom's graph gains metaharness through the new crate only: `cargo tree --locked -i` of the
  metaharness library package lists `loom-session-executor` (and `b10x-loom-sdk` through it) and no
  other Loom package.
- `task check` exits 0, with no live model call.

## ESS first

The session domain (`loom.session`, `ess/domains/session.yaml`) lands in this story's first commit,
together with the executor that implements it, because a domain declared before its implementation
fails `task conform`. The section "The session domain" below holds the draft and the questions takt's
crate must answer first. The red step is the first synthesized session scenario run against the
moved executor.

## Depends on

- takt's standalone session-executor crate (`upstream-blocker:takt-session-executor-crate`), which
  also answers the session domain's open questions (section "The session domain").
- `story:delete-harness-port`, which edits `README.md` and the loom-sdk files first and itself lands
  after every other story that edits `host_ports.rs`, `crates/loom-sdk/src/lib.rs`, the stability
  page or `Cargo.lock` (`story:engineering-protocols-090`, `story:governor-exposes-canon-decision`,
  `story:sdk-confines-any-child`, `story:authority-sees-the-request`, `story:agent-executor-host-port`,
  `story:chooser-executor-for-plugin-turns`).

## Not in scope

- Governing a session per tool call: that waits on metaharness (ADR 0098, Consequences).
- Changes inside metaharness or takt.

## Scope

Crates: a new `loom-session-executor` crate (name inferred), `loom-sdk`.

- `crates/loom-session-executor/` (new), `Cargo.toml` (workspace members), `Cargo.lock`
- `crates/loom-sdk/Cargo.toml`, `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/tests/host_ports.rs`
- `website/docs/reference/sdk-stability.md`
- `ess/SKIPPED.md` (the session scenarios leave it)
- `CHANGELOG.md`, `AGENTS.md`, `README.md`

## The session domain (moved here from story:ess-covers-agent-sessions-and-intake)

Wave 2026-10-20-w4 drafted the `loom.session` domain and did not land it: a domain declared before
its implementation fails `task conform` by design (an unsupported or errored scenario fails the
report; `crates/loom-conformance/tests/conform.rs:31-37`, `ess/SKIPPED.md:7-9`), and `task
ess-gate` refuses an `UNMAPPED:` marker. The domain therefore lands with this story, in its first
commit, once takt's crate answers these questions:

1. Session identity: an id Loom mints, metaharness's session id, or the executor call.
2. The changed-file shape: deletions, renames, binary content, file modes.
3. The session's relation to the Commission run and the executor call: cardinality, owns or
   references.
4. What the base is: a Git commit, the case's implementation revision, or a digest.
5. The trace reference: its form, and whether a session can have none.
6. Which frontier action the change is proposed as, and how the files are carried in the proposal's
   JSON arguments.
7. Whether Proposed and Applied are states of the session or only of Commission's ActionRequest;
   what a running, failed or empty session is.
8. The recheck's other answers (not admitted, needs authority) and an effect's refusal, for a
   session's change.

The draft (validates with `--strict-requires`, synthesizes 66 scenarios over `ess/` with 0
refusals, 5 of them the session's; every marker is one of the questions above):

```yaml
# DRAFT, not committed (story:ess-covers-agent-sessions-and-intake, Part 2). It carries UNMAPPED
# markers, which `task ess-gate` refuses (AGENTS.md § ESS, condition 4), so it stays outside ess/
# until the owner settles them. Sources read, and the only ones modelled from: Atlas ADR 0098
# point 2; story:metaharness-session-executor (Outcome, Acceptance);
# commission.responsibility.ExecutorOutcomeProposedAction and RevalidateActionRequest
# (ess/commission/domains/responsibility.yaml).
domain: loom.session

summary: >-
  The agent session an AgentExecutor runs through metaharness (harness, Claude Code or Codex) in a
  scratch copy of the workspace, the change it returns against the base that copy was taken from,
  and the reference to its trace, which is never evidence.

naming:
  wire: session
  display: Session

types:
  # UNMAPPED: no source names what identifies a session: an id Loom mints, metaharness's own
  # session id, or the executor call that ran it. Uuid is a placeholder so the entity can exist.
  - name: loom.session.SessionId
    kind: newtype
    of: Uuid

  # ADR 0098 point 2: "harness, Claude Code or Codex".
  - name: loom.session.SessionKind
    kind: enum
    variants: [Harness, ClaudeCode, Codex]

  # One file the session changed against its base, as the proposal carries it
  # (story:metaharness-session-executor: "ProposedAction carrying the changed files"; "the admitted
  # proposal's files reach the workspace only through the effect").
  # UNMAPPED: whether a change can delete or rename a file, carry binary content or a file mode;
  # the sources say only "the files it changed". `content: String` is a placeholder.
  - name: loom.session.ChangedFile
    kind: struct
    fields:
      - name: path
        type: String
      - name: content
        type: String

entities:
  # The session one AgentExecutor ran through metaharness (ADR 0098 point 2).
  # UNMAPPED: the relation to the Commission run and executor call it ran for
  # (loom.run.CommissionRunId mirrors commission.responsibility.RunId): one session per executor
  # call, per action or per run is not stated, nor owns against references.
  - name: loom.session.AgentSession
    identity:
      name: session_id
      type: loom.session.SessionId
    fields:
      - name: kind
        type: loom.session.SessionKind
      # The scratch copy of the workspace the session ran in (ADR 0098 point 2); the workspace
      # itself is untouched until the effect applies (story:metaharness-session-executor).
      - name: scratch_workspace
        type: String
      # UNMAPPED: what the base is (a Git commit of the workspace, the case's implementation
      # revision, a content digest) is not stated; `String` is a placeholder.
      - name: base
        type: String
      - name: files
        type: List<loom.session.ChangedFile>
      # The reference to the session's trace: never evidence (ADR 0074, ADR 0098 point 2); the
      # step's observation carries it (story:metaharness-session-executor).
      # UNMAPPED: its form (a path, a URI, metaharness's id) and whether a session can have none.
      - name: trace
        type: String
      # The action the change is proposed as (commission.responsibility.ExecutorOutcomeProposedAction
      # `action`).
      # UNMAPPED: which frontier action a session's change is proposed as, and how the files are
      # carried in `ProposedActionArguments` (Json), are not stated.
      - name: action
        type: String
    # UNMAPPED: whether Proposed and Applied are states of the session or only of
    # Commission's ActionRequest, and what a running, failed or empty session is, are not stated.
    lifecycle:
      initial: Proposed
      states: [Proposed, Applied]
      terminal: [Applied]
      transitions:
        - {name: apply, from: [Proposed], to: Applied}

errors:
  # The case moved while the session ran: the recheck refuses the proposal and the effect is not
  # invoked (story:metaharness-session-executor; commission.responsibility.ActionRequestStale).
  - name: loom.session.SessionCaseMoved
    fields:
      - name: session_id
        type: loom.session.SessionId

events:
  - name: loom.session.SessionChangeProposed
    fields:
      - name: session_id
        type: loom.session.SessionId
      - name: action
        type: String
      - name: files
        type: List<loom.session.ChangedFile>

  - name: loom.session.SessionChangeApplied
    fields:
      - name: session_id
        type: loom.session.SessionId

commands:
  # The executor returns the session's change as a proposed action (ADR 0098 point 2).
  - name: loom.session.ProposeSessionChange
    input:
      - name: session_id
        type: loom.session.SessionId
      - name: kind
        type: loom.session.SessionKind
      - name: scratch_workspace
        type: String
      - name: base
        type: String
      - name: files
        type: List<loom.session.ChangedFile>
      - name: trace
        type: String
      - name: action
        type: String
    outcomes:
      - name: proposed
        creates: loom.session.AgentSession
        instance: session_id
        sets:
          kind: input.kind
          scratch_workspace: input.scratch_workspace
          base: input.base
          files: input.files
          trace: input.trace
          action: input.action
        emits: [loom.session.SessionChangeProposed]
        payload:
          loom.session.SessionChangeProposed:
            session_id: input.session_id
            action: input.action
            files: input.files

  # Commission rechecks the proposal against the case's revision and an effect applies it
  # (ADR 0082; commission.responsibility.RevalidateActionRequest).
  # UNMAPPED: the recheck's other answers (not-admitted, needs-authority) and an effect's refusal
  # are not stated for a session's change; only the moved case and the applied change are.
  - name: loom.session.ApplySessionChange
    input:
      - name: session_id
        type: loom.session.SessionId
    outcomes:
      - name: case-moved
        external: the governor's current revision of the case is not the revision the session ran at
        error: loom.session.SessionCaseMoved
        payload:
          loom.session.SessionCaseMoved:
            session_id: input.session_id
      - name: applied
        moves: loom.session.AgentSession.apply
        instance: session_id
        emits: [loom.session.SessionChangeApplied]
        payload:
          loom.session.SessionChangeApplied:
            session_id: input.session_id
      - name: wrong-state
        wrong_state: true
        error: loom.session.SessionCaseMoved

views:
  - name: loom.session.AgentSessions
    source: loom.session.AgentSession
    consistency: read_your_writes
    fields:
      - name: session_id
        type: loom.session.SessionId
      - name: kind
        type: loom.session.SessionKind
      - name: action
        type: String
      - name: state
        type: loom.session.AgentSession.State
```

## Decisions on the session domain

Decided by the orchestrator session (b10x-chef-orchestrator, message of 2026-10-21, inside ADR 0098
and 0099, no published contract broken beyond this story's release), on the answers in
`~/.cache/loom-wave-2026-10-20-w4/session-questions.md`:

1. Identity: the executor mints a `SessionId(Uuid)` per `run` call; no port change. The vendor's
   `session_id` stays an optional field.
2. Changed files: `{path, content: Optional<String>}`; a deletion is null, a rename is a delete
   plus an add, binary content is refused, file modes are not carried.
3. The link to the run: `commission_id` and `frontier_id`, no `RunId` on the port; the executor
   passes the real ids (takt's crate passes nil UUIDs today).
4. No base field: the base is the frontier's `case_revision`, which Commission binds as the
   request's `expected_case_revision`.
5. The trace reference: an optional field on `ExecutorStep` carrying the transcript digest
   (SHA-256) only; no path, and nothing in the arguments.
6. The proposal names a frontier action; its arguments are `{files}` only. The session's summary
   travels on the `ExecutorStep` observation beside the trace digest, so a repeated proposal keeps
   its identity for the idle check.
7. Proposed and Applied are not session states (they are the ActionRequest's); a session is one
   record. A failed session returns `Suspended(ExternalAvailability)`; an empty one proposes
   `files: []`.
8. The recheck and an effect's refusal are Commission's existing answers; the draft's
   `SessionCaseMoved` and `ApplySessionChange` are dropped.

Two more, from reading takt's crate:

- Stale diff: the crate diffs the scratch copy against the workspace as it is when the session
  ends. The proposal binds `expected_case_revision`, and a test proves that a concurrent workspace
  write moves the case revision so the stale proposal is refused; if that cannot be proven, the
  executor diffs against the frontier's snapshot instead of the live workspace.
- Budget: the executor implements `run_with_usage`, so a session's usage reaches the Run's charges
  and budget.

The crate to move is takt `crates/session-executor` (`b10x-session-executor`) at takt `main`
`7f2843bbc6bf70d0fc3b58b0ead2cac2401c6ef2` (last changed in `c933f17f`); it depends on
`b10x-loom-sdk`, `metaharness`, `metaharness-b10x` and `serde_json` only, and takt's CI on that
commit is green. The decisions above are made in Loom's copy after the move.
