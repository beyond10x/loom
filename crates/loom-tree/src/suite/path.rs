//! The two kinds of path a suite writes: a path into a value (`findings[id=q2].status`), and a
//! relative path of a file below a root (the suite's directory, a cycle's workspace or store).
//!
//! A value path is segments joined by `.`: a member name, `name[<n>]` (a negative `<n>` counts
//! from the end), `name[*]` (every element), `name[<member>=<value>]` (the elements whose member
//! equals the scalar); a name or value holding `.`, `[` or `]` is quoted (`"a.b"`). `.` alone is
//! the item. A path that reaches nothing gives no value. Nothing else is a path.

use std::path::{Component, Path};

use serde_json::Value;

/// One step of a value path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PathStep {
    Member(String),
    Index(i64),
    Every,
    Filter(String, String),
}

/// A parsed value path; no step is the item itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValuePath(pub(crate) Vec<PathStep>);

/// Why a text is no value path.
pub(crate) fn parse_value_path(text: &str) -> Result<ValuePath, String> {
    if text == "." {
        return Ok(ValuePath(Vec::new()));
    }
    if text.is_empty() {
        return Err("is empty".into());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut steps = Vec::new();
    let mut at = 0;
    loop {
        // One segment: an optional name, then any brackets.
        let mut named = false;
        if at < chars.len() && chars[at] == '"' {
            let (name, next) = quoted(&chars, at)?;
            steps.push(PathStep::Member(name));
            at = next;
            named = true;
        } else {
            let start = at;
            while at < chars.len() && !matches!(chars[at], '.' | '[' | ']' | '"') {
                at += 1;
            }
            if at > start {
                steps.push(PathStep::Member(chars[start..at].iter().collect()));
                named = true;
            }
        }
        let mut bracketed = false;
        while at < chars.len() && chars[at] == '[' {
            let close = closing(&chars, at)?;
            steps.push(bracket(&chars[at + 1..close])?);
            at = close + 1;
            bracketed = true;
        }
        if !named && !bracketed {
            return Err(format!("has an empty segment at character {at}"));
        }
        if at == chars.len() {
            break;
        }
        match chars[at] {
            '.' if at + 1 < chars.len() => at += 1,
            '.' => return Err("ends with a `.`".into()),
            ']' => return Err(format!("has a `]` with no `[` at character {at}")),
            other => return Err(format!("has {other:?} at character {at}")),
        }
    }
    Ok(ValuePath(steps))
}

/// The quoted name starting at `chars[at]` (a `"`), and the index after its closing quote.
fn quoted(chars: &[char], at: usize) -> Result<(String, usize), String> {
    let close = chars[at + 1..]
        .iter()
        .position(|c| *c == '"')
        .map(|offset| at + 1 + offset)
        .ok_or_else(|| format!("has a quote at character {at} that is not closed"))?;
    if close == at + 1 {
        return Err(format!("has an empty quoted name at character {at}"));
    }
    Ok((chars[at + 1..close].iter().collect(), close + 1))
}

/// The index of the `]` closing the `[` at `chars[at]`, quotes inside it skipped.
fn closing(chars: &[char], at: usize) -> Result<usize, String> {
    let mut inside_quote = false;
    for (index, c) in chars.iter().enumerate().skip(at + 1) {
        match c {
            '"' => inside_quote = !inside_quote,
            ']' if !inside_quote => return Ok(index),
            '[' if !inside_quote => break,
            _ => {}
        }
    }
    Err(format!("has a `[` at character {at} that is not closed"))
}

/// The step a bracket's content stands for.
fn bracket(content: &[char]) -> Result<PathStep, String> {
    let text: String = content.iter().collect();
    if text == "*" {
        return Ok(PathStep::Every);
    }
    if let Ok(index) = text.parse::<i64>() {
        return Ok(PathStep::Index(index));
    }
    let Some(equals) = content.iter().position(|c| *c == '=') else {
        return Err(format!(
            "has the bracket [{text}], neither an index, `*` nor member=value"
        ));
    };
    let unquote = |part: &[char]| -> Result<String, String> {
        let part: String = part.iter().collect();
        let inner = match part.strip_prefix('"').and_then(|p| p.strip_suffix('"')) {
            Some(inner) => inner.to_owned(),
            None if part.contains(['"', '[', ']', '.']) => {
                return Err(format!("has {part:?} in a bracket, which must be quoted"));
            }
            None => part,
        };
        if inner.is_empty() {
            return Err("has an empty member or value in a bracket".into());
        }
        Ok(inner)
    };
    Ok(PathStep::Filter(
        unquote(&content[..equals])?,
        unquote(&content[equals + 1..])?,
    ))
}

/// Whether the scalar `value` is the text `scalar` names.
fn scalar_is(value: &Value, scalar: &str) -> bool {
    match value {
        Value::String(text) => text == scalar,
        Value::Number(number) => {
            number.to_string() == scalar
                || scalar
                    .parse::<f64>()
                    .ok()
                    .zip(number.as_f64())
                    .is_some_and(|(a, b)| a == b)
        }
        Value::Bool(boolean) => boolean.to_string() == scalar,
        Value::Null => scalar == "null",
        _ => false,
    }
}

/// The values `path` reaches in `item`, in order; none when it reaches nothing.
pub(crate) fn values_at(item: &Value, path: &ValuePath) -> Vec<Value> {
    let mut current = vec![item.clone()];
    for step in &path.0 {
        let mut next = Vec::new();
        for value in current {
            match (step, value) {
                (PathStep::Member(name), Value::Object(mut members)) => {
                    if let Some(member) = members.remove(name) {
                        next.push(member);
                    }
                }
                (PathStep::Index(index), Value::Array(mut items)) => {
                    let length = i64::try_from(items.len()).unwrap_or(i64::MAX);
                    let at = if *index < 0 { length + index } else { *index };
                    if (0..length).contains(&at) {
                        next.push(items.swap_remove(usize::try_from(at).unwrap_or(0)));
                    }
                }
                (PathStep::Every, Value::Array(items)) => next.extend(items),
                (PathStep::Filter(member, scalar), Value::Array(items)) => {
                    next.extend(items.into_iter().filter(|element| {
                        element
                            .get(member)
                            .is_some_and(|found| scalar_is(found, scalar))
                    }))
                }
                _ => {}
            }
        }
        current = next;
    }
    current
}

/// Whether `path` is a relative path that stays below its root: not empty, not absolute, no
/// prefix, and no `..` that leaves the root.
pub(crate) fn stays_below(path: &str) -> bool {
    if path.is_empty() || path.contains('\0') || path.contains('\\') {
        return false;
    }
    let mut depth = 0usize;
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Whether `path` is a relative path of plain names: it [stays below](stays_below) its root, and
/// each of its components, split at `/`, is a name that is neither empty, `.` nor `..`, so no
/// `..` stands in it even where it would stay below.
pub(crate) fn plain_names(path: &str) -> bool {
    stays_below(path)
        && path
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

/// Whether the relative path `path` (segments joined by `/`) matches `pattern`: `*` within one
/// segment, `**` across any number of segments.
pub(crate) fn glob_matches(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| glob_matches(rest, &path[skip..])),
        Some((segment, rest)) => path.split_first().is_some_and(|(first, others)| {
            segment_matches(segment, first) && glob_matches(rest, others)
        }),
    }
}

/// Whether one path segment matches one pattern segment, `*` any run of characters.
fn segment_matches(pattern: &str, segment: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == segment;
    }
    let mut rest = segment;
    let last = parts.len() - 1;
    for (index, part) in parts.iter().enumerate() {
        if index == 0 {
            let Some(after) = rest.strip_prefix(part) else {
                return false;
            };
            rest = after;
        } else if index == last {
            return rest.len() >= part.len() && rest.ends_with(part);
        } else if let Some(found) = rest.find(part) {
            rest = &rest[found + part.len()..];
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glob_matches_within_and_across_segments() {
        let path = |text: &'static str| text.split('/').collect::<Vec<_>>();
        assert!(glob_matches(&path("a/*.yaml"), &path("a/b.yaml")));
        assert!(!glob_matches(&path("a/*.yaml"), &path("a/c/b.yaml")));
        assert!(glob_matches(&path("a/**/b.yaml"), &path("a/c/d/b.yaml")));
        assert!(glob_matches(&path("a/**/b.yaml"), &path("a/b.yaml")));
        assert!(glob_matches(&path("**"), &path("x/y")));
        assert!(glob_matches(&path("*-x*"), &path("a-xb")));
    }

    #[test]
    fn a_relative_path_stays_below_its_root_unless_its_dots_leave_it() {
        assert!(stays_below("a/b"));
        assert!(stays_below("a/../b"));
        assert!(!stays_below("../w"));
        assert!(!stays_below("a/../../w"));
        assert!(!stays_below("/etc/passwd"));
        assert!(!stays_below(""));
    }

    #[test]
    fn a_path_of_plain_names_holds_no_dot_component_at_all() {
        assert!(plain_names("findings.yaml"));
        assert!(plain_names("a/b/.hidden"));
        for path in [
            "", "/etc/x", "../x", "a/../b", "./a", "a/./b", "a//b", "a/", "a\\b",
        ] {
            assert!(!plain_names(path), "{path:?}");
        }
    }
}
