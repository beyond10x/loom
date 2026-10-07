//! Acceptance for `story:connector-action-binding`: a consequential action selected in a Loom run
//! leaves Loom only as a `ProposedAction` (Atlas ADR 0082). Loom makes no Connector invocation and
//! links no Connectors crate; the invocation is Commission's.
//!
//! 1. A run on a frontier admitting `repository.merge`, whose scripted selector picks it, returns
//!    the proposal of `repository.merge` with the arguments its scripted generator produced. Loom is
//!    built from a selector, an argument generator and a prompt and is handed no effect port; an
//!    `EffectPort` invocation takes an `AdmittedRequest`, which only Commission's runtime makes.
//! 2. `b10x-loom-executor`'s dependency graph, normal edges only and transitively, read from
//!    `cargo metadata --locked --offline --all-features` on this tree when the test runs, holds no
//!    package whose name starts with `connectors` or `b10x-connectors`. The same check, applied to
//!    the recorded fixture `tests/fixtures/connector-boundary/metadata-with-connectors.json`,
//!    refuses and names the two Connectors packages that fixture reaches over normal edges.
//!
//! Paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    FrontierData, FrontierId, PrincipalId, ProposedActionArguments, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CHG-1842";

const PROMPT: &str = "land the change";

/// The consequential action the frontier admits and the selector picks.
const MERGE: &str = "repository.merge";

/// An admissible sibling, so the selector has a choice to make.
const INSPECT: &str = "repository.inspect";

/// The package whose dependency graph is checked.
const ROOT: &str = "b10x-loom-executor";

/// Name prefixes of Connectors packages.
const CONNECTOR_PREFIXES: [&str; 2] = ["connectors", "b10x-connectors"];

/// The recorded metadata fixture, under this crate's `tests/`.
const FIXTURE: &str = "fixtures/connector-boundary/metadata-with-connectors.json";

/// The Connectors packages the fixture reaches from the root over normal edges, sorted:
/// `connectors-client` through `b10x-loom-commission`, and `b10x_connectors_secrets` behind a
/// `cfg(unix)` edge of `connectors-client`. Its `connectors-build` is only a build dependency and its
/// `connectors-conformance` only a dependency of a dev dependency, so neither is named.
const FIXTURE_CONNECTORS: [&str; 2] = ["b10x_connectors_secrets", "connectors-client"];

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn admissible(action: &str) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf00)),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 2,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![admissible(INSPECT), admissible(MERGE)],
    })
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The arguments the scripted generator produces for the merge.
fn merge_arguments() -> Value {
    Value::Object(vec![
        ("pull_request".to_owned(), Value::Number("42".to_owned())),
        ("method".to_owned(), Value::Text("squash".to_owned())),
    ])
}

/// A selector that picks [`MERGE`] and records every candidate set it is handed.
#[derive(Default)]
struct PicksMerge(Rc<RefCell<Vec<Vec<CatalogueEntry>>>>);

impl ActionSelector for PicksMerge {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.0.borrow_mut().push(candidates.to_vec());
        Ok(Choice {
            action: MERGE.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// An argument generator that answers [`merge_arguments`] and records every entry it is handed.
#[derive(Default)]
struct MergeArguments(Rc<RefCell<Vec<CatalogueEntry>>>);

impl ArgumentGenerator for MergeArguments {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.borrow_mut().push(entry.clone());
        Ok(merge_arguments())
    }
}

/// This crate's directory, read when the test runs.
fn crate_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo")),
    )
}

/// The workspace root of the tree under test.
fn workspace_root() -> PathBuf {
    let root = crate_dir().join("../..");
    root.canonicalize().unwrap_or(root)
}

/// `cargo metadata --locked --offline --all-features --format-version 1` for this tree: every
/// optional feature and, with no `--filter-platform`, every target, so a Connectors crate behind a
/// feature or a `cfg(target)` table is in the graph too.
fn real_metadata() -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(workspace_root())
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--all-features",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap_or_else(|error| panic!("run `cargo metadata`: {error}"));
    assert!(
        output.status.success(),
        "`cargo metadata` failed ({}):\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("`cargo metadata` printed no JSON: {error}"))
}

fn fixture_metadata() -> serde_json::Value {
    let path = crate_dir().join("tests").join(FIXTURE);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The boundary check's refusal: the Connectors packages [`ROOT`] reaches over normal edges.
#[derive(Debug, PartialEq, Eq)]
struct ConnectorsLinked {
    /// Their package names as the metadata spells them, sorted and without repeats.
    packages: Vec<String>,
}

impl fmt::Display for ConnectorsLinked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{ROOT} reaches Connectors packages over normal dependency edges: {}",
            self.packages.join(", ")
        )
    }
}

/// Whether `name` is a Connectors package: it starts with one of [`CONNECTOR_PREFIXES`], read in
/// lower case and with `_` as `-`, so `b10x_connectors_secrets` is one too.
fn is_connector(name: &str) -> bool {
    let name = name.to_ascii_lowercase().replace('_', "-");
    CONNECTOR_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The boundary check over one `cargo metadata --format-version 1` document: every package
/// [`ROOT`] reaches over normal dependency edges, transitively and for any target, whose name
/// [`is_connector`]. A dev or build edge is not followed. Edges are read from the resolve graph's
/// `deps` with their `dep_kinds`, and a package's name from `packages` by its id.
///
/// A document this check cannot clear panics rather than passing: one without a resolve graph (read
/// with `--no-deps`), without exactly one [`ROOT`], or with an edge to a package it does not list.
fn check(metadata: &serde_json::Value) -> Result<(), ConnectorsLinked> {
    let names: BTreeMap<&str, &str> = metadata["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("the metadata has no `packages` array"))
        .iter()
        .map(|package| {
            let field = |key: &str| {
                package[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("a package without `{key}`: {package}"))
            };
            (field("id"), field("name"))
        })
        .collect();
    let nodes: BTreeMap<&str, &serde_json::Value> = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("the metadata has no resolve graph (read with `--no-deps`?)"))
        .iter()
        .map(|node| {
            let id = node["id"]
                .as_str()
                .unwrap_or_else(|| panic!("a resolve node without `id`: {node}"));
            (id, node)
        })
        .collect();
    let roots: Vec<&str> = names
        .iter()
        .filter(|(_, name)| **name == ROOT)
        .map(|(id, _)| *id)
        .collect();
    let [root] = roots.as_slice() else {
        panic!("expected exactly one {ROOT} package, found {roots:?}");
    };

    let mut reached = BTreeSet::from([*root]);
    let mut pending = vec![*root];
    while let Some(id) = pending.pop() {
        let node = nodes
            .get(id)
            .unwrap_or_else(|| panic!("no resolve node for {id}"));
        let deps = node["deps"]
            .as_array()
            .unwrap_or_else(|| panic!("a resolve node without `deps`: {id}"));
        for dep in deps {
            let normal = dep["dep_kinds"]
                .as_array()
                .unwrap_or_else(|| panic!("an edge of {id} without `dep_kinds`: {dep}"))
                .iter()
                .any(|kind| kind["kind"].is_null());
            if !normal {
                continue;
            }
            let pkg = dep["pkg"]
                .as_str()
                .unwrap_or_else(|| panic!("an edge of {id} without `pkg`: {dep}"));
            if reached.insert(pkg) {
                pending.push(pkg);
            }
        }
    }

    let mut packages: Vec<String> = reached
        .iter()
        .map(|id| {
            *names
                .get(id)
                .unwrap_or_else(|| panic!("an edge reaches {id}, which `packages` does not list"))
        })
        .filter(|name| is_connector(name))
        .map(str::to_owned)
        .collect();
    packages.sort();
    packages.dedup();
    if packages.is_empty() {
        Ok(())
    } else {
        Err(ConnectorsLinked { packages })
    }
}

#[test]
fn consequential_actions_leave_loom_only_as_a_proposal() {
    // 1. Loom proposes the merge its selector picked, with the generated arguments. It was built
    // from a selector, an argument generator and a prompt; no effect port was handed to it.
    let handed = Rc::default();
    let generated = Rc::default();
    let loom = Loom::new(
        PicksMerge(Rc::clone(&handed)),
        MergeArguments(Rc::clone(&generated)),
        PROMPT,
    );
    let outcome = loom.run(&commission(), &frontier());
    assert_eq!(
        outcome,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: MERGE.to_owned(),
            arguments: ProposedActionArguments(merge_arguments()),
        }),
        "the selected merge leaves Loom as its proposal, with its arguments"
    );
    let merge_entry = CatalogueEntry {
        action: MERGE.to_owned(),
        status: CatalogueEntryStatus::Admissible,
    };
    assert_eq!(handed.borrow().len(), 1, "the selector is asked once");
    assert!(
        handed.borrow()[0].contains(&merge_entry),
        "the catalogue projected from the frontier lists {MERGE}: {:?}",
        handed.borrow()
    );
    assert_eq!(
        generated.borrow().as_slice(),
        std::slice::from_ref(&merge_entry),
        "arguments are generated once, for {MERGE}"
    );

    // 2. This tree: no Connectors package in the executor's normal dependency graph.
    if let Err(linked) = check(&real_metadata()) {
        panic!("{linked}");
    }

    // 2. The recorded fixture: the same check refuses and names what it reaches.
    assert_eq!(
        check(&fixture_metadata()),
        Err(ConnectorsLinked {
            packages: FIXTURE_CONNECTORS.map(str::to_owned).to_vec(),
        }),
        "the check refuses the fixture's Connectors packages reached over normal edges, and only \
         those"
    );
}
