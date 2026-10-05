---
format: aep.planning-md/3
id: decision-blocker:confined-tests-publication-authority
kind: decision-blocker
status: open
title: Publish confinement after Gates resolves imported Commission history
relations:
- blocks: story:confined-tests-run
withholds: artifact
revision: 1
---
## Decision needed

Resolve authenticated publication of Loom's imported Commission history without bypassing
Gates. Implementation, all local checks, the bounded adversary repair and real confinement
qualification are complete. Source publication is still required.

## Evidence

`b10x-gates publish` 0.1.14 refuses:

```text
b10x-gates: updated pull request missing or ambiguous
```

The first unassociated GitHub-authored merge in the checked ancestry is
`1d0f4fcd58405f4890225d28788577cdecf4ed9f`, imported from Commission. Its Cargo manifest names
`https://github.com/beyond10x/commission`, and GitHub associates it with
<https://github.com/beyond10x/commission/pull/1>. The Loom commit-to-pulls endpoint returns `[]`.
The released Gates verifier looks up historical merge authority in the delivery repository.

The initial `branch authority missing or ambiguous` refusal was resolved with explicit operator
approval: Loom ruleset `24534858`, `b10x-bot-branch-authority`, is active for all branches with
App `4579525` as its only bypass actor. Its response exactly matches Gates' required fields.

The common security scan passes (208 commits); all new commits are bot-authored and
bot-committed. Neither push nor pull-request creation succeeded. No alternative delivery path
was attempted. The current source is retained on local branch `feat/confined-tests-run`.

## Resume

The Gates maintainer or policy owner must resolve authority for imported histories. Then run
the normal `b10x-gates check` and `b10x-gates publish` path, create the bot PR, and verify the
published head. Do not rewrite imported history, weaken the check, or use another push client.
