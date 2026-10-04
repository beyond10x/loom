// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1 on `story:session-transcript-streaming`: `crates/loom/src/session.rs` driven
//! against the unit's own specification (`ess/domains/run.yaml`, `loom.run.OpenSession`,
//! `loom.run.ResumeSession`, `loom.run.RunEnding`), its acceptance statement, and Harness
//! `transcript.rs` / `open_session` at `798325f0`.
//!
//! Every model here is a scripted [`ModelPort`] in this process: no socket, no frontier tool.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use b10x_loom::harness::turn_loop::{ApproveAll, LoopConfig, LoopEvent, LoopSink};
use b10x_loom::harness::wire::{
    Item, ModelPort, StopReason, StreamEvent, StreamSink, ToolCall, ToolOutcome, ToolPort,
    ToolSpec, TurnOutcome, TurnRequest, WireError, WireId,
};
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{CommissionRunId, RunEnding, SessionData, SessionId};
use b10x_loom::session::{RunPorts, SessionError, SessionFile, outside_workspace, run_and_file};
use serde_json::json;

const RESPONSES: &str = b10x_loom::harness::responses::WIRE;
const MESSAGES: &str = b10x_loom::harness::messages::WIRE;
const RUN: &str = "00000000-0000-4000-8000-0000000adaaa";

// --- the unit's own contract: loom.run.ResumeSession and loom.run.OpenSession -------------------

/// `loom.run.ResumeSession` moves `Filed -> Active` and answers `wrong-state` from any other state
/// (`ess/domains/run.yaml`). Two runs resuming one filed session is the concurrent-writer case: the
/// second resume must be refused, or the second filing must be, or the filed session must hold
/// both runs' turns. `SessionFile::resume` records nothing on disk, so both resumes succeed and the
/// later filing silently drops the earlier run's turns.
#[test]
fn a_session_resumed_twice_does_not_lose_the_first_runs_turns() {
    let root = scratch("resumed_twice");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad001");
    let mut opened = open(&id, RESPONSES, &workspace);
    opened.items.push(Item::user("FILED-TURN"));
    opened
        .file(&sessions, RunEnding::Answered)
        .expect("the first run files");

    let wire = WireId::new(RESPONSES).expect("a wire");
    let mut first =
        SessionFile::resume(&sessions, &id, &wire, &workspace).expect("the first resume");
    let second = SessionFile::resume(&sessions, &id, &wire, &workspace);

    first.items.push(Item::user("FIRST-RESUMER-TURN"));
    first
        .file(&sessions, RunEnding::Answered)
        .expect("the first resumer files");
    let Ok(mut second) = second else {
        return; // `wrong-state`: refused while the session is Active. The specified outcome.
    };
    second.items.push(Item::user("SECOND-RESUMER-TURN"));
    let Ok(_) = second.file(&sessions, RunEnding::Answered) else {
        return; // refused at filing: the lost update did not happen.
    };
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert!(
        stored.items.contains(&Item::user("FIRST-RESUMER-TURN")),
        "a second resume of an Active session was not refused (loom.run.ResumeSession \
         `wrong-state`), and its filing replaced the first resumer's turns: stored {:?}",
        stored.items
    );
}

/// `loom.run.OpenSession` answers `session-exists` for an identity already carried. Opening an id
/// that is already filed and filing it must not replace the filed conversation.
#[test]
fn opening_an_already_filed_session_id_does_not_replace_it() {
    let root = scratch("open_existing");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad002");
    let mut original = open(&id, RESPONSES, &workspace);
    original.items.push(Item::user("ORIGINAL-CONVERSATION"));
    original
        .file(&sessions, RunEnding::Answered)
        .expect("the original files");

    let mut again = open(&id, RESPONSES, &workspace);
    let refiled = again.file(&sessions, RunEnding::Answered);
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert!(
        refiled.is_err() || stored.items.contains(&Item::user("ORIGINAL-CONVERSATION")),
        "a second OpenSession of a filed id was not refused (loom.run.OpenSession \
         `session-exists`) and its filing replaced the original conversation: stored {:?}",
        stored.items
    );
}

/// A resume continues *the same* session by id (story § Domain relations). A file whose name says
/// one session and whose content says another is not that session.
#[test]
fn a_session_file_whose_id_is_not_its_name_is_refused() {
    let root = scratch("mismatched_id");
    let (workspace, sessions) = layout(&root);
    let real = id("00000000-0000-4000-8000-0000000ad003");
    let other = id("00000000-0000-4000-8000-0000000ad004");
    let mut session = open(&real, RESPONSES, &workspace);
    let filed = session.file(&sessions, RunEnding::Answered).expect("files");
    std::fs::copy(&filed, sessions.join(format!("{}.json", other.0.0))).expect("copy");

    let loaded = SessionFile::load(&sessions, &other);
    assert!(
        loaded.is_err(),
        "`{}.json` holds session `{}` and was loaded as if it were `{}`",
        other.0.0,
        loaded.map(|session| session.id).unwrap_or_default(),
        other.0.0
    );
}

// --- run_and_file on every exit path ------------------------------------------------------------

/// "Written whether the run answered or died" (story § Outcome). A run whose loop panics died.
#[test]
fn a_run_whose_loop_panics_still_files_its_session() {
    let root = scratch("panic_files");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad005");
    let mut session = open(&id, RESPONSES, &workspace);
    let mut model = Scripted::panicking(RESPONSES);
    let _ = catch_unwind(AssertUnwindSafe(|| {
        run(&mut model, &mut session, &sessions, "QUESTION")
    }));
    assert!(
        sessions.join(format!("{}.json", id.0.0)).exists(),
        "a run that panicked left no session file in {}",
        sessions.display()
    );
}

/// After a caught panic the caller's session must still hold the conversation it was resumed
/// with: filing it afterwards must not replace the stored turns with nothing.
#[test]
fn a_run_whose_loop_panics_leaves_the_resumed_conversation_in_the_session() {
    let root = scratch("panic_items");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad006");
    let mut session = open(&id, RESPONSES, &workspace);
    session.items.push(Item::user("EARLIER-TURN"));
    let mut model = Scripted::panicking(RESPONSES);
    let _ = catch_unwind(AssertUnwindSafe(|| {
        run(&mut model, &mut session, &sessions, "QUESTION")
    }));
    assert_eq!(
        session.items,
        [Item::user("EARLIER-TURN")],
        "the panic took the session's conversation with it"
    );
}

/// `loom.run.RunEnding`: a run the provider cut short answered nothing, yet the loop returns it as
/// `Ok`. Adapted to coordinator decision 6 (wave 2026-10-04-w15): such a stop is filed `Stopped`
/// (it was `Failed` in adversary pass 1).
#[test]
fn a_run_the_provider_cut_short_is_not_filed_as_answered() {
    let root = scratch("incomplete_ending");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad007");
    let mut session = open(&id, RESPONSES, &workspace);
    let mut model = Scripted::incomplete(RESPONSES);
    let filed = run(&mut model, &mut session, &sessions, "QUESTION");
    let outcome = filed.run.expect("the loop returns the stop as an outcome");
    assert!(
        !outcome.stop.is_completed(),
        "precondition: {:?}",
        outcome.stop
    );
    filed.filed.expect("files");
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert_eq!(
        stored.run_ending(),
        Some(RunEnding::Stopped),
        "a run that stopped {:?} was not filed as Stopped",
        outcome.stop
    );
}

/// A session records the wire its items came from so that a resume on another wire can be refused
/// (story § Outcome). `run_and_file` never compares that recorded wire with the wire the loop's
/// model speaks, so a session labelled `openai-responses` can be filed holding
/// `anthropic-messages` opaque items — and a later resume on `openai-responses` passes the check.
#[test]
fn a_session_is_never_filed_holding_items_of_another_wire_than_it_records() {
    let root = scratch("wire_label");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad008");
    let mut session = open(&id, RESPONSES, &workspace);
    let mut model = Scripted::answering(
        MESSAGES,
        json!({"type": "thinking", "thinking": "T", "signature": "SIG"}),
    );
    let filed = run(&mut model, &mut session, &sessions, "QUESTION");
    if filed.run.is_err() || filed.filed.is_err() {
        return; // refused: the session wire and the loop wire were compared.
    }
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    let foreign: Vec<&Item> = stored
        .items
        .iter()
        .filter(|item| matches!(item, Item::Opaque { wire, .. } if wire.as_str() != stored.wire.as_str()))
        .collect();
    assert_eq!(
        foreign,
        [] as [&Item; 0],
        "session recorded on `{}` was filed holding opaque items of another wire",
        stored.wire.as_str()
    );
}

// --- acceptance 4 against a provider-shaped reasoning item --------------------------------------

/// Acceptance 4, as reworded by coordinator decision 8 (wave 2026-10-04-w15): the filed session
/// stores no streamed reasoning text outside the provider's opaque reasoning item, and stores that
/// item byte for byte. A Responses provider that streams `reasoning_summary_text` puts the same
/// text into the reasoning item's `summary`, so the text is in the file once, inside that item.
/// (Adversary pass 1 asserted the text was absent from the file altogether.)
#[test]
fn reasoning_text_streamed_by_the_provider_is_not_filed() {
    let root = scratch("reasoning_in_item");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad009");
    let mut session = open(&id, RESPONSES, &workspace);
    let streamed = "ADVERSARY-REASONING-SUMMARY";
    let mut model = Scripted::answering(
        RESPONSES,
        json!({"type": "reasoning", "id": "rs_1", "encrypted_content": "BLOB",
               "summary": [{"type": "summary_text", "text": streamed}]}),
    )
    .streaming_reasoning(streamed);
    let mut sink = Collect::default();
    let filed = run_with_sink(&mut model, &mut session, &sessions, "QUESTION", &mut sink);
    filed.run.expect("answers");
    let path = filed.filed.expect("files");
    assert_eq!(
        sink.reasoning,
        [streamed],
        "precondition: the text was streamed"
    );
    let bytes = std::fs::read(&path).expect("reads");
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    let mut outside = stored.clone();
    outside
        .items
        .retain(|item| !matches!(item, Item::Opaque { .. }));
    let outside = serde_json::to_vec(&outside).expect("encodes");
    assert!(
        !contains(&outside, streamed.as_bytes()),
        "acceptance 4: streamed reasoning text `{streamed}` is filed outside the opaque item"
    );
    assert_eq!(
        bytes
            .windows(streamed.len())
            .filter(|window| *window == streamed.as_bytes())
            .count(),
        1,
        "acceptance 4: `{streamed}` is filed other than once, inside the opaque item"
    );
    assert_eq!(
        stored
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Opaque { payload, .. } => Some(payload.clone()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            json!({"type": "reasoning", "id": "rs_1", "encrypted_content": "BLOB",
                "summary": [{"type": "summary_text", "text": streamed}]})
        ],
        "acceptance 4: the opaque item is stored as the provider sent it"
    );
}

// --- outside_workspace --------------------------------------------------------------------------

/// A symlink into the workspace, an in-workspace path spelled with `..`, and a relative path that
/// resolves into the workspace are all refused.
#[cfg(unix)]
#[test]
fn outside_workspace_refuses_symlinks_dotdot_and_relative_paths_into_the_workspace() {
    let root = scratch("outside_workspace_paths");
    let (workspace, _) = layout(&root);
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).expect("create");
    std::fs::create_dir_all(workspace.join("existing")).expect("create");

    std::os::unix::fs::symlink(workspace.join("existing"), outside.join("link")).expect("link");
    assert!(outside_workspace(&outside.join("link"), &workspace).is_err());
    assert!(outside_workspace(&outside.join("link").join("new"), &workspace).is_err());
    assert!(
        outside_workspace(
            &outside.join("..").join("workspace").join("sessions"),
            &workspace
        )
        .is_err()
    );
    // The process's current directory is the crate's manifest directory under `cargo test`.
    let cwd = std::env::current_dir().expect("cwd");
    assert!(outside_workspace(Path::new("adversary-relative-sessions"), &cwd).is_err());
    assert!(!cwd.join("adversary-relative-sessions").exists());
    assert!(outside_workspace(&outside.join("sessions"), &workspace).is_ok());
}

/// "A symlink cannot disguise an in-workspace target" (`outside_workspace` docs). A symlink whose
/// in-workspace target does not exist yet is not followed, because `exists()` is false for it.
#[cfg(unix)]
#[test]
fn outside_workspace_refuses_a_dangling_symlink_into_the_workspace() {
    let root = scratch("outside_workspace_dangling");
    let (workspace, _) = layout(&root);
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).expect("create");
    std::os::unix::fs::symlink(workspace.join("not-yet"), outside.join("link")).expect("link");
    let resolved = outside_workspace(&outside.join("link"), &workspace);
    assert!(
        resolved.is_err(),
        "`{}` points into the workspace and was accepted as {resolved:?}",
        outside.join("link").display()
    );
}

/// A workspace whose name is not UTF-8. Declined by coordinator decision 10 (wave 2026-10-04-w15):
/// ported from Harness, the session cannot be encoded and filing is refused by name rather than
/// silently. Adversary pass 1 asserted it was filed; this asserts the named refusal and that
/// nothing was written.
#[cfg(unix)]
#[test]
fn a_session_over_a_non_utf8_workspace_is_refused_by_name() {
    use std::os::unix::ffi::OsStrExt as _;
    let root = scratch("non_utf8_workspace");
    let workspace = root.join(std::ffi::OsStr::from_bytes(b"workspace-\xff"));
    let sessions = root.join("state").join("sessions");
    std::fs::create_dir_all(&workspace).expect("create");
    let id = id("00000000-0000-4000-8000-0000000ad00a");
    let mut session = open(&id, RESPONSES, &workspace);
    let filed = session.file(&sessions, RunEnding::Answered);
    let Err(SessionError::Refused(message)) = &filed else {
        panic!("not refused by name: {filed:?}");
    };
    assert!(
        message.contains("encoding session") && message.contains(&id.0.0),
        "{message}"
    );
    assert!(
        !sessions.join(format!("{}.json", id.0.0)).exists(),
        "nothing was filed"
    );
}

// --- corrupt files on resume --------------------------------------------------------------------

/// A truncated, empty, or other-version file is refused by name rather than replayed.
#[test]
fn a_corrupt_or_truncated_session_file_is_refused_on_resume() {
    let root = scratch("corrupt");
    let (workspace, sessions) = layout(&root);
    let id = id("00000000-0000-4000-8000-0000000ad00b");
    let mut session = open(&id, RESPONSES, &workspace);
    session.items.push(Item::user("SOMETHING"));
    let path = session.file(&sessions, RunEnding::Answered).expect("files");
    let whole = std::fs::read(&path).expect("reads");
    let wire = WireId::new(RESPONSES).expect("a wire");
    for (name, bytes) in [
        ("truncated", whole[..whole.len() / 2].to_vec()),
        ("empty", Vec::new()),
        // Adapted to coordinator decision 13 (wave 2026-10-04-w15): the format is version 2, so
        // the other version is now 1, Harness's (it was 2 in adversary pass 1).
        (
            "version 1",
            String::from_utf8(whole.clone())
                .expect("utf-8")
                .replacen("\"version\": 2", "\"version\": 1", 1)
                .into_bytes(),
        ),
    ] {
        std::fs::write(&path, &bytes).expect("write");
        let error = SessionFile::resume(&sessions, &id, &wire, &workspace).expect_err(name);
        assert!(
            error.to_string().contains(&*path.to_string_lossy()),
            "{name}: {error}"
        );
    }
}

// --- the scripted model -------------------------------------------------------------------------

enum Script {
    Panic,
    Incomplete,
    Answer(serde_json::Value),
}

struct Scripted {
    wire: WireId,
    script: Script,
    reasoning: Option<&'static str>,
}

impl Scripted {
    fn panicking(wire: &str) -> Self {
        Self::with(wire, Script::Panic)
    }

    fn incomplete(wire: &str) -> Self {
        Self::with(wire, Script::Incomplete)
    }

    fn answering(wire: &str, opaque: serde_json::Value) -> Self {
        Self::with(wire, Script::Answer(opaque))
    }

    fn with(wire: &str, script: Script) -> Self {
        Self {
            wire: WireId::new(wire).expect("a wire"),
            script,
            reasoning: None,
        }
    }

    fn streaming_reasoning(mut self, text: &'static str) -> Self {
        self.reasoning = Some(text);
        self
    }
}

impl ModelPort for Scripted {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        match &self.script {
            Script::Panic => panic!("a port, a tool or a sink panicked mid-run"),
            Script::Incomplete => Ok(TurnOutcome {
                stop_reason: StopReason::Incomplete {
                    reason: "max_output_tokens".to_owned(),
                },
                items: vec![Item::assistant("PARTIAL")],
                usage: None,
            }),
            Script::Answer(opaque) => {
                if let Some(text) = self.reasoning {
                    sink.emit(StreamEvent::ReasoningDelta {
                        text: text.to_owned(),
                    });
                }
                Ok(TurnOutcome {
                    stop_reason: StopReason::EndTurn,
                    items: vec![
                        Item::Opaque {
                            wire: self.wire.clone(),
                            payload: opaque.clone(),
                        },
                        Item::assistant("ANSWER"),
                    ],
                    usage: None,
                })
            }
        }
    }
}

struct NoTools;

impl ToolPort for NoTools {
    fn specs(&self) -> &[ToolSpec] {
        &[]
    }

    fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
        ToolOutcome::ok(json!({}))
    }
}

#[derive(Default)]
struct Collect {
    reasoning: Vec<String>,
}

impl LoopSink for Collect {
    fn emit(&mut self, event: LoopEvent) {
        if let LoopEvent::ReasoningDelta { text } = event {
            self.reasoning.push(text);
        }
    }
}

// --- helpers ------------------------------------------------------------------------------------

fn run(
    model: &mut Scripted,
    session: &mut SessionFile,
    sessions: &Path,
    input: &str,
) -> b10x_loom::session::FiledRun {
    run_with_sink(model, session, sessions, input, &mut Collect::default())
}

fn run_with_sink(
    model: &mut Scripted,
    session: &mut SessionFile,
    sessions: &Path,
    input: &str,
    sink: &mut Collect,
) -> b10x_loom::session::FiledRun {
    let mut tools = NoTools;
    let mut approvals = ApproveAll;
    let config = LoopConfig::new("adversary-model", "ADVERSARY-INSTRUCTIONS");
    let ports = RunPorts {
        model,
        tools: &mut tools,
        approvals: &mut approvals,
        config,
    };
    run_and_file(ports, session, sessions, input, sink)
}

fn open(id: &SessionId, wire: &str, workspace: &Path) -> SessionFile {
    SessionFile::open(
        &SessionData {
            session_id: id.clone(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: wire.to_owned(),
        },
        "adversary-model",
        "http://127.0.0.1:9/v1",
        workspace,
    )
    .expect("a session opens")
}

fn id(text: &str) -> SessionId {
    SessionId(Uuid(text.to_owned()))
}

fn layout(root: &Path) -> (PathBuf, PathBuf) {
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create");
    (workspace, root.join("state").join("sessions"))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-{name}-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear scratch");
    }
    root
}
