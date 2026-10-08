---
format: aep.planning-md/3
id: upstream-blocker:ess-list-membership-predicate
kind: upstream-blocker
status: open
title: ESS has no guard predicate for membership in a List input
relations:
- blocks: story:revalidation-membership-conformance
revision: 1
---
## Waiting on

ESS: a guard predicate true when a subject field is (or is not) an element of a `List` input, for `loom.run.RevalidateSelection` outcome `not-in-frontier` over `frontier_actions: List<String>`, synthesized into scenarios that send a non-empty list. ESS 0.56.0 refuses `action not in input.frontier_actions` and `contains(input.frontier_actions, action)` as ESS-SPEC-012.

## Clears when

An ESS release accepts such a predicate and synthesizes scenarios whose `frontier_actions` decide the outcome.
