---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-acceptance-2
kind: review-result
status: active
title: Plan critic (acceptance), round 2, on story:ess-096-upgrade
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
approve

Round 2: all five findings are answered, and the correction adds no bullet that can't be checked. The story is at store revision 4. I ran the new commands read-only at head `c1ae54ba`:

1. **1a pinned:** the `sed -n '/^  - name: commission.responsibility.Suspension$/,/^    lifecycle:/p' … | grep -B1 'kind: references' | grep -o 'name: .*'` pipeline prints `name: run`, `name: action_request` and `name: case` today, as the bullet says. The start pattern matches exactly once, so the range is the `Suspension` entity. It goes red if `case` stays. `grep -c 'creates: commission.responsibility.Case$'` prints 0, and goes red if 1b is taken.
2. **Order before `already-held`:** `crates/loom-commission-testkit/tests/hold_incomplete_request.rs` does not exist at head, and the bullet marks it new. It checks a full hold followed by a mixed hold on both stores, and fails while `decide_hold` answers `AlreadyHeld` first (`crates/loom-commission/src/ports/suspension.rs:147-156`). The testkit already depends on `b10x-loom-governor` (`crates/loom-commission-testkit/Cargo.toml:12`).
3. **Count:** pinned to `44 scenario(s) (0 authored), 0 refusal(s)`. If the base count differs, the unit stops and reports it instead of editing the number. The count can now fail.
4. **Conform:** `grep -c '^- ' ess/commission/SKIPPED.md` prints 0 at head. The inference holds:
   - `crates/loom-commission-conformance/tests/conform.rs:264` requires the report total to equal the suite size.
   - `:170-196` refuses failed and errored scenarios, and any skipped or unsupported one that `SKIPPED.md` does not name.
   - So with the file empty, a passing gate means every scenario in the suite passed, the three named ones included.
5. **Error name:** `grep -c SuspensionRequestIncomplete` prints 0 at head on both `ess/commission/domains/responsibility.yaml` and `generated/rust/commission/src/responsibility.rs`. The bullet expects at least 1 on each, so it goes red if the error is missing.

Also checked:
- `Taskfile.commission.yml:31` runs `ess specify validate --path ess/commission`, the command the 1a bullet names.
- The test base is now filled in (3192 passed, 0 failed, 10 ignored).

Could not establish:
- The exact `warning[ESS-ENTITY-019]` text Commission gets under 0.96.0. The reviser inferred it from intake's warning, because validation can't run read-only while the pins say 0.86.0. The `sed | grep` part of the same bullet pins 1a regardless.
- The count 44 at the unit's base, and the conform scenarios passing with the changed `decide_hold`. Neither can be checked without building; the ledger says so.

Read: 1 artifact, revision 4, the whole Acceptance section; `.engineering/drafts/ess096-changes-2.md`; the cited lines in `conform.rs`, `ess_gate.rs`, `Taskfile.commission.yml` and the testkit `Cargo.toml`.

```findings
[]
```
