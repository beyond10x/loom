use loom_protocols::{
    InstallStore, ProtocolCatalog, ProtocolSource, SYSTEM_QUERY_YAML, SourceKind,
};
use std::{fs, path::Path, process::Command};
fn memory() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "test".into(),
        revision: String::new(),
        path: String::new(),
    }
}
#[test]
fn catalog_composes_sources_and_accepts_renamed_clock_semantics() {
    let mut catalog = ProtocolCatalog::bundled().unwrap();
    assert!(catalog.get("software-change@1").is_some());
    assert!(
        catalog
            .get("system-query@1")
            .unwrap()
            .clock_compatible()
            .is_ok()
    );
    assert!(
        ProtocolCatalog::engineering()
            .unwrap()
            .get("system-query@1")
            .is_none()
    );
    let yaml = SYSTEM_QUERY_YAML
        .replace("id: system.query", "id: custom.clock")
        .replace(
            "read this machine's clock and return local time and UTC from one instant",
            "read clock",
        );
    catalog.add_yaml("custom-clock@1", &yaml, memory()).unwrap();
    assert!(
        catalog
            .get("custom-clock@1")
            .unwrap()
            .clock_compatible()
            .is_ok()
    );
    assert!(catalog.add_yaml("custom-clock@1", &yaml, memory()).is_err());
    assert!(
        catalog
            .get("software-change@1")
            .unwrap()
            .clock_compatible()
            .is_err()
    );
}
#[test]
fn clock_binding_refuses_changed_authority_and_completion_semantics() {
    let variants = [
        SYSTEM_QUERY_YAML.replace("effect: read", "effect: write"),
        SYSTEM_QUERY_YAML.replace("      claim: time.observed", "      all: []"),
        SYSTEM_QUERY_YAML.replace("artifacts:\n", "artifacts:\n  extra: {}\n"),
        SYSTEM_QUERY_YAML.replace(
            "actions:\n",
            "actions:\n  system.extra:\n    effect: read\n",
        ),
    ];
    for yaml in variants {
        let mut catalog = ProtocolCatalog::default();
        if catalog.add_yaml("custom@1", &yaml, memory()).is_ok() {
            assert!(catalog.get("custom@1").unwrap().clock_compatible().is_err());
        }
    }
}
#[test]
fn malformed_registration_revision_and_oversized_definitions_refuse() {
    let mut catalog = ProtocolCatalog::default();
    for name in [
        "foo", "../foo@1", "foo@01", "foo@0", "foo@2", "Foo@1", "foo/@1",
    ] {
        assert!(
            catalog.add_yaml(name, SYSTEM_QUERY_YAML, memory()).is_err(),
            "{name}"
        );
    }
    assert!(catalog.add_yaml("foo@1", "garbage", memory()).is_err());
    assert!(
        catalog
            .add_yaml("foo@1", &"x".repeat(1024 * 1024 + 1), memory())
            .is_err()
    );
}
#[test]
fn local_install_is_snapshot_atomic_and_explicitly_replaceable() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("clock.yaml");
    fs::write(&source, SYSTEM_QUERY_YAML).unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    store.install_file("custom@1", &source, false).unwrap();
    assert!(store.install_file("custom@1", &source, false).is_err());
    assert!(store.install_file("system-query@1", &source, true).is_err());
    assert!(store.remove("software-change@1").is_err());
    fs::write(&source, "broken").unwrap();
    assert!(store.install_file("custom@1", &source, true).is_err());
    assert_eq!(
        store
            .catalog()
            .unwrap()
            .get("custom@1")
            .unwrap()
            .definition
            .yaml,
        SYSTEM_QUERY_YAML
    );
    let changed = SYSTEM_QUERY_YAML.replace("id: system.query", "id: other.clock");
    fs::write(&source, &changed).unwrap();
    store.install_file("custom@1", &source, true).unwrap();
    fs::remove_file(&source).unwrap();
    assert_eq!(
        store
            .catalog()
            .unwrap()
            .get("custom@1")
            .unwrap()
            .definition
            .yaml,
        changed
    );
    store.remove("custom@1").unwrap();
    assert!(store.catalog().unwrap().get("custom@1").is_none());
}
#[test]
fn corrupt_digest_missing_fields_and_wrong_registration_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("clock.yaml");
    let root = tmp.path().join("installed");
    fs::write(&source, SYSTEM_QUERY_YAML).unwrap();
    let store = InstallStore::new(root.clone());
    for mode in 0..4 {
        store.install_file("custom@1", &source, true).unwrap();
        let path = root.join("custom@1.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match mode {
            0 => json["sha256"] = "forged".into(),
            1 => {
                json.as_object_mut().unwrap().remove("yaml");
            }
            2 => json["name"] = "other@1".into(),
            _ => json["source"]["kind"] = "loom".into(),
        }
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(store.catalog().is_err());
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
#[test]
fn pinned_git_regular_blob_installs_and_runs_offline() {
    let tmp = tempfile::tempdir().unwrap();
    let repository = tmp.path().join("source");
    fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(repository.join("clock.yaml"), SYSTEM_QUERY_YAML).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("clock.yaml", repository.join("link.yaml")).unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "clock data"]);
    let revision = git(&repository, &["rev-parse", "HEAD"]);
    let source = format!(
        "git+{}#{revision}",
        url::Url::from_directory_path(&repository).unwrap()
    );
    let store = InstallStore::new(tmp.path().join("installed"));
    assert!(
        store
            .install_git("missing@1", &source, "missing.yaml", false)
            .is_err()
    );
    #[cfg(unix)]
    assert!(
        store
            .install_git("symlink@1", &source, "link.yaml", false)
            .is_err()
    );
    store
        .install_git("custom@1", &source, "clock.yaml", false)
        .unwrap();
    fs::rename(&repository, tmp.path().join("unavailable")).unwrap();
    let catalog = store.catalog().unwrap();
    let custom = catalog.get("custom@1").unwrap();
    custom.clock_compatible().unwrap();
    assert_eq!(custom.definition.source.revision, revision);
    assert_eq!(custom.definition.yaml, SYSTEM_QUERY_YAML);
}
#[test]
fn git_source_validation_refuses_unpinned_credentials_transports_and_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    let revision = "a".repeat(40);
    for source in [
        "git+https://example.invalid/repo#main".into(),
        format!("git+https://user:secret@example.invalid/repo#{revision}"),
        format!("git+ext::evil#{revision}"),
        format!("git+http://example.invalid/repo#{revision}"),
        format!("git+https://example.invalid/repo?token=secret#{revision}"),
    ] {
        assert!(
            store
                .install_git("custom@1", &source, "clock.yaml", false)
                .is_err()
        );
    }
    for path in [
        "../clock.yaml",
        "/clock.yaml",
        "a/../clock.yaml",
        "a//b",
        "a\\b",
    ] {
        assert!(
            store
                .install_git(
                    "custom@1",
                    &format!("git+https://example.invalid/repo#{revision}"),
                    path,
                    false
                )
                .is_err()
        );
    }
    assert!(!tmp.path().join("installed").exists());
}

#[test]
fn snapshots_preserve_unicode_escaping_and_reject_symlink_records() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("clock.yaml");
    let yaml = SYSTEM_QUERY_YAML.replace(
        "the exact system query for this run",
        "'時刻 café \\\\ \"quoted\"'",
    );
    fs::write(&source, &yaml).unwrap();
    let root = tmp.path().join("installed");
    let store = InstallStore::new(root.clone());
    store.install_file("custom@1", &source, false).unwrap();
    assert_eq!(
        store
            .catalog()
            .unwrap()
            .get("custom@1")
            .unwrap()
            .definition
            .yaml,
        yaml
    );
    #[cfg(unix)]
    {
        let record = root.join("custom@1.json");
        let hidden = tmp.path().join("record.json");
        fs::rename(&record, &hidden).unwrap();
        std::os::unix::fs::symlink(&hidden, &record).unwrap();
        assert!(store.catalog().is_err());
        assert!(store.install_file("custom@1", &source, false).is_err());
        // Explicit replacement replaces the link, without touching its target.
        store.install_file("custom@1", &source, true).unwrap();
        assert!(hidden.is_file());
        assert!(store.catalog().is_ok());
    }
}

#[test]
fn custom_git_attributes_and_hooks_are_never_executed() {
    let tmp = tempfile::tempdir().unwrap();
    let repository = tmp.path().join("source");
    fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(repository.join("clock.yaml"), SYSTEM_QUERY_YAML).unwrap();
    fs::write(
        repository.join(".gitattributes"),
        "clock.yaml filter=poison export-subst\n",
    )
    .unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "data with attributes"]);
    let revision = git(&repository, &["rev-parse", "HEAD"]);
    git(&repository, &["config", "filter.poison.smudge", "false"]);
    git(&repository, &["config", "filter.poison.required", "true"]);
    let source = format!(
        "git+{}#{revision}",
        url::Url::from_directory_path(&repository).unwrap()
    );
    let store = InstallStore::new(tmp.path().join("installed"));
    store
        .install_git("custom@1", &source, "clock.yaml", false)
        .unwrap();
    assert_eq!(
        store
            .catalog()
            .unwrap()
            .get("custom@1")
            .unwrap()
            .definition
            .yaml,
        SYSTEM_QUERY_YAML
    );
    let invalid = source.replace(&revision, &"0".repeat(40));
    assert!(
        store
            .install_git("custom@1", &invalid, "clock.yaml", true)
            .is_err()
    );
    assert_eq!(
        store
            .catalog()
            .unwrap()
            .get("custom@1")
            .unwrap()
            .definition
            .yaml,
        SYSTEM_QUERY_YAML
    );
}
