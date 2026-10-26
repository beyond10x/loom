---
format: aep.planning-md/3
id: review-result:adversary-command-opaque-config-files-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-26-cmdv u4, story:command-opaque-config-files, at 171d55b6
relations:
- reviews: story:command-opaque-config-files
- reviews: story:takt-delivery-fixture-args
revision: 1
---
# Adversary pass 1, wave 2026-10-26-cmdv u4, story:command-opaque-config-files, at 171d55b6

Agent `aep:adversary` (opus). Header and findings block verbatim; prose condensed. The branch also
carries story:takt-delivery-fixture-args (D7, commit 171d55b6), attacked in the same pass.

```
unit: u4 (story:command-opaque-config-files and story:takt-delivery-fixture-args) pass 1, tree loom-cmdv-u4 at 171d55b6 plus one untracked test file
verdict: red
cases: executed 155→158, red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (diff, suite, clippy and task logs)
needs-coordinator: yes. Three red cases need routing: one ConfigMap key the API server refuses, and two ways to hide an escaped credential from the YAML walk.
```

Cases in `crates/loom-operator/tests/adv_cmdv_u4_config_files.rs`, all red:
`adv_a_name_starting_with_two_dots_is_refused`,
`adv_an_escaped_token_in_a_second_yaml_document_is_refused`,
`adv_an_escaped_token_beside_aliases_beyond_the_walk_budget_is_refused`.

Suite: `cargo test -p b10x-loom-operator --locked --no-fail-fast` exit 101, 155 passed, 3 failed (the
new cases); clippy and the five `task` checks exit 0. The API server's refusal of a `..`-prefixed
key is from a reading of apimachinery (`IsConfigMapKey`), not observed on a cluster. Held: name
refusals other than `..`, case variants, refusal texts, CRLF and CR line endings, TOML escapes,
the 4096-bracket and 2 MiB YAML bounds, the open model route without `spec.agent` (D-u4.6), the
fixture commit changing only `golden/takt-delivery.yaml`, and the docs apart from the two claims
below. base64 and `!!binary` are not decoded; the docs do not claim they are.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 770, "category": "boundary", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "plain_file_name admits a spec.configFiles name starting with `..`, a ConfigMap key the API server refuses (IsConfigMapKey, hasChDirPrefix), so the apply fails instead of the resource reading Refused; red: adv_a_name_starting_with_two_dots_is_refused"},
{"file": "crates/loom-operator/src/admission.rs", "line": 851, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "the YAML walk of a config file answers no credential for a multi-document stream or once its alias budget is spent, so an escaped token a YAML reader decodes is admitted, contrary to 'so an escape hides nothing'; red: adv_an_escaped_token_in_a_second_yaml_document_is_refused, adv_an_escaped_token_beside_aliases_beyond_the_walk_budget_is_refused"}
]
```

## Routing

Both warnings go back to the implementor: refuse a `..`-prefixed name as `ConfigFileInvalid`;
walk every document of a YAML stream and refuse `TooLarge` when the walk's budget is spent.
