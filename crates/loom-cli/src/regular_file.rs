//! Opening a file the operator named on the command line, refusing anything but a regular file.
//!
//! A named pipe, a device or a directory is refused before it is read: opening a pipe nobody
//! writes blocks forever, and a device such as `/dev/zero` never ends. The path is checked
//! before it is opened, and the opened handle again, so a path swapped for a pipe in between is
//! refused too; on Unix the open itself does not wait for a writer (`O_NONBLOCK`, which has no
//! effect on a regular file).

use std::fs::File;
use std::path::Path;

/// Opens `path` for reading if it is a regular file (a symbolic link is followed).
///
/// # Errors
/// The reason, without the path, for the caller to name it: the file cannot be inspected or
/// opened, or it is not a regular file.
pub fn open(path: &Path) -> Result<File, String> {
    let not_regular = || "is not a regular file".to_owned();
    let metadata = std::fs::metadata(path).map_err(|error| format!("cannot be read: {error}"))?;
    if !metadata.is_file() {
        return Err(not_regular());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("cannot be read: {error}"))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot be read: {error}"))?;
    if !opened.is_file() {
        return Err(not_regular());
    }
    Ok(file)
}
