//! Exercise the operator launch API against the native host owned by the parent test.
//! Its already-running receipt reuses that process, including on request replay.
use super::{Scratch, a2a_request};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

fn private_json(path: &Path, body: Value) {
    let tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
    serde_json::to_writer(tmp.as_file(), &body).unwrap();
    tmp.persist(path).unwrap();
}

pub(super) fn check(scratch: &Scratch, catalog: &Path, port: u16, operator: &str, agent: &Value) {
    let root = scratch.root.join("dashboard");
    let list = "/api/launch-templates";
    let launch = "/api/launch";
    private_json(
        &root.join("launch-templates.json"),
        json!({"templates":[{
            "id":"review", "spec":scratch.spec(), "state":scratch.state(), "catalog":catalog,
            "model":"lab-agent", "classifier_model":"lab-classifier"
        }]}),
    );
    let viewer = "scoped-launch-test";
    let digest: String = Sha256::digest(viewer.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    private_json(&root.join("viewers.json"), json!({digest:[agent["id"]]}));
    let call = |method: &str, path: &str, auth: Option<&str>, body: Option<&Value>| {
        let (status, raw) = a2a_request(port, method, path, auth, body).unwrap();
        (status, serde_json::from_str::<Value>(&raw).unwrap())
    };
    let request = json!({"template":"review","request_id":"launch-1","harness":"native"});
    for (auth, expected) in [(None, 401), (Some(viewer), 403)] {
        assert_eq!(call("GET", list, auth, None).0, expected);
        assert_eq!(call("POST", launch, auth, Some(&request)).0, expected);
    }
    let operator = Some(operator);
    assert_eq!(
        call("GET", list, operator, None),
        (
            200,
            json!({"templates":[{"id":"review","name":agent["name"],"spec_id":agent["spec_id"],
            "kind":"agent_spec","default_harness":"native","harnesses":["native"],"protocol":agent["protocol"]}]})
        )
    );
    let unsupported = json!({"template":"review","request_id":"unsupported","harness":"codex"});
    assert_eq!(
        call("POST", launch, operator, Some(&unsupported)),
        (400, json!({"error":"invalid_request"}))
    );
    let expected = (
        200,
        json!({"agent_id":agent["id"],"instance":agent["instance"],"kind":"agent_spec","harness":"native","status":"already_running"}),
    );
    assert_eq!(call("POST", launch, operator, Some(&request)), expected);
    assert_eq!(call("POST", launch, operator, Some(&request)), expected);
    let conflict = json!({"template":"another","request_id":"launch-1"});
    assert_eq!(
        call("POST", launch, operator, Some(&conflict)),
        (409, json!({"error":"request_conflict"}))
    );
    let (status, snapshot) = call("GET", "/api/snapshot", operator, None);
    assert_eq!(status, 200);
    assert_eq!(snapshot["agents"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["agents"][0]["instance"], agent["instance"]);
}
