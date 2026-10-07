//! Exercise the actual CLI with an isolated installation store and no model access.
use std::path::Path;
use std::process::{Command, Output};

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(args)
        .env("XDG_DATA_HOME", root)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .output()
        .unwrap()
}

#[test]
fn file_install_list_replace_and_remove_are_available_without_models() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("clock.yaml");
    std::fs::write(
        &source,
        loom_protocols::SYSTEM_QUERY_YAML.replace("system.query", "custom.clock"),
    )
    .unwrap();
    let args = [
        "protocols",
        "add",
        "clock-check@1",
        "--file",
        source.to_str().unwrap(),
    ];
    let installed = cli(directory.path(), &args);
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(!cli(directory.path(), &args).status.success());
    let mut replace = args.to_vec();
    replace.push("--replace");
    assert!(cli(directory.path(), &replace).status.success());
    std::fs::remove_file(source).unwrap();
    let listed = cli(directory.path(), &["protocols", "list"]);
    assert!(listed.status.success());
    let list = String::from_utf8(listed.stdout).unwrap();
    assert!(list.contains("clock-check@1"), "{list}");
    assert!(list.contains("Local"), "{list}");
    assert!(list.contains("sha256:"), "{list}");
    assert!(list.contains("system-query@1"), "{list}");
    assert!(
        cli(directory.path(), &["protocols", "remove", "clock-check@1"])
            .status
            .success()
    );
    assert!(
        !cli(directory.path(), &["protocols", "remove", "system-query@1"])
            .status
            .success()
    );
}

#[test]
fn corrupt_installation_fails_before_models_and_writes_zero_call_report() {
    let directory = tempfile::tempdir().unwrap();
    let store = directory.path().join("loom/protocols");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("broken@1.json"), "broken").unwrap();
    let report = directory.path().join("report.json");
    let output = cli(
        directory.path(),
        &[
            "run",
            "--context-policy",
            "bounded",
            "--context-report",
            report.to_str().unwrap(),
            "current time",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("confinement"));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(report["model_calls"], 0);
    assert_eq!(report["policy"], "bounded");
}
