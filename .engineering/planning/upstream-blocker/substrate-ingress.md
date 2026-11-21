---
format: aep.planning-md/3
id: upstream-blocker:substrate-ingress
kind: upstream-blocker
status: open
title: Substrate serves no ingress to a confined run
tags:
- sdc
relations:
- blocks: story:named-egress-per-action
revision: 2
---
## What is missing

Ingress to a confined run. Read at Substrate tag `0.21.0` on 2026-11-21: `ingress_routes` and
`rate_bounds` of `ConfinementProfile` are reserved, "any value is
confinement.reserved-field-unserved" (`crates/substrate-host/src/confinement/profile.rs:93-96`),
refused first in the admission order (`mod.rs:548-560`); no adapter serves the `network_ingress`
family (`linux.rs:29`). Substrate's story `inbound-endpoint-for-a-confined-exec` is a draft.

## What it blocks

Only the ingress half of `story:named-egress-per-action`: that story refuses a binding with
`ingress` at load until this clears. Its egress half does not wait.

## What clears it

A Substrate release in which an adapter serves a declared inbound endpoint for a run.

## Who

Session `sdc` sent the request to session `substrate` on 2026-11-21. That session answered the
same day with its stories, both drafts under Substrate's
`epic:runs-are-governed-and-limit-hits-are-events`, uncommitted on its branch
`plan/sdc-run-governance`, neither accepted or implemented:
`story:inbound-endpoint-for-a-confined-exec` (an existing draft) and
`story:confinement-profile-ingress-routes` (new).
