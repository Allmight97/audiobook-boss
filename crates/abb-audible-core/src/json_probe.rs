use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStep<'a> {
    Key(&'a str),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedPathStep {
    Key(String),
    Index(usize),
}

impl From<PathStep<'_>> for OwnedPathStep {
    fn from(step: PathStep<'_>) -> Self {
        match step {
            PathStep::Key(key) => Self::Key(key.to_string()),
            PathStep::Index(index) => Self::Index(index),
        }
    }
}

/// The one tree walk: offers each object to `probe` before descending into its
/// values, depth first, so every reader shares one traversal order.
pub(crate) fn find_in_tree<'a, T>(
    value: &'a Value,
    probe: &mut impl FnMut(&'a Map<String, Value>, &[PathStep<'a>]) -> Option<T>,
) -> Option<T> {
    walk(value, &mut Vec::new(), probe)
}

fn walk<'a, T>(
    value: &'a Value,
    path: &mut Vec<PathStep<'a>>,
    probe: &mut impl FnMut(&'a Map<String, Value>, &[PathStep<'a>]) -> Option<T>,
) -> Option<T> {
    match value {
        Value::Object(map) => {
            if let Some(found) = probe(map, path) {
                return Some(found);
            }
            map.iter()
                .find_map(|(key, entry)| descend(entry, PathStep::Key(key), path, probe))
        }
        Value::Array(values) => values
            .iter()
            .enumerate()
            .find_map(|(index, entry)| descend(entry, PathStep::Index(index), path, probe)),
        _ => None,
    }
}

fn descend<'a, T>(
    entry: &'a Value,
    step: PathStep<'a>,
    path: &mut Vec<PathStep<'a>>,
    probe: &mut impl FnMut(&'a Map<String, Value>, &[PathStep<'a>]) -> Option<T>,
) -> Option<T> {
    path.push(step);
    let found = walk(entry, path, probe);
    path.pop();
    found
}

/// First string stored under `key` anywhere in the tree, empty or not.
pub fn find_first_string_for_key(value: &Value, key: &str) -> Option<String> {
    find_in_tree(value, &mut |map, _| {
        map.get(key).and_then(Value::as_str).map(str::to_string)
    })
}

pub(crate) fn find_first_bool_for_key(value: &Value, key: &str) -> Option<bool> {
    find_in_tree(value, &mut |map, _| map.get(key).and_then(Value::as_bool))
}

pub(crate) fn find_array_for_key<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    find_in_tree(value, &mut |map, _| map.get(key).and_then(Value::as_array))
}

/// Tries every key at an object before descending, skipping empty strings.
/// Sibling traversal order wins across subtrees; callers needing global key
/// priority search one key at a time.
pub(crate) fn find_nearest_non_empty_string_for_keys(
    value: &Value,
    keys: &[&str],
) -> Option<String> {
    find_nearest_non_empty_string_with_path(value, keys).map(|(found, _)| found)
}

pub(crate) fn find_nearest_non_empty_string_with_path(
    value: &Value,
    keys: &[&str],
) -> Option<(String, Vec<OwnedPathStep>)> {
    find_in_tree(value, &mut |map, path| {
        keys.iter().find_map(|key| {
            let found = map
                .get(*key)
                .and_then(Value::as_str)
                .filter(|found| !found.is_empty())?;
            let mut steps: Vec<OwnedPathStep> = path.iter().copied().map(Into::into).collect();
            steps.push(OwnedPathStep::Key((*key).to_string()));
            Some((found.to_string(), steps))
        })
    })
}

#[cfg(test)]
#[path = "json_probe_tests.rs"]
mod tests;
