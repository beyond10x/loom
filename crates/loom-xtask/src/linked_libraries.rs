//! `linked-libraries <binary>`: a released `b10x-loom` starts on a machine that lacks Git's and
//! OpenSSL's shared libraries (story:drop-the-substrate-git-feature).
//!
//! It runs `ldd` on the binary and fails, naming each library, when `ldd` names one of
//! [`REFUSED`]: `libgit2`, `libssh2`, `libssl`, `libcrypto` or `libz`. A library is matched by its
//! file name up to `.so`, so `libz.so.1` is refused and `libzstd.so.1` is not. A binary `ldd` calls
//! not dynamic links no shared library and passes.
//!
//! The command needs a built binary, so no test runs it: the tests read recorded `ldd` output
//! (`tests/fixtures/ldd/`) through [`refused_libraries`].

use std::path::Path;
use std::process::Command;

/// The libraries a released binary must not link, each the file name before `.so`.
pub const REFUSED: [&str; 5] = ["libgit2", "libssh2", "libssl", "libcrypto", "libz"];

/// What `ldd` prints for a binary that links no shared library.
const NOT_DYNAMIC: &str = "not a dynamic executable";

/// Runs `ldd` on `binary` and fails, naming each refused library, when it links one.
pub fn linked_libraries(binary: &Path) -> Result<(), String> {
    let output = Command::new("ldd")
        .arg(binary)
        .output()
        .map_err(|e| format!("ldd could not be started: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if stdout.contains(NOT_DYNAMIC) || stderr.contains(NOT_DYNAMIC) {
            println!("`{}` links no shared library", binary.display());
            return Ok(());
        }
        return Err(format!(
            "ldd `{}` failed ({}): {}",
            binary.display(),
            output.status,
            stderr.trim()
        ));
    }
    print!("{stdout}");
    verdict(binary, &stdout)
}

/// The verdict on `ldd`'s output for `binary`: an error naming each refused library, in `ldd`'s
/// order.
fn verdict(binary: &Path, ldd: &str) -> Result<(), String> {
    let refused = refused_libraries(ldd);
    if refused.is_empty() {
        println!(
            "`{}` links none of {}",
            binary.display(),
            REFUSED.join(", ")
        );
        Ok(())
    } else {
        Err(format!(
            "`{}` links {}, which a released binary must not need",
            binary.display(),
            refused.join(", ")
        ))
    }
}

/// The libraries of [`REFUSED`] that `ldd`'s output names, each as `ldd` names it
/// (`libgit2.so.1.9`), in its order.
pub fn refused_libraries(ldd: &str) -> Vec<&str> {
    ldd.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|first| first.rsplit('/').next().unwrap_or(first))
        .filter(|library| {
            library
                .split_once(".so")
                .is_some_and(|(stem, _)| REFUSED.contains(&stem))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ldd` of a debug `b10x-loom` built with Substrate's `git` feature, as recorded on Linux.
    const WITH_LIBGIT2: &str = include_str!("../tests/fixtures/ldd/b10x-loom-with-libgit2.txt");
    /// `ldd` of a `b10x-loom` built without it.
    const WITHOUT_LIBGIT2: &str =
        include_str!("../tests/fixtures/ldd/b10x-loom-without-libgit2.txt");

    #[test]
    fn linked_libraries_refuses_a_binary_that_links_libgit2() {
        assert_eq!(
            refused_libraries(WITH_LIBGIT2),
            [
                "libgit2.so.1.9",
                "libssl.so.3",
                "libcrypto.so.3",
                "libz.so.1",
                "libssh2.so.1"
            ]
        );
        let error = verdict(Path::new("target/debug/b10x-loom"), WITH_LIBGIT2)
            .expect_err("a binary that links libgit2 is refused");
        assert!(error.contains("libgit2.so.1.9"), "{error}");
        assert!(
            error.starts_with("`target/debug/b10x-loom` links "),
            "{error}"
        );
    }

    #[test]
    fn linked_libraries_passes_a_binary_without_them() {
        assert!(
            refused_libraries(WITHOUT_LIBGIT2).is_empty(),
            "{WITHOUT_LIBGIT2}"
        );
        verdict(Path::new("target/release/b10x-loom"), WITHOUT_LIBGIT2)
            .expect("a binary without the refused libraries passes");
    }

    /// A library is its file name up to `.so`: `libzstd` and `libssl3-compat` are not `libz` and
    /// `libssl`, and a library `ldd` names by path or cannot find is still named.
    #[test]
    fn a_library_is_matched_by_its_name_before_so() {
        let ldd = "\tlibzstd.so.1 => /usr/lib/libzstd.so.1 (0x1)\n\
                   \tlibssl3-compat.so.1 => /usr/lib/libssl3-compat.so.1 (0x2)\n\
                   \t/usr/lib/libz.so.1 (0x3)\n\
                   \tlibcrypto.so.3 => not found\n\
                   \tlibgit2.so => /opt/lib/libgit2.so (0x4)\n";
        assert_eq!(
            refused_libraries(ldd),
            ["libz.so.1", "libcrypto.so.3", "libgit2.so"]
        );
    }
}
