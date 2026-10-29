//! `b10x-loom-operator run`, started as a process, builds its cluster client over TLS and reaches
//! the API server instead of panicking. The binary's graph enables both rustls providers (`ring`
//! through kube and reqwest, `aws-lc-rs` through `b10x-substrate-host`), so rustls cannot pick a
//! process default and the binary installs one itself. Loom 0.50.1's image exited 101 at start
//! with "Could not automatically determine the process-level CryptoProvider".
//!
//! The API server is a local listener: the test passes once the process opens a connection to
//! it, which it does only after the TLS client was built. Nothing leaves the host.

use std::fs;
use std::io::Read as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.50.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
";

/// A directory under cargo's per-target test directory, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the test directory");
    dir
}

#[test]
fn run_builds_its_tls_client_and_connects_to_the_cluster() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    listener
        .set_nonblocking(true)
        .expect("a non-blocking listener");
    let port = listener.local_addr().expect("the local address").port();

    let dir = scratch("startup-tls-client");
    let kubeconfig = dir.join("kubeconfig");
    fs::write(
        &kubeconfig,
        format!(
            "apiVersion: v1\nkind: Config\nclusters:\n- name: local\n  cluster:\n    server: \
             https://127.0.0.1:{port}\ncontexts:\n- name: local\n  context: {{cluster: local, \
             user: local, namespace: agents}}\ncurrent-context: local\nusers:\n- name: local\n  \
             user: {{}}\n"
        ),
    )
    .expect("the kubeconfig");
    let config = dir.join("config.yaml");
    fs::write(&config, CONFIG).expect("the controller configuration");
    let stderr = fs::File::create(dir.join("stderr")).expect("the stderr file");

    let mut child = Command::new(env!("CARGO_BIN_EXE_b10x-loom-operator"))
        .arg("run")
        .arg("--config")
        .arg(&config)
        .env_clear()
        .env("KUBECONFIG", &kubeconfig)
        .env("HOME", &dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .expect("the binary starts");

    let deadline = Instant::now() + Duration::from_secs(60);
    let outcome = loop {
        if listener.accept().is_ok() {
            break Ok(());
        }
        if let Some(status) = child.try_wait().expect("the child's status") {
            break Err(format!("exited {status} before connecting"));
        }
        if Instant::now() > deadline {
            break Err("did not connect within 60 s".to_owned());
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let _ = child.wait();

    let mut text = String::new();
    fs::File::open(dir.join("stderr"))
        .and_then(|mut file| file.read_to_string(&mut text))
        .expect("the stderr file reads");
    assert!(!text.contains("CryptoProvider"), "{text}");
    if let Err(why) = outcome {
        panic!("b10x-loom-operator run {why}; stderr:\n{text}");
    }
}
