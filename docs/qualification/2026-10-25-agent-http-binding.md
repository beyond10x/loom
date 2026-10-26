# Agent http binding qualification — 2026-10-25

This records the standing of `story:agent-http-binding` (an agent spec's `http` binding read by
a served turn) at its implementation, before a tagged release. It does not claim a tagged release
or published documentation.

## What was verified headlessly

Every acceptance test of the story runs offline, against a listener on the loopback or a fake
reader, with no network and no credential, and passes in the unit's gate:

| Claim | Test |
|---|---|
| A POST binding is refused before any request, by `agent check` and by the reader | `crates/loom-agent/tests/http.rs`, `a_post_on_an_http_binding_is_refused` |
| A path outside `allow_paths`, with `\`, a `.` or `..` segment or an encoded separator, is refused before any request, even when listed; `agent check` refuses such an entry as `Malformed` | `crates/loom-agent/tests/http.rs`, `a_path_outside_allow_paths_is_refused`; `crates/loom-agent/tests/check.rs`, `an_allow_path_no_read_may_use_is_refused`; ESS `an-allow-path-holding-a-dot-dot-segment-is-refused`; `crates/loom-agent/tests/adv_http_binding.rs`, `adv_a_backslash_dot_dot_path_is_refused_even_when_listed` |
| An unset, empty or blank bearer variable is refused before any request | `crates/loom-agent/tests/http.rs`, `a_missing_bearer_is_refused_before_any_request`, `an_empty_bearer_is_refused_before_any_request`; `crates/loom-agent/src/http.rs`, `an_unset_empty_or_blank_variable_is_no_bearer`; `adv_an_empty_bearer_variable_is_refused_before_any_request` |
| The bearer is sent from the variable `bearer_env` names, with the query as parameters | `crates/loom-agent/tests/http.rs`, `the_bearer_is_read_from_the_variable_bearer_env_and_the_query_is_sent` |
| A response over `max_bytes` is cut at it and marked; the text is never longer than `max_bytes`, cut on a character boundary, a byte that is not UTF-8 dropped | `crates/loom-agent/tests/http.rs`, `a_response_over_max_bytes_is_truncated_and_marked`, `an_answer_is_its_utf8_within_max_bytes`; `crates/loom-agent/src/http.rs`, `the_text_of_a_body_is_never_past_the_bound`; `adv_a_cut_answer_stays_within_max_bytes_on_a_character_boundary`, `adv_a_non_utf8_answer_stays_within_max_bytes`; `crates/loom-plugin/tests/http_reads.rs`, `an_http_read_is_dispatched_to_the_reader_and_marked_when_cut` |
| A redirect is not followed and the refusal names at most 256 bytes of the location, no more than `max_bytes`, marked when cut | `crates/loom-agent/tests/http.rs`, `a_redirect_is_not_followed_and_names_the_location`, `a_long_redirect_location_is_cut_and_marked`; `adv_a_redirect_location_is_held_to_max_bytes` |
| A failing status is a refused read | `crates/loom-agent/tests/http.rs`, `a_failing_status_is_refused` |
| A log line is data, not instructions | `crates/loom-plugin/tests/http_reads.rs`, `a_log_line_is_data_not_instructions`; `crates/loom-agent/tests/http.rs`, `a_served_turn_reads_an_http_binding_and_its_answer_is_data` |
| A `connectors` read is unchanged by an `http` binding beside it | `crates/loom-plugin/tests/http_reads.rs`, `a_connectors_read_is_unchanged_by_an_http_binding_beside_it` |

## Not verified here

- The conformance suite drives `loom.agent.AdmitHttpRead` and `loom.agent.BoundHttpAnswer` on
  their declared inputs alone; that the reader feeds them the binding's method, the path's
  admission, the variable's presence, the status and the bytes read is held by the tests above.
- ESS conformance takes `path_allowed` as an input, so it cannot catch a mutant of the
  `path_allowed` function (`crates/loom-agent/src/http.rs`) or of
  `loom_sdk::agent::plain_http_path`; only the Rust tests in `crates/loom-agent/tests/http.rs`,
  `check.rs` and `adv_http_binding.rs` do.
- Cluster CA (updated 2026-10-26, `story:http-binding-cluster-ca`). Verified offline: an `https`
  loopback endpoint whose certificate a CA minted at test time signs is unavailable to the reader
  without `LOOM_HTTP_CA_FILE` and read with that CA's PEM file
  (`crates/loom-agent/tests/http_trust.rs`,
  `without_the_setting_an_endpoint_a_cluster_ca_signs_is_unavailable`,
  `with_the_ca_file_the_endpoint_a_cluster_ca_signs_is_read`); a missing, empty, certificate-less
  or over-1048576-byte file is refused naming the variable and not the file, by `http_trust` and
  by `agent serve` with exit 1 (`agent_serve_refuses_to_start_on_an_unusable_http_ca_file` in
  `crates/loom-cli/tests/agent_serve.rs`); `loom.agent.AdmitHttpTrust` is conformed by
  `task conform`. `crates/loom-agent/Cargo.toml` now declares reqwest's
  `rustls-tls-webpki-roots` itself. Not verified: no run against a real cluster CA or an
  in-cluster endpoint; no test reads an endpoint the webpki roots sign with the setting present
  (that needs the network), so that the file's roots are added beside the webpki roots rather
  than instead of them rests on the reader's code (`HttpGet::read` adds them to a client that
  keeps its built-in roots); no `agent serve` test starts with a valid CA file, because the
  command line's tests mint no certificate.
- No real log or metrics endpoint was read: every answer in the tests comes from a loopback
  listener, so what a real endpoint answers for an allowed path, and whether `max_bytes` of
  262144 is a useful bound for one, is unverified until the manual run.
- The Outcome's "the `connectors` kind reads the same sources through Connectors' loki and
  prometheus adapters" names no Loom file and is not touched by this story.

## Manual run

No manual run against a live endpoint was made: the implementation session had no in-cluster
endpoint and the story's tests use none. The record of one is pending. To make it:

1. Write a `loom.agent/1` spec with an `http` binding whose `base_url` is the endpoint,
   `allow_paths` its read paths, `method: GET`, `bearer_env` the variable holding the token when
   one is needed, and `max_bytes: 262144`; name the binding in a state's `sources`;
   `b10x-loom agent check --spec <FILE>` says `checked`.
2. `b10x-loom agent serve --spec <FILE> --state <DIR> --once` after a question is posted in an
   allowed channel: the record line names one read `logs get`, and the proposal quotes what the
   endpoint answered.
3. Record the spec (the bearer elided), the request the endpoint logged and the record line
   here, dated.
