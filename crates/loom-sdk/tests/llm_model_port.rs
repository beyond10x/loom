//! `b10x-loom` runs on the SDK's llm-backed model port (`loom_sdk::llm::LlmModelPort`) and defines
//! none of its own. The CLI's source is read at run time from the tree the test runs in.

use std::fs;
use std::path::PathBuf;

use loom_sdk::loom::harness::wire::ModelPort;

/// A file of this crate, read at run time.
fn crate_file(relative: &str) -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    PathBuf::from(manifest).join(relative)
}

/// `b10x-loom` runs on the SDK's llm-backed port and defines no model port of its own.
#[test]
fn the_cli_runs_on_the_sdk_llm_port() {
    fn is_a_model_port<T: ModelPort>() {}
    is_a_model_port::<loom_sdk::llm::LlmModelPort>();

    let cli = crate_file("../loom-cli/src");
    let mut sources = Vec::new();
    let mut pending = vec![cli];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("the CLI source is read") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                sources.push((
                    path.clone(),
                    fs::read_to_string(&path).expect("a source file is read"),
                ));
            }
        }
    }
    for (path, source) in &sources {
        assert!(
            !source.contains("impl ModelPort for"),
            "{} defines a model port of its own",
            path.display()
        );
    }
    assert!(
        sources
            .iter()
            .any(|(_, source)| source.contains("loom_sdk::llm::LlmModelPort")),
        "the CLI names the SDK's llm-backed port"
    );
}
