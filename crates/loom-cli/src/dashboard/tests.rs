use super::*;
use serde_json::{Value, json};
use std::net::TcpStream;

/// Return the observed status, headers and body. Retry connection establishment briefly to
/// allow the second registered host to acquire the listener after the first host exits.
fn get(addr: SocketAddr, path: &str, headers: &str) -> (String, String) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stream = loop {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
            Ok(s) => break s,
            Err(error) if Instant::now() >= deadline => panic!("dashboard connection: {error}"),
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\n{headers}\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.into(), body.into())
}

#[test]
fn state_scoped_history_survives_listener_takeover_and_viewers_only_read_their_agents() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("dashboard");
    let state_a = dir.path().join("a");
    let state_b = dir.path().join("b");
    fs::create_dir(&state_a).unwrap();
    fs::create_dir(&state_b).unwrap();
    let spec =
        loom_sdk::agent::parse(include_str!("../../../../examples/agents/operator.yaml")).unwrap();
    let first = Dashboard::in_scope(
        root.clone(),
        None,
        &state_a,
        &spec,
        "example-revision",
        None,
    )
    .unwrap();
    let address = fs::read_to_string(root.join("address"))
        .unwrap()
        .parse::<SocketAddr>()
        .unwrap();
    let second = Dashboard::in_scope(
        root.clone(),
        None,
        &state_b,
        &spec,
        "example-revision",
        None,
    )
    .unwrap();
    let token = fs::read_to_string(root.join("viewer-token")).unwrap();
    let auth = format!("Authorization: Bearer {token}\r\n");
    assert!(
        get(address, "/?agent=a&run=b", "")
            .0
            .starts_with("HTTP/1.1 200")
    );
    let (contract_status, contract) = get(address, "/api/openapi.json", "");
    assert!(contract_status.starts_with("HTTP/1.1 200"));
    assert!(
        serde_json::from_str::<Value>(&contract).unwrap()["paths"]["/api/snapshot"].is_object()
    );
    let (denied, _) = get(address, "/api/snapshot", "");
    assert!(denied.starts_with("HTTP/1.1 401"));
    assert!(denied.contains("WWW-Authenticate: Bearer"));
    let snapshot = |path: &str, who: &str| {
        let (headers, text) = get(address, path, who);
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers} {text}");
        serde_json::from_str::<Value>(&text).unwrap()
    };
    let active = first.observer();
    active.started("a2a:test-one");
    let during = snapshot("/api/snapshot", &auth);
    assert_eq!(during["agents"].as_array().unwrap().len(), 2);
    assert_eq!(during["stats"]["active"], 1);
    assert!(!during.to_string().contains("test-one"));
    assert_eq!(during["agents"][0]["protocol"], "operator-reply@1");
    assert_eq!(during["agents"][0]["a2a_protocol"], "inbound-answer@1");
    assert_eq!(during["agents"][0]["poll_interval_seconds"], 60);
    assert_eq!(during["agents"][0]["spec_revision"], "example-revision");
    let agent_a = during["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["status"] == "working")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let agent_b = during["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["status"] == "idle")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let line=b10x_loom_plugin::decode_record_line(r#"{"item":"a2a:test-one","outcome":"proposed","reads":[],"proposal":"secret result stays in the source record"}"#).unwrap();
    active.finished("a2a:test-one", Some(&line), false);
    drop(active);
    let after = snapshot("/api/snapshot", &auth);
    assert_eq!(after["stats"]["completed"], 1);
    assert!(!after.to_string().contains("secret result"));
    let unchanged = get(
        address,
        "/api/snapshot",
        &format!(
            "{auth}If-None-Match: \"{}\"\r\n",
            after["cursor"].as_str().unwrap()
        ),
    );
    assert!(unchanged.0.starts_with("HTTP/1.1 304"));
    assert!(unchanged.1.is_empty());
    let restricted = "scoped-test-viewer";
    save(
        &root.join("viewers.json"),
        &json!({(digest(restricted.as_bytes())):[agent_b]}),
    )
    .unwrap();
    let credentials = format!("Authorization: Bearer {restricted}\r\n");
    let scoped = snapshot("/api/snapshot", &credentials);
    assert_eq!(scoped["agents"].as_array().unwrap().len(), 1);
    assert_eq!(scoped["stats"]["total"], 0);
    assert!(
        get(
            address,
            &format!("/api/snapshot?agent={agent_a}"),
            &credentials
        )
        .0
        .starts_with("HTTP/1.1 404")
    );
    drop(first);
    // The starting host is gone. Its peer takes over without moving the URL; the prior
    // owner's last-known status and completed turn remain in the same server inventory.
    let resumed = snapshot("/api/snapshot", &auth);
    assert_eq!(resumed["stats"]["completed"], 1);
    assert_eq!(
        snapshot(&format!("/api/snapshot?agent={agent_a}"), &auth)["agents"][0]["status"],
        "offline"
    );
    drop(second);
}

#[test]
fn legacy_record_replay_is_redacted_and_has_no_invented_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("dashboard");
    let state = dir.path().join("legacy");
    fs::create_dir(&state).unwrap();
    let line = r#"{"item":"a2a:sk-sensitive","outcome":"stopped","reads":[],"detail":"private content","steps":[{"action":"private_action_name","outcome":"refused","reason":"private tool output","steps_left":1,"model_calls_left":2}]}"#;
    fs::write(state.join("record.jsonl"), format!("{line}\n{line}\n")).unwrap();
    let text = include_str!("../../../../examples/agents/operator.yaml");
    let spec = loom_sdk::agent::parse(text).unwrap();
    let revision = digest(text.as_bytes());
    for _ in 0..2 {
        let host = Dashboard::in_scope(root.clone(), None, &state, &spec, &revision, None).unwrap();
        let address = fs::read_to_string(root.join("address"))
            .unwrap()
            .parse()
            .unwrap();
        let auth = format!(
            "Authorization: Bearer {}\r\n",
            fs::read_to_string(root.join("viewer-token")).unwrap()
        );
        let (_, body) = get(address, "/api/snapshot", &auth);
        let snap: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(snap["runs"].as_array().unwrap().len(), 1);
        assert_eq!(snap["agents"][0]["spec_revision"], revision);
        assert_eq!(snap["stats"]["refused_steps"], 1);
        assert!(snap["runs"][0]["ended_at"].is_null());
        assert!(snap["runs"][0]["started_at"].is_null());
        assert_eq!(snap["partial"], true);
        assert_eq!(snap["stats"]["latency_samples"], 0);
        assert!(
            !body.contains("private") && !body.contains("sk-sensitive"),
            "{body}"
        );
        let (_, body) = get(
            address,
            "/api/snapshot?from=2020-01-01T00:00:00Z&to=2020-01-02T00:00:00Z",
            &auth,
        );
        let filtered: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(filtered["stats"]["total"], 0);
        assert_eq!(filtered["stats"]["throughput_per_hour"], 0.0);
        assert_eq!(filtered["partial"], true);
        drop(host);
    }
}
