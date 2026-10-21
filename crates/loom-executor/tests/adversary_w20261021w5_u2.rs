// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1, wave 2026-10-21-w5, U2 (`story:delete-harness-port`).
//!
//! 1. `docs/contracts/loom-action-selection.md` is the action-selection contract Metaharness reads
//!    Loom's selection telemetry from. The unit removed `loom.run.Session`, its commands (among
//!    them `loom.run.CountBoundaryRefusal`) and `Loom::run_loop` and `Loom::sessions`, each named
//!    in a `**Breaking:**` bullet of `CHANGELOG.md`. The contract must not keep promising them.
//! 2. `ess/domains/run.yaml` now says `loom.run.LoopStop` is the payload of the `LoopBudget`
//!    suspension a workflow judgement step raises, and `AGENTS.md` that the run carries the stop
//!    "as `loom.run.LoopStop` declares it". The unit deleted the only code that used the generated
//!    `LoopStop` (the ported loop's stop codec); `workflow.rs` writes the payload by hand. This
//!    case drives a judgement step to each budget stop and holds every payload to the declaration,
//!    both ways: each payload is a declared cause with exactly its declared fields, and each
//!    declared cause is one a judgement step raises.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time. The governor, the effect port and
//! the model are scripted in this process; nothing reaches a network.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeSuspended, Frontier, FrontierAction, FrontierData, FrontierId, GovernorError,
    PrincipalId, SuspensionReason, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_executor::workflow::{self, RecordingEffects, WorkflowExecutor};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error as ModelError, Id, Item, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use serde_json::json;
use serde_yaml_ng::Value as Yaml;

/// The repository root of the tree this test is run from.
fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    PathBuf::from(manifest).join("../..")
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every backticked span of `line`.
fn backticked(line: &str) -> Vec<&str> {
    line.split('`').skip(1).step_by(2).collect()
}

/// Every `name:` value `ess/domains/run.yaml` holds: its declarations and their fields.
fn run_yaml_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start().trim_start_matches("- ").trim_start();
            line.strip_prefix("name: ")
        })
        .map(|name| name.trim().to_owned())
        .collect()
}

/// The `Loom::<method>` paths the `## [Unreleased]` entry of `CHANGELOG.md` names as removed, in
/// its `**Breaking:**` bullets.
fn removed_loom_methods(changelog: &str) -> BTreeSet<String> {
    let unreleased = changelog
        .split("## [Unreleased]")
        .nth(1)
        .and_then(|rest| rest.split("\n## [").next())
        .expect("CHANGELOG.md has an [Unreleased] entry");
    unreleased
        .lines()
        .filter(|line| line.starts_with("- **Breaking:**") && line.contains("removed"))
        .flat_map(|line| {
            backticked(line)
                .into_iter()
                .filter(|span| span.starts_with("Loom::"))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// (1) The action-selection contract names no `loom.run` declaration the run model no longer
/// declares, no `Loom` method the release removes, and no session, now that `loom.run` has none.
#[test]
fn adversary_w20261021w5_u2_the_selection_contract_names_nothing_the_release_removed() {
    const CONTRACT: &str = "docs/contracts/loom-action-selection.md";
    let contract = read(CONTRACT);
    let run_yaml = read("ess/domains/run.yaml");
    let declared = run_yaml_names(&run_yaml);
    let removed = removed_loom_methods(&read("CHANGELOG.md"));
    assert!(
        removed.contains("Loom::run_loop") && removed.contains("Loom::sessions"),
        "precondition: CHANGELOG.md's [Unreleased] entry removes Loom::run_loop and \
         Loom::sessions: {removed:?}"
    );
    assert!(
        !declared.contains("loom.run.Session") && !declared.contains("boundary_refusals"),
        "precondition: ess/domains/run.yaml declares no loom.run.Session and no \
         boundary_refusals"
    );

    let mut stale = Vec::new();
    for (index, line) in contract.lines().enumerate() {
        let at = format!("{CONTRACT}:{}", index + 1);
        for span in backticked(line) {
            if span.starts_with("loom.run.") && !declared.contains(span) {
                stale.push(format!(
                    "{at}: `{span}` is not declared by ess/domains/run.yaml"
                ));
            }
            if removed.contains(span) {
                stale.push(format!(
                    "{at}: `{span}` is removed by a **Breaking:** bullet of CHANGELOG.md"
                ));
            }
            if span == "boundary_refusals" {
                stale.push(format!(
                    "{at}: `{span}` was a field of loom.run.Session, which is removed"
                ));
            }
        }
        if line.to_lowercase().contains("session") {
            stale.push(format!(
                "{at}: names a session, which loom.run no longer declares: {}",
                line.trim()
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "the action-selection contract still promises what the release removed:\n{}",
        stale.join("\n")
    );
}

const CASE: &str = "CASE-ADV-W5-U2";
const MERGE: &str = "repository.merge";
const LIMIT: u64 = 100;

/// One judgement step whose one action is the merge.
const DEFINITION: &str = r#"
name: decide-only
protocol: software-change@1
steps:
  - name: decide
    kind: Judgement
    actions: [repository.merge]
    instructions: Merge when the change is ready.
"#;

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn protocol(name: &str) -> Option<Vec<String>> {
    (name == "software-change@1").then(|| vec![MERGE.to_owned()])
}

/// A case whose frontier lists the merge admissible, at revision 0, and never moves.
struct Case;

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(0)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf00)),
            case_id: case.clone(),
            case_revision: 0,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

/// An effect port nothing reaches here: the judgement call suspends before any effect.
struct NoEffects;

impl EffectPort for NoEffects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// A scripted llm `Model`: each request answers with the next scripted call of the tool it is
/// held to, reporting these input tokens as final usage, or no usage at all for `None`.
struct ScriptedModel {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(serde_json::Value, Option<u64>)>>,
}

impl ScriptedModel {
    fn new(script: Vec<(serde_json::Value, Option<u64>)>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("scripted"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("scripted-model"),
                binding_revision: id("scripted-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            script: Mutex::new(script.into()),
        }
    }

    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, ModelError> {
        let ToolChoice::Named(tool) = &request.tool_choice else {
            return Err(ModelError::protocol("the request is held to no tool"));
        };
        let (arguments, input_tokens) = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| ModelError::protocol("the script has no further turn"))?;
        Ok(TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![Item::ToolCall(ToolCall {
                call_id: CallId::new("call-1").expect("valid"),
                name: tool.clone(),
                arguments,
            })],
            observation: TurnObservation {
                final_usage: input_tokens.is_some(),
                usage: input_tokens.map(|tokens| Usage {
                    input_tokens: Some(tokens),
                    output_tokens: Some(5),
                    ..Usage::default()
                }),
                ..TurnObservation::new(self.provenance.clone())
            },
        })
    }
}

impl Model for ScriptedModel {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, ModelError>> {
        let answered = self.answer(request);
        Box::pin(async move { answered })
    }
}

/// The `LoopBudget` payload of one judgement call held to [`LIMIT`] input tokens, on a model
/// answering its selection request with `input_tokens`.
fn budget_stop(input_tokens: Option<u64>) -> CommissionValue {
    let case = Case;
    let model = ScriptedModel::new(vec![
        (json!({ "action": MERGE }), input_tokens),
        (json!({}), Some(1)),
    ]);
    let effects = RecordingEffects::new(NoEffects);
    let definition = workflow::load(DEFINITION, &protocol).expect("the definition loads");
    let executor = WorkflowExecutor::new(&case, &model, definition, effects.log())
        .with_max_input_tokens(LIMIT);
    let frontier = case
        .frontier(&CaseId(CASE.to_owned()))
        .expect("the case has a frontier");
    match executor.run(&commission(), &frontier) {
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::LoopBudget(stop),
        }) => stop,
        other => panic!("precondition: the call suspends for its budget, not {other:?}"),
    }
}

/// `loom.run.LoopStop` as `ess/domains/run.yaml` declares it: each cause's tag, with the fields of
/// its payload struct by name and declared type.
fn declared_loop_stop(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let spec: Yaml = serde_yaml_ng::from_str(text).expect("run.yaml is YAML");
    let types = spec["types"].as_sequence().expect("run.yaml has types");
    let named = |name: &str| {
        types
            .iter()
            .find(|declared| declared["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("run.yaml declares no type `{name}`"))
    };
    let union = named("loom.run.LoopStop");
    assert_eq!(
        union["tag"].as_str(),
        Some("kind"),
        "LoopStop is tagged `kind`"
    );
    union["variants"]
        .as_mapping()
        .expect("LoopStop declares its variants")
        .iter()
        .map(|(tag, payload)| {
            let tag = tag.as_str().expect("a variant tag is text").to_owned();
            let fields = payload.as_str().map_or_else(BTreeMap::new, |name| {
                named(name)["fields"]
                    .as_sequence()
                    .unwrap_or_else(|| panic!("`{name}` declares fields"))
                    .iter()
                    .map(|field| {
                        (
                            field["name"].as_str().expect("a field name").to_owned(),
                            field["type"].as_str().expect("a field type").to_owned(),
                        )
                    })
                    .collect()
            });
            (tag, fields)
        })
        .collect()
}

/// Why `payload` is not a `loom.run.LoopStop` as `declared` declares it, if it is not; else its tag.
fn as_declared(
    payload: &CommissionValue,
    declared: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<String, String> {
    let CommissionValue::Object(members) = payload else {
        return Err(format!("{payload:?} is not an object"));
    };
    let members: BTreeMap<&str, &CommissionValue> =
        members.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let Some(CommissionValue::Text(tag)) = members.get("kind") else {
        return Err(format!("{payload:?} has no text `kind`"));
    };
    let Some(fields) = declared.get(tag) else {
        return Err(format!(
            "`{tag}` is no cause loom.run.LoopStop declares ({:?})",
            declared.keys().collect::<Vec<_>>()
        ));
    };
    let carried: BTreeSet<&str> = members.keys().copied().filter(|k| *k != "kind").collect();
    let wanted: BTreeSet<&str> = fields.keys().map(String::as_str).collect();
    if carried != wanted {
        return Err(format!(
            "`{tag}` carries {carried:?}; loom.run.LoopStop declares {wanted:?}"
        ));
    }
    for (name, kind) in fields {
        let value = members[name.as_str()];
        let fits = match kind.as_str() {
            "Integer" => {
                matches!(value, CommissionValue::Number(n) if n.parse::<i64>().is_ok())
            }
            "String" => matches!(value, CommissionValue::Text(_)),
            other => {
                return Err(format!(
                    "`{tag}.{name}` has a type this case reads not: {other}"
                ));
            }
        };
        if !fits {
            return Err(format!(
                "`{tag}.{name}` is {value:?}, not a declared {kind}"
            ));
        }
    }
    Ok(tag.clone())
}

/// (2) Each budget stop a judgement step raises is a cause `loom.run.LoopStop` declares, with
/// exactly its declared fields, and every cause it declares is one a judgement step raises.
#[test]
fn adversary_w20261021w5_u2_each_judgement_budget_stop_is_a_declared_loop_stop_and_no_other() {
    let declared = declared_loop_stop(&read("ess/domains/run.yaml"));
    let raised = [
        ("150 input tokens against 100", budget_stop(Some(150))),
        ("no final input-token count", budget_stop(None)),
    ];

    let mut tags = BTreeSet::new();
    let mut refused = Vec::new();
    for (scenario, payload) in &raised {
        match as_declared(payload, &declared) {
            Ok(tag) => {
                tags.insert(tag);
            }
            Err(why) => refused.push(format!("{scenario}: {why}")),
        }
    }
    assert!(
        refused.is_empty(),
        "a judgement step's LoopBudget payload is not loom.run.LoopStop as declared:\n{}",
        refused.join("\n")
    );
    let unraised: Vec<&String> = declared.keys().filter(|tag| !tags.contains(*tag)).collect();
    assert!(
        unraised.is_empty(),
        "loom.run.LoopStop declares causes no judgement step raises: {unraised:?}"
    );
}
