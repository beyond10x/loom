---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-design-1
kind: review-result
status: active
title: Plan critic (design), round 1, on story:ess-096-upgrade
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
needs-revision

story:ess-096-upgrade — the Out of scope section says `story:commission-docs-read-one-ess-version-line` "can close as moot", but the pin move does not deliver that story's outcome: the dispatcher prints two lines again as soon as it is newer than the pin, for example 0.97.0 with the pin at 0.96.0. The sentence should say that story stays open and that the move only hides the problem while the pin equals the dispatcher. — .engineering/planning/story/ess-096-upgrade.md:188-189, `aep plan artifact show story:commission-docs-read-one-ess-version-line` (## Outcome)

**Your four points:**
1. **One unit is right.** The pin move alone stays red (3 refusals in `commission:ess-gate` and `commission:conform`), so any split must run A then B:
   - **A:** 1a, 2a and `decide_hold`, under 0.86.0. Its first commit changes only `responsibility.yaml`; red is `commission:drift`. Its gate is `task check` under 0.86.0, plus the two security tests.
   - **B:** the pin move and regeneration. Its first commit changes only the three `requires:` lines; red is `ess_gate.rs:414`. Its gate is synthesize 44/0 under 0.96.0, plus every drift, docs and conform task.
   - **Why not split:** A exists only because 0.96.0 refuses the current spec, and its "0 refusal(s)" check can only be read once B lands. The split would be a two-item queue that buys nothing.
   - **It may not even be possible:** nothing shows 0.86.0 accepts the 2a guard. The same `any`/`all` form is accepted in the Suspension invariant at `responsibility.yaml:1313-1315`, which suggests it may be.
2. **No `depends_on` edge is missing in my lane.**
   - None of the upgrade's acceptance lines reads anything the nine stories produce. Its scenario count is read at its own base after the first commit, so it holds whatever lands first.
   - None of their acceptance lines reads the pin.
   - `story:child-case-runtime` changes the `Case` lifecycle, other declarations than 1a and 2a, and its red check (`commission:drift`) fails under either pin.
   - If parallel safety asks for an order, the stories should depend on the upgrade, so each one's first commit is written to 0.96.0 rules.
3. **Ownership:**
   - The two security tests are in `story:decision-suspension-reason`'s typed scope. That story is implemented and the upgrade already depends on it, so amending them is fine.
   - The release commit's files are left out explicitly (story line 186).
   - The CHANGELOG line goes under Unreleased, as § Releases allows.
   - The only overlap is the "moot" claim above.
4. **No contradiction.** `decision-suspension-reason` (lines 161-168) says only that a mixed hold is refused and nothing is stored, with no order of outcomes. No proposed story says anything about the order of `already-held`, `storage-failed` or `held`.

**What I read:** 1 drafted artifact and 11 others:
- `aep plan artifact show` on the upgrade, `child-case-runtime`, `commission-docs-read-one-ess-version-line` and `decision-suspension-reason`, and grep of the story files of all 8 stories that name `ess-inputs.yaml`.
- `aep plan artifact relations`, `graph --format json` and `validate` (valid, nothing relevant), and `waves --kind story --status proposed`.
- The ledger, the probe log, `AGENTS.md` § ESS and § Releases, and `responsibility.yaml:1280-1335`.
- Edges: I walked 5 outgoing `depends_on` (all to active or implemented stories) and found 0 incoming, so there is no cycle. I went outside the set to check those stories' own edges.
- Acceptance reads: I traced 4 to a producer. All 4 are recorded by edges: `ess-format-31` for the format line, plus `decision-suspension-reason`, the wave-4 base and the generated trees.

**What I could not establish:**
- The store shows revision 3 (frontmatter line 16 and the graph `version`), not the revision 4 you named, and `history` records nothing. I judged revision 3.
- Whether 0.86.0 validates and synthesizes the 2a branch. Nothing has run it, so whether a split is possible is open.
- **Out of my lane (parallel safety):**
  - Line 2 of `ess/ess-inputs.yaml` against the list lines that 8 proposed stories add to it, none of them ordered against this story.
  - `story:child-case-runtime` against the upgrade: both edit `responsibility.yaml`, regenerate `generated/rust/commission/` and `website/docs/reference/commission/`, and no edge orders them.
- **Out of my lane (scope):** `story:decision-request-entity:121` says "Loom pins `ess` 0.86.0". That goes stale if the upgrade lands first.

```findings
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 189
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Out of scope section says story:commission-docs-read-one-ess-version-line can close as moot, but the pin move does not deliver that story's outcome (the dispatcher prints two lines again as soon as it is newer than the pin), so the sentence should say that story stays open and the move only hides the problem while the pin equals the dispatcher"
```
