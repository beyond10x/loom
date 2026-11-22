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
- depends_on: story:handoff-when-a-program-decides
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
  path: crates/loom-tree/src/engine.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:57Z", actor: "agent:loom", revision: 5}
---
## Outcome

A bound program that calls a model gets its credential without it ever being written down.

- The engine configuration gains a section `secret_slots`, a list of `{name, path}`. Each `name`
  follows Substrate's grammar `[a-z][a-z0-9_]{0,63}`, and `path` is an owner-private file. Loom
  hands the list to Substrate as `HostConfig.secret_slots` (Substrate's `SecretSlot`,
  `crates/substrate-host/src/lib.rs:205-216` at `adfde402`). The path stays in that struct.
- A program binding that selects an aperture (`story:named-egress-per-action`) may name one slot:
  `secret: <name>`. The member is one string, so a binding cannot name a second slot.
- At launch, the runner asks for the slot at descriptor 3 (`SecretSlotRequest {slot, fd: 3}`),
  and Substrate sets `SUBSTRATE_SECRET_SLOTS=<name>=3` (`SECRET_SLOTS_ENV`, wire `lib.rs:374`,
  `:4965-4986`). Loom never opens the slot's file; Substrate reads it at each launch
  (`crates/substrate-host/src/secrets.rs:187-219` at `adfde402`), which keeps "Never read a
  credential file in Loom code or tests" (`AGENTS.md` § Never, `:2838`).
- The secret is never in the bindings document, a record, an observation, a blob Loom writes, a
  run-log line, or Loom's standard output or standard error, error text included. The slot's name
  appears only in the bindings document and in Substrate's applied record. No member is added to
  `StepConfinement`; that type stays `story:named-egress-per-action`'s.
- Refused at load (`tree open`, `tree run`), naming the action, with nothing written:
  - in the document, a slot without an aperture: `BindingsRefusalReason` `SecretWithoutEgress`;
  - against the engine configuration, in `crates/loom-tree/src/engine.rs` beside the `Aperture*`
    checks of `story:named-egress-per-action`: `EngineRefusal` `SecretNameInvalid` for a name
    outside the grammar and `SecretUndeclared` for a name the engine does not declare.
- Refused at the step, before the program starts, naming the slot and never the path:
  `ConfinementRefusal` `SecretSlotUnreadable`, carrying Substrate's reason from
  `exec.secret-slot-unreadable`. That covers a slot file that is missing ("open refused"), one that
  is not a regular file, and one that is not owner-private (`secrets.rs:194-208`).
- Under `--confinement local`, a slot is always refused `SecretUnserved` before any step, with no
  opt-in: the local adapter refuses any slot (`story:agent-session-through-substrate`, :54-56).
- Today an applied record naming any slot is refused `CapabilityUnserved` (`!secret_slots
  .is_empty()`, `crates/loom-intake-slice/src/confinement.rs:2961`). After this story the record
  must name exactly the slots requested; any other record is refused as today.

Security-relevant: yes: a credential crosses the confinement boundary. Security review.

## Why

A bound program has no network today. `ChildCommand::new` sets `NetworkMode::None`
(`confinement.rs:1219`), and `bound_program_child` never selects egress. Once
`story:named-egress-per-action` lets a binding reach a model host, the program still lacks the
credential for it. `story:bound-program-declares-its-environment` refuses variable names carrying
`KEY`, `TOKEN` or `SECRET`, and an environment value is in the document and the record. Substrate's
secret slots (ADR 0012) carry the material as a sealed `memfd` and keep its path out of every
request, fact and error.

## Open decision

D1, Runbook's login as a file path in its bindings copy:

- (a) **Default.** Refused: a credential reaches a program only through a declared slot, never
  under `local`.
- (b) The operator declares the login file as a read-only root for that action
  (`story:bound-program-declares-its-environment`). (c) Both, with (b) only on `local`.
- (b) and (c) conflict with `AGENTS.md` § Never (`:2838`) and with the credential-location
  refusals of `story:bound-program-declares-its-environment`. They need Timo's word and a change
  of that rule. Until then this story builds (a) only.

## ESS first

`ess/` changes first, in its own commit, then `task drift`:
- `ess/domains/bindings.yaml`: `BoundAction` gains `secret: Optional<String>`, and
  `BindingsRefusalReason` gains `SecretWithoutEgress`.
- `ess/domains/engine.yaml` of `story:named-egress-per-action`: the engine configuration gains
  `secret_slots`, and `EngineRefusal` gains `SecretNameInvalid` and `SecretUndeclared`.
- `ConfinementRefusal` gains `SecretUnserved` and `SecretSlotUnreadable`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call.

The slot's file is a test fixture holding a known random value, which only Substrate reads. The
programs are fixture scripts in POSIX sh, not Python (§ No Python for permanent tools), and every
brief of the unit names that rule.

The tests that need a delegated cgroup skip without one only when `LOOM_REQUIRE_DELEGATION` is
unset. The unit's gate sets it and runs them under `systemd-run --user -p Delegate=yes --scope`.
The unit's report lists the tests run and the tests skipped, and a skip counts as red there.

- **Load refusals**, each printed with its reason and action, nothing written: no aperture
  (`a_secret_without_egress_is_refused_at_load`); a name outside the grammar
  (`a_secret_name_outside_the_grammar_is_refused`); an undeclared name
  (`an_undeclared_secret_is_refused`).
- **The secret arrives.** A confined program prints the SHA-256 of what it read from descriptor
  3, equal to the fixture's. Test: `a_confined_program_reads_its_secret_from_descriptor_3`.
- **Unreadable slot files.** A slot file that is missing, one that is a directory, and one with
  mode `0640` are each refused `SecretSlotUnreadable` before the program starts. The refusal names
  the slot and Substrate's reason, and its text holds no part of the path. Tests:
  `a_missing_secret_file_is_refused`, `a_secret_path_that_is_no_file_is_refused` and
  `a_secret_file_that_is_not_owner_private_is_refused`.
- **No leak.** After the successful run and after each refused one, nothing holds the secret's
  bytes or its file path: no file under the store, no run-log line, no blob, and no captured
  standard output or standard error of `b10x-loom`, error text included. The test scans every
  byte. Test: `the_secret_appears_in_no_store_file_log_blob_or_output`.
- **Local.** Under `--confinement local` the run is refused `SecretUnserved` before any step.
  Test: `a_secret_under_local_is_always_refused`.
- **Applied record**, unit tests on the check at `confinement.rs:2938-2965`, each refused
  `CapabilityUnserved`: another slot than the one requested
  (`an_applied_record_with_another_secret_slot_is_refused`); a slot when none was requested
  (`an_applied_record_with_an_unrequested_slot_is_refused`); none when one was requested
  (`an_applied_record_missing_the_requested_slot_is_refused`).

## Not covered

- Which destinations a program reaches: `story:named-egress-per-action`.
- Non-credential variables and paths: `story:bound-program-declares-its-environment`.
- Rotation, expiry or a vault behind the slot file; agent executors; D1 (b) and (c).

## Scope

- `crates/loom-intake-slice/src/confinement.rs`, cited: `:1219`; the wire projection
  `:1678-1694` (`secret_slots: Vec::new()` at `:1693`); the applied-record check `:2938-2965`;
  `driver_config` `:342-356`.
- `crates/loom-intake-slice/src/bindings.rs:414-448` (`bound_program_child`). Cited.
- `crates/loom-bindings/src/decode.rs`: the `ACTION` members `:22-34`. Cited.
- `crates/loom-tree/src/engine.rs` and `ess/domains/engine.yaml`, created by
  `story:named-egress-per-action`. Inferred.
- `ess/domains/bindings.yaml`: `BoundAction` `:199-223`, `BindingsRefusalReason`. Cited.

Edges:

- depends_on `story:named-egress-per-action`, which creates `engine.rs` and `EngineRefusal`.
- depends_on `story:handoff-when-a-program-decides`: it shares the `ACTION` list
  (`decode.rs:22-34`) and `BoundAction` with that story and `story:program-reports-its-cost`.
- `confinement.rs` is shared with `story:tree-run-takes-a-delegated-cgroup`,
  `story:concurrent-confined-runs` and `story:drop-the-substrate-git-feature` (`driver_config`).
  The later unit merges the earlier head.
- Neighbours: `story:bound-program-declares-its-environment` and `story:mixed-egress-launches`.
- Waits for Timo; not activated.
