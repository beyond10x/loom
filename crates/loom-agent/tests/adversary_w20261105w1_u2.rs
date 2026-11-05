//! Adversary cases for `story:agent-run-without-loops` (wave 2026-11-05-w1, unit 2).
//!
//! The story's Outcome: `agent run` runs a spec that declares no `loops`, "and a channel binding
//! without `list`, `get` or `search`, since a run polls nothing". Its Why cites the serve check
//! `agent run` inherits: the channel binding's `list`, `get` and `search` (`serve.rs`, the
//! `polled` lowering). These cases build plugins for one run directly; nothing runs, no model is
//! called and no process is started.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_agent::SpecPlugin;
use b10x_loom_agent::run::RunItem;
use b10x_loom_plugin::{Classification, Decimal, Host, Intent, Plugin, PluginError, PluginState};
use loom_sdk::agent::{AgentSpec, parse};

const WRITE: &str = "message.post";

fn state(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary-u2-{label}-{}-{}",
        std::process::id(),
        now.as_nanos()
    ))
}

/// A spec with `loops` when `loops` is true, the channel binding `chat` declaring `chat_ops`
/// (`list`, `get`, `search`, `write` members as YAML), `writes.mode: mode`, and states naming
/// `answer_actions` for an ask.
fn spec(loops: bool, chat_ops: &str, mode: &str, answer_actions: &str, caps: &str) -> AgentSpec {
    let loops = if loops {
        "loops:
  outer: {poll_interval_seconds: 60, sources: [docs], min_age_minutes: 0, lookback_minutes: 120}
  inner: {protocol: operator-reply@1, step_budget: 8, model_budget: 16, classify_threshold: 0.5}
"
    } else {
        ""
    };
    let text = format!(
        "format: loom.agent/1
agent: {{id: adv-operator, name: Adversary operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
{loops}states:
  - {{name: answer, when: {{intent: Ask}}, actions: {answer_actions}, sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture{chat_ops}}}
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
channel: {{binding: chat, allow: [C0ADV0001], bot_user_id: U0BOT}}
writes: {{mode: {mode}, max_per_hour: 5}}
policy: {{capabilities: {caps}}}
runtime:
  state_dir: agent-state
  connectors: {{program: connectors, state_dir: connectors-state}}
  confinement: none
"
    );
    parse(&text).expect("the adversary spec decodes")
}

fn empty_state() -> PluginState {
    PluginState {
        cursors: Vec::new(),
        handled: Vec::new(),
        failing: Vec::new(),
    }
}

fn classification(intent: Intent) -> Classification {
    Classification {
        intent,
        hints: Vec::new(),
        confidence: Decimal("0.9".to_owned()),
    }
}

/// RED (pre-existing): the story's Why is that `agent run` inherits the serve check on the
/// channel binding's `list`, `get` and `search`, and its Outcome is that a run needs none of them
/// "since a run polls nothing". A spec with `loops` (the only kind that can send, under the
/// decided rule) whose channel binding declares only its `write` is still refused for a run: the
/// `polled` lowering runs for every spec with `loops` and a `channel`, a run or not.
#[test]
fn a_sending_run_needs_no_list_get_or_search_on_its_channel_binding() {
    let spec = spec(
        true,
        &format!(", write: {WRITE}"),
        "auto",
        "[source.read, reply.propose, reply.send, reply.decline]",
        "[datasource.read, reply.propose, reply.send]",
    );
    let run = SpecPlugin::for_run(spec, &state("sending-write-only"));
    match run {
        Ok(run) => assert!(run.sends(), "the channel binding's write is the send"),
        Err(error) => panic!(
            "a run polls nothing, so its channel binding needs no list, get or search: {error}"
        ),
    }
}

/// Green sweep of the decided rule: with no `loops`, for every `writes.mode` and every shape of
/// the channel binding (write only, a full Slack binding, no operation at all), the run is built,
/// can never send, polls nothing and projects no `reply.send`; `agent serve`'s plugin refuses the
/// same spec naming the loops, before anything runs.
#[test]
fn with_no_loops_no_mode_and_no_binding_shape_makes_a_run_send() {
    let shapes = [
        format!(", write: {WRITE}"),
        format!(
            ", list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}"
        ),
        String::new(),
    ];
    let actions = "[source.read, reply.propose, reply.decline]";
    for mode in ["none", "approval", "auto"] {
        // `reply.send` as a capability while writes are `none` is `SendWithoutWrites`.
        let caps = if mode == "none" {
            "[datasource.read, reply.propose]"
        } else {
            "[datasource.read, reply.propose, reply.send]"
        };
        for shape in &shapes {
            let case = format!("mode {mode}, chat{shape}");
            let dir = state("sweep");
            match SpecPlugin::new(spec(false, shape, mode, actions, caps), &dir) {
                Err(PluginError::Config(why)) => {
                    assert!(why.contains("`loops` is undeclared"), "{case}: {why}");
                }
                other => panic!("{case}: serve needs loops: {:?}", other.map(|_| ())),
            }
            let run = SpecPlugin::for_run(spec(false, shape, mode, actions, caps), &dir)
                .unwrap_or_else(|error| panic!("{case}: a run is built: {error}"));
            assert!(!run.sends(), "{case}: a run with no loops never sends");
            assert_eq!(run.config().classify_threshold, None, "{case}");
            let host = Host::new(run.config());
            let item = b10x_loom_agent::run::run_item_now(run.spec(), "What about the deploy?");
            let one = RunItem::new(&run);
            let polled = one
                .poll(&host, &empty_state(), &[])
                .unwrap_or_else(|error| panic!("{case}: {error}"));
            assert!(polled.items.is_empty(), "{case}: a run polls nothing");
            let projection = one.project(&host, &item, &classification(Intent::Ask));
            assert_eq!(
                projection.actions,
                ["source.read", "reply.propose", "reply.decline"],
                "{case}"
            );
            assert!(!dir.exists(), "{case}: building made no state directory");
        }
    }
}
