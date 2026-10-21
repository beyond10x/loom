//! Adversary pass 1 on `story:agent-executor-host-port` (wave 2026-10-20-w4, unit U5).
//!
//! `host_ports.rs` holds `AgentExecutor` with a unit struct: `Sync`, `'static`, and never used as
//! a trait object. The hosts that implement the port are not like it. takt's `SessionExecutor`
//! writes its report into a `RefCell`, so it is not `Sync`; control-plane's
//! `ImplementationExecutor<'_>` and `Publication<'_>` borrow the phase they run in, so they are not
//! `'static`; and the runtime takes `E: AgentExecutor + ?Sized`, so a host may hand it
//! `&dyn AgentExecutor`. A release that adds `Sync` or `'static` to the trait, or makes it
//! dyn-incompatible, breaks those hosts while `host_ports.rs` still compiles. The host executor
//! here has their shape.
//!
//! The page's ports table also leaves out traits a host supplies by their own documentation, and
//! no case reads the kind column of its control-plane table, which this unit changed.
//!
//! Nothing here reaches a model or the network.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

use loom_sdk::commission::model::json::Value;
use loom_sdk::commission::model::primitives::Uuid;
use loom_sdk::commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorStep, Frontier, FrontierData,
    FrontierId, PrincipalId, ProposedActionArguments, commission_state, frontier_state,
};
use loom_sdk::commission::ports::executor::AgentExecutor;

/// The stability page, relative to the crate.
const PAGE: &str = "../../website/docs/reference/sdk-stability.md";

/// The page's heading over the ports a host implements.
const PORTS_HEADING: &str = "## Ports a host implements";

/// The page's heading over the paths control-plane imports.
const CONTROL_PLANE_HEADING: &str = "## Used by control-plane";

/// A host executor with the shape of the hosts that implement the port: it borrows what it
/// proposes, as control-plane's `ImplementationExecutor<'_>` borrows its phase, and records each
/// frontier it is called on in a `RefCell`, as takt's `SessionExecutor` records its report.
struct BorrowingExecutor<'a> {
    proposes: &'a ExecutorOutcome,
    called_on: RefCell<Vec<FrontierId>>,
}

impl AgentExecutor for BorrowingExecutor<'_> {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.called_on
            .borrow_mut()
            .push(frontier.data().frontier_id.clone());
        self.proposes.clone()
    }
}

fn commission_and_frontier() -> (
    Commission<commission_state::Assigned>,
    Frontier<frontier_state::Issued>,
) {
    let case = CaseId("case-1".into());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".into()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-000000000003".into())),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: Vec::new(),
    });
    (commission, frontier)
}

/// A host executor that is neither `Sync` nor `'static`, handed over as `&dyn AgentExecutor`,
/// keeps the provided `run_with_usage`: the call returns its `run`'s outcome, a proposal no
/// default produces, with no usage, and calls `run` once, on the frontier it was given.
#[test]
fn a_host_executor_that_borrows_and_holds_a_refcell_is_called_as_a_trait_object() {
    let proposes = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: "ticket.route".into(),
        arguments: ProposedActionArguments(Value::Null),
    });
    let host = BorrowingExecutor {
        proposes: &proposes,
        called_on: RefCell::new(Vec::new()),
    };
    let (commission, frontier) = commission_and_frontier();

    let executor: &dyn AgentExecutor = &host;
    let step = executor.run_with_usage(&commission, &frontier);

    assert_eq!(
        step,
        ExecutorStep {
            outcome: proposes.clone(),
            usage: None,
            transcript_digest: None,
            summary: None,
        },
        "the provided run_with_usage returns the host's run outcome with no usage"
    );
    assert_eq!(
        *host.called_on.borrow(),
        vec![FrontierId(Uuid(
            "00000000-0000-4000-8000-000000000003".into()
        ))],
        "the provided run_with_usage calls the host's run once, on the frontier it was given"
    );
}

/// The traits a host supplies by their own documentation or the runtime's signature, each named
/// through `loom_sdk` alone: compilation is the witness that the path resolves.
#[allow(unused_imports)]
mod host_supplied {
    use loom_sdk::commission::ports::governor::Governor;
    use loom_sdk::connectors::CredentialResolver;
    use loom_sdk::intake::slice::confinement::TestRunner;
    use loom_sdk::loom::model_selector::Briefing;
}

/// The page's ports table lists every trait a host supplies: the page says "Each port is a trait
/// the host implements and hands to the runtime", and `host_ports.rs` says it names "exactly the
/// ports a host implements".
#[test]
fn the_ports_table_lists_every_trait_a_host_supplies() {
    let supplied = [
        (
            "Briefing",
            "crates/loom-executor/src/model_selector.rs:52-53: \"A host implements the first four\"",
        ),
        (
            "CredentialResolver",
            "crates/loom-connectors/src/lib.rs:71: \"The host supplies it\"",
        ),
        (
            "Governor",
            "crates/loom-commission/src/runtime.rs:717: run_until_blocked takes \
             `G: Governor + ObservationPort + ?Sized`, and ObservationPort is on the table",
        ),
        (
            "TestRunner",
            "crates/loom-intake-slice/src/confinement.rs:43: \"The trusted test execution port\", \
             handed in through `with_runner(Arc<dyn TestRunner>)`",
        ),
    ];
    let page = page();
    let listed: BTreeSet<String> = rows(&page, PORTS_HEADING)
        .into_iter()
        .map(|row| row[0].clone())
        .collect();
    let missing: Vec<String> = supplied
        .iter()
        .filter(|(name, _)| !listed.contains(*name))
        .map(|(name, why)| format!("{name}: {why}"))
        .collect();
    assert!(
        missing.is_empty(),
        "the page's ports table leaves out traits a host supplies: {missing:#?}"
    );
}

/// The control-plane table's kind is `port` exactly for the paths whose trait the ports table
/// lists. This unit relabelled `AgentExecutor` there from `trait` to `port`; no case reads the
/// kind column, so reverting the relabel left the suite green.
#[test]
fn a_control_plane_path_is_kind_port_exactly_when_the_ports_table_lists_it() {
    let page = page();
    let ports: BTreeSet<String> = rows(&page, PORTS_HEADING)
        .into_iter()
        .map(|row| row[0].clone())
        .collect();
    let control_plane = rows(&page, CONTROL_PLANE_HEADING);
    let disagree: Vec<String> = control_plane
        .iter()
        .filter(|row| {
            let name = row[0].rsplit("::").next().unwrap_or(&row[0]);
            ports.contains(name) != (row[1] == "port")
        })
        .map(|row| format!("{} is `{}`", row[0], row[1]))
        .collect();
    assert!(
        disagree.is_empty(),
        "a control-plane row is `port` exactly when the ports table lists its trait: {disagree:#?}"
    );
    assert!(
        control_plane.iter().any(|row| row[0]
            == "loom_sdk::commission::ports::executor::AgentExecutor"
            && row[1] == "port"),
        "the control-plane table lists AgentExecutor as a port"
    );
}

/// The stability page, read at run time.
fn page() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PAGE);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the stability page {} is read: {error}", path.display()))
}

/// The cells of each row of the first table under `heading`, each trimmed and without its
/// backticks; the header row and the separator row are skipped.
fn rows(page: &str, heading: &str) -> Vec<Vec<String>> {
    let mut lines = page.lines().skip_while(|line| line.trim_end() != heading);
    assert!(
        lines.next().is_some(),
        "the stability page has a `{heading}` section"
    );
    let table: Vec<&str> = lines
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .collect();
    assert!(table.len() > 2, "`{heading}` has a table with rows");
    table[2..]
        .iter()
        .map(|row| {
            let cells: Vec<&str> = row.split('|').collect();
            cells[1..cells.len() - 1]
                .iter()
                .map(|cell| cell.trim().trim_matches('`').to_owned())
                .collect()
        })
        .collect()
}
