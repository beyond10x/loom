---
format: aep.planning-md/3
id: review-result:adv-w2-u2-pass-1
kind: review-result
status: active
title: Adversary, loom w2 U2 connectors CLI reads, pass 1
relations:
- reviews: story:connectors-cli-reads
revision: 1
---
unit: U2 `story:connectors-cli-reads`, uncommitted tree `lw2-u2` on `impl/connectors-cli-reads` (HEAD `b4a97ae`)
verdict: NEEDS-CHANGE
cases: executed 36→42, red 2
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 5 (part 6)
needs-coordinator: whether a read timeout goes into `ConnectorsCliConfig`. That is a spec change, and the story names none.

**1. Proof of what I touched.** `git --no-pager diff --stat` shows only the implementor's 17 tracked files, so nothing tracked changed. `git status --short` shows one new file of mine: `?? crates/loom-connectors/tests/connectors_cli_adversary.rs`. Every other untracked path was already there and belongs to the implementor. I touched no non-test path.

**2. Cases added** in `~/.local/state/worktree/trees/b10x/loom/lw2-u2/crates/loom-connectors/tests/connectors_cli_adversary.rs`

| Case | Asserts | Now |
|---|---|---|
| `a_program_that_never_answers_does_not_block_the_read` | `invoke_read` returns within 15 s when the program runs `exec sleep 60` | **red** |
| `a_configured_path_with_a_space_reaches_the_program` | a `--config` path under `Application Support/` reaches argv | **red** |
| `describe_answering_another_operation_is_never_invoked` | a write is asked for, describe answers a read: `Failed`, 0 invokes | green |
| `invoke_answering_another_operation_is_not_a_read` | the invoke answer names another operation: `Failed` | green |
| `an_answer_contradicting_its_exit_status_is_not_a_read` | exit 1 with `ok:true` on stdout, and exit 0 with a refusal on stdout: both `Failed` | green |
| `an_answer_never_carries_its_text_into_the_error` | a token in a truncated answer or in a refusal's text stays out of the error and the `ReadRefusal` | green |

Each red case run alone (`cargo test -p b10x-loom-connectors --locked --test connectors_cli_adversary -- --exact <name>`):
```
thread 'a_program_that_never_answers_does_not_block_the_read' panicked at crates/loom-connectors/tests/connectors_cli_adversary.rs:154:19:
the read had not returned 15 s after it started: the caller is blocked for as long as `connectors` runs
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 15.00s
```
```
thread 'a_configured_path_with_a_space_reaches_the_program' panicked at crates/loom-connectors/tests/connectors_cli_adversary.rs:176:5:
Err(Failed("--config `~/.local/state/worktree/trees/b10x/loom/lw2-u2/target/tmp/connectors_cli_adversary/a_configured_path_with_a_space_reaches_the_program/Application Support/connectors.toml` cannot be passed to connectors"))
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

**3. Suite, run after the cases existed**
- `cargo test -p b10x-loom-connectors --locked --no-fail-fast`: by target 0 / 15 / 3 / 10 / **4 passed, 2 failed** / 8, doc-tests 0. Exit 101.
- The same run with my file left out (`--lib --test adversary --test adversary_read_performed --test connectors_cli --test connectors_invoker`): 36 passed, exit 0. That is where the "before" count comes from.
- Clippy `-D warnings` passes and `cargo fmt --all --check` passes.

**4. Findings** (all against tree `lw2-u2`; `cli.rs` does not exist at the base, so every finding is `introduced`)

| # | file:line | What was measured | What reaches it | Verdict | Origin |
|---|---|---|---|---|---|
| F1 | `crates/loom-connectors/src/cli.rs:391` | `wait_with_output()` has no bound. The caller stayed blocked for 15 s, then the test gave up. | Every read: a poll or a turn's `source.read`. On the real CLI this is a hypothesis only (a locked-keyring prompt, a metadata lock wait). Connectors bounds its own invoke with a read deadline, but nothing bounds describe from the caller's side. Observed with the fake only. | NEEDS-CHANGE | introduced |
| F2 | `crates/loom-connectors/src/cli.rs:368` | `argument()` refuses whitespace, and is also applied to `--config` and `--state-dir` paths. A path is one argv element; only a leading `-` can make it read as a flag. | Only an operator who configures a path with a space. Connectors defaults to XDG or `$HOME/.config`, so the default path never has one. | CONFIRMED | introduced |
| F3 | `crates/loom-connectors/src/cli.rs:252` | Writes are refused only when the profile is exactly `"mutation"`. In connectors v0.38.0 that word comes from the catalog adapter alone (`adapters/catalog/src/lib.rs:1233`). The CLI's own v0.38.0 test (`apps/connectors/src/local/operations.rs:290-337`) describes a write as `"resource"`. Every unit fixture uses `"read"`, a profile no v0.38.0 adapter prints; reads on this host print `generic-http` and `tavily/2026-10`. | No shipped non-catalog adapter with a write was found. Every adapter on this host lists reads only. At v0.38.0 Connectors refuses a write anyway without `--approval-file` (`owner/mutation/execution.rs:161`), and this client never passes that flag. | INFEASIBLE | introduced |
| F4 | `crates/loom-connectors/src/cli.rs:436` | `not_granted` is returned and never revalidated. Connectors documents that ordinary reads never revalidate implicitly (`docs/evidence/gitlab-ci-20260910/README.md:67-68`); the zendesk example revalidates once and then retries. | A long-running poll: once the connection's validation evidence lapses, every read fails until an operator revalidates by hand. The story's "never retried" permits this. | CONFIRMED | introduced |

Proposed fixes (I applied none):
- **F1:** add an optional timeout to the spec, then kill and reap the child when it expires.
- **F2:** check paths for a leading `-` only.
- **F3:** state in `AGENTS.md` that the refusal depends on the adapter publishing `mutation`, and change the read fixtures to a real profile such as `generic-http`.

**5. Attacked and could not break**
- Describing one operation and invoking another, or invoking from a cached describe: the client describes on every read and checks `operation.id`.
- IDs or a describe-supplied revision starting with `-`, or containing whitespace, a newline or NUL: refused, and no process starts.
- Input in argv: it goes on stdin only.
- Environment: everything except the `INHERITED` names is dropped, case-sensitively. None of those names is a credential, and Connectors reads only the XDG variables and `HOME`.
- Tokens in the CLI's answer: they never reach the error text or the `ReadRefusal`.
- Truncated or malformed JSON, and an answer whose exit status contradicts it: `Failed`.
- Owner daemon holding the output pipe open: Connectors starts its owner with null stdio (`owner/transport.rs:791-793`), so it cannot.

**6. Paths written outside the worktree**
- `~/.cache/conductor-dev-analysis/slack-handler/scratch/u2-adv/red-hang.log`
- `~/.cache/conductor-dev-analysis/slack-handler/scratch/u2-adv/red-space.log`
- `~/.cache/conductor-dev-analysis/slack-handler/scratch/u2-adv/suite.log`
- `~/.cache/conductor-dev-analysis/slack-handler/scratch/u2-adv/suite-all.log`
- `~/.cache/conductor-dev-analysis/slack-handler/scratch/u2-adv/suite-before.log`

Inside the worktree, the test directories are under the ignored `target/tmp/connectors_cli_adversary/`. Each hang-case run leaves one `sleep 60` child that exits on its own within 60 s.

**7. Findings block**
```findings
[
  {"file": "crates/loom-connectors/src/cli.rs", "line": 391, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "A connectors process that never exits blocks every read indefinitely because wait_with_output has no bound and the config has no timeout."},
  {"file": "crates/loom-connectors/src/cli.rs", "line": 368, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "The flag-injection check refuses --config and --state-dir paths containing whitespace, though only a leading dash could read as a flag."},
  {"file": "crates/loom-connectors/src/cli.rs", "line": 252, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "The write refusal matches only the catalog adapter's literal mutation profile, while connectors v0.38.0's own write fixture is described as resource and the unit's read fixtures use a read profile no adapter prints."},
  {"file": "crates/loom-connectors/src/cli.rs", "line": 436, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "A poll that outlives the connection's validation evidence gets not_granted on every read with no revalidation path, which Connectors documents a scheduled caller must do explicitly."}
]
```
