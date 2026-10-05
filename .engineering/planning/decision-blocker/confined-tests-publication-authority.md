---
format: aep.planning-md/3
id: decision-blocker:confined-tests-publication-authority
kind: decision-blocker
status: cleared
title: Publish confinement after Gates resolves imported Commission history
relations:
- blocks: story:confined-tests-run
withholds: artifact
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-05T21:43:48Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"approval":1}}, executor: "agent:codex-confined-tests"}
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

## Resolution

The operator approved adopting Loom's already-shipped migration boundary as the trusted
baseline on 2026-10-05. Only Loom's baseline changed, from `c26ce29` to
`9f06e0bd47eefe50f6c900b0ceca526c841fe8ed`; no other policy field changed. That bot-authored
commit closes the intake import and is an ancestor of release `0.1.0`, whose correctness,
security, documentation and release checks succeeded.

The baseline audit reports zero findings. The post-migration security scan passes all 64
subsequent commits, including the confinement work. The policy update was bot-committed and
published in the private policy repository at `6ad54f04f28c910042b59160126663dea0b0bedf`,
then activated locally. The operator separately authorized one organization policy-secret
update with `gh`, preserving the ESS-specific exception split. Existing gates and source
history remain intact. Publication continues through the normal Gates path.
