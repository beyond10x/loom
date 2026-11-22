//! Programs beside the bindings file (`story:bound-program-contract`): with `program_root:
//! bindings`, every `argv[0]` and `arguments_from[0]` that is relative and holds a `/` names a
//! file relative to the directory of the bindings file, and [`resolve_programs`] replaces it by
//! the absolute real path of that file before anything runs.

use std::path::{Component, Path};

use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason, ProgramRoot};

use crate::{Refusal, refuse};

/// `document` with each program path it names beside its bindings file replaced by the absolute
/// real path (links resolved) of that file under `directory`, the directory of the bindings file:
/// with `program_root: bindings`, every `executor.argv[0]` and `arguments_from[0]`, of any
/// executor, that is relative and holds a `/`. A bare name (found on `PATH`) and an absolute path
/// (run as written) are left alone, and a document whose `program_root` is `workspace` or absent
/// is answered unchanged. Nothing is started; only the file system is read.
///
/// # Errors
/// `ProgramNotBesideBindings`, naming the action and the member: an entry with a `..` segment, one
/// that names nothing or no regular file, one whose real path lies outside the real path of
/// `directory` (a link out of it included), and one whose real path is not UTF-8; and, naming
/// `program_root`, a `directory` that cannot be read.
pub fn resolve_programs(
    document: &BindingsDocument,
    directory: &Path,
) -> Result<BindingsDocument, Refusal> {
    let mut resolved = document.clone();
    if document.program_root != Some(ProgramRoot::Bindings) {
        return Ok(resolved);
    }
    let directory = directory.canonicalize().map_err(|error| {
        refuse(
            Reason::ProgramNotBesideBindings,
            None,
            "program_root",
            format!(
                "the directory of the bindings file cannot be read: {}",
                error.kind()
            ),
        )
    })?;
    for bound in &mut resolved.actions {
        let action = bound.action.clone();
        for (member, argv) in [
            ("executor.argv[0]", bound.executor.argv.as_mut()),
            ("arguments_from[0]", bound.arguments_from.as_mut()),
        ] {
            if let Some(program) = argv.and_then(|argv| argv.first_mut()) {
                resolve(&directory, program, &action, member)?;
            }
        }
    }
    Ok(resolved)
}

/// Whether `program` names a file beside the bindings file under `program_root: bindings`: it is
/// relative and holds a `/`.
pub(crate) fn beside(program: &str) -> bool {
    program.contains('/') && !Path::new(program).is_absolute()
}

/// Replaces `program`, when it is [`beside`] the bindings file, by its absolute real path under
/// `directory` (already canonical).
fn resolve(
    directory: &Path,
    program: &mut String,
    action: &str,
    member: &str,
) -> Result<(), Refusal> {
    if !beside(program) {
        return Ok(());
    }
    let refused = |why: &str| {
        Err(refuse(
            Reason::ProgramNotBesideBindings,
            Some(action),
            member,
            format!("{program:?} {why}; it is a file in the directory of the bindings file"),
        ))
    };
    if program.contains('\0') {
        return refused("holds a NUL byte");
    }
    if Path::new(program.as_str())
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return refused("has a .. segment");
    }
    let Ok(real) = directory.join(program.as_str()).canonicalize() else {
        return refused("names nothing");
    };
    if !real.starts_with(directory) {
        return refused(
            "lies outside the directory of the bindings file once its links are resolved",
        );
    }
    if !std::fs::metadata(&real).is_ok_and(|found| found.file_type().is_file()) {
        return refused("names no regular file");
    }
    let Some(real) = real.to_str() else {
        return refused("has a real path that is not UTF-8");
    };
    *program = real.to_owned();
    Ok(())
}
