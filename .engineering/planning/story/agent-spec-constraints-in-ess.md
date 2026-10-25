---
format: aep.planning-md/3
id: story:agent-spec-constraints-in-ess
kind: story
status: draft
title: The loom.agent/1 decoder's constraints are declared in ESS so the CRD refuses what the pod refuses
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:loom-agent-resource-domain
revision: 1
---
## Outcome

Every constraint the `loom.agent/1` decoder enforces on a document is declared in
`ess/domains/agent.yaml`, so the CRD projected from the specification refuses the same documents the
pod's `agent check` refuses: the `format` value set, `port` in 1–65535, non-blank text fields, the
`bearer_env` environment-variable name, and `deadline` as an RFC 3339 timestamp.

## Why

The adversary pass on story:loom-agent-resource-domain
(review-result:adversary-w20261025-resource-pass-1, findings J3 and J5, both pre-existing) found
that `crates/loom-sdk/src/agent.rs` and `crates/loom-sdk/src/models.rs` refuse documents the
specification admits. A CRD can only carry what the specification declares, so a `LoomAgent` the
API server accepts can still be refused by the pod. The same constraints in ESS close the gap for
every projection, not only the CRD.

## Acceptance

Conformance scenarios in `ess/scenarios/` (domain `loom.agent`), each red until the constraint is
declared and the decoder reads the generated type:

- `a_port_outside_the_range_is_refused`: `port: 0` and `port: 65536` are refused by the
  specification's schema, naming the field.
- `a_blank_name_is_refused`: a whitespace-only `name`, `source`, `base_url` or `path` is refused.
- `an_unknown_format_is_refused`: a `format` value outside the declared set is refused, with the set
  named.
- `a_bearer_env_that_is_not_an_environment_name_is_refused`: `bearer_env: "x y"` is refused.
- `an_absent_list_reads_as_empty`: every `List<…>` the `loom.agent` document declares is declared
  with an empty default (or optional), so the CRD projection no longer carries the exemption
  `crates/loom-resource/src/crd.rs` names for `loom.agent` lists.
- `a_deadline_that_is_not_a_timestamp_is_refused`: `deadline: 2026-12-31` is refused; a full
  RFC 3339 instant is accepted, and `timestamp()` in `crates/loom-sdk/src/agent.rs` parses it.
- `deploy/crd/loomagents.loom.beyond10x.dev.yaml` regenerated with the constraints
  (`minimum`/`maximum`, `pattern`, `enum`, `format: date-time`); `task crd-drift` exits 0.
- The decoder's own checks for these constraints are removed or reduced to reading the generated
  type; `task check` exits 0.

## ESS first

The first commit adds the constraints and the scenarios to `ess/` only; the decoder tests go red
where it was more lenient than the specification, then the decoder follows.

## Not in scope

The controller (design § 9 step 6); a Kubernetes end-to-end run; constraints the decoder does not
enforce today.

## Scope

`ess/domains/agent.yaml`, `ess/scenarios/`, `generated/rust/loom/src/agent.rs` (regenerated),
`crates/loom-sdk/src/agent.rs`, `crates/loom-sdk/src/models.rs`, `crates/loom-sdk/tests/`,
`deploy/crd/loomagents.loom.beyond10x.dev.yaml` (regenerated), `website/docs/reference/ess/`
(regenerated), CHANGELOG.md.
