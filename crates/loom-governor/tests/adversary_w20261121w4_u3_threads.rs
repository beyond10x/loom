//! Adversary, wave 2026-11-21-w4 unit U3 (story:engineering-registry-calls-replaced).
//!
//! `builtin()` builds the engineering catalog once, in a `OnceLock`, on its first call. This binary
//! holds one test so that the first call in the process is made by many threads at once: every
//! one of them must see the same catalog (a built-in refused as already registered, an unknown
//! name admitted), none an empty one, and none may panic.

use std::sync::{Arc, Barrier};

use loom_governor::{CanonGovernor, MemoryCaseStore, OpenError};
use loom_protocols::ENGINEERING_FILES;

#[test]
fn adversary_the_first_builtin_lookup_from_many_threads_sees_one_catalog() {
    let (_, bytes) = ENGINEERING_FILES
        .iter()
        .find(|(path, _)| *path == "software-change/1.yaml")
        .expect("software-change/1.yaml is vendored");
    let yaml: &'static str = std::str::from_utf8(bytes).expect("UTF-8");
    let threads = 16;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|index| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let name = if index % 2 == 0 {
                    "software-change@1"
                } else {
                    "story-delivery@2"
                };
                let refused = CanonGovernor::new(MemoryCaseStore::default())
                    .with_protocol_yaml(name, yaml)
                    .map(|_| ());
                let admitted = CanonGovernor::new(MemoryCaseStore::default())
                    .with_protocol_yaml("no-such@1", yaml)
                    .map(|_| ());
                (name, refused, admitted)
            })
        })
        .collect();
    for handle in handles {
        let (name, refused, admitted) = handle.join().expect("no thread panicked");
        assert_eq!(
            refused,
            Err(OpenError::InvalidProtocol {
                protocol: name.to_owned(),
                problem: "protocol name is empty or already registered".to_owned(),
            }),
            "{name}"
        );
        assert_eq!(admitted, Ok(()), "no-such@1 is no built-in");
    }
}
