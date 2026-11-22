//! [`decode_suite`]: a `loom.tree-suite/1` document as the generated `loom.tree.TreeSuite`, its
//! defaults applied to every cycle, or the first member the format refuses, named by its path
//! (`cycles[2].steps[0].do`).

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Read as _;
use std::path::Path;

use loom::tree::{
    ArtifactRevision, CaseSelector, CaseSource, CasesSource, CheckKind, CompareOp, Condition,
    ConditionGroup, Conditions, CycleArea, CycleCondition, CycleExpectation, CycleStep,
    DeclaredName, DocSource, ExitExpectation, FilesSource, Follow, FollowTarget, LengthBound,
    LogSource, Operand, OperandTake, PickPosition, ReadAs, RecordsSource, ScriptedApproval,
    ScriptedChoice, ScriptedClose, ScriptedDecision, SeedEntry, SelectItem, SelectPick, StepKind,
    StopExpectation, SuiteCheck, SuiteCycle, SuiteDefaults, SuiteRefusal, SuiteRefusalReason,
    TreeStop, TreeSuite, UseBound, ValueType,
};
use serde_json::{Map, Value};

use super::availability::{KindUse, known};
use super::path::{parse_value_path, stays_below};
use super::value::written;

/// The `format` of every suite.
pub const SUITE_FORMAT: &str = "loom.tree-suite/1";

/// The most bytes a suite document is read for.
pub const MAX_SUITE_BYTES: u64 = 1024 * 1024;

/// The most bytes of one regex a suite writes.
pub const MAX_REGEX_BYTES: usize = 1024;

/// The rounds of `tree run` a cycle takes when it states no `max_rounds`.
pub const DEFAULT_MAX_ROUNDS: i64 = 40;

/// The most rounds a cycle may state.
pub const MAX_ROUNDS: i64 = 1000;

/// The most bytes of a cycle's or a step's name.
const MAX_NAME_BYTES: usize = 100;

/// A refused suite: the generated `loom.tree.SuiteRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteRefused(pub SuiteRefusal);

impl SuiteRefused {
    /// A refusal of `reason` at `member` saying `detail`.
    pub fn new(
        reason: SuiteRefusalReason,
        member: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self(SuiteRefusal {
            reason,
            member,
            detail: detail.into(),
        })
    }

    /// The reason.
    pub fn reason(&self) -> SuiteRefusalReason {
        self.0.reason
    }

    /// The member path it names.
    pub fn member(&self) -> Option<&str> {
        self.0.member.as_deref()
    }
}

impl fmt::Display for SuiteRefused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:?}", self.0.reason)?;
        if let Some(member) = &self.0.member {
            write!(out, ": {member}")?;
        }
        write!(out, ": {}", self.0.detail)
    }
}

impl std::error::Error for SuiteRefused {}

fn refuse(reason: SuiteRefusalReason, at: &str, detail: impl Into<String>) -> SuiteRefused {
    SuiteRefused::new(reason, Some(at.to_owned()), detail)
}

fn unreadable(at: &str, detail: impl Into<String>) -> SuiteRefused {
    refuse(SuiteRefusalReason::SuiteUnreadable, at, detail)
}

type Decoded<T> = Result<T, SuiteRefused>;

/// Reads and decodes the suite file `path`: a regular file (a link is not followed) of at most
/// [`MAX_SUITE_BYTES`].
///
/// # Errors
/// `SuiteUnreadable` when it is no such file; otherwise what [`decode_suite`] refuses.
pub fn read_suite(path: &Path) -> Decoded<TreeSuite> {
    let whole =
        |detail: String| SuiteRefused::new(SuiteRefusalReason::SuiteUnreadable, None, detail);
    match fs::symlink_metadata(path) {
        Ok(entry) if entry.file_type().is_file() => {}
        Ok(_) => return Err(whole(format!("{} is not a regular file", path.display()))),
        Err(error) => return Err(whole(format!("{}: {error}", path.display()))),
    }
    let mut bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.take(MAX_SUITE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| whole(format!("{}: {error}", path.display())))?;
    if bytes.len() as u64 > MAX_SUITE_BYTES {
        return Err(whole(format!("{} is larger than 1 MiB", path.display())));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| whole(format!("{} is not UTF-8", path.display())))?;
    decode_suite(&text)
}

/// Decodes the suite `text` (see the module documentation).
///
/// # Errors
/// [`SuiteRefused`] naming the first member the format refuses.
pub fn decode_suite(text: &str) -> Decoded<TreeSuite> {
    if text.len() as u64 > MAX_SUITE_BYTES {
        return Err(SuiteRefused::new(
            SuiteRefusalReason::SuiteUnreadable,
            None,
            "the suite is larger than 1 MiB",
        ));
    }
    let document: Value = serde_yaml_ng::from_str(text).map_err(|error| {
        SuiteRefused::new(
            SuiteRefusalReason::SuiteUnreadable,
            None,
            format!("the suite is no YAML document of string-keyed maps: {error}"),
        )
    })?;
    let top = Members::of(&document, "")?;
    top.allow(&["format", "defaults", "cycles"])?;
    let format = top.required_text("format")?;
    if format != SUITE_FORMAT {
        return Err(unreadable(
            "format",
            format!("is {format:?}, not {SUITE_FORMAT}"),
        ));
    }
    let defaults = top
        .get("defaults")
        .map(|value| decode_defaults(value, "defaults"))
        .transpose()?;
    let Some(cycles) = top.get("cycles") else {
        return Err(refuse(
            SuiteRefusalReason::MemberMissing,
            "cycles",
            "a suite holds its cycles",
        ));
    };
    let cycles = list(cycles, "cycles")?;
    let mut decoded = Vec::with_capacity(cycles.len());
    let mut names = BTreeSet::new();
    for (index, cycle) in cycles.iter().enumerate() {
        let at = format!("cycles[{index}]");
        let cycle = decode_cycle(cycle, &at, defaults.as_ref())?;
        if !names.insert(cycle.name.clone()) {
            return Err(refuse(
                SuiteRefusalReason::CycleTwice,
                &format!("{at}.name"),
                format!("the cycle {:?} is named twice", cycle.name),
            ));
        }
        decoded.push(cycle);
    }
    Ok(TreeSuite {
        format,
        defaults,
        cycles: decoded,
    })
}

/// The members of one map of the document, with its member path.
struct Members<'a> {
    at: String,
    members: &'a Map<String, Value>,
}

impl<'a> Members<'a> {
    fn of(value: &'a Value, at: &str) -> Decoded<Self> {
        match value {
            Value::Object(members) => Ok(Self {
                at: at.to_owned(),
                members,
            }),
            other => Err(unreadable(
                if at.is_empty() { "." } else { at },
                format!("is {}, not a map", describe(other)),
            )),
        }
    }

    fn path(&self, name: &str) -> String {
        if self.at.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{name}", self.at)
        }
    }

    /// Refuses the first member not in `declared`.
    fn allow(&self, declared: &[&str]) -> Decoded<()> {
        match self
            .members
            .keys()
            .find(|name| !declared.contains(&name.as_str()))
        {
            Some(name) => Err(refuse(
                SuiteRefusalReason::MemberUndeclared,
                &self.path(name),
                format!(
                    "is not a member the format declares here; it declares {}",
                    declared.join(", ")
                ),
            )),
            None => Ok(()),
        }
    }

    fn get(&self, name: &str) -> Option<&'a Value> {
        self.members.get(name)
    }

    fn text(&self, name: &str) -> Decoded<Option<String>> {
        match self.members.get(name) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(other) => Err(unreadable(
                &self.path(name),
                format!("is {}, not a text", describe(other)),
            )),
        }
    }

    fn required_text(&self, name: &str) -> Decoded<String> {
        self.text(name)?.ok_or_else(|| self.missing(name))
    }

    fn missing(&self, name: &str) -> SuiteRefused {
        refuse(
            SuiteRefusalReason::MemberMissing,
            &self.path(name),
            "is required here",
        )
    }

    fn integer(&self, name: &str) -> Decoded<Option<i64>> {
        match self.members.get(name) {
            None => Ok(None),
            Some(value) => value.as_i64().map(Some).ok_or_else(|| {
                unreadable(
                    &self.path(name),
                    format!("is {}, not an integer", describe(value)),
                )
            }),
        }
    }

    fn boolean(&self, name: &str) -> Decoded<Option<bool>> {
        match self.members.get(name) {
            None => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(other) => Err(unreadable(
                &self.path(name),
                format!("is {}, not true or false", describe(other)),
            )),
        }
    }

    fn texts(&self, name: &str) -> Decoded<Option<Vec<String>>> {
        self.members
            .get(name)
            .map(|value| texts(value, &self.path(name)))
            .transpose()
    }
}

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a text",
        Value::Array(_) => "a list",
        Value::Object(_) => "a map",
    }
}

fn list<'a>(value: &'a Value, at: &str) -> Decoded<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| unreadable(at, format!("is {}, not a list", describe(value))))
}

/// A text, or a list of texts.
fn texts(value: &Value, at: &str) -> Decoded<Vec<String>> {
    match value {
        Value::String(text) => Ok(vec![text.clone()]),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| unreadable(&format!("{at}[{index}]"), "is not a text"))
            })
            .collect(),
        other => Err(unreadable(
            at,
            format!("is {}, not a text or a list of texts", describe(other)),
        )),
    }
}

/// A path of the suite's directory, which must stay in it.
fn suite_path(text: String, at: &str) -> Decoded<String> {
    if stays_below(&text) {
        Ok(text)
    } else {
        Err(refuse(
            SuiteRefusalReason::PathOutsideSuite,
            at,
            "is absolute, empty or leaves the suite's directory",
        ))
    }
}

/// A path of a cycle's workspace or store, which must stay in it.
fn cycle_path(text: String, at: &str) -> Decoded<String> {
    if stays_below(&text) {
        Ok(text)
    } else {
        Err(refuse(
            SuiteRefusalReason::PathOutsideCycle,
            at,
            "is absolute, empty or leaves the cycle's directory",
        ))
    }
}

/// A cycle's or a step's name: a letter or digit, then letters, digits, `-`, `_` or `.`.
fn usable_name(name: &str) -> bool {
    name.len() <= MAX_NAME_BYTES
        && name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The members `defaults` and a cycle share.
const SHARED: &[&str] = &[
    "protocol",
    "bindings",
    "engine",
    "fixtures",
    "revisions",
    "seed",
    "principal",
    "max_rounds",
    "decide",
    "steps",
];

fn decode_defaults(value: &Value, at: &str) -> Decoded<SuiteDefaults> {
    let members = Members::of(value, at)?;
    members.allow(SHARED)?;
    Ok(SuiteDefaults {
        protocol: members.text("protocol")?,
        bindings: members
            .text("bindings")?
            .map(|text| suite_path(text, &members.path("bindings")))
            .transpose()?,
        engine: members.text("engine")?,
        fixtures: members.text("fixtures")?,
        revisions: members
            .get("revisions")
            .map(|value| revisions(value, &members.path("revisions")))
            .transpose()?,
        seed: members
            .get("seed")
            .map(|value| seed(value, &members.path("seed")))
            .transpose()?,
        principal: members.text("principal")?,
        max_rounds: members
            .integer("max_rounds")?
            .map(|rounds| bounded_rounds(rounds, &members.path("max_rounds")))
            .transpose()?,
        decide: members
            .get("decide")
            .map(|value| decisions(value, &members.path("decide")))
            .transpose()?,
        steps: members
            .get("steps")
            .map(|value| steps(value, &members.path("steps")))
            .transpose()?,
    })
}

fn bounded_rounds(rounds: i64, at: &str) -> Decoded<i64> {
    if (1..=MAX_ROUNDS).contains(&rounds) {
        Ok(rounds)
    } else {
        Err(unreadable(
            at,
            format!("is {rounds}, not 1 to {MAX_ROUNDS}"),
        ))
    }
}

fn decode_cycle(value: &Value, at: &str, defaults: Option<&SuiteDefaults>) -> Decoded<SuiteCycle> {
    let members = Members::of(value, at)?;
    let mut declared = vec!["name", "pending", "when", "expect"];
    declared.extend_from_slice(SHARED);
    members.allow(&declared)?;
    let name = members.required_text("name")?;
    if !usable_name(&name) {
        return Err(refuse(
            SuiteRefusalReason::CycleNameUnusable,
            &members.path("name"),
            format!(
                "{name:?} is not a letter or digit then letters, digits, -, _ or ., within {MAX_NAME_BYTES} bytes"
            ),
        ));
    }
    let own = decode_defaults(
        &Value::Object(
            members
                .members
                .iter()
                .filter(|(key, _)| SHARED.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        at,
    )?;
    let fallback = defaults.cloned().unwrap_or(SuiteDefaults {
        protocol: None,
        bindings: None,
        engine: None,
        fixtures: None,
        revisions: None,
        seed: None,
        principal: None,
        max_rounds: None,
        decide: None,
        steps: None,
    });
    let protocol = own.protocol.or(fallback.protocol);
    let bindings = own.bindings.or(fallback.bindings);
    if protocol.is_none() {
        return Err(refuse(
            SuiteRefusalReason::MemberMissing,
            &members.path("protocol"),
            "neither the cycle nor the suite's defaults name the protocol",
        ));
    }
    if bindings.is_none() {
        return Err(refuse(
            SuiteRefusalReason::MemberMissing,
            &members.path("bindings"),
            "neither the cycle nor the suite's defaults name the bindings file",
        ));
    }
    let mut decide = own.decide.unwrap_or_default();
    decide.extend(fallback.decide.unwrap_or_default());
    let steps = own
        .steps
        .or(fallback.steps)
        .unwrap_or_else(|| vec![bare_step(StepKind::Open), bare_step(StepKind::Run)]);
    let expect = match members.get("expect") {
        Some(value) => expectation(value, &members.path("expect"))?,
        None => CycleExpectation {
            exit: None,
            outcome: None,
            stop: None,
            checks: Vec::new(),
        },
    };
    let when = match members.get("when") {
        Some(value) => list(value, &members.path("when"))?
            .iter()
            .enumerate()
            .map(|(index, condition)| {
                cycle_condition(condition, &format!("{}[{index}]", members.path("when")))
            })
            .collect::<Decoded<Vec<_>>>()?,
        None => Vec::new(),
    };
    let cycle = SuiteCycle {
        name,
        pending: members.boolean("pending")?.unwrap_or(false),
        when,
        protocol,
        bindings,
        engine: own.engine.or(fallback.engine),
        fixtures: own.fixtures.or(fallback.fixtures),
        revisions: own.revisions.or(fallback.revisions).unwrap_or_default(),
        seed: own.seed.or(fallback.seed).unwrap_or_default(),
        principal: own.principal.or(fallback.principal),
        max_rounds: own
            .max_rounds
            .or(fallback.max_rounds)
            .unwrap_or(DEFAULT_MAX_ROUNDS),
        decide,
        steps,
        expect,
    };
    references(&cycle, at)?;
    Ok(cycle)
}

fn bare_step(kind: StepKind) -> CycleStep {
    CycleStep {
        r#do: kind,
        name: None,
        at: None,
        exit: None,
        refusal: None,
        engine: None,
        rounds: None,
        checks: Vec::new(),
        case: None,
        capability: None,
        action: None,
        principal: None,
        outcome: None,
        result: None,
        reasons: None,
        deny: None,
        option: None,
    }
}

fn revisions(value: &Value, at: &str) -> Decoded<Vec<ArtifactRevision>> {
    let members = Members::of(value, at)?;
    members
        .members
        .iter()
        .map(|(artifact, revision)| match revision {
            Value::String(revision) => Ok(ArtifactRevision {
                artifact: artifact.clone(),
                revision: revision.clone(),
            }),
            other => Err(unreadable(
                &members.path(artifact),
                format!("is {}, not a revision text", describe(other)),
            )),
        })
        .collect()
}

fn seed(value: &Value, at: &str) -> Decoded<Vec<SeedEntry>> {
    list(value, at)?
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let members = Members::of(entry, &format!("{at}[{index}]"))?;
            members.allow(&["from", "to"])?;
            Ok(SeedEntry {
                from: suite_path(members.required_text("from")?, &members.path("from"))?,
                to: cycle_path(members.required_text("to")?, &members.path("to"))?,
            })
        })
        .collect()
}

fn use_bound(members: &Members<'_>) -> Decoded<UseBound> {
    match members.get("times") {
        None => Ok(UseBound {
            count: Some(1),
            any: false,
        }),
        Some(Value::String(text)) if text == "any" => Ok(UseBound {
            count: None,
            any: true,
        }),
        Some(value) => match value.as_i64() {
            Some(count) if count >= 1 => Ok(UseBound {
                count: Some(count),
                any: false,
            }),
            _ => Err(unreadable(
                &members.path("times"),
                "is neither a count of at least 1 nor any",
            )),
        },
    }
}

fn decisions(value: &Value, at: &str) -> Decoded<Vec<ScriptedDecision>> {
    list(value, at)?
        .iter()
        .enumerate()
        .map(|(index, entry)| decision(entry, &format!("{at}[{index}]")))
        .collect()
}

fn decision(value: &Value, at: &str) -> Decoded<ScriptedDecision> {
    let members = Members::of(value, at)?;
    let take = members.required_text("take")?;
    match take.as_str() {
        "approve" => {
            members.allow(&[
                "take",
                "case",
                "capability",
                "action",
                "principal",
                "result",
                "reasons",
                "deny",
                "times",
                "optional",
            ])?;
            Ok(ScriptedDecision::Approve(ScriptedApproval {
                case: members.text("case")?,
                capability: members.text("capability")?,
                action: members.text("action")?,
                principal: members.text("principal")?,
                result: members.text("result")?,
                reasons: members.texts("reasons")?,
                deny: members.boolean("deny")?,
                times: use_bound(&members)?,
                optional: members.boolean("optional")?.unwrap_or(false),
            }))
        }
        "close" => {
            members.allow(&["take", "case", "outcome", "principal", "times", "optional"])?;
            Ok(ScriptedDecision::Close(ScriptedClose {
                case: members.text("case")?,
                outcome: members.required_text("outcome")?,
                principal: members.text("principal")?,
                times: use_bound(&members)?,
                optional: members.boolean("optional")?.unwrap_or(false),
            }))
        }
        "decide" => {
            members.allow(&["take", "case", "option", "principal", "times", "optional"])?;
            Ok(ScriptedDecision::Decide(ScriptedChoice {
                case: members.text("case")?,
                option: members.text("option")?,
                principal: members.text("principal")?,
                times: use_bound(&members)?,
                optional: members.boolean("optional")?.unwrap_or(false),
            }))
        }
        other => Err(unreadable(
            &members.path("take"),
            format!("is {other:?}, none of approve, close, decide"),
        )),
    }
}

fn step_kind(name: &str) -> Option<StepKind> {
    Some(match name {
        "open" => StepKind::Open,
        "run" => StepKind::Run,
        "approve" => StepKind::Approve,
        "close" => StepKind::Close,
        "decide" => StepKind::Decide,
        "stop" => StepKind::Stop,
        "resume" => StepKind::Resume,
        "allow" => StepKind::Allow,
        "reopen" => StepKind::Reopen,
        "replay" => StepKind::Replay,
        "status" => StepKind::Status,
        "verify" => StepKind::Verify,
        _ => return None,
    })
}

/// The members a step of `kind` declares beside `do`, `name`, `at`, `exit`, `refusal`, `checks`.
fn step_members(kind: StepKind) -> &'static [&'static str] {
    match kind {
        StepKind::Open | StepKind::Status | StepKind::Verify => &[],
        StepKind::Run => &["rounds", "engine"],
        StepKind::Approve => &[
            "case",
            "capability",
            "action",
            "principal",
            "result",
            "reasons",
            "deny",
        ],
        StepKind::Close => &["case", "outcome", "principal"],
        StepKind::Decide => &["case", "option", "principal"],
        StepKind::Stop
        | StepKind::Resume
        | StepKind::Allow
        | StepKind::Reopen
        | StepKind::Replay => &["case", "principal"],
    }
}

fn steps(value: &Value, at: &str) -> Decoded<Vec<CycleStep>> {
    let mut names = BTreeSet::new();
    list(value, at)?
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let step = step(entry, &format!("{at}[{index}]"))?;
            if let Some(name) = &step.name
                && !names.insert(name.clone())
            {
                return Err(refuse(
                    SuiteRefusalReason::StepNameTwice,
                    &format!("{at}[{index}].name"),
                    format!("the step {name:?} is named twice"),
                ));
            }
            Ok(step)
        })
        .collect()
}

fn step(value: &Value, at: &str) -> Decoded<CycleStep> {
    let members = Members::of(value, at)?;
    let kind = members.required_text("do")?;
    let Some(kind) = step_kind(&kind) else {
        return Err(refuse(
            SuiteRefusalReason::StepKindUnknown,
            &members.path("do"),
            format!("{kind:?} is no step kind the format knows"),
        ));
    };
    let mut declared = vec!["do", "name", "at", "exit", "refusal", "checks"];
    declared.extend_from_slice(step_members(kind));
    members.allow(&declared)?;
    let name = members.text("name")?;
    if let Some(name) = &name
        && !usable_name(name)
    {
        return Err(refuse(
            SuiteRefusalReason::CycleNameUnusable,
            &members.path("name"),
            format!(
                "the step name {name:?} is not a letter or digit then letters, digits, -, _ or ."
            ),
        ));
    }
    let at_minutes = members.integer("at")?;
    if at_minutes.is_some_and(|minutes| !(0..=525_600).contains(&minutes)) {
        return Err(unreadable(
            &members.path("at"),
            "is not 0 to 525600 minutes",
        ));
    }
    let rounds = members
        .integer("rounds")?
        .map(|rounds| bounded_rounds(rounds, &members.path("rounds")))
        .transpose()?;
    Ok(CycleStep {
        r#do: kind,
        name,
        at: at_minutes,
        exit: members
            .get("exit")
            .map(|value| exit(value, &members.path("exit")))
            .transpose()?,
        refusal: members.text("refusal")?,
        engine: members.text("engine")?,
        rounds,
        checks: match members.get("checks") {
            Some(value) => checks(value, &members.path("checks"))?,
            None => Vec::new(),
        },
        case: members.text("case")?,
        capability: members.text("capability")?,
        action: members.text("action")?,
        principal: members.text("principal")?,
        outcome: members.text("outcome")?,
        result: members.text("result")?,
        reasons: members.texts("reasons")?,
        deny: members.boolean("deny")?,
        option: members.text("option")?,
    })
}

fn exit(value: &Value, at: &str) -> Decoded<ExitExpectation> {
    if let Some(status) = value.as_i64() {
        return Ok(ExitExpectation {
            status: Some(status),
            not: None,
        });
    }
    let members = Members::of(value, at)?;
    members.allow(&["not"])?;
    match members.integer("not")? {
        Some(not) => Ok(ExitExpectation {
            status: None,
            not: Some(not),
        }),
        None => Err(members.missing("not")),
    }
}

fn tree_stop(name: &str) -> Option<TreeStop> {
    Some(match name {
        "Terminal" => TreeStop::Terminal,
        "AwaitingPerson" => TreeStop::AwaitingPerson,
        "AwaitingAuthority" => TreeStop::AwaitingAuthority,
        "Stuck" => TreeStop::Stuck,
        "BudgetSpent" => TreeStop::BudgetSpent,
        _ => return None,
    })
}

fn expectation(value: &Value, at: &str) -> Decoded<CycleExpectation> {
    let members = Members::of(value, at)?;
    members.allow(&["exit", "outcome", "stop", "checks"])?;
    let stop = match members.get("stop") {
        None => None,
        Some(Value::String(name)) => Some(StopExpectation {
            stop: tree_stop(name).ok_or_else(|| {
                unreadable(&members.path("stop"), format!("{name:?} is no tree stop"))
            })?,
            case: None,
        }),
        Some(value) => {
            let stop = Members::of(value, &members.path("stop"))?;
            stop.allow(&["stop", "case"])?;
            let name = stop.required_text("stop")?;
            Some(StopExpectation {
                stop: tree_stop(&name).ok_or_else(|| {
                    unreadable(&stop.path("stop"), format!("{name:?} is no tree stop"))
                })?,
                case: stop.text("case")?,
            })
        }
    };
    Ok(CycleExpectation {
        exit: members.integer("exit")?,
        outcome: members.text("outcome")?,
        stop,
        checks: match members.get("checks") {
            Some(value) => checks(value, &members.path("checks"))?,
            None => Vec::new(),
        },
    })
}

fn cycle_condition(value: &Value, at: &str) -> Decoded<CycleCondition> {
    let members = Members::of(value, at)?;
    if members.get("protocol").is_some() {
        members.allow(&["protocol", "declares"])?;
        let Some(declares) = members.get("declares") else {
            return Err(members.missing("declares"));
        };
        let declared = Members::of(declares, &members.path("declares"))?;
        declared.allow(&["action", "outcome"])?;
        let name = DeclaredName {
            action: declared.text("action")?,
            outcome: declared.text("outcome")?,
        };
        if name.action.is_some() == name.outcome.is_some() {
            return Err(unreadable(
                &members.path("declares"),
                "names exactly one of action and outcome",
            ));
        }
        return Ok(CycleCondition {
            protocol: members.text("protocol")?,
            declares: Some(name),
            suite: None,
            test: None,
        });
    }
    let Some(file) = members.text("suite")? else {
        return Err(refuse(
            SuiteRefusalReason::MemberMissing,
            at,
            "a condition names a protocol or a file of the suite",
        ));
    };
    let file = suite_path(file, &members.path("suite"))?;
    let path = members.required_text("path")?;
    let tests: Map<String, Value> = members
        .members
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "suite" | "path"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let test = condition(&path, &Value::Object(tests), at, true)?;
    Ok(CycleCondition {
        protocol: None,
        declares: None,
        suite: Some(file),
        test: Some(test),
    })
}

fn check_kind(name: &str) -> Option<CheckKind> {
    Some(match name {
        "exists" => CheckKind::Exists,
        "count" => CheckKind::Count,
        "equals" => CheckKind::Equals,
        "contains" => CheckKind::Contains,
        "lacks" => CheckKind::Lacks,
        "matches" => CheckKind::Matches,
        "not_matches" => CheckKind::NotMatches,
        "all" => CheckKind::All,
        "none" => CheckKind::None,
        "some" => CheckKind::Some,
        "sequence" => CheckKind::Sequence,
        "before" => CheckKind::Before,
        "compare" => CheckKind::Compare,
        _ => return None,
    })
}

/// The members a check of `kind` declares beside `check`, `name` and `select`.
fn check_members(kind: CheckKind) -> &'static [&'static str] {
    match kind {
        CheckKind::Exists | CheckKind::Equals | CheckKind::Contains | CheckKind::Lacks => &["is"],
        CheckKind::Count => &["is", "min", "max"],
        CheckKind::Matches | CheckKind::NotMatches => &["regex"],
        CheckKind::All | CheckKind::None | CheckKind::Some => &["where"],
        CheckKind::Sequence => &["is", "starts", "ends", "within"],
        CheckKind::Before => &["first", "then", "between_none"],
        CheckKind::Compare => &["left", "op", "right"],
    }
}

fn checks(value: &Value, at: &str) -> Decoded<Vec<SuiteCheck>> {
    list(value, at)?
        .iter()
        .enumerate()
        .map(|(index, entry)| check(entry, &format!("{at}[{index}]")))
        .collect()
}

fn values(value: &Value, at: &str) -> Decoded<Vec<loom::json::Value>> {
    Ok(list(value, at)?.iter().map(written).collect())
}

fn check(value: &Value, at: &str) -> Decoded<SuiteCheck> {
    let members = Members::of(value, at)?;
    let name = members.required_text("check")?;
    let Some(kind) = check_kind(&name) else {
        return Err(unreadable(
            &members.path("check"),
            format!("{name:?} is none of the thirteen check kinds"),
        ));
    };
    let mut declared = vec!["check", "name"];
    if kind != CheckKind::Compare {
        declared.push("select");
    }
    declared.extend_from_slice(check_members(kind));
    members.allow(&declared)?;
    let select = if kind == CheckKind::Compare {
        Vec::new()
    } else {
        match members.get("select") {
            Some(value) => selection(value, &members.path("select"))?,
            None => return Err(members.missing("select")),
        }
    };
    let conditions_of = |name: &str| {
        members
            .get(name)
            .map(|value| conditions(value, &members.path(name), true))
            .transpose()
    };
    let lists = |name: &str| {
        members
            .get(name)
            .map(|value| values(value, &members.path(name)))
            .transpose()
    };
    let check = SuiteCheck {
        check: kind,
        name: members.text("name")?,
        select,
        is: members.get("is").map(written),
        min: members.integer("min")?,
        max: members.integer("max")?,
        regex: members
            .text("regex")?
            .map(|regex| usable_regex(regex, &members.path("regex")))
            .transpose()?,
        r#where: conditions_of("where")?,
        starts: lists("starts")?,
        ends: lists("ends")?,
        within: lists("within")?,
        first: conditions_of("first")?,
        then: conditions_of("then")?,
        between_none: conditions_of("between_none")?,
        left: members
            .get("left")
            .map(|value| operand(value, &members.path("left")))
            .transpose()?,
        op: members
            .text("op")?
            .map(|op| compare_op(&op, &members.path("op")))
            .transpose()?,
        right: members
            .get("right")
            .map(|value| operand(value, &members.path("right")))
            .transpose()?,
    };
    required_members(&check, &members)?;
    Ok(check)
}

/// Refuses a check missing what its kind needs.
fn required_members(check: &SuiteCheck, members: &Members<'_>) -> Decoded<()> {
    let need = |present: bool, name: &str| {
        if present {
            Ok(())
        } else {
            Err(members.missing(name))
        }
    };
    match check.check {
        CheckKind::Exists => match &check.is {
            None | Some(loom::json::Value::Bool(_)) => Ok(()),
            Some(_) => Err(unreadable(&members.path("is"), "is not true or false")),
        },
        CheckKind::Count => match &check.is {
            Some(loom::json::Value::Number(_)) if check.min.is_none() && check.max.is_none() => {
                Ok(())
            }
            None if check.min.is_some() || check.max.is_some() => Ok(()),
            _ => Err(unreadable(
                &members.at,
                "a count check gives `is` a count, or `min` and `max`",
            )),
        },
        CheckKind::Equals | CheckKind::Contains | CheckKind::Lacks => {
            need(check.is.is_some(), "is")
        }
        CheckKind::Matches | CheckKind::NotMatches => need(check.regex.is_some(), "regex"),
        CheckKind::All | CheckKind::None | CheckKind::Some => {
            need(check.r#where.is_some(), "where")
        }
        CheckKind::Sequence => {
            let given = [
                check.is.is_some(),
                check.starts.is_some(),
                check.ends.is_some(),
                check.within.is_some(),
            ]
            .iter()
            .filter(|given| **given)
            .count();
            let is_list =
                !matches!(&check.is, Some(value) if !matches!(value, loom::json::Value::Array(_)));
            if given == 1 && is_list {
                Ok(())
            } else {
                Err(unreadable(
                    &members.at,
                    "a sequence check gives exactly one list of is, starts, ends, within",
                ))
            }
        }
        CheckKind::Before => {
            need(check.first.is_some(), "first")?;
            need(check.then.is_some(), "then")
        }
        CheckKind::Compare => {
            need(check.left.is_some(), "left")?;
            need(check.op.is_some(), "op")?;
            need(check.right.is_some(), "right")
        }
    }
}

fn usable_regex(regex: String, at: &str) -> Decoded<String> {
    if regex.len() > MAX_REGEX_BYTES {
        return Err(refuse(
            SuiteRefusalReason::RegexUnusable,
            at,
            format!("is {} bytes, more than {MAX_REGEX_BYTES}", regex.len()),
        ));
    }
    compile_regex(&regex).map_err(|error| refuse(SuiteRefusalReason::RegexUnusable, at, error))?;
    Ok(regex)
}

/// `regex` compiled with the bounds of the suite: the syntax of the Rust `regex` crate, which has
/// no look-around and no back-reference, at most 1 MiB of compiled program.
pub(crate) fn compile_regex(regex: &str) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(regex)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .map_err(|error| {
            let error = error.to_string();
            error
                .lines()
                .last()
                .unwrap_or("is no regex")
                .trim()
                .to_owned()
        })
}

fn compare_op(name: &str, at: &str) -> Decoded<CompareOp> {
    Ok(match name {
        "eq" => CompareOp::Eq,
        "ne" => CompareOp::Ne,
        "lt" => CompareOp::Lt,
        "le" => CompareOp::Le,
        "gt" => CompareOp::Gt,
        "ge" => CompareOp::Ge,
        "subset" => CompareOp::Subset,
        other => {
            return Err(unreadable(
                at,
                format!("{other:?} is none of eq, ne, lt, le, gt, ge, subset"),
            ));
        }
    })
}

fn operand(value: &Value, at: &str) -> Decoded<Operand> {
    let members = Members::of(value, at)?;
    if members.get("value").is_some() {
        members.allow(&["value"])?;
        return Ok(Operand {
            value: members.get("value").map(written),
            select: None,
            take: None,
            plus: None,
            after: None,
        });
    }
    members.allow(&["select", "take", "plus", "after"])?;
    let Some(select) = members.get("select") else {
        return Err(refuse(
            SuiteRefusalReason::MemberMissing,
            at,
            "an operand is a value or a selection",
        ));
    };
    let take = members
        .text("take")?
        .map(|take| {
            Ok(match take.as_str() {
                "value" => OperandTake::Value,
                "count" => OperandTake::Count,
                "size" => OperandTake::Size,
                "digest" => OperandTake::Digest,
                "length" => OperandTake::Length,
                other => {
                    return Err(unreadable(
                        &members.path("take"),
                        format!("{other:?} is none of value, count, size, digest, length"),
                    ));
                }
            })
        })
        .transpose()?;
    Ok(Operand {
        value: None,
        select: Some(selection(select, &members.path("select"))?),
        take,
        plus: members.integer("plus")?,
        after: members.text("after")?,
    })
}

/// The source names a selection may start with.
const SOURCES: &[&str] = &[
    "log", "records", "runs", "cases", "doc", "files", "step", "suite", "requests", "calls",
];

/// The operations a selection applies after its source.
const OPERATIONS: &[&str] = &["where", "pick", "path", "follow", "as"];

fn empty_item() -> SelectItem {
    SelectItem {
        log: None,
        records: None,
        runs: None,
        cases: None,
        doc: None,
        files: None,
        step: None,
        suite: None,
        requests: None,
        calls: None,
        r#where: None,
        pick: None,
        path: None,
        follow: None,
        r#as: None,
    }
}

fn selection(value: &Value, at: &str) -> Decoded<Vec<SelectItem>> {
    let items = list(value, at)?;
    if items.is_empty() {
        return Err(refuse(
            SuiteRefusalReason::SelectUnusable,
            at,
            "a selection starts with exactly one source",
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let at = format!("{at}[{index}]");
            let members = Members::of(item, &at)?;
            if members.members.len() != 1 {
                return Err(refuse(
                    SuiteRefusalReason::SelectUnusable,
                    &at,
                    format!(
                        "an item of a selection has exactly one key, not {}",
                        members.members.len()
                    ),
                ));
            }
            let (key, value) = members.members.iter().next().expect("one key");
            let is_source = SOURCES.contains(&key.as_str());
            let is_operation = OPERATIONS.contains(&key.as_str());
            match (index, is_source, is_operation) {
                (0, false, false) => Err(refuse(
                    SuiteRefusalReason::SourceUnknown,
                    &members.path(key),
                    format!("{key:?} is no source the format knows"),
                )),
                (0, false, true) => Err(refuse(
                    SuiteRefusalReason::SelectUnusable,
                    &members.path(key),
                    "a selection starts with a source, not an operation",
                )),
                (0, true, _) => source(key, value, &members.path(key)),
                (_, true, _) => Err(refuse(
                    SuiteRefusalReason::SelectUnusable,
                    &members.path(key),
                    "a selection holds exactly one source, its first item",
                )),
                (_, false, false) => Err(refuse(
                    SuiteRefusalReason::SelectUnusable,
                    &members.path(key),
                    format!(
                        "{key:?} is no operation; the operations are {}",
                        OPERATIONS.join(", ")
                    ),
                )),
                (_, false, true) => operation(key, value, &members.path(key)),
            }
        })
        .collect()
}

fn case_selector(value: &Value, at: &str) -> Decoded<CaseSelector> {
    let none = CaseSelector {
        path: None,
        protocol: None,
        name: None,
        every: false,
    };
    match value {
        Value::String(text) if text == "*" => Ok(CaseSelector {
            every: true,
            ..none
        }),
        Value::String(text) => {
            let usable = text == "."
                || (!text.is_empty()
                    && text.split('.').all(|segment| {
                        !segment.is_empty()
                            && segment
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                    }));
            if usable {
                Ok(CaseSelector {
                    path: Some(text.clone()),
                    ..none
                })
            } else {
                Err(refuse(
                    SuiteRefusalReason::PathUnusable,
                    at,
                    format!("{text:?} is no case path below the root (`.` is the root)"),
                ))
            }
        }
        other => {
            let members = Members::of(other, at)?;
            members.allow(&["protocol", "name"])?;
            let selector = CaseSelector {
                protocol: members.text("protocol")?,
                name: members.text("name")?,
                ..none
            };
            if selector.protocol.is_some() == selector.name.is_some() {
                return Err(unreadable(at, "a case selector names a protocol or a name"));
            }
            Ok(selector)
        }
    }
}

fn case_of(members: &Members<'_>) -> Decoded<CaseSelector> {
    match members.get("case") {
        Some(value) => case_selector(value, &members.path("case")),
        None => Err(members.missing("case")),
    }
}

fn area(members: &Members<'_>) -> Decoded<CycleArea> {
    match members.required_text("in")?.as_str() {
        "work" => Ok(CycleArea::Work),
        "store" => Ok(CycleArea::Store),
        other => Err(unreadable(
            &members.path("in"),
            format!("{other:?} is neither work nor store"),
        )),
    }
}

fn source(key: &str, value: &Value, at: &str) -> Decoded<SelectItem> {
    let mut item = empty_item();
    match key {
        "log" => {
            let members = Members::of(value, at)?;
            members.allow(&["case", "event"])?;
            let events = members.texts("event")?.unwrap_or_default();
            for (index, event) in events.iter().enumerate() {
                if !known(KindUse::Event, event) {
                    return Err(refuse(
                        SuiteRefusalReason::EventKindUnknown,
                        &format!("{}[{index}]", members.path("event")),
                        format!("{event:?} is no run log event kind the format knows"),
                    ));
                }
            }
            item.log = Some(LogSource {
                case: case_of(&members)?,
                event: events,
            });
        }
        "records" => {
            let members = Members::of(value, at)?;
            members.allow(&["case", "kind"])?;
            item.records = Some(RecordsSource {
                case: case_of(&members)?,
                kind: members.texts("kind")?.unwrap_or_default(),
            });
        }
        "runs" | "requests" | "calls" => {
            let members = Members::of(value, at)?;
            members.allow(&["case"])?;
            let source = Some(CaseSource {
                case: case_of(&members)?,
            });
            match key {
                "runs" => item.runs = source,
                "requests" => item.requests = source,
                _ => item.calls = source,
            }
        }
        "cases" => {
            let members = Members::of(value, at)?;
            members.allow(&["protocol"])?;
            item.cases = Some(CasesSource {
                protocol: members.text("protocol")?,
            });
        }
        "doc" => {
            let members = Members::of(value, at)?;
            members.allow(&["in", "path"])?;
            item.doc = Some(DocSource {
                r#in: area(&members)?,
                path: cycle_path(members.required_text("path")?, &members.path("path"))?,
            });
        }
        "files" => {
            let members = Members::of(value, at)?;
            members.allow(&["in", "glob"])?;
            let glob = members.required_text("glob")?;
            let glob = cycle_path(glob, &members.path("glob"))?;
            if glob.split('/').any(|segment| segment == "..") {
                return Err(refuse(
                    SuiteRefusalReason::PathOutsideCycle,
                    &members.path("glob"),
                    "a pattern holds no `..`",
                ));
            }
            item.files = Some(FilesSource {
                r#in: area(&members)?,
                glob,
            });
        }
        "step" => {
            let Some(name) = value.as_str() else {
                return Err(unreadable(at, "names a step"));
            };
            item.step = Some(name.to_owned());
        }
        "suite" => {
            let Some(file) = value.as_str() else {
                return Err(unreadable(at, "names a file of the suite's directory"));
            };
            item.suite = Some(suite_path(file.to_owned(), at)?);
        }
        _ => unreachable!("a source"),
    }
    Ok(item)
}

fn operation(key: &str, value: &Value, at: &str) -> Decoded<SelectItem> {
    let mut item = empty_item();
    match key {
        "where" => item.r#where = Some(conditions(value, at, true)?),
        "pick" => {
            item.pick = Some(match value {
                Value::String(text) if text == "first" => SelectPick {
                    position: PickPosition::First,
                    index: None,
                },
                Value::String(text) if text == "last" => SelectPick {
                    position: PickPosition::Last,
                    index: None,
                },
                other => match other.as_i64() {
                    Some(index) => SelectPick {
                        position: PickPosition::Index,
                        index: Some(index),
                    },
                    None => return Err(unreadable(at, "is first, last or an index")),
                },
            });
        }
        "path" => {
            let Some(text) = value.as_str() else {
                return Err(refuse(SuiteRefusalReason::PathUnusable, at, "is no text"));
            };
            parse_value_path(text)
                .map_err(|error| refuse(SuiteRefusalReason::PathUnusable, at, error))?;
            item.path = Some(text.to_owned());
        }
        "follow" => {
            item.follow = Some(match value {
                Value::String(text) => Follow {
                    target: match text.as_str() {
                        "blob" => FollowTarget::Blob,
                        "evidence" => FollowTarget::Evidence,
                        "file" => FollowTarget::File,
                        other => {
                            return Err(unreadable(
                                at,
                                format!("{other:?} is none of blob, evidence, file"),
                            ));
                        }
                    },
                    member: None,
                    below: None,
                },
                other => {
                    let members = Members::of(other, at)?;
                    if let Some(member) = members.text("blob")? {
                        members.allow(&["blob"])?;
                        Follow {
                            target: FollowTarget::Blob,
                            member: Some(member),
                            below: None,
                        }
                    } else {
                        members.allow(&["file"])?;
                        let below = members.required_text("file")?;
                        Follow {
                            target: FollowTarget::File,
                            member: None,
                            below: Some(cycle_path(below, &members.path("file"))?),
                        }
                    }
                }
            });
        }
        "as" => {
            item.r#as = Some(match value.as_str() {
                Some("text") => ReadAs::Text,
                Some("lines") => ReadAs::Lines,
                Some("json") => ReadAs::Json,
                Some("jsonl") => ReadAs::Jsonl,
                Some("yaml") => ReadAs::Yaml,
                _ => return Err(unreadable(at, "is none of text, lines, json, jsonl, yaml")),
            });
        }
        _ => unreachable!("an operation"),
    }
    Ok(item)
}

/// The tests a `where`, `first`, `then` or `between_none` gives: a map of paths, and `any_of` at
/// the top level only.
fn conditions(value: &Value, at: &str, top: bool) -> Decoded<Conditions> {
    let members = Members::of(value, at)?;
    let mut all = Vec::new();
    let mut any_of = Vec::new();
    for (path, test) in members.members {
        if path == "any_of" {
            if !top {
                return Err(unreadable(
                    &members.path(path),
                    "any_of stands at one level only",
                ));
            }
            for (index, group) in list(test, &members.path(path))?.iter().enumerate() {
                let group = conditions(group, &format!("{}[{index}]", members.path(path)), false)?;
                any_of.push(ConditionGroup { all: group.all });
            }
            continue;
        }
        all.push(condition(path, test, &members.path(path), false)?);
    }
    Ok(Conditions { all, any_of })
}

/// The tests a condition may give.
const TESTS: &[&str] = &[
    "equals",
    "not_equals",
    "in",
    "not_in",
    "contains",
    "lacks",
    "matches",
    "not_matches",
    "min",
    "max",
    "type",
    "length",
    "exists",
];

/// One condition on `path`: a scalar it equals, or one test. `named` says the test is given as
/// members of the map `test` itself (a `when` entry), never as a scalar.
fn condition(path: &str, test: &Value, at: &str, named: bool) -> Decoded<Condition> {
    parse_value_path(path).map_err(|error| refuse(SuiteRefusalReason::PathUnusable, at, error))?;
    let mut condition = Condition {
        path: path.to_owned(),
        equals: None,
        not_equals: None,
        r#in: None,
        not_in: None,
        contains: None,
        lacks: None,
        matches: None,
        not_matches: None,
        min: None,
        max: None,
        r#type: None,
        length: None,
        exists: None,
    };
    let members = match test {
        Value::Object(_) => Members::of(test, at)?,
        Value::Array(_) => {
            return Err(unreadable(
                at,
                "is a list; a condition is a scalar or one test",
            ));
        }
        scalar if !named => {
            condition.equals = Some(written(scalar));
            return Ok(condition);
        }
        _ => return Err(unreadable(at, "gives one test")),
    };
    members.allow(TESTS)?;
    if members.members.len() != 1 {
        return Err(unreadable(
            at,
            format!("gives exactly one test, not {}", members.members.len()),
        ));
    }
    let (name, value) = members.members.iter().next().expect("one test");
    let here = members.path(name);
    match name.as_str() {
        "equals" => condition.equals = Some(written(value)),
        "not_equals" => condition.not_equals = Some(written(value)),
        "in" => condition.r#in = Some(values(value, &here)?),
        "not_in" => condition.not_in = Some(values(value, &here)?),
        "contains" => condition.contains = Some(written(value)),
        "lacks" => condition.lacks = Some(written(value)),
        "matches" | "not_matches" => {
            let Some(regex) = value.as_str() else {
                return Err(refuse(
                    SuiteRefusalReason::RegexUnusable,
                    &here,
                    "is no text",
                ));
            };
            let regex = usable_regex(regex.to_owned(), &here)?;
            if name == "matches" {
                condition.matches = Some(regex);
            } else {
                condition.not_matches = Some(regex);
            }
        }
        "min" | "max" => {
            if !value.is_number() {
                return Err(unreadable(&here, "is no number"));
            }
            if name == "min" {
                condition.min = Some(written(value));
            } else {
                condition.max = Some(written(value));
            }
        }
        "type" => {
            condition.r#type = Some(match value.as_str() {
                Some("string") => ValueType::String,
                Some("number") => ValueType::Number,
                Some("bool") => ValueType::Bool,
                Some("list") => ValueType::List,
                Some("map") => ValueType::Map,
                Some("null") => ValueType::Null,
                _ => {
                    return Err(unreadable(
                        &here,
                        "is none of string, number, bool, list, map, null",
                    ));
                }
            });
        }
        "length" => {
            let bound = Members::of(value, &here)?;
            bound.allow(&["min", "max"])?;
            condition.length = Some(LengthBound {
                min: bound.integer("min")?,
                max: bound.integer("max")?,
            });
        }
        "exists" => match value {
            Value::Bool(exists) => condition.exists = Some(*exists),
            _ => return Err(unreadable(&here, "is not true or false")),
        },
        _ => unreachable!("a declared test"),
    }
    Ok(condition)
}

/// Refuses an `after` or a `step` source naming no step before it: for a step's checks the step
/// itself or one before it, for `expect` any step of the cycle.
fn references(cycle: &SuiteCycle, at: &str) -> Decoded<()> {
    let names: Vec<Option<&str>> = cycle
        .steps
        .iter()
        .map(|step| step.name.as_deref())
        .collect();
    for (index, step) in cycle.steps.iter().enumerate() {
        let before: Vec<&str> = names[..=index].iter().flatten().copied().collect();
        for (number, check) in step.checks.iter().enumerate() {
            check_references(
                check,
                &before,
                &format!("{at}.steps[{index}].checks[{number}]"),
            )?;
        }
    }
    let every: Vec<&str> = names.iter().flatten().copied().collect();
    for (number, check) in cycle.expect.checks.iter().enumerate() {
        check_references(check, &every, &format!("{at}.expect.checks[{number}]"))?;
    }
    Ok(())
}

fn check_references(check: &SuiteCheck, before: &[&str], at: &str) -> Decoded<()> {
    let unknown = |name: &str, member: String| {
        if before.contains(&name) {
            Ok(())
        } else {
            Err(refuse(
                SuiteRefusalReason::AfterUnknownStep,
                &member,
                format!("{name:?} names no step before it"),
            ))
        }
    };
    let selection = |items: &[SelectItem], member: &str| -> Decoded<()> {
        match items.first().and_then(|item| item.step.as_deref()) {
            Some(name) => unknown(name, format!("{member}[0].step")),
            None => Ok(()),
        }
    };
    selection(&check.select, &format!("{at}.select"))?;
    for (side, operand) in [("left", &check.left), ("right", &check.right)] {
        let Some(operand) = operand else { continue };
        if let Some(items) = &operand.select {
            selection(items, &format!("{at}.{side}.select"))?;
        }
        if let Some(name) = &operand.after {
            unknown(name, format!("{at}.{side}.after"))?;
        }
    }
    Ok(())
}
