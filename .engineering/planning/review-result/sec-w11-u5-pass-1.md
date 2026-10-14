---
format: aep.planning-md/3
id: review-result:sec-w11-u5-pass-1
kind: review-result
status: active
title: Security review, loom w11 U5 regression evidence integrity, pass 1
relations:
- reviews: story:regression-evidence-integrity
revision: 1
---
needs-revision, then fixed in the unit

Security review of story:regression-evidence-integrity, cases in
`crates/loom-governor/tests/security_w11_u5_governor.rs` (7) and
`crates/loom-intake-slice/tests/security_w11_u5_regression.rs` (7): 3 red on the first run, 14
green after the fix.

- A record without `observed_at` was not checked against the observations received, so one citing
  an observation the governor never received was accepted. Decided: acceptance clause 2 holds as
  written. Fixed: every record citing an unreceived observation is refused, naming it.
- Only the first of repeated `observed_at` members was checked. Fixed: every member is checked.
- A test path changed by one `tests.write` and written back to its base bytes by another stayed a
  committed test path, so a failing pre-existing test counted as the regression result. Fixed: a
  path counts only when its bytes differ from those before the run's first write to it.

Not fixed here, kept as notes:
- A one-byte change to an existing failing test file still makes that file's regression run count
  as red first; this follows from "each path written".
- Received observations carry no case, so a record may cite another case's observation or one
  already cited; only the slice's verifier stops reuse.

Attacked and held: shell metacharacters, `..` and absolute paths in `{path}`, `{path}` twice,
other spellings of one instant, non-text `observed_at`, an observation delivered after the record,
a restarted governor, a resumed executor, a 0.21.0 resume context.

```findings
- file: crates/loom-governor/src/lib.rs
  category: acceptance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: a record without observed_at citing an unreceived observation was accepted; fixed
- file: crates/loom-intake-slice/src/executor.rs
  category: integrity
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: a test written and then written back stayed a committed test path; fixed
- file: crates/loom-governor/src/lib.rs
  category: integrity
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: only the first of repeated observed_at members was checked; fixed
- file: crates/loom-intake-slice/src/executor.rs
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: a trivial change to an existing failing test file makes its regression run count as red first; not fixed in this story
- file: crates/loom-governor/src/lib.rs
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: received observations carry no case, so a record may cite another case's observation; not fixed in this story
```
