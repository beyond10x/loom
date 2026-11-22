//! The ESS hard gate (Atlas ADR 0076): the specification under `ess/` validates with
//! `--strict-requires`, compiles, synthesizes its conformance suite (`--suite-format 5`) with 0
//! refusals not covered by an authored scenario (`covers:`), and carries no open question (no
//! `UNMAPPED:` string in any file under `ess/`).
//!
//! The gate is a function over a directory, so it runs on `ess/` and on temporary copies of it.
//! Everything it writes lands under `CARGO_TARGET_TMPDIR`. It uses only the standard library and
//! the `ess` binary on `PATH`. The repository is read from `CARGO_MANIFEST_DIR` at run time.
//!
//! Step 3 over `ess/` synthesizes nothing: it reads the suite `task suite-generate` committed under
//! `generated/ess/loom/`, after the digest of its inputs matched the tree's
//! (`tests/support/ess_suite.rs`). Synthesizing `ess/` takes minutes; `task slow` does it and
//! compares bytes. The copies the probes change synthesize, once per distinct input for every
//! process that shares the cache ([`synthesis_cache`]), and leave out the decision domain
//! ([`drop_decision`]), which costs minutes and none of them is about; the one probe that needs
//! it is `slow_`, run by `task slow`, not by `task ess-gate`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Output;

#[path = "support/ess_suite.rs"]
mod ess_suite;

const MARKER: &str = "UNMAPPED:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Validate,
    Compile,
    Synthesize,
    MarkerScan,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Step::Validate => "validate (ess specify validate --strict-requires)",
            Step::Compile => "compile (ess specify compile --format json)",
            Step::Synthesize => {
                "synthesize (ess verify conform synthesize --suite-format 5, 0 refusals not covered)"
            }
            Step::MarkerScan => "marker scan (no UNMAPPED: under ess/)",
        })
    }
}

#[derive(Debug)]
struct GateFailure {
    step: Step,
    detail: String,
    markers: Vec<Marker>,
}

impl fmt::Display for GateFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ESS gate failed at step {}: {}", self.step, self.detail)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Marker {
    /// Path relative to the scanned directory, `/`-separated.
    file: String,
    /// One-based line number.
    line: usize,
}

impl fmt::Display for Marker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)
    }
}

/// Where step 3 takes its synthesis from.
#[derive(Debug, Clone, Copy)]
enum Synthesis<'a> {
    /// The synthesis committed under the given repository root, whose `ess/` the gate checks:
    /// read after its input digest matched that `ess/` (`ess_suite::committed`).
    Committed(&'a Path),
    /// A synthesis of the checked directory, made once per distinct input ([`synthesis_cache`]).
    Fresh,
}

/// The directory every process of this tree's tests keeps its syntheses of copies in, keyed by
/// input: `LOOM_ESS_SYNTHESIS_CACHE` when set (a replica's suite is handed its outer test's), else
/// `ess-synthesis-cache` under `CARGO_TARGET_TMPDIR`.
fn synthesis_cache() -> PathBuf {
    std::env::var_os("LOOM_ESS_SYNTHESIS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_TARGET_TMPDIR")).join("ess-synthesis-cache"))
}

/// Runs the four gate steps over `dir` in order and stops at the first that does not hold. On
/// success returns the compiled specification.
fn run_gate(dir: &Path, synthesis: Synthesis<'_>) -> Result<Json, GateFailure> {
    let fail = |step, detail: String| GateFailure {
        step,
        detail,
        markers: Vec::new(),
    };

    let validate = ess(&[
        "specify",
        "validate",
        "--path",
        path_arg(dir),
        "--strict-requires",
    ]);
    if !validate.status.success() {
        return Err(fail(Step::Validate, describe(&validate)));
    }

    let compiled = compile(dir)?;

    let (success, stdout, described) = match synthesis {
        Synthesis::Committed(root) => {
            let (record, _) = ess_suite::committed(root, &ess_program())
                .map_err(|detail| fail(Step::Synthesize, detail))?;
            (record.success(), record.stdout.clone(), record.describe())
        }
        Synthesis::Fresh => {
            let run = ess_suite::synthesize_once(&ess_program(), dir, &synthesis_cache())
                .map_err(|detail| fail(Step::Synthesize, detail))?;
            (run.success(), run.stdout.clone(), run.describe())
        }
    };
    synthesize_holds(success, &stdout, &described)
        .map_err(|detail| fail(Step::Synthesize, detail))?;

    let markers = scan_markers(dir);
    if !markers.is_empty() {
        let list: Vec<String> = markers.iter().map(Marker::to_string).collect();
        return Err(GateFailure {
            step: Step::MarkerScan,
            detail: format!("open question marked {MARKER} at {}", list.join(", ")),
            markers,
        });
    }

    Ok(compiled)
}

/// Gate step 2: `ess specify compile --format json`, parsed.
fn compile(dir: &Path) -> Result<Json, GateFailure> {
    let fail = |detail: String| GateFailure {
        step: Step::Compile,
        detail,
        markers: Vec::new(),
    };
    let output = ess(&[
        "specify",
        "compile",
        "--path",
        path_arg(dir),
        "--format",
        "json",
    ]);
    if !output.status.success() {
        return Err(fail(describe(&output)));
    }
    let text =
        String::from_utf8(output.stdout).map_err(|e| fail(format!("output is not UTF-8: {e}")))?;
    Json::parse(&text).map_err(|e| fail(format!("output is not JSON: {e}")))
}

/// Every file under `dir`, every line holding the marker string.
fn scan_markers(dir: &Path) -> Vec<Marker> {
    let mut files = Vec::new();
    collect_files(dir, &mut files);
    files.sort();
    let mut markers = Vec::new();
    for file in files {
        let bytes = fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let text = String::from_utf8_lossy(&bytes);
        let relative = file
            .strip_prefix(dir)
            .expect("file lies under the scanned directory")
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        for (index, line) in text.lines().enumerate() {
            if line.contains(MARKER) {
                markers.push(Marker {
                    file: relative.clone(),
                    line: index + 1,
                });
            }
        }
    }
    markers
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// The two counts of a `--suite-format 5` summary line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Refusals {
    occurrences: u64,
    covered: u64,
}

/// The counts from the synthesize summary line, `… N refusal occurrence(s), M covered by
/// authored`. ess 0.96.0 leaves `, M covered by authored` out when M is 0
/// (`crates/edge/ess-cli/src/coverage.rs:130-134`), so a line without it reads as 0 covered. The
/// summary is the last line carrying an occurrence count; `refused:` lines before it are not read.
fn refusals(stdout: &str) -> Option<Refusals> {
    let count_before = |text: &str, marker: &str| -> Option<u64> {
        let before = &text[..text.find(marker)?];
        let digits = before.rsplit(|c: char| !c.is_ascii_digit()).next()?;
        digits.parse().ok()
    };
    stdout.lines().rev().find_map(|line| {
        let occurrences = count_before(line, " refusal occurrence(s)")?;
        let covered = match line.find(" covered by authored") {
            Some(_) => count_before(line, " covered by authored")?,
            None => 0,
        };
        Some(Refusals {
            occurrences,
            covered,
        })
    })
}

/// Gate step 3 over what synthesize returned: it exited 0, its summary line carries the counts,
/// and every refusal occurrence is covered by an authored scenario. `described` is the run as a
/// failure prints it.
fn synthesize_holds(success: bool, stdout: &str, described: &str) -> Result<(), String> {
    if !success {
        return Err(described.to_owned());
    }
    match refusals(stdout) {
        Some(Refusals {
            occurrences,
            covered,
        }) if covered >= occurrences => Ok(()),
        Some(Refusals {
            occurrences,
            covered,
        }) => Err(format!(
            "{} refusal(s) not covered by an authored scenario: {described}",
            occurrences - covered
        )),
        None => Err(format!("no refusal count in the output: {described}")),
    }
}

/// The `ess` on `PATH`, asked for the release this repository's `ess/ess-inputs.yaml` requires
/// unless the caller set `ESS_TOOLCHAIN` (`ess_suite::Ess`). A copy's own `requires` line never
/// chooses the release: step 1 holds it to this one.
fn ess_program() -> ess_suite::Ess {
    ess_suite::ess_program(&repo_root())
}

fn ess(args: &[&str]) -> Output {
    ess_program()
        .command()
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("run `ess {}` (is ess on PATH?): {e}", args.join(" ")))
}

fn describe(output: &Output) -> String {
    format!(
        "{}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn path_arg(path: &Path) -> &str {
    path.to_str().expect("UTF-8 path")
}

/// The repository the test runs in, read at run time. Worktrees that share a build directory can
/// run a test binary another worktree compiled, so a path baked in with `env!` would name that
/// other tree.
fn repo_root() -> PathBuf {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").expect(
        "CARGO_MANIFEST_DIR is unset: run the ESS gate through `cargo test`, which sets it to the \
         crate the test runs in",
    );
    Path::new(&manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn spec_dir() -> PathBuf {
    repo_root().join("ess")
}

/// A fresh directory under `CARGO_TARGET_TMPDIR`, unique to the calling test and to the
/// repository it runs in. `CARGO_TARGET_TMPDIR` is set only at compile time; it names the build
/// directory, not a source tree, so it holds for every tree sharing that directory. The key keeps
/// two such trees from clearing each other's scratch.
fn scratch(name: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    repo_root().hash(&mut hasher);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("ess_gate")
        .join(format!("{:016x}", hasher.finish()))
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    fs::create_dir_all(&dir).expect("create scratch");
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copy");
    for entry in fs::read_dir(from).expect("read source") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// `ess_suite::drop_decision`: the copy without the decision domain.
fn drop_decision(spec: &Path) {
    ess_suite::drop_decision(spec).unwrap_or_else(|detail| panic!("{detail}"));
}

/// A copy of `ess/` without the decision domain ([`drop_decision`]) in a fresh scratch directory.
fn copy_spec(name: &str) -> PathBuf {
    let copy = scratch(name).join("ess");
    copy_dir(&spec_dir(), &copy);
    drop_decision(&copy);
    copy
}

/// Applies `edit` to the text of `file` under `spec`, asserting it changes it.
fn edit_file(spec: &Path, file: &str, edit: impl FnOnce(String) -> String) {
    let path = spec.join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file} copy: {e}"));
    let edited = edit(text.clone());
    assert_ne!(edited, text, "the edit to {file} applies");
    fs::write(&path, edited).unwrap_or_else(|e| panic!("write {file} copy: {e}"));
}

/// A copy of `ess/` ([`copy_spec`]) with `# UNMAPPED: probe` appended to `domains/run.yaml`.
/// Returns the copy and the appended line's one-based number.
fn copy_with_probe_marker(name: &str) -> (PathBuf, usize) {
    let copy = copy_spec(name);
    let run = copy.join("domains/run.yaml");
    let mut text = fs::read_to_string(&run).expect("read run.yaml copy");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# UNMAPPED: probe\n");
    let line = text.lines().count();
    fs::write(&run, text).expect("write run.yaml copy");
    (copy, line)
}

// Item 3: the whole gate over a copy that synthesize refuses fails at step 3, after steps 1 and 2
// held. The copy lists no authored scenarios (`scenarios: []`): ess 0.96.0 validates and compiles
// it, and `ess verify conform synthesize` refuses an explicit list that selects no file and exits
// non-zero. (Up to ess 0.77.0 this test retyped the confidence to `Optional<Binary64>`, which the
// suite refused as `UnsupportedPrimitive`; 0.82.0 synthesizes `Binary64`.)
#[test]
fn gate_over_a_copy_synthesize_refuses_fails_at_synthesize() {
    let copy = copy_spec("gate_refused_copy");
    let inputs = copy.join("ess-inputs.yaml");
    let listed = fs::read_to_string(&inputs).expect("read ess-inputs.yaml copy");
    let at = listed
        .find("\nscenarios:")
        .expect("ess-inputs.yaml has a top-level `scenarios:` list");
    fs::write(&inputs, format!("{}\nscenarios: []\n", &listed[..at]))
        .expect("write ess-inputs.yaml copy");
    let Err(failure) = run_gate(&copy, Synthesis::Fresh) else {
        panic!("the gate passed a specification synthesize refuses");
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(
        failure.detail.contains("selected no authored files"),
        "the failure carries the refusal: {failure}"
    );
}

/// A command whose `never` outcome no input satisfies. ess validates and compiles it, and
/// `ess verify conform synthesize --suite-format 5` refuses the outcome (`ESS-SYNTH-003`), covers
/// it with no authored scenario and exits 1 (ess 0.96.0); the summary-line counts are the second
/// signal, for a synthesize that exits 0 while refusing.
/// It is a file of its own in domain `loom.run`, so it does not depend on which sections
/// `run.yaml` already declares.
const UNSATISFIABLE_COMMAND: &str = "domain: loom.run

commands:
  - name: loom.run.Probe
    naming:
      wire: probe
      display: Probe
    input:
      - name: strategy
        type: loom.run.SelectionStrategy
    outcomes:
      - name: never
        when:
          all:
            - strategy == Rule
            - strategy == Hybrid
        emits: [loom.run.Probed]
        payload:
          loom.run.Probed:
            strategy: input.strategy
      - name: always
        emits: [loom.run.Probed]
        payload:
          loom.run.Probed:
            strategy: input.strategy

events:
  - name: loom.run.Probed
    naming:
      wire: probed
      display: Probed
    fields:
      - name: strategy
        type: loom.run.SelectionStrategy
";

/// A copy of `ess/` ([`copy_spec`]) with `edit` applied to the text of `file`.
fn copy_with_edit(name: &str, file: &str, edit: impl FnOnce(String) -> String) -> PathBuf {
    let copy = copy_spec(name);
    edit_file(&copy, file, edit);
    copy
}

// Item 3, the refusal count: synthesize refuses one outcome no authored scenario covers; the gate
// fails at step 3.
#[test]
fn gate_over_a_copy_with_an_uncovered_refusal_fails_at_synthesize() {
    let copy = copy_with_edit("gate_refused_uncovered", "ess-inputs.yaml", |text| {
        text.replacen(
            "  - domains/run.yaml\n",
            "  - domains/run.yaml\n  - domains/probe.yaml\n",
            1,
        )
    });
    fs::write(copy.join("domains/probe.yaml"), UNSATISFIABLE_COMMAND).expect("write probe.yaml");
    let Err(failure) = run_gate(&copy, Synthesis::Fresh) else {
        panic!(
            "the gate passed a specification with a refusal no authored scenario covers \
             (ESS-SYNTH-003)"
        );
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(
        failure.detail.contains("loom.run.Probe/outcome/never"),
        "the failure names the uncovered refusal: {failure}"
    );
}

// Item 3, covered refusals: the tree's own `covers:` scenarios cover every refusal of `ess/`, and
// removing one cover makes the gate fail at step 3. Every cover of `ess/` is the decision
// domain's, so the copy keeps it and synthesizes for minutes: the slow lane (`task slow`) runs it.
#[test]
fn slow_gate_over_a_copy_missing_one_cover_fails_at_synthesize() {
    let copy = scratch("gate_cover_removed").join("ess");
    copy_dir(&spec_dir(), &copy);
    edit_file(&copy, "ess-inputs.yaml", |text| {
        text.replacen(
            "  - scenarios/decision/escalated-request-is-superseded.yaml\n",
            "",
            1,
        )
    });
    let Err(failure) = run_gate(&copy, Synthesis::Fresh) else {
        panic!("the gate passed a specification whose supersede transition no scenario covers");
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(
        failure.detail.contains("transition/supersede"),
        "the failure names the uncovered refusal: {failure}"
    );
}

// Item 1, `--strict-requires`: a specification requiring an older ess fails validation.
#[test]
fn gate_over_a_copy_requiring_an_older_ess_fails_at_validate() {
    let copy = copy_with_edit("gate_requires_older", "ess-inputs.yaml", |text| {
        text.replacen("requires: ess 0.96.0", "requires: ess 0.51.0", 1)
    });
    let Err(failure) = run_gate(&copy, Synthesis::Fresh) else {
        panic!(
            "the gate passed a specification requiring ess 0.51.0: validate does not run with \
             --strict-requires"
        );
    };
    assert_eq!(failure.step, Step::Validate, "{failure}");
    assert!(failure.detail.contains("--strict-requires"), "{failure}");
}

// Items 1, 3 and 4: the gate holds on the specification. Step 3 reads the committed synthesis.
#[test]
fn gate_holds_on_the_specification() {
    if let Err(failure) = run_gate(&spec_dir(), Synthesis::Committed(&repo_root())) {
        panic!("{failure}");
    }
    let (_, suite) = ess_suite::committed(&repo_root(), &ess_program())
        .unwrap_or_else(|detail| panic!("{detail}"));
    assert!(
        suite.is_some_and(|suite| !suite.is_empty()),
        "{} holds the suite",
        ess_suite::SUITE
    );
}

// Item 3 over the committed synthesis: the gate holds over a tree whose committed suite was
// synthesized from its `ess/`, and fails at step 3 once a file of `ess/` changed after it, naming
// the command that writes it again, or once the suite file was changed by hand.
#[test]
fn gate_over_a_tree_whose_committed_suite_is_stale_fails_at_synthesize() {
    let root = scratch("committed_stale");
    let spec = root.join("ess");
    copy_dir(&spec_dir(), &spec);
    drop_decision(&spec);
    let ess = ess_program();
    let run = ess_suite::synthesize_once(&ess, &spec, &synthesis_cache())
        .unwrap_or_else(|detail| panic!("synthesize the copy: {detail}"));
    ess_suite::write_generated(&root, &ess, None, &run)
        .unwrap_or_else(|detail| panic!("write the copy's synthesis: {detail}"));
    if let Err(failure) = run_gate(&spec, Synthesis::Committed(&root)) {
        panic!("the gate fails over a tree whose suite was synthesized from it: {failure}");
    }

    let run_yaml = spec.join("domains/run.yaml");
    let original = fs::read_to_string(&run_yaml).expect("read run.yaml copy");
    fs::write(&run_yaml, format!("{original}# changed after the suite\n")).expect("write");
    let Err(failure) = run_gate(&spec, Synthesis::Committed(&root)) else {
        panic!("the gate passed a tree whose ess/ changed after its suite was synthesized");
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(
        failure
            .detail
            .contains("regenerate it with `task suite-generate`"),
        "{failure}"
    );

    fs::write(&run_yaml, original).expect("restore run.yaml copy");
    let suite = root.join(ess_suite::SUITE);
    let mut text = fs::read_to_string(&suite).expect("read the copy's suite");
    text.push(' ');
    fs::write(&suite, text).expect("write the copy's suite");
    let Err(failure) = run_gate(&spec, Synthesis::Committed(&root)) else {
        panic!("the gate passed a committed suite changed by hand");
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(failure.detail.contains("changed by hand"), "{failure}");
}

// Item 2: the compiled specification declares what this story settled.
#[test]
fn compiled_specification_declares_the_settled_relations() {
    let compiled = compile(&spec_dir()).unwrap_or_else(|f| panic!("{f}"));

    let run_id = compiled
        .at(&["types", "loom.run.CommissionRunId", "body"])
        .expect("type loom.run.CommissionRunId is declared");
    assert_eq!(run_id.str_at(&["kind"]), Some("newtype"));
    assert_eq!(run_id.str_at(&["of", "kind"]), Some("primitive"));
    assert_eq!(run_id.str_at(&["of", "name"]), Some("uuid"));

    let run = compiled
        .at(&["types", "loom.workflow.WorkflowRunRecord", "body", "fields"])
        .and_then(Json::as_array)
        .expect("type loom.workflow.WorkflowRunRecord has fields")
        .iter()
        .find(|f| f.str_at(&["name"]) == Some("run"))
        .and_then(|f| f.at(&["type_ref"]))
        .expect("loom.workflow.WorkflowRunRecord has field `run`");
    assert_eq!(run.str_at(&["kind"]), Some("declared"));
    assert_eq!(run.str_at(&["name"]), Some("loom.run.CommissionRunId"));

    let confidence = field(&compiled, "loom.run.Selection", "confidence");
    assert_eq!(confidence.str_at(&["kind"]), Some("optional"));
    assert_eq!(confidence.str_at(&["of", "kind"]), Some("primitive"));
    assert_eq!(confidence.str_at(&["of", "name"]), Some("decimal"));

    let turn_id = field(&compiled, "loom.run.ActionCatalogue", "turn_id");
    assert_eq!(turn_id.str_at(&["kind"]), Some("declared"));
    assert_eq!(turn_id.str_at(&["name"]), Some("loom.run.TurnId"));

    let relations = compiled
        .at(&["entities", "loom.run.Selection", "relations"])
        .and_then(Json::as_array)
        .expect("loom.run.Selection has relations");
    let catalogue = relations
        .iter()
        .find(|r| r.str_at(&["name"]) == Some("catalogue"))
        .expect("loom.run.Selection has relation `catalogue`");
    assert_eq!(catalogue.str_at(&["kind"]), Some("references"));
    assert_eq!(catalogue.str_at(&["cardinality"]), Some("one"));
    assert_eq!(
        catalogue.str_at(&["target"]),
        Some("loom.run.ActionCatalogue")
    );
    assert_eq!(catalogue.str_at(&["via"]), Some("catalogue_id"));

    // The session, its turns and compactions were the ported Harness loop's, and went with it
    // (`story:delete-harness-port`).
    for entity in ["loom.run.Session", "loom.run.Turn", "loom.run.Compaction"] {
        assert!(
            compiled.at(&["entities", entity]).is_none(),
            "{entity} is declared again"
        );
    }
}

/// The `type_ref` of `entity.fields[name]`.
fn field<'a>(compiled: &'a Json, entity: &str, name: &str) -> &'a Json {
    compiled
        .at(&["entities", entity, "fields"])
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("{entity} has fields"))
        .iter()
        .find(|f| f.str_at(&["name"]) == Some(name))
        .unwrap_or_else(|| panic!("{entity} has field `{name}`"))
        .at(&["type_ref"])
        .unwrap_or_else(|| panic!("{entity}.{name} has a type_ref"))
}

// Item 5: the marker scan names the file and the line.
#[test]
fn marker_scan_names_file_and_line_of_an_appended_marker() {
    let (copy, line) = copy_with_probe_marker("marker_scan");
    assert_eq!(
        scan_markers(&copy),
        vec![Marker {
            file: "domains/run.yaml".to_owned(),
            line,
        }]
    );
}

// Item 6: the whole gate over a marked copy passes steps 1 to 3 and fails at the marker scan.
#[test]
fn gate_over_a_marked_copy_fails_at_the_marker_scan() {
    let (copy, line) = copy_with_probe_marker("gate_marked_copy");
    let Err(failure) = run_gate(&copy, Synthesis::Fresh) else {
        panic!("the gate passed a specification carrying `# UNMAPPED: probe`");
    };
    assert_eq!(failure.step, Step::MarkerScan, "{failure}");
    assert_eq!(
        failure.markers,
        vec![Marker {
            file: "domains/run.yaml".to_owned(),
            line,
        }]
    );
    let message = failure.to_string();
    assert!(
        message.contains(&format!("domains/run.yaml:{line}")),
        "{message}"
    );
}

// Item 7: AGENTS.md § ESS states the gate.
#[test]
fn agents_md_states_the_hard_gate() {
    let agents = fs::read_to_string(repo_root().join("AGENTS.md")).expect("read AGENTS.md");
    let section = agents
        .split("\n## ")
        .find(|s| s.starts_with("ESS\n"))
        .expect("AGENTS.md has a section `## ESS`");
    for needle in [
        "task ess-gate",
        "task check",
        "ess specify validate --path ess --strict-requires",
        "ess specify compile --path ess",
        "ess verify conform synthesize",
        "0 refusals",
        MARKER,
        "decision-blocker",
        "No story is implemented while the gate is red",
        "epic:typed-open-questions",
    ] {
        assert!(section.contains(needle), "§ ESS names `{needle}`");
    }
}

// Acceptance: `task check` runs this test through the task `ess-gate`.
#[test]
fn taskfile_check_runs_the_ess_gate() {
    let taskfile = fs::read_to_string(repo_root().join("Taskfile.yml")).expect("read Taskfile.yml");
    let tasks = taskfile
        .split_once("\ntasks:\n")
        .expect("Taskfile has tasks")
        .1;
    let body = |name: &str| -> String {
        let header = format!("\n  {name}:\n");
        let start = format!("\n{tasks}")
            .find(&header)
            .unwrap_or_else(|| panic!("Taskfile has task `{name}`"));
        let rest = &format!("\n{tasks}")[start + header.len()..];
        rest.lines()
            .take_while(|l| l.is_empty() || l.starts_with("    "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        body("ess-gate").contains("cargo test -p b10x-loom-executor --test ess_gate --locked"),
        "task ess-gate runs the ess_gate test"
    );
    assert!(
        body("ess-gate").contains("-- --skip slow_"),
        "task ess-gate leaves the slow lane's cases to task slow"
    );
    assert!(
        body("check")
            .lines()
            .any(|l| l.trim() == "- task: ess-gate"),
        "task check lists ess-gate as its own step"
    );
}

#[test]
fn refusal_count_is_read_from_the_summary_line() {
    let counts = |occurrences, covered| {
        Some(Refusals {
            occurrences,
            covered,
        })
    };
    // What ess 0.96.0 prints for main at c1a1f443: no refusal, so no `covered by authored`.
    assert_eq!(
        refusals("286 selected scenario(s), 103 authored source(s), 0 refusal occurrence(s)"),
        counts(0, 0)
    );
    // One refusal, none covered: the cover count is left out and reads as 0.
    assert_eq!(
        refusals("287 selected scenario(s), 103 authored source(s), 1 refusal occurrence(s)"),
        counts(1, 0)
    );
    assert_eq!(
        refusals(
            "362 selected scenario(s), 107 authored source(s), 10 refusal occurrence(s), \
             10 covered by authored"
        ),
        counts(10, 10)
    );
    // `refused:` lines, then the summary. The counts are read from the summary, the last line
    // that carries both.
    assert_eq!(
        refusals(
            "refused: some construct\nrefused: says 0 refusal occurrence(s), 0 covered by \
             authored in its text\n77 selected scenario(s), 4 authored source(s), \
             11 refusal occurrence(s), 10 covered by authored\n"
        ),
        counts(11, 10)
    );
    // The default-format summary carries no cover count: it is not read as one.
    assert_eq!(
        refusals("3 scenario(s) (1 authored), 12 refusal(s), written to x"),
        None
    );
    assert_eq!(refusals("written to x"), None);
}

// Item 3, the count arm: ess 0.96.0 exits 1 on an uncovered refusal under `--suite-format 5`, so
// no specification makes synthesize exit 0 with one. Step 3's own check is fed that run: exit 0
// and a summary line with more occurrences than covered fails it, and so does a summary without
// counts.
#[test]
fn synthesize_exiting_0_with_an_uncovered_occurrence_fails_step_3() {
    let uncovered = synthesize_holds(
        true,
        "refused[ESS-SYNTH-003]: …\n\
         363 selected scenario(s), 107 authored source(s), 11 refusal occurrence(s), \
         10 covered by authored\n",
        "exit status: 0",
    );
    let Err(detail) = uncovered else {
        panic!("step 3 passed exit 0 with 11 refusal occurrence(s), 10 covered by authored");
    };
    assert!(
        detail.contains("1 refusal(s) not covered by an authored scenario"),
        "{detail}"
    );

    let uncounted = synthesize_holds(true, "written to x", "exit status: 0");
    assert!(
        uncounted
            .as_ref()
            .is_err_and(|detail| detail.contains("no refusal count")),
        "step 3 passed exit 0 with no summary line: {uncounted:?}"
    );

    assert_eq!(
        synthesize_holds(
            true,
            "362 selected scenario(s), 107 authored source(s), 10 refusal occurrence(s), \
             10 covered by authored",
            "exit status: 0"
        ),
        Ok(())
    );
    assert_eq!(
        synthesize_holds(
            true,
            "286 selected scenario(s), 103 authored source(s), 0 refusal occurrence(s)",
            "exit status: 0"
        ),
        Ok(())
    );
    assert_eq!(
        synthesize_holds(false, "", "exit status: 1"),
        Err("exit status: 1".to_owned())
    );
}

/// Just enough JSON to read `ess specify compile --format json`.
#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn parse(text: &str) -> Result<Json, String> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = parser.value()?;
        parser.whitespace();
        if parser.pos != parser.bytes.len() {
            return Err(format!("trailing input at byte {}", parser.pos));
        }
        Ok(value)
    }

    fn at(&self, path: &[&str]) -> Option<&Json> {
        path.iter().try_fold(self, |value, key| match value {
            Json::Object(map) => map.get(*key),
            _ => None,
        })
    }

    fn str_at(&self, path: &[&str]) -> Option<&str> {
        match self.at(path)? {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn whitespace(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", byte as char, self.pos))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(format!("expected `{word}` at byte {}", self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.whitespace();
        match self.bytes.get(self.pos) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => {
                let start = self.pos;
                while matches!(
                    self.bytes.get(self.pos),
                    Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                ) {
                    self.pos += 1;
                }
                let number =
                    std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|e| e.to_string())?;
                Ok(Json::Number(number.to_owned()))
            }
            _ => Err(format!("unexpected input at byte {}", self.pos)),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut map = BTreeMap::new();
        self.whitespace();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Object(map));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            let value = self.value()?;
            map.insert(key, value);
            self.whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(map));
                }
                _ => return Err(format!("expected `,` or `}}` at byte {}", self.pos)),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.whitespace();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.whitespace();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(format!("expected `,` or `]` at byte {}", self.pos)),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while !matches!(self.bytes.get(self.pos), Some(b'"' | b'\\') | None) {
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|e| e.to_string())?,
            );
            match self.bytes.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    let escape = *self
                        .bytes
                        .get(self.pos + 1)
                        .ok_or_else(|| "unterminated escape".to_owned())?;
                    self.pos += 2;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&unit) {
                                self.expect(b'\\')?;
                                self.expect(b'u')?;
                                let low = self.hex4()?;
                                0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                            } else {
                                unit
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| format!("invalid code point {code:#x}"))?,
                            );
                        }
                        other => return Err(format!("invalid escape `\\{}`", other as char)),
                    }
                }
                _ => return Err("unterminated string".to_owned()),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| "short \\u escape".to_owned())?;
        self.pos += 4;
        u32::from_str_radix(std::str::from_utf8(digits).map_err(|e| e.to_string())?, 16)
            .map_err(|e| e.to_string())
    }
}
