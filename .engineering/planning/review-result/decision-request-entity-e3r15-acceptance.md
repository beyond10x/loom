---
format: aep.planning-md/3
id: review-result:decision-request-entity-e3r15-acceptance
kind: review-result
status: active
title: Plan critic (acceptance) on revision e3r15 of story:decision-request-entity
relations:
- reviews: story:decision-request-entity
revision: 1
---
approve

I found nothing to fix. Every acceptance bullet this revision added or changed can fail, and each one names the command or test, the file and the value.

1. **Variant lists:** they match the SDC file. In `~/beyond10x/sdc/ess/domains/decision.yaml` (head `8db4b93`, `ess/` last changed at `00e3557`), `:109` and `:171` are `ordered: true`. `:110` is `[none, weak, moderate, strong]` and `:172` is `[A0, A1, A2, A3, A4, A5]`, and the file has no other `ordered` key. Story `:263-269` writes the same two lists in the same order. Before this story `ess/domains/decision.yaml` does not exist, so the test fails then and passes after.
2. **JSON path:** it is real. `ir-ord-37.json` has `types` as an object keyed by the full type name, with `ordered: true` on `loom.probeorder.Strength` and `loom.probeorder.AutonomyLevel` only. I ran `ess specify compile --path ord-37 --format json` on a copy in `dre-critic/` and stdout has the same shape, exit 0. "No other type" can be checked because `ir.rs:405` drops the field when it is false.
3. **Leftover `ess/31` or dropped order:** none. Of the grep hits, `:125` and `:308` name the raise from `ess/31`. `:135` is the `[]` construct being valid from `ess/31`. `:140`, `:148-149` and `:180-187` are history or the implemented `story:ess-format-31`. All the `ordered` hits match the decision, including Out of scope at `:287-289`.
4. **ESS citations at tag `0.96.0`:** each one says what the story says.
   - `view.rs:1084-1089`: the `min`/`max` arm admits `Leaf::Enum { ordered: true }`.
   - `types.rs:1249-1270`: `ordered_below_format` refuses the key below `ess/36` as `UnsupportedFormatVersion`.
   - `ir.rs:402-407`: the `ordered` field, left out when false.
   - `items.rs:150-153`: the fixed `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
   - `expression.rs:3065`: `fn compare`. Every `ordered` hit in that file is prose about instants and collections.
   - `spec-versions.md:78-79`: the `ess/36` row (`ordered: true`, 0.91.1) and the `ess/37` row ("no construct of its own").
5. **SDC references:**
   - `docs/decisions.md:263-264` holds "two open requests for the same record id must not exist".
   - `:351-353` is item 4, "as the contract, moved into Loom's ESS when it is taken".

Outside my lane (design):
- Story `:269` makes the test require `format: ess/37` exactly. The test then also fails when a later story raises the format, not only "when the format is lowered" as `:270` says. If that is not intended, change `:268` to "declares `format: ess/37` or a later format".
- The ledger (`e3r15-changes.md:110-115`) notes that the Scope figures are stale (`[..; 29]`, `:17`, `:24`). That is for the scoper.

What I read: `story:decision-request-entity`, all of it (1 of 1 ids), plus the diff against HEAD, the e3r15 ledger, the SDC file at today's head and at `00e3557`, the two `decisions.md` ranges, the seven ESS files at tag `0.96.0`, `e3-sdc-answers-6.md:9-11`, `sdc31-validate.log` and the probe IR.

What I could not establish: the flag is shown on the probe domain `loom.probeorder`, not on `loom.decision`, which does not exist yet. I generated no Rust from the domain. The "no `PartialOrd`" claim rests on generator source I read, not on output I produced.

```findings
[]
```
