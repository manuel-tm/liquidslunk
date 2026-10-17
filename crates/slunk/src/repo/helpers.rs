//! What the repositories share: chunked `in (…)` lists, grouping, and
//! sea-query's values bound for rusqlite.

use std::collections::HashMap;
use std::hash::Hash;

use rusqlite::types::Value as SqlValue;
use sea_query::{Value, Values};

/// Stays well under SQLite's limit on bound parameters.
pub const CHUNK: usize = 500;

/// Splits items into chunks for `in (…)` lists.
pub fn chunks<T>(items: &[T]) -> impl Iterator<Item = &[T]> {
    items.chunks(CHUNK)
}

/// `?, ?, ?`: the placeholders of an `in (…)` list.
pub fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

/// Groups rows into a map of lists, keeping their order.
pub fn group_by<T, K: Eq + Hash, V>(
    rows: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
    value: impl Fn(T) -> V,
) -> HashMap<K, Vec<V>> {
    let mut map: HashMap<K, Vec<V>> = HashMap::new();
    for row in rows {
        map.entry(key(&row)).or_default().push(value(row));
    }
    map
}

/// sea-query's values, to bind with rusqlite. sea-query-rusqlite does this,
/// but it's built for an older rusqlite, and two bundled SQLites can't link
/// into one binary.
pub fn bind(values: Values) -> Vec<SqlValue> {
    values.into_iter().map(sql_value).collect()
}

fn sql_value(value: Value) -> SqlValue {
    let integer = |value: Option<i64>| value.map_or(SqlValue::Null, SqlValue::Integer);
    match value {
        Value::Bool(value) => integer(value.map(i64::from)),
        Value::TinyInt(value) => integer(value.map(i64::from)),
        Value::SmallInt(value) => integer(value.map(i64::from)),
        Value::Int(value) => integer(value.map(i64::from)),
        Value::BigInt(value) => integer(value),
        Value::TinyUnsigned(value) => integer(value.map(i64::from)),
        Value::SmallUnsigned(value) => integer(value.map(i64::from)),
        Value::Unsigned(value) => integer(value.map(i64::from)),
        Value::BigUnsigned(value) => integer(value.map(|value| value as i64)),
        Value::Float(value) => value.map_or(SqlValue::Null, |value| SqlValue::Real(value.into())),
        Value::Double(value) => value.map_or(SqlValue::Null, SqlValue::Real),
        Value::String(value) => value.map_or(SqlValue::Null, SqlValue::Text),
        Value::Char(value) => value.map_or(SqlValue::Null, |value| SqlValue::Text(value.into())),
        Value::Bytes(value) => value.map_or(SqlValue::Null, SqlValue::Blob),
        value => unreachable!("liquidslunk binds no {value:?}"),
    }
}
