//! `loom.resource.StageFiles` (`ess/domains/resource.yaml`), decided: what `b10x-loom agent
//! stage-files` may do with a mounted Secret volume and its destination, from the facts it read
//! of both before writing anything.
//!
//! [`stage_files`] refuses in declaration order (a destination that is a link, one that is not a
//! directory, a key whose name is not UTF-8, a key resolving outside the volume, a directory in
//! the destination no key names) and otherwise answers the modes the staged directory and each
//! staged file take, [`DIR_MODE`] and [`FILE_MODE`]. The command reads the facts and does the
//! copying; this module touches no file.

use loom::resource::{
    StageDestination, StageFiles, StageFilesOutcome, StageFilesResponse, StageRefusalReason,
    StageRefused,
};

/// The mode of the staged directory: its owner's only.
pub const DIR_MODE: u32 = 0o700;
/// The mode of each staged file: readable and writable by its owner only.
pub const FILE_MODE: u32 = 0o600;

/// Decides `loom.resource.StageFiles`: the first refusal in declaration order, else `staged`
/// with [`DIR_MODE`] and [`FILE_MODE`].
#[must_use]
pub fn stage_files(input: &StageFiles) -> StageFilesOutcome {
    let refused = |reason: StageRefusalReason| StageRefused { reason };
    match input.destination {
        StageDestination::Link => {
            return StageFilesOutcome::DestinationIsALink {
                error: refused(StageRefusalReason::DestinationIsALink),
            };
        }
        StageDestination::Other => {
            return StageFilesOutcome::DestinationNotADirectory {
                error: refused(StageRefusalReason::DestinationNotADirectory),
            };
        }
        StageDestination::Absent | StageDestination::Directory => {}
    }
    if !input.names_utf8 {
        return StageFilesOutcome::NameNotUtf8 {
            error: refused(StageRefusalReason::NameNotUtf8),
        };
    }
    if !input.keys_inside {
        return StageFilesOutcome::KeyOutsideVolume {
            error: refused(StageRefusalReason::KeyOutsideVolume),
        };
    }
    if input.stale_directory {
        return StageFilesOutcome::StaleDirectory {
            error: refused(StageRefusalReason::StaleDirectory),
        };
    }
    StageFilesOutcome::Staged {
        response: StageFilesResponse {
            directory_mode: i64::from(DIR_MODE),
            file_mode: i64::from(FILE_MODE),
        },
    }
}

/// `reason` as the enum `loom.resource.StageRefusalReason` spells it.
#[must_use]
pub fn stage_refusal_spelling(reason: StageRefusalReason) -> &'static str {
    match reason {
        StageRefusalReason::DestinationIsALink => "DestinationIsALink",
        StageRefusalReason::DestinationNotADirectory => "DestinationNotADirectory",
        StageRefusalReason::NameNotUtf8 => "NameNotUtf8",
        StageRefusalReason::KeyOutsideVolume => "KeyOutsideVolume",
        StageRefusalReason::StaleDirectory => "StaleDirectory",
    }
}

/// `destination` as the enum `loom.resource.StageDestination` spells it.
#[must_use]
pub fn stage_destination_spelling(destination: StageDestination) -> &'static str {
    match destination {
        StageDestination::Absent => "Absent",
        StageDestination::Directory => "Directory",
        StageDestination::Link => "Link",
        StageDestination::Other => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(destination: StageDestination) -> StageFiles {
        StageFiles {
            destination,
            names_utf8: true,
            keys_inside: true,
            stale_directory: false,
        }
    }

    #[test]
    fn the_destination_is_decided_before_any_key() {
        let every_fact_wrong = |destination| StageFiles {
            destination,
            names_utf8: false,
            keys_inside: false,
            stale_directory: true,
        };
        assert!(matches!(
            stage_files(&every_fact_wrong(StageDestination::Link)),
            StageFilesOutcome::DestinationIsALink { .. }
        ));
        assert!(matches!(
            stage_files(&every_fact_wrong(StageDestination::Other)),
            StageFilesOutcome::DestinationNotADirectory { .. }
        ));
        assert!(matches!(
            stage_files(&every_fact_wrong(StageDestination::Directory)),
            StageFilesOutcome::NameNotUtf8 { .. }
        ));
    }

    #[test]
    fn a_key_outside_the_volume_is_decided_before_a_stale_directory() {
        let decided = stage_files(&StageFiles {
            keys_inside: false,
            stale_directory: true,
            ..input(StageDestination::Directory)
        });
        assert!(matches!(
            decided,
            StageFilesOutcome::KeyOutsideVolume { .. }
        ));
    }

    #[test]
    fn a_destination_absent_or_a_directory_is_staged_private() {
        for destination in [StageDestination::Absent, StageDestination::Directory] {
            assert_eq!(
                stage_files(&input(destination)),
                StageFilesOutcome::Staged {
                    response: StageFilesResponse {
                        directory_mode: 0o700,
                        file_mode: 0o600,
                    },
                },
                "{}",
                stage_destination_spelling(destination)
            );
        }
    }
}
