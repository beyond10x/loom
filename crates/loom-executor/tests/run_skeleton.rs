//! Acceptance of `story:run-pipeline-skeleton`: the run pipeline's settled nouns are declared in
//! `ess/`, the generated model carries them through `b10x-loom-executor`'s re-export, and the module files
//! of `epic:loom-native-harness` exist and are declared in `lib.rs`.
//!
//! The repository is read from `CARGO_MANIFEST_DIR` at run time, and the specification is compiled
//! by the `ess` binary on `PATH`. Only the standard library is used.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId, TurnId,
};

/// The commands the story declares.
const COMMANDS: [&str; 4] = [
    "loom.run.ProjectCatalogue",
    "loom.run.SelectAction",
    "loom.run.RequestArguments",
    "loom.run.RevalidateSelection",
];

/// The types the story declares.
const TYPES: [&str; 2] = ["loom.run.CatalogueEntry", "loom.run.CatalogueEntryStatus"];

/// The lifecycle states of `loom.run.Selection`.
const SELECTION_STATES: [&str; 3] = ["Selected", "Admitted", "Refused"];

/// The module files of the epic, relative to `crates/loom-executor/src`, with the module each declares.
const MODULES: [(&str, &str); 8] = [
    ("projection", "projection.rs"),
    ("selection", "selection.rs"),
    ("arguments", "arguments.rs"),
    ("revalidation", "revalidation.rs"),
    ("session", "session.rs"),
    ("compaction", "compaction.rs"),
    ("recovery", "recovery.rs"),
    ("harness", "harness/mod.rs"),
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
}

#[test]
fn run_skeleton_declares_pipeline_nouns() {
    // 1. The compiled specification names the commands, the types and the Selection lifecycle.
    let spec = crate_dir().join("../../ess");
    let output = Command::new("ess")
        .args(["specify", "compile", "--path"])
        .arg(&spec)
        .args(["--format", "json"])
        .output()
        .expect("run `ess specify compile`");
    assert!(
        output.status.success(),
        "`ess specify compile --path {} --format json` failed ({}): {}",
        spec.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let compiled = Json::parse(&String::from_utf8_lossy(&output.stdout))
        .expect("`ess specify compile` writes JSON");
    for command in COMMANDS {
        assert!(
            compiled.at(&["commands", command]).is_some(),
            "the compiled specification declares command {command}"
        );
    }
    for name in TYPES {
        assert!(
            compiled.at(&["types", name]).is_some(),
            "the compiled specification declares type {name}"
        );
    }
    let states: Vec<&str> = compiled
        .at(&["entities", "loom.run.Selection", "lifecycle", "states"])
        .and_then(Json::as_array)
        .expect("loom.run.Selection declares lifecycle states")
        .iter()
        .filter_map(Json::as_str)
        .collect();
    for state in SELECTION_STATES {
        assert!(
            states.contains(&state),
            "loom.run.Selection has lifecycle state {state}; it has {states:?}"
        );
    }

    // The selection records the case revision of its catalogue, taken from SelectAction's input,
    // which SelectAction refuses unless it is that catalogue's; revalidation refuses a stale one
    // by a guard over the stored revision, not externally.
    let mismatch = outcome(&compiled, "loom.run.SelectAction", "revision-mismatch");
    assert_eq!(
        mismatch.at(&["condition", "kind"]).and_then(Json::as_str),
        Some("related"),
        "revision-mismatch is selected by a guard over the catalogue the selection names"
    );
    assert_eq!(
        mismatch.at(&["condition", "entity"]).and_then(Json::as_str),
        Some("loom.run.ActionCatalogue")
    );
    assert_eq!(
        mismatch
            .at(&["condition", "test", "holds", "predicate"])
            .and_then(Json::as_str),
        Some("case_revision != input.case_revision"),
        "revision-mismatch compares the catalogue's case_revision with the input's"
    );
    assert_eq!(
        mismatch.at(&["error"]).and_then(Json::as_str),
        Some("loom.run.CatalogueRevisionMismatch")
    );
    assert!(
        names(&compiled, &["commands", "loom.run.SelectAction", "input"])
            .contains(&"case_revision"),
        "loom.run.SelectAction takes a case_revision input"
    );
    assert!(
        names(&compiled, &["entities", "loom.run.Selection", "fields"]).contains(&"case_revision"),
        "loom.run.Selection stores its case_revision"
    );
    let stale = outcome(&compiled, "loom.run.RevalidateSelection", "stale-revision");
    assert_eq!(
        stale.at(&["condition", "kind"]).and_then(Json::as_str),
        Some("subject_predicate"),
        "stale-revision is selected by a guard over the stored case_revision"
    );
    assert_eq!(
        stale.at(&["condition", "predicate"]).and_then(Json::as_str),
        Some("case_revision != input.case_revision"),
        "stale-revision compares the stored case_revision with the current one"
    );

    // An action id absent from the catalogue's entries is refused by a guard over that catalogue.
    let absent = outcome(&compiled, "loom.run.SelectAction", "not-in-catalogue");
    assert_eq!(
        absent.at(&["condition", "kind"]).and_then(Json::as_str),
        Some("related"),
        "not-in-catalogue is selected by a guard over the catalogue the selection names"
    );
    assert_eq!(
        absent.at(&["condition", "entity"]).and_then(Json::as_str),
        Some("loom.run.ActionCatalogue")
    );
    let membership = absent
        .at(&["condition", "test", "holds", "predicate", "not", "exists"])
        .expect("not-in-catalogue holds when no entry of the catalogue matches");
    assert_eq!(
        membership.at(&["in"]).and_then(Json::as_str),
        Some("entries")
    );
    assert_eq!(
        membership.at(&["that"]).and_then(Json::as_str),
        Some("entry.action == input.action")
    );
    assert_eq!(
        absent.at(&["error"]).and_then(Json::as_str),
        Some("loom.run.ActionNotInCatalogue"),
        "not-in-catalogue refuses with the error that names the action id"
    );

    // 2. An ActionCatalogue built through the re-export reads its entries back in order.
    let catalogue = ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        frontier: "frontier-7".to_owned(),
        case_revision: 7,
        entries: vec![
            CatalogueEntry {
                action: "repository.read".to_owned(),
                status: CatalogueEntryStatus::Admissible,
            },
            CatalogueEntry {
                action: "repository.merge".to_owned(),
                status: CatalogueEntryStatus::ApprovalRequired,
            },
        ],
    });
    let entries: Vec<(&str, CatalogueEntryStatus)> = catalogue
        .data()
        .entries
        .iter()
        .map(|entry| (entry.action.as_str(), entry.status))
        .collect();
    assert_eq!(
        entries,
        [
            ("repository.read", CatalogueEntryStatus::Admissible),
            ("repository.merge", CatalogueEntryStatus::ApprovalRequired),
        ]
    );

    // 3. Each module file exists, and lib.rs declares each with `pub mod`.
    let src = crate_dir().join("src");
    let lib = fs::read_to_string(src.join("lib.rs")).expect("read crates/loom-executor/src/lib.rs");
    for (module, file) in MODULES {
        assert!(
            src.join(file).is_file(),
            "crates/loom-executor/src/{file} exists"
        );
        let declaration = format!("pub mod {module};");
        assert!(
            lib.lines().any(|line| line.trim() == declaration),
            "crates/loom-executor/src/lib.rs declares `{declaration}`"
        );
    }
}

/// The outcome `name` of `command`.
fn outcome<'a>(compiled: &'a Json, command: &str, name: &str) -> &'a Json {
    compiled
        .at(&["commands", command, "outcomes"])
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("{command} declares outcomes"))
        .iter()
        .find(|outcome| outcome.at(&["name"]).and_then(Json::as_str) == Some(name))
        .unwrap_or_else(|| panic!("{command} declares outcome {name}"))
}

/// The `name` of each element of the array at `path`.
fn names<'a>(compiled: &'a Json, path: &[&str]) -> Vec<&'a str> {
    compiled
        .at(path)
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("the compiled specification has an array at {path:?}"))
        .iter()
        .filter_map(|item| item.at(&["name"]).and_then(Json::as_str))
        .collect()
}

/// Just enough JSON to read `ess specify compile --format json`.
#[derive(Debug)]
enum Json {
    Null,
    Bool,
    Number,
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

    fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
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
            Some(b't') => self.literal("true", Json::Bool),
            Some(b'f') => self.literal("false", Json::Bool),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => {
                while matches!(
                    self.bytes.get(self.pos),
                    Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                ) {
                    self.pos += 1;
                }
                Ok(Json::Number)
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

    /// A string; escapes are kept as written, which is enough to match declared names.
    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let start = self.pos;
        loop {
            match self.bytes.get(self.pos) {
                Some(b'"') => break,
                Some(b'\\') => self.pos += 2,
                Some(_) => self.pos += 1,
                None => return Err("unterminated string".to_owned()),
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|e| e.to_string())?
            .to_owned();
        self.pos += 1;
        Ok(text)
    }
}
