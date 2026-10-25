//! Acceptance for `story:agent-serve`: a `loom.agent/1` spec served as a plugin of the host.
//! The poll walks only `channel.allow`; `reply.send` is a governed write the effect port
//! performs only when `loom.agent.AdmitSend` admits it, held for a person's approval in
//! `writes.mode: approval` and performed once after the grant with the idempotency key derived
//! from the admitted request; `SpecAuthority` grants exactly `policy.capabilities`.
//!
//! Every read and write goes to the fake `connectors` of `loom-connectors`, answering the
//! slack-handler's fixtures and the write fixtures; the models are recorded. Nothing reaches a
//! network, a credential or a real channel.

#[path = "../../loom-plugin-slack/tests/support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_agent::{SpecAuthority, SpecPlugin, admit_send};
use b10x_loom_plugin::datasource::{AdapterAlias, ConnectionId, OperationId, WriteOperation};
use b10x_loom_plugin::effects::SENDS_FILE;
use b10x_loom_plugin::{
    DataSourceEffects, Host, Intent, Plugin, PluginError, PluginState, Projection, RecordOutcome,
    SendPolicy, run_plugin,
};
use b10x_loom_plugin_slack::TurnModels;
use chrono::{Duration, SecondsFormat, Utc};
use loom_sdk::agent::{
    AdmitSend, AdmitSendOutcome, AgentSpec, SendRefusalReason, WriteMode, parse,
};
use loom_sdk::commission::model::behaviour::Generated;
use loom_sdk::commission::model::json::Value;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    CaseId, Commission, CommissionData, CommissionId, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, commission_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::AuthorityProvider;
use loom_sdk::commission::ports::suspension::SuspensionPort;
use loom_sdk::commission::runtime::LoopEnd;
use loom_sdk::governor::{FileCaseStore, FileSuspensionStore};
use loom_sdk::intake::slice::grant::{GrantStore, approve, now};
use loom_sdk::intake::slice::handoff::MergeHandoffStore;
use loom_sdk::{CanonGovernor, LoopContext, MemoryCaseStore, ProtocolCatalog};
use serde_json::json;
use support::host::{Fixture, Proposes, Recorded, chat, classification, item};
use support::{CHANNEL, channels, member};

const WRITE: &str = "message.post";
const OPERATOR_REPLY: &str = "operator-reply@1";
const ALL: [&str; 3] = ["datasource.read", "reply.propose", "reply.send"];
const REPLY: &str = "The deploy finished at 10:02; the logs show no error.";

/// A fixture directory whose fake answers the three Slack reads and the `docs` search from the
/// slack-handler's fixtures (`support::fixture` reads them relative to this crate, so the copy is
/// made here).
fn fixture(test: &str) -> Fixture {
    let fixture = Fixture::new("agent-serve", test);
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../loom-plugin-slack/tests/fixtures");
    for (verb, operation) in [
        ("describe", support::LIST),
        ("describe", support::HISTORY),
        ("describe", support::REPLIES),
        ("describe", support::SEARCH),
        ("invoke", support::LIST),
        ("invoke", support::HISTORY),
        ("invoke", support::REPLIES),
        ("invoke", support::SEARCH),
    ] {
        fs::copy(
            fixtures.join(format!("{verb}-{operation}.json")),
            fixture
                .connectors_state()
                .join(format!("{verb}.{operation}.json")),
        )
        .unwrap();
    }
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    fixture
}

/// The fixtures of `loom-connectors`: the write's describe (of `adapter`) and invoke answers.
fn write_fixtures(fixture: &Fixture, adapter: &str) {
    let connectors =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../loom-connectors/tests/fixtures/connectors");
    let describe = fs::read_to_string(connectors.join("describe-write.json")).unwrap();
    fs::write(
        fixture
            .connectors_state()
            .join(format!("describe.{WRITE}.json")),
        describe.replace(
            "\"adapter\":\"chat\"",
            &format!("\"adapter\":\"{adapter}\""),
        ),
    )
    .unwrap();
    fs::copy(
        connectors.join("invoke-route-applied.json"),
        fixture
            .connectors_state()
            .join(format!("invoke.{WRITE}.json")),
    )
    .unwrap();
}

/// The spec under test, over the fixture's connectors, with `writes` and `allow` as given.
fn spec_text(fixture: &Fixture, mode: &str, allow: &[&str], max_per_hour: i64) -> String {
    let allow: Vec<String> = allow.iter().map(|id| (*id).to_owned()).collect();
    format!(
        "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 0, sources: [chat], min_age_minutes: 10, lookback_minutes: 120}}
  inner: {{protocol: {OPERATOR_REPLY}, step_budget: 8, model_budget: 16, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: [source.read, reply.propose, reply.send, reply.decline], sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies, write: {WRITE}}}
  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
channel: {{binding: chat, allow: {allow:?}, bot_user_id: U0BOT}}
writes: {{mode: {mode}, max_per_hour: {max_per_hour}}}
policy: {{capabilities: [datasource.read, reply.propose, reply.send]}}
runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
        state = fixture.plugin_state().display(),
        program = fixture.root.join("connectors").display(),
        connectors = fixture.connectors_state().display(),
    )
}

fn spec(fixture: &Fixture, mode: &str, allow: &[&str], max_per_hour: i64) -> AgentSpec {
    parse(&spec_text(fixture, mode, allow, max_per_hour)).expect("the spec decodes")
}

/// The plugin of `spec` over `fixture`, classifying every item as an ask about `docs` and
/// running each turn on the next script.
fn plugin(
    fixture: &Fixture,
    spec: AgentSpec,
    scripts: Vec<Vec<(&'static str, serde_json::Value)>>,
) -> SpecPlugin {
    let (turns, _made): (TurnModels, _) = support::turns(scripts);
    SpecPlugin::new(spec, &fixture.plugin_state())
        .expect("the spec is served")
        .with_classifier(Box::new(Recorded::classifying("ask", &["docs"], 0.9)))
        .with_turn_models(turns)
}

fn empty_state() -> PluginState {
    PluginState {
        cursors: Vec::new(),
        handled: Vec::new(),
        failing: Vec::new(),
    }
}

/// A Slack `ts` `minutes` before now.
fn ts_ago(minutes: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}.000100", now - minutes * 60)
}

/// The invokes of `operation` the fake logged, each its argv.
fn invokes_of(fixture: &Fixture, operation: &str) -> Vec<Vec<String>> {
    fixture
        .runs("invoke")
        .into_iter()
        .filter(|argv| {
            argv.windows(2)
                .any(|w| w[0] == "--operation" && w[1] == operation)
        })
        .collect()
}

fn flag<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].as_str())
}

// ---- the poll ------------------------------------------------------------------------------------

#[test]
fn a_message_outside_the_allow_list_is_no_item() {
    let fixture = fixture("serve-allow-list-poll");
    write_fixtures(&fixture, "slack");
    channels(
        &fixture,
        json!([member("C0OUTSIDE", 50), member(CHANNEL, 5)]),
    );
    support::history(
        &fixture,
        vec![support::message(
            "U0ALICE",
            &ts_ago(60),
            "Is the deploy finished?",
        )],
    );
    let plugin = plugin(
        &fixture,
        spec(&fixture, "approval", &[CHANNEL], 5),
        Vec::new(),
    );
    let host = Host::new(plugin.config()).with_environment(fixture.environment());

    let polled = plugin
        .poll(&host, &empty_state(), &[])
        .expect("the poll answers");

    let ids: Vec<&str> = polled.items.iter().map(|item| item.id.0.as_str()).collect();
    assert_eq!(ids.len(), 1, "{ids:?}");
    assert!(ids[0].starts_with(&format!("{CHANNEL}:")), "{ids:?}");
    assert_eq!(
        support::history_order(&fixture),
        [CHANNEL],
        "no history of a channel outside the allow list is read"
    );
}

// ---- the effect port alone -----------------------------------------------------------------------

/// A case on `operator-reply@1` about the fixture item's revision.
fn governed() -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&ProtocolCatalog::plugins().expect("the plugin catalog"))
        .expect("the governor admits it");
    let case = governor
        .open(
            OPERATOR_REPLY,
            [("item".to_owned(), "1700000000.000100".to_owned())].into(),
            loom_sdk::CaseInputs::none(),
        )
        .expect("the case opens");
    (governor, case)
}

/// The send policy of a resume (granted) over `fixture`, to the channels `allow`.
fn granted_policy(fixture: &Fixture, allow: &[&str], max_per_hour: i64) -> SendPolicy {
    SendPolicy {
        operation: WriteOperation {
            action: "reply.send".to_owned(),
            adapter: AdapterAlias("chat".to_owned()),
            connection: ConnectionId("conn-fixture".to_owned()),
            operation: OperationId(WRITE.to_owned()),
        },
        allow: allow.iter().map(|id| (*id).to_owned()).collect(),
        mode: WriteMode::Approval,
        max_per_hour,
        granted: true,
        counter: fixture.plugin_state().join(SENDS_FILE),
        proof_file: fixture
            .plugin_state()
            .join("proof.json")
            .display()
            .to_string(),
        proposal: None,
    }
}

struct Ticks(u64);

impl LoopContext for Ticks {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.0 += 1;
        ActionRequestId(Uuid(format!(
            "00000000-0000-4000-8000-{:012x}",
            0x100 + self.0
        )))
    }
    fn observation_id(&mut self) -> ObservationId {
        self.0 += 1;
        ObservationId(Uuid(format!(
            "00000000-0000-4000-8000-{:012x}",
            0x200 + self.0
        )))
    }
    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-25T12:00:00Z".to_owned())
    }
    fn step_budget(&self) -> Option<usize> {
        Some(6)
    }
}

/// One Run on `case` under an authority allowing every capability of the spec: `executor`
/// proposes and `effects` performs.
fn run(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
    executor: &Proposes,
    effects: &DataSourceEffects<'_, MemoryCaseStore>,
) -> LoopEnd {
    let runs = AtomicU64::new(0x300);
    let mut store = Generated::new(RunStore::new(move || {
        RunId(Uuid(format!(
            "00000000-0000-4000-8000-{:012x}",
            runs.fetch_add(1, Ordering::SeqCst)
        )))
    }));
    let commission = Commission::<commission_state::Assigned>::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: case.clone(),
        principal: PrincipalId("loom-agent".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let allows = SpecAuthority::new(ALL.map(str::to_owned).to_vec(), WriteMode::None);
    loom_sdk::run_until_blocked(
        governor,
        executor,
        &allows,
        effects,
        &commission,
        &mut store,
        &mut Ticks(0),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

fn read_then_propose_then_send() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "source.read",
            json!({"source": "chat", "kind": "list", "input": {"channel": CHANNEL}}),
        ),
        ("reply.propose", json!({"text": REPLY})),
        ("reply.send", json!({"channel": CHANNEL, "text": REPLY})),
    ]
}

fn full_projection() -> Projection {
    Projection {
        actions: [
            "source.read",
            "reply.propose",
            "reply.send",
            "reply.decline",
        ]
        .map(str::to_owned)
        .to_vec(),
        sources: vec![chat().name],
    }
}

#[test]
fn a_send_outside_the_allow_list_runs_no_command() {
    let fixture = Fixture::new("agent-serve", "send-outside-allow");
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    write_fixtures(&fixture, "chat");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let (governor, case) = governed();
    let item = item("item-1");
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &full_projection(),
    )
    .with_send(granted_policy(&fixture, &["C0ELSEWHERE"], 5));
    let executor = Proposes::new(read_then_propose_then_send());

    let end = run(&governor, &case, &executor, &effects);

    match &end.outcome {
        RunOutcome::EffectRefused(refused) => assert!(
            refused.reason.contains("OutsideAllowList"),
            "{}",
            refused.reason
        ),
        other => panic!("the run should end on the refused send: {other:?}"),
    }
    assert!(
        invokes_of(&fixture, WRITE).is_empty(),
        "no write runs: {:?}",
        fixture.argv()
    );
    assert!(effects.sent().is_none());
    assert_eq!(
        executor.calls.load(Ordering::SeqCst),
        3,
        "the run ended there"
    );
}

#[test]
fn a_send_without_a_proposal_is_not_admissible() {
    let fixture = Fixture::new("agent-serve", "send-without-proposal");
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    write_fixtures(&fixture, "chat");
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let (governor, case) = governed();
    let item = item("item-1");
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &full_projection(),
    )
    .with_send(granted_policy(&fixture, &[CHANNEL], 5));
    let executor = Proposes::new(vec![
        (
            "source.read",
            json!({"source": "chat", "kind": "list", "input": {"channel": CHANNEL}}),
        ),
        ("reply.send", json!({"channel": CHANNEL, "text": REPLY})),
    ]);

    let end = run(&governor, &case, &executor, &effects);

    assert!(
        invokes_of(&fixture, WRITE).is_empty(),
        "no write runs: {:?}",
        fixture.argv()
    );
    assert!(effects.sent().is_none());
    assert!(
        !matches!(end.outcome, RunOutcome::Completed(_)),
        "nothing was answered: {:?}",
        end.outcome
    );
    assert_eq!(
        admit_send(&AdmitSend {
            mode: WriteMode::Approval,
            channel_allowed: true,
            proposed: false,
            granted: true,
            sent_in_window: 0,
            max_per_hour: 5,
        }),
        AdmitSendOutcome::NoProposal {
            error: loom_sdk::agent::SendRefused {
                reason: SendRefusalReason::NoProposal,
                r#final: false
            }
        }
    );
}

/// Seeds the counter with `n` sends `minutes_ago`.
fn seed_sends(fixture: &Fixture, n: usize, minutes_ago: i64) {
    let at =
        (Utc::now() - Duration::minutes(minutes_ago)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let lines: Vec<String> = (0..n)
        .map(|i| json!({"at": at, "key": format!("seed-{i}")}).to_string())
        .collect();
    fs::write(
        fixture.plugin_state().join(SENDS_FILE),
        format!("{}\n", lines.join("\n")),
    )
    .unwrap();
}

#[test]
fn a_sixth_send_in_the_window_is_refused() {
    let fixture = Fixture::new("agent-serve", "sixth-send");
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    write_fixtures(&fixture, "chat");
    seed_sends(&fixture, 5, 10);
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let (governor, case) = governed();
    let item = item("item-1");
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &full_projection(),
    )
    .with_send(granted_policy(&fixture, &[CHANNEL], 5));
    let executor = Proposes::new(read_then_propose_then_send());

    let end = run(&governor, &case, &executor, &effects);

    match &end.outcome {
        RunOutcome::EffectRefused(refused) => {
            assert!(refused.reason.contains("RateLimited"), "{}", refused.reason);
        }
        other => panic!("the run should end on the refused send: {other:?}"),
    }
    assert!(
        invokes_of(&fixture, WRITE).is_empty(),
        "{:?}",
        fixture.argv()
    );
    assert!(effects.sent().is_none());
}

#[test]
fn a_fifth_send_in_the_window_is_performed_and_counted() {
    let fixture = Fixture::new("agent-serve", "fifth-send");
    fs::create_dir_all(fixture.plugin_state()).unwrap();
    write_fixtures(&fixture, "chat");
    seed_sends(&fixture, 4, 10);
    let config = fixture.config(vec![chat()]);
    let host = fixture.host(&config);
    let (governor, case) = governed();
    let item = item("item-1");
    let effects = DataSourceEffects::new(
        &governor,
        case.clone(),
        &item,
        host.connectors(),
        &config.sources,
        &full_projection(),
    )
    .with_send(granted_policy(&fixture, &[CHANNEL], 5));
    let executor = Proposes::new(read_then_propose_then_send());

    let end = run(&governor, &case, &executor, &effects);

    let sent = effects.sent().expect("the send was performed");
    assert!(
        sent.idempotency_key
            .starts_with("loom-action-request-00000000-0000-4000-8000-0000000001"),
        "the key is derived from the admitted request: {}",
        sent.idempotency_key
    );
    let writes = invokes_of(&fixture, WRITE);
    assert_eq!(writes.len(), 1, "{:?}", fixture.argv());
    assert_eq!(
        flag(&writes[0], "--idempotency-key"),
        Some(sent.idempotency_key.as_str())
    );
    assert_eq!(
        fs::read_to_string(fixture.plugin_state().join(SENDS_FILE))
            .unwrap()
            .lines()
            .count(),
        5,
        "the send is counted"
    );
    assert!(
        matches!(&end.outcome, RunOutcome::Completed(completed) if completed.outcome == "answered"),
        "{:?}",
        end.outcome
    );
    // The window: five sends an hour ago do not count.
    seed_sends(&fixture, 5, 61);
    assert_eq!(
        b10x_loom_plugin::effects::sent_in_window(
            &fixture.plugin_state().join(SENDS_FILE),
            Utc::now()
        )
        .unwrap(),
        0
    );
}

// ---- the served host -----------------------------------------------------------------------------

#[test]
fn a_send_is_held_until_approved() {
    let fixture = fixture("serve-held-until-approved");
    write_fixtures(&fixture, "slack");
    let ts = ts_ago(60);
    channels(&fixture, json!([member(CHANNEL, 5)]));
    support::history(
        &fixture,
        vec![support::message("U0ALICE", &ts, "Is the deploy finished?")],
    );
    let state = fixture.plugin_state();
    let plugin = plugin(
        &fixture,
        spec(&fixture, "approval", &[CHANNEL], 5),
        vec![vec![
            (
                "source_read",
                json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
            ),
            ("reply_propose", json!({"text": REPLY})),
            (
                "reply_send",
                json!({"channel": CHANNEL, "thread_ts": ts, "text": REPLY}),
            ),
        ]],
    );
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let item_id = format!("{CHANNEL}:{ts}");

    let lines = b10x_loom_plugin::run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true))
        .expect("the host runs once");

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].item.0, item_id);
    assert_eq!(lines[0].outcome, RecordOutcome::Proposed, "{lines:?}");
    assert_eq!(lines[0].proposal.as_deref(), Some(REPLY));
    let detail = lines[0].detail.clone().unwrap_or_default();
    let commission = detail
        .strip_prefix("reply.send awaits approval: commission ")
        .unwrap_or_else(|| panic!("the record names the commission: {detail}"))
        .to_owned();
    assert!(
        invokes_of(&fixture, WRITE).is_empty(),
        "nothing is sent before the grant"
    );
    assert!(
        state
            .join("approvals")
            .join(format!("{commission}.json"))
            .is_file()
    );
    assert_eq!(plugin.held().unwrap(), std::slice::from_ref(&commission));
    let suspensions = FileSuspensionStore::open(&state).unwrap();
    assert!(
        suspensions
            .get(&CommissionId(Uuid(commission.clone())))
            .unwrap()
            .is_some(),
        "the Suspension holds the send"
    );
    assert_eq!(
        plugin.resume(&host, &commission),
        Err(b10x_loom_agent::ResumeFailure::NotGranted),
        "a resume before the grant performs nothing"
    );
    assert!(invokes_of(&fixture, WRITE).is_empty());

    // The person approves, as `b10x-loom approve <COMMISSION> reply.send --state <DIR>` does.
    let grants = GrantStore::open(&state).unwrap();
    let handoffs = MergeHandoffStore::open(&state).unwrap();
    let head_of = |_: &CaseId| Err("no merge here".to_owned());
    let granted = approve(
        &suspensions,
        &grants,
        &handoffs,
        &head_of,
        &commission,
        "reply.send",
        now(),
        None,
    )
    .expect("the grant is written");
    assert_eq!(granted.capability, "reply.send");

    let resumed = plugin
        .resume(&host, &commission)
        .expect("the send is performed after the grant");

    assert_eq!(resumed.classification, "applied");
    assert_eq!(
        resumed.idempotency_key,
        format!("loom-action-request-{}", granted.action_request_id.0.0)
    );
    let writes = invokes_of(&fixture, WRITE);
    assert_eq!(writes.len(), 1, "exactly one write: {:?}", fixture.argv());
    assert_eq!(
        flag(&writes[0], "--idempotency-key"),
        Some(resumed.idempotency_key.as_str())
    );
    let proof = flag(&writes[0], "--approval-file").expect("the write names its proof");
    assert!(
        Path::new(proof).is_file() && proof.starts_with(state.to_str().unwrap()),
        "the proof is the grant file in the state directory: {proof}"
    );
    let input: serde_json::Value = fs::read_to_string(fixture.connectors_state().join("stdin.log"))
        .unwrap()
        .lines()
        .filter(|line| line.contains(REPLY))
        .map(|line| serde_json::from_str(line).unwrap())
        .next()
        .expect("the write's input is on stdin");
    assert_eq!(
        input,
        json!({"channel": CHANNEL, "thread_ts": ts, "text": REPLY}),
        "the input is the admitted arguments"
    );
    assert!(
        suspensions
            .get(&CommissionId(Uuid(commission.clone())))
            .unwrap()
            .is_none(),
        "the Suspension is released"
    );
    assert!(plugin.held().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(state.join(SENDS_FILE))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(
        matches!(
            plugin.resume(&host, &commission),
            Err(b10x_loom_agent::ResumeFailure::NotHeld(_))
        ),
        "a second resume performs nothing"
    );
    assert_eq!(invokes_of(&fixture, WRITE).len(), 1);
    drop(suspensions);
    let _ = FileCaseStore::open(&state).unwrap();
}

#[test]
fn a_poll_performs_the_sends_granted_since() {
    let fixture = fixture("serve-poll-resumes");
    write_fixtures(&fixture, "slack");
    let ts = ts_ago(60);
    channels(&fixture, json!([member(CHANNEL, 5)]));
    support::history(
        &fixture,
        vec![support::message("U0ALICE", &ts, "Is the deploy finished?")],
    );
    let state = fixture.plugin_state();
    let plugin = plugin(
        &fixture,
        spec(&fixture, "approval", &[CHANNEL], 5),
        vec![vec![
            (
                "source_read",
                json!({"source": "docs", "kind": "search", "input": {"query": "deploy"}}),
            ),
            ("reply_propose", json!({"text": REPLY})),
            (
                "reply_send",
                json!({"channel": CHANNEL, "thread_ts": ts, "text": REPLY}),
            ),
        ]],
    );
    let host = Host::new(plugin.config()).with_environment(fixture.environment());
    let lines = b10x_loom_plugin::run_plugin_on(&plugin, &host, &state, &AtomicBool::new(true))
        .expect("the host runs once");
    let commission = plugin.held().unwrap().remove(0);
    assert_eq!(lines.len(), 1);
    let suspensions = FileSuspensionStore::open(&state).unwrap();
    let head_of = |_: &CaseId| Err("no merge here".to_owned());
    approve(
        &suspensions,
        &GrantStore::open(&state).unwrap(),
        &MergeHandoffStore::open(&state).unwrap(),
        &head_of,
        &commission,
        "reply.send",
        now(),
        None,
    )
    .unwrap();

    let lines = run_plugin(&plugin, plugin.config(), &state, &AtomicBool::new(true))
        .expect("the next cycle runs");

    assert!(lines.is_empty(), "the item is handled already: {lines:?}");
    assert_eq!(
        invokes_of(&fixture, WRITE).len(),
        1,
        "the granted send ran at the poll"
    );
    let notes = plugin.take_notes();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with(&format!("sent for commission {commission}: applied")),
        "{notes:?}"
    );
}

#[test]
fn writes_mode_auto_is_refused_before_anything_runs() {
    let fixture = fixture("serve-auto-refused");
    let refused = SpecPlugin::new(
        spec(&fixture, "auto", &[CHANNEL], 5),
        &fixture.plugin_state(),
    );
    match refused {
        Err(PluginError::Config(why)) => assert!(why.contains("auto"), "{why}"),
        other => panic!("auto is not served yet: {:?}", other.map(|_| ())),
    }
    assert!(fixture.argv().is_empty(), "nothing ran");
}

#[test]
fn a_state_projects_its_actions_and_sources() {
    let fixture = fixture("serve-projection");
    let plugin = plugin(
        &fixture,
        spec(&fixture, "approval", &[CHANNEL], 5),
        Vec::new(),
    );
    let host = Host::new(plugin.config());
    let item = item("item-1");
    let ask = plugin.project(&host, &item, &classification(Intent::Ask, &[], "0.9"));
    assert_eq!(
        ask.actions,
        [
            "source.read",
            "reply.propose",
            "reply.send",
            "reply.decline"
        ]
    );
    assert_eq!(ask.sources.len(), 1);
    assert_eq!(ask.sources[0].0, "docs");
    let find = plugin.project(&host, &item, &classification(Intent::Find, &[], "0.9"));
    assert!(find.actions.is_empty(), "no state names Find");
    let task = plugin.project(&host, &item, &classification(Intent::Task, &[], "0.9"));
    assert_eq!(task.actions, ["reply.decline"]);
}

// ---- the authority -------------------------------------------------------------------------------

fn request() -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-000000000101".to_owned())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-000000000301".to_owned())),
        case_id: CaseId("case-1".to_owned()),
        expected_case_revision: 1,
        action: "reply.send".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    }
}

fn commission_data() -> CommissionData {
    CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("loom-agent".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    }
}

#[test]
fn the_spec_authority_grants_exactly_the_policy_capabilities() {
    let approval = SpecAuthority::new(ALL.map(str::to_owned).to_vec(), WriteMode::Approval);
    for capability in ["datasource.read", "reply.propose"] {
        assert!(
            matches!(
                approval
                    .decide(&commission_data(), &request(), capability)
                    .unwrap(),
                AuthorityVerdict::Allow(_)
            ),
            "{capability} is granted"
        );
    }
    assert!(
        matches!(
            approval
                .decide(&commission_data(), &request(), "reply.send")
                .unwrap(),
            AuthorityVerdict::ApprovalRequired(_)
        ),
        "reply.send is held for approval"
    );
    for capability in ["repository.merge", "reply.escalate", "", "datasource.read "] {
        assert!(
            matches!(
                approval
                    .decide(&commission_data(), &request(), capability)
                    .unwrap(),
                AuthorityVerdict::Deny(_)
            ),
            "`{capability}` is denied"
        );
    }
    let read_only = SpecAuthority::new(vec!["datasource.read".to_owned()], WriteMode::Approval);
    assert!(matches!(
        read_only
            .decide(&commission_data(), &request(), "reply.send")
            .unwrap(),
        AuthorityVerdict::Deny(_)
    ));
    assert!(matches!(
        read_only
            .decide(&commission_data(), &request(), "reply.propose")
            .unwrap(),
        AuthorityVerdict::Deny(_)
    ));
}

#[test]
fn the_commission_of_an_item_revision_is_stable() {
    let fixture = fixture("serve-commission-id");
    let plugin = plugin(
        &fixture,
        spec(&fixture, "approval", &[CHANNEL], 5),
        Vec::new(),
    );
    let one = plugin.commission_of(&item("item-1"));
    assert_eq!(one, plugin.commission_of(&item("item-1")));
    assert_ne!(one, plugin.commission_of(&item("item-2")));
    assert_eq!(one.0.len(), 36);
    assert_eq!(&one.0[14..15], "4");
    let _: PathBuf = fixture.plugin_state();
}
