//! Serving a `loom.agent/1` document: `b10x-loom agent serve --spec <FILE> --state <DIR>`
//! (`story:agent-serve`).
//!
//! [`SpecPlugin`] is a [`Plugin`] of the plugin host built from a spec: the host runs it with
//! `run_plugin`, so every rule of the host (one attempt counted before it starts, three failures,
//! the record and the state directory's lock) holds. What the spec declares is lowered, nothing
//! is interpreted at run time:
//!
//! - The poll is the slack-handler's over the channel binding's `list`, `get` and `search`
//!   operations, walking only the channels `channel.allow` lists: a message in any other channel
//!   is no item, and no history of it is read.
//! - A turn runs on the inner loop's protocol (`operator-reply@1`), under the state whose `when`
//!   names the item's intent: its actions and sources are the projection. No state for the intent
//!   is a turn that stops. [`SpecAuthority`] grants exactly `policy.capabilities` and denies every
//!   other; in `writes.mode: approval` it holds `reply.send` for a person's approval.
//! - `reply.send` is a governed write ([`b10x_loom_plugin::SendPolicy`], performed by the effect
//!   port through `ConnectorsCli::invoke_write`) on the channel binding's `write` operation, to
//!   the item's channel, of the recorded proposal, under `writes.max_per_hour` counted in
//!   `<state>/sends.jsonl`. In `approval` mode the Run suspends on it: the case and the Suspension
//!   are kept in the state directory, the turn ends `proposed` naming the commission, and the
//!   held send is written to `<state>/approvals/<commission>.json`. `b10x-loom approve
//!   <COMMISSION> reply.send --state <DIR>` writes the grant; [`SpecPlugin::resume`] (`b10x-loom
//!   agent resume`, and every poll of a serving host) then performs the send once through
//!   `resume_after_grant` with the grant file as the write's proof, and releases the Suspension.
//!   `writes.mode: auto` is not served yet and is refused before anything runs; `none` sends
//!   nothing, as the effect port refuses every send with `WritesNone`.
//!
//! The commission of a turn is derived from the agent id, the item id and its revision, so a
//! person names it from the record line alone.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_plugin::effects::{SENDS_FILE, detail};
use b10x_loom_plugin::project::SEND;
use b10x_loom_plugin::turn::stopped;
use b10x_loom_plugin::{
    Classification, DataSourceEffects, Host, InboundItem, Objective, Plugin, PluginConfig,
    PluginError, PluginState, Poll, Projection, Proposal, SendPolicy, TurnModel, TurnOptions,
    TurnResult, turn_with,
};
use b10x_loom_plugin_slack::{SlackConfig, SlackHandler, TurnModels};
use chrono::{SecondsFormat, Utc};
use llm_core::Model;
use loom::datasource::{DataSource, WriteOperation};
use loom_sdk::agent::{AdmitSendOutcome, AgentSpec, Binding, ConnectorsBinding, WriteMode};
use loom_sdk::commission::model::json::Value;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, ObservationId, PrincipalId, RunId, Unit,
    commission_state, frontier_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::executor::AgentExecutor;
use loom_sdk::commission::ports::suspension::SuspensionPort;
use loom_sdk::commission::runtime::{ResumeError, resume_after_grant};
use loom_sdk::governor::{FileCaseStore, FileSuspensionStore};
use loom_sdk::intake::slice::grant::{self as grant, GrantAuthority, GrantStore};
use loom_sdk::{CanonGovernor, LoopContext, ProtocolCatalog};

pub use b10x_loom_plugin::admit_send;
pub use b10x_loom_plugin::effects::send_refusal_spelling;

pub mod a2a;

/// The directory in the state directory holding one file per send awaiting approval.
pub const APPROVALS_DIR: &str = "approvals";

/// The principal every served turn's commission runs for.
pub const PRINCIPAL: &str = "loom-agent";

/// The capability `reply.send` needs.
pub const SEND_CAPABILITY: &str = "reply.send";

/// A plugin of the host built from a `loom.agent/1` document (see the module documentation).
pub struct SpecPlugin {
    spec: AgentSpec,
    config: PluginConfig,
    slack: SlackHandler,
    state: PathBuf,
    protocol: String,
    authority: SpecAuthority,
    send: Option<Send>,
    classifier: Option<Box<dyn Model>>,
    turn_models: Option<TurnModels>,
    notes: Mutex<Vec<String>>,
}

/// What a send needs beyond the policy of one turn.
struct Send {
    operation: WriteOperation,
    allow: Vec<String>,
    mode: WriteMode,
    max_per_hour: i64,
}

impl SpecPlugin {
    /// The plugin of `spec`, keeping cases, Suspensions, the send counter and held sends in
    /// `state`, the host's state directory. The spec is checked against the plugin catalog first
    /// ([`crate::check`]).
    ///
    /// # Errors
    /// [`PluginError::Config`] for a spec the check refuses, or one this host cannot serve:
    /// no `runtime`, no `loops`, no `channel`, a channel binding that is not a connectors binding
    /// with `list`, `get` and `search`, or `writes.mode: auto`.
    pub fn new(spec: AgentSpec, state: &Path) -> Result<Self, PluginError> {
        let config = |why: String| PluginError::Config(format!("the agent spec: {why}"));
        let catalog = ProtocolCatalog::plugins().map_err(PluginError::Config)?;
        let actions = |name: &str| catalog.get(name).map(|entry| entry.actions());
        crate::check(&spec, &actions).map_err(|refused| config(crate::describe(&refused)))?;
        let runtime = spec
            .runtime
            .as_ref()
            .ok_or_else(|| config("`runtime` names no connectors command line".to_owned()))?;
        let loops = spec
            .loops
            .as_ref()
            .ok_or_else(|| config("`loops` is undeclared; nothing would poll".to_owned()))?;
        let channel = spec
            .channel
            .as_ref()
            .ok_or_else(|| config("`channel` is undeclared; nothing would poll".to_owned()))?;
        let binding = spec
            .bindings
            .iter()
            .find_map(|binding| match binding {
                Binding::Connectors(binding) if binding.name == channel.binding => Some(binding),
                _ => None,
            })
            .ok_or_else(|| {
                config(format!(
                    "`channel.binding` `{}` is no connectors binding",
                    channel.binding.0
                ))
            })?;
        let operation = |name: &str, held: &Option<loom::datasource::OperationId>| {
            held.clone().ok_or_else(|| {
                config(format!(
                    "the channel binding `{}` declares no `{name}` operation",
                    binding.name.0
                ))
            })
        };
        let list = operation("list", &binding.list)?;
        let history = operation("get", &binding.get)?;
        let replies = operation("search", &binding.search)?;
        let mode = spec
            .writes
            .as_ref()
            .map_or(WriteMode::None, |writes| writes.mode);
        if mode == WriteMode::Auto {
            return Err(config(
                "`writes.mode: auto` is not served yet; declare `approval` or `none`".to_owned(),
            ));
        }
        let config = PluginConfig {
            connectors: runtime.connectors.clone(),
            sources: spec
                .bindings
                .iter()
                .filter_map(|binding| match binding {
                    Binding::Connectors(binding) => Some(source(binding)),
                    Binding::Http(_) => None,
                })
                .collect(),
            objectives: Vec::new(),
            classify_threshold: loops.inner.classify_threshold.clone(),
            poll_interval_seconds: loops.outer.poll_interval_seconds,
            workspace_roots: Vec::new(),
            checkouts: Vec::new(),
        };
        let slack = SlackHandler::new(SlackConfig {
            plugin: config.clone(),
            adapter: binding.adapter.clone(),
            connection: binding.connection.clone(),
            list_channels: list,
            history,
            replies,
            bot_user_id: channel.bot_user_id.clone(),
            min_age_minutes: loops.outer.min_age_minutes,
            seed: 1,
            lookback_minutes: loops.outer.lookback_minutes,
            channels: Vec::new(),
        })?
        .with_channel_allow(channel.allow.clone());
        let send = binding.write.clone().map(|write| Send {
            operation: WriteOperation {
                action: SEND.to_owned(),
                adapter: binding.adapter.clone(),
                connection: binding.connection.clone(),
                operation: write,
            },
            allow: channel.allow.clone(),
            mode,
            max_per_hour: spec.writes.as_ref().map_or(0, |writes| writes.max_per_hour),
        });
        let authority = SpecAuthority {
            capabilities: spec
                .policy
                .as_ref()
                .map(|policy| policy.capabilities.clone())
                .unwrap_or_default(),
            mode,
        };
        Ok(Self {
            protocol: loops.inner.protocol.clone(),
            spec,
            config,
            slack,
            state: std::path::absolute(state).map_err(|error| {
                PluginError::State(format!(
                    "{} cannot be made absolute: {error}",
                    state.display()
                ))
            })?,
            authority,
            send,
            classifier: None,
            turn_models: None,
            notes: Mutex::new(Vec::new()),
        })
    }

    /// Classifies items with `model`.
    #[must_use]
    pub fn with_classifier(mut self, model: Box<dyn Model>) -> Self {
        self.classifier = Some(model);
        self
    }

    /// Runs each turn on a model `models` makes.
    #[must_use]
    pub fn with_turn_models(mut self, models: TurnModels) -> Self {
        self.turn_models = Some(models);
        self
    }

    /// The spec.
    pub fn spec(&self) -> &AgentSpec {
        &self.spec
    }

    /// The host's configuration lowered from the spec.
    pub fn config(&self) -> &PluginConfig {
        &self.config
    }

    /// The authority of every turn.
    pub fn authority(&self) -> &SpecAuthority {
        &self.authority
    }

    /// The state directory.
    pub fn state(&self) -> &Path {
        &self.state
    }

    /// What the host did beside the record, one line each: the sends it performed or refused at
    /// a poll after their approval. Taking them empties the list.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes())
    }

    fn notes(&self) -> MutexGuard<'_, Vec<String>> {
        self.notes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The commission id of a turn on `item`: derived from the agent id, the item id and its
    /// revision.
    pub fn commission_of(&self, item: &InboundItem) -> Uuid {
        commission_id(&self.spec.agent.id.0, &item.id.0, &item.revision)
    }

    /// The directory of held sends.
    pub fn approvals(&self) -> PathBuf {
        self.state.join(APPROVALS_DIR)
    }

    fn held_path(&self, commission: &str) -> PathBuf {
        self.approvals().join(format!("{commission}.json"))
    }

    /// The send policy of a turn (`granted` false) or of a resume (`granted` true, with the held
    /// `proposal`), with `proof` as the write's proof file.
    fn policy(
        &self,
        granted: bool,
        proposal: Option<Proposal>,
        proof: &Path,
    ) -> Option<SendPolicy> {
        self.send.as_ref().map(|send| SendPolicy {
            operation: send.operation.clone(),
            allow: send.allow.clone(),
            mode: send.mode,
            max_per_hour: send.max_per_hour,
            granted,
            counter: self.state.join(SENDS_FILE),
            proof_file: proof.display().to_string(),
            proposal,
        })
    }

    /// The proof a granted send names: the grant file `b10x-loom approve` wrote.
    fn grant_file(&self, commission: &str) -> PathBuf {
        let mut name = String::from("grant-");
        for byte in commission.as_bytes() {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        self.state.join(name)
    }

    /// Every send held for approval: the commissions whose held file exists.
    ///
    /// # Errors
    /// The approvals directory exists and cannot be listed.
    pub fn held(&self) -> Result<Vec<String>, String> {
        let dir = self.approvals();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("{}: {error}", dir.display())),
        };
        let mut held = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
            if let Some(name) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
            {
                held.push(name.to_owned());
            }
        }
        held.sort();
        Ok(held)
    }

    /// Performs every held send a person granted since (`b10x-loom approve`), each once, and
    /// keeps one note per attempt.
    fn resume_granted(&self, host: &Host<'_>) {
        let held = match self.held() {
            Ok(held) => held,
            Err(why) => {
                self.notes().push(format!("held sends: {why}"));
                return;
            }
        };
        for commission in held {
            let granted = GrantStore::open(&self.state)
                .and_then(|grants| grants.get(&grant_id(&commission)))
                .map(|grant| grant.is_some());
            match granted {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    self.notes()
                        .push(format!("commission {commission}: the grant store: {error}"));
                    continue;
                }
            }
            let note = match self.resume(host, &commission) {
                Ok(resumed) => resumed.to_string(),
                Err(failure) => format!("commission {commission}: {failure}"),
            };
            self.notes().push(note);
        }
    }

    /// Performs the send `commission`'s Suspension holds, once, after its grant: the write goes
    /// through `resume_after_grant` with `GrantAuthority` over the grant, the effect port under
    /// the send policy (`granted`), and the grant file as the proof. The held file is removed
    /// once the resume ran, whatever the write came to.
    ///
    /// # Errors
    /// [`ResumeFailure::NotHeld`] when no send of `commission` is held here;
    /// [`ResumeFailure::NotGranted`] before the grant, with nothing performed;
    /// [`ResumeFailure::Refused`] when the runtime refused the resume (nothing suspended, in
    /// doubt, the case moved); [`ResumeFailure::Failed`] for a store or loop failure.
    pub fn resume(&self, host: &Host<'_>, commission: &str) -> Result<Resumed, ResumeFailure> {
        let failed = |why: String| ResumeFailure::Failed(why);
        let path = self.held_path(commission);
        let held = read_held(&path).map_err(ResumeFailure::NotHeld)?;
        let grants = GrantStore::open(&self.state).map_err(|e| failed(e.to_string()))?;
        let grant = grants
            .get(&grant_id(commission))
            .map_err(|e| failed(e.to_string()))?
            .ok_or(ResumeFailure::NotGranted)?;
        let suspensions =
            FileSuspensionStore::open(&self.state).map_err(|e| failed(e.to_string()))?;
        let stored = suspensions
            .get(&CommissionId(Uuid(commission.to_owned())))
            .map_err(|e| failed(e.to_string()))?
            .ok_or_else(|| ResumeFailure::Refused("nothing is suspended".to_owned()))?;
        let catalog = ProtocolCatalog::plugins().map_err(failed)?;
        let governor = CanonGovernor::new(
            FileCaseStore::open(&self.state).map_err(|e| failed(e.to_string()))?,
        )
        .with_catalog(&catalog)
        .map_err(|e| failed(e.to_string()))?;
        let case = stored.data.case_id.clone();
        let proof = self.grant_file(commission);
        let policy = self
            .policy(true, Some(held.proposal.clone()), &proof)
            .ok_or_else(|| failed("the channel binding declares no write".to_owned()))?;
        // Decided before any process: a refusal here performs nothing and keeps the send held,
        // with its Suspension and the person's grant, for a later resume.
        let channel = detail(&held.item, "channel").unwrap_or_default();
        let refused = match policy.admission(&channel, true).map_err(failed)? {
            AdmitSendOutcome::Admitted => None,
            AdmitSendOutcome::Held => Some("Held".to_owned()),
            AdmitSendOutcome::WritesNone { error }
            | AdmitSendOutcome::OutsideAllowList { error }
            | AdmitSendOutcome::NoProposal { error }
            | AdmitSendOutcome::RateLimited { error } => {
                Some(send_refusal_spelling(error.reason).to_owned())
            }
        };
        if let Some(reason) = refused {
            return Err(ResumeFailure::Refused(format!(
                "{reason}: nothing was sent; the send stays held"
            )));
        }
        let projection = Projection {
            actions: vec![SEND.to_owned()],
            sources: Vec::new(),
        };
        let effects = DataSourceEffects::new(
            &governor,
            case.clone(),
            &held.item,
            host.connectors_over(Vec::new()),
            &[],
            &projection,
        )
        .with_send(policy);
        let authority = GrantAuthority::new(Some(grant), &suspensions);
        let ids = Ids::default();
        let commission_data = Commission::<commission_state::Assigned>::new(CommissionData {
            commission_id: CommissionId(Uuid(commission.to_owned())),
            agent_revision_id: AgentRevisionId(ids.next('f')),
            case_id: case,
            principal: PrincipalId(PRINCIPAL.to_owned()),
            authority_context: AuthorityContext(Value::Null),
        });
        let run_ids = AtomicU64::new(1);
        let mut runs =
            RunStore::new(move || RunId(uuid('f', 0x100 + run_ids.fetch_add(1, Ordering::SeqCst))));
        let end = resume_after_grant(
            &governor,
            &Settles,
            &authority,
            &effects,
            &commission_data,
            &mut runs,
            &suspensions,
            &mut Clock {
                ids: &ids,
                budget: 2,
            },
        );
        match end {
            Ok(_) => {}
            // Nothing was performed; the send stays held for a later resume.
            Err(ResumeError::Refused(refusal)) => {
                return Err(ResumeFailure::Refused(format!("{refusal:?}")));
            }
            Err(error) => return Err(failed(error.to_string())),
        }
        // The request was performed or definitively refused under the process: nothing is held.
        let _ = std::fs::remove_file(&path);
        match effects.sent() {
            Some(sent) => Ok(Resumed {
                commission: commission.to_owned(),
                classification: classification_name(sent.classification).to_owned(),
                idempotency_key: sent.idempotency_key,
                request_id: sent.request_id,
            }),
            None => Err(ResumeFailure::Refused(
                effects
                    .transcript()
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "the send was not performed".to_owned()),
            )),
        }
    }
}

impl Plugin for SpecPlugin {
    fn name(&self) -> &str {
        &self.spec.agent.id.0
    }

    fn poll(
        &self,
        host: &Host<'_>,
        state: &PluginState,
        objectives: &[Objective],
    ) -> Result<Poll, PluginError> {
        self.resume_granted(host);
        self.slack.poll(host, state, objectives)
    }

    fn project(
        &self,
        _host: &Host<'_>,
        _item: &InboundItem,
        classification: &Classification,
    ) -> Projection {
        let configured = |name: &loom::datasource::SourceName| {
            self.config
                .sources
                .iter()
                .any(|source| source.name == *name)
        };
        self.spec
            .states
            .iter()
            .find(|state| state.when.intent == classification.intent)
            .map_or_else(
                || Projection {
                    actions: Vec::new(),
                    sources: Vec::new(),
                },
                |state| Projection {
                    actions: state.actions.clone(),
                    sources: state
                        .sources
                        .iter()
                        .filter(|source| configured(source))
                        .cloned()
                        .collect(),
                },
            )
    }

    fn turn(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
        projection: &Projection,
    ) -> Result<TurnResult, PluginError> {
        if projection.actions.is_empty() {
            return Ok(stopped(
                Vec::new(),
                "no state of the spec admits this intent",
            ));
        }
        let state = |error: &dyn fmt::Display| PluginError::State(error.to_string());
        let store = FileCaseStore::open(&self.state).map_err(|e| state(&e))?;
        let suspensions = FileSuspensionStore::open(&self.state).map_err(|e| state(&e))?;
        let commission = self.commission_of(item);
        let inner = self.spec.loops.as_ref().map(|loops| &loops.inner);
        let step_budget = inner
            .and_then(|inner| usize::try_from(inner.step_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(b10x_loom_plugin::turn::TURN_STEP_BUDGET);
        let model_budget = inner
            .and_then(|inner| u64::try_from(inner.model_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(b10x_loom_plugin::turn::TURN_MODEL_BUDGET);
        let proof = self.grant_file(&commission.0);
        let turned = turn_with(
            host,
            self.turn_model()?,
            item,
            classification,
            projection,
            TurnOptions {
                protocol: &self.protocol,
                authority: &self.authority,
                store,
                suspensions: Some(&suspensions),
                send: self.policy(false, None, &proof),
                commission: Some(commission.clone()),
                step_budget,
                model_budget,
                events: None,
            },
        )?;
        if turned.awaiting_approval
            && let Some(text) = &turned.result.proposal
        {
            write_held(&self.held_path(&commission.0), item, text).map_err(PluginError::State)?;
        }
        Ok(turned.result)
    }

    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        self.classifier.as_deref().ok_or_else(|| {
            PluginError::Unavailable(format!(
                "the agent `{}` was given no classifier model",
                self.spec.agent.id.0
            ))
        })
    }

    fn turn_model(&self) -> Result<TurnModel<'_>, PluginError> {
        match &self.turn_models {
            Some(models) => models(),
            None => Err(PluginError::Unavailable(format!(
                "the agent `{}` was given no turn model",
                self.spec.agent.id.0
            ))),
        }
    }
}

/// The authority of a served turn: grants exactly the spec's `policy.capabilities`, holding
/// `reply.send` for a person's approval in `writes.mode: approval`, and denies every other
/// capability. It decides on the capability alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecAuthority {
    capabilities: Vec<String>,
    mode: WriteMode,
}

impl SpecAuthority {
    /// The authority granting `capabilities` under `mode`.
    pub fn new(capabilities: Vec<String>, mode: WriteMode) -> Self {
        Self { capabilities, mode }
    }

    /// The capabilities granted.
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }
}

impl AuthorityProvider for SpecAuthority {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        if !self.capabilities.iter().any(|held| held == capability) {
            return Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!("the agent spec grants no `{capability}`"),
            }));
        }
        if capability == SEND_CAPABILITY && self.mode == WriteMode::Approval {
            return Ok(AuthorityVerdict::ApprovalRequired(
                AuthorityVerdictApprovalRequired {
                    request: format!("approval for `{capability}`"),
                },
            ));
        }
        Ok(AuthorityVerdict::Allow(Unit(true)))
    }
}

/// What a resumed send came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumed {
    /// The commission.
    pub commission: String,
    /// The mutation classification Connectors answered (`applied`, `not_attempted`, `refused`,
    /// `unknown`).
    pub classification: String,
    /// The idempotency key the write carried.
    pub idempotency_key: String,
    /// Connectors' request id, when it answered one.
    pub request_id: Option<String>,
}

impl fmt::Display for Resumed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sent for commission {}: {} (idempotency key {})",
            self.commission, self.classification, self.idempotency_key
        )
    }
}

/// Why [`SpecPlugin::resume`] performed no send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeFailure {
    /// No send of the commission is held here: the held file is absent or unreadable.
    NotHeld(String),
    /// No grant for the commission: `b10x-loom approve <COMMISSION> reply.send` first.
    NotGranted,
    /// The runtime refused the resume, or the effect refused the send.
    Refused(String),
    /// A store or the loop failed.
    Failed(String),
}

impl fmt::Display for ResumeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotHeld(why) => write!(f, "no send is held: {why}"),
            Self::NotGranted => f.write_str(
                "the send is not granted yet; approve it with `b10x-loom approve <COMMISSION> \
                 reply.send --state <DIR>`",
            ),
            Self::Refused(why) => write!(f, "the send was not performed: {why}"),
            Self::Failed(why) => write!(f, "the resume failed: {why}"),
        }
    }
}

impl std::error::Error for ResumeFailure {}

/// A held send: the item and the proposal, as `<state>/approvals/<commission>.json` keeps them.
struct Held {
    item: InboundItem,
    proposal: Proposal,
}

fn write_held(path: &Path, item: &InboundItem, text: &str) -> Result<(), String> {
    let failed = |error: &dyn fmt::Display| format!("{}: {error}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    }
    let mut details = String::new();
    loom::json::push_value(&mut details, &item.details);
    let details: serde_json::Value = serde_json::from_str(&details).map_err(|e| failed(&e))?;
    let document = serde_json::json!({
        "held_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "item": {
            "id": item.id.0,
            "revision": item.revision,
            "text": item.text,
            "details": details,
        },
        "proposal": text,
    });
    let mut text = serde_json::to_string_pretty(&document).map_err(|e| failed(&e))?;
    text.push('\n');
    std::fs::write(path, text).map_err(|e| failed(&e))
}

fn read_held(path: &Path) -> Result<Held, String> {
    let failed = |error: &dyn fmt::Display| format!("{}: {error}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| failed(&e))?;
    let document: serde_json::Value = serde_json::from_str(&text).map_err(|e| failed(&e))?;
    let member = |value: &serde_json::Value, name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| failed(&format!("`{name}` is missing")))
    };
    let item = &document["item"];
    let details = loom::json::parse(&item["details"].to_string())
        .map_err(|e| failed(&format!("the item's details: {e:?}")))?;
    let id = member(item, "id")?;
    Ok(Held {
        item: InboundItem {
            id: b10x_loom_plugin::ItemId(id.clone()),
            revision: member(item, "revision")?,
            text: member(item, "text")?,
            details,
        },
        proposal: Proposal {
            item: b10x_loom_plugin::ItemId(id),
            text: member(&document, "proposal")?,
        },
    })
}

/// `commission` as the grant store names it.
fn grant_id(commission: &str) -> grant::CommissionId {
    grant::CommissionId(intake_model::primitives::Uuid(commission.to_owned()))
}

/// The data source of a connectors binding.
fn source(binding: &ConnectorsBinding) -> DataSource {
    DataSource {
        name: binding.name.clone(),
        adapter: binding.adapter.clone(),
        connection: binding.connection.clone(),
        list: binding.list.clone(),
        search: binding.search.clone(),
        get: binding.get.clone(),
    }
}

/// The name of a mutation classification.
fn classification_name(classification: loom::datasource::MutationClassification) -> &'static str {
    match classification {
        loom::datasource::MutationClassification::NotAttempted => "not_attempted",
        loom::datasource::MutationClassification::Refused => "refused",
        loom::datasource::MutationClassification::Applied => "applied",
        loom::datasource::MutationClassification::Unknown => "unknown",
    }
}

/// The commission id of a turn on `item` at `revision` for `agent`: two FNV-1a hashes of the
/// three, shaped as a version-4 UUID, so the same item revision always names the same commission.
pub fn commission_id(agent: &str, item: &str, revision: &str) -> Uuid {
    let key = format!("{agent}\0{item}\0{revision}");
    let high = fnv1a(key.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let low = fnv1a(key.as_bytes(), 0x84222325_cbf29ce4);
    let bytes: Vec<u8> = high
        .to_be_bytes()
        .into_iter()
        .chain(low.to_be_bytes())
        .collect();
    let hex: Vec<String> = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let hex = hex.concat();
    Uuid(format!(
        "{}-{}-4{}-8{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    ))
}

fn fnv1a(bytes: &[u8], basis: u64) -> u64 {
    let mut hash = basis;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// An executor that proposes nothing: a resume performs the held request and ends.
struct Settles;

impl AgentExecutor for Settles {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        ExecutorOutcome::NoUsefulAction(Unit(true))
    }
}

/// Ids for one resume, unique within it.
#[derive(Default)]
struct Ids {
    next: AtomicU64,
}

impl Ids {
    fn next(&self, kind: char) -> Uuid {
        uuid(kind, self.next.fetch_add(1, Ordering::SeqCst) + 1)
    }
}

fn uuid(kind: char, n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-9{kind}00-{n:012x}"))
}

struct Clock<'a> {
    ids: &'a Ids,
    budget: usize,
}

impl LoopContext for Clock<'_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(self.ids.next('a'))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(self.ids.next('a'))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true))
    }

    fn step_budget(&self) -> Option<usize> {
        Some(self.budget)
    }
}
