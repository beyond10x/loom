---
format: aep.planning-md/3
id: story:bound-program-model-credential-through-a-secret-slot
kind: story
status: proposed
title: A bound program that calls a model gets its credential through an engine-declared secret slot, never through a document, a record or the log
summary: a binding that selects an aperture may name one Substrate secret slot read as a sealed descriptor; slot without aperture refused; local needs the operator's opt-in; D1 login file
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:named-egress-per-action
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-intake-slice/src/bindings.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:57Z", actor: "agent:loom", revision: 5}
---
## Outcome

A bound program that calls a model can be given its credential without the credential ever being
written down.

- The engine configuration gains a section `secret_slots`, a list of `{name, path}`. Each `name`
  follows Substrate's grammar `[a-z][a-z0-9_]{0,63}`, and `path` is an owner-private file. Loom
  hands the list to Substrate as `HostConfig.secret_slots` (Substrate's `SecretSlot`,
  `crates/substrate-host/src/lib.rs:205-216` at `adfde402`). The path stays in that struct.
- A program binding that selects an aperture (`story:named-egress-per-action`) may name one slot:
  `secret: <name>`.
- At launch, the runner asks for the slot at descriptor 3, as Substrate's `SecretSlotRequest
  {slot, fd: 3}`. The child reads the secret from that descriptor. `SUBSTRATE_SECRET_SLOTS=<name>=3`
  is set by Substrate itself (`SECRET_SLOTS_ENV`, wire `lib.rs:374`, `:4965-4986`).
- The secret is never in the bindings document, a record, an observation, a blob Loom writes or a
  run-log line. Only the slot's name appears, and only in the document and the confinement record.
- Refused at load (`tree open`, `tree run`), naming the action:
  - a slot named without an aperture: `SecretWithoutEgress`;
  - a name outside the grammar: `SecretNameInvalid`;
  - a name the engine does not declare: `SecretUndeclared`;
  - more than one slot.
- Under `--confinement local`, a binding with a slot is refused `SecretUnserved` before any step,
  unless the operator opts in through the engine configuration (`local_secrets: allow`). How the
  descriptor is placed under `local` with the opt-in is inferred; the unit checks Substrate's local
  adapter.
- Loom refuses today an applied confinement record that names any secret slot (`!secret_slots
  .is_empty()`, `crates/loom-intake-slice/src/confinement.rs:2961`). After this story the record
  must name exactly the slots requested, and anything else is refused.

Security-relevant: yes. This passes a credential across the confinement boundary. The unit gets a
security review.

## Why

A bound program has no network today. `ChildCommand::new` sets `NetworkMode::None`
(`confinement.rs:1219`), and `bound_program_child` never selects egress. Once
`story:named-egress-per-action` lets a binding reach a model host, the program still lacks the
credential for it:

- `story:bound-program-declares-its-environment` refuses variable names carrying `KEY`, `TOKEN`
  or `SECRET`, and keeps credentials out;
- an environment value is in the document and in the record;
- Substrate's secret slots (ADR 0012) carry the material as a sealed `memfd` and keep its path out
  of every request, fact and error.

## Open decision

D1, Runbook's login as a file path in its bindings copy:

- (a) **Default.** Refused. A credential reaches a program only through a slot the operator
  declares.
- (b) The operator declares the login file as a read-only root for that action, through
  `story:bound-program-declares-its-environment`. This widens what the program can read and is the
  operator's word, never the document's.
- (c) Both, with (b) only on `local`.

## ESS first

`ess/` changes first, in its own commit:

- `ess/domains/bindings.yaml`: `BoundAction` gains `secret: Optional<String>`, and
  `BindingsRefusalReason` gains `SecretWithoutEgress`, `SecretNameInvalid` and `SecretUndeclared`.
- The engine-configuration types of `story:named-egress-per-action` gain `secret_slots` and
  `local_secrets`.
- `ConfinementRefusal` gains `SecretUnserved`.

Then `task drift`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. The slot's file is a test fixture holding a known random value. A `linux` test runs in a
delegated scope or skips, naming what is missing.

- Each load refusal is printed with its reason and action, and nothing is written:
  - a slot without an aperture, test `a_secret_without_egress_is_refused_at_load`;
  - a name outside the grammar, test `a_secret_name_outside_the_grammar_is_refused`;
  - an undeclared name, test `an_undeclared_secret_is_refused`;
  - two slots, test `a_binding_names_at_most_one_secret`.
- A confined program prints the SHA-256 of what it read from descriptor 3. The digest equals the
  fixture's. Test: `a_confined_program_reads_its_secret_from_descriptor_3`.
- After that run, no file under the store, no run-log line, no blob and no stdout or stderr kept
  holds the secret's bytes or its file path. The test scans every byte. Test:
  `the_secret_appears_in_no_store_file_log_or_blob`.
- Under `--confinement local` without the opt-in, the run is refused `SecretUnserved` before any
  step. Test: `a_secret_under_local_is_refused_without_the_opt_in`.
- An applied record that names a slot other than the one requested is refused. Test:
  `an_applied_record_with_another_secret_slot_is_refused`.

## Not covered

- Which destinations a program reaches: `story:named-egress-per-action`.
- Non-credential variables and paths: `story:bound-program-declares-its-environment`.
- Rotation, expiry or a vault behind the slot file.
- Agent executors.
- D1 (b) and (c).

## Scope

- `crates/loom-intake-slice/src/confinement.rs`, cited:
  - `:1219`;
  - the wire projection `:1678-1694` (`secret_slots: Vec::new()` at `:1693`);
  - the applied-record check `:2938-2965`;
  - `driver_config` `:343-360`.
- `crates/loom-intake-slice/src/bindings.rs:414-448` (`bound_program_child`). Cited.
- `crates/loom-bindings/src/decode.rs`: the `ACTION` members `:22-34`. Cited.
- `crates/loom-tree/src/engine.rs`, from `story:named-egress-per-action`. Inferred.
- `ess/domains/bindings.yaml`: `BoundAction` `:199-223`, `BindingsRefusalReason`. Cited.

Edges:

- depends_on `story:named-egress-per-action`.
- Neighbours: `story:bound-program-declares-its-environment` and `story:mixed-egress-launches`.
