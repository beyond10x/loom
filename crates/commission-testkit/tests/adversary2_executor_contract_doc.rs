//! Adversary pass 2 on `story:agent-executor-port`: `docs/contracts/commission-executor.md`, read
//! as the contract it claims to be, against the generated `ExecutorOutcome` the port returns.
//!
//! The unit rewrote the doc's trait block to "the port as built" and left its outcome block and
//! its prose about `Suspended` alone. Nothing else compares the doc with the model, so these cases
//! do. Both files are read at run time (`CARGO_MANIFEST_DIR`).

use std::path::PathBuf;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn read(relative: &str) -> String {
    let path = root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

const DOC: &str = "docs/contracts/commission-executor.md";
const MODEL: &str = "generated/rust/commission/src/responsibility.rs";

/// The fenced ```rust blocks of a Markdown text, with the 1-based line each starts on.
fn rust_blocks(markdown: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, String)> = None;
    for (index, line) in markdown.lines().enumerate() {
        let fence = line.trim_start().starts_with("```");
        match open.take() {
            None if fence && line.trim() == "```rust" => open = Some((index + 1, String::new())),
            None => {}
            Some(block) if fence => blocks.push(block),
            Some((start, mut body)) => {
                body.push_str(line);
                body.push('\n');
                open = Some((start, body));
            }
        }
    }
    blocks
}

/// The last path segment of a type, as written (`commission::Foo` is `Foo`).
fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default(),
        _ => "<not a path type>".to_owned(),
    }
}

/// A variant's shape: `unit`, `tuple(T, ..)` or `{ name: T, .. }`.
fn shape(fields: &syn::Fields) -> String {
    match fields {
        syn::Fields::Unit => "unit".to_owned(),
        syn::Fields::Unnamed(unnamed) => format!(
            "tuple({})",
            unnamed
                .unnamed
                .iter()
                .map(|field| type_name(&field.ty))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        syn::Fields::Named(named) => format!(
            "{{ {} }}",
            named
                .named
                .iter()
                .map(|field| format!(
                    "{}: {}",
                    field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    type_name(&field.ty)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn find_enum<'a>(file: &'a syn::File, name: &str) -> Option<&'a syn::ItemEnum> {
    file.items.iter().find_map(|item| match item {
        syn::Item::Enum(item) if item.ident == name => Some(item),
        _ => None,
    })
}

fn find_struct<'a>(file: &'a syn::File, name: &str) -> Option<&'a syn::ItemStruct> {
    file.items.iter().find_map(|item| match item {
        syn::Item::Struct(item) if item.ident == name => Some(item),
        _ => None,
    })
}

/// What a variant carries, with a named payload struct looked up in `model`: `{ a: T, .. }` for
/// fields (inline or in the payload struct), `tuple(T)` for a payload that is not a model struct,
/// `unit` for nothing.
fn carried(model: &syn::File, fields: &syn::Fields) -> String {
    match fields {
        syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
            let payload = type_name(&unnamed.unnamed[0].ty);
            match find_struct(model, &payload) {
                Some(item) if matches!(item.fields, syn::Fields::Named(_)) => shape(&item.fields),
                _ => format!("tuple({payload})"),
            }
        }
        other => shape(other),
    }
}

/// The generated `ExecutorOutcome`: `(variant, what it carries)`, sorted by variant.
fn generated_outcome(model: &syn::File) -> Vec<(String, String)> {
    let outcome = find_enum(model, "ExecutorOutcome")
        .unwrap_or_else(|| panic!("{MODEL} declares no enum ExecutorOutcome"));
    let mut variants: Vec<(String, String)> = outcome
        .variants
        .iter()
        .map(|variant| (variant.ident.to_string(), carried(model, &variant.fields)))
        .collect();
    variants.sort();
    variants
}

/// The doc's `pub enum ExecutorOutcome` sketch, with the line its block starts on.
fn documented_outcome() -> (usize, syn::File) {
    let doc = read(DOC);
    rust_blocks(&doc)
        .into_iter()
        .find_map(|(line, body)| {
            let file = syn::parse_file(&body).ok()?;
            find_enum(&file, "ExecutorOutcome")
                .is_some()
                .then_some((line, file))
        })
        .unwrap_or_else(|| panic!("{DOC} shows no `enum ExecutorOutcome` in a ```rust block"))
}

/// The doc's outcome block must describe the outcome the port returns: the same variants, each
/// carrying what the generated variant carries, under the same field names and types. Today the
/// block (`commission-executor.md:30-49`) is the pre-generation sketch: `ProposedAction` has
/// `action: ActionId` and `arguments_json: String` where the model has `action: String` and
/// `arguments: ProposedActionArguments`, every variant is written in a shape (`{ .. }` or unit) that
/// does not construct or match the generated tuple variant, and `ActionId` is not a model type.
#[test]
fn adversary2_contract_doc_outcome_block_matches_generated_executor_outcome() {
    let model =
        syn::parse_file(&read(MODEL)).unwrap_or_else(|error| panic!("parse {MODEL}: {error}"));
    let generated = generated_outcome(&model);

    let (line, documented_file) = documented_outcome();
    let documented = find_enum(&documented_file, "ExecutorOutcome")
        .unwrap_or_else(|| panic!("{DOC}:{line} lost its enum"));
    let mut mismatches = Vec::new();
    for variant in &documented.variants {
        let name = variant.ident.to_string();
        let Some((_, wanted)) = generated.iter().find(|(generated, _)| *generated == name) else {
            mismatches.push(format!("{name}: no such generated variant"));
            continue;
        };
        // The doc may write a payload inline (`{ field: T }`) or by name (`(Payload)`); what it
        // carries must be what the model's variant carries, field names and types included.
        let documented_carries = carried(&model, &variant.fields);
        if &documented_carries != wanted {
            mismatches.push(format!(
                "{name}: doc carries {documented_carries}, generated carries {wanted}"
            ));
        }
        // And it must be written so that it builds: every generated variant is a tuple variant.
        let generated_is_tuple = find_enum(&model, "ExecutorOutcome")
            .and_then(|item| item.variants.iter().find(|v| v.ident == name))
            .is_some_and(|v| matches!(v.fields, syn::Fields::Unnamed(_)));
        if generated_is_tuple && !matches!(variant.fields, syn::Fields::Unnamed(_)) {
            mismatches.push(format!(
                "{name}: doc writes `{}`, which neither builds nor matches the generated tuple variant",
                shape(&variant.fields)
            ));
        }
    }
    let mut documented_names: Vec<String> = documented
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect();
    documented_names.sort();
    let generated_names: Vec<String> = generated.iter().map(|(name, _)| name.clone()).collect();
    if documented_names != generated_names {
        mismatches.push(format!(
            "variant set: doc {documented_names:?}, generated {generated_names:?}"
        ));
    }

    assert!(
        mismatches.is_empty(),
        "{DOC}:{line} `enum ExecutorOutcome` is not the generated union the port returns \
         ({MODEL}):\n  {}",
        mismatches.join("\n  ")
    );
}

/// Every type the doc's Rust blocks name is a type the model or `std` declares: a reader who
/// copies the sketch must be able to resolve it. `ActionId` (`commission-executor.md:33`) is
/// declared nowhere in the generated model.
#[test]
fn adversary2_contract_doc_names_only_types_that_exist() {
    let model_source = read(MODEL);
    let model =
        syn::parse_file(&model_source).unwrap_or_else(|error| panic!("parse {MODEL}: {error}"));
    let declared: Vec<String> = model
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Struct(item) => Some(item.ident.to_string()),
            syn::Item::Enum(item) => Some(item.ident.to_string()),
            syn::Item::Type(item) => Some(item.ident.to_string()),
            syn::Item::Mod(item) => Some(item.ident.to_string()),
            _ => None,
        })
        .collect();
    let prelude = ["String", "Vec", "Option", "Self"];

    let doc = read(DOC);
    let mut unknown = Vec::new();
    for (line, body) in rust_blocks(&doc) {
        let Ok(file) = syn::parse_file(&body) else {
            continue;
        };
        for item in &file.items {
            let syn::Item::Enum(item) = item else {
                continue;
            };
            for variant in &item.variants {
                for field in &variant.fields {
                    let name = type_name(&field.ty);
                    if !prelude.contains(&name.as_str()) && !declared.contains(&name) {
                        unknown.push(format!(
                            "block at {DOC}:{line}: {}::{} names `{name}`",
                            item.ident, variant.ident
                        ));
                    }
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "the contract doc names types the generated model does not declare:\n  {}",
        unknown.join("\n  ")
    );
}

/// The prose outside the code blocks writes outcomes too. Every generated `ExecutorOutcome`
/// variant is a tuple variant, so `Variant { .. }` neither builds nor matches one; the doc's own
/// failure rule (`commission-executor.md:26`, "returns `Suspended { reason: .. }`") is written that
/// way.
#[test]
fn adversary2_contract_doc_prose_writes_outcomes_in_the_generated_shape() {
    let model =
        syn::parse_file(&read(MODEL)).unwrap_or_else(|error| panic!("parse {MODEL}: {error}"));
    let tuple_variants: Vec<String> = find_enum(&model, "ExecutorOutcome")
        .unwrap_or_else(|| panic!("{MODEL} declares no enum ExecutorOutcome"))
        .variants
        .iter()
        .filter(|variant| matches!(variant.fields, syn::Fields::Unnamed(_)))
        .map(|variant| variant.ident.to_string())
        .collect();
    assert_eq!(tuple_variants.len(), 5, "the generated union changed shape");

    let doc = read(DOC);
    let mut in_block = false;
    let mut wrong = Vec::new();
    for (index, line) in doc.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            continue;
        }
        for variant in &tuple_variants {
            if line.contains(&format!("`{variant} {{")) || line.contains(&format!("::{variant} {{"))
            {
                wrong.push(format!("{DOC}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the contract doc's prose writes a generated tuple variant with struct braces:\n  {}",
        wrong.join("\n  ")
    );
}
