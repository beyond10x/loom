---
format: aep.planning-md/3
id: story:next-decision-names-the-principal
kind: story
status: proposed
title: A printed decision names the principal that answers it
summary: 'NextDecision gains optional principal: the --principal its command carries (ApproveAction, Approve); absent for Close and Handoff; refusal of another principal unchanged (approve.rs:238-243, person.rs:65-71)'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T10:29:39Z", actor: "agent:loom", revision: 2}
---
## Outcome

A decision that a person answers names that person, so an engine can check who answers before it
records the answer. Cites are at `ec1a82f9`; the cites added in this revision are at `b9ec2657`.

- **The new member.** `loom.tree.NextDecision` (`ess/domains/tree.yaml:47-69`,
  `generated/rust/loom/src/tree.rs:101-124`) gains `principal: Optional<String>`. It is present
  on exactly the decisions whose `command` carries `--principal`, and it holds the same value:
  - **`ApproveAction`: a person action.** The value is the binding's `executor.principal`, and it
    names the identity Loom checks. A person executor must name a principal
    (`crates/loom-bindings/src/decode.rs:466-472`). `principal` is a person member only
    (`crates/loom-bindings/src/load.rs:241`).
  - **`Approve`: a capability approval of a program action.** A program binding has no principal,
    so the value is always `DEFAULT_PRINCIPAL` (`"operator"`, `crates/loom-tree/src/run.rs:70`).
    It is the principal the printed command records, and it is not a restriction: no check
    compares an approval's principal with a binding. `waits` computes the value
    (`run.rs:927-929`) and today puts it only into the argv (`:942`).
  - **`Close` (`:963-988`) and `Handoff` (`handoffs`, `:846-890`):** absent. Neither command names
    a principal.
- **Values are not special-cased here.** A principal such as `decider:model` is a principal like
  any other string in the tree path: named as declared and accepted as today. `git grep decider
  -- crates/loom-tree crates/loom-cli` finds none. The decision crate gives the prefix a meaning
  of its own (`crates/loom-decision/src/apply.rs:172-178`), outside this path.
- **The printed object.** `decision_json` (`crates/loom-tree/src/encode.rs:188-221`) writes
  `principal` after `capability` when present, and writes no member when absent, as it does for
  every optional member through `put`. Every other member keeps its bytes and its order.
- **The library type.** `TreeRun.decisions`, which `run_tree` returns, carries the same value.
  - The new field changes the generated struct. A consumer that builds a `NextDecision` with a
    struct literal must add the field.
  - Generated model types are outside the SDK promise
    (`website/docs/reference/sdk-stability.md:33-35`).
    `b10x_loom_tree::model` is on that page only once D1 (a) of
    `story:tree-library-exports-what-an-engine-needs` adds `loom_sdk::tree`, and then it is
    *generated*.
  - `CHANGELOG.md` names the member under `Added`, and adds a `**Breaking:**` bullet for the
    struct literal.
- **`tree status` and the run log do not change.** `tree status` has no decision object. Its
  `stop_next` (`ess/domains/tree.yaml:283-284`) is the list of `decides` sentences of the case's
  last `stopped` line (`run.rs:387-391`, read by `last_stop`,
  `crates/loom-tree/src/status.rs:154-166`).
- **Who answers is checked where it is today.** This holds for `ApproveAction` only. A person
  action answered by another principal is refused at two places:
  - `person_action` (`crates/loom-tree/src/approve.rs:218`, check at `:238-243`) refuses
    `NotAPersonAction`, naming the bound person, before anything is written;
  - the person port (`crates/loom-tree/src/person.rs:65-71`) refuses the effect, as a second
    guard.

  This story changes neither. The new member only lets an engine refuse earlier, on its own side.

Security-relevant: yes. For `ApproveAction`, the member names the identity that may answer. It
must equal the principal the command records and Loom checks, or an engine would check against a
different party than Loom does. The unit gets a security review.

## Why

Runbook's in-process engine links `b10x-loom-tree` and calls `run_tree`, `approve`, `close` and
`tree_status`. It answers decisions itself and must check the answering principal against the
binding's declared principal. Today that principal is only inside `command`, an argv Runbook runs
as data and never parses, and `decides` is never parsed either. Runbook treats an unknown member
as optional, so an added optional member is safe for it.

## ESS first

`ess/domains/tree.yaml`: `loom.tree.NextDecision` gains `principal: Optional<String>`, after
`capability`. Its comment (`:41-46`) gains these words: "`principal`: the principal the printed
command records; for `ApproveAction` the bound person Loom checks, for `Approve` the default and no
restriction; absent for `Close` and `Handoff`". Then `task generate` and `task drift`. On the ESS
commit, `generated/rust/loom/src/tree.rs` and the `run.rs` constructors do not compile until the
field is set.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. Fixture programs are POSIX sh, not Python (§ No Python for permanent tools). The CLI tests
use the shared tree fixture of `crates/loom-cli/tests/tree_run.rs` (`command()` at `:118-127`) and
copy its store, rather than run a whole tree per test.

- **A person action** (`tests/tree_run.rs`). A tree waiting on a person action prints an
  `ApproveAction` decision whose `principal` equals the binding's `executor.principal` and the
  `--principal` value in its `command`. Test: `a_person_decision_names_its_principal`.
- **The library carries it** (`crates/loom-tree/tests/main.rs`). `run_tree` on the same fixture
  returns `TreeRun.decisions` whose `ApproveAction` entry holds the same `principal`. Test:
  `run_tree_returns_the_principal_of_a_person_decision`.
- **The round trip** (`tests/tree_run.rs`).
  - Running the printed `command` of an `ApproveAction` with a capability, unchanged, exits 0. The
    `AuthorityRecorded` line it writes (`ess/domains/runlog.yaml:153-161`) has `principal` equal
    to the printed one, and the person record's producer is the same principal.
  - The same `command` with `--principal` changed to another value exits 1, refused
    `NotAPersonAction`, and the store is byte for byte unchanged.

  Test: `the_printed_principal_is_the_one_recorded_and_no_other_is_accepted`.
- **A capability approval** (`tests/tree_run.rs`). An `Approve` decision of a program action names
  `operator`, equal to its `command`. Test: `an_approval_names_the_principal_it_records`.
- **Kinds with no principal** (`tests/tree_run.rs`). A `Close` decision and a `Handoff` decision
  hold no `principal` member. Test: `close_and_handoff_name_no_principal`.
- **Old bytes, at the encoder** (`crates/loom-tree/src/encode.rs`, unit test). `decision_json` of
  a hand-built `NextDecision { principal: None, .. }` of each kind equals fixed bytes kept in the
  test, which are today's print of the same value. So a reader of the old shape reads every member
  it knew. Test: `a_decision_without_a_principal_encodes_as_before`.
- **A model decider** (`tests/tree_run.rs`). A binding whose person principal is `decider:model`
  prints that value, and `tree approve --principal decider:model --action A` is accepted. Test:
  `a_decider_principal_is_named_and_accepted`.
- **Status and log unchanged** (`tests/tree_run.rs`). After the person-action wait, `tree status`
  and the case's run log are byte for byte equal to those of the same fixture before this change,
  kept as fixed bytes in the test. Test: `status_and_run_log_do_not_change`.
- **Generated code.** `slot task drift` exits 0 after `task generate`; on the ESS commit it fails.
- **CHANGELOG.** The `[Unreleased]` entry holds an `Added` bullet naming
  `NextDecision.principal` and a `**Breaking:**` bullet naming the struct literal. Checked by
  `loom-xtask release-check` on the tag.

## Not covered

- Changing who may answer, or any check of the principal beyond today's two.
- A decision object in `tree status` or in the run log.
- Open decision requests and their ids: `story:tree-waits-open-decision-requests`.

## Scope

- Cited:
  - `ess/domains/tree.yaml:41-69`, `:283-284`, `ess/domains/runlog.yaml:153-161` and
    `generated/rust/loom/src/tree.rs:101-124`;
  - `crates/loom-tree/src/run.rs`: `:70`, `handoffs` `:846-890`, `waits` `:894-1005`
    (`:927-929`, `:942`), `:366-374`, `:387-391`;
  - `crates/loom-tree/src/encode.rs:188-221`, with its unit test;
  - `crates/loom-tree/src/approve.rs:218-243` and `crates/loom-tree/src/person.rs:65-71`;
  - `crates/loom-bindings/src/load.rs:241`, `crates/loom-bindings/src/decode.rs:466-472`;
  - `crates/loom-cli/tests/tree_run.rs` and `crates/loom-tree/tests/main.rs`.
- Inferred: `generated/ess/loom/` and the reference page of `loom.tree`, regenerated;
  `CHANGELOG.md`.

Edges:

- depends_on `story:tree-run-command`. No depends_on `story:handoff-when-a-program-decides`.
- **Order** on `ess/domains/tree.yaml`, the generated model files and `run.rs` (`handoffs`
  `:846-890`): `story:child-record-in-the-run-log`, then `story:tree-status-names-what-it-read`,
  then `story:handoff-when-a-program-decides`, then `story:program-reports-its-cost`, then this
  story. The later unit merges the earlier head.
- `story:tree-status-names-what-it-read` also shares `encode.rs:188-221` and `run.rs:366-374`
  beside `:387-391`.
- `story:tree-run-tests-fit-the-budget` shares the fixture of `tests/tree_run.rs` (`:118-127`)
  and `tests/support/loom.rs`.
- `story:tree-waits-open-decision-requests` shares `waits` and `tests/tree_run.rs`.
