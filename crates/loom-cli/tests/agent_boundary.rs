//! `b10x-loom agent boundary` and `b10x-loom agent boundary-ready`
//! (`story:boundary-loopback-relays`): the command line the operator renders into a pod's
//! `boundary` sidecar. Each relay group is all or none and at least one is given; an upstream
//! that is not plain `http://` is refused before anything is bound, naming the flag and never
//! the value; the process listens on `127.0.0.1` at its declared ports only and SIGTERM ends it.
//!
//! The upstream is a fake on `127.0.0.1:0` in this test and the token a fixture under
//! `CARGO_TARGET_TMPDIR`; nothing reaches beyond loopback.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MODEL_PORT: u16 = 8401;

fn loom() -> Command {
    Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
}

fn boundary(arguments: &[&str]) -> Output {
    loom()
        .args(["agent", "boundary"])
        .args(arguments)
        .output()
        .expect("b10x-loom runs")
}

fn ready(ports: &[u16]) -> Output {
    let mut command = loom();
    command.args(["agent", "boundary-ready"]);
    for port in ports {
        command.arg("--port").arg(port.to_string());
    }
    command.output().expect("b10x-loom runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("agent_boundary")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean scratch");
    }
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

#[test]
fn the_boundary_refuses_an_https_upstream_naming_the_flag() {
    for flags in [
        ["--model-upstream", "--model-token", "", ""],
        [
            "--connectors-upstream",
            "--connectors-token",
            "--operation",
            "pipeline.get",
        ],
        [
            "--git-upstream",
            "--git-token",
            "--repository",
            "acme/widgets",
        ],
    ] {
        let [upstream, token, member, value] = flags;
        let mut arguments = vec![upstream, "https://gateway.example:8443", token, "token"];
        if !member.is_empty() {
            arguments.extend([member, value]);
        }
        let output = boundary(&arguments);
        let said = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{upstream}: {said}");
        assert!(said.contains(upstream), "{upstream}: {said}");
        assert!(said.contains("https"), "{upstream}: {said}");
        assert!(!said.contains("gateway.example"), "{upstream}: {said}");
    }
}

#[test]
fn the_boundary_refuses_an_upstream_with_user_information_without_printing_it() {
    for url in [
        "http://agent:fixture-userinfo-value@gateway.example",
        "http://gateway.example/?key=fixture-userinfo-value",
        "http://gateway.example/#fixture-userinfo-value",
    ] {
        let output = boundary(&[
            "--model-upstream",
            "http://gateway.example",
            "--model-token",
            "token",
            "--git-upstream",
            url,
            "--git-token",
            "token",
            "--repository",
            "acme/widgets",
        ]);
        let said = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{url}: {said}");
        assert!(said.contains("--git-upstream"), "{url}: {said}");
        assert!(!said.contains("fixture-userinfo-value"), "{url}: {said}");
    }
}

#[test]
fn each_relay_group_is_all_or_none_and_one_is_required() {
    for (arguments, missing) in [
        (vec![], "--model-upstream"),
        (
            vec!["--model-upstream", "http://gateway.example"],
            "--model-token",
        ),
        (vec!["--model-token", "token"], "--model-upstream"),
        (
            vec![
                "--connectors-upstream",
                "http://connectors.example",
                "--connectors-token",
                "token",
            ],
            "--operation",
        ),
        (
            vec![
                "--connectors-upstream",
                "http://connectors.example",
                "--operation",
                "pipeline.get",
            ],
            "--connectors-token",
        ),
        (vec!["--operation", "pipeline.get"], "--connectors-upstream"),
        (
            vec![
                "--git-upstream",
                "http://connectors.example/git",
                "--git-token",
                "token",
            ],
            "--repository",
        ),
        (vec!["--repository", "acme/widgets"], "--git-upstream"),
        (
            vec![
                "--model-upstream",
                "http://gateway.example",
                "--model-token",
                "token",
                "--repository",
                "acme/widgets",
            ],
            "--git-upstream",
        ),
    ] {
        let output = boundary(&arguments);
        let said = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}: {said}");
        assert!(
            said.contains(missing),
            "{arguments:?} names {missing}: {said}"
        );
    }
}

#[test]
fn a_declared_repository_or_operation_out_of_shape_is_refused_naming_the_flag() {
    for (flag, value) in [
        ("--repository", "../widgets"),
        ("--repository", "acme/widgets.git"),
        ("--repository", "acme"),
    ] {
        let output = boundary(&[
            "--git-upstream",
            "http://connectors.example/git",
            "--git-token",
            "token",
            flag,
            value,
        ]);
        let said = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{value}: {said}");
        assert!(said.contains(flag), "{value}: {said}");
    }
    let output = boundary(&[
        "--connectors-upstream",
        "http://connectors.example",
        "--connectors-token",
        "token",
        "--operation",
        "merge request",
    ]);
    let said = stderr(&output);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(said.contains("--operation"), "{said}");
}

#[test]
fn boundary_ready_exits_0_when_every_port_accepts_and_1_otherwise() {
    let open = TcpListener::bind("127.0.0.1:0").expect("a listener");
    let open_port = open.local_addr().expect("address").port();
    let closed = TcpListener::bind("127.0.0.1:0").expect("a port");
    let closed_port = closed.local_addr().expect("address").port();
    drop(closed);

    let output = ready(&[open_port]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let output = ready(&[open_port, closed_port]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(&closed_port.to_string()),
        "{}",
        stderr(&output)
    );
    let output = ready(&[]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

/// The listening TCP sockets process `pid` holds, from `/proc/net/tcp{,6}` and its descriptors.
fn listening(pid: u32) -> BTreeSet<String> {
    let mut inodes = BTreeSet::new();
    for entry in fs::read_dir(format!("/proc/{pid}/fd")).expect("the child's descriptors") {
        let Ok(target) = fs::read_link(entry.expect("descriptor").path()) else {
            continue;
        };
        let target = target.to_string_lossy().into_owned();
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            inodes.insert(inode.to_owned());
        }
    }
    let mut sockets = BTreeSet::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = fs::read_to_string(table) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // local_address is fields[1], st fields[3] (0A = LISTEN), inode fields[9].
            if fields.len() > 9 && fields[3] == "0A" && inodes.contains(fields[9]) {
                let (address, port) = fields[1].split_once(':').expect("address:port");
                let port = u16::from_str_radix(port, 16).expect("hex port");
                let address = if address.len() == 8 {
                    let raw = u32::from_str_radix(address, 16).expect("hex address");
                    std::net::Ipv4Addr::from(raw.swap_bytes()).to_string()
                } else {
                    format!("v6:{address}")
                };
                sockets.insert(format!("{address}:{port}"));
            }
        }
    }
    sockets
}

#[test]
fn the_boundary_relays_on_its_declared_port_only_and_ends_on_sigterm() {
    let model = SocketAddr::from(([127, 0, 0, 1], MODEL_PORT));
    assert!(
        TcpStream::connect_timeout(&model, Duration::from_millis(200)).is_err(),
        "127.0.0.1:{MODEL_PORT} is already in use by another process; this test needs it free"
    );
    let dir = scratch("sigterm");
    let token = dir.join("token");
    fs::write(&token, "fixture-cli-token\n").expect("fixture token");

    // A fake upstream that answers one request and hands its head to the test.
    let upstream = TcpListener::bind("127.0.0.1:0").expect("upstream");
    let upstream_addr = upstream.local_addr().expect("address");
    let (heads, head) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let (mut stream, _) = upstream.accept().expect("accept");
        let mut received = Vec::new();
        let mut buffer = [0u8; 4096];
        while !received.windows(4).any(|w| w == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            received.extend_from_slice(&buffer[..read]);
        }
        let _ = heads.send(String::from_utf8_lossy(&received).into_owned());
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok");
    });

    let mut child = loom()
        .args(["agent", "boundary", "--model-upstream"])
        .arg(format!("http://{upstream_addr}"))
        .arg("--model-token")
        .arg(&token)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the boundary starts");
    let pid = child.id();

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("child state") {
            let mut said = String::new();
            let _ = child
                .stderr
                .take()
                .expect("stderr")
                .read_to_string(&mut said);
            panic!("the boundary exited before it was ready ({status}): {said}");
        }
        if ready(&[MODEL_PORT]).status.code() == Some(0) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the boundary was not ready in time"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        listening(pid),
        BTreeSet::from([format!("127.0.0.1:{MODEL_PORT}")]),
        "the boundary listens on its declared port on loopback and nothing else"
    );

    let mut agent = TcpStream::connect(model).expect("connect to the boundary");
    agent
        .write_all(
            b"GET /v1/models HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer agent-held-value\r\nConnection: close\r\n\r\n",
        )
        .expect("request");
    agent
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut answer = String::new();
    let _ = agent.read_to_string(&mut answer);
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.ends_with("ok"), "{answer}");
    assert!(!answer.contains("fixture-cli-token"), "{answer}");
    let head = head
        .recv_timeout(Duration::from_secs(10))
        .expect("the upstream received the request")
        .to_ascii_lowercase();
    assert!(head.starts_with("get /v1/models http/1.1\r\n"), "{head}");
    assert!(
        head.contains("authorization: bearer fixture-cli-token\r\n"),
        "{head}"
    );
    assert!(!head.contains("agent-held-value"), "{head}");

    let killed = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill runs");
    assert!(killed.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child state") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "SIGTERM did not end the boundary"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut said = String::new();
    let _ = child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut said);
    assert_eq!(status.code(), Some(0), "{said}");
    assert!(said.contains("model GET /v1/models 200"), "{said}");
    assert!(!said.contains("fixture-cli-token"), "{said}");
    assert!(
        TcpStream::connect_timeout(&model, Duration::from_millis(200)).is_err(),
        "the port is released"
    );
}
