//! Release formats and release media: slunk's lists repository. They have
//! the same columns, so they share it.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use sea_query::{Expr, ExprTrait, LikeExpr, Order, Query, SqliteQueryBuilder};

use super::helpers::{bind, chunks, placeholders};
use crate::Result;
use crate::catalog::{ListItem, ListKind, NameQuery, NameSort};
use crate::paging::like_pattern;

fn table(kind: ListKind) -> &'static str {
    match kind {
        ListKind::ReleaseFormats => "release_formats",
        ListKind::ReleaseMedia => "release_media",
    }
}

/// The table of releases each list's rows are on, and its column.
fn uses_table(kind: ListKind) -> (&'static str, &'static str) {
    match kind {
        ListKind::ReleaseFormats => ("release_formats_releases", "format_id"),
        ListKind::ReleaseMedia => ("release_media_releases", "medium_id"),
    }
}

/// A page, and how many match in all.
pub fn list(conn: &Connection, kind: ListKind, query: &NameQuery) -> Result<(Vec<ListItem>, i64)> {
    let mut select = Query::select();
    select.from(table(kind));
    if let Some(search) = &query.search {
        select.and_where(Expr::col("name").like(LikeExpr::new(like_pattern(search)).escape('\\')));
    }

    let (sql, values) = select
        .clone()
        .expr(Expr::cust("count(*)"))
        .build(SqliteQueryBuilder);
    let total = conn.query_row(&sql, params_from_iter(bind(values)), |row| row.get(0))?;

    let order = if query.sort.descending {
        Order::Desc
    } else {
        Order::Asc
    };
    let by = match query.sort.column {
        Some(NameSort::Name) => Expr::cust(r#""name" collate nocase"#),
        None => Expr::col("id"),
    };
    let (sql, values) = select
        .columns(["id", "name"])
        .order_by_expr(by, order)
        .limit(query.paging.limit as u64)
        .offset(query.paging.offset as u64)
        .build(SqliteQueryBuilder);
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(bind(values)), |row| {
            Ok(ListItem {
                kind,
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok((rows, total))
}

pub fn get(conn: &Connection, kind: ListKind, id: i64) -> Result<Option<ListItem>> {
    let sql = format!(
        r#"select "id", "name" from "{}" where "id" = ?"#,
        table(kind)
    );
    Ok(conn
        .query_row(&sql, [id], |row| {
            Ok(ListItem {
                kind,
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .optional()?)
}

pub fn insert(conn: &Connection, kind: ListKind, name: &str) -> Result<ListItem> {
    let sql = format!(
        r#"insert into "{}" ("name") values (?) returning "id", "name""#,
        table(kind)
    );
    Ok(conn.query_row(&sql, [name], |row| {
        Ok(ListItem {
            kind,
            id: row.get(0)?,
            name: row.get(1)?,
        })
    })?)
}

/// Renames a row. `None` if there's no such row.
pub fn update(conn: &Connection, kind: ListKind, id: i64, name: &str) -> Result<Option<ListItem>> {
    let sql = format!(
        r#"update "{}" set "name" = ? where "id" = ? returning "id", "name""#,
        table(kind)
    );
    Ok(conn
        .query_row(&sql, params![name, id], |row| {
            Ok(ListItem {
                kind,
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .optional()?)
}

pub fn delete(conn: &Connection, kind: ListKind, id: i64) -> Result<()> {
    conn.execute(
        &format!(r#"delete from "{}" where "id" = ?"#, table(kind)),
        [id],
    )?;
    Ok(())
}

/// How many releases have the format or medium: 0 for an unknown id.
pub fn uses(conn: &Connection, kind: ListKind, id: i64) -> Result<i64> {
    Ok(uses_of(conn, kind, &[id])?.get(&id).copied().unwrap_or(0))
}

/// How many releases have each format or medium. One with none is left
/// out.
pub fn uses_of(conn: &Connection, kind: ListKind, ids: &[i64]) -> Result<HashMap<i64, i64>> {
    let (releases, column) = uses_table(kind);
    let mut uses = HashMap::new();
    for chunk in chunks(ids) {
        let sql = format!(
            r#"select "{column}", count(*) from "{releases}" where "{column}" in ({}) group by "{column}""#,
            placeholders(chunk.len())
        );
        let mut statement = conn.prepare(&sql)?;
        for row in statement.query_map(params_from_iter(chunk), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })? {
            let (id, count) = row?;
            uses.insert(id, count);
        }
    }
    Ok(uses)
}
