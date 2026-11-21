---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-acceptance-1
kind: review-result
status: active
title: Plan critic (acceptance), round 1, on story:ess-096-upgrade
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
needs-revision

story:ess-096-upgrade — no acceptance bullet would go red if the unit fixed ESS-SYNTH-001 some way other than 1a: declaring a command that creates a `Case` (option 1b) also gives 0 refusals and the named scenarios, and no bullet says `Suspension` keeps only the relations `run` and `action_request`, or that validate warns `implied_relation` on `case_id` — .engineering/planning/story/ess-096-upgrade.md:64
story:ess-096-upgrade — no acceptance bullet checks that 2a runs its member check before `already-held`: both security tests start from an empty store, so only the synthesized scenario's last step covers the order, and the ledger records that step as observed in the probe only, with no bullet naming it — .engineering/planning/story/ess-096-upgrade.md:94
story:ess-096-upgrade — the scenario count `<n>` is the count the unit reads after its own first commit, and the last bullet forbids any later `ess/` change, so the count check cannot fail; pin it at 44 or drop it — .engineering/planning/story/ess-096-upgrade.md:130
story:ess-096-upgrade — a passing `task commission:conform` cannot show that the named scenarios "are selected and pass": cargo hides the test's output on a pass, conform.rs:320 deletes report.json, and a scenario listed in `ess/commission/SKIPPED.md` still passes the gate, so the bullet needs a check such as `ess/commission/SKIPPED.md` listing no scenario — .engineering/planning/story/ess-096-upgrade.md:139
story:ess-096-upgrade — the 2a error name `SuspensionRequestIncomplete` appears only in the CHANGELOG text bullet, and drift, conform and the tests stay green under any error name, so no bullet checks the declared error in `responsibility.yaml` — .engineering/planning/story/ess-096-upgrade.md:155

Read: 1 artifact, the whole body (`aep plan artifact show story:ess-096-upgrade`, `aep plan artifact list`, `aep plan artifact history`), plus the ledger, the probe log and AGENTS.md:231-240 and :2333. I checked these citations against `71f15f92` and they hold: suspension.rs:143-181, responsibility.yaml:1285, :1310-1332, :1719, :1726 and :1745, ess_gate.rs:414 (11 tests), handoff.rs:429-446, store.rs:258-272, file_suspension.rs:207-220, runtime.rs:2433-2451, conformance lib.rs:571-597, and both security tests (the 28 mixed combinations, and which file-store cases are mixed). The pin grep prints 21 lines, the README/AGENTS grep 3, the formats are ess/31, ess/22 and ess/26, and `git diff c94ccbbf..71f15f92 -- ess/commission` is empty. The rest of the acceptance bullets can be checked by command or file. I found no wrong statement about today's code.

Could not establish:
- The store shows `revision 3`, not the 4 you named. The stored body is identical to `.engineering/drafts/ess096-story-body-2.md`, and the ledger calls itself "revision 2 draft". Either the numbering differs or revision 4 was never written.
- Scenario names. Bullet 2 names `Suspension/invariant/after/commission.responsibility.HoldSuspension/held` and bullet 4 names `Suspension/invariant/after/HoldSuspension/held`. The probe suites are in scratch and I did not open them, so I could not tell which form the suite uses.
- Small citation slip, not mine to judge: `SuspensionAlreadyHeld` sits at responsibility.yaml:1519-1522, not :1518-1523 (the range covers it).
- Out of my lane: the scope list leaves out `ess/commission/SKIPPED.md`.

```findings
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 64
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "no acceptance bullet would go red if the unit fixed ESS-SYNTH-001 some way other than 1a: declaring a command that creates a `Case` (option 1b) also gives 0 refusals and the named scenarios, and no bullet says `Suspension` keeps only the relations `run` and `action_request`, or that validate warns `implied_relation` on `case_id`"
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 94
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "no acceptance bullet checks that 2a runs its member check before `already-held`: both security tests start from an empty store, so only the synthesized scenario's last step covers the order, and the ledger records that step as observed in the probe only, with no bullet naming it"
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 130
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the scenario count `<n>` is the count the unit reads after its own first commit, and the last bullet forbids any later `ess/` change, so the count check cannot fail; pin it at 44 or drop it"
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 139
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a passing `task commission:conform` cannot show that the named scenarios \"are selected and pass\": cargo hides the test's output on a pass, conform.rs:320 deletes report.json, and a scenario listed in `ess/commission/SKIPPED.md` still passes the gate, so the bullet needs a check such as `ess/commission/SKIPPED.md` listing no scenario"
- file: .engineering/planning/story/ess-096-upgrade.md
  line: 155
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the 2a error name `SuspensionRequestIncomplete` appears only in the CHANGELOG text bullet, and drift, conform and the tests stay green under any error name, so no bullet checks the declared error in `responsibility.yaml`"
```
