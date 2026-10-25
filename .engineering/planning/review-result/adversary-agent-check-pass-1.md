---
format: aep.planning-md/3
id: review-result:adversary-agent-check-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:agent-check, unit tree at 3d9da645 plus the uncommitted implementation
relations:
- reviews: story:agent-check
revision: 1
---
Adversary pass 1 on `story:agent-check` (unit tree `wt-5ce7ac161710`, branch `impl/agent-check`, the implementation uncommitted on top of `3d9da645`), `aep:adversary` (opus, high), 2026-10-25. The report as it returned:

```
unit: story:agent-check, working tree wt-5ce7ac161710 (uncommitted on top of 3d9da645)
verdict: red
cases: executed 337→353, red 4
origin: introduced 8, pre-existing 1, undecided 0
wrote-outside-worktree: the assigned scratch directory u1-adv-scratch/ (the full list is in part 6)
needs-coordinator: yes. Decision D4 says a refusal "never repeats the value". D6 says the `Malformed` detail is "the decoder's message verbatim". Finding 2 is where the two collide, and only the coordinator can say which one gives way.
```

**1. Diff.** `git --no-pager diff --stat` reads `52 files changed, 556 insertions(+), 91 deletions(-)`, the same as the implementor's tree, so I changed no tracked file. I added two untracked test files, `crates/loom-agent/tests/adv_check.rs` and `crates/loom-cli/tests/adv_agent_check.rs`. Every mutant ran in a scratch copy (`replica/`), and that copy was restored afterwards (`cmp` shows no differences).

**2. Cases.** Each case was run alone first; the red output below is quoted from those runs.

| file:test | asserts | now |
|---|---|---|
| adv_check.rs:233 `adv_a_password_holding_a_hash_is_refused` | `http://reader:hun#ter2@logs…` is refused as SecretLiteral (D4) | red: `a URL carrying a user name and password is served: Ok(Checked { agent: "platform-operator", protocol: Some("operator-reply@1"), states: 3, bindings: 3 })` |
| adv_check.rs:269 `adv_a_credential_in_an_enum_member_is_not_repeated` | the refusal does not repeat `xoxb-hunter2` | red: ``the refusal repeats the credential: Malformed at document: `writes.mode` is `xoxb-hunter2`, not one of auto, approval, none`` |
| adv_agent_check.rs:51 `adv_a_credential_in_an_enum_member_never_reaches_stderr` | exit 3, and stderr does not contain `hunter2` | red: ``standard error repeats the credential: b10x-loom: refused: Malformed at document: `writes.mode` is `xoxb-hunter2`, …`` |
| adv_agent_check.rs:96 `adv_a_spec_behind_a_byte_order_mark_checks` | the operator example with a UTF-8 BOM in front exits 0 | red: `left: Some(3) right: Some(0)`, `refused: Malformed at document: invalid YAML: deserializing from YAML containing more than one document is not supported` |
| 12 more `adv_*` cases | first binding, connectors `search` query, `get` in lower case, two ordering pairs, order inside unbound-source, writes none without send, `reply.send` under `inbound-answer@1`, `operator-reply@2`, user name without password, not-UTF-8 exits 1, no `--spec` exits 2, empty file is Malformed | green; they kill the mutants below |

**3. Suite.** I ran `cargo test --locked --no-fail-fast -p b10x-loom-agent -p b10x-loom-protocols -p b10x-loom-cli -p b10x-loom-conformance -p b10x-loom-sdk -p b10x-loom-plugin`. It exited 101, with 349 passed, 4 failed and 6 ignored (summed over the per-binary `test result` lines). The two failing targets are `-p b10x-loom-agent --test adv_check` and `-p b10x-loom-cli --test adv_agent_check`, and the 4 failures are the 4 red cases above. The 337 before is the implementor's own count.

**Mutants.** Each ran in the scratch copy against `check.rs`, my `adv_check.rs`, and for the misses also `--test conform` and the CLI `agent_check`:

| mutant | unit's suite | caught by |
|---|---|---|
| M1: swap the UnknownAction and UnboundSource checks (lib.rs:97-98) | missed: check 18/18, conform 3/3, cli 5/5 | `adv_an_unknown_action_is_answered_before_an_unbound_source` |
| M2: swap the SendWithoutWrites and EmptyChannelAllow checks (lib.rs:100-101) | missed, same counts | `adv_send_without_writes_is_answered_before_an_empty_channel_allow` |
| M4: the secret walk skips `bindings[0]` (strings.rs:71) | missed | `adv_a_connection_string_in_the_first_binding_is_refused` |
| M6: `GET` compared case-insensitively (lib.rs:282) | missed | `adv_a_lower_case_get_is_refused` |
| M8: unbound sources searched in reverse order (lib.rs:273) | missed | `adv_a_state_source_is_answered_before_the_outer_loop_and_the_channel` |
| M12: the detail always says "and password" (lib.rs:168) | missed | `adv_a_user_name_without_a_password_has_its_own_detail` |
| M3: drop the query rule; M5: ignore `loops.outer.sources`; M7: refuse when `channel` is missing | caught by check.rs | |
| M9: swap two reasons in the conformance target's `BRANCHES` (agent.rs:36-37); M10: swap two fixtures; M11: return another reason (lib.rs:319) | `task conform` red (4, 4 and 2 failed scenarios) | |

**4. Findings beyond the test cases.**
- Finding 1 (lib.rs:166) is a person pasting a credentialed URL whose password holds `#`. The password then stays in the document. The possible fix: look for `@` before the first `/` after `://`, ignoring `#` and `?`.
- Finding 2 (lib.rs:82, printed at main.rs:89) is INFEASIBLE: I found nobody who writes a token into an enum member. The decoder's message already did this at the base commit (my probe of the base gives the identical text). The new part is that `agent check` now prints it to stderr. The implementor disclosed it.
- Finding 3 (models.rs:350, BOM) is pre-existing: at the base commit the decoder refuses the same input with the same message.

**5. Attacked and could not break:** the protocol tests for operator-reply@1 (`reply.send` blocked or not admissible, `answered` needing all three evidences, `declined`); the plugin catalog holding exactly two entries; a wrong major version; `reply.send` under `inbound-answer@1`; `writes: none` without `reply.send` being serveable; a query in a nested connectors field; not-UTF-8 input exiting 1, a missing `--spec` exiting 2, an empty file refused as Malformed; the file-system play of decision D1 (the M9–M11 mutants went red).

**6. Paths written outside the worktree:** the assigned scratch directory `u1-adv-scratch/` (`replica/`, `base/` = `git archive 3d9da645` plus `crates/loom-sdk/tests/adv_base_probe.rs`, `target/` 4.2G, `mutants.sh`, logs). Inside the worktree, the CLI cases left 6 directories at `target/tmp/adv-agent-check-<pid>`. My lease `agent-check-u1-adv` is released.

**7. Findings block.** Recorded through `--findings` from `u1-adv-findings-1.json`, the author's corrected JSON (the first block used `summary` for `message` and the store refused it).

```findings
[
{"file":"crates/loom-agent/src/lib.rs","line":166,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the userinfo test cuts the authority at the first '#' or '/', so a URL whose password holds '#' is served with the password in the document"},
{"file":"crates/loom-agent/src/lib.rs","line":82,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a credential in an enum member is refused as Malformed, and the decoder message used as detail repeats it on agent check's stderr"},
{"file":"crates/loom-sdk/src/models.rs","line":350,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"a UTF-8 BOM before a valid spec is refused as Malformed with 'more than one document'"},
{"file":"crates/loom-agent/src/lib.rs","line":97,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"swapping the UnknownAction and UnboundSource checks survived the unit's suite and conform"},
{"file":"crates/loom-agent/src/lib.rs","line":100,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"swapping the SendWithoutWrites and EmptyChannelAllow checks survived the unit's suite and conform"},
{"file":"crates/loom-agent/src/strings.rs","line":71,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"skipping bindings[0] in the credential walk survived the unit's suite and conform"},
{"file":"crates/loom-agent/src/lib.rs","line":282,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"accepting 'get' in any case as GET survived the unit's suite and conform"},
{"file":"crates/loom-agent/src/lib.rs","line":273,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"searching unbound sources in reverse order survived the unit's suite and conform"},
{"file":"crates/loom-agent/src/lib.rs","line":168,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"always reporting 'user name and password' for userinfo without a password survived the unit's suite and conform"}
]
```
