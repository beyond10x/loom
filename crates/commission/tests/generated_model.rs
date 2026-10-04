//! The responsibility model reaches `b10x-commission` from the generated crate.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    CommissionState, PrincipalId,
};
use std::path::Path;

/// The header `ess/SKIPPED.md` opens with; each later line names one skipped scenario.
const SKIPPED_HEADER: &str = "# Skipped conformance scenarios\n\
\n\
Each line below names one conformance scenario this repository skips, and why:\n\
`- <scenario id>: <reason>`. The list is empty when nothing is skipped.\n";

#[test]
fn generated_model_reexport() {
    let data = CommissionData {
        commission_id: CommissionId(Uuid("6f1c2a52-0d7e-4a4b-9a57-3c1f1d2b8e01".into())),
        agent_revision_id: AgentRevisionId(Uuid("0b6c4f1e-5d9a-4c2e-8f3b-7a1d2e3f4a5b".into())),
        case_id: CaseId("case-1".into()),
        principal: PrincipalId("principal-1".into()),
        authority_context: AuthorityContext(Value::Object(vec![(
            "scope".into(),
            Value::Text("read".into()),
        )])),
    };

    let commission = Commission::new(data.clone());

    assert_eq!(commission.state(), CommissionState::Assigned);
    assert_eq!(commission.data(), &data);
    assert_eq!(commission.data().case_id, CaseId("case-1".into()));
    assert_eq!(
        commission.data().principal,
        PrincipalId("principal-1".into())
    );
}

/// The scenario ids of the suite synthesized from `ess/` now.
fn synthesized_scenarios(root: &Path) -> Vec<String> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("skipped-md-suite-{}.json", std::process::id()));
    let output = std::process::Command::new("ess")
        .current_dir(root)
        .args(["verify", "conform", "synthesize", "--path", "ess", "--out"])
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("`ess` must be on PATH: {error}"));
    assert!(
        output.status.success(),
        "`ess verify conform synthesize --path ess` exited {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let suite = std::fs::read_to_string(&out)
        .unwrap_or_else(|error| panic!("read {}: {error}", out.display()));
    let _ = std::fs::remove_file(&out);
    let suite = b10x_commission::model::json::parse(&suite)
        .unwrap_or_else(|error| panic!("the suite is not JSON: {error}"));
    match suite.member("scenarios") {
        Some(Value::Object(scenarios)) => scenarios.iter().map(|(id, _)| id.clone()).collect(),
        _ => panic!("the suite has no `scenarios` object"),
    }
}

#[test]
fn every_skipped_entry_names_a_synthesized_scenario_and_gives_a_reason() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = Path::new(&manifest).join("../..");
    let path = root.join("ess/SKIPPED.md");
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} must exist: {error}", path.display()));

    assert!(
        body.starts_with(SKIPPED_HEADER),
        "{} does not open with the header:\n{body}",
        path.display()
    );
    let scenarios = synthesized_scenarios(&root);
    assert!(
        !scenarios.is_empty(),
        "the suite synthesized from ess/ holds no scenario"
    );
    for line in body[SKIPPED_HEADER.len()..]
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let (id, reason) = line
            .strip_prefix("- ")
            .and_then(|entry| entry.split_once(": "))
            .unwrap_or_else(|| panic!("`{line}` is not `- <scenario id>: <reason>`"));
        assert!(
            !reason.trim().is_empty(),
            "the skip of `{id}` gives no reason"
        );
        assert!(
            scenarios.iter().any(|scenario| scenario == id.trim()),
            "the skip of `{id}` names no scenario of the suite synthesized from ess/"
        );
    }
}
