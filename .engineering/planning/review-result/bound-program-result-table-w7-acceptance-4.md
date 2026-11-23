---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-acceptance-4
kind: review-result
status: active
title: Plan critic (acceptance), wave 7 round 4, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
revision: 1
---
approve

Round-3 findings: all three are closed.
- What "no arguments" means: closed. The Outcome now defines it as the `arguments` member equal to `{}` (.engineering/planning/story/bound-program-result-table.md:11-13), and the two argument lines test `{}` and a non-empty object.
- The admission's `NoRunner` text: closed by the new line `the_no_runner_detail_names_no_flag_and_no_file_member`.
- The non-UTF-8 `PATH` answer: closed by three test lines (refused step with its reason, no program started, nothing recorded). The "read in review" sentence is gone.

The argument rule:
- The two lines check the `{}` half and the non-empty-object half.
- The cited behaviour matches what I read: the SDC driver defaults `--args` to `{}` (`sdc/driver/src/main.rs:69`) and falls back only on `args == json!({})` (`:1245-1254`). The intake path always passes `request.arguments` (`bindings.rs:450`).
- No line contradicts the rule. A binding's literal `arguments` must be a mapping (`decode.rs:221`), and `arguments_from` output must be one object. So only a request can carry something other than an object, and the line saying the program gets "one JSON object" (built from an object request) still holds for its fixture.

The test entry point is usable from a test of `crates/loom-bindings`:
- `run_until_blocked` is `pub` in `pub mod runtime` (`runtime.rs:1194`). Its bound `G: Governor + ObservationPort` is met by `CanonGovernor` (`loom-governor/src/lib.rs:1377`, `:1545`).
- `ScriptedExecutor::new` (`fake_executor.rs:42`) and `StaticAuthorityProvider::new` (`fake_authority.rs:44`) are `pub` in the crate `b10x-loom-commission-testkit`. The commission, `RunStore`, `Generated` and `Context` are built from public items (`loom-connectors/tests/adversary.rs:19-38`, `:299-302`).
- A scripted proposal takes any value through `ProposedActionArguments(arguments)` (`adversary.rs:431-436`), so it can propose both `{}` and a non-empty object.

The non-UTF-8 `PATH` tests can be built:
- `Command::env` accepts any `AsRef<OsStr>`, and `OsStr::from_bytes` builds non-UTF-8 bytes on Unix.
- The precedent runs the test binary again with `--exact`, a marker variable and a set `PATH`, and the child asserts its own outcome (`adversary_w20261021w6_u3.rs:82-103`). So the parent reads all three outcomes from the child's exit status and output.

What I read: 1 artifact at store revision 18 (it matches `w7-bprt-body-4.md`), `diff -u` of the two drafts, `runtime.rs:1185-1215`, the testkit's `src`, `loom-connectors/tests/adversary.rs:11-40`, `:299-302` and `:400-470`, `adversary_w20261021w6_u3.rs:73-103`, `bindings.rs:450`, `decode.rs:221`, and the SDC `main.rs:69` and `:1245-1254`.

What I could not establish:
- Not a finding, it does not stop the story being checked: no line checks the `null` half of the rule. An implementation that treats `null` as "no arguments" would pass every line. One more `cat` line ("a request whose `arguments` is `null` gives the program `null`") would close that gap.
- Not verified without a run: whether anything in the child process started under a non-UTF-8 `PATH` (Substrate's `local` adapter opening, the runtime) reads `PATH` before the port does.

```findings
[]
```
