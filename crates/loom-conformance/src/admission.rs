//! `loom.resource.AdmitLoomAgent`, answered by `b10x_loom_operator::admission::admit` (then
//! `schedule_name`) over the plugin catalog, and `loom.resource.StageFiles`, answered by
//! `b10x_loom_resource::stage::stage_files` over its declared inputs alone.
//!
//! `AdmitLoomAgent`'s one input is the resource's `path`, and every refusal is `external:`: the
//! resource, the controller configuration and the catalog are outside the input. This target
//! plays the file system and the controller, as it plays the governor for `not-in-frontier`: the
//! configuration is always `ess/scenarios/resource-admission/config.yaml`; a `path` naming a
//! regular file under the repository root is read as it is (the authored scenarios name the
//! fixtures under `ess/scenarios/resource-admission/resources/`); a synthesized path names none,
//! and the target reads the fixture of the branch the scenario forced with
//! `configure_external_outcome`, `resources/<branch>.yaml`, or, forced nothing,
//! `resources/admitted.yaml`. Admission decides the outcome from what it read: each fixture holds
//! the one defect of its branch, so an admission answering another reason fails the scenario. A
//! forced branch holds for the next invocation only. The branch of a refusal is its reason in
//! kebab case (`NotAVariableName` is `not-a-variable-name`), which is how the specification names
//! each outcome; a reason it does not declare answers a branch it does not declare, and the
//! scenario fails.

use std::path::Path;

use b10x_loom_operator::admission::{Refusal, admit, schedule_name};
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::parse_resource;
use b10x_loom_resource::model::{
    StageDestination, StageFiles, StageFilesOutcome, StageFilesResponse, StageRefused,
};
use b10x_loom_resource::stage::{stage_files, stage_refusal_spelling};
use ess_conformance::target::{SemanticCommandResult, TargetError};
use ess_primitives::node::Node;
use loom_protocols::ProtocolCatalog;

use crate::agent::{file_under, root};
use crate::codec::{self, Input};
use crate::{Live, error, took};

pub(crate) const ADMIT_LOOM_AGENT: &str = "loom.resource.AdmitLoomAgent";
const ADMISSION_REFUSED: &str = "loom.resource.AdmissionRefused";
pub(crate) const STAGE_FILES: &str = "loom.resource.StageFiles";
const STAGE_REFUSED: &str = "loom.resource.StageRefused";

/// Where the configuration and the resources of the scenarios are, under the repository root.
const FIXTURES: &str = "ess/scenarios/resource-admission";

/// The resource read when the path names no file and nothing was forced.
const ADMITTED: &str = "admitted";

/// Whether `branch` is a refusal branch of `AdmitLoomAgent` a scenario may force: one with a
/// fixture.
pub(crate) fn is_branch(branch: &str) -> bool {
    branch != ADMITTED
        && !branch.is_empty()
        && branch
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        && root().is_ok_and(|root| {
            root.join(FIXTURES)
                .join("resources")
                .join(format!("{branch}.yaml"))
                .is_file()
        })
}

fn read(path: &Path) -> Result<String, TargetError> {
    std::fs::read_to_string(path).map_err(|why| {
        TargetError::unavailable(
            format!("reading {}", path.display()),
            format!("the file cannot be read: {why}"),
        )
    })
}

/// The branch a refusal for `reason` answers: the reason in kebab case.
fn branch(reason: &str) -> String {
    let mut branch = String::with_capacity(reason.len() + 4);
    for (index, letter) in reason.chars().enumerate() {
        if letter.is_ascii_uppercase() {
            if index > 0 {
                branch.push('-');
            }
            branch.push(letter.to_ascii_lowercase());
        } else {
            branch.push(letter);
        }
    }
    branch
}

/// The refusal as `loom.resource.AdmissionRefused`.
fn admission_refusal(refusal: Refusal) -> SemanticCommandResult {
    took(ADMIT_LOOM_AGENT, &branch(&refusal.reason)).with_error(
        error(ADMISSION_REFUSED)
            .with("reason", Node::Text(refusal.reason))
            .with("member", Node::Text(refusal.member))
            .with("detail", Node::Text(refusal.detail)),
    )
}

/// Answers one `AdmitLoomAgent`: `None` when the input carries no text `path`.
pub(crate) fn admit_loom_agent(
    live: &mut Live,
    input: &Input,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(path) = codec::text(input, "path") else {
        return Ok(None);
    };
    let forced = live.forced_admission.take();
    let root = root()?;
    let fixtures = root.join(FIXTURES);
    let resource = if let Some(named) = file_under(&root, &path) {
        named
    } else {
        let branch = forced.as_deref().unwrap_or(ADMITTED);
        fixtures.join("resources").join(format!("{branch}.yaml"))
    };
    let config = Config::parse(&read(&fixtures.join("config.yaml"))?).map_err(|why| {
        TargetError::unavailable("reading the scenarios' controller configuration", why.0)
    })?;
    let agent = parse_resource(&read(&resource)?).map_err(|why| {
        TargetError::unavailable(format!("decoding {}", resource.display()), why.to_string())
    })?;
    let name = agent.metadata.name.clone().unwrap_or_default();
    let namespace = agent
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| config.namespace.clone());
    let catalog = ProtocolCatalog::plugins()
        .map_err(|why| TargetError::unavailable("loading the plugin catalog", why))?;
    let actions = |protocol: &str| catalog.get(protocol).map(|entry| entry.actions());
    Ok(Some(
        match admit(&agent.spec, &namespace, &config, &actions)
            .and_then(|_| schedule_name(&name, &agent.spec))
        {
            Ok(()) => took(ADMIT_LOOM_AGENT, ADMITTED),
            Err(refusal) => admission_refusal(refusal),
        },
    ))
}

/// The `loom.resource.StageDestination` field `name`, in the enum's spelling.
fn destination(input: &Input, name: &str) -> Option<StageDestination> {
    match codec::text(input, name)?.as_str() {
        "Absent" => Some(StageDestination::Absent),
        "Directory" => Some(StageDestination::Directory),
        "Link" => Some(StageDestination::Link),
        "Other" => Some(StageDestination::Other),
        _ => None,
    }
}

/// The refusal as `loom.resource.StageRefused`, `reason` in the enum's spelling.
fn stage_refusal(branch: &str, refused: &StageRefused) -> SemanticCommandResult {
    took(STAGE_FILES, branch).with_error(error(STAGE_REFUSED).with(
        "reason",
        Node::Text(stage_refusal_spelling(refused.reason).to_owned()),
    ))
}

/// `staged`, returning the modes.
fn staged(response: &StageFilesResponse) -> SemanticCommandResult {
    let mut result = took(STAGE_FILES, "staged");
    result.response = Some(
        [
            (
                "directory_mode".to_owned(),
                codec::number(response.directory_mode),
            ),
            ("file_mode".to_owned(), codec::number(response.file_mode)),
        ]
        .into_iter()
        .collect(),
    );
    result
}

/// Answers one `StageFiles`: `None` when an input is absent or cannot be held by its type.
pub(crate) fn stage_files_command(input: &Input) -> Option<SemanticCommandResult> {
    let request = StageFiles {
        destination: destination(input, "destination")?,
        names_utf8: codec::boolean(input, "names_utf8")?,
        keys_inside: codec::boolean(input, "keys_inside")?,
        stale_directory: codec::boolean(input, "stale_directory")?,
    };
    Some(match stage_files(&request) {
        StageFilesOutcome::DestinationIsALink { error } => {
            stage_refusal("destination-is-a-link", &error)
        }
        StageFilesOutcome::DestinationNotADirectory { error } => {
            stage_refusal("destination-not-a-directory", &error)
        }
        StageFilesOutcome::NameNotUtf8 { error } => stage_refusal("name-not-utf8", &error),
        StageFilesOutcome::KeyOutsideVolume { error } => {
            stage_refusal("key-outside-volume", &error)
        }
        StageFilesOutcome::StaleDirectory { error } => stage_refusal("stale-directory", &error),
        StageFilesOutcome::Staged { response } => staged(&response),
    })
}
