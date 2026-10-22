//! Adversary cases for `story:agent-domain` (wave 2026-10-22-w3): `loom_sdk::agent::parse` driven
//! against the story's contract, `ess/domains/agent.yaml` and Commission's declared types.

use loom_sdk::agent::{
    self, ActionBudget, BreachStep, BudgetLimit, LimitBreachStep, RunBudget, StopPolicy, parse,
};
use loom_sdk::commission::model::primitives::Timestamp as CommissionTimestamp;
use loom_sdk::commission::model::responsibility as commission;

const HEAD: &str = "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";

fn with(extra: &str) -> String {
    format!("{HEAD}{extra}")
}

// ---- drift guard: the copies of Commission's types, field for field and variant for variant ----

/// Exhaustive on both sides: a field or variant added to, removed from or retyped in either
/// declaration stops this file compiling, so the "field for field" claim of
/// `ess/domains/agent.yaml` (RunBudget, ActionBudget, StopPolicy, LimitBreachStep, BreachStep,
/// BudgetLimit) is held by a program.
fn run_budget(budget: RunBudget) -> commission::RunBudget {
    let RunBudget {
        max_steps,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
        max_tokens,
        deadline,
        per_action,
    } = budget;
    commission::RunBudget {
        max_steps,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
        max_tokens,
        deadline: deadline.map(|instant| CommissionTimestamp(instant.0)),
        per_action: per_action.into_iter().map(action_budget).collect(),
    }
}

fn run_budget_back(budget: commission::RunBudget) -> RunBudget {
    let commission::RunBudget {
        max_steps,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
        max_tokens,
        deadline,
        per_action,
    } = budget;
    RunBudget {
        max_steps,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
        max_tokens,
        deadline: deadline
            .map(|instant| loom_sdk::session::model::primitives::Timestamp(instant.0)),
        per_action: per_action
            .into_iter()
            .map(
                |commission::ActionBudget {
                     action,
                     max_trials,
                     max_tokens_in,
                     max_tokens_out,
                     max_duration_ms,
                     max_cost_micro_usd,
                 }| ActionBudget {
                    action,
                    max_trials,
                    max_tokens_in,
                    max_tokens_out,
                    max_duration_ms,
                    max_cost_micro_usd,
                },
            )
            .collect(),
    }
}

fn action_budget(budget: ActionBudget) -> commission::ActionBudget {
    let ActionBudget {
        action,
        max_trials,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
    } = budget;
    commission::ActionBudget {
        action,
        max_trials,
        max_tokens_in,
        max_tokens_out,
        max_duration_ms,
        max_cost_micro_usd,
    }
}

fn stop_policy(policy: StopPolicy) -> commission::StopPolicy {
    let StopPolicy {
        approval_gate,
        idle_bound,
        on_breach,
        breach_by_limit,
    } = policy;
    commission::StopPolicy {
        approval_gate,
        idle_bound,
        on_breach: breach_step(on_breach),
        breach_by_limit: breach_by_limit
            .into_iter()
            .map(
                |LimitBreachStep { limit, step }| commission::LimitBreachStep {
                    limit: budget_limit(limit),
                    step: breach_step(step),
                },
            )
            .collect(),
    }
}

fn stop_policy_back(policy: commission::StopPolicy) -> StopPolicy {
    let commission::StopPolicy {
        approval_gate,
        idle_bound,
        on_breach,
        breach_by_limit,
    } = policy;
    StopPolicy {
        approval_gate,
        idle_bound,
        on_breach: breach_step_back(on_breach),
        breach_by_limit: breach_by_limit
            .into_iter()
            .map(
                |commission::LimitBreachStep { limit, step }| LimitBreachStep {
                    limit: budget_limit_back(limit),
                    step: breach_step_back(step),
                },
            )
            .collect(),
    }
}

fn breach_step(step: BreachStep) -> commission::BreachStep {
    match step {
        BreachStep::DiscardStep => commission::BreachStep::DiscardStep,
        BreachStep::KeepStep => commission::BreachStep::KeepStep,
    }
}

fn breach_step_back(step: commission::BreachStep) -> BreachStep {
    match step {
        commission::BreachStep::DiscardStep => BreachStep::DiscardStep,
        commission::BreachStep::KeepStep => BreachStep::KeepStep,
    }
}

fn budget_limit(limit: BudgetLimit) -> commission::BudgetLimit {
    match limit {
        BudgetLimit::MaxSteps => commission::BudgetLimit::MaxSteps,
        BudgetLimit::MaxTrials => commission::BudgetLimit::MaxTrials,
        BudgetLimit::MaxTokensIn => commission::BudgetLimit::MaxTokensIn,
        BudgetLimit::MaxTokensOut => commission::BudgetLimit::MaxTokensOut,
        BudgetLimit::MaxDurationMs => commission::BudgetLimit::MaxDurationMs,
        BudgetLimit::MaxCostMicroUsd => commission::BudgetLimit::MaxCostMicroUsd,
        BudgetLimit::MaxTokens => commission::BudgetLimit::MaxTokens,
        BudgetLimit::Deadline => commission::BudgetLimit::Deadline,
    }
}

fn budget_limit_back(limit: commission::BudgetLimit) -> BudgetLimit {
    match limit {
        commission::BudgetLimit::MaxSteps => BudgetLimit::MaxSteps,
        commission::BudgetLimit::MaxTrials => BudgetLimit::MaxTrials,
        commission::BudgetLimit::MaxTokensIn => BudgetLimit::MaxTokensIn,
        commission::BudgetLimit::MaxTokensOut => BudgetLimit::MaxTokensOut,
        commission::BudgetLimit::MaxDurationMs => BudgetLimit::MaxDurationMs,
        commission::BudgetLimit::MaxCostMicroUsd => BudgetLimit::MaxCostMicroUsd,
        commission::BudgetLimit::MaxTokens => BudgetLimit::MaxTokens,
        commission::BudgetLimit::Deadline => BudgetLimit::Deadline,
    }
}

const ALL_LIMITS: [&str; 8] = [
    "MaxSteps",
    "MaxTrials",
    "MaxTokensIn",
    "MaxTokensOut",
    "MaxDurationMs",
    "MaxCostMicroUsd",
    "MaxTokens",
    "Deadline",
];

fn session(budget: &str, stop: &str) -> String {
    with(&format!(
        "session:\n  kind: Codex\n  budget: {budget}\n  stop: {stop}\n  confinement: none\n  tools: {{decisions: ask}}\n"
    ))
}

#[test]
fn adversary_every_decoded_budget_and_stop_policy_converts_one_to_one_to_commissions() {
    let breach = ALL_LIMITS
        .iter()
        .enumerate()
        .map(|(at, limit)| {
            let step = if at % 2 == 0 {
                "KeepStep"
            } else {
                "DiscardStep"
            };
            format!("{{limit: {limit}, step: {step}}}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let text = session(
        "{max_steps: 1, max_tokens_in: 2, max_tokens_out: 3, max_duration_ms: 4, max_cost_micro_usd: 5, max_tokens: 6, deadline: '2026-10-22T10:00:00Z', per_action: [{action: x, max_trials: 7, max_tokens_in: 8, max_tokens_out: 9, max_duration_ms: 10, max_cost_micro_usd: 11}]}",
        &format!(
            "{{approval_gate: false, idle_bound: 3, on_breach: KeepStep, breach_by_limit: [{breach}]}}"
        ),
    );
    let spec = parse(&text).unwrap_or_else(|error| panic!("{error}"));
    let session = spec.session.expect("a session");
    let budget = run_budget(session.budget.clone());
    assert_eq!(budget.max_steps, Some(1));
    assert_eq!(budget.max_tokens, Some(6));
    assert_eq!(
        budget.deadline,
        Some(CommissionTimestamp("2026-10-22T10:00:00Z".to_owned()))
    );
    assert_eq!(budget.per_action[0].max_cost_micro_usd, Some(11));
    assert_eq!(run_budget_back(budget), session.budget);

    let stop = stop_policy(session.stop.clone());
    assert_eq!(stop.on_breach, commission::BreachStep::KeepStep);
    assert_eq!(stop.breach_by_limit.len(), 8);
    assert_eq!(
        stop.breach_by_limit[7].limit,
        commission::BudgetLimit::Deadline
    );
    assert_eq!(stop_policy_back(stop), session.stop);
}

// ---- unknown members at every nesting level the domain declares ----

#[test]
fn adversary_an_unknown_member_is_refused_at_every_nesting_level() {
    let cases: &[(&str, &str)] = &[
        ("zzz: 1\n", "zzz"),
        (
            "session: {kind: Codex, budget: {zzz: 1}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: none, tools: {decisions: ask}}\n",
            "session.budget.zzz",
        ),
        (
            "session: {kind: Codex, budget: {per_action: [{action: x, zzz: 1}]}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: none, tools: {decisions: ask}}\n",
            "session.budget.per_action[0].zzz",
        ),
        (
            "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: KeepStep, breach_by_limit: [{limit: MaxSteps, step: KeepStep, zzz: 1}]}, confinement: none, tools: {decisions: ask}}\n",
            "session.stop.breach_by_limit[0].zzz",
        ),
        (
            "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: none, tools: {decisions: ask, egress: [{name: n, host: h, port: 1, zzz: 1}]}}\n",
            "session.tools.egress[0].zzz",
        ),
        (
            "states: [{name: s, when: {intent: Ask, zzz: 1}}]\n",
            "states[0].when.zzz",
        ),
        (
            "bindings: [{kind: http, name: n, base_url: u, method: GET, max_bytes: 1, zzz: 1}]\n",
            "bindings[0].zzz",
        ),
        (
            "bindings: [{kind: connectors, name: n, adapter: a, connection: c, zzz: 1}]\n",
            "bindings[0].zzz",
        ),
        (
            "expose: {a2a: {port: 1, path: /, card: {name: n, description: d, zzz: 1}}}\n",
            "expose.a2a.card.zzz",
        ),
        (
            "runtime: {state_dir: s, connectors: {program: p, zzz: 1}, confinement: none}\n",
            "runtime.connectors.zzz",
        ),
        (
            "deploy: {kubernetes: {namespace: n, service_account: s, image_repository: i, resources: {cpu: c, memory: m}, egress: [{name: e, port: 1, selector: [{name: a, value: b, zzz: 1}]}]}}\n",
            "deploy.kubernetes.egress[0].selector[0].zzz",
        ),
    ];
    for (extra, at) in cases {
        let refused = parse(&with(extra)).expect_err(at);
        assert_eq!(
            refused.to_string(),
            format!("unknown member `{at}`"),
            "{extra}"
        );
    }
    // A binding member of the other kind is unknown to this one.
    let refused = parse(&with(
        "bindings: [{kind: http, name: n, base_url: u, method: GET, max_bytes: 1, adapter: a}]\n",
    ))
    .expect_err("an http binding with a connectors member");
    assert_eq!(refused.to_string(), "unknown member `bindings[0].adapter`");
}

// ---- duplicate keys: the second value must never silently replace the first ----

#[test]
fn adversary_a_member_given_twice_is_refused() {
    let yaml = "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\nwrites: {mode: none, max_per_hour: 0}\nwrites: {mode: auto, max_per_hour: 1000}\n";
    assert!(parse(yaml).is_err(), "a top-level member twice decodes");
    let nested = with(
        "runtime: {state_dir: a, state_dir: b, connectors: {program: p}, confinement: none}\n",
    );
    assert!(parse(&nested).is_err(), "a nested member twice decodes");
    let json = r#"{"format": "loom.agent/1", "agent": {"id": "a", "id": "b", "name": "A", "brief": "b"}, "model": {"routes": "r", "roles": []}}"#;
    assert!(parse(json).is_err(), "a JSON member twice decodes");
}

// ---- size, depth and YAML features ----

#[test]
fn adversary_a_document_past_the_byte_bound_is_refused() {
    let padding = "#".repeat(2 * 1024 * 1024);
    let text = format!("{HEAD}{padding}\n");
    let refused = parse(&text).expect_err("an oversized document decodes");
    assert_eq!(refused.to_string(), "the document exceeds its byte bound");
}

#[test]
fn adversary_deep_nesting_is_refused_without_overflowing_the_stack() {
    let depth = 100_000;
    let text = format!(
        "{HEAD}policy: {{capabilities: {}{}}}\n",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    assert!(parse(&text).is_err(), "a deeply nested document decodes");
}

#[test]
fn adversary_alias_expansion_past_its_bound_is_refused() {
    let mut text = format!("{HEAD}x0: &a0 [aaaaaaaa]\n");
    for level in 1..20 {
        let previous = level - 1;
        text.push_str(&format!(
            "x{level}: &a{level} [*a{previous}, *a{previous}, *a{previous}, *a{previous}, *a{previous}, *a{previous}, *a{previous}, *a{previous}, *a{previous}]\n"
        ));
    }
    assert!(parse(&text).is_err());
}

#[test]
fn adversary_a_plain_alias_decodes_and_a_merge_key_is_refused() {
    let alias = with("states:\n  - &s {name: s, when: {intent: Ask}}\n  - *s\n");
    assert_eq!(parse(&alias).expect("an alias decodes").states.len(), 2);
    let merge = with("states:\n  - &s {name: s, when: {intent: Ask}}\n  - {<<: *s, name: t}\n");
    assert!(parse(&merge).is_err(), "a merge key decodes");
}

#[test]
fn adversary_a_second_document_is_refused() {
    let text = format!("{HEAD}---\n{HEAD}");
    assert!(parse(&text).is_err(), "a stream of two documents decodes");
}

#[test]
fn adversary_a_custom_tag_on_a_value_is_refused() {
    assert!(
        parse("format: loom.agent/1\nagent: {id: !Secret a, name: A, brief: b}\nmodel: {routes: r, roles: []}\n")
            .is_err()
    );
    assert!(
        parse("format: loom.agent/1\nagent: !Agent {id: a, name: A, brief: b}\nmodel: {routes: r, roles: []}\n")
            .is_err()
    );
}

#[test]
fn adversary_a_key_that_is_not_text_is_refused() {
    let refused =
        parse(&with("writes: {mode: auto, max_per_hour: 1, 7: x}\n")).expect_err("a number key");
    assert_eq!(refused.to_string(), "`writes` holds a key that is not text");
}

// ---- enum spellings outside the declared set ----

#[test]
fn adversary_every_enum_refuses_a_spelling_it_does_not_declare() {
    let cases: &[&str] = &[
        "session: {kind: claudecode, budget: {}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: none, tools: {decisions: ask}}\n",
        "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: keep_step}, confinement: none, tools: {decisions: ask}}\n",
        "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: KeepStep, breach_by_limit: [{limit: max_steps, step: KeepStep}]}, confinement: none, tools: {decisions: ask}}\n",
        "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: bwrap, tools: {decisions: ask}}\n",
        "session: {kind: Codex, budget: {}, stop: {approval_gate: true, on_breach: KeepStep}, confinement: none, tools: {decisions: Frame}}\n",
        "states: [{name: s, when: {intent: ask}}]\n",
        "writes: {mode: Auto, max_per_hour: 1}\n",
        "runtime: {state_dir: s, connectors: {program: p}, confinement: None}\n",
        "bindings: [{kind: grpc, name: n}]\n",
    ];
    for extra in cases {
        assert!(parse(&with(extra)).is_err(), "decodes: {extra}");
    }
    // `confinement: none` must stay a text, not YAML's null: `~` and `null` are not `none`.
    assert!(
        parse(&with(
            "runtime: {state_dir: s, connectors: {program: p}, confinement: ~}\n"
        ))
        .is_err(),
        "a null confinement decodes"
    );
}

// ---- required members: present but empty ----

#[test]
fn adversary_an_empty_agent_id_is_refused() {
    // `loom_sdk::models::parse_config`, which this decoder is built like, refuses an empty text
    // (`text_field`, "a non-empty text"); the agent's id is required by the story.
    let text = "format: loom.agent/1\nagent: {id: '', name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";
    assert!(parse(text).is_err(), "an empty agent id decodes");
    let blank = "format: loom.agent/1\nagent: {id: '  ', name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";
    assert!(parse(blank).is_err(), "a blank agent id decodes");
}

// ---- secrets: a reference is expected, never a literal ----

#[test]
fn adversary_a_bearer_token_literal_where_a_variable_name_is_expected_is_refused() {
    // `ess/domains/agent.yaml`: `bearer_env` is "the environment variable holding a bearer
    // token". A pasted token is not a variable name and must not be accepted into a spec file.
    let text = with(
        "bindings: [{kind: http, name: logs, base_url: 'https://logs.example', method: GET, max_bytes: 1, bearer_env: 'Bearer eyJhbGciOiJIUzI1NiJ9.e30.ZRrHA1JJJW8opsbCGfG_HACGpVUMN_a9IV7pAx_Zmeo'}]\n",
    );
    assert!(
        parse(&text).is_err(),
        "a bearer token literal decodes as a variable name"
    );
}

// ---- integers out of their range ----

#[test]
fn adversary_a_port_outside_one_to_65535_is_refused() {
    for port in ["0", "-1", "65536"] {
        let text = with(&format!(
            "expose: {{a2a: {{port: {port}, path: /a2a, card: {{name: n, description: d}}}}}}\n"
        ));
        assert!(parse(&text).is_err(), "port {port} decodes");
    }
}

// ---- cost of a refusal ----

#[test]
fn adversary_probe_nesting_cost() {
    for depth in [1_000usize, 2_000, 4_000, 8_000] {
        let text = format!(
            "{HEAD}policy: {{capabilities: {}{}}}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        let started = std::time::Instant::now();
        let refused = parse(&text).expect_err("nested");
        eprintln!("depth {depth}: {:?} ({refused})", started.elapsed());
    }
}

// ---- the stability page and the module agree ----

#[test]
fn adversary_the_decoded_spec_is_the_type_the_page_names() {
    let spec: agent::AgentSpec = parse(HEAD).expect("the minimal spec decodes");
    assert_eq!(spec.agent.id.0, "a");
    assert!(spec.session.is_none() && spec.states.is_empty() && spec.bindings.is_empty());
}
