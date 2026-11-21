---
format: aep.planning-md/3
id: story:executable-allow-list
kind: story
status: draft
title: Loom starts no process whose program the engine's allow-list and deny-list do not admit
summary: Checked at bindings load and before each start; descendants held where Substrate proves it
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
revision: 2
---
## Outcome

The engine configuration holds an allow-list and a deny-list of executables, and Loom starts no
process whose program is not admitted. A program is named by the absolute path it resolves to; a
list entry is a path or a directory. The check runs when a bindings file is loaded (every
`argv[0]` and `arguments_from[0]` must resolve, be allowed and not be denied, or the file is
refused naming the action and the program) and again immediately before each start, on the path
actually executed. A binding never extends the lists. What a started program may itself execute
is Substrate's to enforce, and this story passes the same lists to Substrate where the opened
adapter proves the fact; where it does not, the step's record says descendants were not held to
the list.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "a process allow-list and
deny-list (which executables may run at all)". Loom runs any argv a bindings file names
(`crates/loom-intake-slice/src/bindings.rs:182-195` refuses only an empty program). Substrate
0.21.0 has an executable allow-list only on `macos` (`exec_subpaths`,
`crates/substrate-host/src/confinement/macos.rs:3-35`); `linux` refuses a profile that uses it
(`linux.rs:29-31`), its seccomp filter covers `socket(2)` only, and no adapter has a deny-list.
So Loom can hold the program it starts to the lists today, and descendants only when Substrate
ships the control.

## ESS first

`ess/domains/tree.yaml`: `ExecutablePolicy` (allow, deny) in the engine configuration, the
refusal `ProgramNotAdmitted` (action, program, the list entry that decided), and
`descendants_held` on the step record. On that commit `task drift` fails and
`crates/loom-bindings/tests/executable_policy.rs` does not compile.

## Acceptance

- A bindings file whose `argv[0]` resolves outside the allow-list is refused at load
  `ProgramNotAdmitted`; one inside it loads; one inside an allowed directory but named on the
  deny-list is refused, naming the deny entry.
- A relative `argv[0]` is resolved against the workspace and the engine's `PATH`, and the
  resolved path is what is checked and what is started.
- A program replaced by a symbolic link to a denied program between load and start is not
  started.
- Loom never hands Substrate a deny-list alone: an engine configuration with a deny-list and no
  allow-list is refused when it is loaded, naming the rule (agreed with session `substrate`,
  2026-11-21).
- With an empty allow-list nothing starts; with no policy configured the behaviour is today's and
  the step record says no policy applied.
- Under an adapter that proves the executable fact, a bound allowed program that tries to exec a
  denied one fails; under one that does not, it succeeds and the record says
  `descendants_held: false`.
- `task check` exits 0.

## Out of scope

The control inside Substrate for `linux`, `oci` and `pod`
(`upstream-blocker:substrate-executable-allow-list`). Checking a program by content digest.
