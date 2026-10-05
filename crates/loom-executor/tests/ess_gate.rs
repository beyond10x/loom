//! The ESS hard gate (Atlas ADR 0076): the specification under `ess/` validates with
//! `--strict-requires`, compiles, synthesizes its conformance suite with 0 refusals, and carries no
//! open question (no `UNMAPPED:` string in any file under `ess/`).
//!
//! The gate is a function over a directory, so it runs on `ess/` and on temporary copies of it.
//! Everything it writes lands under `CARGO_TARGET_TMPDIR`. It uses only the standard library and
//! the `ess` binary on `PATH`. The repository is read from `CARGO_MANIFEST_DIR` at run time.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
            Step::Synthesize => "synthesize (ess verify conform synthesize, 0 refusals)",
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

/// Runs the four gate steps over `dir` in order and stops at the first that does not hold. On
/// success returns the compiled specification.
fn run_gate(dir: &Path, suite_out: &Path) -> Result<Json, GateFailure> {
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

    let synthesize = ess(&[
        "verify",
        "conform",
        "synthesize",
        "--path",
        path_arg(dir),
        "--out",
        path_arg(suite_out),
    ]);
    if !synthesize.status.success() {
        return Err(fail(Step::Synthesize, describe(&synthesize)));
    }
    match refusals(&String::from_utf8_lossy(&synthesize.stdout)) {
        Some(0) => {}
        Some(n) => {
            return Err(fail(
                Step::Synthesize,
                format!("{n} refusal(s): {}", describe(&synthesize)),
            ));
        }
        None => {
            return Err(fail(
                Step::Synthesize,
                format!("no refusal count in the output: {}", describe(&synthesize)),
            ));
        }
    }

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

/// The count from the synthesize summary line, `… N refusal(s), written to …`. The summary is the
/// last line carrying a count; `refused:` lines before it are not read for one.
fn refusals(stdout: &str) -> Option<u64> {
    stdout.lines().rev().find_map(|line| {
        let before = &line[..line.find(" refusal(s)")?];
        let digits = before.rsplit(|c: char| !c.is_ascii_digit()).next()?;
        digits.parse().ok()
    })
}

fn ess(args: &[&str]) -> Output {
    Command::new("ess")
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

/// A copy of `ess/` with `# UNMAPPED: probe` appended to `domains/run.yaml`. Returns the copy and
/// the appended line's one-based number.
fn copy_with_probe_marker(name: &str) -> (PathBuf, usize) {
    let copy = scratch(name).join("ess");
    copy_dir(&spec_dir(), &copy);
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
// held. `Optional<Binary64>` validates and compiles, and the conformance suite refuses it. Every
// confidence is retyped: `SelectAction` copies its input into `Selection.confidence`, and
// retyping one side alone fails validation instead.
#[test]
fn gate_over_a_copy_synthesize_refuses_fails_at_synthesize() {
    let copy = scratch("gate_refused_copy").join("ess");
    copy_dir(&spec_dir(), &copy);
    let run = copy.join("domains/run.yaml");
    let text = fs::read_to_string(&run).expect("read run.yaml copy");
    assert!(
        text.contains("type: Optional<Decimal>"),
        "run.yaml declares the confidence as Optional<Decimal>"
    );
    fs::write(
        &run,
        text.replace("type: Optional<Decimal>", "type: Optional<Binary64>"),
    )
    .expect("write run.yaml copy");

    let suite = copy.parent().expect("scratch").join("loom-suite.json");
    let Err(failure) = run_gate(&copy, &suite) else {
        panic!("the gate passed a specification synthesize refuses");
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(
        failure.detail.contains("UnsupportedPrimitive"),
        "the failure carries the refusal: {failure}"
    );
}

/// A command whose `never` outcome no input satisfies. ess 0.52.0 validates and compiles it, and
/// `ess verify conform synthesize` refuses the outcome (`ESS-SYNTH-003`) and still exits 0 with
/// `1 refusal(s)` on its summary line: the refusal count is the only signal that step 3 fails.
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

/// A copy of `ess/` in a fresh scratch directory, with `edit` applied to the text of `file`.
fn copy_with_edit(name: &str, file: &str, edit: impl FnOnce(String) -> String) -> PathBuf {
    let copy = scratch(name).join("ess");
    copy_dir(&spec_dir(), &copy);
    let path = copy.join(file);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file} copy: {e}"));
    let edited = edit(text.clone());
    assert_ne!(edited, text, "the edit to {file} applies");
    fs::write(&path, edited).unwrap_or_else(|e| panic!("write {file} copy: {e}"));
    copy
}

// Item 3, the refusal count: synthesize exits 0 and refuses one outcome; the gate fails at step 3.
#[test]
fn gate_over_a_copy_synthesize_refuses_with_exit_0_fails_at_synthesize() {
    let copy = copy_with_edit("gate_refused_exit_0", "ess-inputs.yaml", |text| {
        text.replacen(
            "  - domains/run.yaml\n",
            "  - domains/run.yaml\n  - domains/probe.yaml\n",
            1,
        )
    });
    fs::write(copy.join("domains/probe.yaml"), UNSATISFIABLE_COMMAND).expect("write probe.yaml");
    let suite = copy.parent().expect("scratch").join("loom-suite.json");
    let Err(failure) = run_gate(&copy, &suite) else {
        panic!(
            "the gate passed a specification synthesize refuses with exit 0 and 1 refusal(s) \
             (ESS-SYNTH-003)"
        );
    };
    assert_eq!(failure.step, Step::Synthesize, "{failure}");
    assert!(failure.detail.contains("1 refusal(s)"), "{failure}");
}

// Item 1, `--strict-requires`: a specification requiring an older ess fails validation.
#[test]
fn gate_over_a_copy_requiring_an_older_ess_fails_at_validate() {
    let copy = copy_with_edit("gate_requires_older", "ess-inputs.yaml", |text| {
        text.replacen("requires: ess 0.52.0", "requires: ess 0.51.0", 1)
    });
    let suite = copy.parent().expect("scratch").join("loom-suite.json");
    let Err(failure) = run_gate(&copy, &suite) else {
        panic!(
            "the gate passed a specification requiring ess 0.51.0: validate does not run with \
             --strict-requires"
        );
    };
    assert_eq!(failure.step, Step::Validate, "{failure}");
    assert!(failure.detail.contains("--strict-requires"), "{failure}");
}

// Items 1, 3 and 4: the gate holds on the specification.
#[test]
fn gate_holds_on_the_specification() {
    let suite = scratch("gate_holds").join("loom-suite.json");
    if let Err(failure) = run_gate(&spec_dir(), &suite) {
        panic!("{failure}");
    }
    assert!(suite.is_file(), "suite written to {}", suite.display());
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

    let commission_run = field(&compiled, "loom.run.Session", "commission_run");
    assert_eq!(commission_run.str_at(&["kind"]), Some("declared"));
    assert_eq!(
        commission_run.str_at(&["name"]),
        Some("loom.run.CommissionRunId")
    );

    let confidence = field(&compiled, "loom.run.Selection", "confidence");
    assert_eq!(confidence.str_at(&["kind"]), Some("optional"));
    assert_eq!(confidence.str_at(&["of", "kind"]), Some("primitive"));
    assert_eq!(confidence.str_at(&["of", "name"]), Some("decimal"));

    let turn_id = field(&compiled, "loom.run.ActionCatalogue", "turn_id");
    assert_eq!(turn_id.str_at(&["kind"]), Some("declared"));
    assert_eq!(turn_id.str_at(&["name"]), Some("loom.run.TurnId"));

    let relations = compiled
        .at(&["entities", "loom.run.Turn", "relations"])
        .and_then(Json::as_array)
        .expect("loom.run.Turn has relations");
    let catalogue = relations
        .iter()
        .find(|r| r.str_at(&["name"]) == Some("catalogue"))
        .expect("loom.run.Turn has relation `catalogue`");
    assert_eq!(catalogue.str_at(&["kind"]), Some("owns"));
    assert_eq!(catalogue.str_at(&["cardinality"]), Some("one"));
    assert_eq!(
        catalogue.str_at(&["target"]),
        Some("loom.run.ActionCatalogue")
    );
    assert_eq!(catalogue.str_at(&["via"]), Some("turn_id"));
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
    let suite = copy.parent().expect("scratch").join("loom-suite.json");
    let Err(failure) = run_gate(&copy, &suite) else {
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
        body("check")
            .lines()
            .any(|l| l.trim() == "- task: ess-gate"),
        "task check lists ess-gate as its own step"
    );
}

#[test]
fn refusal_count_is_read_from_the_summary_line() {
    assert_eq!(
        refusals("0 scenario(s) (0 authored), 0 refusal(s), written to x"),
        Some(0)
    );
    assert_eq!(
        refusals("3 scenario(s) (1 authored), 12 refusal(s), written to x"),
        Some(12)
    );
    // ess 0.52.0 can exit 0 while refusing: `refused:` lines, then the summary. The count is read
    // from the summary, the last line that carries one.
    assert_eq!(
        refusals(
            "refused: some construct\nrefused: says 0 refusal(s) in its text\n\
             3 scenario(s) (0 authored), 2 refusal(s), written to /x/suite.json\n"
        ),
        Some(2)
    );
    assert_eq!(refusals("written to x"), None);
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
