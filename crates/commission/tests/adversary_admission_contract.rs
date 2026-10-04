//! Adversary pass 1 (wave 2026-10-04-w4, unit commission/frontier-admission): the rewritten
//! `docs/contracts/frontier.md` says it "follows that specification" and gives the frontier's
//! shape. This case reads both files when it runs and checks that the shape's keys are exactly the
//! fields the specification declares for `Frontier` (its identity included) and for the item
//! types it holds.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repository() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run through cargo");
    PathBuf::from(manifest).join("../..")
}

fn read(relative: &str) -> String {
    let path = repository().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The identity and field names of one declaration in a domain file, read line by line.
fn declared_fields(domain: &str, name: &str) -> BTreeSet<String> {
    let header = format!("  - name: {name}");
    let mut lines = domain.lines().skip_while(|line| *line != header);
    assert!(lines.next().is_some(), "{name} is not declared");
    let mut section = "";
    let mut names = BTreeSet::new();
    for line in lines {
        if line.starts_with("  - ") || (!line.is_empty() && !line.starts_with(' ')) {
            break;
        }
        if line.len() > 4 && line.starts_with("    ") && !line[4..].starts_with(' ') {
            section = line.trim();
            continue;
        }
        let field = match section {
            "identity:" => line.strip_prefix("      name: "),
            "fields:" => line.strip_prefix("      - name: "),
            _ => None,
        };
        if let Some(field) = field {
            names.insert(field.trim().to_owned());
        }
    }
    assert!(!names.is_empty(), "{name} declares no fields");
    names
}

/// The keys the contract's `Shape:` block uses.
fn shape_keys(contract: &str) -> BTreeSet<String> {
    let after = contract
        .split_once("Shape:")
        .expect("the contract has a Shape: section")
        .1;
    let block = after
        .split_once("```yaml")
        .expect("the Shape: section opens a yaml block")
        .1
        .split_once("```")
        .expect("the yaml block closes")
        .0;
    block
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("- ").unwrap_or(line);
            let (key, _) = line.split_once(':')?;
            key.chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
                .then(|| key.to_owned())
        })
        .collect()
}

#[test]
fn frontier_contract_shape_is_the_specified_fields() {
    let domain = read("ess/domains/responsibility.yaml");
    let mut specified = BTreeSet::new();
    for name in [
        "commission.responsibility.Frontier",
        "commission.responsibility.FrontierClaim",
        "commission.responsibility.FrontierObligation",
        "commission.responsibility.FrontierAction",
    ] {
        specified.extend(declared_fields(&domain, name));
    }
    let shown = shape_keys(&read("docs/contracts/frontier.md"));

    let missing: Vec<_> = specified.difference(&shown).collect();
    let extra: Vec<_> = shown.difference(&specified).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "docs/contracts/frontier.md's shape disagrees with ess/domains/responsibility.yaml: \
         specified but not shown {missing:?}; shown but not specified {extra:?}"
    );
}
