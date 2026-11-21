---
format: aep.planning-md/3
id: review-result:ess-format-37-design-2
kind: review-result
status: active
title: Plan critic (design), round 2, on story:ess-format-37
relations:
- reviews: story:ess-format-37
revision: 1
---
approve

Findings 1, 2 and 5 are fixed.
- **Finding 1:** every acceptance bullet on the suite, the compiled model and the validate line now compares against the parent of the first commit (story revision 4, `## Acceptance` bullets 3-5). The numbers from `9b4170fb` stay in the body as the probe only, and the body says they move with `story:boundary-admits-declared-http-egress`.
- **Finding 2:** the story now has two commits. The first changes only `ess/system.yaml` and the second only `CHANGELOG.md`, in `## Outcome`, `## ESS first` and acceptance bullet 2.
- **Finding 5:** the epic (revision 17) lists 0b `story:ess-format-37` at :219-220. Its Out of scope at :329-331 and its acceptance at :422-423 say `ess/37`.

Joining wave 6 causes no collision and no ordering problem.
- The only file it shares with either wave 6 story is `CHANGELOG.md`, and both sides only append to it.
- It does not touch `ess/ess-inputs.yaml`, `ess/domains/resource.yaml`, `ess/scenarios/` or `generated/`. The egress story does not touch `ess/system.yaml`.
- The one real interaction is that the egress story's new enum variants and `external:` outcomes of `AdmitLoomAgent` would be read under `ess/37`, and nobody has probed that pair. Because the checks are now relative, whichever story lands second re-runs its own checks against a parent that already holds the other story:
  - if the egress story lands second, its `task drift` and `task conform` run at `ess/37`;
  - if the format story lands second, its byte-identity checks run on a parent that already holds the egress changes.

  So no edge is needed.
- `depends_on` is unchanged, and `story:ess-096-upgrade` is implemented, so nothing blocks it inside the wave.

The revision introduced no new design defect.

What I read: the story at revision 4 and the epic at revision 17 again; the egress story's `## ESS first` and `## Waits on`; `bound-commands-run-confined.md`. Commands: `aep plan artifact show` and `grep`.

What I could not establish:
- Whether `ess verify conform synthesize` has `--out` and `--check` flags as acceptance bullet 3 uses them. I ran no `ess`; that is the acceptance critic's lane.
- `## Waits on` says the story shares `CHANGELOG.md` with both wave 6 stories, but `story:bound-commands-run-confined` names no `CHANGELOG.md` in its scope or body. That is a wording slip in the parallel-safety lane, not a design finding.

```findings
[]
```
