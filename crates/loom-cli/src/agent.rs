//! What `b10x-loom agent check` reads and prints: the spec from a regular file, checked by
//! `b10x_loom_agent::check_text` against the plugin catalog, and one line for either answer; and
//! what `agent serve` and `agent resume` read: the checked spec decoded ([`read_spec`]) and the
//! models its `model` section names ([`spec_models`]).
//!
//! A serveable spec is `checked <agent>: <n> states, <m> bindings, protocol <protocol>` on
//! standard output (`protocol none` without loops); a refused one is
//! `refused: <reason> at <member>: <detail>` on standard error, the reason in the enum's
//! spelling. Both lines are made printable, so a value of the document can start no new line.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use b10x_loom_agent::{Checked, ResumeFailure, SpecRefused, check_text, describe};
use b10x_loom_intake_slice::run::printable;
use llm_core::Model;
use loom_protocols::ProtocolCatalog;
use loom_sdk::agent::AgentSpec;
use loom_sdk::models::{ModelRole, from_routes};

use crate::regular_file;

/// The exit status of a refused spec, as [`crate::AGENT_CHECK_HELP`] states it.
pub const REFUSED: u8 = 3;

/// How many bytes of a spec are read: one past the decoder's bound, so a larger document is
/// refused by the decoder as `Malformed` rather than read whole.
const MAX_SPEC_BYTES: u64 = 2 * 1024 * 1024 + 1;

/// Why `agent check` gave no answer, or the refusal it gave.
#[derive(Debug)]
pub enum CheckFailure {
    /// The spec or the plugin catalog cannot be read (exit status 1), with the reason.
    Unreadable(String),
    /// The spec cannot be served (exit status [`REFUSED`]).
    Refused(SpecRefused),
}

/// Reads the spec at `path` and checks it against the plugin catalog.
///
/// # Errors
/// [`CheckFailure::Unreadable`] naming the file when it is no regular file, cannot be read or is
/// not UTF-8, or when the plugin catalog cannot be loaded; [`CheckFailure::Refused`] with the
/// first refusal.
pub fn check_spec(path: &Path) -> Result<Checked, CheckFailure> {
    let text = spec_text(path)?;
    let catalog = ProtocolCatalog::plugins()
        .map_err(|why| CheckFailure::Unreadable(format!("the plugin catalog: {why}")))?;
    let actions = |name: &str| catalog.get(name).map(|entry| entry.actions());
    check_text(&text, &actions).map_err(CheckFailure::Refused)
}

/// The text of the spec at `path`, read as [`check_spec`] reads it.
fn spec_text(path: &Path) -> Result<String, CheckFailure> {
    let named = |why: &str| CheckFailure::Unreadable(format!("--spec {}: {why}", path.display()));
    let mut bytes = Vec::new();
    regular_file::open(path)
        .map_err(|why| named(&why))?
        .take(MAX_SPEC_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| named(&format!("cannot be read: {error}")))?;
    String::from_utf8(bytes).map_err(|_| named("is not UTF-8"))
}

/// Reads and checks the spec at `path` as [`check_spec`] does, and decodes it.
///
/// # Errors
/// As [`check_spec`].
pub fn read_spec(path: &Path) -> Result<AgentSpec, CheckFailure> {
    check_spec(path)?;
    let text = spec_text(path)?;
    loom_sdk::agent::parse(&text)
        .map_err(|error| CheckFailure::Unreadable(format!("--spec {}: {error}", path.display())))
}

/// The models of a served spec: the classifier, then the turn model.
pub type SpecModels = (Box<dyn Model>, Arc<dyn Model>);

/// The models a served spec runs on: the classifier and the turn model, each the name its flag
/// gives, else the spec's role, of the routes document the spec's `model.routes` names relative
/// to the spec at `path`. Each must take a forced tool call.
///
/// # Errors
/// A role neither declared nor given, a routes document that cannot be loaded or does not serve
/// the name, or a model that cannot take a forced tool call, naming the role and the flag.
pub fn spec_models(
    spec: &AgentSpec,
    path: &Path,
    model: Option<&str>,
    classifier: Option<&str>,
) -> Result<SpecModels, String> {
    let routes: PathBuf = path.parent().map_or_else(
        || PathBuf::from(&spec.model.routes),
        |dir| dir.join(&spec.model.routes),
    );
    let served = |role: ModelRole, name: &str, flag: &str, given: Option<&str>| {
        let model_name = match given {
            Some(given) => given.to_owned(),
            None => spec
                .model
                .roles
                .iter()
                .find(|entry| entry.role == role)
                .map(|entry| entry.model.clone())
                .ok_or_else(|| {
                    format!(
                        "--spec {}: declares no model for the role `{name}`; declare one under \
                         `model.roles` or give {flag}",
                        path.display()
                    )
                })?,
        };
        let model = from_routes(&routes, &model_name)
            .map_err(|error| format!("the role `{name}` ({flag} {model_name}): {error}"))?;
        let capabilities = model.capabilities();
        if !capabilities.tools || !capabilities.tool_choice {
            return Err(format!(
                "the role `{name}` is served by `{model_name}`, a model that cannot take a forced \
                 tool call"
            ));
        }
        Ok(model)
    };
    let classifier = served(
        ModelRole::Classifier,
        "classifier",
        "--classifier-model",
        classifier,
    )?;
    let agent = served(ModelRole::Agent, "agent", "--model", model)?;
    Ok((Box::new(ArcModel(classifier)), agent))
}

/// A boxed view of a shared model.
struct ArcModel(Arc<dyn Model>);

impl Model for ArcModel {
    fn provenance(&self) -> &llm_core::Provenance {
        self.0.provenance()
    }

    fn capabilities(&self) -> &llm_core::Capabilities {
        self.0.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a llm_core::TurnRequest,
        sink: &'a mut dyn llm_core::StreamSink,
        cancel: &'a llm_core::Cancel,
    ) -> llm_core::BoxFuture<'a, Result<llm_core::TurnOutcome, llm_core::Error>> {
        self.0.turn(request, sink, cancel)
    }
}

/// The exit status of a failed `agent resume`, as [`crate::AGENT_RESUME_HELP`] states it.
pub fn resume_exit_status(failure: &ResumeFailure) -> u8 {
    match failure {
        ResumeFailure::NotHeld(_) => 3,
        ResumeFailure::NotGranted => 4,
        ResumeFailure::Refused(_) => 5,
        ResumeFailure::Failed(_) => 1,
    }
}

/// The line a serveable spec prints.
pub fn checked_line(checked: &Checked) -> String {
    printable(&format!(
        "checked {}: {} states, {} bindings, protocol {}",
        checked.agent,
        checked.states,
        checked.bindings,
        checked.protocol.as_deref().unwrap_or("none")
    ))
}

/// The line a refused spec prints on standard error.
pub fn refused_line(refusal: &SpecRefused) -> String {
    printable(&format!("refused: {}", describe(refusal)))
}
