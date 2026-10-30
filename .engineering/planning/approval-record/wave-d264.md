---
format: aep.planning-md/3
id: approval-record:wave-d264
kind: approval-record
status: approved
title: 'Wave D264 accepted under the orchestrator''s grant: keyless loopback endpoint, engineering-protocols 0.15.2, Connectors invoke form'
tags:
- non-interactive
relations:
- decides: story:session-executor-keyless-loopback
- decides: story:engineering-protocols-0-15-pin
- decides: story:connectors-cli-invoke-form
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T06:45:41Z", actor: "human:timo", revision: 2}
---
# Wave D264 accepted under the orchestrator's grant: keyless loopback endpoint, engineering-protocols 0.15.2, Connectors invoke form

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved item D264 under the grant in the workspace instructions (class C: a wave, its commits,
one bot pull request, its merge and one release, Loom 0.52.0), 2026-10-30, in these words:

- "Story: \"the session executor admits a keyless loopback endpoint\". In loom 0.51.0,
  `check_api_key_secret` in loom-session-executor (src/lib.rs:1176-1188) refuses a b10x-harness
  model endpoint that has no token file and no secret."
- "Admit a keyless endpoint only when its URL is a loopback address (127.0.0.1, ::1 or localhost).
  Better still, admit it only when it equals the boundary's model URL, if the executor can see
  that." "A non-loopback endpoint with no key is still refused, with the same error as today."
- "The consumer is takt wave 47, story U2 AC2/AC4: a keyless loopback model endpoint for a takt run
  in the credential-free pod."
- "Ride-along: move loom's engineering-protocols pin from 0.14.0 to 0.15.2. 0.15.2 is released and
  verified."
- "Ride-along: check for the connectors forms removed in v0.60.0 ... If loom calls them, move them
  to the new form in this wave."
- "Pick up 1 or 2 other accepted loom stories that do not collide, if any are small. Cap the wave at
  4 units."

Decisions taken inside the item:

1. The executor does not see the boundary's model URL (it is the pod's environment, not the
   executor's input), so a keyless endpoint is admitted on a loopback host: `127.0.0.0/8`, `::1`,
   or `localhost`, read from the parsed URL's host.
2. Loom calls `operations invoke` (`crates/loom-connectors/src/cli.rs`, `src/cli/write.rs`), so
   the Connectors form moves in this wave as `story:connectors-cli-invoke-form`.
3. No fourth unit: the three proposed stories (`a2a-record-and-streaming`,
   `agent-spec-constraints-in-crd`, `operator-constraints-in-ess`) are not small, and the second
   changes the CRD a kind-cluster trial reads.
4. The ESS pin stays at 0.77.0, the newest ESS release.
