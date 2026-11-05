//! Adversary finding of wave 2026-11-05-w1, unit U2 (`story:agent-run-without-loops`): `check`
//! admits a spec with no `loops` whose states `inbound-answer@1` allows, because `agent run` runs
//! it, but `agent serve` refuses it at start ("nothing would poll"). An `AgentServe` resource
//! whose spec declares no `loops`, with states or without, is therefore refused at admission as
//! `Malformed` at `spec.agent.loops`, beside the `runtime.connectors` refusal, instead of being
//! admitted and crash-looping. A `Command` resource, whose program may be `agent run`, still
//! admits it.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::admission::{Refusal, admit};
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::parse_resource;
use loom_protocols::ProtocolCatalog;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ess/scenarios/resource-admission")
}

fn read(relative: &str) -> String {
    let path = fixtures().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// `text` with `from` replaced once by `to`, failing when `from` is not in it.
fn with(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the fixture holds `{from}`");
    text.replacen(from, to, 1)
}

/// The answer `admit` gives the resource `text` under the scenarios' configuration.
fn answer(text: &str) -> Result<(), Refusal> {
    let config = Config::parse(&read("config.yaml")).expect("the scenarios' configuration");
    let agent = parse_resource(text).expect("the resource parses");
    let catalog = ProtocolCatalog::plugins().expect("the plugin catalog");
    let actions = |protocol: &str| catalog.get(protocol).map(|entry| entry.actions());
    admit(&agent.spec, "agents", &config, &actions).map(|_| ())
}

const LOOPS: &str = "    loops:
      outer: {poll_interval_seconds: 60, sources: [chat], min_age_minutes: 0, lookback_minutes: 120}
      inner: {protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}
";

const STATES: &str = "    states:
      - {name: answer, when: {intent: Ask}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [logs, metrics, chat]}
      - {name: look-up, when: {intent: Find}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [logs, metrics]}
      - {name: hand-off, when: {intent: Task}, actions: [reply.decline], sources: []}
";

/// The admitted `AgentServe` fixture with no `loops`, its states those `inbound-answer@1` admits.
fn serve_without_loops() -> String {
    let text = with(&read("resources/admitted.yaml"), LOOPS, "");
    with(
        &text,
        STATES,
        "    states:
      - {name: answer, when: {intent: Ask}, actions: [source.read, reply.propose, reply.decline], sources: [logs, metrics, chat]}
",
    )
}

fn assert_refused_at_loops(answer: Result<(), Refusal>, case: &str) {
    match answer {
        Ok(()) => panic!("{case}: an AgentServe resource with no loops was admitted"),
        Err(refusal) => {
            assert_eq!(refusal.reason, "Malformed", "{case}: {refusal:?}");
            assert_eq!(refusal.member, "spec.agent.loops", "{case}: {refusal:?}");
            assert!(
                refusal.detail.contains("agent serve"),
                "{case}: the detail names the serve: {refusal:?}"
            );
        }
    }
}

#[test]
fn the_admitted_fixture_with_its_loops_is_still_admitted() {
    answer(&read("resources/admitted.yaml")).expect("the fixture is admitted");
}

#[test]
fn an_agent_serve_resource_with_states_and_no_loops_is_refused_at_loops() {
    assert_refused_at_loops(answer(&serve_without_loops()), "states, no loops");
}

/// The variant admitted before this wave, too: no `loops` and no `states`.
#[test]
fn an_agent_serve_resource_with_no_states_and_no_loops_is_refused_at_loops() {
    let text = with(&read("resources/admitted.yaml"), LOOPS, "");
    let text = with(&text, STATES, "");
    assert_refused_at_loops(answer(&text), "no states, no loops");
}

/// A `Command` resource runs its own program (`agent run` among them), so a spec with no
/// `loops` is admitted: the admitted `Command` fixture declares none, and the same holds with the
/// `AgentServe` fixture's no-loops document as its `spec.agent`.
#[test]
fn a_command_resource_with_no_loops_is_admitted() {
    let command = read("resources/admitted-command.yaml");
    assert!(
        !command.contains("loops:"),
        "the Command fixture declares no loops"
    );
    answer(&command).expect("the Command fixture is admitted");
    let serve = serve_without_loops();
    let as_command = with(
        &serve,
        "\n  routes:\n",
        "\n  command: {args: [--once]}\n  routes:\n",
    );
    answer(&as_command).expect("a Command resource with no loops is admitted");
}
