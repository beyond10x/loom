---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-acceptance-1
kind: review-result
status: active
title: Plan critic (acceptance), wave 7 round 1, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
revision: 1
---
needs-revision

story:bound-program-result-table — the Outcome promises that "a host that gives no runner gets the refusal `NoRunner`" and that the port calls `preflight_child`, but no Acceptance line checks either, and the port takes a required `Arc<dyn TestRunner>`, so the line has to name the runner it means (for example `UnconfinedRunner`, which keeps the default `run_child`, `executor.rs:1165`) and that the refusal is `NoRunner`, the program never starts and nothing is recorded — .engineering/planning/story/bound-program-result-table.md:37-38 (no line under `## Acceptance`, :73-102)
story:bound-program-result-table — "`task check` exits 0." already passes at HEAD and passes again after the story, so it shows no change and names no outcome of this story — .engineering/planning/story/bound-program-result-table.md:102
story:bound-program-result-table — the blob line joins five separate outcomes in one bullet (the record cites the observation, the SHA-256, the path `blobs/<sha256>`, the order stdout then stderr, one file for two runs), and two of them name no test: "written … through a temporary file and a rename" leaves no state a test can read afterwards, and "`crates/loom-governor` is not changed by this story" is a diff check, not a test — .engineering/planning/story/bound-program-result-table.md:92-95
story:bound-program-result-table — the lines at :77, :80, :89 and :96 each hold two to four outcomes that can pass or fail independently (exit 0, exit 3 and timeout fallthrough; pointer read and three refusal causes; revision, instant, producer and ignored members; refusal at the cap and survival across a reopen), so the story can be partly done and still not fail its checks — .engineering/planning/story/bound-program-result-table.md:77
story:bound-program-result-table — no Acceptance line names a test; only :97 names the file `crates/loom-bindings/tests/bound_program.rs`, so each of the other lines has no check that a reviewer can name as red before the story and green after it — .engineering/planning/story/bound-program-result-table.md:73-101

What I read: 1 artifact (`aep plan artifact show story:bound-program-result-table`, store revision 14, not 13 as the brief says), `git diff` of the story against HEAD `aa943153`, and at HEAD `confinement.rs` (`TestRunner` :78-115, `TestExecution` :196-207, `ChildCommand` :1166-1450, limits :3096-3104), `confinement/engine.rs` (:89, :134, local route :300-380), `bindings.rs` (:66-84, `child` :317-361, `report` :604, `submit` :653), `loom-sdk/src/lib.rs` :100-123, `executor.rs` :1163-1165.

What I could not establish:
- Revision fix confirmed: the Outcome no longer says "cleared environment" or "injected runner", and no Acceptance, ESS-first or Out-of-scope sentence still describes behaviour from before w6. Acceptance :84-85 ("only variable is `PATH`") matches the new Outcome.
- Public items: `ChildCommand::new`, `with_stdin`, `with_timeout`, `with_environment`, `TestRunner`, `TestExecution`, `ExecEnvironment` (`confinement.rs:56`) and `ConfinementRefusal` (`:19-25`) are all `pub` and exported through `pub mod confinement`. The story's text needs no `pub(crate)` item.
- Out of my lane (design): on the `linux` adapter, `CommandRunner::child` mounts an absolute program outside `/usr`, `/bin`, `/lib`, `/lib64` with `at_host_path` (`bindings.rs:351-357`). The new port's Outcome (:34-37) builds no such mount, and "Not in this story" (:110-113) keeps read-only roots out. So a program at an absolute path, such as an `ess` binary under the home directory, would not start under `linux`.
- Out of my lane (Not-in-this-story claims): "each stream up to 1 MiB" holds on `linux` (`confinement.rs:3100`). The `local` route admits its profile with "no bounds" (`engine.rs:308-309`), so the claim is not shown for `local`.
- Not verified (no build run): that a shell stub sees only `PATH`. A shell sets `PWD` and `SHLVL` itself, so the check at :84 is only meaningful if the stub reports its environment without a shell.

```findings
- file: .engineering/planning/story/bound-program-result-table.md
  line: 37
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises that a host that gives no runner gets the refusal NoRunner and that the port calls preflight_child, but no Acceptance line checks either, and the port takes a required Arc<dyn TestRunner>, so the line has to name the runner it means (for example UnconfinedRunner, which keeps the default run_child) and that the refusal is NoRunner, the program never starts and nothing is recorded"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 102
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: pre-existing
  message: "'task check exits 0.' already passes at HEAD and passes again after the story, so it shows no change and names no outcome of this story"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 92
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: pre-existing
  message: "the blob line joins five separate outcomes in one bullet, and two of them name no test: the temporary-file-and-rename write leaves no state a test can read afterwards, and 'crates/loom-governor is not changed by this story' is a diff check, not a test"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 77
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: pre-existing
  message: "the lines at :77, :80, :89 and :96 each hold two to four outcomes that can pass or fail independently, so the story can be partly done and still not fail its checks"
- file: .engineering/planning/story/bound-program-result-table.md
  line: 73
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: pre-existing
  message: "no Acceptance line names a test; only :97 names the file crates/loom-bindings/tests/bound_program.rs, so each of the other lines has no check that a reviewer can name as red before the story and green after it"
```
