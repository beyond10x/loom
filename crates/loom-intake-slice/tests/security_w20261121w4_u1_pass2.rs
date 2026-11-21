//! Security review pass 2, wave 2026-11-21-w4, unit U1 (`story:confinement-opens-from-engine-config`):
//! conformance checks of the correction at 37de3e91 (F2's refusal for resume, F3's escape).
//!
//! The one child a case starts is `true`, with no network.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use b10x_loom_intake_slice::confinement::{
    Backend, ConfinementAdapter, ConfinementError, EngineConfinement, EngineRunner, TestExecution,
    TestRunner, escape_reason, local_label,
};
use b10x_loom_intake_slice::executor::TestCommand;
use b10x_loom_intake_slice::intent::{IntentRequest, ResumeHost};
use b10x_loom_intake_slice::run::SliceError;

fn local_runner(reason: &str) -> EngineRunner {
    EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(reason.to_owned()),
        cgroup_root: None,
    })
    .expect("local opens with a reason")
}

fn request(runner: Arc<dyn TestRunner>, workspace: &Path) -> IntentRequest {
    IntentRequest {
        intent: "Run the tests.".into(),
        workspace: Some(workspace.to_owned()),
        runner: Some(runner),
        test: TestCommand::new("true", Vec::<String>::new()),
        max_steps: 1,
        threshold: 0.5,
        inputs: b10x_loom_intake_slice::case::RUN_DEFAULTS,
        test_paths: b10x_loom_intake_slice::test_paths::TestPaths::default(),
        bindings: None,
    }
}

fn host() -> ResumeHost {
    ResumeHost {
        confinement: Backend::None,
        cgroup_root: None,
        model: "recorded".to_owned(),
        catalog: None,
        config: None,
    }
}

/// A host's runner that wraps an `EngineRunner` and forwards what `TestRunner` asks of a runner
/// (`run`, `backend`, `label`, and `local_reason`, as its documentation asks of a wrapper).
struct Wrapped(EngineRunner);

impl TestRunner for Wrapped {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.0.run(command, root)
    }
    fn backend(&self) -> Backend {
        self.0.backend()
    }
    fn label(&self) -> String {
        self.0.label()
    }
    fn local_reason(&self) -> Option<&str> {
        self.0.local_reason()
    }
}

// A wrapper that does not forward `local_reason` is the documented duty of the host
// (`review-result:confinement-opens-from-engine-config-security-2`, finding A, declined by the coordinator).
#[test]
fn security_w20261121w4_u1_pass2_a_wrapped_local_runner_is_not_kept_for_resume() {
    let directory = tempfile::tempdir().unwrap();
    let workspace: PathBuf = directory.path().canonicalize().unwrap();
    let direct = local_runner("x");
    assert!(
        host()
            .check(&request(Arc::new(direct.clone()), &workspace))
            .is_err(),
        "the correction refuses an EngineRunner under local"
    );
    let wrapped = Wrapped(direct);
    assert_eq!(wrapped.label(), "local (x)", "the wrapper names local");
    let refused = host()
        .check(&request(Arc::new(wrapped), &workspace))
        .expect_err("a wrapper that forwards local_reason is refused by ResumeHost::check");
    assert!(
        matches!(&refused, SliceError::State(why) if why.contains("a local run is not kept for resume")),
        "the refusal names why and keeps nothing: {refused:?}"
    );
}

/// Every character the pass-2 brief names as breaking a line or reordering text.
const BREAKS: [char; 25] = [
    '\u{0B}', '\u{0C}', '\u{1C}', '\u{1D}', '\u{1E}', '\u{85}', '\u{2028}', '\u{2029}', '\u{202A}',
    '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    '\u{061C}', '\u{200E}', '\u{200F}', '\n', '\r', '\t', '\u{1b}', '\0',
];

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w20261121w4_u1_pass2_every_line_break_and_bidi_control_is_escaped_in_label_and_record()
{
    let reason: String = std::iter::once('x').chain(BREAKS).chain(['y']).collect();
    let escaped = escape_reason(&reason);
    for character in BREAKS {
        assert!(
            !escaped.contains(character),
            "{:?} reaches the escaped reason: {escaped:?}",
            character
        );
    }
    let runner = local_runner(&reason);
    assert_eq!(runner.label(), local_label(&reason));
    assert_eq!(runner.label(), format!("local ({escaped})"));
    let directory = tempfile::tempdir().unwrap();
    let ran = runner
        .run(
            &TestCommand::new("true", Vec::<String>::new()),
            &directory.path().canonicalize().unwrap(),
        )
        .expect("true runs");
    assert_eq!(ran.applied.local_reason.as_deref(), Some(escaped.as_str()));
}

#[test]
fn security_w20261121w4_u1_pass2_invisible_characters_that_hide_text_are_escaped() {
    // Zero width space, joiner, word joiner, byte order mark, and a tag character (U+E0041,
    // TAG LATIN CAPITAL LETTER A), which renders as nothing and carries ASCII a model reads.
    let hidden = ['\u{200B}', '\u{200D}', '\u{2060}', '\u{FEFF}', '\u{E0041}'];
    let reason: String = std::iter::once('x').chain(hidden).collect();
    let escaped = escape_reason(&reason);
    let shown: Vec<char> = hidden
        .into_iter()
        .filter(|character| escaped.contains(*character))
        .collect();
    assert!(
        shown.is_empty(),
        "invisible characters reach the label, the record and the model's briefing unescaped: \
         {shown:?} in {escaped:?}"
    );
}

#[test]
fn security_w20261121w4_u1_pass2_the_recorded_reason_tells_what_was_given() {
    // The record keeps only the escaped reason. Two different reasons the operator can give, one
    // with a real newline and one with the six characters `\u{a}` typed, must not be recorded as
    // the same text, or a reader of the record cannot tell which reason was given.
    let typed = escape_reason("a\\u{a}b");
    let newline = escape_reason("a\nb");
    assert_ne!(
        typed, newline,
        "the escape is not injective: a backslash is left as it is, so the record of a typed \
         `\\u{{a}}` equals the record of a newline"
    );
}
