//! Acceptance for `story:port-skeleton`: the port vocabulary is declared in ESS and generated, the
//! bootstrap contracts have left the crate root, and the empty port, module, fake and kit files
//! exist with their `mod` lines.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time:
//! a build directory shared between worktrees reuses binaries across them.

// Expectation 4: these paths resolve. The modules are empty until their stories fill them, so
// nothing below names an item in them.
#[allow(unused_imports)]
use b10x_commission::ports::{authority, evidence, executor, governor};
#[allow(unused_imports)]
use b10x_commission::{action_request, admission, outcome, runtime};
#[allow(unused_imports)]
use b10x_commission_testkit::{fake_authority, fake_executor, fake_governor, kits};

use b10x_commission::model::json::{self, Value};
use b10x_commission::model::responsibility as model;
use std::path::{Path, PathBuf};
use std::process::Command;

const NS: &str = "commission.responsibility.";

/// The ESS primitives, as a specification writes them.
const PRIMITIVES: [&str; 10] = [
    "String",
    "Boolean",
    "Integer",
    "Decimal",
    "Binary64",
    "Timestamp",
    "Duration",
    "Uuid",
    "Bytes",
    "Json",
];

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// The model `ess specify compile --path ess --format json` prints for this tree.
fn compiled_model(root: &Path) -> Value {
    let out = Command::new("ess")
        .args(["specify", "compile", "--path"])
        .arg(root.join("ess"))
        .args(["--format", "json"])
        .output()
        .unwrap_or_else(|error| panic!("run `ess specify compile`: {error}"));
    assert!(
        out.status.success(),
        "`ess specify compile` failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    json::parse(&String::from_utf8_lossy(&out.stdout))
        .unwrap_or_else(|error| panic!("compiled model is not JSON: {error:?}"))
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

fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        other => panic!("expected an array, found {}", other.describes()),
    }
}

fn object(value: &Value) -> &[(String, Value)] {
    match value {
        Value::Object(members) => members,
        other => panic!("expected an object, found {}", other.describes()),
    }
}

/// A compiled type reference in the spelling the specification writes it.
fn spelling(type_ref: &Value) -> String {
    let name = || text(member(type_ref, &["name"]));
    match text(member(type_ref, &["kind"])) {
        "declared" => name().to_string(),
        "primitive" => PRIMITIVES
            .iter()
            .find(|primitive| primitive.eq_ignore_ascii_case(name()))
            .unwrap_or_else(|| panic!("unknown primitive `{}`", name()))
            .to_string(),
        "list" => format!("List<{}>", spelling(member(type_ref, &["of"]))),
        other => panic!("type reference kind `{other}` is not expected here"),
    }
}

/// A primitive as written, or a bare name qualified into this domain.
fn qualified(name: &str) -> String {
    if PRIMITIVES.contains(&name) {
        name.to_string()
    } else {
        format!("{NS}{name}")
    }
}

fn declared_type<'a>(model: &'a Value, name: &str) -> &'a Value {
    member(model, &["types", &format!("{NS}{name}")])
}

fn assert_enum(model: &Value, name: &str, expected: &[&str]) {
    let body = member(declared_type(model, name), &["body"]);
    assert_eq!(
        text(member(body, &["kind"])),
        "enum",
        "{name} is not an enum"
    );
    let variants: Vec<&str> = array(member(body, &["variants"]))
        .iter()
        .map(text)
        .collect();
    assert_eq!(variants, expected, "{name} variants");
}

/// A union, tagged `kind`, with exactly these variants and payloads (payloads in the
/// specification's spelling; a bare name is in this domain).
fn assert_union(model: &Value, name: &str, expected: &[(&str, &str)]) {
    let body = member(declared_type(model, name), &["body"]);
    assert_eq!(
        text(member(body, &["kind"])),
        "union",
        "{name} is not a union"
    );
    assert_eq!(text(member(body, &["tag"])), "kind", "{name} tag");
    let mut found: Vec<(String, String)> = object(member(body, &["variants"]))
        .iter()
        .map(|(variant, payload)| (variant.clone(), spelling(payload)))
        .collect();
    found.sort();
    let mut wanted: Vec<(String, String)> = expected
        .iter()
        .map(|(variant, payload)| (variant.to_string(), qualified_payload(payload)))
        .collect();
    wanted.sort();
    assert_eq!(found, wanted, "{name} variants");
}

fn qualified_payload(payload: &str) -> String {
    match payload
        .strip_prefix("List<")
        .and_then(|p| p.strip_suffix('>'))
    {
        Some(inner) => format!("List<{}>", qualified(inner)),
        None => qualified(payload),
    }
}

/// A struct with exactly these fields, in this order.
fn assert_struct(model: &Value, name: &str, expected: &[(&str, &str)]) {
    let body = member(declared_type(model, name), &["body"]);
    assert_eq!(
        text(member(body, &["kind"])),
        "struct",
        "{name} is not a struct"
    );
    let found: Vec<(String, String)> = array(member(body, &["fields"]))
        .iter()
        .map(|field| {
            (
                text(member(field, &["name"])).to_string(),
                spelling(member(field, &["type_ref"])),
            )
        })
        .collect();
    let wanted: Vec<(String, String)> = expected
        .iter()
        .map(|(field, ty)| (field.to_string(), qualified_payload(ty)))
        .collect();
    assert_eq!(found, wanted, "{name} fields");
}

fn assert_newtype(model: &Value, name: &str, of: &str) {
    let body = member(declared_type(model, name), &["body"]);
    assert_eq!(
        text(member(body, &["kind"])),
        "newtype",
        "{name} is not a newtype"
    );
    assert_eq!(spelling(member(body, &["of"])), of, "{name} wraps");
}

/// The fields of an entity, by name and spelling.
fn entity_fields(model: &Value, entity: &str) -> Vec<(String, String)> {
    array(member(
        model,
        &["entities", &format!("{NS}{entity}"), "fields"],
    ))
    .iter()
    .map(|field| {
        (
            text(member(field, &["name"])).to_string(),
            spelling(member(field, &["type_ref"])),
        )
    })
    .collect()
}

fn has_field(fields: &[(String, String)], name: &str, ty: &str) -> bool {
    fields.iter().any(|(n, t)| n == name && t == ty)
}

/// Every `trait` and `enum` item in a Rust source, at any depth.
fn traits_and_enums(source: &str) -> Vec<String> {
    use syn::visit::Visit;

    struct Scan(Vec<String>);
    impl<'ast> Visit<'ast> for Scan {
        fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
            self.0.push(format!("trait {}", item.ident));
            syn::visit::visit_item_trait(self, item);
        }
        fn visit_item_trait_alias(&mut self, item: &'ast syn::ItemTraitAlias) {
            self.0.push(format!("trait {}", item.ident));
            syn::visit::visit_item_trait_alias(self, item);
        }
        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            self.0.push(format!("enum {}", item.ident));
            syn::visit::visit_item_enum(self, item);
        }
    }

    let file = syn::parse_file(source).unwrap_or_else(|error| panic!("parse lib.rs: {error}"));
    let mut scan = Scan(Vec::new());
    scan.visit_file(&file);
    scan.0
}

/// The package names `cargo tree --prefix none` prints, one per line.
fn tree_packages(tree: &str) -> Vec<&str> {
    tree.lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect()
}

#[test]
fn skeleton_lands_port_vocabulary_and_modules() {
    let root = root();
    let model = compiled_model(&root);

    // 1. The port vocabulary, in the model `ess specify compile` prints.
    assert_enum(
        &model,
        "GovernorError",
        &["UnknownCase", "GovernorUnavailable"],
    );
    assert_newtype(&model, "Unit", "Boolean");
    assert_newtype(&model, "ProposedActionArguments", "Json");
    assert_newtype(&model, "HumanDecisionRequest", "Json");
    assert_union(
        &model,
        "CompletionDetermination",
        &[
            ("Open", "Unit"),
            ("Complete", "CompletionDeterminationComplete"),
        ],
    );
    assert_struct(
        &model,
        "CompletionDeterminationComplete",
        &[("outcome", "String")],
    );
    assert_union(
        &model,
        "SuspensionReason",
        &[
            ("Authority", "Json"),
            ("Human", "HumanDecisionRequest"),
            ("Evidence", "List<String>"),
            ("Time", "Json"),
            ("Dependency", "List<CaseId>"),
            ("Budget", "Json"),
            ("ExternalAvailability", "Json"),
        ],
    );
    assert_union(
        &model,
        "ExecutorOutcome",
        &[
            ("ProposedAction", "ExecutorOutcomeProposedAction"),
            ("NeedsHumanJudgment", "ExecutorOutcomeNeedsHumanJudgment"),
            ("Suspended", "ExecutorOutcomeSuspended"),
            ("NoUsefulAction", "Unit"),
            ("CompletedLocalReasoning", "Unit"),
        ],
    );
    assert_struct(
        &model,
        "ExecutorOutcomeProposedAction",
        &[
            ("action", "String"),
            ("arguments", "ProposedActionArguments"),
        ],
    );
    assert_struct(
        &model,
        "ExecutorOutcomeNeedsHumanJudgment",
        &[("request", "HumanDecisionRequest")],
    );
    assert_struct(
        &model,
        "ExecutorOutcomeSuspended",
        &[("reason", "SuspensionReason")],
    );
    assert_union(
        &model,
        "AuthorityVerdict",
        &[
            ("Allow", "Unit"),
            ("Deny", "AuthorityVerdictDeny"),
            ("ApprovalRequired", "AuthorityVerdictApprovalRequired"),
        ],
    );
    assert_struct(&model, "AuthorityVerdictDeny", &[("reason", "String")]);
    assert_struct(
        &model,
        "AuthorityVerdictApprovalRequired",
        &[("request", "String")],
    );
    assert_union(
        &model,
        "RunOutcome",
        &[
            ("Completed", "RunOutcomeCompleted"),
            ("Suspended", "RunOutcomeSuspended"),
            ("NeedsAuthority", "RunOutcomeNeedsAuthority"),
            ("NeedsHumanJudgment", "RunOutcomeNeedsHumanJudgment"),
            ("NeedsExternalEvidence", "RunOutcomeNeedsExternalEvidence"),
            ("NoAdmissibleAction", "Unit"),
        ],
    );
    assert_struct(&model, "RunOutcomeCompleted", &[("outcome", "String")]);
    assert_struct(
        &model,
        "RunOutcomeSuspended",
        &[("reason", "SuspensionReason")],
    );
    assert_struct(&model, "RunOutcomeNeedsAuthority", &[("request", "String")]);
    assert_struct(
        &model,
        "RunOutcomeNeedsHumanJudgment",
        &[("request", "HumanDecisionRequest")],
    );
    assert_struct(
        &model,
        "RunOutcomeNeedsExternalEvidence",
        &[("requirements", "List<String>")],
    );

    // 2. Observation and Evidence. Run's Suspended state and its transitions moved to
    //    story:run-outcomes (coordinator decision 4), so they are not checked here.
    let observation = entity_fields(&model, "Observation");
    assert!(
        has_field(&observation, "observed_at", "Timestamp")
            && has_field(&observation, "payload", "Json"),
        "Observation fields: {observation:?}"
    );
    assert!(
        member(&model, &["entities", &format!("{NS}Observation")])
            .member("relations")
            .is_none_or(|relations| array(relations).is_empty()),
        "Observation gained a relation"
    );
    let evidence_fields = entity_fields(&model, "Evidence");
    assert!(
        has_field(&evidence_fields, "facts", "Json")
            && has_field(&evidence_fields, "provenance", "Json"),
        "Evidence fields: {evidence_fields:?}"
    );
    let relations = array(member(
        &model,
        &["entities", &format!("{NS}Evidence"), "relations"],
    ));
    let observations = relations
        .iter()
        .find(|relation| relation.member("name").map(text) == Some("observations"))
        .expect("Evidence keeps its `observations` relation");
    let shape: Vec<(&str, &str)> = object(observations)
        .iter()
        .map(|(key, value)| (key.as_str(), text(value)))
        .collect();
    assert_eq!(
        shape,
        [
            ("name", "observations"),
            ("kind", "references"),
            ("target", "commission.responsibility.Observation"),
            ("cardinality", "many"),
            ("via", "observation_ids"),
        ],
        "Evidence.observations changed"
    );

    // 3. Each type of item 1 reaches this crate through b10x-commission's re-export, generated.
    let names = [
        std::any::type_name::<model::GovernorError>(),
        std::any::type_name::<model::Unit>(),
        std::any::type_name::<model::ProposedActionArguments>(),
        std::any::type_name::<model::HumanDecisionRequest>(),
        std::any::type_name::<model::CompletionDetermination>(),
        std::any::type_name::<model::CompletionDeterminationComplete>(),
        std::any::type_name::<model::SuspensionReason>(),
        std::any::type_name::<model::ExecutorOutcome>(),
        std::any::type_name::<model::ExecutorOutcomeProposedAction>(),
        std::any::type_name::<model::ExecutorOutcomeNeedsHumanJudgment>(),
        std::any::type_name::<model::ExecutorOutcomeSuspended>(),
        std::any::type_name::<model::AuthorityVerdict>(),
        std::any::type_name::<model::AuthorityVerdictDeny>(),
        std::any::type_name::<model::AuthorityVerdictApprovalRequired>(),
        std::any::type_name::<model::RunOutcome>(),
        std::any::type_name::<model::RunOutcomeCompleted>(),
        std::any::type_name::<model::RunOutcomeSuspended>(),
        std::any::type_name::<model::RunOutcomeNeedsAuthority>(),
        std::any::type_name::<model::RunOutcomeNeedsHumanJudgment>(),
        std::any::type_name::<model::RunOutcomeNeedsExternalEvidence>(),
    ];
    for name in names {
        assert!(
            name.starts_with("commission::"),
            "{name} is not the generated type"
        );
    }

    // 5. The real dependency tree of b10x-commission names no b10x-canon.
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(&root)
        .args([
            "tree",
            "--locked",
            "-p",
            "b10x-commission",
            "-e",
            "normal",
            "--prefix",
            "none",
        ])
        .output()
        .unwrap_or_else(|error| panic!("run `cargo tree`: {error}"));
    let tree = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "`cargo tree` failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let packages = tree_packages(&tree);
    assert!(
        packages.contains(&"b10x-commission"),
        "`cargo tree` did not print b10x-commission:\n{tree}"
    );
    assert!(
        !packages.contains(&"b10x-canon"),
        "b10x-commission still depends on b10x-canon:\n{tree}"
    );

    // 6. The crate root defines no trait and no enum.
    let lib = root.join("crates/commission/src/lib.rs");
    let source = std::fs::read_to_string(&lib)
        .unwrap_or_else(|error| panic!("read {}: {error}", lib.display()));
    let found = traits_and_enums(&source);
    assert!(found.is_empty(), "{} defines {found:?}", lib.display());
}
