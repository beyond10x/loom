---
format: aep.planning-md/3
id: review-result:decision-suspension-reason-security-1
kind: review-result
status: active
title: Security review pass 1 on story:decision-suspension-reason (wave 2026-11-21-w3, unit U2, head 794fa6c2)
relations:
- reviews: story:decision-suspension-reason
revision: 1
---
unit: story:decision-suspension-reason (w3 U2), head 794fa6c2, tree clean apart from my 2 new test files
verdict: red
cases: executed 916→925, red 1
origin: introduced 1, pre-existing 0, undecided 1
wrote-outside-worktree: 1 directory, `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w3/u2/sec1/` (logs, scripts, diffs, story copy)
needs-coordinator: yes. Decide whether finding 1 holds the unit now: nothing in the tree calls `resume_after_decision` yet.

`git diff --stat` is empty: both files are new and untracked (`git status` at the end). Neither is an implementation file.

**Invariants**
1. Does not hold. In `crates/loom-commission-testkit/tests/security_w20261121w3_u2.rs`, `security_a_stale_decision_resume_does_not_consume_a_later_hold_for_another_request` fails. Run alone: `the DR-1 decision resumed the run held for DR-2: the resume ended Ok(NoAdmissibleAction(Unit(true))), the executor ran 1 times, and the DR-2 hold is now None`. These pass: another case gives `OtherCommission`; a run store holding the Run for another commission gives `OtherCommission`; `resume_after_grant` on a `Resuming` decision hold gives `HeldForDecision` with no governor call. Restart and second resume are covered by the implementor's tests.
2. Holds (runtime.rs:1619-1625, :1732). `Resumed::Decision` performs nothing, and the first iteration reloads the case at its current revision. The smuggled case below shows effects invoked 0.
3. Holds. I read every changed line of `git diff 39283f1b..HEAD -- '*/tests/*'`. Each one unwraps or wraps a member in `Some`, or adds `("Decision","DecisionHold")` in `skeleton.rs`. No case is deleted, no `#[ignore]` added, no assertion weakened.
4. Holds. `security_a_suspension_file_written_before_the_story_decodes_as_an_approval_hold` passes: a hand-written file in the base format, with `arguments` null and with `{}`, reads back as Authority with all four members. `..._whose_reason_and_members_disagree_is_refused_and_kept` passes for 9 mismatched files: get, take, release and hold each refuse, and the file stays byte-identical. `security_the_file_store_writes_no_hold_whose_reason_and_members_disagree` passes because `enact`'s read-back check refuses before writing.
5. Holds. `crates/loom-intake-slice/tests/security_w20261121w3_u2_approve.rs` `security_approve_writes_no_grant_for_a_decision_hold` passes for Pending and Resuming holds, clean or smuggled. The grant directory holds only `.lock`, and the Suspension is unchanged.
6. Holds. `security_a_decision_hold_with_a_smuggled_request_is_never_performed` passes. `resume_after_grant` refuses `HeldForDecision` with no governor call. `resume_after_decision` with a request id returns `ResumeError::Store` and keeps the hold. With only action and arguments smuggled, nothing is invoked. `approve` needs an Authority capability (`grant.rs:355`).

**Suite** (`ESS_TOOLCHAIN=0.86.0`, the gate's own commands)
- `cargo test -p b10x-loom-commission -p b10x-loom-commission-conformance -p b10x-loom-commission-testkit --locked --no-fail-fast`: exit 101, 520 passed, 1 failed (mine), 103 binaries. A first run without `ESS_TOOLCHAIN` had 20 extra failures from the `ess` version check; logs are in `sec1/suite-commission.log`.
- `cargo test -p b10x-loom-intake-slice -p b10x-loom-mandate --locked --no-fail-fast`: exit 0, 404 passed, 4 ignored.

**Findings**
1. Blocker, NEEDS-CHANGE, introduced. Sequence: a resume reads hold DR-1. Before its take, another resume consumes DR-1 and the run is held again for DR-2. The first resume's take (runtime.rs:2827-2830, `action_request_id: None`) then matches the DR-2 hold, because `other_request` (ports/suspension.rs:180) only compares request ids, and None equals None. It releases DR-2 and continues the run without that decision. The release-first path (:2856) has the same gap when the take answers `WrongState`. Reachable when two processes resume over one file store. No caller exists today; the next story adds one.
   Fix: take and release name the decision request (ESS `TakeSuspension`/`ReleaseSuspension` input), and `decide_take`/`decide_release` compare it with the held `DecisionHold.request_id`.
2. Note, INFEASIBLE, introduced. The conformance target can store a Suspension that breaks the "present together" invariant. For a Decision message with explicit `arguments: null`, the codec reads arguments as `Some(Null)` while the file store reads them as absent (conformance lib.rs:537, codec `optional_json`). `decide_hold` (ports/suspension.rs:135) and the in-memory store check no invariant. The synthesized suite has no Decision hold, so nothing reaches this.
3. Note, judgement, origin undecided (story design, not run against base). A crash after the release and before the loop's first step leaves the Run with no Suspension, and nothing can resume it. This holds on both paths and follows from the story's own order.

**Implementor decisions reviewed**
- D1 release-first: at most one resume continues, because the release decides. With one run store per process I found no interleaving that loses the run without a crash. The gap in finding 1 remains.
- D3: if the release fails after `ResumeRun`, the hold stays `Resuming` on disk and a later resume recovers it. D4 matches story lines 90-95. D5 and D6 are correct. D6 is needed: an old Authority file with null `arguments` would not decode under story line 100. HeldFor* carrying no data matches the story.

**Also cleared**
- Every Suspension writer: the runtime always pairs Authority with a request and Decision with none; the file store refuses the forbidden combination. `grant.rs` and `handoff.rs` write none.
- CLI: `resume` maps the three new refusals to 19, and no existing exit status changed.
- Boundaries: `crates/loom-commission/Cargo.toml`, `.engineering/`, `ess/system.yaml`, `ess/domains`, `ess/intake`, `locked_dir.rs`, `file_store.rs`, `loom-governor/src/lib.rs` and the workspace version are all unchanged. No new `/home/` literal, no `UNMAPPED:`.

**Could not establish**
- Not run by me: drift, docs-drift, docs-check, deps-guard and the CLI suite. I relied on the implementor's gate exits, all 0.
- Disk at the end: 87G free on `/`, `target/` at 41G.

`git status --short`:
```
?? crates/loom-commission-testkit/tests/security_w20261121w3_u2.rs
?? crates/loom-intake-slice/tests/security_w20261121w3_u2_approve.rs
```

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 2827
  category: concurrency
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: resume_after_decision takes and releases with no request id, so a stale resume for DR-1 consumes a later DR-2 decision hold and continues the run without DR-2's decision
- file: crates/loom-commission-conformance/src/lib.rs
  line: 537
  category: integrity
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the conformance codec reads a null arguments of a Decision hold as present while the file store reads it absent, and neither decide_hold nor the scenario store refuses the invariant-breaking result
- file: crates/loom-commission/src/runtime.rs
  line: 2848
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: undecided
  message: a crash after the release and before the first loop step leaves a decision-held Run with no Suspension and nothing able to resume it, as the story's order implies
```
