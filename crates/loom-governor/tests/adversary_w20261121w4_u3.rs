//! Adversary, wave 2026-11-21-w4 unit U3 (story:engineering-registry-calls-replaced).
//!
//! A governor built without a catalog falls back to the vendored engineering catalog. The set of
//! registrations it answers there must be the set the pinned registry answered at the base commit
//! (`63668a97`, `registry::list()` of engineering-protocols `0.16.0`), written out here as a literal
//! rather than read from the catalog under test. `with_protocol` refuses a name the fallback holds
//! and admits one it does not, so it observes the fallback through the public API.

use std::collections::BTreeMap;

use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore, OpenError};
use loom_protocols::ENGINEERING_FILES;

/// `registry::list()` at the base commit, as `name@major`, in its order (name, then major).
const BASE_BUILTINS: [&str; 10] = [
    "eval-gated-change@1",
    "incident-response@1",
    "loop-improvement@1",
    "ml-experiment@1",
    "protocol-adopt@1",
    "self-improvement@1",
    "software-change@1",
    "story-delivery@1",
    "story-delivery@2",
    "support-triage@1",
];

/// Names the registry did not hold under exactly `name@major` at the base commit.
const NOT_BUILTIN: [&str; 12] = [
    "no-such@1",
    "software-change@2",
    "story-delivery@3",
    "software.change@1",
    "Software-change@1",
    "software-change@01",
    "software-change@+1",
    "eval-gated-change@0",
    "support-triage@1@1",
    "system-query@1",
    "inbound-answer@1",
    "operator-reply@1",
];

fn software_change_yaml() -> &'static str {
    let (_, bytes) = ENGINEERING_FILES
        .iter()
        .find(|(path, _)| *path == "software-change/1.yaml")
        .expect("software-change/1.yaml is vendored");
    std::str::from_utf8(bytes).expect("UTF-8")
}

fn governor() -> CanonGovernor<MemoryCaseStore> {
    CanonGovernor::new(MemoryCaseStore::default())
}

#[test]
fn adversary_a_catalogless_governor_answers_exactly_the_base_builtins() {
    let yaml = software_change_yaml();
    for name in BASE_BUILTINS {
        match governor().with_protocol_yaml(name, yaml) {
            Err(OpenError::InvalidProtocol { protocol, problem }) => {
                assert_eq!(protocol, name);
                assert_eq!(
                    problem, "protocol name is empty or already registered",
                    "{name} is a built-in, so it cannot be registered over"
                );
            }
            Err(other) => {
                panic!("{name}: a built-in is refused as already registered, got {other:?}")
            }
            Ok(_) => panic!("{name}: the fallback lost a built-in the base registry held"),
        }
    }
    for name in NOT_BUILTIN {
        if let Err(error) = governor().with_protocol_yaml(name, yaml) {
            panic!("{name}: the base registry did not hold it, so it registers, got {error:?}");
        }
    }
}

/// Every base built-in is found by `open`: an empty revision map is refused for what the
/// protocol declares, never as an unknown protocol; a name outside the set is unknown.
#[test]
fn adversary_open_finds_every_base_builtin_and_nothing_else() {
    for name in BASE_BUILTINS {
        match governor().open(name, BTreeMap::new(), CaseInputs::none()) {
            Err(OpenError::UnknownProtocol { protocol }) => {
                panic!("{protocol}: the fallback lost a built-in the base registry held")
            }
            Err(_) => {}
            Ok(case) => panic!("{name}: opened {case:?} with no revisions"),
        }
    }
    for name in NOT_BUILTIN {
        assert_eq!(
            governor().open(name, BTreeMap::new(), CaseInputs::none()),
            Err(OpenError::UnknownProtocol {
                protocol: name.to_owned()
            }),
            "{name}"
        );
    }
}
