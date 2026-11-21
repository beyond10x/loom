---
format: aep.planning-md/3
id: story:suspension-stores-agree-on-a-misnamed-hold
kind: story
status: draft
title: Both Suspension stores refuse a hold whose identity names another commission
summary: SuspensionStore holds what FileSuspensionStore refuses storage-failed; found in wave 2026-11-21-w5, older than it
tags:
- from-finding
relations:
- depends_on: story:ess-096-upgrade
- informed_by: review-result:ess-096-upgrade-security-2
- serves: vision:O1
revision: 1
---
## Outcome

A `HoldSuspension` whose `suspension.commission_id` names another commission than its
`commission_id` gets the same answer from `SuspensionStore` and from `FileSuspensionStore`, and
neither stores it.

## Found

Found 2026-11-21 in wave 2026-11-21-w5 by the correction after the first security review of
`story:ess-096-upgrade`, and read again by the second pass
(`review-result:ess-096-upgrade-security-2`, point 3). Older than that wave: the same code is at
`29e5d199`.

- `FileSuspensionStore::hold` refuses such a hold `StorageFailed` ("the Suspension's identity
  names another commission than its `commission_id`") before it reads a file
  (`crates/loom-governor/src/file_suspension.rs:219-226` at `1210f456`).
- `SuspensionStore::hold` has no such check: it decides over, and stores under,
  `input.suspension.commission_id` (`crates/loom-commission/src/ports/suspension.rs:418-423`).
  A hold with all four request members or with none is held there.
- `crates/loom-commission-testkit/tests/hold_incomplete_request.rs:507`
  (`a_misnamed_hold_with_all_four_or_none_is_held_in_memory_and_refused_by_the_file_store`)
  asserts the difference as it is.
- Since wave 2026-11-21-w5 a mixed hold is `IncompleteRequest` on both stores whatever its
  identity; that half is settled.
- Nothing found that sends such a hold: the runtime builds both ids from one
  (`crates/loom-commission/src/runtime.rs:2427-2429`).

## Acceptance

- The case at `hold_incomplete_request.rs:507` is replaced by one that expects, for each reason
  and for all four members and none, the same refusal from both stores, `SuspensionPort::get`
  returning nothing for either commission and no file written.
- The story names which store changes and the outcome both give. If the specification
  (`ess/commission/domains/responsibility.yaml`, `HoldSuspension`) can declare the refusal, the
  unit's first commit declares it there; if it stays `storage-failed`, the story says why.
- `task commission:conform` passes every scenario and `ess/commission/SKIPPED.md` lists none.
- A `**Breaking:**` CHANGELOG bullet if `SuspensionStore::hold` refuses an input it held.

## Out of scope

- The order of `incomplete-request` before the identity check (`story:ess-096-upgrade`).
- Any other port's check of an identity against its command's ids.
