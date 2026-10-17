//! Adversary cases for `loom_sdk::models`: configuration fields, routes paths, non-regular and
//! oversized files, secrets that are missing, literal or unreachable, and the stability page.
//!
//! A case that needs its own process environment (no `HOME`, a variable set, another working
//! directory, a clean peak-memory figure) runs an ignored `child_*` test of this binary in a child
//! process; a child does nothing unless its parent set `LOOM_SDK_ADVERSARY_CHILD`.

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use llm_core::{Cancel, ErrorCode, Item, Model, TurnRequest, VecSink};
use loom_sdk::models::{ModelRole, from_config, from_routes, parse_config};

const CHILD: &str = "LOOM_SDK_ADVERSARY_CHILD";
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Runs the ignored test `name` of this binary in a child process and asserts it passed.
fn child(name: &str, configure: impl FnOnce(&mut Command)) {
    let mut command = Command::new(std::env::current_exe().expect("the test binary"));
    command
        .args([
            "--exact",
            name,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1");
    configure(&mut command);
    let output = command.output().expect("the child runs");
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "child `{name}` failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

fn refusal<T>(result: Result<T, loom_sdk::models::ModelsError>) -> String {
    match result {
        Ok(_) => panic!("expected a refusal"),
        Err(error) => error.to_string(),
    }
}

fn serving(model: &dyn Model) -> String {
    model.provenance().model.as_str().to_owned()
}

/// The first turn of `model` on `name`, as its error.
fn first_turn(model: &dyn Model, name: &str) -> llm_core::Error {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let request = TurnRequest::new(name, vec![Item::user("hello")]);
    let mut sink = VecSink::new(16, 1 << 16);
    runtime
        .block_on(model.turn(&request, &mut sink, &Cancel::new()))
        .expect_err("the first turn is refused")
}

const ANON: &str = "format: llm.routes/1
endpoints:
  lab:
    base_url: http://127.0.0.1:9/v1
    protocol: chat-completions
    auth: {kind: anonymous}
routes:
  sonnet: lab/claude-sonnet
";

fn dir_with(routes: &str, config: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    fs::write(dir.path().join("routes.yaml"), routes).expect("routes written");
    fs::write(dir.path().join("models.yaml"), config).expect("config written");
    dir
}

const AGENT: &str = "routes: routes.yaml\nroles:\n  - {role: agent, model: sonnet}\n";

// ---------------------------------------------------------------------------------------------
// Configuration fields

#[test]
fn a_duplicate_key_or_unknown_entry_field_is_refused_naming_file_and_field() {
    let dir = dir_with(ANON, AGENT);
    let config = dir.path().join("models.yaml");
    let load = |text: &str| {
        fs::write(&config, text).expect("config written");
        refusal(from_config(&config))
    };

    let message = load("routes: routes.yaml\nroutes: other.yaml\nroles: []\n");
    assert!(
        message.contains("models.yaml") && message.contains("routes"),
        "{message}"
    );

    let message =
        load("routes: routes.yaml\nroles:\n  - {role: agent, role: classifier, model: sonnet}\n");
    assert!(
        message.contains("models.yaml") && message.contains("role"),
        "{message}"
    );

    let message =
        load("routes: routes.yaml\nroles:\n  - {role: agent, model: sonnet, weight: 2}\n");
    assert!(
        message.contains("models.yaml") && message.contains("weight"),
        "{message}"
    );

    let message = load("routes: routes.yaml\nroles:\n  - {role: Agent, model: sonnet}\n");
    assert!(
        message.contains("models.yaml") && message.contains("Agent"),
        "{message}"
    );

    let message = load(
        "routes: routes.yaml\nroles:\n  - {role: classifier, model: sonnet}\n  - {role: agent, model: sonnet}\n  - {role: classifier, model: sonnet}\n",
    );
    assert!(
        message.contains("models.yaml")
            && message.contains("classifier")
            && message.contains("twice"),
        "{message}"
    );

    // Non-UTF-8 and non-mapping configurations.
    fs::write(&config, b"routes: \xff\xfe\n").expect("config written");
    assert!(refusal(from_config(&config)).contains("models.yaml"));
    let message = load("- routes\n");
    assert!(message.contains("models.yaml"), "{message}");
}

/// Expanding YAML aliases into owned values before any field is checked costs far more than the
/// byte bound: llm's routes parser walks aliases against a budget for exactly this reason. A
/// configuration of under 100 KiB must not cost 128 MiB.
#[test]
fn an_alias_bomb_in_a_configuration_is_refused_before_it_expands() {
    child("child_alias_bomb", |_| {});
}

#[test]
#[ignore = "run by its parent in a child process"]
fn child_alias_bomb() {
    if !in_child() {
        return;
    }
    fn peak_kib() -> u64 {
        let status = fs::read_to_string("/proc/self/status").expect("proc status");
        status
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))
            .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
            .expect("VmHWM")
    }
    let anchor = "x".repeat(64 * 1024);
    let aliases = vec!["*a"; 2048].join(", ");
    let text = format!("routes: routes.yaml\nroles: []\npad: &a \"{anchor}\"\nmore: [{aliases}]\n");
    assert!(text.len() < 100 * 1024);
    let before = peak_kib();
    let result = parse_config(&text);
    let grown = peak_kib().saturating_sub(before);
    assert!(result.is_err(), "the unknown fields are refused");
    assert!(
        grown < 16 * 1024,
        "parsing a {} byte configuration raised peak memory by {grown} KiB",
        text.len()
    );
}

// ---------------------------------------------------------------------------------------------
// Routes paths

#[test]
fn an_absolute_routes_path_is_used_as_it_is() {
    let routes_dir = tempfile::tempdir().expect("a temporary directory");
    let routes = routes_dir.path().join("elsewhere.yaml");
    fs::write(&routes, ANON).expect("routes written");
    let dir = dir_with(ANON, "");
    let config = dir.path().join("models.yaml");
    fs::write(
        &config,
        format!(
            "routes: {}\nroles:\n  - {{role: agent, model: sonnet}}\n",
            routes.display()
        ),
    )
    .expect("config written");
    let models = from_config(&config).expect("an absolute routes path loads");
    assert_eq!(
        serving(models.get(ModelRole::Agent).expect("agent").as_ref()),
        "sonnet"
    );
}

#[test]
fn a_bare_configuration_name_resolves_routes_in_the_working_directory() {
    let dir = dir_with(ANON, AGENT);
    let path = dir.path().to_path_buf();
    child("child_bare_configuration_name", |command| {
        command.current_dir(&path);
    });
}

#[test]
#[ignore = "run by its parent in a child process"]
fn child_bare_configuration_name() {
    if !in_child() {
        return;
    }
    let models = from_config(Path::new("models.yaml")).expect("a bare name loads");
    assert!(models.get(ModelRole::Agent).is_some());
}

/// The acceptance asks that a missing or invalid routes document be refused naming the file:
/// through `from_config`, the file a host wrote and can fix is the configuration that named it.
#[test]
fn a_missing_routes_document_names_the_configuration_that_names_it() {
    let dir = dir_with(
        ANON,
        "routes: absent.yaml\nroles:\n  - {role: agent, model: sonnet}\n",
    );
    let message = refusal(from_config(&dir.path().join("models.yaml")));
    assert!(
        message.contains("absent.yaml") && message.contains("models.yaml"),
        "{message}"
    );
}

// ---------------------------------------------------------------------------------------------
// Non-regular and oversized files

fn mkfifo(path: &Path) {
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).expect("a C path");
    // SAFETY: `name` is a valid NUL-terminated path for the duration of the call.
    let status = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    assert_eq!(status, 0, "mkfifo {}", path.display());
}

#[test]
fn non_regular_files_are_refused_without_blocking() {
    let dir = dir_with(ANON, AGENT);
    let fifo = dir.path().join("pipe.yaml");
    mkfifo(&fifo);
    let link = dir.path().join("link.yaml");
    std::os::unix::fs::symlink(&fifo, &link).expect("symlink");
    let subdir = dir.path().join("sub.yaml");
    fs::create_dir(&subdir).expect("dir");

    for path in [&fifo, &link, &subdir] {
        let message = refusal(from_config(path));
        assert!(message.contains(&path.display().to_string()), "{message}");
        let message = refusal(from_routes(path, "sonnet"));
        assert!(message.contains(&path.display().to_string()), "{message}");
    }
    let message = refusal(from_routes(Path::new("/dev/zero"), "sonnet"));
    assert!(message.contains("/dev/zero"), "{message}");

    // A configuration whose routes path names a pipe, a directory, a device.
    let config = dir.path().join("models.yaml");
    for routes in ["pipe.yaml", "link.yaml", "sub.yaml", "/dev/zero"] {
        fs::write(
            &config,
            format!("routes: {routes}\nroles:\n  - {{role: agent, model: sonnet}}\n"),
        )
        .expect("config written");
        let message = refusal(from_config(&config));
        assert!(message.contains(routes), "{message}");
    }

    // A symlink to a regular file loads; a dangling one and a loop are refused by name.
    let real = dir.path().join("models.yaml");
    let alias = dir.path().join("alias.yaml");
    std::os::unix::fs::symlink(&real, &alias).expect("symlink");
    fs::write(&real, AGENT).expect("config written");
    assert!(
        from_config(&alias)
            .expect("a link to a file loads")
            .get(ModelRole::Agent)
            .is_some()
    );
    let dangling = dir.path().join("dangling.yaml");
    std::os::unix::fs::symlink(dir.path().join("nothing"), &dangling).expect("symlink");
    assert!(refusal(from_config(&dangling)).contains("dangling.yaml"));
    let loop_a = dir.path().join("loop-a.yaml");
    let loop_b = dir.path().join("loop-b.yaml");
    std::os::unix::fs::symlink(&loop_b, &loop_a).expect("symlink");
    std::os::unix::fs::symlink(&loop_a, &loop_b).expect("symlink");
    assert!(refusal(from_config(&loop_a)).contains("loop-a.yaml"));
}

#[test]
fn files_over_the_byte_bound_are_refused_and_one_at_it_loads() {
    let dir = dir_with(ANON, AGENT);
    let config = dir.path().join("models.yaml");

    // Exactly at the bound: padding in a comment.
    let pad = MAX_BYTES - AGENT.len() - 2;
    let at_bound = format!("{AGENT}#{}\n", "x".repeat(pad));
    assert_eq!(at_bound.len(), MAX_BYTES);
    fs::write(&config, &at_bound).expect("config written");
    assert!(
        from_config(&config)
            .expect("a file at its bound loads")
            .get(ModelRole::Agent)
            .is_some()
    );
    assert!(parse_config(&at_bound).is_ok());

    let over = format!("{AGENT}#{}\n", "x".repeat(pad + 1));
    fs::write(&config, &over).expect("config written");
    let message = refusal(from_config(&config));
    assert!(
        message.contains("models.yaml") && message.contains("bound"),
        "{message}"
    );
    assert!(parse_config(&over).is_err());

    let routes = dir.path().join("routes.yaml");
    fs::write(&routes, format!("{ANON}#{}\n", "x".repeat(MAX_BYTES))).expect("routes written");
    fs::write(&config, AGENT).expect("config written");
    let message = refusal(from_config(&config));
    assert!(
        message.contains("routes.yaml") && message.contains("bound"),
        "{message}"
    );
}

// ---------------------------------------------------------------------------------------------
// Secrets

/// A routes document whose credentialed endpoints all point at `port`.
fn secret_routes(port: u16, absent_file: &Path, home_file: &str) -> String {
    format!(
        "format: llm.routes/1
endpoints:
  by-env:
    base_url: http://127.0.0.1:{port}/v1
    protocol: messages
    auth: {{kind: bearer, secret: env:LOOM_SDK_ADVERSARY_NEVER_SET}}
  by-file:
    base_url: http://127.0.0.1:{port}/v1
    protocol: chat-completions
    auth: {{kind: bearer, secret: 'file:{}'}}
  by-home:
    base_url: http://127.0.0.1:{port}/v1
    protocol: responses
    auth: {{kind: bearer, secret: 'file:~/{home_file}'}}
routes:
  env-model: by-env/m
  file-model: by-file/m
  home-model: by-home/m
",
        absent_file.display()
    )
}

#[test]
fn a_missing_secret_is_refused_unauthorized_before_any_connection() {
    assert!(std::env::var_os("LOOM_SDK_ADVERSARY_NEVER_SET").is_none());
    let listener = TcpListener::bind("127.0.0.1:0").expect("a listener");
    listener.set_nonblocking(true).expect("non-blocking");
    let port = listener.local_addr().expect("an address").port();
    let dir = tempfile::tempdir().expect("a temporary directory");
    let home_file = format!(".loom-sdk-adversary-absent-{}", std::process::id());
    let routes = dir.path().join("routes.yaml");
    fs::write(
        &routes,
        secret_routes(port, &dir.path().join("absent-secret"), &home_file),
    )
    .expect("routes written");

    for name in ["env-model", "file-model", "home-model"] {
        let model = from_routes(&routes, name).expect("the route builds");
        let error = first_turn(model.as_ref(), name);
        assert_eq!(error.code, ErrorCode::Unauthorized, "{name}: {error:?}");
        match listener.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            other => panic!("{name}: a connection was opened before the secret: {other:?}"),
        }
    }
    // The listener does accept: the absence above is not a closed port.
    let _client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(listener.accept().is_ok());
}

#[test]
fn a_refusal_never_carries_a_literal_secret() {
    let literal = "sk-ant-adversary-LITERAL-0001";
    let routes = format!(
        "format: llm.routes/1
endpoints:
  leaky:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {{kind: bearer, secret: {literal}}}
routes:
  sonnet: leaky/m
"
    );
    let dir = dir_with(&routes, AGENT);
    let message = refusal(from_routes(&dir.path().join("routes.yaml"), "sonnet"));
    assert!(!message.contains(literal), "{message}");
    assert!(
        message.contains("routes.yaml") && message.contains("leaky"),
        "{message}"
    );
    let message = refusal(from_config(&dir.path().join("models.yaml")));
    assert!(!message.contains(literal), "{message}");
    assert!(message.contains("routes.yaml"), "{message}");
}

#[test]
fn a_turn_refusal_never_carries_a_file_secret() {
    let value = "sk-ant-adversary-FILE-0002";
    let dir = tempfile::tempdir().expect("a temporary directory");
    let secret = dir.path().join("secret");
    fs::write(&secret, value).expect("secret written");
    // World-readable: llm refuses the source; the refusal must not carry its content.
    fs::set_permissions(&secret, std::os::unix::fs::PermissionsExt::from_mode(0o644))
        .expect("chmod");
    let routes = dir.path().join("routes.yaml");
    fs::write(
        &routes,
        format!(
            "format: llm.routes/1
endpoints:
  f:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {{kind: bearer, secret: 'file:{}'}}
routes:
  m: f/m
",
            secret.display()
        ),
    )
    .expect("routes written");
    let model = from_routes(&routes, "m").expect("the route builds");
    let error = first_turn(model.as_ref(), "m");
    let shown = format!("{error:?} {}", error.message);
    assert!(!shown.contains(value), "{shown}");
}

#[test]
fn a_turn_refusal_never_carries_an_environment_secret() {
    child("child_environment_secret", |command| {
        command.env(
            "LOOM_SDK_ADVERSARY_SECRET",
            "sk-ant-adversary-ENV-0003\r\nX-Injected: 1",
        );
    });
}

#[test]
#[ignore = "run by its parent in a child process"]
fn child_environment_secret() {
    if !in_child() {
        return;
    }
    let dir = tempfile::tempdir().expect("a temporary directory");
    let routes = dir.path().join("routes.yaml");
    fs::write(
        &routes,
        "format: llm.routes/1
endpoints:
  e:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {kind: bearer, secret: env:LOOM_SDK_ADVERSARY_SECRET}
routes:
  m: e/m
",
    )
    .expect("routes written");
    let model = from_routes(&routes, "m").expect("the route builds");
    let error = first_turn(model.as_ref(), "m");
    let shown = format!("{error:?} {}", error.message);
    assert!(!shown.contains("sk-ant-adversary-ENV-0003"), "{shown}");
}

/// Without `HOME`, a `file:~/` endpoint cannot resolve; the refusal names the document and the
/// endpoint. A role served by another endpoint of the same document needs no home and loads.
#[test]
#[ignore = "needs a per-endpoint resolver call in llm; until then every endpoint of the routes document must resolve (https://github.com/beyond10x/llm/issues/36)"]
fn an_unset_home_refuses_only_the_endpoint_that_needs_it() {
    let dir = dir_with(
        "format: llm.routes/1
endpoints:
  lab:
    base_url: http://127.0.0.1:9/v1
    protocol: chat-completions
    auth: {kind: anonymous}
  homevault:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {kind: bearer, secret: 'file:~/.config/llm/key'}
routes:
  sonnet: lab/claude-sonnet
  vaulted: homevault/m
",
        AGENT,
    );
    let path = dir.path().to_path_buf();
    child("child_unset_home", |command| {
        command
            .env_remove("HOME")
            .env("LOOM_SDK_ADVERSARY_DIR", &path);
    });
}

#[test]
#[ignore = "run by its parent in a child process"]
fn child_unset_home() {
    if !in_child() {
        return;
    }
    assert!(std::env::var_os("HOME").is_none());
    let dir = PathBuf::from(std::env::var_os("LOOM_SDK_ADVERSARY_DIR").expect("the dir"));
    let message = refusal(from_routes(&dir.join("routes.yaml"), "vaulted"));
    assert!(
        message.contains("routes.yaml") && message.contains("homevault"),
        "{message}"
    );
    let models = from_config(&dir.join("models.yaml"))
        .expect("the agent role's endpoint needs no home: the configuration loads");
    assert!(models.get(ModelRole::Agent).is_some());
}

/// The same for a `keychain:` endpoint, which this build refuses: a routes document shared with
/// llm's own tools may hold one, and a role that does not use it still loads.
#[test]
#[ignore = "needs a per-endpoint resolver call in llm; until then every endpoint of the routes document must resolve (https://github.com/beyond10x/llm/issues/36)"]
fn a_keychain_endpoint_no_role_uses_does_not_refuse_the_configuration() {
    let dir = dir_with(
        "format: llm.routes/1
endpoints:
  lab:
    base_url: http://127.0.0.1:9/v1
    protocol: chat-completions
    auth: {kind: anonymous}
  kc:
    base_url: http://127.0.0.1:9/v1
    protocol: messages
    auth: {kind: bearer, secret: keychain:llm/anthropic}
routes:
  sonnet: lab/claude-sonnet
  locked: kc/m
",
        AGENT,
    );
    let message = refusal(from_routes(&dir.path().join("routes.yaml"), "locked"));
    assert!(message.contains("kc"), "{message}");
    let models = from_config(&dir.path().join("models.yaml"))
        .expect("the agent role's endpoint needs no keychain: the configuration loads");
    assert!(models.get(ModelRole::Agent).is_some());
}

// ---------------------------------------------------------------------------------------------
// The stability page against the module

/// Every public item of `src/models.rs` is on the stability page as `loom_sdk::models::<item>`.
#[test]
fn every_public_models_item_is_on_the_stability_page() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join("src/models.rs")).expect("models.rs");
    let page = fs::read_to_string(root.join("../../website/docs/reference/sdk-stability.md"))
        .expect("the page");
    let mut items = Vec::new();
    for line in source.lines() {
        let name = if let Some(rest) = line.strip_prefix("pub fn ") {
            rest.split(['(', '<']).next()
        } else if let Some(rest) = line.strip_prefix("pub struct ") {
            rest.split([' ', '{', '<', ';']).next()
        } else if let Some(rest) = line.strip_prefix("pub enum ") {
            rest.split([' ', '{', '<']).next()
        } else if let Some(rest) = line.strip_prefix("pub use ") {
            rest.trim_end_matches(';').rsplit("::").next()
        } else {
            None
        };
        if let Some(name) = name {
            items.push(name.trim().to_owned());
        }
    }
    assert!(items.len() >= 8, "{items:?}");
    for item in items {
        let path = format!("`loom_sdk::models::{item}`");
        assert!(page.contains(&path), "the stability page lists {path}");
    }
}
