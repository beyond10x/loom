//! The forced `pick_protocol` call and the checks on its answer.

use std::fmt::{self, Write as _};

use b10x_llm_tool_call::{ModelError, call_tool};
use llm_core::{Item, Model, ToolName, ToolSpec};
use loom_protocols::ProtocolCatalog;
use serde_json::{Map, Value, json};

/// The tool [`classify`] forces on the model.
pub const PICK_TOOL: &str = "pick_protocol";

/// The router's proposal for one intent: `intake.routing.ProtocolPick` without its identities.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolPick {
    /// The picked registry entry as `name@major`, e.g. `software-change@1`.
    pub protocol: String,
    /// How sure the model is, from 0 to 1. The ESS field is a `Decimal`; here it is an `f64`,
    /// because the model answers with a JSON number and the threshold it is compared with is one.
    pub confidence: f64,
    /// Why the model picked this protocol, as it gave them.
    pub reasons: Vec<String>,
}

/// Why [`classify`] gave no pick.
#[derive(Debug)]
pub enum RouterError {
    /// The threshold is not a number from 0 to 1, so no confidence could be held to it.
    InvalidThreshold(f64),
    /// A built-in protocol of the registry cannot be read: Canon cannot parse it or finds it
    /// invalid.
    Registry(canon_engineering::registry::Error),
    /// The host catalog cannot be assembled.
    Catalog(String),
    /// The forced call gave no usable answer.
    Model(ModelError),
    /// The call's arguments are not a pick: a field is missing or of the wrong type, or the
    /// confidence lies outside 0 to 1.
    Malformed(String),
    /// The model picked a protocol the registry does not hold.
    OutsideRegistry { protocol: String },
    /// The model picked a registry protocol, less confidently than the threshold asks.
    Unsure {
        protocol: String,
        confidence: f64,
        threshold: f64,
    },
}

impl fmt::Display for RouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidThreshold(threshold) => {
                write!(f, "the threshold {threshold} is not a number from 0 to 1")
            }
            Self::Registry(error) => write!(f, "the protocol registry cannot be read: {error}"),
            Self::Catalog(error) => write!(f, "the protocol catalog cannot be read: {error}"),
            Self::Model(error) => write!(f, "no protocol was picked: {error}"),
            Self::Malformed(why) => write!(f, "the model's pick is malformed: {why}"),
            Self::OutsideRegistry { protocol } => {
                write!(
                    f,
                    "the model picked {protocol}, which the registry does not hold"
                )
            }
            Self::Unsure {
                protocol,
                confidence,
                threshold,
            } => write!(
                f,
                "the model picked {protocol} with confidence {confidence}, below the threshold {threshold}"
            ),
        }
    }
}

impl std::error::Error for RouterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Registry(error) => Some(error),
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

/// Proposes the protocol of the ELS registry `intent` should run under.
///
/// One forced `pick_protocol` call on `model`: the instructions describe every registry entry by
/// its Canon description and artifact descriptions, the intent is the one user item, and the
/// tool's `protocol` field is an enum of the entries as `name@major`. Nothing is retried.
///
/// # Errors
/// [`RouterError::InvalidThreshold`] for a threshold outside 0 to 1 (before any call),
/// [`RouterError::Catalog`] when the engineering catalog cannot be assembled, [`RouterError::Model`] when the call
/// gives no answer, [`RouterError::Malformed`] for arguments that are not a pick,
/// [`RouterError::OutsideRegistry`] for a protocol the registry does not hold and
/// [`RouterError::Unsure`] for a confidence below `threshold`.
pub async fn classify(
    intent: &str,
    model: &dyn Model,
    threshold: f64,
) -> Result<ProtocolPick, RouterError> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(RouterError::InvalidThreshold(threshold));
    }
    let catalog = ProtocolCatalog::engineering().map_err(RouterError::Catalog)?;
    classify_with_catalog(intent, model, threshold, &catalog).await
}

/// Proposes a protocol from the host's immutable catalog. The proposal conveys no authority.
///
/// Uses the same request and answer checks as [`classify`], but offers exactly the supplied
/// catalog, including host-admitted definitions from any supported source.
///
/// # Errors
/// The same threshold, model, malformed-answer, membership and confidence errors as [`classify`].
pub async fn classify_with_catalog(
    intent: &str,
    model: &dyn Model,
    threshold: f64,
    catalog: &ProtocolCatalog,
) -> Result<ProtocolPick, RouterError> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(RouterError::InvalidThreshold(threshold));
    }
    let registry = Registry::from_catalog(catalog);
    if registry.entries.is_empty() {
        return Err(RouterError::Catalog("no protocols are admitted".into()));
    }
    let arguments = call_tool(
        model,
        &registry.instructions,
        vec![Item::user(intent)],
        registry.tool(),
    )
    .await
    .map_err(RouterError::Model)?;
    registry.check(&arguments, threshold)
}

/// The registry as the model is shown it.
struct Registry {
    /// Every entry as `name@major`, in registry order.
    entries: Vec<String>,
    /// The standing instruction: the task and each entry's descriptions.
    instructions: String,
}

impl Registry {
    fn from_catalog(catalog: &ProtocolCatalog) -> Self {
        let mut entries = Vec::new();
        let mut instructions = String::from(
            "Pick the one protocol below that the user's intent should run under, by calling \
             pick_protocol. Give your confidence from 0 to 1 and your reasons. Pick only a \
             listed protocol; when none fits, say so with a low confidence.\n\nProtocols:\n",
        );
        for definition in catalog.iter() {
            let entry = definition.name();
            let description = definition.description();
            // Writing to a String cannot fail.
            let _ = writeln!(instructions, "\n- {entry}: {description}");
            let artifacts = definition.artifacts();
            if !artifacts.is_empty() {
                let _ = writeln!(instructions, "  Artifacts:");
            }
            for (artifact, description) in artifacts {
                let _ = writeln!(instructions, "  - {artifact}: {description}");
            }
            entries.push(entry.to_owned());
        }
        Self {
            entries,
            instructions,
        }
    }

    /// The forced tool, its `protocol` an enum of the entries.
    fn tool(&self) -> ToolSpec {
        ToolSpec {
            name: ToolName::new(PICK_TOOL).expect("pick_protocol is a valid tool name"),
            description: "Pick the protocol of the registry the intent should run under."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "protocol": {
                        "type": "string",
                        "enum": self.entries,
                        "description": "the picked protocol, as name@major"
                    },
                    "confidence": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "description": "how sure the pick is, from 0 to 1"
                    },
                    "reasons": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "why this protocol fits the intent"
                    }
                },
                "required": ["protocol", "confidence", "reasons"],
                "additionalProperties": false
            }),
        }
    }

    /// The call's arguments as a pick held to the registry and `threshold`.
    fn check(&self, arguments: &Value, threshold: f64) -> Result<ProtocolPick, RouterError> {
        let fields = arguments
            .as_object()
            .ok_or_else(|| RouterError::Malformed("the arguments are not an object".to_owned()))?;
        let protocol = string(fields, "protocol")?;
        if !self.entries.contains(&protocol) {
            return Err(RouterError::OutsideRegistry { protocol });
        }
        let confidence = fields
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| RouterError::Malformed("`confidence` is not a number".to_owned()))?;
        if !(0.0..=1.0).contains(&confidence) {
            return Err(RouterError::Malformed(format!(
                "`confidence` {confidence} is not from 0 to 1"
            )));
        }
        let reasons = fields
            .get("reasons")
            .and_then(Value::as_array)
            .and_then(|reasons| {
                reasons
                    .iter()
                    .map(|reason| reason.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                RouterError::Malformed("`reasons` is not a list of strings".to_owned())
            })?;
        if confidence < threshold {
            return Err(RouterError::Unsure {
                protocol,
                confidence,
                threshold,
            });
        }
        Ok(ProtocolPick {
            protocol,
            confidence,
            reasons,
        })
    }
}

/// The string field `name` of `fields`.
fn string(fields: &Map<String, Value>, name: &str) -> Result<String, RouterError> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| RouterError::Malformed(format!("`{name}` is not a string")))
}
