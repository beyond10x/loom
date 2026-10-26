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
//!    over. Before a `Command` resource's Job is created, its Job of another generation (a Job's
//!    pod template cannot change) and a Deployment an earlier release made for it are deleted;
//!    while either still terminates nothing is applied and the reconcile comes again after
//!    [`RETRY`], so an old pod and the new run never share the workspace. A Job of the
//!    resource's generation is never sent again: one that has finished (`Complete` or `Failed`)
//!    has nothing applied and its run is `ObserveRun`; one still active has the other children
//!    applied and its run, as read, is `ObserveRun`. Otherwise the children are applied by
//!    server-side apply, field manager `b10x-loom-operator`, forced; the Deployment's `Available`
//!    condition in the apply's answer, when its status has observed the applied generation, is
//!    `ObserveDeployment`, and the created Job's run is `ObserveRun`. A finished run's `ObserveRun`
//!    carries its exit code, end time and reason, read from the Job's own pod (`observation`):
//!    one its label selector names whose controlling owner reference is the Job's uid. While the
//!    Job counts the run over but its own pod is in no final phase and its program container has
//!    not terminated, nothing is observed and the reconcile comes again after [`RETRY`]. A pod
//!    still terminating [`TERMINATION_SLACK`] past its deletion deadline does not hold it: the run
//!    ends with no exit code and the reason [`NEVER_TERMINATED`]. Once a Job of the generation was
//!    seen counting its run over it is never created again: if it is gone, the run ends at once
//!    from what its pods still say. Then each child it controls that this generation does not
//!    render (a Service once `expose.a2a` is gone, the ServiceAccount an earlier generation named)
//!    is deleted. Every delete propagates in the foreground: what the child owns, a Job's pod
//!    included, goes before it does.
//! 6. A `Completed` or `Failed` resource has no child applied, read or deleted: its run is over
//!    until a new generation respecifies it.
//! 7. The record's row is published as the status by server-side apply on the status
//!    subresource, unless the status already reads so.
//!
//! The children a resource controls are found by its uid, not by the names its current spec
//! renders: each child kind is listed by the label `app.kubernetes.io/instance: <resource>`, and
//! each child the controller applied for that uid is read by name as well; either one counts
//! only when its controlling owner reference names the resource's uid. A kind the API server
//! does not serve (a list answered 404) holds none.
//!
//! The next reconcile comes after [`REQUEUE`], on any change to the resource or to a Deployment
//! or Job it owns, and after [`RETRY`] when one fails. After a restart the record is empty: every
//! resource is created again at its cluster generation, checked again, and its status rewritten.
//! A `Command` resource whose finished Job is still there reads its run from it and does not run
//! again; one whose Job was deleted while the controller was down runs once more.
//! The controller reads and writes no Secret, and reads a Pod only to learn how a finished run
//! ended; it watches none.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use b10x_loom_resource::model::{
    ApplyLoomAgent, ApplyLoomAgentOutcome, DeleteLoomAgent, DeploymentAvailability, LoomAgentState,
    LoomAgents, ObserveDeployment, ObserveRun, RecordCheck, ResourceName, RunPhase,
};
use b10x_loom_resource::{LoomAgentRecord, ResourceEvent, Timestamp, UnmetObligation};
use futures::StreamExt;
use k8s_openapi::NamespaceResourceScope;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{
    ConfigMap, ContainerStateTerminated, ContainerStatus, Pod, Service, ServiceAccount,
};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
use k8s_openapi::jiff::{SignedDuration, Timestamp as JiffTimestamp};
use kube::api::{
    Api, DeleteParams, ListParams, Patch, PatchParams, Preconditions, PropagationPolicy,
};
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
use crate::render::{self, Children, GENERATION_ANNOTATION, Workload};
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
/// longer renders is found by name as well as by its label, and the code a list of its
/// Job's pods was last refused with.
#[derive(Debug)]
struct Held {
    uid: String,
    applied: BTreeSet<(Kind, String)>,
    pods_refused: Option<u16>,
    over: Option<JobRun>,
}

/// What every reconcile shares: the client, the configuration, the plugin catalog the check
/// reads, the record, and the clock.
pub struct Context {
    client: Client,
    config: Config,
    catalog: ProtocolCatalog,
    record: Mutex<LoomAgentRecord>,
    held: Mutex<HashMap<String, Held>>,
    clock: fn() -> JiffTimestamp,
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
            clock: JiffTimestamp::now,
        })
    }

    /// This context, reading the time from `clock` instead of the system clock: the time a
    /// terminating pod's deletion deadline is compared with.
    #[must_use]
    pub fn with_clock(self, clock: fn() -> k8s_openapi::jiff::Timestamp) -> Self {
        Self { clock, ..self }
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
            pods_refused: None,
            over: None,
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

    /// Notes the code the API server answered the last list of the pods of the resource `name`
    /// with, when it refused it (403 or 404), or that it did not.
    fn refuse_pods(&self, name: &str, code: Option<u16>) {
        if let Some(held) = self.held().get_mut(name) {
            held.pods_refused = code;
        }
    }

    /// Notes that the Job `run` of the resource `name` was seen counting its run over.
    fn saw_over(&self, name: &str, run: &JobRun) {
        if let Some(held) = self.held().get_mut(name) {
            held.over = Some(run.clone());
        }
    }

    /// The Job of the resource `name` last seen counting its run over, when it is of
    /// `generation`; one of another generation is forgotten.
    fn seen_over(&self, name: &str, generation: Option<i64>) -> Option<JobRun> {
        let mut held = self.held();
        let held = held.get_mut(name)?;
        if held
            .over
            .as_ref()
            .is_some_and(|run| run.generation != generation)
        {
            held.over = None;
        }
        held.over.clone()
    }

    /// The code the last list of the pods of the resource `name` was refused with, if it was.
    fn pods_refused(&self, name: &str) -> Option<u16> {
        self.held().get(name).and_then(|held| held.pods_refused)
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
    Job,
    NetworkPolicy,
    Service,
}

impl Kind {
    const ALL: [Self; 6] = [
        Self::ServiceAccount,
        Self::ConfigMap,
        Self::Deployment,
        Self::Job,
        Self::NetworkPolicy,
        Self::Service,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::ServiceAccount => "ServiceAccount",
            Self::ConfigMap => "ConfigMap",
            Self::Deployment => "Deployment",
            Self::Job => "Job",
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

/// The `K` named `name` in `namespace`, if there is one.
async fn read<K>(client: &Client, namespace: &str, name: &str) -> Result<Option<K>, kube::Error>
where
    K: Resource<Scope = NamespaceResourceScope, DynamicType = ()>
        + Clone
        + Debug
        + DeserializeOwned,
{
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    api.get_opt(name).await
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
    Ok(match read::<K>(client, namespace, name).await? {
        None => Holding::Absent,
        Some(child) => holding(child.meta(), resource, owner),
    })
}

/// Deletes the `K` named `name` in `namespace`, preconditioned on `uid`, the uid read; one that
/// is gone or was replaced meanwhile is left alone. Propagation is `Foreground`: what the child
/// owns (a Job's pod, a Deployment's ReplicaSets and pods) goes first, and the child stays,
/// terminating, until it has (the API server's default for a Job orphans its pod). Answers whether
/// the child is gone: false while it terminates, or when another object took its name.
async fn delete_uid<K>(
    client: &Client,
    namespace: &str,
    name: &str,
    uid: &str,
) -> Result<bool, kube::Error>
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
        propagation_policy: Some(PropagationPolicy::Foreground),
        ..DeleteParams::default()
    };
    match api.delete(name, &params).await {
        Ok(answer) => Ok(answer.is_right()),
        Err(kube::Error::Api(status)) if status.code == 404 => Ok(true),
        Err(kube::Error::Api(status)) if status.code == 409 => Ok(false),
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
        Kind::Job => held::<Job>(client, namespace, name, resource, owner).await,
        Kind::NetworkPolicy => {
            held::<NetworkPolicy>(client, namespace, name, resource, owner).await
        }
        Kind::Service => held::<Service>(client, namespace, name, resource, owner).await,
    }
}

/// Deletes the child of `kind` named `name` whose uid is `uid`; answers whether it is gone.
async fn delete_kind(
    kind: Kind,
    client: &Client,
    namespace: &str,
    name: &str,
    uid: &str,
) -> Result<bool, kube::Error> {
    match kind {
        Kind::ServiceAccount => delete_uid::<ServiceAccount>(client, namespace, name, uid).await,
        Kind::ConfigMap => delete_uid::<ConfigMap>(client, namespace, name, uid).await,
        Kind::Deployment => delete_uid::<Deployment>(client, namespace, name, uid).await,
        Kind::Job => delete_uid::<Job>(client, namespace, name, uid).await,
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
        Kind::Job => listed::<Job>(client, namespace, resource, owner).await,
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
        match &children.workload {
            Workload::Deployment(deployment) => (Kind::Deployment, deployment.name_any()),
            Workload::Job(job) => (Kind::Job, job.name_any()),
        },
        (Kind::NetworkPolicy, children.network_policy.name_any()),
    ];
    if let Some(service) = &children.service {
        wanted.push((Kind::Service, service.name_any()));
    }
    wanted
}

/// What a Job the resource controls says of its run: the generation its pod template names and
/// the run's phase, and the Job as read, whose pod says how a finished run ended.
#[derive(Debug, Clone)]
struct JobRun {
    generation: Option<i64>,
    phase: RunPhase,
    job: Job,
}

/// One child a resource renders, and who controls it now; for a Job the resource controls, what
/// it says of its run.
#[derive(Debug, Clone)]
struct Surveyed {
    kind: Kind,
    name: String,
    holding: Holding,
    run: Option<JobRun>,
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
        let (holding, run) = if kind == Kind::Job {
            match read::<Job>(client, namespace, &name).await? {
                None => (Holding::Absent, None),
                Some(job) => {
                    let holding = holding(job.meta(), resource, owner);
                    let run = matches!(holding, Holding::Ours(_)).then(|| JobRun {
                        generation: job_generation(&job),
                        phase: run_phase(&job),
                        job: job.clone(),
                    });
                    (holding, run)
                }
            }
        } else {
            (
                held_kind(kind, client, namespace, &name, resource, owner).await?,
                None,
            )
        };
        found.push(Surveyed {
            kind,
            name,
            holding,
            run,
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
fn not_owned(found: &[Surveyed], spec: Option<&AgentSpec>) -> Option<admission::Refusal> {
    let child = found
        .iter()
        .find(|child| child.holding == Holding::Foreign)?;
    let named_by_spec = child.kind == Kind::ServiceAccount
        && spec
            .and_then(|spec| spec.deploy.as_ref())
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

/// The generation a Job's pod template names (`loom.beyond10x.dev/generation`), if it names one
/// that is a number.
fn job_generation(job: &Job) -> Option<i64> {
    job.spec
        .as_ref()?
        .template
        .metadata
        .as_ref()?
        .annotations
        .as_ref()?
        .get(GENERATION_ANNOTATION)?
        .parse()
        .ok()
}

/// What a Job reports of its run: Succeeded once its condition `Complete` is True or a pod
/// succeeded; Failed once its condition `Failed` is True or a pod failed, since `backoffLimit: 0`
/// starts no other; Active otherwise, a status not yet written included.
fn run_phase(job: &Job) -> RunPhase {
    let Some(status) = job.status.as_ref() else {
        return RunPhase::Active;
    };
    let holds = |kind: &str| {
        status
            .conditions
            .iter()
            .flatten()
            .any(|condition| condition.type_ == kind && condition.status == "True")
    };
    if holds("Complete") || status.succeeded.unwrap_or_default() > 0 {
        RunPhase::Succeeded
    } else if holds("Failed") || status.failed.unwrap_or_default() > 0 {
        RunPhase::Failed
    } else {
        RunPhase::Active
    }
}

/// The run of the resource `resource` as `ObserveRun` takes it: the Job's phase and, once the
/// run is over, how it ended; `None` while the run is over by the Job's count but its own pod
/// still terminates, so the caller comes again after [`RETRY`] instead of observing an end it
/// cannot read yet. An active run says nothing more and reads no pod. A finished one reads the
/// exit code and end time from the Job's pod: the pods its label selector names (the resource's
/// instance label when it has none) and whose controlling owner reference is the Job's uid, so a
/// pod of another Job never speaks for it. Of those, the one whose program container
/// ([`render::CONTAINER`]) terminated last answers; its `reason` comes along. With no such pod
/// (deleted before its container's end was read, or one in a final phase whose container never
/// ran) the exit code and end time are absent and the reason is the Job's own, from its `Failed`
/// or `Complete` condition. A list answered 403 or 404 is the same, logged, and the resource's
/// `Completed` condition says its pod could not be read.
async fn observation(
    ctx: &Context,
    namespace: &str,
    resource: &ResourceName,
    job: Option<&Job>,
    phase: RunPhase,
    wait: bool,
) -> Result<Option<ObserveRun>, kube::Error> {
    let mut observed = ObserveRun {
        name: resource.clone(),
        phase,
        exit_code: None,
        finished_at: None,
        reason: None,
    };
    let Some(job) = job.filter(|_| phase != RunPhase::Active) else {
        return Ok(Some(observed));
    };
    let read = pod_read(&ctx.client, namespace, &resource.0, job, (ctx.clock)()).await?;
    ctx.refuse_pods(
        &resource.0,
        match read {
            PodRead::Refused(code) => Some(code),
            _ => None,
        },
    );
    match read {
        PodRead::Ended(end) => {
            observed.exit_code = Some(i64::from(end.exit_code));
            observed.finished_at = end.finished_at.as_ref().and_then(wire_time);
            observed.reason = end.reason;
        }
        PodRead::Ending if wait => return Ok(None),
        PodRead::Ending | PodRead::Absent => observed.reason = job_reason(job, phase),
        PodRead::Stuck => observed.reason = Some(NEVER_TERMINATED.to_owned()),
        PodRead::Refused(code) => {
            eprintln!(
                "b10x-loom-operator: {}: the pod list was answered {code}; the run's exit code \
                 is not read",
                resource.0
            );
            observed.reason = job_reason(job, phase);
        }
    }
    Ok(Some(observed))
}

/// What the pods of a Job whose run is over say of how it ended.
enum PodRead {
    /// The program container of the Job's own pod terminated; the latest when there are more.
    Ended(ContainerStateTerminated),
    /// None terminated yet, and a pod of the Job's own is in no final phase: its container is
    /// still being stopped, so its end comes.
    Ending,
    /// None terminated, and each pod of the Job's own still in no final phase is being deleted
    /// and past its grace period by more than [`TERMINATION_SLACK`]: its node no longer
    /// answers, and nothing will write its container's end.
    Stuck,
    /// No pod of the Job's own has a program container that terminated or will.
    Absent,
    /// The API server answered the list with this code (403 or 404).
    Refused(u16),
}

/// How the program container of the Job's own pods ended, or that it is still ending.
async fn pod_read(
    client: &Client,
    namespace: &str,
    resource: &str,
    job: &Job,
    now: JiffTimestamp,
) -> Result<PodRead, kube::Error> {
    let Some(uid) = job.metadata.uid.as_deref() else {
        return Ok(PodRead::Absent);
    };
    let selector = job
        .spec
        .as_ref()
        .and_then(|spec| spec.selector.as_ref())
        .and_then(|selector| selector.match_labels.as_ref())
        .filter(|labels| !labels.is_empty())
        .map_or_else(
            || instance(resource),
            |labels| {
                labels
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join(",")
            },
        );
    let api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pods = match api.list(&ListParams::default().labels(&selector)).await {
        Ok(list) => list.items,
        Err(kube::Error::Api(status)) if matches!(status.code, 403 | 404) => {
            return Ok(PodRead::Refused(status.code));
        }
        Err(error) => return Err(error),
    };
    let own: Vec<Pod> = pods
        .into_iter()
        .filter(|pod| {
            pod.metadata
                .owner_references
                .iter()
                .flatten()
                .any(|reference| reference.controller == Some(true) && reference.uid == uid)
        })
        .collect();
    let ended = own
        .iter()
        .filter_map(|pod| program(pod)?.state.as_ref()?.terminated.clone())
        .max_by(|left, right| {
            let at = |end: &ContainerStateTerminated| end.finished_at.as_ref().map(|time| time.0);
            at(left).cmp(&at(right))
        });
    if let Some(end) = ended {
        return Ok(PodRead::Ended(end));
    }
    let unfinished: Vec<&Pod> = own
        .iter()
        .filter(|pod| {
            let phase = pod
                .status
                .as_ref()
                .and_then(|status| status.phase.as_deref());
            !matches!(phase, Some("Succeeded" | "Failed"))
        })
        .collect();
    Ok(if unfinished.is_empty() {
        PodRead::Absent
    } else if unfinished.iter().all(|pod| stuck(pod, now)) {
        PodRead::Stuck
    } else {
        PodRead::Ending
    })
}

/// How long past its deletion deadline (`deletionTimestamp` plus `deletionGracePeriodSeconds`) a
/// pod of a finished run may still be terminating before the controller stops waiting for its
/// container's end and records the run without an exit code.
pub const TERMINATION_SLACK: Duration = Duration::from_secs(60);

/// The reason a run's end carries when its pod never finished terminating.
pub const NEVER_TERMINATED: &str = "PodNeverFinishedTerminating";

/// Whether `pod` is being deleted and its deletion deadline plus [`TERMINATION_SLACK`] is past
/// at `now`.
fn stuck(pod: &Pod, now: JiffTimestamp) -> bool {
    let Some(deleted) = pod.metadata.deletion_timestamp.as_ref() else {
        return false;
    };
    let grace = pod
        .metadata
        .deletion_grace_period_seconds
        .unwrap_or(0)
        .max(0);
    let slack = i64::try_from(TERMINATION_SLACK.as_secs()).unwrap_or(i64::MAX);
    let limit = SignedDuration::from_secs(grace.saturating_add(slack));
    deleted
        .0
        .checked_add(limit)
        .is_ok_and(|deadline| deadline < now)
}

/// The status of the program container of `pod`, if the pod reports one.
fn program(pod: &Pod) -> Option<&ContainerStatus> {
    pod.status
        .as_ref()?
        .container_statuses
        .as_ref()?
        .iter()
        .find(|container| container.name == render::CONTAINER)
}

/// A Kubernetes time as the API server writes it.
fn wire_time(time: &Time) -> Option<Timestamp> {
    match serde_json::to_value(time) {
        Ok(serde_json::Value::String(text)) => Some(Timestamp(text)),
        _ => None,
    }
}

/// The reason of the Job's condition that says how its run ended: `Failed` for a failed run,
/// `Complete` for a succeeded one, when it holds and names one.
fn job_reason(job: &Job, phase: RunPhase) -> Option<String> {
    let kind = match phase {
        RunPhase::Failed => "Failed",
        RunPhase::Succeeded => "Complete",
        RunPhase::Active => return None,
    };
    job.status
        .as_ref()?
        .conditions
        .iter()
        .flatten()
        .find(|condition| condition.type_ == kind && condition.status == "True")?
        .reason
        .clone()
}

/// What the applied workload reports: the Deployment's availability, once its status observed
/// its generation, or the Job's run.
enum Observed {
    Deployment(Option<DeploymentAvailability>),
    Job(RunPhase, Option<Box<Job>>),
}

/// Before a `Command` resource's Job is created: deletes the Job of another generation `stale`
/// names, and a Deployment of the Job's name the resource or a predecessor controls (one an
/// earlier release made for a `Command` resource), and answers whether both are gone. Until they
/// are, the Job waits, so the old pod and the new run never share the workspace. An `AgentServe`
/// resource's Deployment waits for nothing: it is updated in place, `Recreate`.
async fn workloads_gone(
    ctx: &Context,
    namespace: &str,
    resource: &str,
    owner: &str,
    children: &Children,
    stale: Option<(&Surveyed, &String, &JobRun)>,
) -> Result<bool, kube::Error> {
    if !matches!(children.workload, Workload::Job(_)) {
        return Ok(true);
    }
    let mut gone = true;
    if let Some((job, uid, _)) = stale {
        if delete_kind(job.kind, &ctx.client, namespace, &job.name, uid).await? {
            ctx.unremember(resource, job.kind, &job.name);
        } else {
            gone = false;
        }
    }
    let deployment = children.workload.name();
    let kind = Kind::Deployment;
    if let Holding::Ours(uid) | Holding::Predecessor(uid) =
        held_kind(kind, &ctx.client, namespace, &deployment, resource, owner).await?
    {
        if delete_kind(kind, &ctx.client, namespace, &deployment, &uid).await? {
            ctx.unremember(resource, kind, &deployment);
        } else {
            gone = false;
        }
    }
    Ok(gone)
}

/// Applies `children`, and answers what the applied workload reports. A Job of this generation
/// that already runs (`running`, the phase it was read with) is not sent again: its pod template
/// cannot change, and one rendered since (a new default image, new default resources) would be
/// refused as immutable for as long as the run lasts.
async fn apply_children(
    client: &Client,
    namespace: &str,
    children: &Children,
    running: Option<RunPhase>,
) -> Result<Observed, kube::Error> {
    apply(client, namespace, &children.service_account).await?;
    apply(client, namespace, &children.config_map).await?;
    let observed = match (&children.workload, running) {
        (Workload::Deployment(deployment), _) => {
            Observed::Deployment(availability(&apply(client, namespace, deployment).await?))
        }
        (Workload::Job(_), Some(phase)) => Observed::Job(phase, None),
        (Workload::Job(job), None) => {
            let applied = apply(client, namespace, job).await?;
            Observed::Job(run_phase(&applied), Some(Box::new(applied)))
        }
    };
    apply(client, namespace, &children.network_policy).await?;
    if let Some(service) = &children.service {
        apply(client, namespace, service).await?;
    }
    Ok(observed)
}

/// Publishes `row` as the status of the resource `name`, unless `current` already reads so.
async fn publish(
    ctx: &Context,
    namespace: &str,
    name: &str,
    current: Option<&crate::resource::LoomAgentStatus>,
    row: &LoomAgents,
) -> Result<(), kube::Error> {
    let mut status = status::status(row);
    if let Some(code) = ctx.pods_refused(name) {
        status::pods_unread(&mut status, code);
    }
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
                let refusal = not_owned(&found, admitted.spec.as_ref());
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
            variant: agent.spec.variant(),
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
            let job = found
                .iter()
                .find_map(|child| match (&child.holding, &child.run) {
                    (Holding::Ours(uid), Some(run)) => Some((child, uid, run)),
                    _ => None,
                });
            if !predecessors.is_empty() {
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
            let running = job
                .filter(|(_, _, run)| run.generation == agent.metadata.generation)
                .map(|(_, _, run)| run);
            if running.is_none()
                && let Some(seen) = ctx.seen_over(&name, agent.metadata.generation)
            {
                let observed =
                    observation(&ctx, &namespace, &key, Some(&seen.job), seen.phase, false).await?;
                if let Some(observed) = observed {
                    ctx.record().observe_run(observed)?;
                }
                if let Some(row) = row(&ctx, &key) {
                    publish(&ctx, &namespace, &name, agent.status.as_ref(), &row).await?;
                }
                return Ok(Action::requeue(REQUEUE));
            }
            if running.is_none()
                && !workloads_gone(&ctx, &namespace, &name, &owner, children, job).await?
            {
                if let Some(row) = row(&ctx, &key) {
                    publish(&ctx, &namespace, &name, agent.status.as_ref(), &row).await?;
                }
                return Ok(Action::requeue(RETRY));
            }
            let mut ending = false;
            match running {
                Some(run) if run.phase != RunPhase::Active => {
                    ctx.saw_over(&name, run);
                    match observation(&ctx, &namespace, &key, Some(&run.job), run.phase, true)
                        .await?
                    {
                        Some(observed) => {
                            ctx.record().observe_run(observed)?;
                        }
                        None => ending = true,
                    }
                }
                _ => {
                    let current: BTreeSet<(Kind, String)> = wanted(children).into_iter().collect();
                    for (kind, child) in &current {
                        ctx.remember(&name, *kind, child);
                    }
                    let phase = running.map(|run| run.phase);
                    match apply_children(&ctx.client, &namespace, children, phase).await? {
                        Observed::Deployment(Some(availability)) => {
                            ctx.record().observe_deployment(ObserveDeployment {
                                name: key.clone(),
                                availability,
                            })?;
                        }
                        Observed::Deployment(None) => {}
                        Observed::Job(phase, applied) => {
                            let job = applied.as_deref().or(running.map(|run| &run.job));
                            match observation(&ctx, &namespace, &key, job, phase, true).await? {
                                Some(observed) => {
                                    ctx.record().observe_run(observed)?;
                                }
                                None => ending = true,
                            }
                        }
                    }
                    if !owner.is_empty() {
                        delete_owned(&ctx, &namespace, &name, &owner, &current).await?;
                    }
                }
            }
            if ending {
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
/// the configured namespace and every Deployment and Job they own.
///
/// # Errors
/// The plugin catalog cannot be loaded.
pub async fn run(client: Client, config: Config) -> Result<(), String> {
    let namespace = config.namespace.clone();
    let ctx = Arc::new(Context::new(client.clone(), config)?);
    let agents: Api<LoomAgent> = Api::namespaced(client.clone(), &namespace);
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
    let jobs: Api<Job> = Api::namespaced(client, &namespace);
    let forget = Arc::clone(&ctx);
    Controller::new(agents, watcher::Config::default())
        .owns(deployments, watcher::Config::default())
        .owns(jobs, watcher::Config::default())
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
