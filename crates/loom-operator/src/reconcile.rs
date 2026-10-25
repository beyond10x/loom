//! The controller: one reconcile of a `LoomAgent`, and the loop that runs it over every resource
//! of the configured namespace.
//!
//! The lifecycle is the generated behaviour of `loom.resource` over
//! [`b10x_loom_resource::LoomAgentRecord`], held in the controller's memory. One reconcile:
//!
//! 1. A resource being deleted is forgotten (`DeleteLoomAgent`); its children go by garbage
//!    collection, through their owner reference. A resource gone from the watch is forgotten the
//!    same way. The record is keyed by name, so the controller holds each name's uid beside it: a
//!    resource whose uid differs from the one held (deleted and created again under its name) is
//!    forgotten first and then applied as created.
//! 2. `ApplyLoomAgent` at the resource's generation: `created` or `respecified` leave it
//!    `Pending`; a `stale` generation (an older read than the record's) is retried later.
//! 3. A `Pending` resource is admitted ([`crate::admission`]); then each child it would render is
//!    read, and one that exists without this resource as its controller is refused
//!    (`ChildNotOwned`, naming its kind and name), unless its controller is a `LoomAgent` of the
//!    same name with another uid (a predecessor). The answer is recorded with `RecordCheck`: exit
//!    code 0 and the check's line, or 3 and the refusal. A `ChildNotOwned` refusal is about the
//!    cluster, not the spec: on each later reconcile at the same generation the children are read
//!    again, and once none is someone else's the record is forgotten and the generation applied
//!    and checked as created, in the same reconcile.
//! 4. A `Refused` resource has no children: on every reconcile each child it controls is deleted,
//!    preconditioned on the uid read, so a delete that failed is tried again.
//! 5. A `Running` or `Degraded` resource has its children read again: a child a predecessor
//!    controls is deleted and the children are applied on the next reconcile, after [`RETRY`]; a
//!    child someone else controls fails the reconcile ([`Error::NotOwned`]) and is never taken
//!    over. Otherwise the children are applied by server-side apply, field manager
//!    `b10x-loom-operator`, forced; the Deployment's `Available` condition in the apply's answer,
//!    when its status has observed the applied generation, is `ObserveDeployment`. Then each
//!    child it controls that this generation does not render (a Service once `expose.a2a` is
//!    gone, the ServiceAccount an earlier generation named) is deleted.
//! 6. The record's row is published as the status by server-side apply on the status
//!    subresource, unless the status already reads so.
//!
//! The children a resource controls are found by its uid, not by the names its current spec
//! renders: each child kind is listed by the label `app.kubernetes.io/instance: <resource>`, and
//! each child the controller applied for that uid is read by name as well; either one counts
//! only when its controlling owner reference names the resource's uid. A kind the API server
//! does not serve (a list answered 404) holds none.
//!
//! The next reconcile comes after [`REQUEUE`], on any change to the resource or to a Deployment
//! it owns, and after [`RETRY`] when one fails. After a restart the record is empty: every
//! resource is created again at its cluster generation, checked again, and its status rewritten.
//! The controller reads and writes no Secret and reads no Pod.

use std::collections::{BTreeSet, HashMap};
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
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, ListParams, Patch, PatchParams, Preconditions};
use kube::runtime::controller::{self, Action, Controller};
use kube::runtime::watcher;
use kube::{Client, Resource, ResourceExt};
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::AgentSpec;
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
    /// A child of a running resource exists and someone else controls it; it is not taken over.
    NotOwned {
        /// The child's kind.
        kind: &'static str,
        /// The child's name.
        name: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kube(error) => write!(f, "the API server: {error}"),
            Self::Record(unmet) => write!(f, "the record: {unmet}"),
            Self::Unnamed => f.write_str("a LoomAgent without metadata.name"),
            Self::NotOwned { kind, name } => write!(
                f,
                "the {kind} {name} exists and this resource does not control it"
            ),
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

/// What the controller holds of a resource beside its record: its uid, and each child (kind and
/// name) it applied for that uid and has not seen deleted, so a child a later generation no
/// longer renders is found by name as well as by its label.
#[derive(Debug)]
struct Held {
    uid: String,
    applied: BTreeSet<(Kind, String)>,
}

/// What every reconcile shares: the client, the configuration, the plugin catalog the check
/// reads, and the record.
pub struct Context {
    client: Client,
    config: Config,
    catalog: ProtocolCatalog,
    record: Mutex<LoomAgentRecord>,
    held: Mutex<HashMap<String, Held>>,
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
            held: Mutex::new(HashMap::new()),
        })
    }

    fn record(&self) -> MutexGuard<'_, LoomAgentRecord> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn held(&self) -> MutexGuard<'_, HashMap<String, Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds `uid` as the uid of the resource `name`. When another uid was held, the record's
    /// entry belongs to a resource since deleted and created again under the name: it is
    /// forgotten, with the children applied for it, so the new resource is applied as created.
    fn hold(&self, name: &str, uid: &str) {
        if uid.is_empty() {
            return;
        }
        let fresh = || Held {
            uid: uid.to_owned(),
            applied: BTreeSet::new(),
        };
        let replaced = {
            let mut held = self.held();
            let entry = held.entry(name.to_owned()).or_insert_with(fresh);
            if entry.uid == uid {
                false
            } else {
                *entry = fresh();
                true
            }
        };
        if replaced {
            let _ = self.record().delete_loom_agent(DeleteLoomAgent {
                name: ResourceName(name.to_owned()),
            });
        }
    }

    /// Each child applied for the resource `name` and not seen deleted.
    fn applied(&self, name: &str) -> Vec<(Kind, String)> {
        self.held()
            .get(name)
            .map(|held| held.applied.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Notes that the child `kind` `child` is applied for the resource `name`.
    fn remember(&self, name: &str, kind: Kind, child: &str) {
        if let Some(held) = self.held().get_mut(name) {
            held.applied.insert((kind, child.to_owned()));
        }
    }

    /// Notes that the child `kind` `child` of the resource `name` is gone or not its own.
    fn unremember(&self, name: &str, kind: Kind, child: &str) {
        if let Some(held) = self.held().get_mut(name) {
            held.applied.remove(&(kind, child.to_owned()));
        }
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
        self.held().remove(name);
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

/// The kinds of child a resource renders, in the order they are applied and deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    ServiceAccount,
    ConfigMap,
    Deployment,
    NetworkPolicy,
    Service,
}

impl Kind {
    const ALL: [Self; 5] = [
        Self::ServiceAccount,
        Self::ConfigMap,
        Self::Deployment,
        Self::NetworkPolicy,
        Self::Service,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::ServiceAccount => "ServiceAccount",
            Self::ConfigMap => "ConfigMap",
            Self::Deployment => "Deployment",
            Self::NetworkPolicy => "NetworkPolicy",
            Self::Service => "Service",
        }
    }
}

/// Who controls a child that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Holding {
    /// No such object.
    Absent,
    /// The resource itself, by its uid. Carries the child's own uid.
    Ours(String),
    /// A `LoomAgent` of the same name with another uid: one deleted and created again, whose
    /// children the garbage collector has not removed yet. Carries the child's own uid.
    Predecessor(String),
    /// Anyone else, or no controller at all.
    Foreign,
}

/// Who controls the object `meta` describes, seen from the resource `resource` whose uid is
/// `owner`.
fn holding(meta: &ObjectMeta, resource: &str, owner: &str) -> Holding {
    let controller = meta
        .owner_references
        .iter()
        .flatten()
        .find(|reference| reference.controller == Some(true));
    let uid = meta.uid.clone().unwrap_or_default();
    match controller {
        Some(reference) if !owner.is_empty() && reference.uid == owner => Holding::Ours(uid),
        Some(reference)
            if reference.kind == LoomAgent::kind(&())
                && reference.api_version.split('/').next() == Some(&*LoomAgent::group(&()))
                && reference.name == resource =>
        {
            Holding::Predecessor(uid)
        }
        _ => Holding::Foreign,
    }
}

/// Who controls the `K` named `name` in `namespace`.
async fn held<K>(
    client: &Client,
    namespace: &str,
    name: &str,
    resource: &str,
    owner: &str,
) -> Result<Holding, kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    Ok(match api.get_opt(name).await? {
        None => Holding::Absent,
        Some(child) => holding(child.meta(), resource, owner),
    })
}

/// Deletes the `K` named `name` in `namespace`, preconditioned on `uid`, the uid read; one that
/// is gone or was replaced meanwhile is left alone.
async fn delete_uid<K>(
    client: &Client,
    namespace: &str,
    name: &str,
    uid: &str,
) -> Result<(), kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    let params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: Some(uid.to_owned()),
            resource_version: None,
        }),
        ..DeleteParams::default()
    };
    match api.delete(name, &params).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(status)) if status.code == 404 || status.code == 409 => Ok(()),
        Err(error) => Err(error),
    }
}

/// Who controls the child of `kind` named `name`.
async fn held_kind(
    kind: Kind,
    client: &Client,
    namespace: &str,
    name: &str,
    resource: &str,
    owner: &str,
) -> Result<Holding, kube::Error> {
    match kind {
        Kind::ServiceAccount => {
            held::<ServiceAccount>(client, namespace, name, resource, owner).await
        }
        Kind::ConfigMap => held::<ConfigMap>(client, namespace, name, resource, owner).await,
        Kind::Deployment => held::<Deployment>(client, namespace, name, resource, owner).await,
        Kind::NetworkPolicy => {
            held::<NetworkPolicy>(client, namespace, name, resource, owner).await
        }
        Kind::Service => held::<Service>(client, namespace, name, resource, owner).await,
    }
}

/// Deletes the child of `kind` named `name` whose uid is `uid`.
async fn delete_kind(
    kind: Kind,
    client: &Client,
    namespace: &str,
    name: &str,
    uid: &str,
) -> Result<(), kube::Error> {
    match kind {
        Kind::ServiceAccount => delete_uid::<ServiceAccount>(client, namespace, name, uid).await,
        Kind::ConfigMap => delete_uid::<ConfigMap>(client, namespace, name, uid).await,
        Kind::Deployment => delete_uid::<Deployment>(client, namespace, name, uid).await,
        Kind::NetworkPolicy => delete_uid::<NetworkPolicy>(client, namespace, name, uid).await,
        Kind::Service => delete_uid::<Service>(client, namespace, name, uid).await,
    }
}

/// The label selector of every child of the resource `resource`.
fn instance(resource: &str) -> String {
    format!("app.kubernetes.io/instance={resource}")
}

/// The name and uid of each `K` labelled as a child of the resource `resource` that the
/// resource, whose uid is `owner`, controls. A kind the API server does not serve (404) holds
/// none.
async fn listed<K>(
    client: &Client,
    namespace: &str,
    resource: &str,
    owner: &str,
) -> Result<Vec<(String, String)>, kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    let list = match api
        .list(&ListParams::default().labels(&instance(resource)))
        .await
    {
        Ok(list) => list,
        Err(kube::Error::Api(status)) if status.code == 404 => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(list
        .items
        .iter()
        .filter_map(|child| match holding(child.meta(), resource, owner) {
            Holding::Ours(uid) => Some((child.name_any(), uid)),
            _ => None,
        })
        .collect())
}

/// [`listed`] for the child kind `kind`.
async fn listed_kind(
    kind: Kind,
    client: &Client,
    namespace: &str,
    resource: &str,
    owner: &str,
) -> Result<Vec<(String, String)>, kube::Error> {
    match kind {
        Kind::ServiceAccount => listed::<ServiceAccount>(client, namespace, resource, owner).await,
        Kind::ConfigMap => listed::<ConfigMap>(client, namespace, resource, owner).await,
        Kind::Deployment => listed::<Deployment>(client, namespace, resource, owner).await,
        Kind::NetworkPolicy => listed::<NetworkPolicy>(client, namespace, resource, owner).await,
        Kind::Service => listed::<Service>(client, namespace, resource, owner).await,
    }
}

/// A child the resource controls: its kind, name and uid.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Owned {
    kind: Kind,
    name: String,
    uid: String,
}

/// Every child the resource `resource`, whose uid is `owner`, controls, except those in `keep`,
/// in the order [`Kind`] gives: each kind listed by the instance label, then each child applied
/// for the uid read by name, so one whose label was changed is found all the same. A child read
/// by name that is gone or not the resource's is no longer remembered.
async fn owned(
    ctx: &Context,
    namespace: &str,
    resource: &str,
    owner: &str,
    keep: &BTreeSet<(Kind, String)>,
) -> Result<Vec<Owned>, kube::Error> {
    let mut found = BTreeSet::new();
    for kind in Kind::ALL {
        for (name, uid) in listed_kind(kind, &ctx.client, namespace, resource, owner).await? {
            if !keep.contains(&(kind, name.clone())) {
                found.insert(Owned { kind, name, uid });
            }
        }
    }
    for (kind, name) in ctx.applied(resource) {
        if keep.contains(&(kind, name.clone()))
            || found
                .iter()
                .any(|child| child.kind == kind && child.name == name)
        {
            continue;
        }
        match held_kind(kind, &ctx.client, namespace, &name, resource, owner).await? {
            Holding::Ours(uid) => {
                found.insert(Owned { kind, name, uid });
            }
            _ => ctx.unremember(resource, kind, &name),
        }
    }
    Ok(found.into_iter().collect())
}

/// Deletes every child the resource `resource`, whose uid is `owner`, controls, except those in
/// `keep`, each preconditioned on the uid read.
async fn delete_owned(
    ctx: &Context,
    namespace: &str,
    resource: &str,
    owner: &str,
    keep: &BTreeSet<(Kind, String)>,
) -> Result<(), kube::Error> {
    for child in owned(ctx, namespace, resource, owner, keep).await? {
        delete_kind(child.kind, &ctx.client, namespace, &child.name, &child.uid).await?;
        ctx.unremember(resource, child.kind, &child.name);
    }
    Ok(())
}

/// The kind and name of each of `children`, in the order they are applied.
fn wanted(children: &Children) -> Vec<(Kind, String)> {
    let mut wanted = vec![
        (Kind::ServiceAccount, children.service_account.name_any()),
        (Kind::ConfigMap, children.config_map.name_any()),
        (Kind::Deployment, children.deployment.name_any()),
        (Kind::NetworkPolicy, children.network_policy.name_any()),
    ];
    if let Some(service) = &children.service {
        wanted.push((Kind::Service, service.name_any()));
    }
    wanted
}

/// One child a resource renders, and who controls it now.
#[derive(Debug, Clone)]
struct Surveyed {
    kind: Kind,
    name: String,
    holding: Holding,
}

/// Reads each of `children` of the resource `resource`, whose uid is `owner`.
async fn survey(
    client: &Client,
    namespace: &str,
    resource: &str,
    owner: &str,
    children: &Children,
) -> Result<Vec<Surveyed>, kube::Error> {
    let wanted = wanted(children);
    let mut found = Vec::with_capacity(wanted.len());
    for (kind, name) in wanted {
        let holding = held_kind(kind, client, namespace, &name, resource, owner).await?;
        found.push(Surveyed {
            kind,
            name,
            holding,
        });
    }
    Ok(found)
}

/// The reason of a refusal the children's survey gave, not the spec.
pub const CHILD_NOT_OWNED: &str = "ChildNotOwned";

/// Whether `message`, a recorded check, is a refusal the children's survey gave.
fn survey_refusal(message: &str) -> bool {
    message
        .strip_prefix("refused: ")
        .and_then(|rest| rest.strip_prefix(CHILD_NOT_OWNED))
        .is_some_and(|rest| rest.starts_with(" at "))
}

/// The refusal of a resource one of whose children someone else controls: the first such child.
fn not_owned(found: &[Surveyed], spec: &AgentSpec) -> Option<admission::Refusal> {
    let child = found
        .iter()
        .find(|child| child.holding == Holding::Foreign)?;
    let named_by_spec = child.kind == Kind::ServiceAccount
        && spec
            .deploy
            .as_ref()
            .and_then(|deploy| deploy.kubernetes.as_ref())
            .is_some();
    Some(admission::Refusal {
        reason: CHILD_NOT_OWNED.to_owned(),
        member: if named_by_spec {
            "spec.agent.deploy.kubernetes.service_account"
        } else {
            "metadata.name"
        }
        .to_owned(),
        detail: format!(
            "the {} {} exists and this resource does not control it",
            child.kind.as_str(),
            child.name
        ),
    })
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
    ctx.hold(&name, &owner);

    let admitted = ctx.admit(&agent, &namespace);
    let rendered = admitted.as_ref().map(|admitted| {
        (
            admitted,
            render::children(&agent, &name, &namespace, admitted, &ctx.config),
        )
    });
    let mut surveyed = None;
    let recorded = ctx.record().agent(&key);
    if let (Ok((_, children)), Some(recorded)) = (&rendered, &recorded)
        && recorded.state == LoomAgentState::Refused
        && recorded.data.observed_generation == generation
        && recorded
            .data
            .check
            .as_ref()
            .is_some_and(|check| survey_refusal(&check.message))
    {
        let found = survey(&ctx.client, &namespace, &name, &owner, children).await?;
        if found.iter().all(|child| child.holding != Holding::Foreign) {
            ctx.record()
                .delete_loom_agent(DeleteLoomAgent { name: key.clone() })?;
            surveyed = Some(found);
        }
    }

    let applied = ctx.record().apply_loom_agent(ApplyLoomAgent {
        name: key.clone(),
        generation,
    })?;
    if matches!(applied, ApplyLoomAgentOutcome::Stale { .. }) {
        return Ok(Action::requeue(RETRY));
    }

    let state = ctx.record().agent(&key).map(|held| held.state);
    if state == Some(LoomAgentState::Pending) {
        let checked = match &rendered {
            Ok((admitted, children)) => {
                let found = match surveyed.take() {
                    Some(found) => found,
                    None => survey(&ctx.client, &namespace, &name, &owner, children).await?,
                };
                let refusal = not_owned(&found, &admitted.spec);
                surveyed = Some(found);
                refusal.map_or_else(|| Ok(admitted.message()), Err)
            }
            Err(refusal) => Err((*refusal).clone()),
        };
        let (exit_code, message) = match checked {
            Ok(message) => (0, message),
            Err(refusal) => (admission::REFUSED, refusal.message()),
        };
        ctx.record().record_check(RecordCheck {
            name: key.clone(),
            generation,
            exit_code,
            message,
        })?;
    }

    let state = ctx.record().agent(&key).map(|held| held.state);
    match (state, &rendered) {
        (Some(LoomAgentState::Refused), _) if !owner.is_empty() => {
            delete_owned(&ctx, &namespace, &name, &owner, &BTreeSet::new()).await?;
        }
        (Some(LoomAgentState::Running | LoomAgentState::Degraded), Ok((_, children))) => {
            let found = match surveyed {
                Some(found) => found,
                None => survey(&ctx.client, &namespace, &name, &owner, children).await?,
            };
            if let Some(child) = found.iter().find(|child| child.holding == Holding::Foreign) {
                return Err(Error::NotOwned {
                    kind: child.kind.as_str(),
                    name: child.name.clone(),
                });
            }
            let predecessors: Vec<&Surveyed> = found
                .iter()
                .filter(|child| matches!(child.holding, Holding::Predecessor(_)))
                .collect();
            if predecessors.is_empty() {
                let current: BTreeSet<(Kind, String)> = wanted(children).into_iter().collect();
                for (kind, child) in &current {
                    ctx.remember(&name, *kind, child);
                }
                let available = apply_children(&ctx.client, &namespace, children).await?;
                if let Some(availability) = available {
                    ctx.record().observe_deployment(ObserveDeployment {
                        name: key.clone(),
                        availability,
                    })?;
                }
                if !owner.is_empty() {
                    delete_owned(&ctx, &namespace, &name, &owner, &current).await?;
                }
            } else {
                for child in predecessors {
                    if let Holding::Predecessor(uid) = &child.holding {
                        delete_kind(child.kind, &ctx.client, &namespace, &child.name, uid).await?;
                    }
                }
                if let Some(row) = row(&ctx, &key) {
                    publish(&ctx, &namespace, &name, agent.status.as_ref(), &row).await?;
                }
                return Ok(Action::requeue(RETRY));
            }
        }
        _ => {}
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
