//! Acceptance for `story:sdk-host-port-stability`: the stability page,
//! `website/docs/reference/sdk-stability.md`, names exactly the ports a host implements and the
//! `loom_sdk` paths control-plane imports, and every one of them resolves through `loom_sdk`.
//!
//! The witness is compilation. Every port is implemented below by a host type with trivial
//! bodies, naming only `loom_sdk::` paths: a port the SDK stops exporting, or whose signature
//! changes, fails this file before any case runs. Every path of the page's control-plane table is
//! one `use` line in [`control_plane`]. The cases then read the page and assert that its tables
//! list exactly what this file implements and imports, so the page cannot name a path that does
//! not exist, nor leave one out.
//!
//! Nothing here reaches a model or the network. No port body is called, except the host
//! executor's `run`, which [`a_host_executor_implementing_run_alone_keeps_the_provided_run_with_usage`]
//! reaches through the one call of the provided `AgentExecutor::run_with_usage`.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// The page this file holds, relative to the crate.
const PAGE: &str = "../../website/docs/reference/sdk-stability.md";

/// The page's heading over the ports a host implements.
const PORTS_HEADING: &str = "## Ports a host implements";

/// The page's heading over the paths control-plane imports.
const CONTROL_PLANE_HEADING: &str = "## Used by control-plane";

/// The page's heading over the types and functions an embedding calls.
const EMBEDDING_HEADING: &str = "## Types and functions an embedding calls";

/// Implements each port for its host value and records the port's path, as written, in
/// `IMPLEMENTED`: the list the page's port table is compared with.
macro_rules! host_ports {
    ($($port:path => $host:expr;)*) => {
        /// Every port this file implements, each as its `loom_sdk` path.
        const IMPLEMENTED: &[&str] = &[$(stringify!($port)),*];

        /// Holds each host value to its port; the bound is the witness.
        fn witness() {
            $({
                fn holds<T: $port>(_: T) {}
                holds($host);
            })*
        }
    };
}

host_ports! {
    loom_sdk::commission::ports::authority::AuthorityProvider => hosts::Authority;
    loom_sdk::commission::ports::suspension::SuspensionPort => hosts::Suspensions;
    loom_sdk::governor::FallibleCaseStore => hosts::FallibleCases;
    loom_sdk::governor::CaseStore => hosts::Cases;
    loom_sdk::governor::EvaluationTime => hosts::Time;
    loom_sdk::commission::ports::effect::EffectPort => hosts::Effects;
    loom_sdk::commission::ports::connector::ConnectorInvoker => hosts::Connectors;
    loom_sdk::commission::ports::evidence::ObservationPort => hosts::Observations;
    loom_sdk::commission::ports::evidence::EvidencePort => hosts::Evidence;
    loom_sdk::commission::ports::executor::AgentExecutor => hosts::Executor;
    loom_sdk::loom::selection::ActionSelector => hosts::Selector;
    loom_sdk::loom::workflow::WorkflowRunStore => hosts::RunRecords;
    loom_sdk::loom::arguments::ArgumentGenerator => hosts::Arguments;
    loom_sdk::loom::harness::wire::ModelPort => hosts::Model;
    loom_sdk::loom::harness::wire::ToolPort => hosts::Tools;
    loom_sdk::loom::harness::wire::BearerSource => hosts::Bearers;
    loom_sdk::loom::harness::turn_loop::ApprovalPort => hosts::Approvals;
    loom_sdk::loom::harness::turn_loop::TurnEnvironmentProvider => hosts::Environment;
    loom_sdk::loom::harness::turn_loop::HookPort => hosts::Hooks;
    loom_sdk::loom::harness::turn_loop::LoopSink => hosts::LoopEvents;
    loom_sdk::loom::harness::wire::StreamSink => hosts::StreamEvents;
    loom_sdk::commission::runtime::LoopContext => hosts::Context;
}

/// One host type per port, each implemented through `loom_sdk` paths alone.
mod hosts {
    use loom_sdk::commission::model::json::Value;
    use loom_sdk::commission::model::primitives::Timestamp;
    use loom_sdk::commission::model::responsibility::{
        ActionBindingData, ActionRequestId, AuthorityVerdict, CaseId, Commission, CommissionData,
        CommissionId, EffectOutcome, ExecutorOutcome, Frontier, GovernorError, HoldSuspension,
        HoldSuspensionOutcome, Observation, ObservationData, ObservationId, ReleaseSuspension,
        ReleaseSuspensionOutcome, SuspensionSnapshot, TakeSuspension, TakeSuspensionOutcome, Unit,
        commission_state, frontier_state, observation_state,
    };
    use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
    use loom_sdk::commission::ports::connector::ConnectorInvoker;
    use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
    use loom_sdk::commission::ports::evidence::{
        AttributedEvidence, EvidencePort, ObservationPort,
    };
    use loom_sdk::commission::ports::executor::AgentExecutor;
    use loom_sdk::commission::ports::suspension::{SuspensionPort, SuspensionStoreError};
    use loom_sdk::commission::runtime::LoopContext;
    use loom_sdk::governor::{CaseState, CaseStore, EvaluationTime, FallibleCaseStore};
    use loom_sdk::loom::arguments::{ArgumentContext, ArgumentGenerator};
    use loom_sdk::loom::harness::turn_loop::{
        ApprovalDecision, ApprovalPort, EnvironmentError, HookPort, LoopEvent, LoopSink,
        TurnEnvironment, TurnEnvironmentProvider, TurnEnvironmentRequest,
    };
    use loom_sdk::loom::harness::wire::{
        Bearer, BearerSource, ModelPort, StreamEvent, StreamSink, ToolCall, ToolOutcome, ToolPort,
        ToolSpec, TurnOutcome, TurnRequest, WireError, WireId,
    };
    use loom_sdk::loom::model::run::{CatalogueEntry, CommissionRunId, SelectionStrategy};
    use loom_sdk::loom::model::workflow::WorkflowRunRecord;
    use loom_sdk::loom::selection::{ActionSelector, Choice, SelectionContext, SelectorError};
    use loom_sdk::loom::workflow::{RunStoreError, WorkflowRunStore};

    /// The body every port method has here: the witness compiles them and never calls them.
    const NEVER_CALLED: &str = "a compile witness: no port body is called";

    pub struct Authority;

    impl AuthorityProvider for Authority {
        fn decide(
            &self,
            _commission: &CommissionData,
            _request: &loom_sdk::commission::model::responsibility::ActionRequestData,
            _capability: &str,
        ) -> Result<AuthorityVerdict, AuthorityProviderError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Suspensions;

    impl SuspensionPort for Suspensions {
        fn hold(&self, _input: HoldSuspension) -> HoldSuspensionOutcome {
            unreachable!("{NEVER_CALLED}")
        }

        fn take(&self, _input: TakeSuspension) -> TakeSuspensionOutcome {
            unreachable!("{NEVER_CALLED}")
        }

        fn release(&self, _input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
            unreachable!("{NEVER_CALLED}")
        }

        fn get(
            &self,
            _commission: &CommissionId,
        ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct FallibleCases;

    impl FallibleCaseStore for FallibleCases {
        type Error = String;

        fn insert(&self, _state: CaseState) -> Result<bool, Self::Error> {
            unreachable!("{NEVER_CALLED}")
        }

        fn get(&self, _case: &CaseId) -> Result<Option<CaseState>, Self::Error> {
            unreachable!("{NEVER_CALLED}")
        }

        fn update<T>(
            &self,
            _case: &CaseId,
            _change: impl FnOnce(&mut CaseState) -> T,
        ) -> Result<Option<T>, Self::Error> {
            unreachable!("{NEVER_CALLED}")
        }

        fn observe(&self, _observation: ObservationData) -> Result<(), Self::Error> {
            unreachable!("{NEVER_CALLED}")
        }

        fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Cases;

    impl CaseStore for Cases {
        fn insert(&self, _state: CaseState) -> bool {
            unreachable!("{NEVER_CALLED}")
        }

        fn get(&self, _case: &CaseId) -> Option<CaseState> {
            unreachable!("{NEVER_CALLED}")
        }

        fn update<T>(
            &self,
            _case: &CaseId,
            _change: impl FnOnce(&mut CaseState) -> T,
        ) -> Option<T> {
            unreachable!("{NEVER_CALLED}")
        }

        fn observe(&self, _observation: ObservationData) {
            unreachable!("{NEVER_CALLED}")
        }

        fn observations(&self) -> Vec<ObservationData> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Time;

    impl EvaluationTime for Time {
        fn at(&self, _case: &CaseId) -> Result<Option<String>, GovernorError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct RunRecords;

    impl WorkflowRunStore for RunRecords {
        fn load(&self, _run: &CommissionRunId) -> Result<Option<WorkflowRunRecord>, RunStoreError> {
            unreachable!("{NEVER_CALLED}")
        }

        fn save(&self, _record: &WorkflowRunRecord) -> Result<(), RunStoreError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Effects;

    impl EffectPort for Effects {
        fn performs(&self, _action: &str) -> bool {
            unreachable!("{NEVER_CALLED}")
        }

        fn invoke(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _request: &AdmittedRequest,
        ) -> Result<EffectOutcome, EffectError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Connectors;

    impl ConnectorInvoker for Connectors {
        fn invoke(
            &self,
            _binding: &ActionBindingData,
            _request: &AdmittedRequest,
        ) -> Result<EffectOutcome, EffectError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Observations;

    impl ObservationPort for Observations {
        fn observe(
            &self,
            _observation: Observation<observation_state::Reported>,
        ) -> Result<(), GovernorError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Evidence;

    impl EvidencePort for Evidence {
        fn receive(&self, _evidence: AttributedEvidence) -> Result<(), GovernorError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    /// The outcome the host executor's `run` proposes for every frontier.
    pub const EXECUTOR_OUTCOME: ExecutorOutcome = ExecutorOutcome::NoUsefulAction(Unit(true));

    /// Implements `run` alone and keeps the provided `run_with_usage`, the call the runtime makes.
    /// Its `run` is the one port body here that runs: the provided method reaches it.
    pub struct Executor;

    impl AgentExecutor for Executor {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            EXECUTOR_OUTCOME
        }
    }

    pub struct Selector;

    impl ActionSelector for Selector {
        fn select(
            &self,
            _context: &SelectionContext,
            _candidates: &[CatalogueEntry],
        ) -> Result<Choice, SelectorError> {
            unreachable!("{NEVER_CALLED}")
        }

        fn strategy(&self) -> SelectionStrategy {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Arguments;

    impl ArgumentGenerator for Arguments {
        fn generate(
            &self,
            _context: &ArgumentContext,
            _entry: &CatalogueEntry,
        ) -> Result<Value, String> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Model;

    impl ModelPort for Model {
        fn wire(&self) -> &WireId {
            unreachable!("{NEVER_CALLED}")
        }

        fn turn(
            &mut self,
            _request: &TurnRequest,
            _sink: &mut dyn StreamSink,
        ) -> Result<TurnOutcome, WireError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Tools;

    impl ToolPort for Tools {
        fn specs(&self) -> &[ToolSpec] {
            unreachable!("{NEVER_CALLED}")
        }

        fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Bearers;

    impl BearerSource for Bearers {
        fn bearer(&self) -> Result<Bearer, WireError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Approvals;

    impl ApprovalPort for Approvals {
        fn decide(&mut self, _call: &ToolCall, _spec: &ToolSpec) -> ApprovalDecision {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Environment;

    impl TurnEnvironmentProvider for Environment {
        fn refresh(
            &mut self,
            _request: TurnEnvironmentRequest,
        ) -> Result<TurnEnvironment, EnvironmentError> {
            unreachable!("{NEVER_CALLED}")
        }
    }

    /// Every `HookPort` method has a default; a host overrides the ones it needs.
    pub struct Hooks;

    impl HookPort for Hooks {}

    pub struct LoopEvents;

    impl LoopSink for LoopEvents {
        fn emit(&mut self, _event: LoopEvent) {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct StreamEvents;

    impl StreamSink for StreamEvents {
        fn emit(&mut self, _event: StreamEvent) {
            unreachable!("{NEVER_CALLED}")
        }
    }

    pub struct Context;

    impl LoopContext for Context {
        fn action_request_id(&mut self) -> ActionRequestId {
            unreachable!("{NEVER_CALLED}")
        }

        fn observation_id(&mut self) -> ObservationId {
            unreachable!("{NEVER_CALLED}")
        }

        fn now(&mut self) -> Timestamp {
            unreachable!("{NEVER_CALLED}")
        }

        fn step_budget(&self) -> Option<usize> {
            unreachable!("{NEVER_CALLED}")
        }
    }
}

/// One `use` per path of the page's control-plane table, read from
/// `~/beyond10x/control-plane/crates/control-plane-runtime` and its xtask: the `loom_sdk` items it
/// imports, the items it names inline, and the items of `loom_sdk::loom::harness::wire` it names
/// through that module. Associated functions and enum variants are left to their type.
///
/// [`every_control_plane_path_on_the_page_resolves`] reads these lines out of this file, between
/// the two marker comments, so keep one path per `use` and nothing else between them.
#[allow(unused_imports)]
mod control_plane {
    // control-plane paths: begin
    use loom_sdk::ActionSelector;
    use loom_sdk::ArgumentGenerator;
    use loom_sdk::CanonGovernor;
    use loom_sdk::Loom;
    use loom_sdk::LoopContext;
    use loom_sdk::commission::model::behaviour::Generated;
    use loom_sdk::commission::model::json;
    use loom_sdk::commission::model::primitives::Timestamp;
    use loom_sdk::commission::model::primitives::Uuid;
    use loom_sdk::commission::model::responsibility;
    use loom_sdk::commission::outcome::RunStore;
    use loom_sdk::commission::ports::authority::AuthorityProvider;
    use loom_sdk::commission::ports::authority::AuthorityProviderError;
    use loom_sdk::commission::ports::effect::AdmittedRequest;
    use loom_sdk::commission::ports::effect::EffectError;
    use loom_sdk::commission::ports::effect::EffectPort;
    use loom_sdk::commission::ports::evidence::submit_evidence;
    use loom_sdk::commission::ports::executor::AgentExecutor;
    use loom_sdk::commission::ports::governor::Governor;
    use loom_sdk::governor::CaseState;
    use loom_sdk::governor::FallibleCaseStore;
    use loom_sdk::loom::arguments::ArgumentContext;
    use loom_sdk::loom::harness::turn_loop::AgentLoop;
    use loom_sdk::loom::harness::turn_loop::Budget;
    use loom_sdk::loom::harness::turn_loop::DenyAll;
    use loom_sdk::loom::harness::turn_loop::LoopConfig;
    use loom_sdk::loom::harness::turn_loop::LoopEvent;
    use loom_sdk::loom::harness::turn_loop::LoopSink;
    use loom_sdk::loom::harness::turn_loop::OutputSchema;
    use loom_sdk::loom::harness::turn_loop::RunLedger;
    use loom_sdk::loom::harness::wire;
    use loom_sdk::loom::harness::wire::Item;
    use loom_sdk::loom::harness::wire::ModelPort;
    use loom_sdk::loom::harness::wire::StreamSink;
    use loom_sdk::loom::harness::wire::ToolCall;
    use loom_sdk::loom::harness::wire::ToolName;
    use loom_sdk::loom::harness::wire::ToolOutcome;
    use loom_sdk::loom::harness::wire::ToolPort;
    use loom_sdk::loom::harness::wire::ToolSpec;
    use loom_sdk::loom::harness::wire::TurnOutcome;
    use loom_sdk::loom::harness::wire::TurnRequest;
    use loom_sdk::loom::harness::wire::Usage;
    use loom_sdk::loom::harness::wire::WireError;
    use loom_sdk::loom::harness::wire::WireErrorCode;
    use loom_sdk::loom::harness::wire::WireId;
    use loom_sdk::loom::model::primitives::Uuid as LoomUuid;
    use loom_sdk::loom::model::run::CatalogueEntry;
    use loom_sdk::loom::model::run::CommissionRunId;
    use loom_sdk::loom::model::run::RunEnding;
    use loom_sdk::loom::model::run::SelectionStrategy;
    use loom_sdk::loom::model::run::SessionData;
    use loom_sdk::loom::model::run::SessionId;
    use loom_sdk::loom::selection::Choice;
    use loom_sdk::loom::selection::SelectionContext;
    use loom_sdk::loom::selection::SelectorError;
    use loom_sdk::loom::session::SessionFile;
    use loom_sdk::run_until_blocked;
    // control-plane paths: end
}

/// One `use` per `loom_sdk::models` path of the page's embedding table: a host's models, loaded
/// from a configuration or a routes document.
///
/// [`every_models_path_on_the_page_resolves`] reads these lines out of this file, between the two
/// marker comments, so keep one path per `use` and nothing else between them.
#[allow(unused_imports)]
mod models {
    // models paths: begin
    use loom_sdk::models::ModelRole;
    use loom_sdk::models::Models;
    use loom_sdk::models::ModelsConfig;
    use loom_sdk::models::ModelsError;
    use loom_sdk::models::RoleModel;
    use loom_sdk::models::from_config;
    use loom_sdk::models::from_routes;
    use loom_sdk::models::parse_config;
    // models paths: end
}

/// The `loom_sdk::connectors` paths the page's embedding table lists: the command-line client and
/// its governed write, and the host evidence adapter.
///
/// [`every_connectors_path_on_the_page_resolves`] reads these lines out of this file, between the
/// two marker comments, so keep one path per `use` and nothing else between them.
#[allow(unused_imports)]
mod connectors {
    // connectors paths: begin
    use loom_sdk::connectors::cli::ConnectorsCli;
    use loom_sdk::connectors::cli::WriteError;
    use loom_sdk::connectors::cli::datasource::MutationClassification;
    use loom_sdk::connectors::cli::datasource::WriteApproval;
    use loom_sdk::connectors::cli::datasource::WriteOperation;
    use loom_sdk::connectors::cli::datasource::WriteRefusal;
    use loom_sdk::connectors::cli::datasource::WriteResult;
    use loom_sdk::connectors::cli::idempotency_key;
    use loom_sdk::connectors::evidence::WriteEvidenceAdapter;
    use loom_sdk::connectors::evidence::WriteEvidenceError;
    // connectors paths: end

    /// The governed write's signature, as a host calls it: compilation is the witness.
    #[allow(dead_code)]
    fn invoke_write_signature(
        cli: &ConnectorsCli,
        request: &loom_sdk::commission::ports::effect::AdmittedRequest,
        operation: &WriteOperation,
        input: &loom_sdk::connectors::cli::json::Value,
        approval: &WriteApproval,
    ) -> Result<WriteResult, WriteError> {
        let _key: String = idempotency_key(request);
        cli.invoke_write(request, operation, input, approval)
    }
}

/// The stability page, read at run time.
fn page() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PAGE);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the stability page {} is read: {error}", path.display()))
}

/// The cells of column `column` of the first table under `heading`, each without its backticks.
///
/// The table starts at the first line beginning with `|` after the heading and ends at the first
/// line that does not; its header row and the separator row under it are skipped.
fn table_column(page: &str, heading: &str, column: usize) -> Vec<String> {
    let mut lines = page.lines().skip_while(|line| line.trim_end() != heading);
    assert!(
        lines.next().is_some(),
        "the stability page has a `{heading}` section"
    );
    let rows: Vec<&str> = lines
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .collect();
    assert!(
        rows.len() > 2,
        "`{heading}` has a table with a header, a separator and at least one row"
    );
    rows[2..]
        .iter()
        .map(|row| {
            let cell = row
                .split('|')
                .nth(column + 1)
                .unwrap_or_else(|| panic!("`{heading}` row {row:?} has a column {column}"));
            cell.trim().trim_matches('`').to_owned()
        })
        .collect()
}

/// `path` without the spaces `stringify!` may put around `::`.
fn compact(path: &str) -> String {
    path.split_whitespace().collect()
}

/// Every element of `items` once, refusing a duplicate.
fn unique(items: Vec<String>, what: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for item in items {
        assert!(set.insert(item.clone()), "{what} lists {item} twice");
    }
    set
}

#[test]
fn a_host_implements_every_port_through_loom_sdk_alone() {
    // The witness is that this file compiles: each host value is held to its port's bound.
    witness();

    let names: BTreeSet<&str> = IMPLEMENTED
        .iter()
        .map(|path| path.rsplit("::").next().unwrap_or(path).trim())
        .collect();
    let story: BTreeSet<&str> = [
        "AuthorityProvider",
        "SuspensionPort",
        "FallibleCaseStore",
        "CaseStore",
        "EvaluationTime",
        "EffectPort",
        "ConnectorInvoker",
        "ObservationPort",
        "EvidencePort",
        "AgentExecutor",
        "ActionSelector",
        "WorkflowRunStore",
        "ArgumentGenerator",
        "ModelPort",
        "ToolPort",
        "BearerSource",
        "ApprovalPort",
        "TurnEnvironmentProvider",
        "HookPort",
        "LoopSink",
        "StreamSink",
        "LoopContext",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        names, story,
        "the witness implements every port a host implements"
    );
    for path in IMPLEMENTED {
        assert!(
            compact(path).starts_with("loom_sdk::"),
            "{path} is named through loom_sdk"
        );
    }
}

/// A host executor that implements `run` alone, through
/// `loom_sdk::commission::ports::executor::AgentExecutor`, keeps the provided `run_with_usage`,
/// the call the runtime makes: it returns `run`'s outcome and reports no usage. The page's
/// `AgentExecutor` row names both methods.
#[test]
fn a_host_executor_implementing_run_alone_keeps_the_provided_run_with_usage() {
    use loom_sdk::commission::model::json::Value;
    use loom_sdk::commission::model::primitives::Uuid;
    use loom_sdk::commission::model::responsibility::{
        AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
        ExecutorStep, Frontier, FrontierData, FrontierId, PrincipalId,
    };
    use loom_sdk::commission::ports::executor::AgentExecutor;

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

    let step: ExecutorStep =
        AgentExecutor::run_with_usage(&hosts::Executor, &commission, &frontier);
    assert_eq!(
        step,
        ExecutorStep {
            outcome: hosts::EXECUTOR_OUTCOME,
            usage: None,
        },
        "the provided run_with_usage returns run's outcome with no usage"
    );
}

#[test]
fn the_stability_page_lists_exactly_the_ports_this_test_implements() {
    let page = page();
    let listed = unique(table_column(&page, PORTS_HEADING, 1), "the port table");
    let implemented = unique(
        IMPLEMENTED.iter().map(|path| compact(path)).collect(),
        "the witness",
    );
    assert_eq!(
        listed, implemented,
        "the page's port table lists exactly the ports this test implements"
    );
}

#[test]
fn every_control_plane_path_on_the_page_resolves() {
    let source = include_str!("host_ports.rs");
    let imported: Vec<String> = source
        .lines()
        .skip_while(|line| line.trim() != "// control-plane paths: begin")
        .skip(1)
        .take_while(|line| line.trim() != "// control-plane paths: end")
        .map(|line| {
            let path = line
                .trim()
                .strip_prefix("use ")
                .and_then(|rest| rest.strip_suffix(';'))
                .unwrap_or_else(|| panic!("{line:?} is one `use` of one path"));
            path.split(" as ").next().unwrap_or(path).to_owned()
        })
        .collect();
    assert!(
        !imported.is_empty(),
        "the control-plane block holds the imports"
    );
    let imported = unique(imported, "the control-plane block");

    let page = page();
    let listed = unique(
        table_column(&page, CONTROL_PLANE_HEADING, 0),
        "the control-plane table",
    );
    assert_eq!(
        listed, imported,
        "the page's control-plane table equals the `use` lines that resolve here"
    );
}

/// The `use` paths of this file between `// <marker>: begin` and `// <marker>: end`.
fn used_paths(marker: &str) -> Vec<String> {
    let source = include_str!("host_ports.rs");
    let begin = format!("// {marker}: begin");
    let end = format!("// {marker}: end");
    source
        .lines()
        .skip_while(|line| line.trim() != begin)
        .skip(1)
        .take_while(|line| line.trim() != end)
        .map(|line| {
            let path = line
                .trim()
                .strip_prefix("use ")
                .and_then(|rest| rest.strip_suffix(';'))
                .unwrap_or_else(|| panic!("{line:?} is one `use` of one path"));
            path.split(" as ").next().unwrap_or(path).to_owned()
        })
        .collect()
}

#[test]
fn every_models_path_on_the_page_resolves() {
    let imported = unique(used_paths("models paths"), "the models block");
    assert!(!imported.is_empty(), "the models block holds the imports");

    let page = page();
    let listed = unique(
        table_column(&page, EMBEDDING_HEADING, 0)
            .iter()
            .flat_map(|cell| cell.split(','))
            .map(|path| path.trim().trim_matches('`').to_owned())
            .filter(|path| path.starts_with("loom_sdk::models::"))
            .collect(),
        "the embedding table",
    );
    assert_eq!(
        listed, imported,
        "the page's embedding table lists exactly the `loom_sdk::models` paths that resolve here"
    );
    for path in [
        "loom_sdk::models::from_config",
        "loom_sdk::models::from_routes",
        "loom_sdk::models::parse_config",
        "loom_sdk::models::ModelsConfig",
    ] {
        assert!(listed.contains(path), "the page lists {path}");
    }
}

#[test]
fn every_connectors_path_on_the_page_resolves() {
    let imported = unique(used_paths("connectors paths"), "the connectors block");
    assert!(
        !imported.is_empty(),
        "the connectors block holds the imports"
    );

    let page = page();
    let listed = unique(
        table_column(&page, EMBEDDING_HEADING, 0)
            .iter()
            .flat_map(|cell| cell.split(','))
            .map(|path| path.trim().trim_matches('`').to_owned())
            .filter(|path| path.starts_with("loom_sdk::connectors::"))
            .collect(),
        "the embedding table",
    );
    assert_eq!(
        listed, imported,
        "the page's embedding table lists exactly the `loom_sdk::connectors` paths that resolve here"
    );
    for path in [
        "loom_sdk::connectors::cli::ConnectorsCli",
        "loom_sdk::connectors::cli::idempotency_key",
        "loom_sdk::connectors::evidence::WriteEvidenceAdapter",
    ] {
        assert!(listed.contains(path), "the page lists {path}");
    }
}

/// A host reads the `canon-decision/1` document behind a frontier the governor issued, by its id,
/// through `loom_sdk::CanonGovernor` alone; the page's `loom_sdk::CanonGovernor` row covers the
/// method.
#[test]
fn a_host_reads_the_decision_document_through_loom_sdk() {
    use loom_sdk::commission::model::primitives::Uuid;
    use loom_sdk::commission::model::responsibility::{CaseId, FrontierId, GovernorError};

    let governor = loom_sdk::CanonGovernor::new(loom_sdk::MemoryCaseStore::default());
    let frontier = FrontierId(Uuid("00000000-0000-4000-8000-000000000001".into()));
    let document: Result<Option<loom_sdk::governor::model::json::Value>, GovernorError> =
        loom_sdk::CanonGovernor::decision_document(&governor, &CaseId("case-1".into()), &frontier);
    assert_eq!(
        document,
        Err(GovernorError::UnknownCase),
        "a case the governor does not hold has no document"
    );
}
