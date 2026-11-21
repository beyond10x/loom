---
format: aep.planning-md/3
id: review-result:ess-format-37-design-1
kind: review-result
status: active
title: Plan critic (design), round 1, on story:ess-format-37
relations:
- reviews: story:ess-format-37
revision: 1
---
needs-revision

story:ess-format-37 — its acceptance fixes `181 scenario(s)` and `16 file(s), 97 scenario(s)` as read at `9b4170fb`, but the active wave-6 story story:boundary-admits-declared-http-egress adds authored scenarios to `ess/ess-inputs.yaml` and refusal reasons to `ess/domains/resource.yaml`, and no edge or wording covers that, so the lines fail once wave 6 lands; state the validate line, the suite count and the ids as "the same as the parent commit's" and keep the numbers in the body only as the probe — .engineering/planning/story/ess-format-37.md:85-88, .engineering/planning/story/boundary-admits-declared-http-egress.md:184-191,284

story:ess-format-37 — its single commit puts `CHANGELOG.md` into the unit's first commit, but AGENTS.md § ESS requires the first commit to change only `ess/`, and the exemption the story claims covers only the red test; say two commits, `ess/system.yaml` alone and then `CHANGELOG.md`, and change the "exactly two files" bullet to match — .engineering/planning/story/ess-format-37.md:24,75,82; AGENTS.md:231-235; precedent `git show --stat 4ab05d5b` (`ess/system.yaml` only)

story:autonomy-grant-table — "Loom's `ess/31` has none; `ordered:` needs `ess/36`, `story:ess-format-31`" goes false once the main system is at `ess/37` and the decision domain keeps `ordered: true`; reword it to cite story:ess-format-37 and keep "the holder order is the resolver's own code". It will be ordered after story:ess-format-37 through story:decision-request-entity once that edge lands — .engineering/planning/story/autonomy-grant-table.md:63

story:decider-port-and-grounding-check — "the Loom copy drops `ordered: true`, which `ess/31` does not have" goes false for the same reason; keep "tested by membership, never by order" and drop the reason. It will be ordered after story:ess-format-37 through story:autonomy-grant-table — .engineering/planning/story/decider-port-and-grounding-check.md:68

epic:decision-requests-in-the-runtime — its story list names story 0 as story:ess-format-31 only, its Out of scope says "`ordered:` enums (... story 1 drops `ordered: true`)", and its acceptance says "`ess/system.yaml` declares `format: ess/31`". All three need story:ess-format-37 added or the text moved to `ess/37` — .engineering/planning/epic/decision-requests-in-the-runtime.md:216,325-326,417

Answers to the five questions:
1. Edges are right. `decomposes`, `serves` and the implemented `depends_on: story:ess-096-upgrade` are the same as story:ess-format-31's, and that story is reachable through it. I walked 12 `depends_on` edges from the story to 11 nodes, all implemented. story:decision-request-entity is not among them, so the planned edge from it to this story creates no cycle. That planned edge also orders 27 stories after this one, both stale stories above included.
2. Proposed or draft stories that write `ess/system.yaml` and have no `depends_on` path to or from this story: story:tree-run-command, story:gates-document-evaluated-in-the-governor, story:observe-executor-reads-workspace-files and story:engine-configuration-document (draft). Each only adds a `domains:` line and none states the format, so none goes stale. Ordering them is the parallel-safety critic's question. story:decision-request-entity:13,166,231 is stale too and is already being revised.
3. It shares no file with story:bound-commands-run-confined. It shares `CHANGELOG.md` with story:boundary-admits-declared-http-egress (:276,292), but they run in different waves. The real coupling is finding 1. In the next wave's candidates (`aep plan artifact waves --status proposed`, wave 1), it shares `CHANGELOG.md` with story:engineering-protocols-dependency-removed (:165) and story:standing-tree-rests-quiescent (:210). story:decision-request-entity shares `ess/system.yaml` and `CHANGELOG.md` and moves to a later wave once its edge lands. story:bound-program-result-table shares nothing.
4. It is rightly its own story. It changes no behaviour while story:decision-request-entity adds a domain and a crate, and story:ess-format-31 set the same split. The commit shape is finding 2.
5. The epic puts raising the intake system (`ess/26`) and the commission system (`ess/22`) out of scope (:325), and no open story owns it. That does not matter here, because the decision domain goes into the main system only.

What I read: 15 artifacts (this story, story:ess-format-31, story:ess-096-upgrade, story:decision-request-entity, the epic, both wave-6 stories, the system.yaml writers above, the next wave's candidates). Commands: `aep plan artifact show` / `relations` / `graph --format json` / `waves --status proposed` / `validate` (valid; it reports only old review records), the grep you gave, plus `git log` and `git show --stat` on `ess/system.yaml`. I traced 4 acceptance reads to a producer (the suite count, the validate line, the state of `CHANGELOG.md` and `ess/` at the parent commit). 1 has a producer outside this story (story:boundary-admits-declared-http-egress), and no edge records it.

What I could not establish:
- Whether the synthesized count of 181 also changes after wave 6. I ran no `ess`; that authored scenarios are added is in the egress story's text.
- Whether the new domains added after the raise trip the `ess/32` rule that refuses two enum variants spelled alike on the wire. That is a note, not a finding.
- Whether `ess_gate.rs` checks commit shape. Only the AGENTS.md text was read.
- Out of my lane (parallel safety): `CHANGELOG.md` is shared with the next wave's candidates, and the four stories that write `ess/system.yaml` have no ordering edge.

```findings
- file: .engineering/planning/story/ess-format-37.md
  line: 85
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance fixes `181 scenario(s)` and `16 file(s), 97 scenario(s)` as read at `9b4170fb`, but the active wave-6 story story:boundary-admits-declared-http-egress adds authored scenarios to `ess/ess-inputs.yaml` and refusal reasons to `ess/domains/resource.yaml`, and no edge or wording covers that, so the lines fail once wave 6 lands; state the validate line, the suite count and the ids as the same as the parent commit's"
- file: .engineering/planning/story/ess-format-37.md
  line: 24
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its single commit puts `CHANGELOG.md` into the unit's first commit, but AGENTS.md § ESS requires the first commit to change only `ess/`, and the exemption the story claims covers only the red test; say two commits, `ess/system.yaml` alone and then `CHANGELOG.md`"
- file: .engineering/planning/story/autonomy-grant-table.md
  line: 63
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"Loom's `ess/31` has none; `ordered:` needs `ess/36`\" goes false once the main system is at `ess/37` and the decision domain keeps `ordered: true`; reword it to cite story:ess-format-37"
- file: .engineering/planning/story/decider-port-and-grounding-check.md
  line: 68
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "\"the Loom copy drops `ordered: true`, which `ess/31` does not have\" goes false for the same reason; keep \"tested by membership, never by order\" and drop the reason"
- file: .engineering/planning/epic/decision-requests-in-the-runtime.md
  line: 216
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its story list names story 0 as story:ess-format-31 only, its Out of scope says story 1 drops `ordered: true` (:325-326), and its acceptance says `format: ess/31` (:417); all three need story:ess-format-37 added or the text moved to `ess/37`"
```
