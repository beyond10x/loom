---
format: aep.planning-md/3
id: evaluation-plan:ess-hardening-20261006
kind: evaluation-plan
status: draft
title: Bounded ESS hardening of the merged Loom runtime
relations:
- informed_by: story:loom-ess-conformance
- informed_by: story:confined-tests-run
revision: 1
---
## Scope

Bounded ESS hardening of Loom at `04a1a73a7073affb99ef730e8dedb7eabc862794`, requested by the operator on 2026-10-06. Review all three specifications (`ess/`, `ess/commission/`, `ess/intake/`) against their checked-in design and current accepted decisions. Execute mutation tests only against real implementation adapters whose baseline runs green; distinguish synthesis from execution and record every exclusion.

## Procedure

1. Validate and compile the specifications, then run the existing Rust conformance and executor tests.
2. Independently review design/spec correspondence, first proving the review finds a deliberately removed declaration in a scratch copy.
3. Exercise native semantic diff against the 0.1.0 release, with identical-spec and removed-variant controls.
4. Emit specification mutants and run the existing Rust Commission conformance adapter against emitted suites. Record every kill, survivor, unsupported case and unavailable site.
5. Use compiled IR for any additional guard or deterministic probes. Stop techniques that lack a trustworthy implementation target, and identify that limit rather than manufacturing passing evidence.
6. Retain commands, red/green evidence, classified findings and unrun techniques in a verification report. Do not decide open product semantics during this audit.

## Boundary

No release, provider/model calls or change to the earlier live fixture. Existing scope decisions and explicit deferred stories remain authoritative. Running audit tooling is Rust. Only this managed worktree may change; source checkouts belonging to other sessions remain untouched.
