---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-acceptance-r3
kind: review-result
status: active
title: Plan critic acceptance, round 3, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

- `story:boundary-admits-declared-http-egress` — the test of condition 4, the operator's gate, changes only the port and the label set and empties the list. It never uses an entry whose namespace differs from the allowed one, or a label set that is a strict subset of the allowed one. A gate that compares only selector and port therefore passes, and the agent reaches pods with those labels in any namespace that is not an upstream's. — `.engineering/planning/story/boundary-admits-declared-http-egress.md:210-211` against `:75-78`
- `story:connectors-relay-client` — the go-away test says only "the fake answers the describe, then stops listening". It does not require the fake to answer with `connection: close` (as the precedent does, `tests/connectors_invoker.rs:276`) and to close its listener before it writes that answer. Without that, the invoke can be accepted by a closing listener, leave, and come back `Unknown` under the table's last row instead of `Refused` `relay-unavailable`. — `.engineering/planning/story/connectors-relay-client.md:174-175` against `:109`
- `story:send-input-in-the-declared-shape` — case 7 must "also name the argument". But the rule it tests refuses a required body with no argument under it, so no argument is at fault, and the expected `argument` value is undefined (ESS makes it `Optional`). — `.engineering/planning/story/send-input-in-the-declared-shape.md:184` against `:69-70`, `:132`
- `story:send-input-in-the-declared-shape` — case 8 passes with the object-schema check ignored. A `type: string` schema has no properties, so with `{channel, text}` every argument fits nowhere and is refused `input-shape-unknown` anyway. The case does not assert that the refusal names no argument, and its fixture declares no properties a skipped check would place. — `.engineering/planning/story/send-input-in-the-declared-shape.md:180`, `:162`
- `story:agent-selects-the-connectors-relay` — two `Unset` rules have no test. `Unset`'s methods should answer `no-connectors-transport:` with no request and no process. `run_plugin_on` should refuse an `Unset` host, and that is `agent serve`'s entry (`main.rs:320`). Only `run_item` is tested. — `.engineering/planning/story/agent-selects-the-connectors-relay.md:89-91`, `:105-107` against `:305-307`
- `story:agent-selects-the-connectors-relay` — the three `cli` and `relay` scenarios each run as the binary under `serve`, `run` and `resume`, but nothing says what those tests assert; only a refusal has an observable. A blank URL beside `runtime.connectors` is checked by nothing else in the binary. — `.engineering/planning/story/agent-selects-the-connectors-relay.md:292-294`
- `epic:boundary-agent-reaches-connectors-and-http-sources` — "The Slack poll and `agent serve` under the relay start no `connectors` program (same story)" now follows the `story:connectors-relay-client` bullet, which this revision inserted, so it points at a story with no poll or serve test. In r1 and r2 it followed the story-4 bullet. — `.engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md:107`

**Round-2 findings**
- Relay blocker: fixed. The table has three columns (`connectors-relay-client.md:101-109`), every test names its request (`:148-149`), and a write with no listener is now `WriteError::Failed` (`:171-173`), matching `:103`.
- Shape `body_required`: fixed (case 7, `:177-179`). The new defect above is in how it names the argument, not in its coverage.
- Egress golden: fixed. A named test reads the golden file (`:191-199`), as `render.rs:3140` does. `boundary(true, false, false)` exists at `render.rs:1770`.
- Select early return: fixed (`:314-328`). A search confirms the only early returns are `adversary_d243_u1.rs:294`, `:329` and `:403`. `:282` and `:319` now assert their specific refusals, so they can fail.

**The source scans you asked about**
- `src/relay.rs`: the scan can fail for the defect. The only process start in the crate is inside `ConnectorsCli`'s impl (`cli.rs:442`), and the scan forbids `ConnectorsCli`, `Command`, `std::process` and `tokio::process`. The reused `admits`, `same`, `sorted`, `unknown` and `classified` (`write.rs:319-384`) start nothing. The sentence saying so is a claim the scan does not check, but it is true in the tree.
- `src/shape.rs`: the scan fails on a direct process start in a pure module. Neither scan sees a process started through another crate. Story 4's marker tests watch for processes at agent level and cover that.

**The security invariants**
- No credential: story 1 `an_http_egress_adds_no_credential_variable_or_volume`; story 2 `the_relay_client_sends_no_credential`; story 3 `the_shaped_relay_write_sends_no_credential`. In story 4 the client's credential is unobservable behind the boundary, and the story says so.
- Only the declared destination: story 1 "exactly one more rule" (but see the condition-4 finding); story 2 the loopback-URL test and `a_call_naming_another_instance_sends_nothing`; story 4 `SeveralInstances` with no connection.
- Same write policy: story 2 the key, admits and once-only tests; story 4 the `AdmitSend`, relay-refusal and resume-key tests.
- No model or network call: every body uses a loopback fake.

**What I read:** all five ids in full (the epic and four stories) from the stored files, plus `git diff HEAD` of the epic. Also the r2 acceptance verdict, the `e6r2-changes.md` ledger, and the r1 and r2 drafts to place each defect. In the tree: `tests/connectors_cli.rs:455-490`, `cli.rs` structure, `cli/write.rs:50-60,170-186,315-386`, `tests/connectors_invoker.rs:125-160,725-760`, `render.rs:130-160,1765-1870,2586-2640`, `adversary_d243_u1.rs:278-335,395-410`, `effects.rs:84-89,460-480`, `poll.rs:255-270`, `loom-boundary/src/lib.rs:548-569`, `examples/agents/operator.yaml:28-44`, `admission.rs:1650-1666,1688-1694`. In Connectors, `connectors-client` at `v0.62.0` `:48-56,283-301,317-332`: the citations hold.

**What I could not establish**
- Whether reusing `classified`, which reads the command line's `{ok, result}` / `error.data` shape (`write.rs:367-375`), is correct on a v1alpha2 envelope. That is out of my lane (design).
- Out of my lane (scope / parallel-safety): the egress story's scope entries name `resources/boundary-allowed-http-egress.yaml`, `boundary-egress-namespace-not-given.yaml` and `boundary-unbound-egress-namespace.yaml` (`:48-55`). Its ESS-first section and Scope prose name `boundary-http-egress-allowed.yaml`, `boundary-egress-not-allowed.yaml` and `boundary-egress-unbound.yaml` (`:178-179`, `:276`).
- I did not check the catalog's `v0.73.0` line citations.

```findings
- file: .engineering/planning/story/boundary-admits-declared-http-egress.md
  line: 210
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the test of condition 4, the operator's gate, changes only the port and the label set and empties the list; it never uses an entry whose namespace differs from the allowed one, or a label set that is a strict subset of the allowed one, so a gate that compares only selector and port passes, and the agent reaches pods with those labels in any namespace that is not an upstream's"
- file: .engineering/planning/story/connectors-relay-client.md
  line: 174
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the go-away test says only 'the fake answers the describe, then stops listening'; it does not require the fake to answer with connection: close and to close its listener before it writes that answer, so the invoke can be accepted by a closing listener, leave, and come back Unknown under the table's last row instead of Refused relay-unavailable"
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 184
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "case 7 must also name the argument, but the rule it tests refuses a required body with no argument under it, so no argument is at fault and the expected argument value is undefined"
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 180
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "case 8 passes with the object-schema check ignored: a type: string schema has no properties, so every argument fits nowhere and is refused input-shape-unknown anyway; the case does not assert that the refusal names no argument, and its fixture declares no properties a skipped check would place"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 89
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "two Unset rules have no test: Unset's methods answering no-connectors-transport: with no request and no process, and run_plugin_on refusing an Unset host, which is agent serve's entry; only run_item is tested"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 292
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the three cli and relay scenarios each run as the binary under serve, run and resume, but nothing says what those tests assert; only a refusal has an observable, and a blank URL beside runtime.connectors is checked by nothing else in the binary"
- file: .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md
  line: 107
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'(same story)' now follows the story:connectors-relay-client bullet this revision inserted, so the poll and serve no-program check points at a story with no poll or serve test; name story:agent-selects-the-connectors-relay"
```
