//! Tag definitions and their values: slunk's tags repository. Each type has a
//! definition table, and a value table per owner, release or track (slunk's
//! design §4.3). The three types' tables have the same columns, so one
//! repository serves them.

use std::collections::HashMap;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sea_query::{Expr, Value};

use super::helpers::{chunks, group_by, placeholders};
use crate::Result;
use crate::paging::{Paging, like_pattern};
use crate::tagging::{
    TAG_TYPES, Tag, TagFilter, TagOwner, TagQuery, TagSort, TagType, TagUsage, TagValue,
    TagValueRow, ValueCount,
};

/// A type's definition table.
fn tags_table(kind: TagType) -> &'static str {
    match kind {
        TagType::Text => "text_tags",
        TagType::Number => "real_number_tags",
        TagType::Date => "date_tags",
    }
}

/// A type's value table, for releases or tracks.
fn value_table(owner: TagOwner, kind: TagType) -> &'static str {
    match (owner, kind) {
        (TagOwner::Release, TagType::Text) => "release_text_tags",
        (TagOwner::Release, TagType::Number) => "release_real_number_tags",
        (TagOwner::Release, TagType::Date) => "release_date_tags",
        (TagOwner::Track, TagType::Text) => "track_text_tags",
        (TagOwner::Track, TagType::Number) => "track_real_number_tags",
        (TagOwner::Track, TagType::Date) => "track_date_tags",
    }
}

/// A value table's owner column: `release_id` or `track_id`.
fn owner_column(owner: TagOwner) -> &'static str {
    match owner {
        TagOwner::Release => "release_id",
        TagOwner::Track => "track_id",
    }
}

fn owner_table(owner: TagOwner) -> &'static str {
    match owner {
        TagOwner::Release => "releases",
        TagOwner::Track => "tracks",
    }
}

/// The order types are listed in when names tie.
fn rank(kind: TagType) -> usize {
    TAG_TYPES
        .iter()
        .position(|&other| other == kind)
        .unwrap_or(0)
}

/// Every type's tags as one table, with `type` and `rank` columns.
fn all_tags(types: &[TagType]) -> String {
    types
        .iter()
        .map(|&kind| {
            format!(
                r#"select '{}' as "type", {} as "rank", "id", "name", "multiple" from "{}""#,
                kind.as_str(),
                rank(kind),
                tags_table(kind)
            )
        })
        .collect::<Vec<_>>()
        .join(" union all ")
}

/// A tag from a row of `type, id, name, multiple`.
fn tag_of(row: &Row) -> rusqlite::Result<Tag> {
    let kind: String = row.get(0)?;
    Ok(Tag {
        kind: TagType::named(&kind).expect("a tag type"),
        id: row.get(1)?,
        name: row.get(2)?,
        multiple: row.get::<_, i64>(3)? == 1,
    })
}

/// A tag of a known type, from a row of `id, name, multiple`.
fn typed_tag(kind: TagType) -> impl Fn(&Row) -> rusqlite::Result<Tag> {
    move |row| {
        Ok(Tag {
            kind,
            id: row.get(0)?,
            name: row.get(1)?,
            multiple: row.get::<_, i64>(2)? == 1,
        })
    }
}

/// A value as SQLite stores it: text, a real, or an integer for a date.
fn sql_value(value: &TagValue) -> SqlValue {
    match value {
        TagValue::Text(text) => SqlValue::Text(text.clone()),
        TagValue::Number(number) => SqlValue::Real(*number),
        TagValue::Date(epoch) => SqlValue::Integer(*epoch),
    }
}

/// A stored value, by its tag's type.
fn tag_value(kind: TagType, value: SqlValue) -> TagValue {
    match (kind, value) {
        (TagType::Text, SqlValue::Text(text)) => TagValue::Text(text),
        (TagType::Number, SqlValue::Real(number)) => TagValue::Number(number),
        (TagType::Number, SqlValue::Integer(number)) => TagValue::Number(number as f64),
        (TagType::Date, SqlValue::Integer(epoch)) => TagValue::Date(epoch),
        (TagType::Date, SqlValue::Real(epoch)) => TagValue::Date(epoch as i64),
        (kind, value) => unreachable!("a {} tag's value is {value:?}", kind.as_str()),
    }
}

/// Every type's tags when `query.kind` is `None`. A page, and how many match
/// in all.
pub fn list(conn: &Connection, query: &TagQuery) -> Result<(Vec<Tag>, i64)> {
    let types = match &query.kind {
        Some(kind) => std::slice::from_ref(kind),
        None => &TAG_TYPES[..],
    };
    let from = format!("({})", all_tags(types));
    let search = query.search.as_deref().map(like_pattern);
    let filter = if search.is_some() {
        r#"where "name" like ? escape '\'"#
    } else {
        ""
    };
    let total = conn.query_row(
        &format!("select count(*) as total from {from} {filter}"),
        params_from_iter(&search),
        |row| row.get(0),
    )?;

    let direction = if query.sort.descending { "desc" } else { "asc" };
    let by_name = match query.sort.column {
        Some(TagSort::Name) => format!(r#""name" collate nocase {direction}, "#),
        None => String::new(),
    };
    let sql = format!(
        r#"select "type", "id", "name", "multiple" from {from} {filter}
        order by {by_name}"id" {direction}, "rank"
        limit ? offset ?"#
    );
    let mut values: Vec<SqlValue> = search.into_iter().map(SqlValue::Text).collect();
    values.extend([
        SqlValue::Integer(query.paging.limit),
        SqlValue::Integer(query.paging.offset),
    ]);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(values), tag_of)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

pub fn get(conn: &Connection, kind: TagType, id: i64) -> Result<Option<Tag>> {
    let sql = format!(
        r#"select "id", "name", "multiple" from "{}" where "id" = ?"#,
        tags_table(kind)
    );
    Ok(conn.query_row(&sql, [id], typed_tag(kind)).optional()?)
}

/// The tags of these ids that exist, in no particular order.
pub fn get_many(conn: &Connection, kind: TagType, ids: &[i64]) -> Result<Vec<Tag>> {
    let mut found = Vec::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "id", "name", "multiple" from "{}" where "id" in ({})"#,
            tags_table(kind),
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for tag in statement.query_map(params_from_iter(chunk), typed_tag(kind))? {
            found.push(tag?);
        }
    }
    Ok(found)
}

/// Tags of any type with exactly this name, ignoring case.
pub fn find_by_name(conn: &Connection, name: &str) -> Result<Vec<Tag>> {
    let sql = format!(
        r#"select "type", "id", "name", "multiple" from ({})
        where "name" = ? collate nocase
        order by "rank""#,
        all_tags(&TAG_TYPES)
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([name], tag_of)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn insert(conn: &Connection, kind: TagType, name: &str, multiple: bool) -> Result<Tag> {
    let sql = format!(
        r#"insert into "{}" ("name", "multiple") values (?, ?) returning "id", "name", "multiple""#,
        tags_table(kind)
    );
    Ok(conn.query_row(&sql, params![name, i64::from(multiple)], typed_tag(kind))?)
}

/// Renames a tag, or switches it between single and multiple values. `None`
/// if there's no such tag.
pub fn update(
    conn: &Connection,
    kind: TagType,
    id: i64,
    name: &str,
    multiple: bool,
) -> Result<Option<Tag>> {
    let sql = format!(
        r#"update "{}" set "name" = ?, "multiple" = ? where "id" = ? returning "id", "name", "multiple""#,
        tags_table(kind)
    );
    Ok(conn
        .query_row(
            &sql,
            params![name, i64::from(multiple), id],
            typed_tag(kind),
        )
        .optional()?)
}

pub fn delete(conn: &Connection, kind: TagType, id: i64) -> Result<()> {
    conn.execute(
        &format!(r#"delete from "{}" where "id" = ?"#, tags_table(kind)),
        [id],
    )?;
    Ok(())
}

/// How many releases and tracks have a value.
pub fn usage(conn: &Connection, kind: TagType, id: i64) -> Result<TagUsage> {
    let count = |owner: TagOwner| -> Result<i64> {
        let sql = format!(
            r#"select count(distinct "{}") as count from "{}" where "tag_id" = ?"#,
            owner_column(owner),
            value_table(owner, kind)
        );
        Ok(conn.query_row(&sql, [id], |row| row.get(0))?)
    };
    Ok(TagUsage {
        releases: count(TagOwner::Release)?,
        tracks: count(TagOwner::Track)?,
    })
}

/// A text tag's values, the most used first, and how many there are.
/// Values compare ignoring case, so "Rock" and "rock" count as one.
pub fn values(
    conn: &Connection,
    id: i64,
    paging: Paging,
    search: Option<&str>,
) -> Result<(Vec<ValueCount>, i64)> {
    let pattern = search.map(like_pattern);
    let filter = if pattern.is_some() {
        r#"and "value" like ? escape '\'"#
    } else {
        ""
    };
    let both = format!(
        r#"(
        select "value", 1 as "release", 0 as "track" from "release_text_tags"
        where "tag_id" = ? {filter}
        union all
        select "value", 0, 1 from "track_text_tags"
        where "tag_id" = ? {filter}
      )"#
    );
    let mut values = Vec::new();
    for _ in 0..2 {
        values.push(SqlValue::Integer(id));
        values.extend(pattern.clone().map(SqlValue::Text));
    }
    let total = conn.query_row(
        &format!(r#"select count(distinct "value" collate nocase) as total from {both}"#),
        params_from_iter(&values),
        |row| row.get(0),
    )?;

    let sql = format!(
        r#"select "value", sum("release") as releases, sum("track") as tracks
        from {both}
        group by "value" collate nocase
        order by count(*) desc, "value" collate nocase
        limit ? offset ?"#
    );
    values.extend([
        SqlValue::Integer(paging.limit),
        SqlValue::Integer(paging.offset),
    ]);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(values), |row| {
            Ok(ValueCount {
                value: row.get(0)?,
                releases: row.get(1)?,
                tracks: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

/// Owners with several values, which keep a tag from being single-value, by
/// id, and how many there are.
pub fn multi_value_owners(
    conn: &Connection,
    owner: TagOwner,
    kind: TagType,
    id: i64,
    paging: Paging,
) -> Result<(Vec<i64>, i64)> {
    let column = owner_column(owner);
    let several = format!(
        r#"(
        select "{column}" as "id" from "{}"
        where "tag_id" = ?
        group by "{column}"
        having count(*) > 1
      )"#,
        value_table(owner, kind)
    );
    let total = conn.query_row(
        &format!("select count(*) as total from {several}"),
        [id],
        |row| row.get(0),
    )?;
    let mut statement = conn.prepare(&format!(
        r#"select "id" from {several} order by "id" limit ? offset ?"#
    ))?;
    let rows = statement
        .query_map(params![id, paging.limit, paging.offset], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

/// Values by owner: by tag name, ignoring case, then in the order they were
/// set.
pub fn values_of(
    conn: &Connection,
    owner: TagOwner,
    ids: &[i64],
) -> Result<HashMap<i64, Vec<TagValueRow>>> {
    let column = owner_column(owner);
    let mut rows: Vec<(i64, TagValueRow)> = Vec::new();
    for chunk in chunks(ids) {
        let parts: Vec<String> = TAG_TYPES
            .iter()
            .map(|&kind| {
                format!(
                    r#"select '{}' as "type", {} as "rank",
                    v."{column}" as "ownerId", v."tag_id" as "tagId",
                    v."value" as "value", t."name" as "name", v."id" as "valueId"
                    from "{}" v
                    join "{}" t on t."id" = v."tag_id"
                    where v."{column}" in ({})"#,
                    kind.as_str(),
                    rank(kind),
                    value_table(owner, kind),
                    tags_table(kind),
                    placeholders(chunk.len())
                )
            })
            .collect();
        let sql = format!(
            r#"select "type", "ownerId", "tagId", "value" from ({})
            order by "ownerId", "name" collate nocase, "rank", "tagId", "valueId""#,
            parts.join(" union all ")
        );
        let mut statement = conn.prepare(&sql)?;
        let found = statement.query_map(
            params_from_iter(chunk.iter().chain(chunk).chain(chunk)),
            |row| {
                let kind = TagType::named(&row.get::<_, String>(0)?).expect("a tag type");
                Ok((
                    row.get(1)?,
                    TagValueRow {
                        kind,
                        tag_id: row.get(2)?,
                        value: tag_value(kind, row.get(3)?),
                    },
                ))
            },
        )?;
        for row in found {
            rows.push(row?);
        }
    }
    Ok(group_by(rows, |(owner, _)| *owner, |(_, row)| row))
}

/// Replaces every tag value on a release or track. Rows are inserted in
/// order, so their ids keep the order they were given in.
pub fn set_values(
    conn: &Connection,
    owner: TagOwner,
    id: i64,
    values: &[TagValueRow],
) -> Result<()> {
    let column = owner_column(owner);
    for kind in TAG_TYPES {
        conn.execute(
            &format!(
                r#"delete from "{}" where "{column}" = ?"#,
                value_table(owner, kind)
            ),
            [id],
        )?;
    }
    for row in values {
        conn.execute(
            &format!(
                r#"insert into "{}" ("{column}", "tag_id", "value") values (?, ?, ?)"#,
                value_table(owner, row.kind)
            ),
            params![id, row.tag_id, sql_value(&row.value)],
        )?;
    }
    Ok(())
}

/// Adds a value to each owner that doesn't have it. With `replace`, their
/// other values of the tag go first. Returns how many owners changed.
pub fn add_value(
    conn: &Connection,
    owner: TagOwner,
    ids: &[i64],
    row: &TagValueRow,
    replace: bool,
) -> Result<i64> {
    let table = value_table(owner, row.kind);
    let column = owner_column(owner);
    let value = sql_value(&row.value);
    let mut changed = 0;
    for chunk in chunks(ids) {
        let ids = placeholders(chunk.len());
        if replace {
            // Same value, ignoring case for text, stays as it is
            let sql = format!(
                r#"delete from "{table}"
                where "{column}" in ({ids}) and "tag_id" = ? and "value" <> ?"#
            );
            let mut values: Vec<SqlValue> = chunk.iter().map(|&id| SqlValue::Integer(id)).collect();
            values.extend([SqlValue::Integer(row.tag_id), value.clone()]);
            conn.execute(&sql, params_from_iter(values))?;
        }
        // Owners that have the value already are skipped. (INSERT OR IGNORE
        // wouldn't do: the single-value trigger aborts before the index.)
        let sql = format!(
            r#"insert into "{table}" ("{column}", "tag_id", "value")
            select o."id", ?, ? from "{}" o
            where o."id" in ({ids})
              and not exists (
                select 1 from "{table}" v
                where v."{column}" = o."id" and v."tag_id" = ? and v."value" = ?
              )
            order by o."id""#,
            owner_table(owner)
        );
        let mut values = vec![SqlValue::Integer(row.tag_id), value.clone()];
        values.extend(chunk.iter().map(|&id| SqlValue::Integer(id)));
        values.extend([SqlValue::Integer(row.tag_id), value.clone()]);
        changed += conn.execute(&sql, params_from_iter(values))? as i64;
    }
    Ok(changed)
}

/// Removes a tag's values from each owner: every value, or just `value`.
/// Returns how many owners changed.
pub fn remove_values(
    conn: &Connection,
    owner: TagOwner,
    ids: &[i64],
    kind: TagType,
    tag_id: i64,
    value: Option<&TagValue>,
) -> Result<i64> {
    let table = value_table(owner, kind);
    let column = owner_column(owner);
    let mut changed = 0;
    for chunk in chunks(ids) {
        let matches = format!(
            r#""{column}" in ({}) and "tag_id" = ? {}"#,
            placeholders(chunk.len()),
            if value.is_some() {
                r#"and "value" = ?"#
            } else {
                ""
            }
        );
        let mut values: Vec<SqlValue> = chunk.iter().map(|&id| SqlValue::Integer(id)).collect();
        values.push(SqlValue::Integer(tag_id));
        values.extend(value.map(sql_value));
        let count: i64 = conn.query_row(
            &format!(
                r#"select count(distinct "{column}") as count from "{table}" where {matches}"#
            ),
            params_from_iter(&values),
            |row| row.get(0),
        )?;
        changed += count;
        conn.execute(
            &format!(r#"delete from "{table}" where {matches}"#),
            params_from_iter(&values),
        )?;
    }
    Ok(changed)
}

/// Whether the release or track exists.
pub fn owner_exists(conn: &Connection, owner: TagOwner, id: i64) -> Result<bool> {
    let sql = format!(r#"select 1 from "{}" where "id" = ?"#, owner_table(owner));
    Ok(conn.query_row(&sql, [id], |_| Ok(())).optional()?.is_some())
}

/// Releases' or tracks' titles, by id, to name them.
pub fn owner_titles(
    conn: &Connection,
    owner: TagOwner,
    ids: &[i64],
) -> Result<HashMap<i64, String>> {
    let mut titles = HashMap::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "id", "title" from "{}" where "id" in ({})"#,
            owner_table(owner),
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })? {
            let (id, title) = row?;
            titles.insert(id, title);
        }
    }
    Ok(titles)
}

/// A list filter on a tag's values, for releases or tracks (slunk's design
/// §7.6): `releases.id in (select release_id …)`, which uses the value
/// table's `(tag_id, value)` index.
pub fn tag_matches(owner: TagOwner, filter: &TagFilter) -> Expr {
    let mut conditions = vec![r#""tag_id" = ?"#];
    let mut values = vec![Value::from(filter.tag_id)];
    if let Some(equals) = &filter.equals {
        conditions.push(r#""value" = ?"#);
        values.push(equals.clone().into());
    }
    if let Some(contains) = &filter.contains {
        conditions.push(r#""value" like ? escape '\'"#);
        values.push(like_pattern(contains).into());
    }
    if let Some(min) = &filter.min {
        conditions.push(r#""value" >= ?"#);
        values.push(bound(min));
    }
    if let Some(max) = &filter.max {
        conditions.push(r#""value" <= ?"#);
        values.push(bound(max));
    }
    Expr::cust_with_values(
        format!(
            r#""{}"."id" in (select "{}" from "{}" where {})"#,
            owner_table(owner),
            owner_column(owner),
            value_table(owner, filter.kind),
            conditions.join(" and ")
        ),
        values,
    )
}

fn bound(value: &TagValue) -> Value {
    match value {
        TagValue::Text(text) => text.clone().into(),
        TagValue::Number(number) => (*number).into(),
        TagValue::Date(epoch) => (*epoch).into(),
    }
}
