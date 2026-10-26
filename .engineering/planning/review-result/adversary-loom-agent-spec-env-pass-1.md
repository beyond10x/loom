---
format: aep.planning-md/3
id: review-result:adversary-loom-agent-spec-env-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-26-cmdv u3, story:loom-agent-spec-env, at a4a24e8e
relations:
- reviews: story:loom-agent-spec-env
revision: 1
---
# Adversary pass 1, wave 2026-10-26-cmdv u3, story:loom-agent-spec-env, at a4a24e8e

Agent `aep:adversary` (opus). Header and findings block verbatim; prose condensed.

```
unit: story:loom-agent-spec-env (u3), tree loom-cmdv-u3 at a4a24e8e plus one untracked test file
verdict: red
cases: executed 135→140, red 3
origin: introduced 6, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (suite and task logs)
needs-coordinator: yes. Three red cases need routing: the credential walk misses a credential that Kubernetes `$(VAR)` expansion assembles at runtime.
```

Cases in `crates/loom-operator/tests/adv_cmdv_u3_spec_env.rs`:
`adv_the_expansion_helper_is_the_kubelets` (green), `adv_a_name_starting_with_an_underscore_is_admitted`
(green), and three red: `adv_a_token_joined_from_two_spec_env_values_is_refused`,
`adv_a_url_with_a_password_joined_from_two_spec_env_values_is_refused`,
`adv_a_token_joined_from_spec_env_into_spec_command_is_refused`.

Suite: `cargo test -p b10x-loom-operator --locked --no-fail-fast` exit 101, 137 passed, 3 failed
(only the new cases). `task drift`, `task crd-drift`, `task chart-check`, `task docs-check`,
`task conform` (3 passed) and clippy exit 0. The quadratic duplicate-name check was measured on a
debug build (10k / 20k / 40k entries: 1.4 / 3.9 / 12.7 s); the API server's object size cap was not
measured. The NUL-byte, control-prefixed and 200 KiB values are admitted; their runtime effect on a
cluster is unknown.

```findings
[
{"file": "crates/loom-operator/src/admission.rs", "line": 256, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "Kubernetes $(VAR) expansion joins two admitted spec.env values into a token or password URL whose bytes all lie in the resource; red: adv_a_token_joined_from_two_spec_env_values_is_refused, adv_a_url_with_a_password_joined_from_two_spec_env_values_is_refused"},
{"file": "crates/loom-operator/src/admission.rs", "line": 256, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "spec.command args expand $(NAME) from spec.env, so a token prefix in spec.env joins a token in args unwalked; red: adv_a_token_joined_from_spec_env_into_spec_command_is_refused"},
{"file": "docs/design/loom-agent-crd-deployment.md", "line": 333, "category": "contract-drift", "severity": "note", "verdict": "needs-revision", "origin": "introduced", "message": "design and README.md:353 call spec.env values plain and say a secret has only secretRef/secretFiles as its way in; kubelet expansion refutes both"},
{"file": "crates/loom-operator/src/admission.rs", "line": 302, "category": "property", "severity": "note", "verdict": "needs-revision", "origin": "introduced", "message": "the duplicate-name check is quadratic: 40000 entries take 12.4 s against 2.7 s with an early exit (debug build), on every reconcile"},
{"file": "crates/loom-operator/src/admission.rs", "line": 256, "category": "judgement", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "a value referencing only a Secret key inside a URL's user information is refused SecretLiteral though it holds no literal"},
{"file": "crates/loom-operator/src/admission.rs", "line": 285, "category": "judgement", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "a spec.env name equal to a secretRef Secret key overrides it (env wins over envFrom); undecidable at admission without Secret read access"}
]
```

## Routing

The coordinator sends the two warnings and the two needs-revision notes back to the implementor:
walk each `spec.env` value and each `spec.command` argument after the kubelet's `$(NAME)` expansion
against the names defined before it (`spec.env` in order, plus the controller's own), keep the
adversary's cases unchanged, make the duplicate check linear, and correct the design and README.
The two approve notes stay recorded: the false refusal of a `$(KEY)` reference in a URL's user
information is the safe side, and the override of a `secretRef` key needs a Secret read the
controller does not have.
