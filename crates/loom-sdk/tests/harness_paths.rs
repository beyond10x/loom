//! Every `loom_sdk` path the "Move from Harness" section of
//! `website/docs/guides/move-to-loom.md` names resolves.
//!
//! The module below is the compile witness: one `use` per path the section names, so a path that
//! moves or disappears fails the build. The test holds the two lists equal, so a path added to
//! the guide without a `use` here, or a `use` the guide no longer names, fails it.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[allow(unused_imports)]
mod witness {
    use loom_sdk::Loom;
    use loom_sdk::loom::credentials::ResolvedBearer;
    use loom_sdk::loom::harness::http::HttpTransport;
    use loom_sdk::loom::harness::http::RetryPolicy;
    use loom_sdk::loom::harness::messages::MessagesClient;
    use loom_sdk::loom::harness::responses::ResponsesClient;
    use loom_sdk::loom::harness::turn_loop::AgentLoop;
    use loom_sdk::loom::harness::wire::BearerSource;
    use loom_sdk::loom::harness::wire::ModelPort;
    use loom_sdk::loom::harness::wire::ToolPort;
    use loom_sdk::loom::session;
}

const SECTION: &str = "## Move from Harness";

/// Reads a file of the tree this test is run from, relative to this crate's manifest directory.
/// The directory is taken at run time: a test binary reused from a shared target directory must
/// not read the tree it was built in.
fn read(relative: &str) -> String {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    let path = PathBuf::from(manifest).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The text of the guide's section, up to the next level-two heading; the heading appears once.
fn section(page: &str) -> String {
    let count = page.lines().filter(|line| line.trim() == SECTION).count();
    assert_eq!(count, 1, "`{SECTION}` appears {count} times; expected once");
    page.lines()
        .skip_while(|line| line.trim() != SECTION)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every code span of `text` that starts `loom_sdk`.
fn guide_paths(text: &str) -> BTreeSet<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| *span == "loom_sdk" || span.starts_with("loom_sdk::"))
        .map(str::to_owned)
        .collect()
}

/// The path of every `use loom_sdk…;` line of the witness module. Each names one path, with no
/// group, glob or rename, so a line is exactly the path it proves.
fn witness_paths(source: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for line in source.lines().map(str::trim) {
        let Some(path) = line
            .strip_prefix("use loom_sdk")
            .and_then(|rest| rest.strip_suffix(';'))
            .map(|rest| format!("loom_sdk{rest}"))
        else {
            continue;
        };
        assert!(
            !path.contains(['{', '*', ' ']),
            "`{line}` names more than one path"
        );
        assert!(paths.insert(path), "`{line}` repeats a path");
    }
    paths
}

#[test]
fn every_path_the_harness_guide_names_resolves() {
    let guide = guide_paths(&section(&read("../../website/docs/guides/move-to-loom.md")));
    assert!(
        !guide.is_empty(),
        "the section names no loom_sdk path"
    );
    let witnessed = witness_paths(&read("tests/harness_paths.rs"));
    assert_eq!(
        guide, witnessed,
        "the loom_sdk paths the guide names and the uses of this file's witness differ"
    );
}
