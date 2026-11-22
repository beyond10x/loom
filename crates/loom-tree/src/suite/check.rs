//! [`evaluate_checks`]: the thirteen check kinds over what a selection gives, and the first
//! difference, which names the check, its kind and the expected value. What was found is printed
//! only when it is a number, a boolean or a value Loom declares; any other text is printed as its
//! length and the first 12 hex digits of its SHA-256, so a program's output never reaches a
//! summary line ([`printable_found`]).

use std::collections::{BTreeMap, BTreeSet};

use loom::tree::{
    CheckDifference, CheckKind, CompareOp, Condition, Conditions, DifferenceReason, Operand,
    OperandTake, SuiteCheck, SuiteCycle, ValueType,
};
use serde_json::Value;

use super::availability::{KindUse, SUITE_AVAILABILITY};
use super::decode::compile_regex;
use super::path::{parse_value_path, values_at};
use super::select::{CycleView, SelectFailure, Selected, select_values};
use super::value::{as_written, number, read, same, sha256_hex};

/// The operands taken right after a step, by the place of their check and their side
/// (`steps[2].checks[0].left`, `expect.checks[1].right`).
pub type Captured = BTreeMap<String, Vec<Selected>>;

/// The run log event kinds, tree stops and the outcomes, evidence kinds, actions and evidence
/// results the protocols of `view` declare: the found values a difference prints as they are.
fn declared(view: &CycleView<'_>) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = SUITE_AVAILABILITY
        .iter()
        .filter(|entry| entry.used == KindUse::Event)
        .map(|entry| entry.name.to_owned())
        .collect();
    names.extend(
        [
            "Terminal",
            "AwaitingPerson",
            "AwaitingAuthority",
            "Stuck",
            "BudgetSpent",
        ]
        .map(str::to_owned),
    );
    for entry in view.catalog.iter() {
        names.insert(entry.name().to_owned());
        let Ok(document) = serde_yaml_ng::from_str::<Value>(&entry.definition.yaml) else {
            continue;
        };
        for section in ["outcomes", "evidence_kinds", "actions", "artifacts"] {
            if let Some(members) = document[section].as_object() {
                names.extend(members.keys().cloned());
            }
        }
        collect_results(&document["claims"], &mut names);
    }
    names
}

/// Every `result` text below `value`.
fn collect_results(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(members) => {
            for (name, member) in members {
                match (name.as_str(), member) {
                    ("result", Value::String(result)) => {
                        names.insert(result.clone());
                    }
                    _ => collect_results(member, names),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_results(item, names)),
        _ => {}
    }
}

/// `value` as a difference prints what was found: a number, a boolean, null or a text in
/// `declared` as itself, a list of those as JSON, and anything else as its length in bytes and the
/// first 12 hex digits of the SHA-256 of its text.
pub fn printable_found(value: &Value, declared: &BTreeSet<String>) -> String {
    fn plain(value: &Value, declared: &BTreeSet<String>) -> bool {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => true,
            Value::String(text) => declared.contains(text),
            Value::Array(items) => items.iter().all(|item| plain(item, declared)),
            Value::Object(_) => false,
        }
    }
    if plain(value, declared) {
        return as_written(value);
    }
    let text = as_written(value);
    format!(
        "{} bytes, sha256 {}",
        text.len(),
        &sha256_hex(text.as_bytes())[..12]
    )
}

/// Whether `item` holds every condition of `conditions` and, when it has groups, every condition
/// of one of them.
pub(crate) fn holds(item: &Value, conditions: &Conditions) -> Result<bool, SelectFailure> {
    for condition in &conditions.all {
        if !condition_holds(item, condition)? {
            return Ok(false);
        }
    }
    if conditions.any_of.is_empty() {
        return Ok(true);
    }
    for group in &conditions.any_of {
        let mut all = true;
        for condition in &group.all {
            if !condition_holds(item, condition)? {
                all = false;
                break;
            }
        }
        if all {
            return Ok(true);
        }
    }
    Ok(false)
}

fn bad(detail: impl Into<String>) -> SelectFailure {
    SelectFailure {
        reason: DifferenceReason::StepFailed,
        detail: detail.into(),
    }
}

fn has(container: &Value, wanted: &Value) -> bool {
    match (container, wanted) {
        (Value::String(text), Value::String(part)) => text.contains(part.as_str()),
        (Value::Array(items), wanted) => items.iter().any(|item| same(item, wanted)),
        _ => false,
    }
}

fn matches(value: &Value, regex: &str) -> Result<bool, SelectFailure> {
    let regex = compile_regex(regex).map_err(bad)?;
    Ok(value.as_str().is_some_and(|text| regex.is_match(text)))
}

fn type_of(value: &Value) -> ValueType {
    match value {
        Value::Null => ValueType::Null,
        Value::Bool(_) => ValueType::Bool,
        Value::Number(_) => ValueType::Number,
        Value::String(_) => ValueType::String,
        Value::Array(_) => ValueType::List,
        Value::Object(_) => ValueType::Map,
    }
}

fn length_of(value: &Value) -> Option<i64> {
    match value {
        Value::String(text) => i64::try_from(text.chars().count()).ok(),
        Value::Array(items) => i64::try_from(items.len()).ok(),
        _ => None,
    }
}

/// Whether the values at the condition's path hold its test. A test that asks for something
/// (`equals`, `in`, `contains`, `matches`, `min`, `max`, `type`, `length`) needs at least one value
/// and holds when every value holds it; one that asks for an absence (`not_equals`, `not_in`,
/// `lacks`, `not_matches`) holds when no value breaks it; `exists` counts the values.
fn condition_holds(item: &Value, condition: &Condition) -> Result<bool, SelectFailure> {
    let path = parse_value_path(&condition.path).map_err(bad)?;
    let found = values_at(item, &path);
    if let Some(exists) = condition.exists {
        return Ok(found.is_empty() != exists);
    }
    let every =
        |test: &dyn Fn(&Value) -> Result<bool, SelectFailure>| -> Result<bool, SelectFailure> {
            if found.is_empty() {
                return Ok(false);
            }
            for value in &found {
                if !test(value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        };
    let no_value_breaks =
        |test: &dyn Fn(&Value) -> Result<bool, SelectFailure>| -> Result<bool, SelectFailure> {
            for value in &found {
                if !test(value)? {
                    return Ok(false);
                }
            }
            Ok(true)
        };
    if let Some(expected) = &condition.equals {
        let expected = read(expected);
        return every(&|value| Ok(same(value, &expected)));
    }
    if let Some(expected) = &condition.not_equals {
        let expected = read(expected);
        return no_value_breaks(&|value| Ok(!same(value, &expected)));
    }
    if let Some(list) = &condition.r#in {
        let list: Vec<Value> = list.iter().map(read).collect();
        return every(&|value| Ok(list.iter().any(|entry| same(value, entry))));
    }
    if let Some(list) = &condition.not_in {
        let list: Vec<Value> = list.iter().map(read).collect();
        return no_value_breaks(&|value| Ok(!list.iter().any(|entry| same(value, entry))));
    }
    if let Some(wanted) = &condition.contains {
        let wanted = read(wanted);
        return every(&|value| Ok(has(value, &wanted)));
    }
    if let Some(wanted) = &condition.lacks {
        let wanted = read(wanted);
        return no_value_breaks(&|value| Ok(!has(value, &wanted)));
    }
    if let Some(regex) = &condition.matches {
        return every(&|value| matches(value, regex));
    }
    if let Some(regex) = &condition.not_matches {
        return no_value_breaks(&|value| Ok(value.as_str().is_none() || !matches(value, regex)?));
    }
    if let Some(min) = &condition.min {
        let min = number(&read(min)).unwrap_or(f64::NAN);
        return every(&|value| Ok(number(value).is_some_and(|found| found >= min)));
    }
    if let Some(max) = &condition.max {
        let max = number(&read(max)).unwrap_or(f64::NAN);
        return every(&|value| Ok(number(value).is_some_and(|found| found <= max)));
    }
    if let Some(wanted) = condition.r#type {
        return every(&|value| Ok(type_of(value) == wanted));
    }
    if let Some(bound) = &condition.length {
        return every(&|value| {
            Ok(length_of(value).is_some_and(|length| {
                bound.min.is_none_or(|min| length >= min)
                    && bound.max.is_none_or(|max| length <= max)
            }))
        });
    }
    Ok(true)
}

fn kind_name(kind: CheckKind) -> &'static str {
    match kind {
        CheckKind::Exists => "exists",
        CheckKind::Count => "count",
        CheckKind::Equals => "equals",
        CheckKind::Contains => "contains",
        CheckKind::Lacks => "lacks",
        CheckKind::Matches => "matches",
        CheckKind::NotMatches => "not_matches",
        CheckKind::All => "all",
        CheckKind::None => "none",
        CheckKind::Some => "some",
        CheckKind::Sequence => "sequence",
        CheckKind::Before => "before",
        CheckKind::Compare => "compare",
    }
}

/// The values of `selected`.
fn plain(selected: &[Selected]) -> Vec<Value> {
    selected.iter().map(|item| item.value.clone()).collect()
}

/// One check being evaluated: its label, its kind and what is printed.
struct Judge<'a> {
    label: String,
    kind: CheckKind,
    declared: &'a BTreeSet<String>,
}

impl Judge<'_> {
    fn differs(
        &self,
        reason: DifferenceReason,
        expected: impl Into<String>,
        found: Option<String>,
    ) -> CheckDifference {
        CheckDifference {
            check: self.label.clone(),
            kind: kind_name(self.kind).to_owned(),
            reason,
            expected: expected.into(),
            found,
        }
    }

    fn failed(&self, expected: impl Into<String>, found: Option<String>) -> CheckDifference {
        self.differs(DifferenceReason::CheckFailed, expected, found)
    }

    fn found(&self, value: &Value) -> Option<String> {
        Some(printable_found(value, self.declared))
    }

    fn read_failure(&self, failure: SelectFailure) -> CheckDifference {
        self.differs(
            failure.reason,
            "a selection that reads",
            Some(failure.detail),
        )
    }
}

/// The value of one compare operand, or its difference.
fn operand_value(
    operand: &Operand,
    side: &str,
    key: &str,
    op: CompareOp,
    view: &CycleView<'_>,
    captured: &Captured,
    judge: &Judge<'_>,
) -> Result<Value, CheckDifference> {
    if let Some(value) = &operand.value {
        return Ok(read(value));
    }
    let items = match &operand.after {
        Some(_) => captured.get(key).cloned().unwrap_or_default(),
        None => select_values(operand.select.as_deref().unwrap_or_default(), view)
            .map_err(|failure| judge.read_failure(failure))?,
    };
    let one = |items: &[Selected]| -> Result<Selected, CheckDifference> {
        match items {
            [item] => Ok(item.clone()),
            _ => Err(judge.differs(
                DifferenceReason::OperandNotOne,
                format!("{side}: one value"),
                Some(items.len().to_string()),
            )),
        }
    };
    let bytes_of = |item: &Selected| -> Vec<u8> {
        match (&item.bytes, &item.value) {
            (Some(bytes), _) => bytes.as_ref().clone(),
            (None, Value::String(text)) => text.clone().into_bytes(),
            (None, other) => other.to_string().into_bytes(),
        }
    };
    let take = operand.take.unwrap_or(OperandTake::Value);
    let mut value = match take {
        OperandTake::Value if op == CompareOp::Subset && items.len() != 1 => {
            Value::Array(plain(&items))
        }
        OperandTake::Value => one(&items)?.value,
        OperandTake::Count => Value::from(items.len()),
        OperandTake::Size => Value::from(bytes_of(&one(&items)?).len()),
        OperandTake::Digest => Value::String(sha256_hex(&bytes_of(&one(&items)?))),
        OperandTake::Length => {
            let item = one(&items)?;
            match length_of(&item.value) {
                Some(length) => Value::from(length),
                None => {
                    return Err(judge.failed(
                        format!("{side}: a text or a list"),
                        judge.found(&item.value),
                    ));
                }
            }
        }
    };
    if let Some(plus) = operand.plus {
        value = match value.as_i64() {
            Some(number) => Value::from(number.saturating_add(plus)),
            None => {
                return Err(judge.failed(
                    format!("{side}: an integer to add {plus} to"),
                    judge.found(&value),
                ));
            }
        };
    }
    Ok(value)
}

fn compare(left: &Value, op: CompareOp, right: &Value) -> bool {
    let ordered = |test: fn(std::cmp::Ordering) -> bool| match (left, right) {
        (Value::Number(_), Value::Number(_)) => number(left)
            .zip(number(right))
            .and_then(|(a, b)| a.partial_cmp(&b))
            .is_some_and(test),
        (Value::String(a), Value::String(b)) => test(a.cmp(b)),
        _ => false,
    };
    match op {
        CompareOp::Eq => same(left, right),
        CompareOp::Ne => !same(left, right),
        CompareOp::Lt => ordered(|o| o.is_lt()),
        CompareOp::Le => ordered(|o| o.is_le()),
        CompareOp::Gt => ordered(|o| o.is_gt()),
        CompareOp::Ge => ordered(|o| o.is_ge()),
        CompareOp::Subset => {
            let elements = |value: &Value| match value {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            };
            let right = elements(right);
            elements(left)
                .iter()
                .all(|element| right.iter().any(|other| same(element, other)))
        }
    }
}

fn op_name(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "eq",
        CompareOp::Ne => "ne",
        CompareOp::Lt => "lt",
        CompareOp::Le => "le",
        CompareOp::Gt => "gt",
        CompareOp::Ge => "ge",
        CompareOp::Subset => "subset",
    }
}

fn written_list(list: &[loom::json::Value]) -> Vec<Value> {
    list.iter().map(read).collect()
}

/// Whether `pattern` is a subsequence of `values`.
fn within(values: &[Value], pattern: &[Value]) -> bool {
    let mut wanted = pattern.iter().peekable();
    for value in values {
        if wanted.peek().is_some_and(|next| same(value, next)) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

fn evaluate_one(
    check: &SuiteCheck,
    place: &str,
    view: &CycleView<'_>,
    captured: &Captured,
    judge: &Judge<'_>,
) -> Result<(), CheckDifference> {
    let selected = if check.check == CheckKind::Compare {
        Vec::new()
    } else {
        match select_values(&check.select, view) {
            Ok(selected) => selected,
            Err(failure) => return Err(judge.read_failure(failure)),
        }
    };
    let values = plain(&selected);
    let count = values.len();
    let found_count = Some(count.to_string());
    let only = |expected: String| -> Result<&Value, CheckDifference> {
        match values.as_slice() {
            [value] => Ok(value),
            _ => Err(judge.failed(
                format!("one value {expected}"),
                Some(format!("{count} values")),
            )),
        }
    };
    let expected_is = check.is.as_ref().map(read).unwrap_or(Value::Bool(true));
    match check.check {
        CheckKind::Exists => {
            let wanted = expected_is.as_bool().unwrap_or(true);
            if (count > 0) == wanted {
                Ok(())
            } else {
                Err(judge.failed(
                    if wanted {
                        "at least one value"
                    } else {
                        "no value"
                    },
                    found_count.clone(),
                ))
            }
        }
        CheckKind::Count => {
            let (min, max) = match expected_is.as_i64() {
                Some(is) if check.is.is_some() => (Some(is), Some(is)),
                _ => (check.min, check.max),
            };
            let length = i64::try_from(count).unwrap_or(i64::MAX);
            if min.is_none_or(|min| length >= min) && max.is_none_or(|max| length <= max) {
                Ok(())
            } else {
                let expected = match (min, max) {
                    (Some(min), Some(max)) if min == max => format!("{min}"),
                    (Some(min), Some(max)) => format!("{min} to {max}"),
                    (Some(min), None) => format!("at least {min}"),
                    (None, Some(max)) => format!("at most {max}"),
                    (None, None) => "any count".into(),
                };
                Err(judge.failed(expected, found_count.clone()))
            }
        }
        CheckKind::Equals => {
            let value = only(as_written(&expected_is))?;
            if same(value, &expected_is) {
                Ok(())
            } else {
                Err(judge.failed(as_written(&expected_is), judge.found(value)))
            }
        }
        CheckKind::Contains | CheckKind::Lacks => {
            let value = only(as_written(&expected_is))?;
            let holds = has(value, &expected_is) == (check.check == CheckKind::Contains);
            if holds {
                Ok(())
            } else {
                Err(judge.failed(as_written(&expected_is), judge.found(value)))
            }
        }
        CheckKind::Matches | CheckKind::NotMatches => {
            let regex = check.regex.clone().unwrap_or_default();
            let value = only(format!("matching {regex}"))?;
            if !value.is_string() {
                return Err(judge.failed(format!("a text matching {regex}"), judge.found(value)));
            }
            let matched = matches(value, &regex).map_err(|failure| judge.read_failure(failure))?;
            if matched == (check.check == CheckKind::Matches) {
                Ok(())
            } else {
                Err(judge.failed(regex, judge.found(value)))
            }
        }
        CheckKind::All | CheckKind::None | CheckKind::Some => {
            let conditions = check.r#where.clone().unwrap_or(Conditions {
                all: Vec::new(),
                any_of: Vec::new(),
            });
            let mut holding = 0usize;
            for value in &values {
                if holds(value, &conditions).map_err(|failure| judge.read_failure(failure))? {
                    holding += 1;
                }
            }
            let (good, expected) = match check.check {
                CheckKind::All => (
                    count > 0 && holding == count,
                    "every value holds where, at least one value",
                ),
                CheckKind::None => (holding == 0, "no value holds where"),
                _ => (holding > 0, "at least one value holds where"),
            };
            if good {
                Ok(())
            } else {
                Err(judge.failed(expected, Some(format!("{holding} of {count} hold"))))
            }
        }
        CheckKind::Sequence => {
            let (form, pattern) = if let Some(is) = &check.is {
                (
                    "is",
                    match read(is) {
                        Value::Array(items) => items,
                        other => vec![other],
                    },
                )
            } else if let Some(starts) = &check.starts {
                ("starts", written_list(starts))
            } else if let Some(ends) = &check.ends {
                ("ends", written_list(ends))
            } else {
                (
                    "within",
                    written_list(check.within.as_deref().unwrap_or_default()),
                )
            };
            let holds = match form {
                "is" => {
                    values.len() == pattern.len()
                        && values.iter().zip(&pattern).all(|(a, b)| same(a, b))
                }
                "starts" => {
                    values.len() >= pattern.len()
                        && values.iter().zip(&pattern).all(|(a, b)| same(a, b))
                }
                "ends" => {
                    values.len() >= pattern.len()
                        && values[values.len() - pattern.len()..]
                            .iter()
                            .zip(&pattern)
                            .all(|(a, b)| same(a, b))
                }
                _ => within(&values, &pattern),
            };
            if holds {
                Ok(())
            } else {
                Err(judge.failed(
                    format!("{form} {}", Value::Array(pattern)),
                    judge.found(&Value::Array(values.clone())),
                ))
            }
        }
        CheckKind::Before => {
            let empty = Conditions {
                all: Vec::new(),
                any_of: Vec::new(),
            };
            let test = |conditions: &Conditions, value: &Value| {
                holds(value, conditions).map_err(|failure| judge.read_failure(failure))
            };
            let first = check.first.as_ref().unwrap_or(&empty);
            let then = check.then.as_ref().unwrap_or(&empty);
            let mut last_first = None;
            for (index, value) in values.iter().enumerate() {
                if test(first, value)? {
                    last_first = Some(index);
                }
            }
            let Some(last_first) = last_first else {
                return Err(judge.failed("a value holding first", Some("none".into())));
            };
            let mut next_then = None;
            for (index, value) in values.iter().enumerate().skip(last_first + 1) {
                if test(then, value)? {
                    next_then = Some(index);
                    break;
                }
            }
            let Some(next_then) = next_then else {
                return Err(judge.failed(
                    "a value holding then after the last holding first",
                    Some("none".into()),
                ));
            };
            if let Some(between) = &check.between_none {
                for value in &values[last_first + 1..next_then] {
                    if test(between, value)? {
                        return Err(judge.failed(
                            "no value holding between_none between first and then",
                            judge.found(value),
                        ));
                    }
                }
            }
            Ok(())
        }
        CheckKind::Compare => {
            let op = check.op.unwrap_or(CompareOp::Eq);
            let operand = |side: &str, operand: &Option<Operand>| {
                let Some(operand) = operand else {
                    return Err(judge.failed(format!("{side}: an operand"), None));
                };
                operand_value(
                    operand,
                    side,
                    &format!("{place}.{side}"),
                    op,
                    view,
                    captured,
                    judge,
                )
            };
            let left = operand("left", &check.left)?;
            let right = operand("right", &check.right)?;
            if compare(&left, op, &right) {
                Ok(())
            } else {
                Err(judge.failed(
                    format!(
                        "{} {}",
                        op_name(op),
                        printable_found(&right, judge.declared)
                    ),
                    judge.found(&left),
                ))
            }
        }
    }
}

/// The first check of `checks` that does not hold over what `view` shows, as its difference;
/// `None` when all hold. `place` is where the checks stand (`expect`, `steps[2]`), which names a
/// check without a `name` (`expect.checks[0]`) and keys the operands `captured` holds.
pub fn evaluate_checks(
    checks: &[SuiteCheck],
    place: &str,
    view: &CycleView<'_>,
    captured: &Captured,
) -> Option<CheckDifference> {
    let declared = declared(view);
    for (index, check) in checks.iter().enumerate() {
        let location = format!("{place}.checks[{index}]");
        let judge = Judge {
            label: check.name.clone().unwrap_or_else(|| location.clone()),
            kind: check.check,
            declared: &declared,
        };
        if let Err(difference) = evaluate_one(check, &location, view, captured, &judge) {
            return Some(difference);
        }
    }
    None
}

/// Takes, as `view` stands right after the step named `step`, every compare operand of `cycle`
/// whose `after` names it, into `captured`.
///
/// # Errors
/// The difference a selection gives, naming its check.
pub fn capture_after(
    step: &str,
    cycle: &SuiteCycle,
    view: &CycleView<'_>,
    captured: &mut Captured,
) -> Result<(), CheckDifference> {
    let declared = declared(view);
    let places = cycle
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| (format!("steps[{index}]"), &step.checks))
        .chain(std::iter::once(("expect".to_owned(), &cycle.expect.checks)));
    for (place, checks) in places {
        for (index, check) in checks.iter().enumerate() {
            let location = format!("{place}.checks[{index}]");
            for (side, operand) in [("left", &check.left), ("right", &check.right)] {
                let Some(operand) = operand else { continue };
                if operand.after.as_deref() != Some(step) {
                    continue;
                }
                let items = select_values(operand.select.as_deref().unwrap_or_default(), view)
                    .map_err(|failure| {
                        Judge {
                            label: check.name.clone().unwrap_or_else(|| location.clone()),
                            kind: check.check,
                            declared: &declared,
                        }
                        .read_failure(failure)
                    })?;
                captured.insert(format!("{location}.{side}"), items);
            }
        }
    }
    Ok(())
}
