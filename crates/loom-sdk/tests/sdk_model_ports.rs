//! A host runs a governed loop on the SDK's `claude` command port
//! (`loom_sdk::loom::wire::ClaudeCodePort`) without writing a model port of its own.
//!
//! The `claude` here is a stand-in: a shell script this test writes under `CARGO_TARGET_TMPDIR` and
//! puts first on `PATH`. It records its arguments and its standard input and prints
//! `tests/fixtures/claude-code/result.json`, a result in the shape `claude -p --output-format json`
//! prints (its ids are synthetic). No model, network or credential is reached. Every path below
//! is a `loom_sdk` path.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use loom_sdk::commission::model::behaviour::Generated;
use loom_sdk::commission::model::json::Value as CommissionValue;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, CompletionDetermination, EffectOutcome,
    EffectOutcomePerformed, Frontier, FrontierAction, FrontierData, FrontierId, FrontierObligation,
    GovernorError, Observation, ObservationId, PrincipalId, RunId, Unit, commission_state,
    frontier_state, observation_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use loom_sdk::commission::ports::evidence::ObservationPort;
use loom_sdk::commission::ports::governor::Governor;
use loom_sdk::commission::runtime::{LoopContext, run_until_blocked};
use loom_sdk::loom::harness::governed::LoopExecutor;
use loom_sdk::loom::harness::turn_loop::LoopConfig;
use loom_sdk::loom::harness::wire::ModelPort;
use loom_sdk::loom::model::primitives::Uuid;
use loom_sdk::loom::model::run::{CommissionRunId, SessionData, SessionId};
use loom_sdk::loom::wire::ClaudeCodePort;
use loom_sdk::loom::{EmptyObjectArguments, FirstAdmissibleSelector};
use loom_sdk::{Loom, MeteredExecutor, MeteredModelPort, RunMeter};

const CASE: &str = "CASE-SDK-MODEL-PORTS";
const EDIT: &str = "repository.edit";
/// The model the port is built for, which the stand-in is asked to run.
const MODEL: &str = "claude-sonnet-4-5";

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

/// The case: open at revision 1, its frontier listing the admissible edit.
struct Case;

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(1)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf1)),
            case_id: case.clone(),
            case_revision: 1,
            claims: Vec::new(),
            obligations: vec![FrontierObligation {
                obligation: "tests-pass".to_owned(),
                open: true,
            }],
            actions: vec![FrontierAction {
                action: EDIT.to_owned(),
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

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Grants nothing: the edit needs no capability.
struct NoAuthority;

impl AuthorityProvider for NoAuthority {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Err(AuthorityProviderError {
            message: format!("no capability is granted here, `{capability}` included"),
        })
    }
}

/// Performs every action and changes nothing.
struct Effects;

impl EffectPort for Effects {
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

/// Fresh ids, a fixed trusted time and a budget of one step.
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-16T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// A file of this crate, read at run time.
fn crate_file(relative: &str) -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    PathBuf::from(manifest).join(relative)
}

/// Writes the stand-in `claude` into `bin`, recording into `dir` and printing `fixture`.
fn stand_in(bin: &Path, dir: &Path, fixture: &Path) {
    let script = format!(
        "#!/bin/sh\n\
         printf '%s\\n' \"$@\" > '{argv}'\n\
         cat > '{stdin}'\n\
         cat '{fixture}'\n",
        argv = dir.join("argv").display(),
        stdin = dir.join("stdin").display(),
        fixture = fixture.display(),
    );
    let program = bin.join("claude");
    fs::write(&program, script).expect("the stand-in is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755))
            .expect("the stand-in is executable");
    }
}

/// A fresh scratch directory under `CARGO_TARGET_TMPDIR`.
fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loom-sdk-claude-{}-{nanos}", std::process::id()));
    fs::create_dir_all(dir.join("bin")).expect("the scratch directory is made");
    dir
}

#[test]
fn a_host_runs_a_governed_loop_on_the_sdk_command_port() {
    let dir = scratch();
    let fixture = crate_file("tests/fixtures/claude-code/result.json");
    stand_in(&dir.join("bin"), &dir, &fixture);
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut search = vec![dir.join("bin")];
    search.extend(std::env::split_paths(&path));
    // SAFETY: this test is the only one in its binary, so no other thread reads the environment
    // while it is written.
    unsafe {
        std::env::set_var(
            "PATH",
            std::env::join_paths(search).expect("a joinable PATH"),
        );
    }

    let recorded: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&fixture).expect("the fixture is read"))
            .expect("the fixture is JSON");
    let usage = &recorded["usage"];
    let figure = |name: &str| {
        i64::try_from(usage[name].as_u64().expect("a recorded figure")).expect("a small figure")
    };
    // The command reports its input figures disjointly; a turn's input is the whole of them.
    let input = figure("input_tokens")
        + figure("cache_read_input_tokens")
        + figure("cache_creation_input_tokens");

    let port = ClaudeCodePort::new(MODEL, &dir);
    let wire = port.wire().as_str().to_owned();
    let meter = RunMeter::new(None);
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "Make the change.",
    )
    .with_governor(&Case);
    let looped = LoopExecutor::new(
        &loom,
        MeteredModelPort::new(port, &meter),
        LoopConfig::new(MODEL, "Make the change the case asks for."),
        SessionData {
            session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000e001".to_owned())),
            commission_run: CommissionRunId(Uuid(
                "00000000-0000-4000-8000-00000000e0ab".to_owned(),
            )),
            wire,
            boundary_refusals: 0,
        },
    );
    let executor = MeteredExecutor::new(looped, &meter);
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));

    let end = run_until_blocked(
        &Case,
        &executor,
        &NoAuthority,
        &Effects,
        &commission(),
        &mut runs,
        &mut Context { ids: 0 },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let report = meter.report(&end.run_id);

    let argv = fs::read_to_string(dir.join("argv")).expect("the stand-in ran");
    let argv: Vec<&str> = argv.lines().collect();
    assert!(argv.contains(&"-p"), "one print-mode invocation: {argv:?}");
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--output-format", "json"]),
        "the JSON result is asked for: {argv:?}"
    );
    assert!(
        argv.windows(2).any(|pair| pair == ["--model", MODEL]),
        "the port's model is asked for: {argv:?}"
    );
    let stdin = fs::read_to_string(dir.join("stdin")).expect("the stand-in read its input");
    assert!(
        stdin.contains("repository_edit"),
        "the turn's catalogue reaches the command: {stdin}"
    );

    assert_eq!(
        report.model_calls, 1,
        "one turn, one invocation: {report:?}"
    );
    assert_eq!(report.input_tokens, input, "{report:?}");
    assert_eq!(
        report.cached_input_tokens,
        figure("cache_read_input_tokens"),
        "{report:?}"
    );
    assert_eq!(report.output_tokens, figure("output_tokens"), "{report:?}");
    assert!(
        end.admitted
            .iter()
            .any(|request| request.action.as_str() == EDIT),
        "the turn's call of the edit was proposed and admitted: {end:#?}"
    );
}
