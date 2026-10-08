// SPDX-License-Identifier: Apache-2.0

//! Session and transcript state for one run: what a run leaves behind, so a second run can pick it
//! up (`loom.run.Session`, `loom.run.OpenSession`, `loom.run.FileSession`,
//! `loom.run.ResumeSession`, `loom.run.ReleaseSession`).
//!
//! Ported from Harness `crates/harness-cli/src/transcript.rs` at `798325f0` (the `Session` file,
//! `save`, `load`, `check_id`, `outside_workspace` and the owner-only modes) and from Harness
//! `open_session` and `run_command` in `crates/harness-cli/src/lib.rs` at the same revision (the
//! cross-wire refusal; filing however the run ended). What changed:
//!
//! - the identity is the generated [`SessionId`], and the session names the [`CommissionRunId`] it
//!   serves and how its last run ended ([`RunEnding`]);
//! - the file records the session's lifecycle state (`Active`, `Filed`): a resume claims a `Filed`
//!   session by writing it `Active` and refuses any other ([`SessionStateConflict`]), and a session
//!   opened under an identity already filed is refused before its run sends anything
//!   ([`SessionExists`]);
//! - the refusals are the generated [`SessionWireMismatch`], [`SessionStateConflict`] and
//!   [`SessionExists`];
//! - a run whose loop panics is filed as failed, with what its turns already spent, before the
//!   panic carries on;
//! - a session is filed up to its last completed turn: a turn left unfinished (a tool call with no
//!   result, as when the run stopped awaiting an approval) is not stored;
//! - every write is fsynced, file and directory, and the format is version 2: a Harness version 1
//!   file is refused by name.
//!
//! # Turns are stateless and replayed whole
//!
//! The loop hands back the whole conversation on every exit path ([`AgentLoop::run_in`]), and a
//! session stores it verbatim: opaque provider items with their payload never read and their wire
//! never rewritten. Streamed reasoning text reaches the caller's [`LoopSink`] as it arrives and is
//! not an item, so it is stored nowhere except inside the provider's own opaque item, when the
//! provider puts it there.
//!
//! # What `run_and_file` does not carry
//!
//! [`run_and_file`] builds the loop from [`RunPorts`] so that the wire it checks is the model
//! port's own. The loop it builds has **no operator hooks, no cancel handle and no turn
//! environment**: a run through it cannot be cancelled except by ending the process, consults no
//! hook before a call, and offers every published tool on every turn. It is for runs that need
//! none of them; a run that needs one builds its own [`AgentLoop`] and files the session with
//! [`SessionFile::extend`], [`SessionFile::spent`] and [`SessionFile::file`].
//!
//! # Turns
//!
//! [`TurnRecord`] is Loom's own, not ported: the sessions a governed run records its turns into,
//! those turns, the catalogue each turn was offered and the compactions made in them, in memory
//! (`loom.run.OpenSession`, `loom.run.ResumeSession`, `loom.run.FileSession`,
//! `loom.run.InterruptSession`, `loom.run.ReleaseSession`, `loom.run.RecordTurn`,
//! `loom.run.RecordCompaction`, `loom.run.ProjectCatalogue`, generated). It
//! is the run's record of what each completed turn added and what each compaction cost, not a
//! transcript a following run replays; the session file above is that, and its format does not
//! carry compactions or the `Interrupted` state.
//!
//! # Recovering a session by hand
//!
//! Three files can outlive a run that died without unwinding (killed, out of memory, power lost),
//! and each is refused by name rather than guessed at:
//!
//! - **a session left `Active`**: every resume is refused as held by another run. Once no run holds
//!   it, [`SessionFile::release`] files it back as [`RunEnding::Failed`] (`loom.run.ReleaseSession`)
//!   and it resumes again;
//! - **`<id>.json.lock`**, from a resume that died inside its claim: remove it once no resume runs;
//! - **`<id>.json.tmp`**, from a filing that died before placing its file: remove it once no filer
//!   runs.
//!
//! # Filesystem
//!
//! A new session is placed with a hard link, which succeeds only where no file exists: that is what
//! refuses two runs creating one identity. A filesystem without hard links (vfat, exFAT, some FUSE
//! and SMB mounts) refuses every new session by name; keep sessions on one that has them.
//!
//! # Where a session lives
//!
//! Outside the workspace, always: filing and resuming refuse a directory that resolves inside the
//! run's workspace. The directory is forced to `0700` and the file is created `0600` on unix — a
//! transcript is whatever the model read.
//!
//! # What is deliberately not in the file
//!
//! **No credential**, of any kind: the credential is fetched per call from a source the caller
//! names. **No instruction text**: the standing instruction is derived from the run's
//! configuration and is re-derived by whoever resumes the session.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::harness::turn_loop::{
    AgentLoop, ApprovalPort, LoopConfig, LoopError, LoopEvent, LoopOutcome, LoopSink, RunLedger,
};
use crate::harness::wire::{Item, ModelPort, ToolPort, Usage, WireId};
use crate::model::behaviour::{
    ActionCatalogueStorage, CompactionStorage, Generated, SessionStorage, TurnStorage,
};
use crate::model::obligation::UnmetObligation;
use crate::model::primitives::Uuid;
use crate::model::run::obligations::{
    FileSessionBehavior, InterruptSessionBehavior, OpenSessionBehavior, ProjectCatalogueBehavior,
    RecordCompactionBehavior, RecordTurnBehavior, ReleaseSessionBehavior, ResumeSessionBehavior,
};
use crate::model::run::{
    ActionCatalogueSnapshot, CatalogueId, CommissionRunId, CompactionId, CompactionSnapshot,
    FileSession, FileSessionOutcome, InterruptSession, InterruptSessionOutcome, OpenSession,
    OpenSessionOutcome, ProjectCatalogue, ProjectCatalogueOutcome, RecordCompaction,
    RecordCompactionOutcome, RecordTurn, RecordTurnOutcome, ReleaseSession, ReleaseSessionOutcome,
    ResumeSession, ResumeSessionOutcome, RunEnding, SessionData, SessionExists, SessionId,
    SessionSnapshot, SessionState, SessionStateConflict, SessionWireMismatch, TurnId, TurnSnapshot,
};

/// The shape this module writes and the only one it reads.
///
/// Version 1 is Harness's session file, which this one extends; a file that says anything else is
/// refused **by name** rather than parsed hopefully: a session is replayed into a model at the
/// caller's expense.
pub const SESSION_VERSION: u32 = 2;

/// A session as it is filed: one run's conversation, in the form a following run can replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionFile {
    /// The file format, always [`SESSION_VERSION`] when this code wrote it.
    pub version: u32,
    /// The session's identity, the canonical text of its [`SessionId`], and the file's own name.
    pub id: String,
    /// The commission run the session executes, the canonical text of its [`CommissionRunId`].
    pub commission_run: String,
    /// The wire the conversation was produced on.
    ///
    /// Carried because an opaque item may not cross wires: resuming a session on another wire is
    /// a refusal somebody has to be able to make, and it cannot be made without knowing which wire
    /// the items came from.
    pub wire: WireId,
    /// Where the session is in its lifecycle: `Active` while a run holds it, `Filed` after.
    pub state: Lifecycle,
    pub model: String,
    pub base_url: String,
    /// The workspace of the run that last held the session.
    pub workspace: PathBuf,
    pub created_unix: u64,
    pub updated_unix: u64,
    /// Turns started across every run folded in.
    pub turns: u64,
    /// How the last run folded in ended, or [`None`] before any was filed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ending: Option<Ending>,
    /// The conversation, verbatim, opaque items included.
    pub items: Vec<Item>,
    /// One entry per turn the provider reported for, across every run folded in.
    pub usage: Vec<Usage>,
    /// What the session cost so far, or [`None`] when no run in it was priced.
    pub cost_micro_usd: Option<u64>,
    /// The structured answer of the last run folded in, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured: Option<serde_json::Value>,
    /// What this value may write: a session just opened, a session this value claimed by resuming
    /// it, or nothing. Never written to the file.
    #[serde(skip)]
    hold: Hold,
}

/// [`SessionState`] as it is written in a session file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    Active,
    Filed,
}

impl From<Lifecycle> for SessionState {
    fn from(state: Lifecycle) -> Self {
        match state {
            Lifecycle::Active => Self::Active,
            Lifecycle::Filed => Self::Filed,
        }
    }
}

/// [`RunEnding`] as it is written in a session file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ending {
    Answered,
    Stopped,
    Failed,
}

impl From<RunEnding> for Ending {
    fn from(ending: RunEnding) -> Self {
        match ending {
            RunEnding::Answered => Self::Answered,
            RunEnding::Stopped => Self::Stopped,
            RunEnding::Failed => Self::Failed,
        }
    }
}

impl From<Ending> for RunEnding {
    fn from(ending: Ending) -> Self {
        match ending {
            Ending::Answered => Self::Answered,
            Ending::Stopped => Self::Stopped,
            Ending::Failed => Self::Failed,
        }
    }
}

/// What a [`SessionFile`] value may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Hold {
    /// Opened by [`SessionFile::open`]: its first filing creates the file, and refuses when one
    /// already exists.
    Opened,
    /// Claimed by [`SessionFile::resume`]: its filing replaces the file it claimed.
    Claimed,
    /// Read by [`SessionFile::load`], or already filed: it writes nothing.
    #[default]
    Released,
}

/// Why a session could not be opened, filed, read or resumed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// The session was recorded on one wire and is resumed or run on another
    /// (`loom.run.ResumeSession`, outcome `cross-wire`). Names both wires.
    #[error(
        "session `{}` was recorded on the `{}` wire and this run speaks `{}`; a provider's own \
         reasoning items are replayed verbatim and may not cross wires, so resume it on `{}` or \
         open a new session",
        .0.session_id.0.0,
        .0.session_wire,
        .0.wire,
        .0.session_wire
    )]
    WireMismatch(SessionWireMismatch),
    /// The session is not `Filed`, so it cannot be resumed: another run holds it
    /// (`loom.run.ResumeSession`, outcome `wrong-state`).
    #[error(
        "the session is {:?}, not Filed: another run holds it, and a resume would lose that run's \
         turns",
        .0.state
    )]
    StateConflict(SessionStateConflict),
    /// A session under this identity is already filed (`loom.run.OpenSession`, outcome
    /// `session-exists`).
    #[error(
        "session `{}` is already filed; resume it, or open a new session under another identity",
        .0.session_id.0.0
    )]
    Exists(SessionExists),
    /// Anything else, named with the path or value it happened on.
    #[error("{0}")]
    Refused(String),
}

impl SessionFile {
    /// An empty session for a run that is about to start (`loom.run.OpenSession`).
    ///
    /// Its first filing creates the file and is refused with [`SessionError::Exists`] when a
    /// session under the same identity is already filed.
    ///
    /// # Errors
    ///
    /// Refuses an identity that is not one file name, a wire that is not a wire identifier, and a
    /// workspace that cannot be resolved.
    pub fn open(
        data: &SessionData,
        model: impl Into<String>,
        base_url: impl Into<String>,
        workspace: &Path,
    ) -> Result<Self, SessionError> {
        let id = data.session_id.0.0.clone();
        check_id(&id)?;
        let wire = WireId::new(data.wire.clone()).map_err(|error| {
            SessionError::Refused(format!("`{}` is not a wire: {error}", data.wire))
        })?;
        let now = unix_now();
        Ok(Self {
            version: SESSION_VERSION,
            id,
            commission_run: data.commission_run.0.0.clone(),
            wire,
            state: Lifecycle::Active,
            model: model.into(),
            base_url: base_url.into(),
            workspace: resolve_workspace(workspace)?,
            created_unix: now,
            updated_unix: now,
            turns: 0,
            ending: None,
            items: Vec::new(),
            usage: Vec::new(),
            cost_micro_usd: None,
            structured: None,
            hold: Hold::Opened,
        })
    }

    /// The session as the run model holds it.
    pub fn data(&self) -> SessionData {
        SessionData {
            session_id: SessionId(Uuid(self.id.clone())),
            commission_run: CommissionRunId(Uuid(self.commission_run.clone())),
            wire: self.wire.as_str().to_owned(),
        }
    }

    /// How the last run folded in ended.
    pub fn run_ending(&self) -> Option<RunEnding> {
        self.ending.map(RunEnding::from)
    }

    /// Folds a run that returned an outcome into this session.
    ///
    /// The outcome's items **replace** rather than append: the loop replays the whole conversation
    /// every turn and hands back all of it. The usage list appends, because each entry is one turn
    /// that was really billed.
    pub fn extend(&mut self, outcome: &LoopOutcome) {
        self.items.clone_from(&outcome.items);
        self.fold_spend(&outcome.usage, outcome.turns, outcome.cost_micro_usd);
        self.structured.clone_from(&outcome.structured);
    }

    /// Folds what a run that never produced an outcome **spent** into this session.
    ///
    /// Items are not touched here: the caller stores the vector the loop wrote back.
    pub fn spent(&mut self, ledger: &RunLedger) {
        self.fold_spend(&ledger.usage, ledger.turns, ledger.cost_micro_usd);
    }

    fn fold_spend(&mut self, usage: &[Usage], turns: u64, cost_micro_usd: Option<u64>) {
        self.usage.extend(usage.iter().cloned());
        self.turns = self.turns.saturating_add(turns);
        self.cost_micro_usd = match (self.cost_micro_usd, cost_micro_usd) {
            (None, None) => None,
            (spent, added) => Some(spent.unwrap_or(0).saturating_add(added.unwrap_or(0))),
        };
        self.updated_unix = unix_now();
    }

    /// Files the session into `dir` after a run that ended as `ending` (`loom.run.FileSession`),
    /// and answers where it went. The session is `Filed` afterwards, and this value writes nothing
    /// more: a later run resumes it.
    ///
    /// `dir` must resolve outside the session's workspace. The file appears whole or not at all:
    /// written to `<id>.json.tmp`, created new with mode `0600` and fsynced, then moved onto
    /// `<id>.json` in a directory forced to `0700`, which is fsynced after the move.
    ///
    /// # Errors
    ///
    /// Names a directory inside the workspace; [`SessionError::StateConflict`] for a session this
    /// value does not hold, because it was read or already filed (`FileSession` `wrong-state`);
    /// [`SessionError::Exists`] for a session opened under an identity already filed; a temporary
    /// file another filer left or holds; and every other failure with the path it happened on.
    pub fn file(&mut self, dir: &Path, ending: RunEnding) -> Result<PathBuf, SessionError> {
        let dir = outside_workspace(dir, &self.workspace)?;
        if self.hold == Hold::Released {
            return Err(SessionError::StateConflict(SessionStateConflict {
                state: self.state.into(),
            }));
        }
        let previous = (self.state, self.ending, self.updated_unix);
        self.state = Lifecycle::Filed;
        self.ending = Some(ending.into());
        self.updated_unix = unix_now();
        let written = match self.hold {
            Hold::Opened => self.write(&dir, Placement::Create),
            _ => self.write(&dir, Placement::Replace),
        };
        match written {
            Ok(path) => {
                self.hold = Hold::Released;
                Ok(path)
            }
            Err(error) => {
                (self.state, self.ending, self.updated_unix) = previous;
                Err(error)
            }
        }
    }

    /// Reads one filed session by identity, without claiming it.
    ///
    /// # Errors
    ///
    /// Names the file when it cannot be read, when it is not a session file, when it says a
    /// version this build does not read, and when the identity it holds is not the one its name
    /// says.
    pub fn load(dir: &Path, id: &SessionId) -> Result<Self, SessionError> {
        let id = id.0.0.as_str();
        check_id(id)?;
        let path = dir.join(format!("{id}.json"));
        let text = fs::read_to_string(&path).map_err(|error| {
            refused(format!("reading the session `{}`: {error}", path.display()))
        })?;
        let session = Self::parse(&text, &path)?;
        if session.id != id {
            return Err(refused(format!(
                "`{}` holds session `{}`, not `{id}`; it is refused rather than resumed as the \
                 wrong session",
                path.display(),
                session.id
            )));
        }
        Ok(session)
    }

    /// The filed session `id`, continued by a run speaking `wire` over `workspace`
    /// (`loom.run.ResumeSession`).
    ///
    /// The same session, not a new one: its identity, items and spend carry on, and filing it
    /// again replaces the same file. Resuming **claims** it: under an exclusive `<id>.json.lock`
    /// the file is read again, checked, and written back `Active` with this run's workspace, so a
    /// second resume is refused until this run files it.
    ///
    /// # Errors
    ///
    /// [`SessionError::WireMismatch`], naming both wires, for a session recorded on another wire,
    /// and [`SessionError::StateConflict`] for one not `Filed` — both before anything is sent.
    /// Also a directory inside `workspace`, a claim another resume holds, and the failures of
    /// [`SessionFile::load`].
    pub fn resume(
        dir: &Path,
        id: &SessionId,
        wire: &WireId,
        workspace: &Path,
    ) -> Result<Self, SessionError> {
        let workspace = resolve_workspace(workspace)?;
        let dir = outside_workspace(dir, &workspace)?;
        check_id(&id.0.0)?;
        let _claim = Claim::take(&dir, &id.0.0)?;
        let mut session = Self::load(&dir, id)?;
        if session.wire.as_str() != wire.as_str() {
            return Err(SessionError::WireMismatch(SessionWireMismatch {
                session_id: id.clone(),
                session_wire: session.wire.as_str().to_owned(),
                wire: wire.as_str().to_owned(),
            }));
        }
        if session.state != Lifecycle::Filed {
            return Err(SessionError::StateConflict(SessionStateConflict {
                state: session.state.into(),
            }));
        }
        session.state = Lifecycle::Active;
        session.workspace = workspace;
        session.updated_unix = unix_now();
        session.write(&dir, Placement::Replace)?;
        session.hold = Hold::Claimed;
        Ok(session)
    }

    /// Files back, as [`RunEnding::Failed`], a session left `Active` by a run that died without
    /// filing it (`loom.run.ReleaseSession`), so a later run can resume it.
    ///
    /// An operator's recovery: call it only once no run holds the session. It is taken under the
    /// same `<id>.json.lock` a resume takes, and the conversation is left as last filed.
    ///
    /// # Errors
    ///
    /// [`SessionError::StateConflict`] for a session that is not `Active`, a claim another resume
    /// holds, a directory inside the session's recorded workspace, and the failures of
    /// [`SessionFile::load`].
    pub fn release(dir: &Path, id: &SessionId) -> Result<PathBuf, SessionError> {
        check_id(&id.0.0)?;
        let _claim = Claim::take(dir, &id.0.0)?;
        let mut session = Self::load(dir, id)?;
        if session.state != Lifecycle::Active {
            return Err(SessionError::StateConflict(SessionStateConflict {
                state: session.state.into(),
            }));
        }
        let dir = outside_workspace(dir, &session.workspace)?;
        session.state = Lifecycle::Filed;
        session.ending = Some(Ending::Failed);
        session.updated_unix = unix_now();
        session.write(&dir, Placement::Replace)
    }

    /// Files a claimed session back unchanged, releasing the claim: a resumed session whose run
    /// was refused before it started is filed again, as it was.
    fn unclaim(&mut self, dir: &Path) -> Result<PathBuf, SessionError> {
        let dir = outside_workspace(dir, &self.workspace)?;
        self.state = Lifecycle::Filed;
        self.updated_unix = unix_now();
        let path = self.write(&dir, Placement::Replace)?;
        self.hold = Hold::Released;
        Ok(path)
    }

    /// One session's JSON, with the version checked before the shape is trusted.
    fn parse(text: &str, path: &Path) -> Result<Self, SessionError> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            refused(format!(
                "`{}` is not a session file: {error}",
                path.display()
            ))
        })?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                refused(format!(
                    "`{}` does not say which session version it is, so it cannot be replayed",
                    path.display()
                ))
            })?;
        if version == 1 {
            return Err(refused(format!(
                "`{}` is a version 1 session, Harness's format, and this build reads version \
                 {SESSION_VERSION}; it is refused rather than replayed under the wrong shape",
                path.display()
            )));
        }
        if version != u64::from(SESSION_VERSION) {
            return Err(refused(format!(
                "`{}` is a version {version} session and this build reads version \
                 {SESSION_VERSION}; it is refused rather than replayed under the wrong shape",
                path.display()
            )));
        }
        // From the text, not from `value`: a build that unifies serde_json's `arbitrary_precision`
        // hands an integer above `u64::MAX` in a `Value` to the internally tagged items as a
        // `u128`, which serde's buffer for them refuses; read from the text, it stays a number.
        serde_json::from_str(text).map_err(|error| {
            refused(format!(
                "`{}` is not a session file: {error}",
                path.display()
            ))
        })
    }

    /// Writes the session into `dir`: a new temporary file, fsynced, then created as or moved onto
    /// `<id>.json`, and the directory fsynced.
    fn write(&self, dir: &Path, how: Placement) -> Result<PathBuf, SessionError> {
        check_id(&self.id)?;
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|error| {
                refused(format!(
                    "creating the session directory `{}`: {error}",
                    dir.display()
                ))
            })?;
        }
        restrict(dir)?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| refused(format!("encoding session `{}`: {error}", self.id)))?;
        let temporary = dir.join(format!("{}.json.tmp", self.id));
        let path = dir.join(format!("{}.json", self.id));
        write_private(&temporary, text.as_bytes())?;
        let placed = match how {
            // A hard link is created only where nothing exists: two runs opening one identity
            // cannot both create it, and an identity already filed is refused, not replaced.
            Placement::Create => fs::hard_link(&temporary, &path).map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    SessionError::Exists(SessionExists {
                        session_id: SessionId(Uuid(self.id.clone())),
                    })
                } else {
                    refused(format!(
                        "creating `{}` as a hard link to `{}`: {error}; a new session is placed \
                         with a hard link, so keep sessions on a filesystem that has them",
                        path.display(),
                        temporary.display()
                    ))
                }
            }),
            Placement::Replace => fs::rename(&temporary, &path).map_err(|error| {
                refused(format!(
                    "renaming `{}` onto `{}`: {error}",
                    temporary.display(),
                    path.display()
                ))
            }),
        };
        if how == Placement::Create || placed.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        placed?;
        sync_dir(dir)?;
        Ok(path)
    }
}

/// How [`SessionFile::write`] places the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// Only where no file exists.
    Create,
    /// Over the file this value claimed.
    Replace,
}

/// The exclusive claim one resume holds on a session while it reads, checks and claims it.
///
/// `<id>.json.lock`, created new: a second resume of the same session at the same moment is
/// refused by name rather than reading the same `Filed` state. Removed when dropped.
struct Claim {
    path: PathBuf,
}

impl Claim {
    fn take(dir: &Path, id: &str) -> Result<Self, SessionError> {
        let path = dir.join(format!("{id}.json.lock"));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == ErrorKind::AlreadyExists {
                    refused(format!(
                        "session `{id}` is being resumed by another run (`{}` exists); if no run \
                         is resuming it, remove that file",
                        path.display()
                    ))
                } else {
                    refused(format!("claiming `{}`: {error}", path.display()))
                }
            })?;
        Ok(Self { path })
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The sessions a governed run records its turns into, those turns, the catalogue offered for each
/// turn and the compactions made in them, each in the order recorded.
///
/// `loom.run.OpenSession`, `loom.run.ResumeSession`, `loom.run.FileSession`,
/// `loom.run.InterruptSession`, `loom.run.ReleaseSession`, `loom.run.RecordTurn`,
/// `loom.run.RecordCompaction` and `loom.run.ProjectCatalogue` are the generated behaviour over
/// this record: a session opened under an identity it already holds is refused `session-exists`,
/// only a `Filed` or `Interrupted` session resumes, only an `Active` one is filed, interrupted or
/// released, a turn or a compaction is refused for a session it does not hold or one no longer
/// `Active`, and a catalogue projected under an identity it already holds is refused
/// `catalogue-exists`. A session, a turn or a compaction stored under an identity already held
/// replaces it. Catalogues a governed run offers are kept as they were offered, one per offer,
/// never replaced; one stored through [`ActionCatalogueStorage`] under an identity already held
/// replaces the first held under it.
#[derive(Debug, Default)]
pub struct TurnRecord {
    sessions: Vec<SessionSnapshot>,
    turns: Vec<TurnSnapshot>,
    catalogues: Vec<ActionCatalogueSnapshot>,
    compactions: Vec<CompactionSnapshot>,
}

impl TurnRecord {
    /// Every session opened here.
    #[must_use]
    pub fn sessions(&self) -> &[SessionSnapshot] {
        &self.sessions
    }

    /// The state `session` is in, or [`None`] for a session never opened here.
    #[must_use]
    pub fn state_of(&self, session: &SessionId) -> Option<SessionState> {
        self.sessions
            .iter()
            .find(|held| &held.data.session_id == session)
            .map(|held| held.state)
    }

    /// Every catalogue a turn was offered, in the order offered.
    #[must_use]
    pub fn catalogues(&self) -> &[ActionCatalogueSnapshot] {
        &self.catalogues
    }

    /// Keeps `catalogue` as the one offered for its turn.
    pub(crate) fn offered(&mut self, catalogue: ActionCatalogueSnapshot) {
        self.catalogues.push(catalogue);
    }

    /// Every recorded turn, in the order recorded.
    #[must_use]
    pub fn turns(&self) -> &[TurnSnapshot] {
        &self.turns
    }

    /// How many turns `session` holds.
    #[must_use]
    pub fn turns_of(&self, session: &SessionId) -> usize {
        self.turns
            .iter()
            .filter(|turn| &turn.data.session_id == session)
            .count()
    }

    /// Every recorded compaction, in the order recorded.
    #[must_use]
    pub fn compactions(&self) -> &[CompactionSnapshot] {
        &self.compactions
    }

    /// How many compactions `session` holds.
    #[must_use]
    pub fn compactions_of(&self, session: &SessionId) -> usize {
        self.compactions
            .iter()
            .filter(|compaction| &compaction.data.session_id == session)
            .count()
    }

    /// Runs one generated behaviour over this record, which it holds for the call.
    fn generated<T>(&mut self, behaviour: impl FnOnce(&mut Generated<Self>) -> T) -> T {
        let mut generated = Generated::new(std::mem::take(self));
        let answer = behaviour(&mut generated);
        *self = generated.ports;
        answer
    }
}

impl SessionStorage for TurnRecord {
    fn get(&self, identity: &SessionId) -> Option<SessionSnapshot> {
        self.sessions
            .iter()
            .find(|held| &held.data.session_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: SessionSnapshot) {
        match self
            .sessions
            .iter_mut()
            .find(|held| held.data.session_id == snapshot.data.session_id)
        {
            Some(held) => *held = snapshot,
            None => self.sessions.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &SessionId) {
        self.sessions
            .retain(|held| &held.data.session_id != identity);
    }

    fn list(&self) -> Vec<SessionSnapshot> {
        self.sessions.clone()
    }
}

impl TurnStorage for TurnRecord {
    fn get(&self, identity: &TurnId) -> Option<TurnSnapshot> {
        self.turns
            .iter()
            .find(|held| &held.data.turn_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: TurnSnapshot) {
        match self
            .turns
            .iter_mut()
            .find(|held| held.data.turn_id == snapshot.data.turn_id)
        {
            Some(held) => *held = snapshot,
            None => self.turns.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &TurnId) {
        self.turns.retain(|held| &held.data.turn_id != identity);
    }
}

impl CompactionStorage for TurnRecord {
    fn get(&self, identity: &CompactionId) -> Option<CompactionSnapshot> {
        self.compactions
            .iter()
            .find(|held| &held.data.compaction_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: CompactionSnapshot) {
        match self
            .compactions
            .iter_mut()
            .find(|held| held.data.compaction_id == snapshot.data.compaction_id)
        {
            Some(held) => *held = snapshot,
            None => self.compactions.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &CompactionId) {
        self.compactions
            .retain(|held| &held.data.compaction_id != identity);
    }
}

impl ActionCatalogueStorage for TurnRecord {
    fn get(&self, identity: &CatalogueId) -> Option<ActionCatalogueSnapshot> {
        self.catalogues
            .iter()
            .find(|held| &held.data.catalogue_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: ActionCatalogueSnapshot) {
        match self
            .catalogues
            .iter_mut()
            .find(|held| held.data.catalogue_id == snapshot.data.catalogue_id)
        {
            Some(held) => *held = snapshot,
            None => self.catalogues.push(snapshot),
        }
    }

    fn delete(&mut self, identity: &CatalogueId) {
        self.catalogues
            .retain(|held| &held.data.catalogue_id != identity);
    }

    fn list(&self) -> Vec<ActionCatalogueSnapshot> {
        self.catalogues.clone()
    }
}

/// `loom.run.ProjectCatalogue`, generated, over this record: a catalogue identity is projected
/// once.
impl ProjectCatalogueBehavior for TurnRecord {
    fn project_catalogue(
        &mut self,
        input: ProjectCatalogue,
    ) -> Result<ProjectCatalogueOutcome, UnmetObligation> {
        self.generated(|generated| generated.project_catalogue(input))
    }
}

/// `loom.run.ReleaseSession`, generated, over this record: only an `Active` session is released,
/// filed back as failed.
impl ReleaseSessionBehavior for TurnRecord {
    fn release_session(
        &mut self,
        input: ReleaseSession,
    ) -> Result<ReleaseSessionOutcome, UnmetObligation> {
        self.generated(|generated| generated.release_session(input))
    }
}

/// `loom.run.OpenSession`, generated, over this record.
impl OpenSessionBehavior for TurnRecord {
    fn open_session(&mut self, input: OpenSession) -> Result<OpenSessionOutcome, UnmetObligation> {
        self.generated(|generated| generated.open_session(input))
    }
}

/// `loom.run.RecordTurn`, generated, over this record.
impl RecordTurnBehavior for TurnRecord {
    fn record_turn(&mut self, input: RecordTurn) -> Result<RecordTurnOutcome, UnmetObligation> {
        self.generated(|generated| generated.record_turn(input))
    }
}

/// `loom.run.RecordCompaction`, generated, over this record: only into an `Active` session.
impl RecordCompactionBehavior for TurnRecord {
    fn record_compaction(
        &mut self,
        input: RecordCompaction,
    ) -> Result<RecordCompactionOutcome, UnmetObligation> {
        self.generated(|generated| generated.record_compaction(input))
    }
}

/// `loom.run.InterruptSession`, generated, over this record: only an `Active` session is
/// interrupted.
impl InterruptSessionBehavior for TurnRecord {
    fn interrupt_session(
        &mut self,
        input: InterruptSession,
    ) -> Result<InterruptSessionOutcome, UnmetObligation> {
        self.generated(|generated| generated.interrupt_session(input))
    }
}

/// `loom.run.ResumeSession`, generated, over this record: only a `Filed` or `Interrupted` session
/// resumes.
impl ResumeSessionBehavior for TurnRecord {
    fn resume_session(
        &mut self,
        input: ResumeSession,
    ) -> Result<ResumeSessionOutcome, UnmetObligation> {
        self.generated(|generated| generated.resume_session(input))
    }
}

/// `loom.run.FileSession`, generated, over this record: only an `Active` session is filed.
impl FileSessionBehavior for TurnRecord {
    fn file_session(&mut self, input: FileSession) -> Result<FileSessionOutcome, UnmetObligation> {
        self.generated(|generated| generated.file_session(input))
    }
}

/// The ports one run uses. [`run_and_file`] builds the loop from them, so the wire it compares
/// with the session's is the model port's own.
pub struct RunPorts<'a> {
    pub model: &'a mut dyn ModelPort,
    pub tools: &'a mut dyn ToolPort,
    pub approvals: &'a mut dyn ApprovalPort,
    pub config: LoopConfig,
}

/// A run over a session, and where the session was filed afterwards.
#[derive(Debug)]
pub struct FiledRun {
    /// What the loop answered.
    pub run: Result<LoopOutcome, LoopError>,
    /// Where the session was filed, or why it could not be.
    pub filed: Result<PathBuf, SessionError>,
}

/// Runs a loop over `session`'s conversation with the new `input`, then files the session into
/// `dir` however the run ended: [`RunEnding::Answered`] when the model answered,
/// [`RunEnding::Stopped`] for any other stop, [`RunEnding::Failed`] for an error. A run that
/// failed on turn twenty is exactly the one whose nineteen completed turns must survive.
///
/// The session is filed up to its last completed turn: the items of a turn left unfinished — a tool
/// call with no result after it, as when the run stopped awaiting an approval — are not stored,
/// because replaying a call without its result is a request no provider accepts.
///
/// Refused before the loop is built, with nothing sent: `run` is [`LoopError::Config`] and
/// `filed` names the refusal.
///
/// - [`SessionError::WireMismatch`] for a session recorded on another wire than the model port's.
///   A session this value claimed by resuming it is filed back unchanged, so the claim is released.
/// - [`SessionError::Exists`] for a session opened under an identity already filed in `dir`.
/// - A `dir` inside the session's workspace.
///
/// The loop is built without hooks, a cancel handle or a turn environment (see the module docs).
///
/// # Panics
///
/// When the loop panics, the session is filed as [`RunEnding::Failed`] holding the conversation it
/// held before the run and what the run's turns spent, tallied from the [`LoopEvent::Usage`] and
/// [`LoopEvent::Cost`] events that passed through `sink`, and the panic then carries on.
pub fn run_and_file(
    ports: RunPorts<'_>,
    session: &mut SessionFile,
    dir: &Path,
    input: impl Into<String>,
    sink: &mut dyn LoopSink,
) -> FiledRun {
    let RunPorts {
        model,
        tools,
        approvals,
        config,
    } = ports;
    if let Err(refusal) = before_the_run(session, model.wire(), dir) {
        return FiledRun {
            run: Err(LoopError::Config(refusal.to_string())),
            filed: Err(refusal),
        };
    }
    let before = session.items.clone();
    let mut items = std::mem::take(&mut session.items);
    let mut spend = RunLedger::default();
    let input = input.into();
    let mut tally = Tally::new(sink);
    let run = catch_unwind(AssertUnwindSafe(|| {
        AgentLoop::new(model, tools, approvals, config)
            .run_in(&mut items, &mut spend, input, &mut tally)
    }));
    let run = match run {
        Ok(run) => run,
        Err(panic) => {
            session.items = before;
            session.fold_spend(&tally.usage, tally.turns, tally.cost_micro_usd);
            // The run died; its session is filed all the same, and the panic is not swallowed.
            let _ = session.file(dir, RunEnding::Failed);
            resume_unwind(panic);
        }
    };
    let ending = match &run {
        Ok(outcome) => {
            session.extend(outcome);
            if outcome.stop.is_completed() {
                RunEnding::Answered
            } else {
                RunEnding::Stopped
            }
        }
        Err(_) => {
            session.items = items;
            session.spent(&spend);
            RunEnding::Failed
        }
    };
    drop_unfinished_turn(&mut session.items);
    let filed = session.file(dir, ending);
    FiledRun { run, filed }
}

/// The refusals [`run_and_file`] makes before the loop is built.
fn before_the_run(
    session: &mut SessionFile,
    wire: &WireId,
    dir: &Path,
) -> Result<(), SessionError> {
    if wire.as_str() != session.wire.as_str() {
        let refusal = SessionError::WireMismatch(SessionWireMismatch {
            session_id: SessionId(Uuid(session.id.clone())),
            session_wire: session.wire.as_str().to_owned(),
            wire: wire.as_str().to_owned(),
        });
        if session.hold == Hold::Claimed {
            session.unclaim(dir)?;
        }
        return Err(refusal);
    }
    let resolved = outside_workspace(dir, &session.workspace)?;
    if session.hold == Hold::Opened && resolved.join(format!("{}.json", session.id)).exists() {
        return Err(SessionError::Exists(SessionExists {
            session_id: SessionId(Uuid(session.id.clone())),
        }));
    }
    Ok(())
}

/// Removes the items of a trailing turn that did not complete: from the model's output that opened
/// the first tool call with no result after it, to the end.
///
/// A turn's output is the run of model items (`AssistantText`, `ToolCall`, `Opaque`) after the
/// last user input or tool result; the cut is made at its start, so a completed turn is never split.
fn drop_unfinished_turn(items: &mut Vec<Item>) {
    let unanswered = items.iter().enumerate().position(|(at, item)| match item {
        Item::ToolCall(call) => !items[at + 1..].iter().any(
            |later| matches!(later, Item::ToolResult { call_id, .. } if *call_id == call.call_id),
        ),
        _ => false,
    });
    let Some(mut cut) = unanswered else {
        return;
    };
    while cut > 0
        && !matches!(
            items[cut - 1],
            Item::UserText { .. } | Item::ToolResult { .. }
        )
    {
        cut -= 1;
    }
    items.truncate(cut);
}

/// Forwards every event to the caller's sink and tallies what the run spent, so that a run whose
/// loop panics — and hands back no ledger — is not filed as free.
struct Tally<'s> {
    sink: &'s mut dyn LoopSink,
    usage: Vec<Usage>,
    cost_micro_usd: Option<u64>,
    turns: u64,
}

impl<'s> Tally<'s> {
    fn new(sink: &'s mut dyn LoopSink) -> Self {
        Self {
            sink,
            usage: Vec::new(),
            cost_micro_usd: None,
            turns: 0,
        }
    }

    /// Usage and cost count wherever they were reported, a delegate's included, as
    /// [`RunLedger::usage`] does; turns count the run's own.
    fn count(&mut self, event: &LoopEvent, nested: bool) {
        match event {
            LoopEvent::Usage(usage) => self.usage.push(usage.clone()),
            LoopEvent::Cost { micro_usd, .. } => {
                self.cost_micro_usd =
                    Some(self.cost_micro_usd.unwrap_or(0).saturating_add(*micro_usd));
            }
            LoopEvent::TurnStarted { .. } if !nested => self.turns += 1,
            LoopEvent::Delegated { event, .. } => self.count(event, true),
            _ => {}
        }
    }
}

impl LoopSink for Tally<'_> {
    fn emit(&mut self, event: LoopEvent) {
        self.count(&event, false);
        self.sink.emit(event);
    }
}

/// Resolves a possibly not-yet-created directory and refuses one inside `workspace`.
///
/// Existing ancestors are canonicalized, so a symlink cannot disguise an in-workspace target, and
/// every component is judged by its own metadata: a dangling symlink is refused, because where it
/// would lead cannot be resolved until something creates it.
///
/// # Errors
///
/// Names an unresolvable path or workspace, a dangling symlink, and a directory which resolves
/// inside the workspace.
pub fn outside_workspace(dir: &Path, workspace: &Path) -> Result<PathBuf, SessionError> {
    let workspace = resolve_workspace(workspace)?;
    let mut candidate = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                refused(format!(
                    "resolving the current directory for sessions: {error}"
                ))
            })?
            .join(dir)
    };
    let no_ancestor = || {
        refused(format!(
            "session directory `{}` has no existing ancestor",
            dir.display()
        ))
    };
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() && !candidate.exists() => {
                return Err(refused(format!(
                    "session directory `{}` passes through `{}`, a symlink to nothing yet; where \
                     it leads cannot be judged against workspace `{}`",
                    dir.display(),
                    candidate.display(),
                    workspace.display()
                )));
            }
            Ok(_) => break,
            Err(_) => {
                let name = candidate.file_name().ok_or_else(no_ancestor)?;
                missing.push(name.to_owned());
                candidate = candidate.parent().ok_or_else(no_ancestor)?.to_path_buf();
            }
        }
    }
    candidate = candidate.canonicalize().map_err(|error| {
        refused(format!(
            "resolving session directory ancestor `{}`: {error}",
            candidate.display()
        ))
    })?;
    for component in missing.into_iter().rev() {
        candidate.push(component);
    }
    if candidate.starts_with(&workspace) {
        return Err(refused(format!(
            "session directory `{}` is inside workspace `{}`; transcripts must be written outside \
             the workspace",
            candidate.display(),
            workspace.display()
        )));
    }
    Ok(candidate)
}

fn resolve_workspace(workspace: &Path) -> Result<PathBuf, SessionError> {
    workspace.canonicalize().map_err(|error| {
        refused(format!(
            "resolving workspace `{}`: {error}",
            workspace.display()
        ))
    })
}

fn refused(message: String) -> SessionError {
    SessionError::Refused(message)
}

/// Seconds since the epoch, or zero on a clock set before it.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Refuses an identifier that is not exactly one file name.
///
/// `<dir>/<id>.json` with an `id` of `../../.ssh/config` is a write outside the session directory.
/// Letters, digits, `-`, `_` and `.`, and never `..` or a leading `.`.
fn check_id(id: &str) -> Result<(), SessionError> {
    let legal =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.';
    if id.is_empty() || !id.bytes().all(legal) || id.contains("..") || id.starts_with('.') {
        return Err(refused(format!(
            "`{id}` is not a session identifier: identifiers are letters, digits, `-`, `_` and \
             `.`, and never `..` or a leading `.`"
        )));
    }
    Ok(())
}

/// Makes the session directory readable only by its owner.
#[cfg(unix)]
fn restrict(dir: &Path) -> Result<(), SessionError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
        refused(format!(
            "restricting the session directory `{}` to its owner: {error}",
            dir.display()
        ))
    })
}

/// Creates `path` new — a temporary file another filer left or holds is refused by name, never
/// removed — with mode `0600`, writes `bytes` and fsyncs it.
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SessionError> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| not_created(path, &error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            let _ = fs::remove_file(path);
            refused(format!("writing `{}`: {error}", path.display()))
        })
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SessionError> {
    use std::io::Write as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| not_created(path, &error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| refused(format!("writing `{}`: {error}", path.display())))
}

/// Fsyncs the directory, so the move or link that placed the file survives a crash.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<(), SessionError> {
    fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|error| refused(format!("syncing `{}`: {error}", dir.display())))
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> Result<(), SessionError> {
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_dir: &Path) -> Result<(), SessionError> {
    Ok(())
}

/// Why the temporary file could not be created. One that exists was left or is held by another
/// filer: refused by name, never removed, with how to recover.
fn not_created(path: &Path, error: &std::io::Error) -> SessionError {
    if error.kind() == ErrorKind::AlreadyExists {
        refused(format!(
            "`{}` exists: another filer is writing this session, or one died before placing it; \
             if no filer is running, remove that file",
            path.display()
        ))
    } else {
        refused(format!(
            "creating private session file `{}`: {error}",
            path.display()
        ))
    }
}
