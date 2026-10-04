//! Adversary pass 2 on `story:stale-revision-action-request`: the compiled ESS model against the
//! story's decisions.
//!
//! `action_request_revalidation` checks only the `AuthorityDecision` relation (expectation 7). The
//! story's `## ESS first` also decides `Run.requests` (owns, many, via `run_id`) and
//! `ActionRequest.case` (references, one, via `case_id`), and the request's fields. Deleting either
//! relation from `ess/` and regenerating leaves every gate step green, so nothing pinned them.
//! `adversary2_request_relations_match_the_story` pins them and proves, on a mutated copy, that it
//! can fail.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`).

use b10x_commission::model::json::{self, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const NS: &str = "commission.responsibility.";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn compile(spec: &Path) -> Value {
    let run = Command::new("ess")
        .args(["specify", "compile", "--path"])
        .arg(spec)
        .args(["--format", "json"])
        .output()
        .unwrap_or_else(|error| panic!("run `ess specify compile`: {error}"));
    assert!(
        run.status.success(),
        "`ess specify compile` failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    json::parse(&String::from_utf8_lossy(&run.stdout))
        .unwrap_or_else(|error| panic!("compiled model is not JSON: {error:?}"))
}

fn text(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn array<'a>(value: Option<&'a Value>, what: &str) -> &'a [Value] {
    match value {
        Some(Value::Array(items)) => items,
        other => panic!("{what}: {other:?}"),
    }
}

fn entity<'a>(model: &'a Value, name: &str) -> Result<&'a Value, String> {
    model
        .member("entities")
        .and_then(|entities| entities.member(&format!("{NS}{name}")))
        .ok_or_else(|| format!("the compiled model declares no {NS}{name}"))
}

fn has_relation(
    model: &Value,
    owner: &str,
    kind: &str,
    target: &str,
    cardinality: &str,
    via: &str,
) -> Result<(), String> {
    let relations = match entity(model, owner)?.member("relations") {
        Some(Value::Array(relations)) => relations.as_slice(),
        _ => &[],
    };
    let found = relations.iter().any(|relation| {
        text(relation.member("kind")) == Some(kind)
            && text(relation.member("target")) == Some(&format!("{NS}{target}"))
            && text(relation.member("cardinality")) == Some(cardinality)
            && text(relation.member("via")) == Some(via)
    });
    if found {
        Ok(())
    } else {
        Err(format!(
            "{owner} has no `{kind}` relation to {cardinality} {target} via `{via}`: {relations:?}"
        ))
    }
}

/// The field `name` of `owner`, as its declared type name (`kind` `declared`), or a failure.
fn declared_field(model: &Value, owner: &str, name: &str, ty: &str) -> Result<(), String> {
    let fields = array(entity(model, owner)?.member("fields"), "fields");
    let field = fields
        .iter()
        .find(|field| text(field.member("name")) == Some(name))
        .ok_or_else(|| format!("{owner} has no field `{name}`"))?;
    let type_ref = field
        .member("type_ref")
        .ok_or_else(|| format!("{owner}.{name} has no type_ref"))?;
    if text(type_ref.member("kind")) == Some("declared")
        && text(type_ref.member("name")) == Some(&format!("{NS}{ty}"))
    {
        Ok(())
    } else {
        Err(format!(
            "{owner}.{name} is not exactly {NS}{ty}: {type_ref:?}"
        ))
    }
}

/// Every relation and identity-bearing field the story's `## ESS first` decides.
fn story_decisions(model: &Value) -> Result<(), String> {
    has_relation(model, "Run", "owns", "ActionRequest", "many", "run_id")?;
    has_relation(
        model,
        "ActionRequest",
        "references",
        "Case",
        "one",
        "case_id",
    )?;
    has_relation(
        model,
        "AuthorityDecision",
        "references",
        "ActionRequest",
        "one",
        "action_request_id",
    )?;
    declared_field(model, "ActionRequest", "run_id", "RunId")?;
    declared_field(model, "ActionRequest", "case_id", "CaseId")?;
    declared_field(
        model,
        "ActionRequest",
        "arguments",
        "ProposedActionArguments",
    )?;
    // Not optional: a decision always names the one request it covers.
    declared_field(
        model,
        "AuthorityDecision",
        "action_request_id",
        "ActionRequestId",
    )?;
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|error| panic!("create {}: {error}", to.display()));
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display()))
    {
        let entry = entry.unwrap_or_else(|error| panic!("{}: {error}", from.display()));
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", entry.path().display()));
        }
    }
}

#[test]
fn adversary2_request_relations_match_the_story() {
    let model = compile(&root().join("ess"));
    if let Err(problem) = story_decisions(&model) {
        panic!("{problem}");
    }

    // The check can fail: a copy of `ess/` without `Run.requests` compiles and is refused.
    let scratch =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("adversary2_request_relations_mutant");
    if scratch.exists() {
        std::fs::remove_dir_all(&scratch)
            .unwrap_or_else(|error| panic!("clear {}: {error}", scratch.display()));
    }
    copy_tree(&root().join("ess"), &scratch);
    let domain = scratch.join("domains/responsibility.yaml");
    let source = std::fs::read_to_string(&domain)
        .unwrap_or_else(|error| panic!("read {}: {error}", domain.display()));
    let relation = "      - name: requests\n        kind: owns\n        target: \
                    commission.responsibility.ActionRequest\n        cardinality: many\n        \
                    via: run_id\n";
    assert!(
        source.contains(relation),
        "precondition: Run.requests is in the copy"
    );
    let header = "    relations:\n      # A request is made inside one run";
    let mutated = source.replacen(relation, "", 1);
    // Drop the now-empty `relations:` key with its comment, so the copy still validates.
    let mutated = match mutated.find(header) {
        Some(start) => {
            let end = mutated[start..]
                .find("    # Resuming after a restart")
                .map(|offset| start + offset)
                .unwrap_or_else(|| panic!("precondition: Run lifecycle comment not found"));
            format!("{}{}", &mutated[..start], &mutated[end..])
        }
        None => panic!("precondition: Run.requests header not found"),
    };
    std::fs::write(&domain, mutated)
        .unwrap_or_else(|error| panic!("write {}: {error}", domain.display()));
    let mutant = compile(&scratch);
    assert!(
        story_decisions(&mutant).is_err(),
        "the check passed a model without Run.requests"
    );
    std::fs::remove_dir_all(&scratch)
        .unwrap_or_else(|error| panic!("clear {}: {error}", scratch.display()));
}

/// `ess/domains/responsibility.yaml` says of `ActionRequest`: "Revalidation takes the request as
/// its input". The request's identity is `action_request_id`, and `AuthorityDecision` reaches a
/// request only through it. The command's input drops it, so a `needs-authority` outcome of the
/// command cannot say which request needs the decision.
#[test]
fn adversary2_request_revalidation_input_is_the_request() {
    let model = compile(&root().join("ess"));
    let command = model
        .member("commands")
        .and_then(|commands| commands.member(&format!("{NS}RevalidateActionRequest")))
        .unwrap_or_else(|| panic!("the compiled model declares no RevalidateActionRequest"));
    let input: Vec<&str> = array(command.member("input"), "input")
        .iter()
        .filter_map(|field| text(field.member("name")))
        .collect();
    let request: Vec<&str> = std::iter::once("action_request_id")
        .chain(
            array(
                entity(&model, "ActionRequest")
                    .unwrap_or_else(|problem| panic!("{problem}"))
                    .member("fields"),
                "fields",
            )
            .iter()
            .filter_map(|field| text(field.member("name"))),
        )
        .collect();
    let missing: Vec<&&str> = request
        .iter()
        .filter(|name| !input.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "RevalidateActionRequest's input is not the request: it lacks {missing:?} (input \
         {input:?}, request {request:?})"
    );
}
