//! Commission's ESS specification is a hard gate (Atlas ADR 0076).
//!
//! The specification under `ess/` passes four steps, in order, and the gate fails on the first that
//! does not hold, naming the step and quoting its output:
//!
//! 1. `ess specify validate --path ess --strict-requires` exits 0;
//! 2. `ess specify compile --path ess --format json` exits 0 and prints JSON;
//! 3. `ess verify conform synthesize --path ess --out <scratch>/suite.json` exits 0 and reports 0
//!    refusals;
//! 4. no file under `ess/` contains the open-question marker.
//!
//! `ess` must be on `PATH`. When it cannot be run the gate fails and names it; it never skips.

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, Frontier, FrontierAction, FrontierClaim, FrontierData, FrontierId,
    FrontierObligation, Truth,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

/// The open-question marker no file under `ess/` may carry. An unsettled question is recorded in
/// the planning store as a `decision-blocker` instead.
const MARKER: &str = "UNMAPPED:";

/// The binary the gate runs.
const ESS: &str = "ess";

/// The repository whose `ess/` is checked, read when the test runs rather than when it was built.
///
/// A test binary is reused across worktrees that share one `CARGO_TARGET_DIR`; a path baked in at
/// build time would then check another worktree's specification. Cargo sets `CARGO_MANIFEST_DIR`
/// for every test it runs, so its absence means the binary was started some other way.
fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| {
        panic!(
            "CARGO_MANIFEST_DIR is unset: run this gate through cargo (`task ess-gate`), which \
             names the crate whose repository's ess/ it checks"
        )
    });
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn specification() -> PathBuf {
    root().join("ess")
}

/// A fresh directory under `CARGO_TARGET_TMPDIR`, removed when dropped.
///
/// `CARGO_TARGET_TMPDIR` is fixed at build time (cargo sets it only then). That is safe here: it
/// decides where scratch goes, never what is checked, it lies in the target directory the binary
/// itself was built in, and each directory is unique to its process and moment.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(purpose: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ess-gate-{purpose}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir)
            .unwrap_or_else(|error| panic!("create {}: {error}", dir.display()));
        Self { dir }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Runs `ess` with `args` in `cwd`. Fails, naming the step and the binary, when `ess` cannot run.
fn run_ess(step: &str, args: &[&str], cwd: &Path) -> Result<Output, String> {
    Command::new(ESS)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| {
            format!(
                "{step}: cannot run `{ESS}` ({error}); the gate needs ess 0.52.0 on PATH \
                 (ess/ess-inputs.yaml)"
            )
        })
}

/// The command line and both streams of one run, for a failure message.
fn quote(args: &[&str], output: &Output) -> String {
    format!(
        "`{ESS} {}` ({})\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Step 1: the specification is valid, refusing an `ess` older than `ess-inputs.yaml` requires.
fn validate(spec: &Path, cwd: &Path) -> Result<(), String> {
    const STEP: &str = "step 1 (validate)";
    let spec = spec.to_string_lossy();
    let args = ["specify", "validate", "--path", &spec, "--strict-requires"];
    let output = run_ess(STEP, &args, cwd)?;
    if !output.status.success() {
        return Err(format!("{STEP} failed: {}", quote(&args, &output)));
    }
    Ok(())
}

/// Step 2: the specification compiles, and the compiled model is JSON.
fn compile(spec: &Path, cwd: &Path) -> Result<Value, String> {
    const STEP: &str = "step 2 (compile)";
    let spec = spec.to_string_lossy();
    let args = ["specify", "compile", "--path", &spec, "--format", "json"];
    let output = run_ess(STEP, &args, cwd)?;
    if !output.status.success() {
        return Err(format!("{STEP} failed: {}", quote(&args, &output)));
    }
    let text = String::from_utf8(output.stdout.clone())
        .map_err(|error| format!("{STEP}: output is not UTF-8 ({error})"))?;
    json::parse(&text).map_err(|error| {
        format!(
            "{STEP}: output does not parse as JSON ({error}): {}",
            quote(&args, &output)
        )
    })
}

/// Step 3: the conformance suite synthesizes with no refusal.
fn synthesize(spec: &Path, cwd: &Path) -> Result<(), String> {
    const STEP: &str = "step 3 (synthesize)";
    let suite = cwd.join("suite.json");
    let suite_arg = suite.to_string_lossy().into_owned();
    let spec = spec.to_string_lossy();
    let args = [
        "verify",
        "conform",
        "synthesize",
        "--path",
        &spec,
        "--out",
        &suite_arg,
    ];
    let output = run_ess(STEP, &args, cwd)?;
    if !output.status.success() {
        return Err(format!("{STEP} failed: {}", quote(&args, &output)));
    }
    let report = String::from_utf8_lossy(&output.stdout);
    match refusals(&report) {
        Some(0) => {}
        Some(count) => {
            return Err(format!(
                "{STEP}: {count} refusal(s): {}",
                quote(&args, &output)
            ));
        }
        None => {
            return Err(format!(
                "{STEP}: the report states no refusal count: {}",
                quote(&args, &output)
            ));
        }
    }
    if !suite.is_file() {
        return Err(format!(
            "{STEP}: no suite written to {}: {}",
            suite.display(),
            quote(&args, &output)
        ));
    }
    Ok(())
}

/// The refusal count of a synthesize report, or `None` when it has no summary line.
///
/// Only the summary line counts: the last line of the shape
/// `<n> scenario(s) (<n> authored), <n> refusal(s), …`. A `refused:` detail line before it quotes
/// the author's own text, which may say anything — `0 refusal(s)` included.
fn refusals(report: &str) -> Option<u64> {
    report.lines().rev().find_map(summary_refusals)
}

/// The refusal count of one line, if it is a synthesize summary line.
fn summary_refusals(line: &str) -> Option<u64> {
    let (scenarios, rest) = line.split_once(" scenario(s) (")?;
    scenarios.parse::<u64>().ok()?;
    let (authored, rest) = rest.split_once(" authored), ")?;
    authored.parse::<u64>().ok()?;
    let (refused, _) = rest.split_once(" refusal(s)")?;
    refused.parse().ok()
}

/// Every line under `dir` that carries the marker, as `<file>:<line>: <text>`, in path order.
fn markers(dir: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    collect_files(dir, &mut files).map_err(|error| format!("read {}: {error}", dir.display()))?;
    files.sort();
    let mut found = Vec::new();
    for file in files {
        let bytes = fs::read(&file).map_err(|error| format!("read {}: {error}", file.display()))?;
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.contains(MARKER) {
                found.push(format!("{}:{}: {}", file.display(), index + 1, line.trim()));
            }
        }
    }
    Ok(found)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// Step 4: the specification carries no open question.
fn scan(spec: &Path) -> Result<(), String> {
    let found = markers(spec)?;
    if found.is_empty() {
        return Ok(());
    }
    Err(format!(
        "step 4 (no {MARKER}): {} line(s) carry an open question; record it in the planning store \
         as a decision-blocker instead:\n{}",
        found.len(),
        found.join("\n")
    ))
}

/// The four steps, in order, stopping at the first that does not hold.
fn gate(spec: &Path, scratch: &Path) -> Result<(), String> {
    validate(spec, scratch)?;
    compile(spec, scratch)?;
    synthesize(spec, scratch)?;
    scan(spec)
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
    for entry in
        fs::read_dir(from).unwrap_or_else(|error| panic!("read {}: {error}", from.display()))
    {
        let entry = entry.unwrap_or_else(|error| panic!("read {}: {error}", from.display()));
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap_or_else(|error| {
                panic!(
                    "copy {} to {}: {error}",
                    entry.path().display(),
                    target.display()
                )
            });
        }
    }
}

/// Expectations 1–4: the specification under `ess/` passes the gate.
#[test]
fn ess_gate() {
    let scratch = Scratch::new("spec");
    if let Err(failure) = gate(&specification(), &scratch.dir) {
        panic!("ESS gate failed at {failure}");
    }
}

/// Expectation 4, stated on its own: the header of `ess/domains/responsibility.yaml` included,
/// no file under `ess/` carries the marker.
#[test]
fn specification_carries_no_open_question() {
    let found = markers(&specification()).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        found.is_empty(),
        "open questions under ess/:\n{}",
        found.join("\n")
    );
}

/// Expectation 5: a marker added to a copy of `ess/` fails the gate, which names its file and line.
#[test]
fn gate_names_an_added_marker() {
    let scratch = Scratch::new("probe");
    let copy = scratch.dir.join("ess");
    copy_tree(&specification(), &copy);
    let domain = copy.join("domains/responsibility.yaml");
    let mut text = fs::read_to_string(&domain)
        .unwrap_or_else(|error| panic!("read {}: {error}", domain.display()));
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# UNMAPPED: probe\n");
    fs::write(&domain, &text).unwrap_or_else(|error| panic!("write {}: {error}", domain.display()));
    let line = text.lines().count();

    let failure = match gate(&copy, &scratch.dir) {
        Ok(()) => panic!("the gate passed a specification carrying `# UNMAPPED: probe`"),
        Err(failure) => failure,
    };
    let named = format!("{}:{line}: # UNMAPPED: probe", domain.display());
    assert!(
        failure.starts_with("step 4 ") && failure.contains(&named),
        "the gate did not fail at the scan naming `{named}`:\n{failure}"
    );
}

/// An invariant on Frontier that no view publishes, so synthesis cannot witness it.
const UNWITNESSED_INVARIANT: &str = "    invariants:\n      - case_revision >= 0\n";

/// A command creating a Frontier, which obliges the scenario the invariant above refuses: an item
/// of the domain's `commands:` list.
const ISSUING_COMMAND: &str = "  - name: commission.responsibility.IssueFrontier
    input:
      - name: case_id
        type: commission.responsibility.CaseId
      - name: case_revision
        type: Integer
    outcomes:
      - name: issued
        creates: commission.responsibility.Frontier
        instance: frontier_id
        emits: [commission.responsibility.FrontierIssued]
        payload:
          commission.responsibility.FrontierIssued:
            frontier_id: {generated: true}
        sets:
          case_id: input.case_id
          case_revision: input.case_revision
";

/// The event [`ISSUING_COMMAND`] emits: an item of the domain's `events:` list.
const ISSUED_EVENT: &str = "  - name: commission.responsibility.FrontierIssued
    fields:
      - name: frontier_id
        type: commission.responsibility.FrontierId
";

/// `text` with `item` added to its top-level `section` list, opening the section at the end when
/// the domain has none. A second top-level key of the same name would not be YAML ess accepts.
fn add_to_section(text: &str, section: &str, item: &str) -> String {
    let header = format!("\n{section}:\n");
    match text.matches(&header).count() {
        0 => format!("{text}{header}{item}"),
        1 => text.replacen(&header, &format!("{header}{item}"), 1),
        n => panic!("the domain opens `{section}:` {n} times"),
    }
}

/// Expectation 3: ess 0.52.0 exits 0 while it refuses a scenario, so the count is what fails the
/// gate. A copy of `ess/` whose synthesis refuses one scenario (ESS-SYNTH-011) fails at step 3,
/// naming the count.
#[test]
fn gate_fails_on_a_refusal_ess_exits_zero_for() {
    let scratch = Scratch::new("refusal");
    let copy = scratch.dir.join("ess");
    copy_tree(&specification(), &copy);
    let domain = copy.join("domains/responsibility.yaml");
    let text = fs::read_to_string(&domain)
        .unwrap_or_else(|error| panic!("read {}: {error}", domain.display()));
    let lifecycle_end = "      terminal: [Issued]\n";
    assert_eq!(
        text.matches(lifecycle_end).count(),
        1,
        "{}: expected exactly one Frontier lifecycle ending in `[Issued]`",
        domain.display()
    );
    let text = text.replacen(
        lifecycle_end,
        &format!("{lifecycle_end}{UNWITNESSED_INVARIANT}"),
        1,
    );
    let text = add_to_section(&text, "commands", ISSUING_COMMAND);
    let text = add_to_section(&text, "events", ISSUED_EVENT);
    fs::write(&domain, &text).unwrap_or_else(|error| panic!("write {}: {error}", domain.display()));

    let failure = match gate(&copy, &scratch.dir) {
        Ok(()) => panic!("the gate passed a specification whose synthesis refuses a scenario"),
        Err(failure) => failure,
    };
    assert!(
        failure.starts_with("step 3 (synthesize): 1 refusal(s): ")
            && failure.contains("ESS-SYNTH-011")
            && failure.contains("exit status: 0"),
        "the gate did not fail at step 3 on a counted refusal ess exits 0 for:\n{failure}"
    );
}

/// Expectation 1: `--strict-requires`. A copy of `ess/` that requires an older ess validates
/// without the flag, so only the flag fails it, and the gate fails at step 1.
#[test]
fn gate_fails_at_step_1_on_an_older_required_ess() {
    let scratch = Scratch::new("older-requires");
    let copy = scratch.dir.join("ess");
    copy_tree(&specification(), &copy);
    let inputs = copy.join("ess-inputs.yaml");
    let text = fs::read_to_string(&inputs)
        .unwrap_or_else(|error| panic!("read {}: {error}", inputs.display()));
    assert_eq!(
        text.matches("requires: ess 0.52.0").count(),
        1,
        "{}: expected exactly one `requires: ess 0.52.0`",
        inputs.display()
    );
    fs::write(
        &inputs,
        text.replace("requires: ess 0.52.0", "requires: ess 0.51.0"),
    )
    .unwrap_or_else(|error| panic!("write {}: {error}", inputs.display()));

    let failure = match gate(&copy, &scratch.dir) {
        Ok(()) => panic!("the gate passed a specification that requires ess 0.51.0"),
        Err(failure) => failure,
    };
    assert!(
        failure.starts_with("step 1 (validate) failed: ") && failure.contains("--strict-requires"),
        "the gate did not fail at step 1 on `requires: ess 0.51.0`:\n{failure}"
    );
}

/// Expectation 3: the count comes from the summary line alone. A `refused:` detail line quoting
/// author text that reads `0 refusal(s)`, or even a whole summary-shaped phrase, does not count.
#[test]
fn refusal_count_ignores_quoted_text_before_the_summary() {
    let report = "refused: refusal[ESS-SYNTH-011]: entity commission.responsibility.Frontier has \
                  no scenario `x`\n  `case_id != \"see 0 refusal(s) here\"` reads what no view \
                  publishes\n  `0 scenario(s) (0 authored), 0 refusal(s), quoted` is not it\n\
                  1 scenario(s) (0 authored), 1 refusal(s), written to /x/suite.json\n";
    assert_eq!(refusals(report), Some(1));
    assert_eq!(refusals("see 0 refusal(s) here\n"), None);
}

fn member<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |at, name| {
        at.member(name)
            .unwrap_or_else(|| panic!("compiled model has no `{}`", path.join(".")))
    })
}

fn text(value: &Value) -> &str {
    match value {
        Value::Text(text) => text,
        other => panic!("expected a string, found {}", other.describes()),
    }
}

/// Expectation 6: the compiled Frontier holds lists of Commission's own value types, and
/// `ActionStatus` is an enum of exactly three variants.
#[test]
fn compiled_frontier_holds_commission_value_types() {
    let scratch = Scratch::new("compile");
    let model =
        compile(&specification(), &scratch.dir).unwrap_or_else(|failure| panic!("{failure}"));

    let frontier = member(&model, &["entities", "commission.responsibility.Frontier"]);
    let Value::Array(fields) = member(frontier, &["fields"]) else {
        panic!("Frontier.fields is not an array");
    };
    for (field, element) in [
        ("claims", "commission.responsibility.FrontierClaim"),
        (
            "obligations",
            "commission.responsibility.FrontierObligation",
        ),
        ("actions", "commission.responsibility.FrontierAction"),
    ] {
        let declared = fields
            .iter()
            .find(|candidate| candidate.member("name").map(text) == Some(field))
            .unwrap_or_else(|| panic!("Frontier has no field `{field}`"));
        let type_ref = member(declared, &["type_ref"]);
        assert_eq!(
            text(member(type_ref, &["kind"])),
            "list",
            "Frontier.{field}"
        );
        assert_eq!(
            text(member(type_ref, &["of", "kind"])),
            "declared",
            "Frontier.{field}"
        );
        assert_eq!(
            text(member(type_ref, &["of", "name"])),
            element,
            "Frontier.{field}"
        );
    }

    let status = member(&model, &["types", "commission.responsibility.ActionStatus"]);
    assert_eq!(text(member(status, &["body", "kind"])), "enum");
    let Value::Array(variants) = member(status, &["body", "variants"]) else {
        panic!("ActionStatus.body.variants is not an array");
    };
    let variants: Vec<&str> = variants.iter().map(text).collect();
    assert_eq!(variants, ["Admissible", "ApprovalRequired", "Blocked"]);
}

/// Expectation 7: a Frontier holding one claim, one obligation and one action that needs approval
/// is built from the generated model `b10x-commission` re-exports.
#[test]
fn frontier_built_from_the_reexported_model() {
    let claim = FrontierClaim {
        claim: "tests.pass".into(),
        value: Truth::Unknown,
    };
    let obligation = FrontierObligation {
        obligation: "verify.tests".into(),
        open: true,
    };
    let action = FrontierAction {
        action: "repository.merge".into(),
        status: ActionStatus::ApprovalRequired,
        capability: Some("repository.write".into()),
        reasons: vec!["claim tests.pass is unknown".into()],
    };
    let data = FrontierData {
        frontier_id: FrontierId(Uuid("3d0f6a1e-8b2c-4d5e-9f60-718293a4b5c6".into())),
        case_id: CaseId("case-1".into()),
        case_revision: 17,
        claims: vec![claim.clone()],
        obligations: vec![obligation.clone()],
        actions: vec![action.clone()],
    };

    let frontier = Frontier::new(data.clone());

    assert_eq!(frontier.data(), &data);
    assert_eq!(frontier.data().claims, [claim]);
    assert_eq!(frontier.data().obligations, [obligation]);
    assert_eq!(frontier.data().actions, [action]);
    let held = &frontier.data().actions[0];
    assert_eq!(held.status, ActionStatus::ApprovalRequired);
    assert_eq!(held.capability.as_deref(), Some("repository.write"));
}

/// The lines of the block that opens with `header` (at `indent` spaces), up to the next line at
/// that indent or less.
fn block<'a>(body: &'a str, header: &str, indent: usize) -> Vec<&'a str> {
    let mut lines = body.lines();
    let opening = format!("{}{header}", " ".repeat(indent));
    if !lines.any(|line| line.trim_end() == opening) {
        return Vec::new();
    }
    lines
        .take_while(|line| line.trim().is_empty() || line.len() - line.trim_start().len() > indent)
        .collect()
}

/// Expectation 8: `task check` runs the gate through `ess-gate`, and AGENTS.md § ESS names
/// ADR 0076.
#[test]
fn check_runs_the_gate_and_agents_states_it() {
    let taskfile_path = root().join("Taskfile.yml");
    let taskfile = fs::read_to_string(&taskfile_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", taskfile_path.display()));
    let check = block(&taskfile, "check:", 2);
    assert!(
        check.iter().any(|line| line.trim() == "- task: ess-gate"),
        "Taskfile.yml: task `check` does not list `ess-gate` as a step:\n{}",
        check.join("\n")
    );
    let ess_gate = block(&taskfile, "ess-gate:", 2);
    let commands: Vec<&str> = ess_gate
        .iter()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("- "))
        .collect();
    assert_eq!(
        commands,
        ["- cargo test --locked -p b10x-commission --test ess_gate"],
        "Taskfile.yml: task `ess-gate` must run exactly the whole ess_gate test, with no filter:\n{}",
        ess_gate.join("\n")
    );

    let agents_path = root().join("AGENTS.md");
    let agents = fs::read_to_string(&agents_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", agents_path.display()));
    let section: Vec<&str> = agents
        .lines()
        .skip_while(|line| line.trim_end() != "## ESS")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .collect();
    let section = section
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "ADR 0076",
        "ess-gate",
        "decision-blocker",
        "No story is implemented while the gate is red, whether or not it edits `ess/`",
        "the `UNMAPPED:` scan is removed when the ESS release that refuses open entries \
         (beyond10x/ess `epic:typed-open-questions`) is pinned",
    ] {
        assert!(
            section.contains(required),
            "AGENTS.md § ESS does not name `{required}`:\n{section}"
        );
    }
}

// ---- adversary pass 1 (wave 2026-10-04-w2): cases that pin the scan and the refusal count ----
// They sit inside ess_gate.rs because `markers` and `refusals` are private to that test binary.

/// ADR 0076 item 4: "fails on any `UNMAPPED:` string under `ess/`" — at any depth, in a hidden
/// file, in a file that is not YAML, and in the middle of a line (the base header carried it
/// mid-line: "is marked UNMAPPED: and stays").
#[test]
fn adversary_scan_finds_nested_hidden_non_yaml_and_mid_line_markers() {
    let scratch = Scratch::new("adversary-scan");
    let dir = scratch.dir.join("ess");
    let nested = dir.join("domains/deep/er");
    fs::create_dir_all(&nested).unwrap();
    fs::write(dir.join("system.yaml"), "format: ess/20\n").unwrap();
    fs::write(
        nested.join("notes.md"),
        "one\ntwo is marked UNMAPPED: mid\n",
    )
    .unwrap();
    fs::write(dir.join(".open"), "UNMAPPED: hidden\n").unwrap();
    fs::write(dir.join("domains/x.yml"), "a: b # see UNMAPPED: trailing\n").unwrap();

    let found = markers(&dir).unwrap_or_else(|error| panic!("{error}"));
    let expected = [
        format!("{}:1: UNMAPPED: hidden", dir.join(".open").display()),
        format!(
            "{}:2: two is marked UNMAPPED: mid",
            nested.join("notes.md").display()
        ),
        format!(
            "{}:1: a: b # see UNMAPPED: trailing",
            dir.join("domains/x.yml").display()
        ),
    ];
    for want in &expected {
        assert!(
            found.contains(want),
            "scan missed `{want}`; found:\n{}",
            found.join("\n")
        );
    }
    assert_eq!(found.len(), 3, "found:\n{}", found.join("\n"));
    assert!(scan(&dir).unwrap_err().starts_with("step 4 "));
}

/// Expectation 3: the count is read from ess 0.52.0's report line, and a nonzero count is not 0.
#[test]
fn adversary_refusal_count_is_read_from_the_report() {
    assert_eq!(
        refusals("0 scenario(s) (0 authored), 0 refusal(s), written to /x/suite.json\n"),
        Some(0)
    );
    assert_eq!(
        refusals(
            "refused: some construct\nrefused: another\n3 scenario(s) (0 authored), 2 refusal(s), \
             written to /x/suite.json\n"
        ),
        Some(2)
    );
    assert_eq!(refusals("3 scenario(s) (0 authored), written\n"), None);
}
