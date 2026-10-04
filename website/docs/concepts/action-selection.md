---
title: Action selection and argument generation
sidebar_label: Action selection
sidebar_position: 2
description: Why Loom separates which action to take from how to fill in its arguments, and the safety rules every selector follows.
---

# Action selection and argument generation

Choosing which admissible action to take is often a narrow classification problem. Constructing
that action's arguments needs the full context and a stronger model. Loom keeps the two apart, so
routine routing can use a cheaper, faster decision while argument generation stays with the
reasoning model.

## Two roles

**`ActionSelector` picks one action.** Given the run's context and the candidate actions, it returns
one action id and, optionally, a confidence. Planned strategies: a reasoning-model selector, a fast
typed selector, a rule selector, and a hybrid.

**`ArgumentGenerator` fills in one action.** Given the selected action, it returns that action's
arguments as JSON. It never sees the choice between actions; it only completes the one already
chosen.

A selector is given the selection context and the projected catalogue's entries, and returns one
choice; Loom refuses a choice the catalogue does not list. A generator is given the argument context
and the one catalogue entry the selection names, never the rest of the catalogue, and returns a
JSON value, which becomes the proposed action's arguments. Before the generator is called, Loom
records the argument request against the selection it serves. The candidate descriptors in the
contract are the design the catalogue entries grow into.

## Safety rules

These hold for every selector, fast or slow, local or hosted. They are stated in the repository's
action-selection contract,
[`docs/contracts/loom-action-selection.md`](https://github.com/beyond10x/loom/blob/main/docs/contracts/loom-action-selection.md).

1. Candidate labels originate from the **current frontier**.
2. **Unknown action ids are rejected.** A selector cannot produce an action the frontier does not
   contain.
3. **Selection confidence never grants authority.** A confident choice is still only a choice among
   actions that were already admissible.
4. A selected action is **revalidated before any effect**.
5. **Low confidence falls back** to a stronger path.
6. For catalogues larger than about 20 actions, a selector may choose a **family first, then an
   action** within it.
7. Selection telemetry should be available to **Metaharness** for evaluation.

Rule 2 is enforced today: Loom puts every selection to Commission's admission check, and
one it refuses, such as an action outside the frontier, ends the run with `NoUsefulAction`
whatever the selector reported. The others are design.

## Beyond tool selection

The same fast typed decisions may later route between model tiers, decide whether a step needs deep
reasoning or human review, pick a compaction strategy, or route between specialist sub-agents. These
are optimizations inside an envelope that is already governed.
