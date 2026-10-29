//! Adversary cases for `story:registration-name-protocol-id`: a refused registration names the
//! registration its protocol id takes, only when that name would itself register, from the id the
//! definition declares when it parses, through every caller, without echoing unbounded input.
use loom_protocols::{
    InstallStore, ProtocolCatalog, ProtocolSource, SYSTEM_QUERY_YAML, SourceKind,
};
use std::fs;

fn memory() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "test".into(),
        revision: String::new(),
        path: String::new(),
    }
}

/// The clock definition, declaring protocol `id` at `revision`.
fn declaring(id: &str, revision: u64) -> String {
    let yaml = SYSTEM_QUERY_YAML
        .replace("  id: system.query\n", &format!("  id: '{id}'\n"))
        .replace("  revision: 1\n", &format!("  revision: {revision}\n"));
    assert!(yaml.contains(&format!("  id: '{id}'\n")), "{yaml}");
    assert!(
        yaml.contains(&format!("  revision: {revision}\n")),
        "{yaml}"
    );
    yaml
}

/// The registration a refusal names, if it names one.
fn suggested(refusal: &str) -> Option<String> {
    let (_, rest) = refusal.rsplit_once(" registers as ")?;
    Some(rest.split([',', ' ']).next().unwrap_or(rest).to_owned())
}

fn refusal(name: &str, yaml: &str) -> String {
    ProtocolCatalog::default()
        .add_yaml(name, yaml, memory())
        .expect_err(name)
}

/// `protocols add team.review@2 file.yaml` where the file declares `team.review/1`: the install
/// store's refusal names a registration, and that registration must install. The case used
/// `self.improvement`, which engineering-protocols 0.13.0 bundles, and a bundled registration is
/// never replaced (story:engineering-protocols-0-13-pin).
#[test]
fn protocols_add_names_a_registration_that_installs_when_the_file_declares_another_revision() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("team-review.yaml");
    fs::write(&source, declaring("team.review", 1)).unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    let refused = store
        .install_file("team.review@2", &source, false)
        .expect_err("a dotted registration is refused");
    let named = suggested(&refused).expect("the refusal names a registration");
    if let Err(error) = store.install_file(&named, &source, false) {
        panic!(
            "story:engineering-protocols-0-13-pin: the registration the refusal named, {named}, does not install: {error}\nrefusal: {refused}"
        );
    }
}

/// One host input, two callers: `ProtocolCatalog::add_yaml` and `protocols add` (the install
/// store) name the same registration for the same name and the same definition, the one the
/// definition's protocol id takes.
#[test]
fn add_yaml_and_protocols_add_name_the_same_registration_for_one_input() {
    let yaml = declaring("self.improvement", 1);
    let from_catalog = suggested(&refusal("other.thing@1", &yaml));

    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("self-improvement.yaml");
    fs::write(&source, &yaml).unwrap();
    let refused = InstallStore::new(tmp.path().join("installed"))
        .install_file("other.thing@1", &source, false)
        .expect_err("a dotted registration is refused");
    let from_store = suggested(&refused);

    assert_eq!(from_catalog.as_deref(), Some("self-improvement@1"));
    assert_eq!(
        from_store, from_catalog,
        "protocols add names another registration than add_yaml for the same input: {refused}"
    );
}

/// A refusal that names no registration but states the mapping ("a protocol id registers with
/// its dots and underscores as hyphens ...") must not lead the host to a name the same refusal
/// refuses: for a protocol id Canon accepts, the name the stated mapping gives registers.
#[test]
fn a_refusal_that_states_the_mapping_never_leads_to_a_name_the_same_refusal_refuses() {
    for id in ["3d.render", "Self.Improvement"] {
        let yaml = declaring(id, 1);
        let first = refusal(&format!("{id}@1"), &yaml);
        if suggested(&first).is_some() || !first.contains("dots and underscores as hyphens") {
            continue;
        }
        let mapped = format!("{}@1", id.replace(['.', '_'], "-"));
        if let Err(second) = ProtocolCatalog::default().add_yaml(&mapped, &yaml, memory()) {
            panic!(
                "protocol id {id}/1: the refusal states the mapping, the mapping gives {mapped}, \
                 and {mapped} is refused again\nfirst: {first}\nsecond: {second}"
            );
        }
    }
}

/// A refusal never echoes unbounded input: a 1 MiB name is refused with a bounded message, from
/// `add_yaml` and from the install store (`protocols remove`).
#[test]
fn a_refusal_does_not_echo_an_unbounded_name() {
    let name = format!("{}@1", "x.".repeat(512 * 1024));
    let catalog = refusal(&name, SYSTEM_QUERY_YAML);
    assert!(
        catalog.len() <= 2048,
        "add_yaml refusal is {} bytes for a {}-byte name: {}...",
        catalog.len(),
        name.len(),
        &catalog[..200]
    );
    let tmp = tempfile::tempdir().unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    let removed = store.remove(&name).expect_err("an invalid name is refused");
    assert!(
        removed.len() <= 2048,
        "protocols remove refusal is {} bytes for a {}-byte name: {}...",
        removed.len(),
        name.len(),
        &removed[..200]
    );
}

/// The refusal says why: a name made only of lowercase letters, digits and hyphens before `@`
/// with a positive canonical major, the rule the refusal states, is refused for a rule the
/// refusal must then state (a leading letter, at most 100 bytes).
#[test]
fn a_refusal_states_a_rule_the_refused_name_breaks() {
    let leading_digit = refusal("2fa-check@1", &declaring("2fa.check", 1));
    assert!(
        ["start", "begin", "first"]
            .iter()
            .any(|word| leading_digit.contains(word)),
        "2fa-check@1 is lowercase letters, digits and hyphens before @, and the refusal names no \
         other rule: {leading_digit}"
    );
    let long = format!("{}@1", "a".repeat(101));
    let too_long = refusal(&long, SYSTEM_QUERY_YAML);
    assert!(
        too_long.contains("100"),
        "a 101-byte stem of lowercase letters is refused, and the refusal names no length: \
         {too_long}"
    );
}

/// The suggestion comes from the id and revision the definition declares when the registration
/// given disagrees with it (the unit brief's rule), not from the registration given.
#[test]
fn the_suggestion_comes_from_the_declared_id_when_the_registration_given_disagrees() {
    let declared_other_id = refusal("other.thing@1", &declaring("self.improvement", 1));
    assert_eq!(
        suggested(&declared_other_id).as_deref(),
        Some("self-improvement@1"),
        "{declared_other_id}"
    );
    let yaml = declaring("self.improvement", 2);
    let declared_other_revision = refusal("self.improvement@1", &yaml);
    assert_eq!(
        suggested(&declared_other_revision).as_deref(),
        Some("self-improvement@2"),
        "{declared_other_revision}"
    );
    ProtocolCatalog::default()
        .add_yaml("self-improvement@2", &yaml, memory())
        .expect("the named registration registers");
}

/// Every suggestion is the documented mapping and registers; ids the mapping cannot turn into a
/// registration get no suggestion.
#[test]
fn every_suggestion_is_the_documented_mapping_and_registers() {
    let id_100 = "a.".repeat(50);
    let mapped_100 = format!("{}@1", "a-".repeat(50));
    let id_101 = format!("{id_100}b");
    let cases: Vec<(&str, Option<&str>)> = vec![
        ("self.improvement", Some("self-improvement@1")),
        ("a.b.c.d", Some("a-b-c-d@1")),
        ("eval.gated_change", Some("eval-gated-change@1")),
        ("self-improvement.v2", Some("self-improvement-v2@1")),
        ("v2.proto", Some("v2-proto@1")),
        ("a..b", Some("a--b@1")),
        ("foo.", Some("foo-@1")),
        ("foo_", Some("foo-@1")),
        (&id_100, Some(&mapped_100)),
        (".foo", None),
        ("_foo", None),
        ("2fa.check", None),
        ("Self.Improvement", None),
        ("selbst.verbesserung.\u{e4}", None),
        (&id_101, None),
    ];
    for (id, expected) in cases {
        let yaml = declaring(id, 1);
        let refused = refusal(&format!("{id}@1"), &yaml);
        assert_eq!(suggested(&refused).as_deref(), expected, "{id}: {refused}");
        if let Some(name) = expected {
            ProtocolCatalog::default()
                .add_yaml(name, &yaml, memory())
                .unwrap_or_else(|e| panic!("{id}: suggested {name} does not register: {e}"));
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    for (name, expected) in [
        ("self.improvement@0", None),
        ("self.improvement@01", None),
        ("self.improvement", None),
        ("self.improvement/1", Some("self-improvement@1")),
        ("self.improvement@1@1", None),
    ] {
        let refused = store.remove(name).expect_err(name);
        assert_eq!(
            suggested(&refused).as_deref(),
            expected,
            "{name}: {refused}"
        );
    }
}

/// A refusal escapes control and bidirectional-format characters from the name it echoes.
#[test]
fn a_refusal_echoes_no_control_or_format_character() {
    let name = "\u{1b}[2J\u{202e}x.y\u{2066}\u{200b}\u{feff}\u{2028}@1\n\u{7f}";
    let refused = refusal(name, SYSTEM_QUERY_YAML);
    let tmp = tempfile::tempdir().unwrap();
    let removed = InstallStore::new(tmp.path().join("installed"))
        .remove(name)
        .expect_err("an invalid name is refused");
    for text in [&refused, &removed] {
        assert!(
            !text.chars().any(|c| c.is_control()
                || matches!(
                    c,
                    '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                        | '\u{200b}'
                        | '\u{feff}'
                        | '\u{2028}'
                        | '\u{2029}'
                )),
            "{text:?}"
        );
    }
}

/// Nothing that registered before is refused now.
#[test]
fn every_name_the_old_rule_admits_still_registers() {
    let stem_100 = format!("{}@1", "a".repeat(100));
    for name in [
        "a@1",
        "a-@1",
        "a--b@1",
        "z9@1",
        "x1-2-3@1",
        stem_100.as_str(),
    ] {
        ProtocolCatalog::default()
            .add_yaml(name, SYSTEM_QUERY_YAML, memory())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("clock.yaml");
    fs::write(&source, declaring("custom.clock", 1)).unwrap();
    let store = InstallStore::new(tmp.path().join("installed"));
    store
        .install_file("custom-clock@1", &source, false)
        .unwrap();
    store.remove("custom-clock@1").unwrap();
}
