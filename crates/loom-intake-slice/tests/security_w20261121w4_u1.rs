//! Security review, wave 2026-11-21-w4, unit U1 (`story:confinement-opens-from-engine-config`):
//! conformance checks of `EngineRunner` against the invariants of the review brief.
//!
//! Every child a case starts is `/bin/sh`, `sleep`, `true` or this test binary, with no network.
//! A case that needs `SUBSTRATE_CONFINEMENT_LOCAL` sets it on a child process of this binary only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use b10x_loom_intake_slice::confinement::{
    BaselineEnvironment, ChildCommand, ChildError, ChildSignal, ConfinementAdapter,
    ConfinementRefusal, EngineConfinement, EngineRunner, ExecEnvironment, TestRunner as _,
};
use b10x_loom_intake_slice::context::ContextPolicy;
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_intake_slice::run::{RunOptions, SliceRequest, run_with_options};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason as TurnStop, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::{Value, json};

const REASON: &str = "security review runs host processes on purpose";

fn local(reason: Option<&str>) -> EngineConfinement {
    EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: reason.map(str::to_owned),
        cgroup_root: None,
    }
}

fn runner_with(reason: &str) -> EngineRunner {
    EngineRunner::open(&local(Some(reason))).expect("local opens with a reason")
}

struct Fixture {
    _directory: tempfile::TempDir,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        Self {
            _directory: directory,
            workspace,
            outside,
        }
    }
}

fn shell(script: &str, workspace: &Path, args: &[&str]) -> ChildCommand {
    let mut words = vec!["-c".to_owned(), script.to_owned(), "sh".to_owned()];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    ChildCommand::new("/bin/sh", words, workspace).with_environment(ExecEnvironment {
        allow: vec![BaselineEnvironment::Path],
        set: BTreeMap::new(),
    })
}

/// Names `export -p` printed, as dash (`export NAME=…`) or bash (`declare -x NAME=…`) prints them.
fn exported_names(printed: &str) -> BTreeSet<String> {
    printed
        .lines()
        .filter_map(|line| {
            line.strip_prefix("export ")
                .or_else(|| line.strip_prefix("declare -x "))
        })
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        })
        .collect()
}

/// What a shell adds to its own exported set without being given it.
const SHELL_MADE: [&str; 4] = ["PWD", "OLDPWD", "SHLVL", "_"];

/// Runs this binary again for the case `name` alone, with `env` set on that process only.
fn rerun(name: &str, env: &[(&str, &str)]) -> (bool, String) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", name, "--nocapture", "--test-threads=1"]);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().expect("the case starts its own binary");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let ran = printed
        .lines()
        .any(|line| line == format!("test {name} ... ok"))
        && printed.contains("test result: ok. 1 passed;");
    (output.status.success() && ran, printed)
}

// ---------------------------------------------------------------------------------------------
// Invariant 1 and 2: nothing reaches `local` but the configuration naming it with a reason.

#[test]
fn security_w20261121w4_u1_a_reason_beside_linux_is_refused_and_never_opens_local() {
    let error = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Linux,
        local_reason: Some(REASON.to_owned()),
        cgroup_root: None,
    })
    .expect_err("linux with a reason is refused");
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");

    let error = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(REASON.to_owned()),
        cgroup_root: Some("/sys/fs/cgroup/loom-review".to_owned()),
    })
    .expect_err("local with a cgroup root is refused");
    assert_eq!(error.reason, ConfinementRefusal::ScopeInvalid, "{error}");

    let linux = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Linux,
        local_reason: None,
        cgroup_root: None,
    })
    .expect("linux builds and opens nothing");
    assert_eq!(linux.label(), "substrate");
    assert_eq!(linux.backend(), intake_backend_substrate());
}

fn intake_backend_substrate() -> b10x_loom_intake_slice::confinement::Backend {
    b10x_loom_intake_slice::confinement::Backend::Substrate
}

#[test]
fn security_w20261121w4_u1_reasons_without_a_word_are_missing() {
    for reason in [
        "\0", "\n", "\r\n", "\u{200B}", "\u{2028}", "\u{FEFF}", "---", "\u{3164}", "\t\0 ",
    ] {
        let error = EngineRunner::open(&local(Some(reason)))
            .expect_err("a reason with no word opens nothing");
        assert_eq!(
            error.reason,
            ConfinementRefusal::LocalReasonMissing,
            "{reason:?}: {error}"
        );
    }
}

const POLICY_CASE: &str = "LOOM_SECURITY_W20261121W4_U1_POLICY";

#[test]
fn security_w20261121w4_u1_only_an_unset_or_exact_permit_policy_opens_local() {
    const NAME: &str = "security_w20261121w4_u1_only_an_unset_or_exact_permit_policy_opens_local";
    if let Some(expect) = std::env::var_os(POLICY_CASE) {
        let opened = EngineRunner::open(&local(Some(REASON)));
        if expect == "opens" {
            opened.expect("the host's policy permits local");
        } else {
            let error = opened.expect_err("the host's policy forbids local");
            assert_eq!(
                error.reason,
                ConfinementRefusal::CapabilityUnserved,
                "{error}"
            );
            assert!(
                error.detail.contains("SUBSTRATE_CONFINEMENT_LOCAL"),
                "{error}"
            );
            assert!(
                error.detail.contains("confinement.local-refused"),
                "{error}"
            );
        }
        return;
    }
    for (value, expect) in [
        ("", "refused"),
        ("PERMIT", "refused"),
        ("permit ", "refused"),
        ("permit\n", "refused"),
        ("allow", "refused"),
        ("permit", "opens"),
    ] {
        let (ok, printed) = rerun(
            NAME,
            &[
                (POLICY_CASE, expect),
                ("SUBSTRATE_CONFINEMENT_LOCAL", value),
            ],
        );
        assert!(ok, "policy {value:?} must be {expect}:\n{printed}");
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: the reason cannot forge a second line of the run's output.

#[test]
fn security_w20261121w4_u1_the_label_escapes_control_characters_of_the_reason() {
    let runner = runner_with("x\nconfinement: substrate\r\u{1b}[2K\u{85}\0");
    let label = runner.label();
    assert!(label.starts_with("local (x"), "{label:?}");
    assert!(
        !label.chars().any(char::is_control),
        "no control character reaches the label: {label:?}"
    );
    assert_eq!(label.lines().count(), 1, "{label:?}");
}

#[test]
fn security_w20261121w4_u1_the_label_breaks_no_line_on_a_unicode_separator() {
    let runner = runner_with("x\u{2028}confinement: substrate\u{2029}");
    let label = runner.label();
    assert!(
        !label.contains(['\u{2028}', '\u{2029}']),
        "a Unicode line or paragraph separator in the reason reaches the confinement line \
         unescaped, where a reader splitting on Unicode line breaks sees a second line: {label:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 5: what reaches a `local` child of Loom's own environment.

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_a_local_child_gets_none_of_looms_environment() {
    let fixture = Fixture::new();
    let runner = runner_with(REASON);
    let mut child = shell("export -p", &fixture.workspace, &[]);
    child = child.with_environment(ExecEnvironment {
        allow: vec![BaselineEnvironment::Path],
        set: BTreeMap::from([("REVIEW_SET".to_owned(), "1".to_owned())]),
    });
    let result = runner.run_child(&child).expect("export -p runs");
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    let printed = String::from_utf8_lossy(&result.child_output.stdout).into_owned();
    let names = exported_names(&printed);
    let allowed: BTreeSet<String> = ["PATH", "REVIEW_SET"]
        .into_iter()
        .chain(SHELL_MADE)
        .map(str::to_owned)
        .collect();
    let extra: Vec<&String> = names.difference(&allowed).collect();
    assert!(extra.is_empty(), "run_child passed {extra:?}:\n{printed}");
    assert!(
        printed.contains("PATH='/usr/bin:/bin'") || printed.contains("PATH=\"/usr/bin:/bin\""),
        "the baseline PATH is Substrate's, not Loom's:\n{printed}"
    );

    let command = TestCommand::new("/bin/sh", ["-c", "export -p"]);
    let result = runner
        .run(&command, &fixture.workspace)
        .expect("the test runs");
    assert_eq!(result.exit_code, Some(0), "{}", result.output);
    let printed = String::from_utf8_lossy(&result.child_output.stdout).into_owned();
    let names = exported_names(&printed);
    let allowed: BTreeSet<String> = [
        "PATH",
        "HOME",
        "RUSTUP_HOME",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "CARGO_NET_OFFLINE",
        "CARGO_BUILD_JOBS",
        "RUSTUP_AUTO_INSTALL",
    ]
    .into_iter()
    .chain(SHELL_MADE)
    .map(str::to_owned)
    .collect();
    let extra: Vec<&String> = names.difference(&allowed).collect();
    assert!(extra.is_empty(), "run passed {extra:?}:\n{printed}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_a_credential_named_variable_is_refused_before_launch() {
    let fixture = Fixture::new();
    let marker = fixture.outside.join("ran");
    let runner = runner_with(REASON);
    for name in [
        "API_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "HTTPS_PROXY",
        "GIT_PASSWORD",
    ] {
        let child = shell(
            r#"printf x > "$1""#,
            &fixture.workspace,
            &[marker.to_str().unwrap()],
        )
        .with_environment(ExecEnvironment {
            allow: vec![BaselineEnvironment::Path],
            set: BTreeMap::from([(name.to_owned(), "value".to_owned())]),
        });
        assert!(runner.run_child(&child).is_err(), "run_child with {name}");
        assert!(
            runner.spawn_child(&child).is_err(),
            "spawn_child with {name}"
        );
        assert!(!marker.exists(), "no child started with {name}");
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 6: the deadline and a host signal end the whole group.

fn group_is_gone(group: i32) -> bool {
    matches!(
        nix::sys::signal::killpg(nix::unistd::Pid::from_raw(group), None),
        Err(nix::errno::Errno::ESRCH)
    )
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_a_live_group_ignoring_term_is_killed_after_the_grace() {
    let fixture = Fixture::new();
    let child = shell(
        "trap '' TERM; echo $$; sleep 30 & sleep 30",
        &fixture.workspace,
        &[],
    )
    .with_timeout(Duration::from_secs(60));
    let mut live = runner_with(REASON)
        .spawn_child(&child)
        .expect("starts live");
    let mut read = Vec::new();
    while !read.contains(&b'\n') {
        match live.read(Duration::from_secs(5)) {
            Ok(Some(frame)) => read.extend(frame.bytes),
            Ok(None) => break,
            Err(ChildError::ReadTimedOut) => panic!("no group id within 5 s"),
            Err(error) => panic!("the read failed: {error}"),
        }
    }
    let group: i32 = String::from_utf8_lossy(&read)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .expect("the group id");
    let started = Instant::now();
    live.signal(ChildSignal::Terminate)
        .expect("the signal is sent");
    let result = live.wait().expect("a group the host killed is a result");
    assert!(
        started.elapsed() < Duration::from_secs(9),
        "ended {:?} after the signal",
        started.elapsed()
    );
    assert_eq!(result.exit.exit_code, None, "{:?}", result.exit);
    assert!(result.exit.signal.is_some(), "{:?}", result.exit);
    assert!(group_is_gone(group), "a member of group {group} survived");
    assert_eq!(result.applied.adapter.as_deref(), Some("local"));
}

const ESCAPE_CASE: &str = "LOOM_SECURITY_W20261121W4_U1_ESCAPE";
const ESCAPE_PIDFILE: &str = "LOOM_SECURITY_W20261121W4_U1_PIDFILE";

/// A grandchild that leaves the run's process group (`setsid`) and keeps its output open: what
/// Loom's deadline kill under `local` reaches, and what it reports.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_a_process_that_left_the_group_is_refused_not_hidden() {
    const NAME: &str =
        "security_w20261121w4_u1_a_process_that_left_the_group_is_refused_not_hidden";
    if std::env::var_os(ESCAPE_CASE).is_some() {
        // SAFETY: setsid takes no memory; this process is a background member of the run's
        // group, never its leader, so it may start a session of its own.
        assert!(unsafe { libc::setsid() } > 0, "setsid");
        let pidfile = std::env::var(ESCAPE_PIDFILE).unwrap();
        std::fs::write(pidfile, std::process::id().to_string()).unwrap();
        std::thread::sleep(Duration::from_secs(25));
        return;
    }
    let fixture = Fixture::new();
    let pidfile = fixture.outside.join("escaped.pid");
    let exe = std::env::current_exe().unwrap();
    let script = format!("\"$1\" --exact {NAME} --nocapture --test-threads=1 & sleep 30");
    let child = ChildCommand::new(
        "/bin/sh",
        vec![
            "-c".to_owned(),
            script,
            "sh".to_owned(),
            exe.to_str().unwrap().to_owned(),
        ],
        &fixture.workspace,
    )
    .with_environment(ExecEnvironment {
        allow: vec![BaselineEnvironment::Path],
        set: BTreeMap::from([
            (ESCAPE_CASE.to_owned(), "1".to_owned()),
            (
                ESCAPE_PIDFILE.to_owned(),
                pidfile.to_str().unwrap().to_owned(),
            ),
        ]),
    })
    .with_timeout(Duration::from_secs(2));
    let started = Instant::now();
    let outcome = runner_with(REASON).run_child(&child);
    let elapsed = started.elapsed();
    let escaped: Option<i32> = std::fs::read_to_string(&pidfile)
        .ok()
        .and_then(|text| text.trim().parse().ok());
    let alive = escaped.is_some_and(|pid| unsafe { libc::kill(pid, 0) } == 0);
    if let Some(pid) = escaped {
        // SAFETY: SIGKILL to the pid the escaped helper wrote; no memory is involved.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    println!("escaped={escaped:?} alive={alive} elapsed={elapsed:?} outcome={outcome:?}");
    let error = outcome.expect_err("a run a process outside its group holds is refused");
    assert_eq!(
        error.reason,
        ConfinementRefusal::CapabilityUnserved,
        "{error}"
    );
    assert!(error.detail.contains("left the group"), "{error}");
    assert!(escaped.is_some(), "the helper started and left the group");
    assert!(
        alive,
        "the process that left the group is not reached by the group kill"
    );
    assert!(
        elapsed < Duration::from_secs(20),
        "the refusal comes within the kill settle bounds: {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 7: a `local` run is not kept for resume, by the library either.

#[test]
fn security_w20261121w4_u1_the_library_refuses_to_keep_a_local_run_for_resume() {
    let fixture = Fixture::new();
    let request = b10x_loom_intake_slice::intent::IntentRequest {
        intent: "Run the tests.".into(),
        workspace: Some(fixture.workspace.clone()),
        runner: Some(Arc::new(runner_with("x"))),
        test: TestCommand::new("true", Vec::<String>::new()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
        bindings: None,
    };
    // `ResumeHost::check` is what a host calls before a stateful run keeps its resume context
    // (`run_prepared_intent_with_state` keeps one before the run, from `host.confinement`).
    for confinement in [
        b10x_loom_intake_slice::confinement::Backend::None,
        b10x_loom_intake_slice::confinement::Backend::Substrate,
    ] {
        let host = b10x_loom_intake_slice::intent::ResumeHost {
            confinement,
            cgroup_root: None,
            model: "recorded".to_owned(),
            catalog: None,
            config: None,
        };
        assert!(
            host.check(&request).is_err(),
            "a resume context of a local run would be kept as {confinement:?}, and a resume \
             would reopen it under another adapter with no reason"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: every `confinement:` line a run prints names `local`.

struct Constant {
    provenance: Provenance,
    capabilities: Capabilities,
    arguments: Value,
}

impl Constant {
    fn new(arguments: Value) -> Self {
        let id = |text: &str| Id::new(text).unwrap();
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("recorded"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8192)
            },
            arguments,
        }
    }
}

impl Model for Constant {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _: &'a mut dyn StreamSink,
        _: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!("expected a forced tool")
            };
            Ok(TurnOutcome {
                stop_reason: TurnStop::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new("call-1").unwrap(),
                    name: tool.clone(),
                    arguments: self.arguments.clone(),
                })],
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_the_slice_run_line_names_local_and_its_reason() {
    let fixture = Fixture::new();
    let request = SliceRequest {
        runner: Arc::new(runner_with("x")),
        intent: "Run the tests.".into(),
        workspace: fixture.workspace.clone(),
        test: TestCommand::new("true", Vec::<String>::new()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
    };
    // The classifier is unsure, so the run stops right after its confinement line and no test runs.
    let classifier = Constant::new(
        json!({"protocol":"software-change@1", "confidence":0.1, "reasons":["unsure"]}),
    );
    let agent = Constant::new(json!({}));
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut out = Vec::new();
    let run = run_with_options(
        &request,
        &governor,
        &governor,
        &classifier,
        &agent,
        &mut out,
        &RunOptions {
            context_policy: ContextPolicy::Legacy,
            context_report: None,
        },
    );
    let printed = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = printed
        .lines()
        .filter(|line| line.starts_with("confinement:"))
        .collect();
    assert_eq!(
        lines,
        ["confinement: local (x)"],
        "a local runner's run line ({run:?}):\n{printed}"
    );
}
