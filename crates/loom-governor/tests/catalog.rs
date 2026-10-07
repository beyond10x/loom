//! A host catalog is the exact immutable protocol set the governor admits.
use std::collections::BTreeMap;

use b10x_loom_commission::ports::governor::Governor;
use loom_governor::{CanonGovernor, MemoryCaseStore, OpenError};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};

const YAML: &str = r#"
format: protocol/1
protocol: {id: custom.clock, revision: 1, description: Read a host clock.}
artifacts: {intent: {}}
evidence_kinds: {time_observation: {}}
claims:
  observed:
    true_when:
      evidence: {kind: time_observation, result: observed, subject: intent}
actions:
  system.time.read:
    effect: read
    may_produce: [{evidence: time_observation}]
outcomes:
  answered:
    requires: {claim: observed}
"#;

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "embedding-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

#[test]
fn host_catalog_definitions_open_and_project_without_builtin_special_cases() {
    let mut catalog = ProtocolCatalog::default();
    catalog.add_yaml("custom-clock@1", YAML, source()).unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    let case = governor
        .open(
            "custom-clock@1",
            BTreeMap::from([("intent".into(), "query-1".into())]),
        )
        .unwrap();
    let frontier = governor.frontier(&case).unwrap().into_data();
    assert_eq!(frontier.actions.len(), 1);
    assert_eq!(frontier.actions[0].action, "system.time.read");
    assert!(matches!(
        governor.open("software-change@1", BTreeMap::new()),
        Err(OpenError::UnknownProtocol { .. })
    ));
    assert!(matches!(
        governor.open("custom-clock@2", BTreeMap::new()),
        Err(OpenError::UnknownProtocol { .. })
    ));
}

#[test]
fn prior_registration_and_subsequent_extension_cannot_change_catalog_definitions() {
    let catalog = ProtocolCatalog::bundled().unwrap();
    let prior = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("custom-clock@1", YAML)
        .unwrap();
    assert!(matches!(
        prior.with_catalog(&catalog),
        Err(OpenError::InvalidProtocol { .. })
    ));

    let sealed = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    assert!(matches!(
        sealed.with_protocol_yaml("custom-clock@1", YAML),
        Err(OpenError::InvalidProtocol { .. })
    ));

    let sealed = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .unwrap();
    assert!(matches!(
        sealed.with_catalog(&catalog),
        Err(OpenError::InvalidProtocol { .. })
    ));
}

#[test]
fn catalog_rejects_definitions_commission_cannot_represent() {
    let mut catalog = ProtocolCatalog::engineering().unwrap();
    let yaml = YAML.replace(
        "effect: read",
        "effect: read\n    requires: [{capability: one}, {capability: two}]",
    );
    catalog.add_yaml("custom-clock@1", &yaml, source()).unwrap();
    assert!(
        matches!(CanonGovernor::new(MemoryCaseStore::default()).with_catalog(&catalog),
        Err(OpenError::InvalidProtocol { problem, .. }) if problem.contains("at most one capability"))
    );
}

#[test]
fn case_initializer_must_use_the_same_definition_bytes_as_the_governor() {
    let mut admitted = ProtocolCatalog::engineering().unwrap();
    admitted.add_yaml("custom-clock@1", YAML, source()).unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&admitted)
        .unwrap();
    governor.validate_catalog(&admitted).unwrap();

    let mut changed = ProtocolCatalog::engineering().unwrap();
    changed
        .add_yaml(
            "custom-clock@1",
            &YAML.replace("Read a host clock.", "A different description."),
            source(),
        )
        .unwrap();
    assert!(matches!(
        governor.validate_catalog(&changed),
        Err(OpenError::InvalidProtocol { .. })
    ));
    assert!(matches!(
        governor.validate_catalog(&ProtocolCatalog::engineering().unwrap()),
        Err(OpenError::InvalidProtocol { .. })
    ));
}
