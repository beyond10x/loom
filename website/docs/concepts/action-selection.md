---
title: Action selection and argument generation
sidebar_label: Action selection
sidebar_position: 2
description: Why Loom separates which action to take from how to fill in its arguments, and the safety rules every selector follows.
status: shipped
lede: Choosing which admissible action to take is often a narrow classification problem. Constructing that action's arguments needs the full context and a stronger model. Loom keeps the two apart, so routine routing can use a cheaper, faster decision while argument generation stays with the reasoning model.
source: docs/contracts/loom-action-selection.md
source_url: https://github.com/beyond10x/loom/blob/main/docs/contracts/loom-action-selection.md
---

## Selector and argument generator

**`ActionSelector` picks one action.** Given the run's context and the candidate actions, it returns
one action id and, optionally, a confidence. Two strategies exist: the slice's `ModelSelector`, a
model asked for one forced `select_action` tool call, and scripted rule selectors in tests and the
SDK example. A fast typed selector is planned.

**`ArgumentGenerator` fills in one action.** Given the selected action, it returns that action's
arguments as JSON. It does not see the choice between actions and completes only the one already
chosen.

A selector is given the selection context and the projected catalogue's entries and returns one
choice, which Loom refuses when the catalogue does not list it.

A generator is given the argument context and the one catalogue entry the selection names, not the
rest of the catalogue, and returns a JSON value that becomes the proposed action's arguments.
Before the generator is called, Loom records the argument request against the selection it serves.
The candidate descriptors in the contract are the design the catalogue entries grow into.

A host that hands Loom the argument schemas of the protocol it runs (`Loom::with_argument_schemas`,
built from `ProtocolEntry::argument_schemas`) gets the generated arguments checked against the
schema the protocol declares for the action, with Canon's `check_arguments`, before revalidation.
An action whose protocol declares no schema takes only `{}`. Arguments that do not match are
recorded `Mismatched` on their argument request, naming the action, the JSON pointer and the
keyword, and the step proposes nothing. The schema comes from the loaded protocol alone, never from
the model. The plugin host's turns check their arguments this way.

## Safety rules

These hold for every selector, fast or slow, local or hosted, and are stated in the repository's
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
7. Selection telemetry is available to the embedding host and to trials for evaluation (Atlas
   ADR 0101), and is never evidence.

Rules 1 to 4 hold today: the catalogue is projected from the frontier of each step, and Loom puts
every selection to Commission's admission check. A selection it refuses, such as an action outside the
frontier, is not proposed, whatever confidence the selector reported. The Commission runtime
revalidates every proposal before the effect port sees it.

Rule 7 holds too: Loom writes one `SelectionRecord` per selection, with its strategy, candidate
count, chosen action, confidence, latency and the tokens the selector reported. On an overruled
fast selection the record adds the strategy it fell back to, and a selection refused at the
execution boundary adds no record. Rule 5 holds through `HybridSelector`, at a threshold the host supplies,
and rule 6 is design.

## Beyond tool selection

The same fast typed decisions may later route between model tiers, decide whether a step needs deep
reasoning or human review, pick a compaction strategy, or route between specialist sub-agents. These
are optimizations inside an envelope that is already governed.
