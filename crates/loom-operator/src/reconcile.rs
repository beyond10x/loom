//! The controller: one reconcile of a `LoomAgent`, and the loop that runs it over every resource
//! of the configured namespace.
//!
//! The lifecycle is the generated behaviour of `loom.resource` over
//! [`b10x_loom_resource::LoomAgentRecord`], held in the controller's memory. One reconcile:
//!
//! 1. A resource being deleted is forgotten (`DeleteLoomAgent`); its children go by garbage
//!    collection, through their owner reference. A resource gone from the watch is forgotten the
//!    same way.
//! 2. `ApplyLoomAgent` at the resource's generation: `created` or `respecified` leave it
//!    `Pending`; a `stale` generation (an older read than the record's) is retried later.
//! 3. A `Pending` resource is admitted ([`crate::admission`]) and the answer recorded with
//!    `RecordCheck`: exit code 0 and the check's line, or 3 and the refusal. A resource refused
//!    now loses the children an earlier generation had: each child it owns as controller is
//!    deleted, preconditioned on the uid read.
//! 4. A `Running` or `Degraded` resource has its children applied by server-side apply, field
//!    manager `b10x-loom-operator`, forced; the Deployment's `Available` condition in the apply's
//!    answer, when its status has observed the applied generation, is `ObserveDeployment`. A
//!    Service an earlier generation had is deleted once `expose.a2a` is gone.
//! 5. The record's row is published as the status by server-side apply on the status
//!    subresource, unless the status already reads so.
//!
//! The next reconcile comes after [`REQUEUE`], on any change to the resource or to a Deployment
//! it owns, and after [`RETRY`] when one fails. After a restart the record is empty: every
//! resource is created again at its cluster generation, checked again, and its status rewritten.
//! The controller reads and writes no Secret and reads no Pod.

use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use b10x_loom_resource::model::{
    ApplyLoomAgent, ApplyLoomAgentOutcome, DeleteLoomAgent, DeploymentAvailability, LoomAgentState,
    LoomAgents, ObserveDeployment, RecordCheck, ResourceName,
};
use b10x_loom_resource::{LoomAgentRecord, ResourceEvent, UnmetObligation};
use futures::StreamExt;
use k8s_openapi::NamespaceResourceScope;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Service, ServiceAccount};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use kube::api::{Api, DeleteParams, Patch, PatchParams, Preconditions};
use kube::runtime::controller::{self, Action, Controller};
use kube::runtime::watcher;
use kube::{Client, Resource, ResourceExt};
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::parse;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::FIELD_MANAGER;
use crate::admission::{self, Admitted};
use crate::config::Config;
use crate::render::{self, Children};
use crate::resource::LoomAgent;
use crate::status;

/// How long a reconciled resource waits for its next reconcile when nothing changes.
pub const REQUEUE: Duration = Duration::from_secs(300);
/// How long a resource waits after a reconcile failed.
pub const RETRY: Duration = Duration::from_secs(30);

/// Why a reconcile failed.
#[derive(Debug)]
pub enum Error {
    /// The API server refused or could not be reached.
    Kube(kube::Error),
    /// The generated behaviour refused a request it declares no outcome for.
    Record(UnmetObligation),
    /// The resource has no name.
    Unnamed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kube(error) => write!(f, "the API server: {error}"),
            Self::Record(unmet) => write!(f, "the record: {unmet}"),
            Self::Unnamed => f.write_str("a LoomAgent without metadata.name"),
        }
    }
}

impl std::error::Error for Error {}

impl From<kube::Error> for Error {
    fn from(error: kube::Error) -> Self {
        Self::Kube(error)
    }
}

impl From<UnmetObligation> for Error {
    fn from(unmet: UnmetObligation) -> Self {
        Self::Record(unmet)
    }
}

/// What every reconcile shares: the client, the configuration, the plugin catalog the check
/// reads, and the record.
pub struct Context {
    client: Client,
    config: Config,
    catalog: ProtocolCatalog,
    record: Mutex<LoomAgentRecord>,
}

impl Context {
    /// A context over `client` under `config`, with an empty record.
    ///
    /// # Errors
    /// The plugin catalog cannot be loaded.
    pub fn new(client: Client, config: Config) -> Result<Self, String> {
        Ok(Self {
            client,
            config,
            catalog: ProtocolCatalog::plugins()?,
            record: Mutex::new(LoomAgentRecord::new()),
        })
    }

    fn record(&self) -> MutexGuard<'_, LoomAgentRecord> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The view `LoomAgents`: one row per resource the record holds.
    #[must_use]
    pub fn rows(&self) -> Vec<LoomAgents> {
        self.record().loom_agents().unwrap_or_default()
    }

    /// Every event the record's outcomes published, in order.
    #[must_use]
    pub fn events(&self) -> Vec<ResourceEvent> {
        self.record().events().to_vec()
    }

    /// Forgets the resource `name`: `DeleteLoomAgent`. A name the record does not hold is
    /// answered `unknown` and changes nothing.
    pub fn forget(&self, name: &str) {
        let _ = self.record().delete_loom_agent(DeleteLoomAgent {
            name: ResourceName(name.to_owned()),
        });
    }

    fn admit(&self, agent: &LoomAgent, namespace: &str) -> Result<Admitted, admission::Refusal> {
        let actions = |protocol: &str| self.catalog.get(protocol).map(|entry| entry.actions());
        admission::admit(&agent.spec, namespace, &self.config, &actions)
    }
}

fn row(ctx: &Context, name: &ResourceName) -> Option<LoomAgents> {
    ctx.rows().into_iter().find(|row| &row.name == name)
}

fn params() -> PatchParams {
    PatchParams::apply(FIELD_MANAGER).force()
}

/// Applies `object` in `namespace` by server-side apply.
async fn apply<K>(client: &Client, namespace: &str, object: &K) -> Result<K, kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned
        + Serialize,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    api.patch(&object.name_any(), &params(), &Patch::Apply(object))
        .await
}

/// Deletes the `K` named `name` in `namespace` when the resource whose uid is `owner` owns it as
/// its controller; a child that is gone, or someone else's, is left alone.
async fn delete_owned<K>(
    client: &Client,
    namespace: &str,
    name: &str,
    owner: &str,
) -> Result<(), kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    let Some(child) = api.get_opt(name).await? else {
        return Ok(());
    };
    let owned = child
        .owner_references()
        .iter()
        .any(|reference| reference.controller == Some(true) && reference.uid == owner);
    if !owned {
        return Ok(());
    }
    let params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: child.uid(),
            resource_version: None,
        }),
        ..DeleteParams::default()
    };
    match api.delete(name, &params).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(status)) if status.code == 404 => Ok(()),
        Err(error) => Err(error),
    }
}

/// What the Deployment reports of its availability, once its status observed its generation.
fn availability(deployment: &Deployment) -> Option<DeploymentAvailability> {
    let status = deployment.status.as_ref()?;
    if let (Some(generation), Some(observed)) =
        (deployment.metadata.generation, status.observed_generation)
        && observed < generation
    {
        return None;
    }
    let available = status
        .conditions
        .as_ref()?
        .iter()
        .find(|condition| condition.type_ == "Available")?;
    match available.status.as_str() {
        "True" => Some(DeploymentAvailability::Available),
        "False" => Some(DeploymentAvailability::Unavailable),
        _ => None,
    }
}

/// Applies `children`, and answers what the Deployment reports of its availability.
async fn apply_children(
    client: &Client,
    namespace: &str,
    children: &Children,
) -> Result<Option<DeploymentAvailability>, kube::Error> {
    apply(client, namespace, &children.service_account).await?;
    apply(client, namespace, &children.config_map).await?;
    let deployment = apply(client, namespace, &children.deployment).await?;
    apply(client, namespace, &children.network_policy).await?;
    if let Some(service) = &children.service {
        apply(client, namespace, service).await?;
    }
    Ok(availability(&deployment))
}

/// Deletes every child the resource `name`, whose uid is `owner`, may own.
async fn delete_children(
    client: &Client,
    namespace: &str,
    name: &str,
    owner: &str,
    service_account: &str,
) -> Result<(), kube::Error> {
    delete_owned::<ServiceAccount>(client, namespace, service_account, owner).await?;
    delete_owned::<ConfigMap>(client, namespace, &render::config_map_name(name), owner).await?;
    delete_owned::<Deployment>(client, namespace, name, owner).await?;
    delete_owned::<NetworkPolicy>(client, namespace, name, owner).await?;
    delete_owned::<Service>(client, namespace, name, owner).await
}

/// Publishes `row` as the status of the resource `name`, unless `current` already reads so.
async fn publish(
    ctx: &Context,
    namespace: &str,
    name: &str,
    current: Option<&crate::resource::LoomAgentStatus>,
    row: &LoomAgents,
) -> Result<(), kube::Error> {
    let status = status::status(row);
    if current == Some(&status) {
        return Ok(());
    }
    let api: Api<LoomAgent> = Api::namespaced(ctx.client.clone(), namespace);
    let patch = serde_json::json!({
        "apiVersion": LoomAgent::api_version(&()),
        "kind": LoomAgent::kind(&()),
        "metadata": {"name": name},
        "status": status,
    });
    api.patch_status(name, &params(), &Patch::Apply(&patch))
        .await?;
    Ok(())
}

/// Reconciles `agent` once, as the module documentation orders it.
///
/// # Errors
/// [`Error`]: the API server failed, the record refused, or the resource has no name.
pub async fn reconcile(agent: Arc<LoomAgent>, ctx: Arc<Context>) -> Result<Action, Error> {
    let name = agent
        .metadata
        .name
        .clone()
        .filter(|name| !name.is_empty())
        .ok_or(Error::Unnamed)?;
    if agent.metadata.deletion_timestamp.is_some() {
        ctx.forget(&name);
        return Ok(Action::await_change());
    }
    let namespace = agent
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| ctx.config.namespace.clone());
    let generation = agent.metadata.generation.unwrap_or_default();
    let key = ResourceName(name.clone());
    let owner = agent.metadata.uid.clone().unwrap_or_default();

    let applied = ctx.record().apply_loom_agent(ApplyLoomAgent {
        name: key.clone(),
        generation,
    })?;
    if matches!(applied, ApplyLoomAgentOutcome::Stale { .. }) {
        return Ok(Action::requeue(RETRY));
    }

    let admitted = ctx.admit(&agent, &namespace);
    let state = ctx.record().agent(&key).map(|held| held.state);
    if state == Some(LoomAgentState::Pending) {
        let (exit_code, message) = match &admitted {
            Ok(admitted) => (0, admitted.message()),
            Err(refusal) => (admission::REFUSED, refusal.message()),
        };
        ctx.record().record_check(RecordCheck {
            name: key.clone(),
            generation,
            exit_code,
            message,
        })?;
        if admitted.is_err() && !owner.is_empty() {
            let spec = admission::yaml(&agent.spec.agent, "spec.agent")
                .ok()
                .and_then(|text| parse(&text).ok());
            let account = render::service_account_name(&name, spec.as_ref());
            delete_children(&ctx.client, &namespace, &name, &owner, &account).await?;
        }
    }

    let state = ctx.record().agent(&key).map(|held| held.state);
    if let (Some(LoomAgentState::Running | LoomAgentState::Degraded), Ok(admitted)) =
        (state, &admitted)
    {
        let children = render::children(&agent, &name, &namespace, admitted, &ctx.config);
        let available = apply_children(&ctx.client, &namespace, &children).await?;
        if children.service.is_none() && !owner.is_empty() {
            delete_owned::<Service>(&ctx.client, &namespace, &name, &owner).await?;
        }
        if let Some(availability) = available {
            ctx.record().observe_deployment(ObserveDeployment {
                name: key.clone(),
                availability,
            })?;
        }
    }

    if let Some(row) = row(&ctx, &key) {
        publish(&ctx, &namespace, &name, agent.status.as_ref(), &row).await?;
    }
    Ok(Action::requeue(REQUEUE))
}

/// What follows a failed reconcile: another after [`RETRY`].
#[must_use]
pub fn error_policy(_agent: Arc<LoomAgent>, _error: &Error, _ctx: Arc<Context>) -> Action {
    Action::requeue(RETRY)
}

/// Runs the controller over `client` under `config` until its watch ends: every `LoomAgent` of
/// the configured namespace and every Deployment they own.
///
/// # Errors
/// The plugin catalog cannot be loaded.
pub async fn run(client: Client, config: Config) -> Result<(), String> {
    let namespace = config.namespace.clone();
    let ctx = Arc::new(Context::new(client.clone(), config)?);
    let agents: Api<LoomAgent> = Api::namespaced(client.clone(), &namespace);
    let deployments: Api<Deployment> = Api::namespaced(client, &namespace);
    let forget = Arc::clone(&ctx);
    Controller::new(agents, watcher::Config::default())
        .owns(deployments, watcher::Config::default())
        .shutdown_on_signal()
        .run(reconcile, error_policy, ctx)
        .for_each(|result| {
            match result {
                Ok(_) => {}
                Err(controller::Error::ObjectNotFound(object)) => {
                    forget.forget(&object.name);
                    eprintln!("b10x-loom-operator: forgot {}", object.name);
                }
                Err(error) => eprintln!("b10x-loom-operator: {error}"),
            }
            futures::future::ready(())
        })
        .await;
    Ok(())
}
