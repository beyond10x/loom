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
    /// The host catalog cannot be assembled: for [`classify`], the vendored engineering catalog
    /// cannot be built, its lock refusing included.
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
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

/// Proposes the protocol of the vendored engineering catalog (`ProtocolCatalog::engineering`)
/// `intent` should run under.
///
/// One forced `pick_protocol` call on `model`: the instructions describe every registry entry by
/// its Canon description and artifact descriptions, the intent is the one user item, and the
/// tool's `protocol` field is an enum of the entries as `name@major`. Nothing is retried.
///
/// # Errors
/// [`RouterError::InvalidThreshold`] for a threshold outside 0 to 1 (before any call),
/// [`RouterError::Catalog`] when the engineering catalog cannot be built, a lock refusal
/// included, with its error text and before any call, [`RouterError::Model`] when the call gives
/// no answer, [`RouterError::Malformed`] for arguments that are not a pick,
/// [`RouterError::OutsideRegistry`] for a protocol the registry does not hold and
/// [`RouterError::Unsure`] for a confidence below `threshold`.
pub async fn classify(
    intent: &str,
    model: &dyn Model,
    threshold: f64,
) -> Result<ProtocolPick, RouterError> {
    classify_in(intent, model, threshold, ProtocolCatalog::engineering()).await
}

/// [`classify`] over `catalog`, the engineering catalog as it was built or why it was not: a
/// catalog that could not be built is [`RouterError::Catalog`] with its error text, before any
/// call.
async fn classify_in(
    intent: &str,
    model: &dyn Model,
    threshold: f64,
    catalog: Result<ProtocolCatalog, String>,
) -> Result<ProtocolPick, RouterError> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(RouterError::InvalidThreshold(threshold));
    }
    let catalog = catalog.map_err(RouterError::Catalog)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use llm_core::{
        BoxFuture, CallId, Cancel, Capabilities, Error, Id, Protocol, Provenance, StopReason,
        StreamSink, ToolCall, TurnObservation, TurnOutcome, TurnRequest,
    };
    use loom_protocols::{ENGINEERING_FILES, ENGINEERING_LOCK};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A recorded model that counts its turns and answers each with a confident pick of
    /// `software-change@1`.
    struct Counted {
        provenance: Provenance,
        capabilities: Capabilities,
        turns: AtomicUsize,
    }

    impl Counted {
        fn new() -> Self {
            let id = |value: &str| Id::new(value).expect("fixture identifier");
            Self {
                provenance: Provenance {
                    protocol: Protocol::Responses,
                    provider: id("recorded"),
                    account: id("recorded"),
                    endpoint: id("recorded"),
                    model: id("recorded-model"),
                    binding_revision: id("rev-1"),
                },
                capabilities: Capabilities {
                    tools: true,
                    tool_choice: true,
                    ..Capabilities::text(32_768, 2_048)
                },
                turns: AtomicUsize::new(0),
            }
        }
    }

    impl Model for Counted {
        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }
        fn turn<'a>(
            &'a self,
            _request: &'a TurnRequest,
            _sink: &'a mut dyn StreamSink,
            _cancel: &'a Cancel,
        ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
            self.turns.fetch_add(1, Ordering::SeqCst);
            let mut observation = TurnObservation::new(self.provenance.clone());
            observation.final_usage = true;
            let outcome = TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: vec![Item::ToolCall(ToolCall {
                    call_id: CallId::new("call_1").expect("call id"),
                    name: ToolName::new(PICK_TOOL).expect("tool name"),
                    arguments: json!({
                        "protocol": "software-change@1",
                        "confidence": 0.9,
                        "reasons": ["a change"]
                    }),
                })],
                observation,
            };
            Box::pin(async move { Ok(outcome) })
        }
    }

    #[tokio::test]
    async fn a_catalog_failing_its_lock_is_a_catalog_error_and_asks_no_model() {
        let mut files: Vec<(&str, Vec<u8>)> = ENGINEERING_FILES
            .iter()
            .map(|(path, bytes)| (*path, bytes.to_vec()))
            .collect();
        let (_, bytes) = files
            .iter_mut()
            .find(|(path, _)| *path == "software-change/1.yaml")
            .expect("software-change/1.yaml is vendored");
        let at = bytes
            .windows(b"Deliver a change".len())
            .position(|window| window == b"Deliver a change")
            .expect("the description");
        bytes[at] = b'd';
        let borrowed: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect();
        let catalog = ProtocolCatalog::from_vendored(&borrowed, ENGINEERING_LOCK)
            .map_err(|error| error.to_string());
        let refusal = catalog
            .clone()
            .expect_err("the lock refuses the changed file");
        let model = Counted::new();
        match classify_in("change the parser", &model, 0.5, catalog).await {
            Err(RouterError::Catalog(text)) => {
                assert!(
                    text.starts_with(
                        "engineering protocol lock refused: DigestMismatch software-change/1.yaml"
                    ),
                    "{text}"
                );
                assert_eq!(text, refusal, "the catalog's error text, unchanged");
            }
            other => panic!("a lock refusal is RouterError::Catalog, got {other:?}"),
        }
        assert_eq!(model.turns.load(Ordering::SeqCst), 0, "no model was asked");

        // The same model, over the catalog as built, is asked once and its pick returned.
        let pick = classify_in(
            "change the parser",
            &model,
            0.5,
            ProtocolCatalog::engineering(),
        )
        .await
        .expect("a pick");
        assert_eq!(pick.protocol, "software-change@1");
        assert_eq!(model.turns.load(Ordering::SeqCst), 1);
    }
}
