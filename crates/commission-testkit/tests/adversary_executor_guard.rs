//! Adversary pass 1 on `story:agent-executor-port`: the dependency guard, driven for real.
//!
//! The guard is `executor_port_contract` in `tests/executor_port.rs`, the test `task deps-guard`
//! runs. Its matcher and its `cargo tree` call are private to that file, so these cases run its
//! compiled test binary (the sibling of this one) with `CARGO_MANIFEST_DIR` pointing into a
//! fixture workspace. The guard derives its root from that variable at run time, so it runs its
//! own `cargo tree` against the fixture: a fixture whose `b10x-commission` depends on a refused
//! crate must make the guard fail.
//!
//! Every fixture crate is a local path crate: nothing is fetched.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn deny_list() -> Vec<String> {
    let path = root().join("model-provider-deny.txt");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The newest `executor_port-<hash>` test binary beside this one, refused when it is older than
/// `executor_port.rs` (a stale binary would test another tree's guard).
fn guard_binary() -> PathBuf {
    let me = std::env::current_exe().unwrap_or_else(|error| panic!("current_exe: {error}"));
    let deps = me
        .parent()
        .unwrap_or_else(|| panic!("{} has no parent", me.display()));
    let newest = std::fs::read_dir(deps)
        .unwrap_or_else(|error| panic!("read {}: {error}", deps.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_none()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("executor_port-"))
        })
        .filter_map(|path| {
            let modified = path.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max()
        .unwrap_or_else(|| {
            panic!(
                "no executor_port test binary in {}: build it with \
                 `cargo test -p b10x-commission-testkit --no-run`",
                deps.display()
            )
        });
    let source = root().join("crates/commission-testkit/tests/executor_port.rs");
    let source_modified = source
        .metadata()
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| panic!("stat {}: {error}", source.display()));
    assert!(
        newest.0 >= source_modified,
        "{} is older than {}: rebuild it",
        newest.1.display(),
        source.display()
    );
    let listed = Command::new(&newest.1)
        .arg("--list")
        .output()
        .unwrap_or_else(|error| panic!("run {} --list: {error}", newest.1.display()));
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains("executor_port_contract: test"),
        "{} does not list executor_port_contract",
        newest.1.display()
    );
    newest.1
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// A fixture workspace laid out like this repository: `crates/commission` is `b10x-commission`
/// with `manifest_tail` appended to its manifest, each name in `stubs` is an empty path crate
/// under `stubs/`, and the repository's real deny list sits at the root.
fn fixture(case: &str, manifest_tail: &str, stubs: &[(&str, &str)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-executor-guard")
        .join(case);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .unwrap_or_else(|error| panic!("clear {}: {error}", dir.display()));
    }
    let mut members = vec!["\"crates/commission\"".to_owned()];
    for (name, tail) in stubs {
        members.push(format!("\"stubs/{name}\""));
        write(
            &dir.join("stubs").join(name).join("Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{tail}"
            ),
        );
        write(&dir.join("stubs").join(name).join("src/lib.rs"), "");
    }
    write(
        &dir.join("Cargo.toml"),
        &format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    );
    write(
        &dir.join("crates/commission/Cargo.toml"),
        &format!(
            "[package]\nname = \"b10x-commission\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             {manifest_tail}"
        ),
    );
    write(&dir.join("crates/commission/src/lib.rs"), "");
    // The guard's root is `CARGO_MANIFEST_DIR/../..`; the directory must exist to resolve.
    std::fs::create_dir_all(dir.join("crates/commission-testkit"))
        .unwrap_or_else(|error| panic!("create testkit dir: {error}"));
    std::fs::copy(
        root().join("model-provider-deny.txt"),
        dir.join("model-provider-deny.txt"),
    )
    .unwrap_or_else(|error| panic!("copy deny list: {error}"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let lock = Command::new(&cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(&dir)
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap_or_else(|error| panic!("run cargo generate-lockfile: {error}"));
    assert!(
        lock.status.success(),
        "fixture {case} does not resolve:\n{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    dir
}

/// `cargo tree -p b10x-commission -e normal --prefix none` in `dir` with `extra` flags: the
/// fixture's own proof that the dependency it plants is real.
fn tree(dir: &Path, extra: &[&str]) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(dir)
        .args([
            "tree",
            "--offline",
            "-p",
            "b10x-commission",
            "-e",
            "normal",
            "--prefix",
            "none",
        ])
        .args(extra)
        .output()
        .unwrap_or_else(|error| panic!("run cargo tree: {error}"));
    assert!(
        out.status.success(),
        "cargo tree failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn names(listing: &str, name: &str) -> bool {
    listing
        .lines()
        .any(|line| line.split_whitespace().next() == Some(name))
}

/// Runs the real guard against `dir`: `(passed, its output)`.
fn run_guard(dir: &Path) -> (bool, String) {
    let out = Command::new(guard_binary())
        .args(["executor_port_contract", "--exact", "--test-threads=1"])
        .env("CARGO_MANIFEST_DIR", dir.join("crates/commission-testkit"))
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap_or_else(|error| panic!("run the guard: {error}"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

const PROVIDER: &str = "async-openai";

fn provider_is_denied() {
    assert!(
        deny_list().iter().any(|name| name == PROVIDER),
        "{PROVIDER} is no longer on model-provider-deny.txt; pick another denied crate"
    );
}

/// Asserts the guard fails on `dir` because it names `crate_name` as refused.
fn assert_refused(dir: &Path, crate_name: &str, how: &str) {
    let (passed, output) = run_guard(dir);
    assert!(
        !passed,
        "the dependency guard passed a b10x-commission that depends on {crate_name} {how}:\n{output}"
    );
    assert!(
        output.contains("depends on refused crates") && output.contains(crate_name),
        "the guard failed, but not by refusing {crate_name}:\n{output}"
    );
}

/// Control: a plain normal dependency on a denied crate is refused. This is the case that shows
/// the harness can see a refusal at all; it also fails when the guard's `cargo tree` call stops
/// listing dependencies (`--depth 0`, `-e build`), which `executor_port_contract` alone misses.
#[test]
fn guard_refuses_a_plain_provider_dependency() {
    provider_is_denied();
    let dir = fixture(
        "plain",
        &format!("[dependencies]\n{PROVIDER} = {{ path = \"../../stubs/{PROVIDER}\" }}\n"),
        &[(PROVIDER, "")],
    );
    assert_refused(&dir, PROVIDER, "as a plain dependency");
}

/// A dependency renamed in the manifest is still listed under its package name.
#[test]
fn guard_refuses_a_renamed_provider_dependency() {
    provider_is_denied();
    let dir = fixture(
        "renamed",
        &format!(
            "[dependencies]\nllm-client = {{ package = \"{PROVIDER}\", path = \"../../stubs/{PROVIDER}\" }}\n"
        ),
        &[(PROVIDER, "")],
    );
    assert_refused(&dir, PROVIDER, "renamed to llm-client");
}

/// A denied crate two hops away is refused.
#[test]
fn guard_refuses_a_transitive_provider_dependency() {
    provider_is_denied();
    let dir = fixture(
        "transitive",
        "[dependencies]\nglue = { path = \"../../stubs/glue\" }\n",
        &[
            (PROVIDER, ""),
            (
                "glue",
                &format!("\n[dependencies]\n{PROVIDER} = {{ path = \"../{PROVIDER}\" }}\n"),
            ),
        ],
    );
    assert_refused(&dir, PROVIDER, "through glue");
}

/// A denied crate behind a non-default Cargo feature. `cargo build -p b10x-commission --features
/// openai` (or any workspace member that enables the feature) links it; the guard's
/// `cargo tree` resolves default features only and never sees it.
#[test]
fn guard_refuses_a_provider_behind_a_feature() {
    provider_is_denied();
    let dir = fixture(
        "feature",
        &format!(
            "[features]\nopenai = [\"dep:{PROVIDER}\"]\n\n[dependencies]\n\
             {PROVIDER} = {{ path = \"../../stubs/{PROVIDER}\", optional = true }}\n"
        ),
        &[(PROVIDER, "")],
    );
    assert!(
        names(&tree(&dir, &["--all-features"]), PROVIDER),
        "fixture: b10x-commission with all features does not depend on {PROVIDER}"
    );
    assert_refused(&dir, PROVIDER, "behind the non-default feature `openai`");
}

/// A refused crate under a target that is not the host's. `cargo build --target
/// wasm32-unknown-unknown -p b10x-commission` links it; the guard's `cargo tree` lists the host
/// target only.
#[test]
fn guard_refuses_loom_for_another_target() {
    let dir = fixture(
        "target",
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\n\
         b10x-loom = { path = \"../../stubs/b10x-loom\" }\n",
        &[("b10x-loom", "")],
    );
    assert!(
        names(&tree(&dir, &["--target", "all"]), "b10x-loom"),
        "fixture: b10x-commission for all targets does not depend on b10x-loom"
    );
    assert_refused(&dir, "b10x-loom", "for target_arch = \"wasm32\"");
}

/// The deny list names at least one crate for each major hosted or local model provider. The
/// families are the ones the wave brief names: OpenAI, Anthropic, Google, Cohere, Mistral, AWS
/// Bedrock, Ollama, llama.cpp.
#[test]
fn deny_list_covers_each_major_provider_family() {
    let deny = deny_list();
    let families: [(&str, &[&str]); 8] = [
        ("OpenAI", &["openai"]),
        ("Anthropic", &["anthropic", "clust"]),
        ("Google", &["gemini", "google-generative", "google-genai"]),
        ("Cohere", &["cohere"]),
        ("Mistral", &["mistral"]),
        ("AWS Bedrock", &["bedrock"]),
        ("Ollama", &["ollama"]),
        ("llama.cpp", &["llama-cpp", "llama_cpp"]),
    ];
    let missing: Vec<&str> = families
        .iter()
        .filter(|(_, needles)| {
            !deny
                .iter()
                .any(|name| needles.iter().any(|needle| name.contains(needle)))
        })
        .map(|(family, _)| *family)
        .collect();
    assert!(
        missing.is_empty(),
        "model-provider-deny.txt names no crate for {missing:?}; it names {deny:?}"
    );
}
