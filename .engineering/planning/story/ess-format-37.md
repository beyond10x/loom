---
format: aep.planning-md/3
id: story:ess-format-37
kind: story
status: implemented
title: Loom's main ESS system declares ess/37 on the pinned ess 0.96.0, no generated file changed
summary: ess/system.yaml from ess/31 to ess/37 in two commits; compiled model, suite and generated files keep their bytes against the parent commit
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:ess-096-upgrade
scope:
- confidence: cited
  path: ess/system.yaml
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T11:15:51Z", actor: "agent:sdc", revision: 7, decided_on: {"recorded":{"review_outcome":3}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T11:16:57Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":3}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
- {from: "active", to: "implemented", at: "2026-11-21T12:56:31Z", actor: "agent:sdc", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
---
## Outcome

Loom's main ESS system, `ess/system.yaml`, declares `format: ess/37` instead of `ess/31`, on the
toolchain Loom already pins (`requires: ess 0.96.0`, `ess/ess-inputs.yaml:2`; CI installs the same
release, `.github/workflows/check.yml:60-61` and `.github/workflows/pages.yml:35-36`). No command's
outcome, no stored row and no generated file changes. `ess/intake/system.yaml` (`ess/26`) and
`ess/commission/system.yaml` (`ess/22`) stay as they are, and no ESS toolchain moves.

The story has two commits: the first changes only `ess/system.yaml` (one line), the second only
`CHANGELOG.md` (one `### Changed` bullet).

What the raise changes, read from a probe of the raise on `9b4170fb` with `ess` 0.96.0
(`ess/system.yaml` edited, nothing else):

- `ess specify compile --path ess --format json` writes the same bytes at `ess/31` and at
  `ess/37`, and `ess verify conform synthesize` writes the same suite (same sha256): 181
  scenarios, 0 authored, 0 refusals, `ess-conformance/34`, the same `spec_digest` and
  `contract_digest`, the same seven notes.
- `ess specify validate --path ess --strict-requires` prints the same line at both formats,
  `loom v1 — 16 file(s), 97 scenario(s), valid`, with no warning.
- `task generate`, `task docs-generate` and `task crd-generate` rewrite 77 files
  (`generated/rust/loom/` 23, `website/docs/reference/` 27, `website/data/ess/` 23,
  `docs/contracts/` 2, `deploy/crd/` 1, `charts/loom-agents/crds/` 1), and `git status --short`
  afterwards names `ess/system.yaml` only: no generated file carries the format, so none changes.
- `task spec`, `task ess-gate`, `task drift`, `task crd-drift`, `task no-hand-model`,
  `task conform` and `task docs-check` exit 0.

These numbers are what the probe measured on `9b4170fb`. They move when a story of the same wave
adds scenarios (`story:boundary-admits-declared-http-egress` adds authored scenarios and refusal
reasons to the main system), so the acceptance below states the suite, the compiled model and the
validate line relative to the parent of the story's first commit.

Two format rules of `ess/32` to `ess/37` apply to a document that opens no new construct, and
neither reaches a Loom declaration: from `ess/32` two variants of one enum spelled alike on the
wire are refused (validation stays green), and from `ess/34` a named `when:` or `when_related:`
refusal of a command addressing an existing row observes that row completely (the suite keeps
its bytes). Every other rule of the six formats is a construct Loom does not declare.

After the change, `ess/31` stays named as a reason in six places outside `generated/` and
`.engineering/`
(`AGENTS.md:1754`, `CHANGELOG.md:36`, `crates/loom-resource/tests/adversary_w20261121w4_u2.rs:2`,
`crates/loom-resource/tests/record.rs:420`, `crates/loom-session-executor/src/lib.rs:2193`,
`ess/domains/resource.yaml:500`). Each says what `ess/31` allows or what the `ess/31` raise did,
which stays true at `ess/37`, so none is changed. No test asserts a format string of the main
system.

## Why

`story:decision-request-entity` copies SDC's decision domain (`~/beyond10x/sdc/ess/`,
`format: ess/37`), which declares `ordered: true` on `Strength` and `AutonomyLevel`
(`~/beyond10x/sdc/ess/domains/decision.yaml:109` and `:171`). `ess` 0.96.0 accepts the key from
`ess/36` and refuses it at `ess/31` and `ess/35` as `unsupported_format_version` naming `ess/36`
(the probe: the two declarations copied into a scratch domain of Loom's main system validate at
`ess/36` and `ess/37`; the format history entry `ess/36` in `~/beyond10x/ess`, tag `0.96.0`,
`website/docs/reference/spec-versions.md:78`). Session `sdc` decided that a format story first
raises the main system from `ess/31` to `ess/37`, so that the entity story can copy the decision
domain with `ordered: true` as SDC writes it. `ess/37` is the format the SDC file declares and
the newest 0.96.0 reads; it opens no construct of its own (`spec-versions.md:79`).

## Waits on

Nothing in this epic. `story:decision-request-entity` waits on this story. It shares
`CHANGELOG.md` with the two other stories of wave 2026-11-21-w6 and no other file.

## ESS first

`ess/system.yaml`: `format: ess/31` becomes `format: ess/37`; no other declaration in `ess/`
changes. Exempt from a red test (AGENTS.md § ESS, no behaviour change): the raise changes no
behaviour, the compiled model and the synthesized suite keep their bytes, and `task drift` stays
green on the first commit. The first commit changes only `ess/` (`ess/system.yaml`, one line), as
AGENTS.md § ESS requires; the second changes only `CHANGELOG.md`.

## Acceptance

- `ess/system.yaml` declares `format: ess/37`; `ess/intake/system.yaml` still declares `ess/26`,
  `ess/commission/system.yaml` `ess/22`, and `ess/ess-inputs.yaml` `requires: ess 0.96.0`.
- The story's first commit changes only `ess/system.yaml` (one line) and its second only
  `CHANGELOG.md`; `git diff --stat` of the two names nothing under `generated/`, `website/`,
  `deploy/`, `charts/` or `docs/`.
- `ess verify conform synthesize --path ess --out S` on the parent of the first commit, then
  `ess verify conform synthesize --path ess --out S --check` on the first commit, exits 0 (the
  suite keeps its bytes), and both runs report the same `N scenario(s) (… authored), 0 refusal(s)`
  line.
- `ess specify compile --path ess --format json` has the same sha256 on the parent of the first
  commit and on the first commit.
- `ess specify validate --path ess --strict-requires` prints the same line on the parent of the
  first commit and on the first commit, with no warning.
- After `task generate`, `task docs-generate` and `task crd-generate`, `git status --short` is
  empty; `task drift`, `task crd-drift`, `task docs-check`, `task ess-gate` and `task conform`
  exit 0.
- `cargo test -p b10x-loom-conformance --locked` and
  `cargo test -p b10x-loom-executor --test ess_gate --test adversary2_ess_gate --locked` exit 0
  with no assertion edited.
- `CHANGELOG.md` under `[Unreleased]` has one `### Changed` bullet naming the raise of
  `ess/system.yaml` to `ess/37` and this story, and no `**Breaking:**` bullet for it.
- `git grep -n "format: ess/31" -- ess/` prints nothing.
- `task check` exits 0.

## Out of scope

`ess/intake/system.yaml` and `ess/commission/system.yaml` and their formats. Any change of the
ESS toolchain (`ess` 0.96.0 stays). The copy of the decision domain, `ordered:` included
(`story:decision-request-entity`). Any use of a construct `ess/32` to `ess/37` open (a
`when_related:` beside a row set, `fields: []`, a state's `wire:` or `summary:`, a transition's
`summary:`, `provided:`, a struct, map or `Json` input `example:`, a compensating `deletes:`,
`repeated: last_wins`, an expression or Duration measure, `ordered:`). Rewording the six places
that name `ess/31` as a reason. Any change of a command's outcome.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w6 from the unit's commits (`git diff --stat f51c2141 d3fa9878`: 2 files, 2 insertions, 1 deletion). Every line is cited from that diff.

- **Primary surface:** `ess/system.yaml` — cited, 1 of the 2 changed files is under `ess/` (the other is `CHANGELOG.md`)
- **Files (ESS sources):** `ess/system.yaml` (1 line changed, 1 insertion and 1 deletion: line 1 `format: ess/31` becomes `format: ess/37`; commit `8b2535c1`) — cited
- **Files (generated output):** none changed — cited; `git diff --numstat f51c2141 d3fa9878` names no path under `generated/`, `website/`, `deploy/`, `charts/` or `docs/`
- **Files (source):** none — cited; no file under `crates/` or `src/`
- **Files (new tests):** none — cited
- **Files (existing tests):** none — cited
- **Files (documents):** `CHANGELOG.md` (1 line added under `[Unreleased]` `### Changed`, at line 42 of the file: the raise of `ess/system.yaml` to `ess/37` on `ess` 0.96.0, naming `story:ess-format-37`; commit `d3fa9878`) — cited
- **Symbols:** none added, changed or removed — cited; the change is a format declaration
- **Not predicted and touched:** none — cited; both changed files are named by the stored Scope (`ess/system.yaml` typed, `CHANGELOG.md` in its Files bullet)
- **Predicted and not touched:** none — cited; the read-only checks the stored Scope names (`ess/ess-inputs.yaml:2`, `ess/intake/system.yaml:1`, `ess/commission/system.yaml:1`) are untouched, as stated
- **Review:** `ls .engineering/planning/review-result | grep ess-format-37` shows `ess-format-37-acceptance-1.md` (needs-revision: suite bytes, one commit), `ess-format-37-acceptance-2.md` (approve), `ess-format-37-design-1.md` (needs-revision: numbers tied to `9b4170fb`, one commit, two sibling stories' reasons) and `ess-format-37-design-2.md` (approve). These are plan-time critic results; no security review was listed for this unit, and the coordinator read both commits (`8b2535c1`, `d3fa9878`)
