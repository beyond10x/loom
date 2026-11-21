---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-design-2
kind: review-result
status: active
title: Plan critic (design), round 2, on story:ess-096-upgrade
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
approve

**Your three questions:**
- **My round 1 finding is answered.** Out of scope now says `story:commission-docs-read-one-ess-version-line` stays open, and why (`.engineering/planning/story/ess-096-upgrade.md:222-224`). The Outcome makes the same condition instead of promising one line with none (`:27-29`).
- **The correction adds no design defect.**
  - The new test `hold_incomplete_request.rs` belongs to the same unit as the `decide_hold` change it proves, and no other story names it.
  - The new checks only read this story's own files: the 1a relation check, the scenario count pinned at 44 with "stop and report" if it differs, the empty `SKIPPED.md` and the error-name count.
  - The edges are unchanged: 5 outgoing `depends_on`, 0 incoming, so there is no cycle.
- **Surfaces shared with `story:connectors-relay-client`:**
  - **Typed scope:** none. Its 8 paths are under `crates/loom-connectors/`, plus `crates/loom-sdk/src/lib.rs` and `website/docs/reference/sdk-stability.md`. The upgrade marks `sdk-stability.md` explicitly as not in scope.
  - **Body only (its § Scope):** besides `CHANGELOG.md`, it names `Cargo.lock` (`connectors-relay-client.md:327`) and `AGENTS.md` § Connectors (`:333`). The upgrade edits `Cargo.lock` for the ess packages and `AGENTS.md` § ESS. So it is the same files in different entries, with neither acceptance reading what the other writes. Whether they can merge side by side is a parallel-safety question, not a split defect.

**What I read:**
- The revision 4 body at `c1ae54ba`: Outcome, Acceptance, Out of scope and Scope.
- The ledger `ess096-changes-2.md` and `w5-sdc-approval.md`.
- `story:connectors-relay-client`, both its typed scope (`show --format json`) and its body.
- `aep plan artifact graph --format json` for edges into and out of the story.
- A grep of the planning store for `hold_incomplete_request`.

**What I could not establish:**
- **Wave 6 has no edges behind it (out of my lane, and a change to other stories, not this one):** `w5-sdc-approval.md:5` puts `decision-request-entity`, `boundary-admits-declared-http-egress` and `bound-commands-run-confined` in wave 6 "on 0.96.0". No `depends_on` edge from them to this story records that order (incoming edges: none). `decision-request-entity.md:121` still says "Loom pins `ess` 0.86.0".
- Whether 0.86.0 accepts the 2a guard: still unrun, as in round 1.

```findings
[]
```
