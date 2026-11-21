---
format: aep.planning-md/3
id: review-result:connectors-relay-client-security-1
kind: review-result
status: active
title: Security review, pass 1, of unit U2 of wave 2026-11-21-w5 (story:connectors-relay-client) at 69793e4d
relations:
- reviews: story:connectors-relay-client
revision: 1
---
```
unit: story:connectors-relay-client, worktree loom-w1121e-u2 at head 69793e4d plus one untracked test file
verdict: green
cases: executed 149→163, red 0
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: 35 files, all in ~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w5/u2/sec1/
needs-coordinator: no
```

All eight invariants hold. I added 14 test cases and none failed. There is one finding: a possibly flaky test in the implementor's suite (a note, not a blocker).

1. `git --no-pager diff --stat` is empty. `git status --short` shows only `?? crates/loom-connectors/tests/security_w20261121w5_u2.rs`. No implementation file was touched, even temporarily.

2. My cases are all in that file. I ran each one alone first, before the package suite. Each finished `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out` (logs: `sec1/alone-*.log`). The timed ones took 5.50 s (connect bound, read) and 0.01 s (endless body).

3. Package suite run afterwards: `ESS_TOOLCHAIN=0.86.0 CARGO_INCREMENTAL=0 cargo test -p b10x-loom-connectors --locked` exited 0. Every lane passed at its usual count (`connectors_relay` 34) and mine shows `ok. 14 passed`. After formatting and a fix for one lint complaint, `cargo clippy --test security_w20261121w5_u2 -- -D warnings` exits 0, `rustfmt --check` exits 0, and the file reran 14/14 green.

**Per invariant:**
- **1, URL rule: holds.** `every_other_spelling_of_a_relay_url_is_refused_naming_only_the_rule` tries 66 values. Every one is refused by both `loopback_url` and `new`, and the refusal text is exactly `RelayUrlRefused::RULE`, so it repeats nothing of the value. The values cover `localhost` and `localhost.`, `127.1`, `127.0.0.2`, `0.0.0.0`, `[::1]`, `[::ffff:127.0.0.1]`, decimal, hex and octal IPv4, a zone id, user information, a path, `/`, a query, a fragment, other schemes, upper case, ports 0, 00, 08402, 65536 and no port, full-width digits, white space, NUL/DEL/CR/LF, and a second URL after a separator. Ports 1 and 65535 are accepted. This matches the story's rule.
- **2, every request goes only to the relay URL: holds.**
  - `no_3xx_is_followed_with_or_without_a_location`: statuses 300–308 and 399, with and without a `location`, at the describe and at the invoke, for a read and a write. The other listener sees 0 connections, and a write's invoke comes back `Unknown` after one send.
  - `proxy_and_credential_variables_change_nothing` reruns the binary as a child with all six proxy variables and a non-covering `NO_PROXY` set. The proxy sees 0 connections. The variables go only into the child's own environment, so they cannot leak into parallel cases.
  - There is no DNS lookup: the URL is rebuilt from the port alone (`relay.rs:97`).
- **3, no credential: holds.** The same child test also sets 9 credential-style variables, and no request carries `authorization`, `proxy-authorization`, `x-api-key`, `cookie` or the token. `a_set_cookie_is_never_sent_back` passes. `src/relay.rs` contains no `env::` call and no file access.
- **4, one pair: holds.** `every_member_naming_another_adapter_or_connection_sends_nothing` tries another adapter, another connection, and both, against all five methods plus `over`. The fake records 0 connections. The descriptor's `instance` and `adapter` are not compared with the pair. The story does not ask for that, and the client holds no Connectors instance id to compare against. For a write, the boundary's operation allowlist is the guard.
- **5, writes: holds.**
  - `a_write_of_an_operation_the_describe_does_not_list_sends_no_invoke` returns `not_found` after one describe and no invoke.
  - `the_same_admitted_write_twice_carries_one_key_and_two_request_ids`: one key, two distinct `loom-relay-<32 hex>` request ids. Nothing treats the request id as a secret or as an idempotency key.
  - It uses the same `admits` and proof-path rules as the command-line write (`relay.rs:395-419` against `write.rs:150-216`).
  - On a lost answer the write comes back `Unknown` (`relay.rs:484`).
- **6, answers: holds.**
  - `an_answer_that_is_neither_the_boundarys_nor_connectors_never_reads_as_not_sent` sends 13 bodies at a write's invoke: a boundary body with a third member, a boundary code at the wrong status, a message that is not text, a duplicated key, a code in upper case, a boundary body in a 2xx, a refusal body at 200, a success body at 401, an unknown member, a truncated body, a length past the limit, a text/html body, and no answer. Each gives `Unknown` with no request id and no code, sent once; the same bodies at a read give `upstream-protocol:`.
  - The size bound is `RESPONSE_LIMIT` (4 MiB): `a_body_that_never_ends_is_cut_at_the_response_limit` returns the "response limit" message well before 64 MiB is streamed.
  - The Connectors host source writes no `{"error","message"}` body, so an upstream answer cannot pass for a boundary one.
- **7, bounds and lifetime: holds.** The brief said there was no loopback way to test the connect bound; there is one, and it is deterministic on Linux. A listener with an accept queue of 1, filled with connections, makes further connects hang.
  - `a_connect_that_never_completes_ends_at_the_connect_bound` returns `relay-unavailable:` in 4–12 s.
  - `a_write_whose_invoke_cannot_connect_is_refused_unavailable_at_the_connect_bound` returns `Refused relay-unavailable`.
  - Without the 5 s connect bound, the 20 s request bound would fire instead and both cases would fail. That is from reading `relay.rs:711-721`; I did not run that change.
  - `a_clone_and_over_still_answer_after_the_original_drops`, `a_call_inside_another_tokio_runtime_answers_and_dropping_there_does_not_panic` (current-thread and multi-thread) and `concurrent_calls_on_clones_answer_with_distinct_request_ids` (40 reads from 8 threads) all pass. The runtime pattern is the same as `ConnectorsInvoker`'s (`lib.rs:216-241`).
- **8, nothing existing weakened: holds.** The only changes are two visibility widenings (`pub(crate)`) and the remedy text moved into a function with the same text. Every existing lane passes at its base count. No test was deleted or marked `#[ignore]`; the only change to an existing test is four `use` lines in `host_ports.rs`. Nothing changed under `.engineering/`, `ess/` or `generated/`, nor `Cargo.toml`. The only manifest and lock changes are one `reqwest` line with `default-features = false` and one line in `Cargo.lock`. `status.json` may be edited by hand (AGENTS.md:330). Nothing the story puts out of scope is present.
- **What the safety rests on:** a write answered `token_unavailable`, `upstream_unreachable` or `upstream_timeout` is reported as "nothing was sent". That is true because the boundary sends those answers before or at connecting upstream, never after writing the request (`loom-boundary/src/lib.rs:430-431`, `:595-602`). I got this fact only to step 3 (walked through the code): `loom-connectors` cannot load the boundary in a test without a manifest change.

**Finding (note, judgement, CONFIRMED, introduced), `tests/connectors_relay.rs:316`:** `unreachable_url` binds port 0, then closes the listener. On this shared machine another process can take that port before the case runs. `a_relay_that_cannot_be_reached_is_unavailable` (`:1116`) would then send `GET /v1/describe` to that unrelated local process, and the case would fail with a message that does not start with `relay-unavailable:`. Nothing has been seen to trigger this. The fix: keep a socket bound to the port but not listening (a Tokio `TcpSocket` that is bound and never `listen`ed). Connects to it are refused, and no other process can take the port.

**Not settled:** the step-3 boundary fact above, and the connect-bound mutation, which I reasoned through but did not run.

**Paths written outside the worktree:** 35 files in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w5/u2/sec1/`:
- scripts: `build.sh`, `alone.sh`, `alone-endless.sh`, `suite.sh`, `recheck.sh`
- `alone-*.log` (one per case, 14), `alone-endless-2.log`
- status files: `alone-status.txt`, `alone-endless-2-status.txt`, `build-status.txt`, `suite-status.txt`, `recheck-status.txt`
- disk checks: `alone-endless-disk.txt`, `build-disk.txt`, `suite-disk.txt`, `recheck-disk.txt`
- logs: `build.log`, `clippy.log`, `clippy-2.log`, `fmt.log`, `suite.log`, `recheck-test.log`

`target/` is at 8.3G and 147G is free on `/`. My session lease is released.

```findings
- file: crates/loom-connectors/tests/connectors_relay.rs
  line: 316
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "unreachable_url releases its port before use, so another local process can take it and a_relay_that_cannot_be_reached_is_unavailable then sends a describe to that process and fails red"
```
