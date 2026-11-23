//! What an engine that runs trees in-process needs beside this crate's functions, so that it
//! depends on `b10x-loom-tree` alone: a runner on a cgroup root it holds, the host's protocol
//! catalog, the encoder of the run-log lines its observer gets, the cancel handle and the
//! principal an `Approve` decision names.
//!
//! [`TreeRunner`] builds the runner [`RunSettings::runner`](crate::RunSettings::runner) takes the
//! way `b10x-loom tree run` builds its own: Substrate's `linux` adapter on a delegated cgroup root
//! this process holds ([`TreeRunner::on_root`], [`TreeRunner::in_own_scope`]), or Substrate's
//! `local` adapter for [`UNCONFINED_FLAG_REASON`](crate::UNCONFINED_FLAG_REASON)
//! ([`TreeRunner::local`]). It never discovers a root: a runner whose driver looks for an
//! ancestor may take one another run uses, and opening a driver there ends that run's programs.
//!
//! The module narrows nothing: [`RunSettings::runner`](crate::RunSettings::runner) takes any
//! [`TestRunner`], and [`EngineRunner::open`](crate::EngineRunner::open) without a root still lets
//! its driver discover one. This is the path that does not.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    Backend, CgroupRootHold, ChildCommand, ConfinementAdapter, EngineConfinement, EngineRunner,
    SubstrateRunner, TestExecution, TestRunner, hold_cgroup_root, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::executor::TestCommand;
use loom::tree::TreeRefusalReason;
use loom_protocols::InstallStore;

use crate::Refused;

/// The cancel handle every port of a run holds; [`run_tree`](crate::run_tree) sets it from the
/// caller's flag, and a program it is set for is ended.
pub use b10x_loom_intake_slice::confinement::Cancel;
/// Why a runner refused to launch, as the [`TestRunner`] port answers it.
pub use b10x_loom_intake_slice::confinement::ConfinementError;
/// The JSON line of a run-log line, without its line feed: the bytes the case's log holds for it,
/// the line feed after them.
pub use b10x_loom_runlog::encode as encode_line;
/// The protocol catalog [`run_tree`](crate::run_tree), [`tree_status`](crate::tree_status) and
/// [`OpenRequest::catalog`](crate::OpenRequest::catalog) take.
pub use loom_protocols::ProtocolCatalog;

/// The principal an `Approve` decision names (`NextDecision.principal`) for a capability no
/// `person` binding of the case holds, and the one its printed `tree approve` command records.
pub use crate::run::DEFAULT_PRINCIPAL;

/// The host's protocol catalog: the bundled protocols and the user's installed ones
/// (`$XDG_DATA_HOME/loom/protocols`, else `$HOME/.local/share/loom/protocols`), as `b10x-loom`
/// reads it.
///
/// # Errors
/// `UnknownProtocol` naming why the installed catalog cannot be read.
pub fn host_catalog() -> Result<ProtocolCatalog, Refused> {
    InstallStore::user()
        .and_then(|store| store.catalog())
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the host's protocol catalog cannot be read: {error}"),
            )
        })
}

/// The runner of an embedded tree run, on a cgroup root this process holds or unconfined.
///
/// The runner it hands out ([`TreeRunner::runner`]) holds the root: the root is released only
/// once this value and every clone of that runner are dropped, so no runner outlives its hold.
/// A root is held once per process: a second [`TreeRunner::on_root`] on a root this process holds
/// is refused as one another process holds.
pub struct TreeRunner {
    runner: Arc<HeldRunner>,
}

impl TreeRunner {
    /// Substrate's `linux` adapter on `root`, which this process holds from now on; never a root
    /// discovered.
    ///
    /// # Errors
    /// `ConfinementRefused`, naming the root, as `b10x-loom tree run` words it: a root that cannot
    /// be resolved or opened, one another process or another runner of this process holds, and
    /// one that does not qualify (no cgroup v2 directory, not writable, not delegated, this
    /// process not in a child of it, not empty).
    pub fn on_root(root: &Path) -> Result<Self, Refused> {
        let hold = hold_cgroup_root(root).map_err(confinement_refused)?;
        let substrate = SubstrateRunner::new(Some(hold.root().to_owned()));
        Ok(Self {
            runner: Arc::new(HeldRunner {
                inner: Box::new(substrate),
                hold: Some(Arc::new(hold)),
            }),
        })
    }

    /// For a process started in a delegated scope (`systemd-run --user --scope -p
    /// Delegate=yes`): moves the calling process, every thread of it, into a leaf of the scope,
    /// hands the scope's children cpu, memory and pids, and holds the scope as
    /// [`TreeRunner::on_root`] does. Calling it accepts that move.
    ///
    /// # Errors
    /// `ConfinementRefused`: the process is in no cgroup v2, the leaf cannot be made or entered,
    /// or the scope does not qualify as [`TreeRunner::on_root`] says.
    pub fn in_own_scope() -> Result<Self, Refused> {
        let scope = prepare_delegated_cgroup().map_err(confinement_refused)?;
        Self::on_root(&scope)
    }

    /// Substrate's `local` adapter, unconfined and declared insecure, opened for
    /// [`UNCONFINED_FLAG_REASON`](crate::UNCONFINED_FLAG_REASON), the one reason the bound-program
    /// port admits; its runner answers that reason ([`TestRunner::local_reason`]). No root is held.
    ///
    /// # Errors
    /// `ConfinementRefused`: the host's policy forbids `local`.
    pub fn local() -> Result<Self, Refused> {
        let engine = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .map_err(confinement_refused)?;
        Ok(Self {
            runner: Arc::new(HeldRunner {
                inner: Box::new(engine),
                hold: None,
            }),
        })
    }

    /// The runner for [`RunSettings::runner`](crate::RunSettings::runner). It holds the root
    /// itself: each clone keeps it held after this value is dropped.
    pub fn runner(&self) -> Arc<dyn TestRunner> {
        Arc::clone(&self.runner) as Arc<dyn TestRunner>
    }
}

impl fmt::Debug for TreeRunner {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("TreeRunner")
            .field("label", &self.runner.inner.label())
            .field(
                "root",
                &self.runner.hold.as_ref().map(|hold| hold.root().to_owned()),
            )
            .finish()
    }
}

/// `error` as the refusal of a tree command: `ConfinementRefused` saying it.
fn confinement_refused(error: ConfinementError) -> Refused {
    Refused::new(TreeRefusalReason::ConfinementRefused, error.to_string())
}

/// A runner and the hold of the root it launches under, released when the last clone of the
/// `Arc` that holds this value is dropped.
struct HeldRunner {
    inner: Box<dyn TestRunner>,
    /// Held for as long as the runner lives; `None` under `local`.
    hold: Option<Arc<CgroupRootHold>>,
}

/// Every method answers what the inner runner answers, the defaulted ones included.
impl TestRunner for HeldRunner {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.inner.run_child(child)
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}
