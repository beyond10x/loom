//! The `LoomAgent` `CustomResourceDefinition`, projected from Loom's specification.
//!
//! [`manifest`] takes the two JSON Schema documents `ess generate --path ess --kind schema` writes,
//! [`SPEC_SCHEMA`] (`loom.resource.LoomAgentSpec`, which holds `loom.agent.AgentSpec` verbatim) and
//! [`ENTITY_SCHEMA`] (the entity `loom.resource.LoomAgent`), and returns the YAML of one
//! `apiextensions.k8s.io/v1` `CustomResourceDefinition` (group [`GROUP`], version [`VERSION`],
//! kind [`KIND`], namespaced, with a status subresource) whose `openAPIV3Schema` is `spec` from
//! the spec type and `status` from the entity's fields other than its identity [`IDENTITY`].
//! `loom-xtask crd-generate` writes it to [`FILE`] and `loom-xtask crd-drift` compares it.
//!
//! The projection makes the schema structural, as the Kubernetes API server requires. Every keyword
//! of a schema node is carried, rewritten to an equivalent, dropped as an annotation or refused by
//! name; none is passed over unexamined:
//!
//! * carried as written: the keywords of `CARRIED` (`type`, `enum`, `required`, `format`,
//!   `pattern`, `description` and the numeric, length and item bounds), except a `format` of
//!   `MISREAD_FORMATS`; every member the specification declares required is required, except a
//!   list the `loom.agent` document declares, which its decoder reads as empty when absent
//!   (`ABSENT_LIST_IS_EMPTY`);
//! * dropped, annotations only ESS reads: `$defs`, `$schema`, `$id`, `title`, `x-ess-name`,
//!   `x-ess-field`, `x-ess-provenance`, an `x-ess-kind` of `struct`, `newtype`, `enum` or
//!   `entity`, and an `x-ess-map-key` of `String` (a JSON object's keys are text);
//! * every `$ref` into `$defs` is inlined, recursively; a reference reached again while it is being
//!   inlined is refused as [`CrdError::Cycle`], naming the type;
//! * `const: x` becomes `enum: [x]`;
//! * `additionalProperties: false` is removed, the one form the API server refuses (it prunes an
//!   undeclared member instead); `additionalProperties: true` becomes
//!   `x-kubernetes-preserve-unknown-fields: true`; an `additionalProperties` schema (ESS `Map`) is
//!   projected and kept, so a map keeps its entries, and refused beside `properties`;
//! * an ESS `Json` (an empty schema, or one holding only a `description`; here `spec.routes`)
//!   becomes an untyped node that preserves unknown fields, wherever it stands, so it admits any
//!   JSON value as the specification does (a structural schema allows a node without a `type`
//!   when it preserves unknown fields);
//! * a `Decimal` (`type: string`, `format: decimal`) becomes `type: number`: the agent document's
//!   decoder (`loom_sdk::agent::parse`) reads a decimal written as a number
//!   (`classify_threshold: 0.5`), and Kubernetes has no number-or-string type, so a decimal
//!   written as a quoted string, which the decoder also reads, is refused by the API server;
//! * a tagged union (`x-ess-kind: union`, projected by ESS as a `oneOf` of `{<tag>: {const},
//!   value: {$ref}}`) is flattened to the wire form Loom's decoder reads, the tag beside the
//!   variant's fields: `properties` holds the tag, with an `enum` of every variant, and the union
//!   of every variant's fields; `required` holds the tag and the fields every variant requires;
//!   `oneOf` holds one entry per variant: the tag's one-value `enum`, that variant's `required`,
//!   and `not: {anyOf: [{required: [<f>]}, …]}` over every field another variant declares and it
//!   does not, so the flattened union admits exactly what one of its variants admits. Two variants
//!   declaring one field with different schemas are refused as [`CrdError::UnionConflict`],
//!   naming the field. A variant whose value is not a struct (a map, a `Json`, a scalar newtype)
//!   is refused as [`CrdError::Union`], naming the union and the variant, since the flattened
//!   form carries only declared members;
//! * a `format` the API server reads otherwise than ESS writes it is refused as
//!   [`CrdError::Keyword`], naming the field and the format: `duration`, which ESS writes for an
//!   ISO 8601 duration and the API server reads as a Go duration (`date-time` is RFC 3339 on both
//!   sides, and carried);
//! * anything else, an `x-ess-*` key not listed above included, is refused as
//!   [`CrdError::Keyword`], naming it.
//!
//! What the schema then holds of the agent document: a member it does not declare is pruned by the
//! API server, not refused (a client sends `fieldValidation=Strict` to have it refused); a declared
//! member of the wrong type, or of another variant of a union, is refused by the schema; what
//! reaches the pod is a document the schema admitted, which `b10x-loom agent check` reads again.
//! A member the schema lets a document omit is one the decoder lets it omit too; the decoder's
//! checks the specification does not declare (the `format` value, port ranges, non-blank text,
//! variable names, RFC 3339 instants) are the decoder's alone.
//!
//! The finished document is checked by [`violations`] before it is written. The YAML has sorted
//! keys, starts with [`HEADER`] and ends with one newline, so one specification gives one file.

use serde_json::{Map, Value, json};

/// The API group (design decision row 2).
pub const GROUP: &str = "loom.beyond10x.dev";
/// The one served and stored version.
pub const VERSION: &str = "v1alpha1";
/// The resource's kind.
pub const KIND: &str = "LoomAgent";
/// The list's kind.
pub const LIST_KIND: &str = "LoomAgentList";
/// The plural resource name.
pub const PLURAL: &str = "loomagents";
/// The singular resource name.
pub const SINGULAR: &str = "loomagent";
/// The definition's own name, `<plural>.<group>`.
pub const NAME: &str = "loomagents.loom.beyond10x.dev";
/// Where the manifest is committed, relative to the repository root.
pub const FILE: &str = "deploy/crd/loomagents.loom.beyond10x.dev.yaml";
/// The spec type's document, relative to the directory `ess generate --kind schema` writes.
pub const SPEC_SCHEMA: &str = "schema/types/loom.resource.LoomAgentSpec.schema.json";
/// The entity's document, relative to the directory `ess generate --kind schema` writes.
pub const ENTITY_SCHEMA: &str = "schema/entities/loom.resource.LoomAgent.schema.json";
/// The first line of the manifest.
pub const HEADER: &str = "# generated by loom-xtask crd-generate from ess/; do not edit\n";
/// The entity's identity field: the resource's `metadata.name`, so not part of its status.
pub const IDENTITY: &str = "name";

/// Where a `$ref` into the document's own definitions starts.
const DEFINITIONS: &str = "#/$defs/";

/// Annotations only ESS reads, dropped wherever they stand as a keyword.
const ANNOTATIONS: [&str; 7] = [
    "$defs",
    "$schema",
    "$id",
    "title",
    "x-ess-name",
    "x-ess-field",
    "x-ess-provenance",
];

/// The `x-ess-kind`s that only annotate a node the other keywords already describe; `union` is
/// flattened, and any other kind is refused.
const ANNOTATED_KINDS: [&str; 4] = ["struct", "newtype", "enum", "entity"];

/// The one `x-ess-map-key` a structural schema states without a keyword: a JSON object's keys are
/// text.
const TEXT_KEY: &str = "String";

/// Keywords carried over as they are.
const CARRIED: [&str; 12] = [
    "type",
    "enum",
    "required",
    "format",
    "pattern",
    "description",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
];

/// The `format`s the API server reads otherwise than ESS writes them, refused by name: ESS writes
/// `duration` for an ISO 8601 duration, which the API server reads as a Go duration.
const MISREAD_FORMATS: [&str; 1] = ["duration"];

/// The extension that lets a structural schema node hold members it does not declare, or, on a
/// node without a `type`, any JSON value.
const PRESERVE_UNKNOWN: &str = "x-kubernetes-preserve-unknown-fields";

/// Keys no projected document holds as a keyword; nor does it hold an `x-ess-*` key or an
/// `additionalProperties` that is not a schema.
const FORBIDDEN: [&str; 6] = ["$ref", "$defs", "$schema", "$id", "const", "title"];

/// What a structural schema does not set inside `allOf`, `anyOf`, `oneOf` or `not`.
const NOT_IN_A_JUNCTOR: [&str; 5] = [
    "type",
    "description",
    "default",
    "additionalProperties",
    "nullable",
];

/// The junctors of a structural schema.
const JUNCTORS: [&str; 4] = ["oneOf", "anyOf", "allOf", "not"];

/// The specification documents, by domain, whose decoder reads an absent list as empty: a
/// `List<…>` member such a document declares required is not required in the CRD. The leniency
/// is the agent document's decoder's (`crates/loom-sdk/src/agent.rs`, `Members::list`); a list
/// another document declares stays required, as its decoder requires it (`model.roles`, from the
/// models document). story:agent-spec-constraints-in-ess makes it a declaration in ESS, which
/// replaces this table.
const ABSENT_LIST_IS_EMPTY: [&str; 1] = ["loom.agent"];

/// Whether the definition `name` (`<domain>.<Type>`) is one of an `ABSENT_LIST_IS_EMPTY`
/// document.
fn absent_list_is_empty(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(domain, _)| ABSENT_LIST_IS_EMPTY.contains(&domain))
}

/// Whether the projected node `node` preserves unknown fields.
fn preserves_unknown(node: &Map<String, Value>) -> bool {
    node.get(PRESERVE_UNKNOWN) == Some(&Value::Bool(true))
}

/// What the projected node `node` holds when it is not a struct, an object of declared members
/// only; `None` for a struct.
fn not_a_struct(node: &Map<String, Value>) -> Option<String> {
    let kind = node.get("type").and_then(Value::as_str);
    if preserves_unknown(node) {
        return Some(match kind {
            None => "any JSON value".to_owned(),
            Some(_) => "an object of any members".to_owned(),
        });
    }
    if node.contains_key("additionalProperties") {
        return Some("a map".to_owned());
    }
    match kind {
        Some("object") => None,
        Some(other) => Some(format!("a `{other}`")),
        None => Some("an untyped value".to_owned()),
    }
}

/// Takes out of the object `projected`'s `required` every member whose projected schema is a list.
fn require_no_list(projected: &mut Map<String, Value>) {
    let lists: Vec<String> = projected
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(_, schema)| schema.get("type").and_then(Value::as_str) == Some("array"))
        .map(|(name, _)| name.clone())
        .collect();
    let Some(Value::Array(required)) = projected.get_mut("required") else {
        return;
    };
    required.retain(|member| {
        !member
            .as_str()
            .is_some_and(|name| lists.iter().any(|l| l == name))
    });
    if required.is_empty() {
        projected.remove("required");
    }
}

/// Why a projection was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrdError {
    /// A `$ref` that does not point into the document's own `$defs`.
    ForeignReference {
        /// Where it stands.
        at: String,
        /// The reference as written.
        reference: String,
    },
    /// A `$ref` naming no definition.
    UnknownReference {
        /// Where it stands.
        at: String,
        /// The definition it names.
        reference: String,
    },
    /// A definition reached again while it is being inlined.
    Cycle {
        /// The definition.
        reference: String,
    },
    /// Two variants of a union declare one field with different schemas.
    UnionConflict {
        /// The union.
        union: String,
        /// The field.
        property: String,
    },
    /// A union not in the shape ESS projects one.
    Union {
        /// The union.
        union: String,
        /// What is wrong with it.
        detail: String,
    },
    /// A keyword the projection does not carry into a structural schema.
    Keyword {
        /// Where it stands.
        at: String,
        /// The keyword.
        keyword: String,
    },
    /// A schema node the projection cannot read.
    Shape {
        /// Where it stands.
        at: String,
        /// What is wrong with it.
        detail: String,
    },
    /// The projected document breaks a rule of a structural schema.
    NotStructural {
        /// Each broken rule, with where.
        violations: Vec<String>,
    },
    /// The document could not be written as YAML.
    Yaml(String),
}

impl std::fmt::Display for CrdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignReference { at, reference } => {
                write!(f, "{at}: `$ref` `{reference}` does not point into `$defs`")
            }
            Self::UnknownReference { at, reference } => {
                write!(
                    f,
                    "{at}: `$ref` names `{reference}`, which `$defs` does not define"
                )
            }
            Self::Cycle { reference } => write!(
                f,
                "`{reference}` refers to itself, so it cannot be inlined into a structural schema"
            ),
            Self::UnionConflict { union, property } => write!(
                f,
                "the variants of union `{union}` declare `{property}` with different schemas, so \
                 it cannot stand beside the tag once"
            ),
            Self::Union { union, detail } => write!(f, "union `{union}`: {detail}"),
            Self::Keyword { at, keyword } => write!(
                f,
                "{at}: keyword `{keyword}` has no projection into a structural schema"
            ),
            Self::Shape { at, detail } => write!(f, "{at}: {detail}"),
            Self::NotStructural { violations } => write!(
                f,
                "the projected document is not a structural schema: {}",
                violations.join("; ")
            ),
            Self::Yaml(detail) => write!(f, "write the manifest as YAML: {detail}"),
        }
    }
}

impl std::error::Error for CrdError {}

/// The YAML of the `CustomResourceDefinition` projected from `spec_schema` ([`SPEC_SCHEMA`]) and
/// `entity_schema` ([`ENTITY_SCHEMA`]), starting with [`HEADER`].
///
/// # Errors
///
/// A [`CrdError`] naming the reference, union, keyword or node the projection cannot carry, or the
/// rule the projected document breaks.
pub fn manifest(spec_schema: &Value, entity_schema: &Value) -> Result<String, CrdError> {
    let document = document(spec_schema, entity_schema)?;
    let found = violations(&document);
    if !found.is_empty() {
        return Err(CrdError::NotStructural { violations: found });
    }
    let text = serde_yaml_ng::to_string(&yaml(&document))
        .map_err(|error| CrdError::Yaml(error.to_string()))?;
    Ok(format!("{HEADER}{}\n", text.trim_end()))
}

/// The `CustomResourceDefinition` projected from `spec_schema` and `entity_schema`, as a value.
///
/// # Errors
///
/// As [`manifest`], apart from the structural check and the YAML.
pub fn document(spec_schema: &Value, entity_schema: &Value) -> Result<Value, CrdError> {
    Ok(json!({
        "apiVersion": "apiextensions.k8s.io/v1",
        "kind": "CustomResourceDefinition",
        "metadata": {"name": NAME},
        "spec": {
            "group": GROUP,
            "names": {
                "kind": KIND,
                "listKind": LIST_KIND,
                "plural": PLURAL,
                "singular": SINGULAR,
            },
            "scope": "Namespaced",
            "versions": [{
                "name": VERSION,
                "served": true,
                "storage": true,
                "subresources": {"status": {}},
                "additionalPrinterColumns": [
                    {"name": "State", "type": "string", "jsonPath": ".status.state"},
                    {"name": "Generation", "type": "integer", "jsonPath": ".status.observedGeneration"},
                    {"name": "Age", "type": "date", "jsonPath": ".metadata.creationTimestamp"},
                ],
                "schema": {"openAPIV3Schema": open_api_schema(spec_schema, entity_schema)?},
            }],
        },
    }))
}

/// The `openAPIV3Schema`: `spec` from the spec type, required; `status` from the entity without
/// its identity.
///
/// # Errors
///
/// As [`document`].
pub fn open_api_schema(spec_schema: &Value, entity_schema: &Value) -> Result<Value, CrdError> {
    let spec = Projector::over(spec_schema).schema(spec_schema, "spec")?;
    let mut status = Projector::over(entity_schema).schema(entity_schema, "status")?;
    let Some(node) = status.as_object_mut() else {
        return Err(shape("status", "the entity does not project to an object"));
    };
    if let Some(properties) = node.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove(IDENTITY);
    }
    if let Some(required) = node.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|name| name.as_str() != Some(IDENTITY));
    }
    for (part, projected) in [("spec", &spec), ("status", &status)] {
        if projected.get("type") != Some(&Value::from("object")) {
            return Err(shape(part, "does not project to an object"));
        }
    }
    Ok(json!({
        "type": "object",
        "properties": {"spec": spec, "status": status},
        "required": ["spec"],
    }))
}

fn shape(at: &str, detail: &str) -> CrdError {
    CrdError::Shape {
        at: at.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Inlines one document's definitions into structural schema nodes.
struct Projector<'d> {
    definitions: Option<&'d Map<String, Value>>,
    inlining: Vec<String>,
}

impl<'d> Projector<'d> {
    fn over(document: &'d Value) -> Self {
        Self {
            definitions: document.get("$defs").and_then(Value::as_object),
            inlining: Vec::new(),
        }
    }

    /// One schema node, projected.
    fn schema(&mut self, node: &Value, at: &str) -> Result<Value, CrdError> {
        let Some(map) = node.as_object() else {
            return Err(shape(at, "a schema that is not an object"));
        };
        if let Some(reference) = map.get("$ref") {
            return self.reference(map, reference, at);
        }
        if map.get("x-ess-kind").and_then(Value::as_str) == Some("union") {
            return self.union(map, at);
        }
        let mut projected = self.keywords(map, at)?;
        if projected.keys().all(|key| key == "description") {
            // ESS `Json`: any JSON value.
            projected.insert(PRESERVE_UNKNOWN.to_owned(), Value::Bool(true));
            return Ok(Value::Object(projected));
        }
        if projected.get("format").and_then(Value::as_str) == Some("decimal") {
            projected.remove("format");
            projected.remove("pattern");
            projected.insert("type".to_owned(), Value::from("number"));
        }
        if !projected.contains_key("type") && !preserves_unknown(&projected) {
            return Err(shape(
                at,
                "a schema with neither a type, a reference nor unknown fields preserved",
            ));
        }
        Ok(Value::Object(projected))
    }

    /// A `$ref`, inlined, with whatever stands beside it projected over it.
    fn reference(
        &mut self,
        map: &Map<String, Value>,
        reference: &Value,
        at: &str,
    ) -> Result<Value, CrdError> {
        let written = reference.as_str().unwrap_or_default();
        let Some(name) = written.strip_prefix(DEFINITIONS) else {
            return Err(CrdError::ForeignReference {
                at: at.to_owned(),
                reference: written.to_owned(),
            });
        };
        if self.inlining.iter().any(|open| open == name) {
            return Err(CrdError::Cycle {
                reference: name.to_owned(),
            });
        }
        let Some(definition) = self.definitions.and_then(|all| all.get(name)) else {
            return Err(CrdError::UnknownReference {
                at: at.to_owned(),
                reference: name.to_owned(),
            });
        };
        self.inlining.push(name.to_owned());
        let inlined = self.schema(definition, at);
        self.inlining.pop();
        let mut inlined = inlined?;
        let mut beside = map.clone();
        beside.remove("$ref");
        let beside = self.keywords(&beside, at)?;
        if let Some(node) = inlined.as_object_mut() {
            node.extend(beside);
        }
        Ok(inlined)
    }

    /// The keywords of one node, each projected, dropped or refused.
    fn keywords(
        &mut self,
        map: &Map<String, Value>,
        at: &str,
    ) -> Result<Map<String, Value>, CrdError> {
        let mut projected = Map::new();
        for (key, value) in map {
            match key.as_str() {
                annotation if ANNOTATIONS.contains(&annotation) => {}
                "x-ess-kind"
                    if value
                        .as_str()
                        .is_some_and(|kind| ANNOTATED_KINDS.contains(&kind)) => {}
                "x-ess-map-key" if value.as_str() == Some(TEXT_KEY) => {}
                "additionalProperties" => match value {
                    Value::Bool(false) => {}
                    Value::Bool(true) => {
                        projected.insert(PRESERVE_UNKNOWN.to_owned(), Value::Bool(true));
                    }
                    Value::Object(_) => {
                        projected.insert(key.clone(), self.schema(value, &format!("{at}.*"))?);
                    }
                    _ => {
                        return Err(shape(
                            at,
                            "`additionalProperties` is neither a boolean nor a schema",
                        ));
                    }
                },
                "const" => {
                    if map.contains_key("enum") {
                        return Err(shape(at, "both `const` and `enum`"));
                    }
                    projected.insert("enum".to_owned(), json!([value]));
                }
                "properties" => {
                    let Some(properties) = value.as_object() else {
                        return Err(shape(at, "`properties` is not a map"));
                    };
                    let mut each = Map::new();
                    for (name, property) in properties {
                        each.insert(
                            name.clone(),
                            self.schema(property, &format!("{at}.{name}"))?,
                        );
                    }
                    projected.insert(key.clone(), Value::Object(each));
                }
                "items" => {
                    projected.insert(key.clone(), self.schema(value, &format!("{at}[]"))?);
                }
                "format"
                    if value
                        .as_str()
                        .is_some_and(|format| MISREAD_FORMATS.contains(&format)) =>
                {
                    return Err(CrdError::Keyword {
                        at: at.to_owned(),
                        keyword: format!("format: {}", value.as_str().unwrap_or_default()),
                    });
                }
                carried if CARRIED.contains(&carried) => {
                    projected.insert(key.clone(), value.clone());
                }
                other => {
                    return Err(CrdError::Keyword {
                        at: at.to_owned(),
                        keyword: other.to_owned(),
                    });
                }
            }
        }
        if self
            .inlining
            .last()
            .is_some_and(|definition| absent_list_is_empty(definition))
        {
            require_no_list(&mut projected);
        }
        if projected.contains_key("properties") && projected.contains_key("additionalProperties") {
            return Err(shape(
                at,
                "`properties` beside an `additionalProperties` schema, which a structural schema \
                 does not allow",
            ));
        }
        Ok(projected)
    }

    /// A tagged union, flattened: the tag beside the fields of every variant.
    fn union(&mut self, map: &Map<String, Value>, at: &str) -> Result<Value, CrdError> {
        let union = map
            .get("x-ess-name")
            .and_then(Value::as_str)
            .unwrap_or(at)
            .to_owned();
        let refuse = |detail: String| CrdError::Union {
            union: union.clone(),
            detail,
        };
        let Some(tag) = map.get("x-ess-union-tag").and_then(Value::as_str) else {
            return Err(refuse("no `x-ess-union-tag`".to_owned()));
        };
        let Some(variants) = map.get("oneOf").and_then(Value::as_array) else {
            return Err(refuse("no `oneOf` of variants".to_owned()));
        };
        if let Some(other) = map.keys().find(|key| {
            !ANNOTATIONS.contains(&key.as_str())
                && !["x-ess-kind", "x-ess-union-tag", "oneOf"].contains(&key.as_str())
        }) {
            return Err(CrdError::Keyword {
                at: at.to_owned(),
                keyword: other.clone(),
            });
        }
        let mut fields = Map::new();
        // Each variant's tag, the fields it declares and those it requires, in declaration order.
        let mut declared: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
        for (index, variant) in variants.iter().enumerate() {
            if let Some(other) = variant.as_object().and_then(|variant| {
                variant.keys().find(|key| {
                    !ANNOTATIONS.contains(&key.as_str())
                        && !["type", "properties", "required", "additionalProperties"]
                            .contains(&key.as_str())
                })
            }) {
                return Err(refuse(format!("variant {index} holds `{other}`")));
            }
            let Some(members) = variant.get("properties").and_then(Value::as_object) else {
                return Err(refuse(format!("variant {index} declares no properties")));
            };
            let Some(name) = members
                .get(tag)
                .and_then(|member| member.get("const"))
                .and_then(Value::as_str)
            else {
                return Err(refuse(format!("variant {index} has no `{tag}` const")));
            };
            if declared.iter().any(|(seen, _, _)| seen == name) {
                return Err(refuse(format!("variant `{name}` is declared twice")));
            }
            if let Some(other) = members.keys().find(|key| *key != tag && *key != "value") {
                return Err(refuse(format!(
                    "variant `{name}` holds `{other}` beside `{tag}` and `value`"
                )));
            }
            let (own, required) = match members.get("value") {
                None => (Map::new(), Vec::new()),
                Some(value) => {
                    let projected = self.schema(value, &format!("{at}<{name}>"))?;
                    let what = projected
                        .as_object()
                        .map_or_else(|| Some("no schema".to_owned()), not_a_struct);
                    if let Some(what) = what {
                        return Err(refuse(format!(
                            "variant `{name}` holds {what}, not a struct, and a flattened union \
                             carries only struct variants"
                        )));
                    }
                    let own = projected
                        .get("properties")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    let required: Vec<String> = projected
                        .get("required")
                        .and_then(Value::as_array)
                        .map(|names| {
                            names
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default();
                    (own, required)
                }
            };
            let names: Vec<String> = own.keys().cloned().collect();
            for (field, schema) in own {
                let conflict = field == tag
                    || fields
                        .get(&field)
                        .is_some_and(|declared: &Value| *declared != schema);
                if conflict {
                    return Err(CrdError::UnionConflict {
                        union: union.clone(),
                        property: field,
                    });
                }
                fields.insert(field, schema);
            }
            declared.push((name.to_owned(), names, required));
        }
        if declared.is_empty() {
            return Err(refuse("no variant".to_owned()));
        }
        let mut entries = Vec::new();
        let mut required_by_all: Option<Vec<String>> = None;
        for (name, own, required) in &declared {
            let mut tagged = Map::new();
            tagged.insert(tag.to_owned(), json!({"enum": [name]}));
            let mut entry = Map::new();
            entry.insert("properties".to_owned(), Value::Object(tagged));
            if !required.is_empty() {
                entry.insert("required".to_owned(), json!(required));
            }
            let mut foreign: Vec<&String> = fields.keys().filter(|f| !own.contains(f)).collect();
            foreign.sort();
            if !foreign.is_empty() {
                let each: Vec<Value> = foreign
                    .into_iter()
                    .map(|field| json!({"required": [field]}))
                    .collect();
                entry.insert("not".to_owned(), json!({"anyOf": each}));
            }
            entries.push(Value::Object(entry));
            required_by_all = Some(match required_by_all {
                None => required.clone(),
                Some(all) => all
                    .into_iter()
                    .filter(|field| required.contains(field))
                    .collect(),
            });
        }
        let tags: Vec<&String> = declared.iter().map(|(name, _, _)| name).collect();
        let mut properties = Map::new();
        properties.insert(tag.to_owned(), json!({"type": "string", "enum": tags}));
        properties.extend(fields);
        let mut required = vec![tag.to_owned()];
        required.extend(required_by_all.unwrap_or_default());
        Ok(json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "oneOf": entries,
        }))
    }
}

/// Every rule of a structural schema `document` breaks, with where: a forbidden key anywhere in
/// the document ([`FORBIDDEN`], `x-ess-*` and an `additionalProperties` that is not a schema, a
/// property's name not counting as a key); in its `openAPIV3Schema`, a schema node without a
/// `type` that does not preserve unknown fields, `properties` beside an `additionalProperties`
/// schema, and inside a junctor, at any depth, a key of [`NOT_IN_A_JUNCTOR`] or a member the
/// schema outside the junctor does not declare. Empty means none.
#[must_use]
pub fn violations(document: &Value) -> Vec<String> {
    let mut found = Vec::new();
    forbidden(document, "", false, &mut found);
    if let Some(versions) = document
        .get("spec")
        .and_then(|spec| spec.get("versions"))
        .and_then(Value::as_array)
    {
        for (index, version) in versions.iter().enumerate() {
            if let Some(schema) = version
                .get("schema")
                .and_then(|schema| schema.get("openAPIV3Schema"))
            {
                structural(
                    schema,
                    &format!("/spec/versions/{index}/schema/openAPIV3Schema"),
                    &mut found,
                );
            }
        }
    }
    found
}

fn forbidden(value: &Value, at: &str, names: bool, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, member) in map {
                let path = format!("{at}/{key}");
                let not_a_schema = key == "additionalProperties" && !member.is_object();
                if !names
                    && (FORBIDDEN.contains(&key.as_str())
                        || key.starts_with("x-ess-")
                        || not_a_schema)
                {
                    found.push(format!("{path}: a key no structural schema holds"));
                }
                forbidden(member, &path, !names && key == "properties", found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                forbidden(item, &format!("{at}/{index}"), false, found);
            }
        }
        _ => {}
    }
}

fn structural(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(node) = schema.as_object() else {
        found.push(format!("{at}: not a schema"));
        return;
    };
    if !node.contains_key("type") && !preserves_unknown(node) {
        found.push(format!("{at}: no type"));
    }
    for (name, property) in node
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        structural(property, &format!("{at}/properties/{name}"), found);
    }
    if let Some(values) = node
        .get("additionalProperties")
        .filter(|values| values.is_object())
    {
        if node.contains_key("properties") {
            found.push(format!("{at}: `properties` beside `additionalProperties`"));
        }
        structural(values, &format!("{at}/additionalProperties"), found);
    }
    if let Some(items) = node.get("items") {
        structural(items, &format!("{at}/items"), found);
    }
    junctors(node, node, at, found);
}

/// The junctors of `node`, each entry checked against `outside`, the schema they constrain.
fn junctors(
    node: &Map<String, Value>,
    outside: &Map<String, Value>,
    at: &str,
    found: &mut Vec<String>,
) {
    for junctor in JUNCTORS {
        let entries = match node.get(junctor) {
            None => continue,
            Some(Value::Array(entries)) => entries.iter().collect::<Vec<_>>(),
            Some(single) => vec![single],
        };
        for (index, entry) in entries.into_iter().enumerate() {
            let here = if junctor == "not" {
                format!("{at}/not")
            } else {
                format!("{at}/{junctor}/{index}")
            };
            inside_junctor(entry, outside, &here, found);
        }
    }
}

/// One schema inside a junctor: none of [`NOT_IN_A_JUNCTOR`], and every member it names declared
/// by `outside`, the schema it constrains.
fn inside_junctor(
    entry: &Value,
    outside: &Map<String, Value>,
    here: &str,
    found: &mut Vec<String>,
) {
    let Some(entry) = entry.as_object() else {
        found.push(format!("{here}: not a schema"));
        return;
    };
    for key in entry.keys() {
        if NOT_IN_A_JUNCTOR.contains(&key.as_str()) {
            found.push(format!("{here}/{key}: not allowed inside a junctor"));
        }
    }
    let declared = outside.get("properties").and_then(Value::as_object);
    for name in entry
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !declared.is_some_and(|properties| properties.contains_key(name)) {
            found.push(format!(
                "{here}: `{name}` is not declared outside the junctor"
            ));
        }
    }
    for (name, property) in entry
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        match declared
            .and_then(|properties| properties.get(name))
            .and_then(Value::as_object)
        {
            None => found.push(format!(
                "{here}: `{name}` is not declared outside the junctor"
            )),
            Some(beside) => {
                inside_junctor(
                    property,
                    beside,
                    &format!("{here}/properties/{name}"),
                    found,
                );
            }
        }
    }
    if let Some(items) = entry.get("items") {
        match outside.get("items").and_then(Value::as_object) {
            None => found.push(format!(
                "{here}: `items` is not declared outside the junctor"
            )),
            Some(beside) => inside_junctor(items, beside, &format!("{here}/items"), found),
        }
    }
    junctors(entry, outside, here, found);
}

/// `value` as YAML, every map's keys in sorted order.
fn yaml(value: &Value) -> serde_yaml_ng::Value {
    match value {
        Value::Null => serde_yaml_ng::Value::Null,
        Value::Bool(flag) => serde_yaml_ng::Value::Bool(*flag),
        Value::Number(number) => match (number.as_i64(), number.as_u64(), number.as_f64()) {
            (Some(integer), _, _) => serde_yaml_ng::Value::Number(integer.into()),
            (None, Some(unsigned), _) => serde_yaml_ng::Value::Number(unsigned.into()),
            (None, None, Some(float)) => serde_yaml_ng::Value::Number(float.into()),
            (None, None, None) => serde_yaml_ng::Value::String(number.to_string()),
        },
        Value::String(text) => serde_yaml_ng::Value::String(text.clone()),
        Value::Array(items) => serde_yaml_ng::Value::Sequence(items.iter().map(yaml).collect()),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut mapping = serde_yaml_ng::Mapping::new();
            for key in keys {
                mapping.insert(serde_yaml_ng::Value::String(key.clone()), yaml(&map[key]));
            }
            serde_yaml_ng::Value::Mapping(mapping)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn violations_name_each_forbidden_key_and_untyped_node() {
        let document = json!({
            "spec": {"versions": [{"schema": {"openAPIV3Schema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "a": {"$ref": "#/$defs/x"},
                    "b": {"type": "object", "additionalProperties": false},
                    "c": {"type": "string", "const": "c", "x-ess-kind": "enum"},
                },
            }}}]},
        });
        assert_eq!(
            violations(&document),
            [
                "/spec/versions/0/schema/openAPIV3Schema/properties/a/$ref: a key no structural schema holds",
                "/spec/versions/0/schema/openAPIV3Schema/properties/b/additionalProperties: a key no structural schema holds",
                "/spec/versions/0/schema/openAPIV3Schema/properties/c/const: a key no structural schema holds",
                "/spec/versions/0/schema/openAPIV3Schema/properties/c/x-ess-kind: a key no structural schema holds",
                "/spec/versions/0/schema/openAPIV3Schema/properties/a: no type",
            ]
        );
    }

    #[test]
    fn violations_name_what_a_junctor_may_not_hold() {
        let document = json!({
            "spec": {"versions": [{"schema": {"openAPIV3Schema": {
                "type": "object",
                "properties": {"kind": {"type": "string"}},
                "oneOf": [
                    {"properties": {"kind": {"enum": ["a"], "type": "string"}}, "required": ["port"]},
                    {"type": "object"},
                    {"properties": {"kind": {"enum": ["b"]}}, "not": {"anyOf": [{"required": ["kind"]}, {"required": ["ghost"], "nullable": true}]}},
                ],
            }}}]},
        });
        assert_eq!(
            violations(&document),
            [
                "/spec/versions/0/schema/openAPIV3Schema/oneOf/0: `port` is not declared outside the junctor",
                "/spec/versions/0/schema/openAPIV3Schema/oneOf/0/properties/kind/type: not allowed inside a junctor",
                "/spec/versions/0/schema/openAPIV3Schema/oneOf/1/type: not allowed inside a junctor",
                "/spec/versions/0/schema/openAPIV3Schema/oneOf/2/not/anyOf/1/nullable: not allowed inside a junctor",
                "/spec/versions/0/schema/openAPIV3Schema/oneOf/2/not/anyOf/1: `ghost` is not declared outside the junctor",
            ]
        );
    }

    #[test]
    fn an_unknown_keyword_is_refused_naming_it() {
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        for (keyword, value) in [
            ("anyOf", json!([])),
            ("oneOf", json!([])),
            ("x-ess-union-tag", json!("kind")),
            ("x-ess-mystery", json!(true)),
            ("x-ess-map-key", json!("Integer")),
            ("x-ess-kind", json!("mystery")),
            ("uniqueItems", json!(true)),
        ] {
            let mut a = json!({"type": "string"});
            a[keyword] = value;
            let spec = json!({"type": "object", "properties": {"a": a}});
            assert_eq!(
                open_api_schema(&spec, &entity),
                Err(CrdError::Keyword {
                    at: "spec.a".to_owned(),
                    keyword: keyword.to_owned()
                }),
                "`{keyword}`"
            );
        }
    }

    #[test]
    fn every_keyword_is_carried_rewritten_or_dropped_as_its_row_says() {
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let mut node = json!({"title": "A", "$id": "a", "$schema": "s", "x-ess-name": "t.A",
            "x-ess-field": "a", "x-ess-provenance": {}, "x-ess-kind": "newtype",
            "x-ess-map-key": "String", "additionalProperties": false});
        for keyword in CARRIED {
            node[keyword] = json!(keyword);
        }
        node["type"] = json!("string");
        let spec = json!({"type": "object", "properties": {"a": node}});
        let projected = open_api_schema(&spec, &entity).expect("projects");
        let mut kept: Vec<&str> = projected["properties"]["spec"]["properties"]["a"]
            .as_object()
            .expect("a")
            .keys()
            .map(String::as_str)
            .collect();
        kept.sort_unstable();
        let mut carried = CARRIED.to_vec();
        carried.sort_unstable();
        assert_eq!(kept, carried, "carried survive, annotations are dropped");

        let open = json!({"type": "object", "properties": {"a": {"type": "object", "additionalProperties": true}}});
        assert_eq!(
            open_api_schema(&open, &entity).expect("projects")["properties"]["spec"]["properties"]
                ["a"],
            json!({"type": "object", "x-kubernetes-preserve-unknown-fields": true})
        );
        let constant =
            json!({"type": "object", "properties": {"a": {"type": "string", "const": "x"}}});
        assert_eq!(
            open_api_schema(&constant, &entity).expect("projects")["properties"]["spec"]["properties"]
                ["a"],
            json!({"type": "string", "enum": ["x"]})
        );
    }

    /// A union of three variants: a member two of them declare is refused only in the third's
    /// entry, and one all three declare in none.
    #[test]
    fn a_variant_entry_refuses_exactly_the_members_it_does_not_declare() {
        let spec = json!({
            "$ref": "#/$defs/t.U",
            "$defs": {
                "t.U": {"x-ess-kind": "union", "x-ess-union-tag": "kind", "oneOf": [
                    {"properties": {"kind": {"const": "a"}, "value": {"$ref": "#/$defs/t.A"}}},
                    {"properties": {"kind": {"const": "b"}, "value": {"$ref": "#/$defs/t.B"}}},
                    {"properties": {"kind": {"const": "c"}, "value": {"$ref": "#/$defs/t.C"}}},
                ]},
                "t.A": {"type": "object", "properties": {"name": {"type": "string"}, "port": {"type": "integer"}}, "required": ["name"]},
                "t.B": {"type": "object", "properties": {"name": {"type": "string"}, "port": {"type": "integer"}, "path": {"type": "string"}}, "required": ["name", "path"]},
                "t.C": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
            },
        });
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let projected = open_api_schema(&spec, &entity).expect("projects");
        assert_eq!(
            projected["properties"]["spec"]["oneOf"],
            json!([
                {"properties": {"kind": {"enum": ["a"]}}, "required": ["name"], "not": {"anyOf": [{"required": ["path"]}]}},
                {"properties": {"kind": {"enum": ["b"]}}, "required": ["name", "path"]},
                {"properties": {"kind": {"enum": ["c"]}}, "required": ["name"], "not": {"anyOf": [{"required": ["path"]}, {"required": ["port"]}]}},
            ])
        );
        assert_eq!(
            projected["properties"]["spec"]["required"],
            json!(["kind", "name"])
        );
    }

    /// A list a `loom.agent` definition requires, inline or not, is not required; a member of it
    /// that is not a list is; a list another document requires stays required, inlined into a
    /// `loom.agent` definition or not.
    #[test]
    fn only_a_list_of_the_agent_document_may_be_absent() {
        let list = json!({"type": "array", "items": {"type": "string"}});
        let spec = json!({
            "$ref": "#/$defs/loom.agent.A",
            "$defs": {
                "loom.agent.A": {"type": "object", "properties": {
                    "states": list,
                    "format": {"type": "string"},
                    "model": {"$ref": "#/$defs/loom.models.M"},
                    "inline": {"type": "object", "properties": {"allow": list}, "required": ["allow"]},
                }, "required": ["states", "format", "model", "inline"]},
                "loom.models.M": {"type": "object", "properties": {"roles": list}, "required": ["roles"]},
            },
        });
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let projected = open_api_schema(&spec, &entity).expect("projects");
        let agent = &projected["properties"]["spec"];
        assert_eq!(agent["required"], json!(["format", "model", "inline"]));
        assert_eq!(agent["properties"]["inline"].get("required"), None);
        assert_eq!(agent["properties"]["model"]["required"], json!(["roles"]));

        let elsewhere =
            json!({"type": "object", "properties": {"roles": list}, "required": ["roles"]});
        assert_eq!(
            open_api_schema(&elsewhere, &entity).expect("projects")["properties"]["spec"]["required"],
            json!(["roles"])
        );
        assert!(absent_list_is_empty("loom.agent.AgentSpec"));
        assert!(!absent_list_is_empty("loom.agents.AgentSpec"));
        assert!(!absent_list_is_empty("loom.models.ModelsConfig"));
    }

    /// A union of a struct variant `s` and the variant `other`, whose value is `value`.
    fn union_with(value: &Value) -> Value {
        json!({
            "$ref": "#/$defs/t.U",
            "$defs": {
                "t.U": {"x-ess-name": "t.U", "x-ess-kind": "union", "x-ess-union-tag": "kind", "oneOf": [
                    {"properties": {"kind": {"const": "s"}, "value": {"$ref": "#/$defs/t.S"}}},
                    {"properties": {"kind": {"const": "other"}, "value": {"$ref": "#/$defs/t.V"}}},
                ]},
                "t.S": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
                "t.V": value,
            },
        })
    }

    #[test]
    fn a_union_variant_that_is_not_a_struct_is_refused_naming_it() {
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        for (value, what) in [
            (
                json!({"x-ess-kind": "newtype", "type": "object", "additionalProperties": {"type": "string"}, "x-ess-map-key": "String"}),
                "a map",
            ),
            (json!({"x-ess-kind": "newtype"}), "any JSON value"),
            (
                json!({"x-ess-kind": "newtype", "type": "string"}),
                "a `string`",
            ),
        ] {
            assert_eq!(
                open_api_schema(&union_with(&value), &entity),
                Err(CrdError::Union {
                    union: "t.U".to_owned(),
                    detail: format!(
                        "variant `other` holds {what}, not a struct, and a flattened union \
                         carries only struct variants"
                    ),
                }),
                "{value}"
            );
        }
        let unit = json!({
            "$ref": "#/$defs/t.U",
            "$defs": {"t.U": {"x-ess-kind": "union", "x-ess-union-tag": "kind", "oneOf": [
                {"properties": {"kind": {"const": "none"}}},
            ]}},
        });
        assert!(
            open_api_schema(&unit, &entity).is_ok(),
            "a variant with no value"
        );
    }

    /// ESS `Json` admits any JSON value, so its node is untyped and preserves unknown fields,
    /// wherever it stands; an object that admits any member stays an object.
    #[test]
    fn json_is_untyped_and_preserves_unknown_fields_wherever_it_stands() {
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let spec = json!({
            "$ref": "#/$defs/t.A",
            "$defs": {
                "t.A": {"type": "object", "properties": {
                    "raw": {},
                    "noted": {"description": "any value"},
                    "named": {"$ref": "#/$defs/t.Raw"},
                    "values": {"type": "object", "additionalProperties": {}, "x-ess-map-key": "String"},
                    "list": {"type": "array", "items": {}},
                    "open": {"type": "object", "additionalProperties": true},
                }},
                "t.Raw": {"x-ess-kind": "newtype", "title": "Raw"},
            },
        });
        let projected = open_api_schema(&spec, &entity).expect("projects");
        let a = &projected["properties"]["spec"]["properties"];
        let any = json!({"x-kubernetes-preserve-unknown-fields": true});
        assert_eq!(a["raw"], any);
        assert_eq!(
            a["noted"],
            json!({"description": "any value", "x-kubernetes-preserve-unknown-fields": true})
        );
        assert_eq!(a["named"], any);
        assert_eq!(
            a["values"],
            json!({"type": "object", "additionalProperties": any})
        );
        assert_eq!(a["list"], json!({"type": "array", "items": any}));
        assert_eq!(
            a["open"],
            json!({"type": "object", "x-kubernetes-preserve-unknown-fields": true})
        );
    }

    /// A `format` the API server reads otherwise than ESS writes it is refused, naming the field
    /// and the format; one both read alike is carried.
    #[test]
    fn a_format_the_api_server_reads_otherwise_is_refused_naming_it() {
        let entity = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let field = |format: &str| json!({"type": "object", "properties": {"timeout": {"type": "string", "format": format}}});
        assert_eq!(
            open_api_schema(&field("duration"), &entity),
            Err(CrdError::Keyword {
                at: "spec.timeout".to_owned(),
                keyword: "format: duration".to_owned(),
            })
        );
        assert_eq!(
            open_api_schema(&field("date-time"), &entity).expect("projects")["properties"]["spec"]
                ["properties"]["timeout"],
            json!({"type": "string", "format": "date-time"})
        );
    }
}
